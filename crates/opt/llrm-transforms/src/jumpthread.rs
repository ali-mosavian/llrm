//! A switch in a loop that the paths into it decide, entered from the end of each path at the case it decides:
//! LLVM's `DFAJumpThreading` and GCC's finite-state-machine threader (`tree-ssa-threadbackward.cc`).
//!
//! `state = step(state); switch (state)` in a loop: the switch reads a phi of the loop's header, and the value that comes
//! round is a constant on each path into the join that makes it (or constant from the preheader). Each such path is
//! given its own copy of the blocks from the join down to the switch, and the copy of the switch is a jump to the case
//! its constant picks: the dispatch is gone from that path. The copies cost code, as LLVM's and GCC's do, so a path
//! is taken only where its blocks hold `MAX_PATH` instructions or fewer and the whole function `MAX_COPIED`, and
//! never where the build is tuned for size.
//!
//! Only what this reads: a `switch` on a phi in a block of a loop (`p` in `H`), `H` reaching the switch's block `S`
//! through blocks each with the one before as its only predecessor, and `p`'s input from the loop being a constant,
//! or a phi in its own block `J` (a block ending in a jump to `H`) with a constant from the predecessor `P`.
//! The copies of `J..S` have `H`'s phis as the values they take on the path; a value made in the originals and read
//! outside them is made again by the copies, and an updater (`SsaUpdater`) joins the two where the paths meet.
//! The result is not a natural loop any more where several cases lead back: later loop passes skip it, as after
//! LLVM's.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::ssa::SsaUpdater;
use llrm_analysis::{cfg, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};

use crate::lcssa::arms;

/// GCC's `max-fsm-thread-path-insns`: instructions one path's copies may hold.
const MAX_PATH: usize = 100;
/// GCC's `max-fsm-thread-paths`, in instructions rather than paths: what one function may copy in all.
const MAX_COPIED: usize = 400;

/// `size`: code size outranks speed, and a path is never copied.
pub struct JumpThread {
    pub size: bool,
}

impl FunctionPass for JumpThread {
    fn name(&self) -> &'static str {
        "jumpthread"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        if self.size {
            return PreservedAnalyses::all();
        }
        let outer = std::rc::Rc::clone(analyses.outer());
        if !threaded(unit.context, unit.layout, unit.function, &outer) {
            return PreservedAnalyses::all();
        }
        // What the copies know of their state: a product by a constant, a compare of one. Only where something was copied.
        for pass in [&mut crate::fold::Fold as &mut dyn FunctionPass, &mut crate::algebraic::Algebraic { size: false }, &mut crate::decide::Decide, &mut crate::dead::Dead] {
            analyses.invalidate(&PreservedAnalyses::none());
            pass.run(unit, analyses);
        }
        PreservedAnalyses::none()
    }
}

/// One path found: the block its first edge leaves, the blocks copied, and the case it ends in.
struct Path {
    from: BlockId,
    blocks: Vec<BlockId>,
    target: BlockId,
}

/// Every switch's paths that can be threaded, a switch's all at once within the limits; whether any was.
pub fn threaded(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &Outer) -> bool {
    // The common function has no switch: nothing is found, or analysed, for it.
    if !function.walk().any(|(_, inst)| function.instruction(inst).opcode == Opcode::Switch) {
        return false;
    }
    let mut copied = 0;
    let mut changed = false;
    while let Some(paths) = found(context, layout, function, outer, copied) {
        llrm_support::debug!("jumpthread", "{} paths, {} instructions copied so far", paths.len(), copied);
        copied += copy(context, function, &paths);
        changed = true;
    }
    if changed {
        cfg::_unreachable(context, function);
    }
    changed
}

/// The paths of the first switch that has any, within what is left of `MAX_COPIED`.
fn found(context: &mut Context, layout: &DataLayout, function: &Function, outer: &Outer, copied: usize) -> Option<Vec<Path>> {
    let shape = cfg::Shape::of(function);
    let unit = memory::Unit::within(context, layout, function, outer).with_shape(&shape);
    for loop_ in &shape.loops {
        for &at in &loop_.body {
            let block = cfg::block(at);
            let Some(last) = function.terminator(block) else { continue };
            if function.instruction(last).opcode != Opcode::Switch {
                continue;
            }
            let Operand::Value(state) = function.instruction(last).operands[0] else { continue };
            let Some(chain) = chain_to_phi(function, loop_, block, state) else { continue };
            let header = chain[0];
            let ValueDef::Instruction(phi) = function.value(state).def else { continue };
            let mut paths = Vec::new();
            let mut spent = copied;
            for (value, from) in arms(function, phi) {
                // The constant on each way in, and the blocks copied ahead of the chain.
                let ways = resolved(function, &unit, value, from, 4);
                for (source, prefix, number) in ways {
                    let Some(target) = destination(function, &unit, last, number) else { continue };
                    let blocks: Vec<BlockId> = prefix.into_iter().chain(chain.iter().copied()).collect();
                    if blocks.contains(&source) {
                        continue;
                    }
                    if !blocks.iter().all(|&one| copyable(function, one)) {
                        continue;
                    }
                    let size: usize = blocks.iter().map(|&one| function.block(one).instructions().len()).sum();
                    if size > MAX_PATH || spent + size > MAX_COPIED {
                        continue;
                    }
                    spent += size;
                    paths.push(Path { from: source, blocks, target });
                }
            }
            if !paths.is_empty() {
                return Some(paths);
            }
        }
    }
    None
}

/// Whether `block` may be copied: no call that unwinds to a handler (a second landing pad is another handler), no landing pad,
/// no stack object (a copy is another frame object), no call that may not be duplicated.
fn copyable(function: &Function, block: BlockId) -> bool {
    function.block(block).instructions().iter().all(|&inst| {
        let instruction = function.instruction(inst);
        match &instruction.opcode {
            Opcode::Invoke(_) | Opcode::LandingPad { .. } | Opcode::Resume | Opcode::Alloca { .. } => false,
            Opcode::Call(info) => !llrm_mir::memory::has(&info.attrs, "noduplicate"),
            _ => true,
        }
    })
}

/// The ways `value` is a constant where it is read at the end of `from`: each the block its way leaves, the blocks
/// from there on that make it (phis, one after another), and the constant.
fn resolved(function: &Function, unit: &memory::Unit, value: Operand, from: BlockId, depth: usize) -> Vec<(BlockId, Vec<BlockId>, u128)> {
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
        // Where the way leaves a block by two edges to `from`, it is not one way.
        if function.terminator(source).is_some_and(|end| function.instruction(end).operands.iter().filter(|one| **one == Operand::Block(from)).count() != 1) {
            continue;
        }
        for (leaves, mut blocks, number) in resolved(function, unit, input, source, depth - 1) {
            blocks.push(from);
            out.push((leaves, blocks, number));
        }
    }
    out
}

/// The blocks from the one holding `state` as a phi to `block`, each the only successor-by-predecessor of the one
/// before; `None` where `state` is no phi of a block of the loop that reaches `block` so.
fn chain_to_phi(function: &Function, loop_: &Loop, block: BlockId, state: ValueId) -> Option<Vec<BlockId>> {
    let ValueDef::Instruction(phi) = function.value(state).def else { return None };
    if function.instruction(phi).opcode != Opcode::Phi {
        return None;
    }
    let header = function.parent(phi)?;
    let mut chain = vec![block];
    while *chain.last()? != header {
        let last = *chain.last()?;
        let [only] = function.predecessors(last)[..] else { return None };
        if !loop_.body.contains(&cfg::id(only)) || chain.contains(&only) || chain.len() > 8 {
            return None;
        }
        chain.push(only);
    }
    chain.reverse();
    Some(chain)
}

/// Where the switch `last` goes for `number`.
fn destination(function: &Function, unit: &memory::Unit, last: InstId, number: u128) -> Option<BlockId> {
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

/// What one path's copies are: its blocks, and the value each original value is in them.
struct Copies {
    at: BTreeMap<BlockId, BlockId>,
    map: BTreeMap<ValueId, Operand>,
}

/// Every path's blocks copied, each path's first copy reached from the block its way in leaves, its last jumping to the
/// case it ends in; the instructions copied.
fn copy(context: &mut Context, function: &mut Function, paths: &[Path]) -> usize {
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
            // The way in: a phi takes what it takes from the block before on the path.
            let before = if index == 0 { path.from } else { originals[index - 1] };
            for inst in function.block(block).instructions().to_vec() {
                let instruction = function.instruction(inst).clone();
                if instruction.opcode == Opcode::Phi {
                    // A phi of the path's one way in: the copy has one input, the block before it on the path. A value
                    // it names that the originals make is made again on the paths that lead here, and the updater below
                    // reads it where it is read.
                    let taken = arms(function, inst).into_iter().find(|(_, from)| *from == before).expect("an input from the path");
                    let way = if index == 0 { path.from } else { clones[index - 1] };
                    let copy = function.create_instruction(Opcode::Phi, instruction.ty, vec![mapped(taken.0, &map), Operand::Block(way)], Flags::default(), None);
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
            // The jump on: along the path, the others as they were; the last is a jump to its target.
            let end = function.terminator(block).expect("a terminated block");
            let terminator = function.instruction(end).clone();
            let void = terminator.ty;
            if block == last {
                let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(path.target)], Flags::default(), None);
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
                let jump = function.create_instruction(terminator.opcode.clone(), void, operands, terminator.flags, None);
                function.insert(jump, Position::End(clone)).expect("a new block");
            }
        }
        // Each block the copies leave for gets an input from the copy that leaves.
        for (&block, &clone) in &at {
            let leaving: Vec<BlockId> = function.successors(clone).into_iter().filter(|to| !at.values().any(|one| one == to)).collect();
            for to in leaving {
                for phi in function.block(to).instructions().to_vec() {
                    if function.instruction(phi).opcode != Opcode::Phi {
                        break;
                    }
                    if let Some(taken) = arms(function, phi).into_iter().find(|(_, from)| *from == block) {
                        let mut inputs = function.instruction(phi).operands.clone();
                        inputs.extend([mapped(taken.0, &map), Operand::Block(clone)]);
                        function.set_operands(phi, inputs);
                    }
                }
            }
        }
        // The way in is the copy's now: the first original's phis lose it.
        let first = originals[0];
        let entry = function.terminator(path.from).expect("a terminated block");
        let redirected = function.instruction(entry).operands.iter().map(|&one| if one == Operand::Block(first) { Operand::Block(clones[0]) } else { one }).collect();
        function.set_operands(entry, redirected);
        for phi in function.block(first).instructions().to_vec() {
            if function.instruction(phi).opcode != Opcode::Phi {
                break;
            }
            let kept: Vec<Operand> = function.instruction(phi).operands.chunks(2).filter(|pair| pair[1] != Operand::Block(path.from)).flatten().copied().collect();
            function.set_operands(phi, kept);
        }
        made.push(Copies { at, map });
    }
    // A value made in the originals and read beyond them is made by each copy too, and a copy reads what the originals made
    // before it, in the iteration before: one updater joins the definitions where the paths meet.
    let originals: BTreeSet<BlockId> = paths.iter().flat_map(|path| path.blocks.iter().copied()).collect();
    for &block in &originals {
        for inst in function.block(block).instructions().to_vec() {
            let Some(result) = function.instruction(inst).result else { continue };
            // Read beyond them: by an instruction outside them, or by a phi from a block outside them.
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
                    at.is_some_and(|block| !originals.contains(&block))
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
