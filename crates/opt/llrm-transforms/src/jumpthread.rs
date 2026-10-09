//! A switch in a loop that the paths into it decide, entered from the end of
//! each path at the case it decides: LLVM's `DFAJumpThreading` and GCC's
//! finite-state-machine threader (`tree-ssa-threadbackward.cc`).
//!
//! `state = step(state); switch (state)` in a loop: the switch reads a phi of
//! the loop's header, and the value that comes round is a constant on each path
//! into the join that makes it (or constant from the preheader). Each such path
//! is given its own copy of the blocks from the join down to the switch, and
//! the copy of the switch is a jump to the case its constant picks: the
//! dispatch is gone from that path. The copies cost code, as LLVM's and GCC's
//! do, so a path is taken only where its blocks hold `MAX_PATH` instructions or
//! fewer and the whole function `MAX_COPIED`, and never where the build is
//! tuned for size, except the branch whose copies hold nothing.
//!
//! Only what this reads: a `switch` on a phi in a block of a loop (`p` in `H`),
//! `H` reaching the switch's block `S` through blocks each with the one before
//! as its only predecessor, and `p`'s input from the loop being a constant,
//! or a phi in its own block `J` (a block ending in a jump to `H`) with a
//! constant from the predecessor `P`. The copies of `J..S` have `H`'s phis as
//! the values they take on the path; a value made in the originals and read
//! outside them is made again by the copies, and an updater (`SsaUpdater`)
//! joins the two where the paths meet. The result is not a natural loop any
//! more where several cases lead back: later loop passes skip it, as after
//! LLVM's.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::ssa::SsaUpdater;
use llrm_analysis::{cfg, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};

use crate::lcssa::arms;

/// GCC's `max-fsm-thread-path-insns`: instructions one path's copies may hold.
const MAX_PATH: usize = 100;
/// GCC's `max-jump-thread-duplication-stmts` (params.opt:589): instructions a
/// branch's path may copy besides the phis and the compare the threading kills.
/// Tuned for size, GCC threads only where it kills every statement of the block
/// (`tree-ssa-threadupdate.cc:2077`): none.
const BRANCH_PATH: usize = 15;
/// GCC's `fsm-scale-path-stmts` (params.opt:165): `profitable_path_p` rejects a
/// path of `n` instructions where `n * 2 >= BRANCH_PATH`.
const PATH_SCALE: usize = 2;
/// GCC's `max-fsm-thread-paths`, in instructions rather than paths: what one
/// function may copy in all.
const MAX_COPIED: usize = 400;

/// `size`: code size outranks speed, and a path is never copied.
pub struct JumpThread {
    pub size: bool,
}

impl FunctionPass for JumpThread {
    fn name(&self) -> &'static str {
        "jumpthread"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if !threaded(unit.context, unit.layout, unit.function, &outer, self.size) {
            return PreservedAnalyses::all();
        }
        // What the copies know of their state: a product by a constant, a
        // compare of one. Only where something was copied.
        for pass in [
            &mut crate::fold::Fold as &mut dyn FunctionPass,
            &mut crate::algebraic::Algebraic { size: false },
            &mut crate::decide::Decide,
            &mut crate::dead::Dead,
        ] {
            analyses.invalidate(&PreservedAnalyses::none());
            pass.run(unit, analyses);
        }
        PreservedAnalyses::none()
    }
}

/// One path found: the block its first edge leaves, the blocks copied, and the
/// case it ends in.
struct Path {
    from: BlockId,
    blocks: Vec<BlockId>,
    target: BlockId,
}

/// Every switch's paths that can be threaded, a switch's all at once within the
/// limits; whether any was.
pub fn threaded(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    size: bool,
) -> bool {
    // The common function has nothing to thread: no analysis is made for it.
    let unit = memory::Unit::within(context, layout, function, outer);
    let any = function
        .walk()
        .any(
            |(_, inst)| function.instruction(inst).opcode.is_terminator()
                && decided(function, &unit, inst).is_some_and(|(state, _)| {
                    matches!(
                        function.value(state).def,
                        ValueDef::Instruction(phi) if function.instruction(phi).opcode == Opcode::Phi
                    )
                }),
        );
    if !any {
        return false;
    }
    let mut copied = 0;
    let mut changed = false;
    // The paths of the branches are those of the function as it came: a branch
    // the copies made is not threaded again, or a loop would be peeled one
    // trip at a time. GCC registers its threads once and applies them together.
    let mut rounds = 0;
    while let Some(paths) = found(context, layout, function, outer, copied, size, rounds == 0) {
        rounds += 1;
        llrm_support::debug!("jumpthread", "{} paths, {} instructions copied so far", paths.len(), copied);
        // A path of phis and a branch copies no instruction and still spends
        // the budget: or a loop of them never ends.
        copied += copy(context, function, &paths).max(paths.len());
        changed = true;
    }
    if changed {
        cfg::_unreachable(context, function);
    }
    changed
}

/// The paths of the first switch or branch that has any, within what is left of
/// `MAX_COPIED`.
fn found(
    context: &mut Context,
    layout: &DataLayout,
    function: &Function,
    outer: &Outer,
    copied: usize,
    size: bool,
    branches: bool,
) -> Option<Vec<Path>> {
    let shape = cfg::Shape::of(function);
    let unit = memory::Unit::within(context, layout, function, outer).with_shape(&shape);
    let blocks: Vec<BlockId> = function
        .walk()
        .filter(|&(_, inst)| function.instruction(inst).opcode.is_terminator())
        .map(|(block, _)| block)
        .collect();
    for block in blocks {
        let Some(last) = function.terminator(block) else { continue };
        // The innermost loop holding the block, the one a chain of blocks to
        // the state's phi stays within.
        let loop_ =
            shape.loops.iter().filter(|one| one.body.contains(&cfg::id(block))).min_by_key(|one| one.body.len());
        // A switch is the state machine of a loop; a branch on a constant a
        // path decides is threaded wherever it is.
        let Some((state, decide)) = decided(function, &unit, last) else { continue };
        let is_switch = function.instruction(last).opcode == Opcode::Switch;
        if is_switch && (size || loop_.is_none()) || !is_switch && !branches {
            continue;
        }
        let Some(chain) = chain_to_phi(function, loop_, block, state) else { continue };
        let ValueDef::Instruction(phi) = function.value(state).def else { continue };
        let limit = match (is_switch, size) {
            (true, _) => MAX_PATH,
            (false, false) => (BRANCH_PATH - 1) / PATH_SCALE,
            (false, true) => 0,
        };
        // The compare of the branch is decided by the threading, and is not a
        // copy.
        let killed = match function.instruction(last).operands.first() {
            Some(&Operand::Value(condition)) if !is_switch => match function.value(condition).def {
                ValueDef::Instruction(made) if matches!(function.instruction(made).opcode, Opcode::ICmp(_)) => {
                    Some(made)
                }
                _ => None,
            },
            _ => None,
        };
        let mut paths = Vec::new();
        let mut spent = copied;
        for (value, from) in arms(function, phi) {
            // The constant on each way in, and the blocks copied ahead of the
            // chain.
            let ways = resolved(function, &unit, value, from, 4);
            for (source, prefix, number) in ways {
                let Some(target) = decide(number) else { continue };
                let blocks: Vec<BlockId> = prefix.into_iter().chain(chain.iter().copied()).collect();
                if blocks.contains(&source) {
                    continue;
                }
                if !blocks.iter().all(|&one| copyable(function, one)) {
                    continue;
                }
                // Into a loop through its header from outside is a rotation,
                // which `Rotate` makes where it pays: GCC
                // allows it only for the idioms of `thread_through_loop_header`
                // (tree-ssa-threadupdate.cc:1712), and not before the loop
                // passes are done.
                if !is_switch
                    && blocks.iter().any(|&one| {
                        shape.loops.iter().any(|l| l.header == cfg::id(one) && !l.body.contains(&cfg::id(source)))
                    })
                {
                    continue;
                }
                let copies: usize = if is_switch {
                    blocks.iter().map(|&one| function.block(one).instructions().len()).sum()
                } else {
                    // Besides the phis, the branch and its compare, what the
                    // copies hold.
                    blocks
                        .iter()
                        .flat_map(|&one| function.block(one).instructions().iter().copied())
                        .filter(|&inst| {
                            function.instruction(inst).opcode != Opcode::Phi
                                && !function.instruction(inst).opcode.is_terminator()
                                && Some(inst) != killed
                        })
                        .count()
                };
                // GCC counts the phis of a block with several predecessors and
                // successors (`tree-ssa-threadbackward.cc`,
                // possibly_profitable_path_p): each is a phi at the
                // points where the copies rejoin the originals, and a move
                // there. The state's own phi dies.
                let phis: usize = if is_switch {
                    0
                } else {
                    blocks
                        .iter()
                        .filter(|&&one| function.predecessors(one).len() > 1 && function.successors(one).len() > 1)
                        .map(|&one| {
                            function
                                .block(one)
                                .instructions()
                                .iter()
                                .filter(|&&inst| function.instruction(inst).opcode == Opcode::Phi && inst != phi)
                                .count()
                        })
                        .sum()
                };
                llrm_support::debug!("jumpthread", "path from {:?}: {} copies, {} phis", source, copies, phis);
                let copies = copies + phis;
                if copies > limit || spent + copies > MAX_COPIED {
                    continue;
                }
                // A path of phis and a branch still costs a block, or the
                // budget would never run out.
                spent += copies.max(1);
                paths.push(Path { from: source, blocks, target });
            }
        }
        if !paths.is_empty() {
            return Some(paths);
        }
    }
    None
}

/// What the terminator `last` decides from a phi: the phi's value, and where
/// control goes for a constant of it. A switch on the phi; a branch on an `i1`
/// phi; a branch on a compare of the phi with a constant.
fn decided<'a>(
    function: &'a Function,
    unit: &'a memory::Unit,
    last: InstId,
) -> Option<(ValueId, Box<dyn Fn(u128) -> Option<BlockId> + 'a>)> {
    let terminator = function.instruction(last);
    let Operand::Value(condition) = *terminator.operands.first()? else { return None };
    match terminator.opcode {
        Opcode::Switch => Some((condition, Box::new(move |number| destination(function, unit, last, number)))),
        Opcode::Br if terminator.operands.len() == 3 => {
            let (Operand::Block(yes), Operand::Block(no)) = (terminator.operands[1], terminator.operands[2]) else {
                return None;
            };
            let ValueDef::Instruction(made) = function.value(condition).def else { return None };
            let compare = function.instruction(made);
            match compare.opcode {
                Opcode::Phi => Some((condition, Box::new(move |number| Some(if number & 1 == 1 { yes } else { no })))),
                Opcode::ICmp(predicate) => {
                    let Operand::Value(state) = compare.operands[0] else { return None };
                    let constant = unit.int_constant(compare.operands[1])?;
                    let bits = unit.int_bits(compare.operands[0])?;
                    Some((
                        state,
                        Box::new(move |number| Some(if holds(predicate, bits, number, constant) { yes } else { no })),
                    ))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// `left predicate right` for two integers of `bits` bits.
fn holds(
    predicate: IntPredicate,
    bits: u32,
    left: u128,
    right: u128,
) -> bool {
    let mask = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
    let (left, right) = (left & mask, right & mask);
    let signed = |one: u128| if bits >= 128 { one as i128 } else { ((one << (128 - bits)) as i128) >> (128 - bits) };
    match predicate {
        IntPredicate::Eq => left == right,
        IntPredicate::Ne => left != right,
        IntPredicate::Ugt => left > right,
        IntPredicate::Uge => left >= right,
        IntPredicate::Ult => left < right,
        IntPredicate::Ule => left <= right,
        IntPredicate::Sgt => signed(left) > signed(right),
        IntPredicate::Sge => signed(left) >= signed(right),
        IntPredicate::Slt => signed(left) < signed(right),
        IntPredicate::Sle => signed(left) <= signed(right),
    }
}

/// Whether `block` may be copied: no call that unwinds to a handler (a second
/// landing pad is another handler), no landing pad, no stack object (a copy is
/// another frame object), no call that may not be duplicated.
fn copyable(
    function: &Function,
    block: BlockId,
) -> bool {
    function
        .block(block)
        .instructions()
        .iter()
        .all(
            |&inst| {
                let instruction = function.instruction(inst);
                match &instruction.opcode {
                    Opcode::Invoke(_) | Opcode::LandingPad { .. } | Opcode::Resume | Opcode::Alloca { .. } => false,
                    Opcode::Call(info) => !llrm_mir::memory::has(&info.attrs, "noduplicate"),
                    _ => true,
                }
            },
        )
}

/// The ways `value` is a constant where it is read at the end of `from`: each
/// the block its way leaves, the blocks from there on that make it (phis, one
/// after another), and the constant.
fn resolved(
    function: &Function,
    unit: &memory::Unit,
    value: Operand,
    from: BlockId,
    depth: usize,
) -> Vec<(BlockId, Vec<BlockId>, u128)> {
    if let Some(number) = unit.int_constant(value) {
        return vec![(from, Vec::new(), number)];
    }
    let Operand::Value(made) = value else { return Vec::new() };
    let ValueDef::Instruction(phi) = function.value(made).def else { return Vec::new() };
    if depth == 0 || function.instruction(phi).opcode != Opcode::Phi || function.parent(phi) != Some(from) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (input, source) in arms(function, phi) {
        // Where the way leaves a block by two edges to `from`, it is not one
        // way.
        if function.terminator(source).is_some_and(|end| {
            function.instruction(end).operands.iter().filter(|one| **one == Operand::Block(from)).count() != 1
        }) {
            continue;
        }
        for (leaves, mut blocks, number) in resolved(function, unit, input, source, depth - 1) {
            blocks.push(from);
            out.push((leaves, blocks, number));
        }
    }
    out
}

/// The blocks from the one holding `state` as a phi to `block`, each the only
/// successor-by-predecessor of the one before; `None` where `state` is no phi
/// of a block of the loop that reaches `block` so.
fn chain_to_phi(
    function: &Function,
    loop_: Option<&Loop>,
    block: BlockId,
    state: ValueId,
) -> Option<Vec<BlockId>> {
    let ValueDef::Instruction(phi) = function.value(state).def else { return None };
    if function.instruction(phi).opcode != Opcode::Phi {
        return None;
    }
    let header = function.parent(phi)?;
    let mut chain = vec![block];
    while *chain.last()? != header {
        let last = *chain.last()?;
        let [only] = function.predecessors(last)[..] else { return None };
        if loop_.is_some_and(|one| !one.body.contains(&cfg::id(only))) || chain.contains(&only) || chain.len() > 8 {
            return None;
        }
        chain.push(only);
    }
    chain.reverse();
    Some(chain)
}

/// Where the switch `last` goes for `number`.
fn destination(
    function: &Function,
    unit: &memory::Unit,
    last: InstId,
    number: u128,
) -> Option<BlockId> {
    let switch = function.instruction(last);
    let Operand::Block(default) = switch.operands[1] else { return None };
    for pair in switch.operands[2..].chunks(2) {
        let [case, Operand::Block(target)] = pair else { return None };
        if unit.int_constant(*case)? == number {
            return Some(*target);
        }
    }
    Some(default)
}

/// What one path's copies are: its blocks, and the value each original value is
/// in them.
struct Copies {
    at: BTreeMap<BlockId, BlockId>,
    map: BTreeMap<ValueId, Operand>,
}

/// Every path's blocks copied, each path's first copy reached from the block
/// its way in leaves, its last jumping to the case it ends in; the instructions
/// copied.
fn copy(
    context: &mut Context,
    function: &mut Function,
    paths: &[Path],
) -> usize {
    let mut count = 0;
    let mut made = Vec::new();
    for path in paths {
        let originals = &path.blocks;
        let last = *originals.last().expect("a path has a block");
        let mut after = path.from;
        let clones: Vec<BlockId> = originals
            .iter()
            .map(|_| {
                let clone = function.create_block(None);
                function.insert_block(clone, Some(after)).expect("a block after the path");
                after = clone;
                clone
            })
            .collect();
        let at: BTreeMap<BlockId, BlockId> = originals.iter().copied().zip(clones.iter().copied()).collect();
        let mut map: BTreeMap<ValueId, Operand> = BTreeMap::new();
        let mapped = |one: Operand, map: &BTreeMap<ValueId, Operand>| match one {
            Operand::Value(value) => map.get(&value).copied().unwrap_or(one),
            other => other,
        };
        for (index, &block) in originals.iter().enumerate() {
            let clone = clones[index];
            // The way in: a phi takes what it takes from the block before on
            // the path.
            let before = if index == 0 { path.from } else { originals[index - 1] };
            for inst in function.block(block).instructions().to_vec() {
                let instruction = function.instruction(inst).clone();
                if instruction.opcode == Opcode::Phi {
                    // A phi of the path's one way in: the copy has one input,
                    // the block before it on the path. A value
                    // it names that the originals make is made again on the
                    // paths that lead here, and the updater below
                    // reads it where it is read.
                    let taken = arms(function, inst)
                        .into_iter()
                        .find(|(_, from)| *from == before)
                        .expect("an input from the path");
                    let way = if index == 0 { path.from } else { clones[index - 1] };
                    let copy = function.create_instruction(
                        Opcode::Phi,
                        instruction.ty,
                        vec![mapped(taken.0, &map), Operand::Block(way)],
                        Flags::default(),
                        None,
                    );
                    function.insert(copy, Position::End(clone)).expect("a new block");
                    if let (Some(result), Some(made)) = (instruction.result, function.instruction(copy).result) {
                        map.insert(result, Operand::Value(made));
                    }
                    continue;
                }
                if instruction.opcode.is_terminator() {
                    continue;
                }
                let copy = function.clone_instruction(inst);
                let operands = instruction.operands.iter().map(|&one| mapped(one, &map)).collect();
                function.set_operands(copy, operands);
                function.insert(copy, Position::End(clone)).expect("a new block");
                if let (Some(result), Some(made)) = (instruction.result, function.instruction(copy).result) {
                    map.insert(result, Operand::Value(made));
                }
                count += 1;
            }
            // The jump on: along the path, the others as they were; the last is
            // a jump to its target.
            let end = function.terminator(block).expect("a terminated block");
            let terminator = function.instruction(end).clone();
            let void = terminator.ty;
            if block == last {
                let jump = function.create_instruction(
                    Opcode::Br,
                    void,
                    vec![Operand::Block(path.target)],
                    Flags::default(),
                    None,
                );
                function.insert(jump, Position::End(clone)).expect("a new block");
            } else {
                let next = originals[index + 1];
                let operands = terminator
                    .operands
                    .iter()
                    .map(|&one| match one {
                        Operand::Block(to) if to == next => Operand::Block(at[&to]),
                        Operand::Value(_) => mapped(one, &map),
                        other => other,
                    })
                    .collect();
                let jump =
                    function.create_instruction(terminator.opcode.clone(), void, operands, terminator.flags, None);
                function.insert(jump, Position::End(clone)).expect("a new block");
            }
        }
        // Each block the copies leave for gets an input from the copy that
        // leaves.
        for (&block, &clone) in &at {
            let leaving: Vec<BlockId> =
                function.successors(clone).into_iter().filter(|to| !at.values().any(|one| one == to)).collect();
            for to in leaving {
                for phi in function.block(to).instructions().to_vec() {
                    if function.instruction(phi).opcode != Opcode::Phi {
                        break;
                    }
                    // One input for each edge the block has to it.
                    let mut inputs = function.instruction(phi).operands.clone();
                    for (value, _) in arms(function, phi).into_iter().filter(|(_, from)| *from == block) {
                        inputs.extend([mapped(value, &map), Operand::Block(clone)]);
                    }
                    function.set_operands(phi, inputs);
                }
            }
        }
        // The way in is the copy's now: the first original's phis lose it.
        let first = originals[0];
        let entry = function.terminator(path.from).expect("a terminated block");
        let redirected = function
            .instruction(entry)
            .operands
            .iter()
            .map(|&one| if one == Operand::Block(first) { Operand::Block(clones[0]) } else { one })
            .collect();
        function.set_operands(entry, redirected);
        for phi in function.block(first).instructions().to_vec() {
            if function.instruction(phi).opcode != Opcode::Phi {
                break;
            }
            let kept: Vec<Operand> = function
                .instruction(phi)
                .operands
                .chunks(2)
                .filter(|pair| pair[1] != Operand::Block(path.from))
                .flatten()
                .copied()
                .collect();
            function.set_operands(phi, kept);
        }
        made.push(Copies { at, map });
    }
    // A value made in the originals and read beyond them is made by each copy
    // too, and a copy reads what the originals made before it, in the
    // iteration before: one updater joins the definitions where the paths meet.
    let originals: BTreeSet<BlockId> = paths.iter().flat_map(|path| path.blocks.iter().copied()).collect();
    for &block in &originals {
        for inst in function.block(block).instructions().to_vec() {
            let Some(result) = function.instruction(inst).result else { continue };
            // Read anywhere but its own block: an original block is reached
            // from the copies too, and reads what meets there.
            let outside: Vec<_> = function
                .users(result)
                .iter()
                .cloned()
                .filter(|one| {
                    let user = function.instruction(one.user);
                    let at = match (user.opcode == Opcode::Phi).then(|| user.operands[one.index as usize + 1]) {
                        Some(Operand::Block(from)) => Some(from),
                        _ => function.parent(one.user),
                    };
                    at.is_some_and(|at| at != block)
                })
                .collect();
            if outside.is_empty() {
                continue;
            }
            let mut updater = SsaUpdater::new(function.value(result).ty, None);
            updater.add_available_value(block, Operand::Value(result));
            for copies in &made {
                if let (Some(&clone), Some(&other)) = (copies.at.get(&block), copies.map.get(&result)) {
                    updater.add_available_value(clone, other);
                }
            }
            for one in outside {
                updater.rewrite_use(context, function, one);
            }
        }
    }
    count
}

#[cfg(test)]
#[path = "jumpthread_tests.rs"]
mod tests;
