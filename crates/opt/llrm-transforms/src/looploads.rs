//! A value a loop's last trip left in memory is carried to the next trip in a
//! register, so the next trip does not load it again: LLVM's
//! LoopLoadElimination (a store read one trip later) and the loop case of
//! GVN's load PRE (a load read one trip later), gcc's PRE of a load across
//! the back edge.
//!
//! # How it works
//! A candidate is a plain load in the header of a rotated innermost loop with
//! one preheader and one latch, behind nothing but values and loads, so that it
//! runs on the first trip as soon as the preheader does and cannot trap
//! anywhere it did not.
//!
//! A provider is a plain load or store in the loop of the same type and width
//! whose pointer is the candidate's pointer one trip on: the same
//! computation, each header phi replaced by its value from the latch. The
//! provider's cell is the one the candidate reads on the next trip.
//!
//! The cell's value at the end of the latch is then found by a forward walk of
//! the body in reverse postorder: a store or load of the same bytes leaves its
//! value, a write that may touch the bytes (`memoryssa::may_clobber`, calls by
//! their `CallEffects`) leaves none, and a block's value is its predecessors'
//! when they agree and a phi of them when they differ. The header starts with
//! none. A walk that ends with a value at the latch, and with nothing in the
//! header before the candidate that may write the bytes, replaces the
//! candidate by a header phi of a copy of the load in the preheader and that
//! value.
//!
//! # Example
//! Bubble sort's inner loop reads `a[j]` and `a[j + 1]`; the next trip reads
//! the second as its first, or what a swap stored there. Before:
//!
//! ```mir
//! b1:
//!   %j = phi i32 [ 0, %b0 ], [ %next, %b3 ]
//!   %p = getelementptr i32, ptr @a, i32 %j
//!   %x = load i32, ptr %p
//!   %next = add i32 %j, 1
//!   %q = getelementptr i32, ptr @a, i32 %next
//!   %y = load i32, ptr %q
//!   %c = icmp sgt i32 %x, %y
//!   br i1 %c, label %b2, label %b3
//! ```
//!
//! After: `%x` is a header phi of a load of `a[0]` in the preheader and, from
//! the latch, a phi of `%y` and the value the swap stored at `a[j + 1]`.
//!
//! # Runs
//! Once, after the loops are rotated and before `lsr` chooses their pointers.
//!
//! # Needs
//! `memoryssa::Accesses`, `cfg::Shape`.
//!
//! # Keeps
//! Blocks and loops; no other analysis.
//!
//! # See also
//! LLVM `LoopLoadElimination`, `GVN::PerformLoadPRE`. [`crate::loadjoins`] is
//! load PRE at joins, which refuses a back edge.

use std::collections::BTreeMap;

use llrm_analysis::avail::{loaded_into, stored_from};
use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::memoryssa::{Accesses, changes, may_clobber, same_bytes};
use llrm_mir::edit::Position;
use llrm_mir::memory::{Callees, only_value};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};

/// How deep a pointer's computation is followed.
const DEPTH: usize = 8;

pub struct LoopLoads;

impl FunctionPass for LoopLoads {
    fn name(&self) -> &'static str {
        "looploads"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let (context, layout) = (&*unit.context, unit.layout);
        let shape = analyses.get::<cfg::Shape>(context, layout, unit.function);
        if shape.loops.is_empty() {
            return PreservedAnalyses::all();
        }
        let Ok(accesses) = Accesses::managed(context, layout, unit.function, analyses) else {
            return PreservedAnalyses::all();
        };
        let outer = std::rc::Rc::clone(analyses.outer());
        let carried = planned(
            &Unit::within(context, layout, unit.function, &outer).with_shape(&shape),
            outer.callees(),
            &accesses,
            &shape.loops,
        );
        if carried.is_empty() {
            return PreservedAnalyses::all();
        }
        for one in carried {
            if let Err(error) = applied(unit.function, one) {
                panic!("looploads: {error}");
            }
        }
        PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
    }
}

/// What a block leaves in the cell: a value, or the phi made in a block.
#[derive(Clone, Debug, PartialEq)]
enum Held {
    Value(Operand),
    Merge(BlockId),
}

/// A header load that the last trip's cell serves.
#[derive(Debug)]
struct Carried {
    load: InstId,
    header: BlockId,
    preheader: BlockId,
    latch: BlockId,
    /// The phis the walk made, in the order of the body.
    merges: Vec<(BlockId, Vec<(BlockId, Held)>)>,
    last: Held,
}

fn planned(
    unit: &Unit,
    callees: &Callees,
    accesses: &Accesses,
    loops: &[Loop],
) -> Vec<Carried> {
    let function = unit.function;
    let graph = cfg::graph(function);
    let mut found = Vec::new();
    for one in loops {
        if loops.iter().any(|other| other.header != one.header && one.body.contains(&other.header)) {
            continue;
        }
        let (Some(preheader), Some(&latch)) = (crate::hoist::_preheader(&graph, one), one.latches.first()) else {
            continue;
        };
        let (header, preheader, latch) = (cfg::block(one.header), cfg::block(preheader), cfg::block(latch));
        if one.latches.len() != 1 || function.successors(preheader) != [header] {
            continue;
        }
        let Some(order) = ordered(function, one, header) else { continue };
        for (index, &load) in function.block(header).instructions().iter().enumerate() {
            if let Some((merges, last)) = carried(unit, callees, accesses, one, &order, header, latch, index, load) {
                found.push(Carried { load, header, preheader, latch, merges, last });
            }
        }
    }
    found
}

/// The body in reverse postorder from the header, back edges left out; none if
/// a block is met before one of its predecessors, which is not a natural
/// loop of one entry.
fn ordered(
    function: &Function,
    loop_: &Loop,
    header: BlockId,
) -> Option<Vec<BlockId>> {
    let inside = |block: BlockId| loop_.body.contains(&cfg::id(block));
    let mut post = Vec::new();
    let mut seen = std::collections::BTreeSet::from([header]);
    let mut stack = vec![(header, function.successors(header))];
    while let Some((block, rest)) = stack.last_mut() {
        match rest.pop() {
            Some(next) if inside(next) && next != header && seen.insert(next) => {
                let successors = function.successors(next);
                stack.push((next, successors));
            }
            Some(_) => {}
            None => {
                post.push(*block);
                stack.pop();
            }
        }
    }
    post.reverse();
    let position: BTreeMap<BlockId, usize> = post.iter().enumerate().map(|(at, &block)| (block, at)).collect();
    let well_formed = post
        .iter()
        .all(
            |&block| block == header
                || function
                    .predecessors(block)
                    .iter()
                    .all(|parent| position.get(parent).is_some_and(|&at| at < position[&block])),
        );
    (well_formed && position.len() == loop_.body.len()).then_some(post)
}

#[allow(clippy::too_many_arguments)]
fn carried(
    unit: &Unit,
    callees: &Callees,
    accesses: &Accesses,
    loop_: &Loop,
    order: &[BlockId],
    header: BlockId,
    latch: BlockId,
    index: usize,
    load: InstId,
) -> Option<(Vec<(BlockId, Vec<(BlockId, Held)>)>, Held)> {
    let function = unit.function;
    let (cell, result) = loaded_into(unit, accesses, load)?;
    let ty = function.value(result).ty;
    let pointer = function.instruction(load).operands[0];
    let inside = |block: BlockId| loop_.body.contains(&cfg::id(block));
    let before = &function.block(header).instructions()[..index];
    let clobbers = |cell: &MemRef, inst: InstId| {
        changes(cell, false, accesses.writes(inst), |written| may_clobber(unit, None, cell, written))
    };
    if before.iter().any(|&prior| {
        !(only_value(unit.context, callees, function, prior)
            || matches!(function.instruction(prior).opcode, Opcode::Load { .. }))
            || clobbers(&cell, prior)
    }) || !at_entry_possible(function, header, &inside, pointer, DEPTH)
    {
        return None;
    }
    let body: Vec<InstId> = order.iter().flat_map(|&block| function.block(block).instructions().to_vec()).collect();
    let provider = |inst: InstId| -> Option<(MemRef, Operand, Operand)> {
        let pointer_of = |at: usize| function.instruction(inst).operands[at];
        stored_from(unit, accesses, inst)
            .map(|(cell, value)| (cell, value, pointer_of(1)))
            .or_else(
                || loaded_into(unit, accesses, inst).map(|(cell, value)| (cell, Operand::Value(value), pointer_of(0))),
            )
    };
    let anchor = body
        .iter()
        .copied()
        .filter(|&inst| inst != load)
        .find_map(
            |inst| {
                let (provided, value, at) = provider(inst)?;
                (unit.operand_type(value) == Some(ty)
                    && provided.width == cell.width
                    && one_trip_on(function, header, latch, &inside, pointer, at, DEPTH))
                .then_some(provided)
            },
        )?;
    let mut out: BTreeMap<BlockId, Option<Held>> = BTreeMap::new();
    let mut merges = Vec::new();
    for &block in order {
        let mut state = if block == header {
            None
        } else {
            let parents: Vec<(BlockId, Option<Held>)> =
                function.predecessors(block).into_iter().map(|parent| (parent, out[&parent].clone())).collect();
            if parents.iter().any(|(_, held)| held.is_none()) {
                None
            } else if parents.windows(2).all(|pair| pair[0].1 == pair[1].1) {
                parents[0].1.clone()
            } else {
                merges.push((block, parents.into_iter().map(|(parent, held)| (parent, held.expect("held"))).collect()));
                Some(Held::Merge(block))
            }
        };
        for &inst in function.block(block).instructions() {
            match provider(inst) {
                Some((provided, value, _))
                    if unit.operand_type(value) == Some(ty) && same_bytes(unit, &anchor, &provided) =>
                {
                    state = Some(Held::Value(value));
                }
                _ if clobbers(&anchor, inst) => state = None,
                _ => {}
            }
        }
        out.insert(block, state);
    }
    Some((merges, out.remove(&latch)??))
}

/// Whether `later`, at the next trip, is `earlier` now: each header phi in
/// `later` is its value from the latch, and the computations are alike.
fn one_trip_on(
    function: &Function,
    header: BlockId,
    latch: BlockId,
    inside: &dyn Fn(BlockId) -> bool,
    later: Operand,
    earlier: Operand,
    depth: usize,
) -> bool {
    let (Operand::Value(value), true) = (later, depth > 0) else { return later == earlier };
    let ValueDef::Instruction(def) = function.value(value).def else { return later == earlier };
    if !function.parent(def).is_some_and(inside) {
        return later == earlier;
    }
    let instruction = function.instruction(def);
    if instruction.opcode == Opcode::Phi {
        if function.parent(def) != Some(header) {
            return false;
        }
        let carried = instruction.operands.chunks(2).find(|arm| arm[1] == Operand::Block(latch)).map(|arm| arm[0]);
        return carried.is_some_and(|carried| alike(function, carried, earlier, depth));
    }
    let Operand::Value(other) = earlier else { return false };
    let ValueDef::Instruction(twin) = function.value(other).def else { return false };
    let twin = function.instruction(twin);
    instruction.opcode == twin.opcode
        && instruction.ty == twin.ty
        && instruction.operands.len() == twin.operands.len()
        && instruction
            .operands
            .iter()
            .zip(&twin.operands)
            .all(|(&one, &two)| one_trip_on(function, header, latch, inside, one, two, depth - 1))
}

/// Whether two operands are the same value, or computations of it.
fn alike(
    function: &Function,
    one: Operand,
    other: Operand,
    depth: usize,
) -> bool {
    if one == other {
        return true;
    }
    let (Operand::Value(one), Operand::Value(other), true) = (one, other, depth > 0) else { return false };
    let (ValueDef::Instruction(left), ValueDef::Instruction(right)) =
        (function.value(one).def, function.value(other).def)
    else {
        return false;
    };
    let (left, right) = (function.instruction(left), function.instruction(right));
    left.opcode != Opcode::Phi
        && left.opcode == right.opcode
        && left.ty == right.ty
        && left.operands.len() == right.operands.len()
        && left.operands.iter().zip(&right.operands).all(|(&a, &b)| alike(function, a, b, depth - 1))
}

/// Whether `pointer` can be computed in the preheader for the first trip: it is
/// built from values outside the loop and the header's phis by computations
/// that only make a value.
fn at_entry_possible(
    function: &Function,
    header: BlockId,
    inside: &dyn Fn(BlockId) -> bool,
    pointer: Operand,
    depth: usize,
) -> bool {
    let Operand::Value(value) = pointer else { return !matches!(pointer, Operand::Block(_)) };
    let ValueDef::Instruction(def) = function.value(value).def else { return true };
    if !function.parent(def).is_some_and(inside) {
        return true;
    }
    let instruction = function.instruction(def);
    if instruction.opcode == Opcode::Phi {
        return function.parent(def) == Some(header);
    }
    depth > 0
        && function.parent(def) == Some(header)
        && llrm_mir::memory::pure_operation(&instruction.opcode)
        && instruction.operands.iter().all(|&one| at_entry_possible(function, header, inside, one, depth - 1))
}

/// `pointer` as the first trip computes it, made in the preheader.
fn at_entry(
    function: &mut Function,
    header: BlockId,
    preheader: BlockId,
    pointer: Operand,
) -> Result<Operand, String> {
    let Operand::Value(value) = pointer else { return Ok(pointer) };
    let ValueDef::Instruction(def) = function.value(value).def else { return Ok(pointer) };
    if function.parent(def) != Some(header) {
        return Ok(pointer);
    }
    if function.instruction(def).opcode == Opcode::Phi {
        let from = function.instruction(def).operands.chunks(2).find(|arm| arm[1] == Operand::Block(preheader));
        return from.map(|arm| arm[0]).ok_or_else(|| "a header phi without the preheader".to_owned());
    }
    let copy = function.clone_instruction(def);
    for at in 0..function.instruction(copy).operands.len() {
        let entry = at_entry(function, header, preheader, function.instruction(copy).operands[at])?;
        function.set_operand(copy, at, entry);
    }
    function.insert(copy, Position::Before(function.terminator(preheader).ok_or("a preheader without a branch")?))?;
    Ok(Operand::Value(function.instruction(copy).result.ok_or("a computation without a value")?))
}

fn applied(
    function: &mut Function,
    carried: Carried,
) -> Result<(), String> {
    let Carried { load, header, preheader, latch, merges, last } = carried;
    let result = function.instruction(load).result.ok_or("a load without a value")?;
    let (ty, name) = (function.value(result).ty, function.value(result).name.clone());

    let mut made: BTreeMap<BlockId, Operand> = BTreeMap::new();
    for (block, arms) in merges {
        let arms = arms
            .into_iter()
            .flat_map(|(parent, held)| {
                let value = match held {
                    Held::Value(value) => value,
                    Held::Merge(at) => made[&at],
                };
                [value, Operand::Block(parent)]
            })
            .collect();
        let phi = function.create_instruction(Opcode::Phi, ty, arms, Flags::default(), None);
        function.insert(phi, Position::Before(function.block(block).instructions()[0]))?;
        made.insert(block, Operand::Value(function.instruction(phi).result.ok_or("a phi without a value")?));
    }

    let pointer = function.instruction(load).operands[0];
    let first = function.clone_instruction(load);
    let entry = at_entry(function, header, preheader, pointer)?;
    function.set_operand(first, 0, entry);
    function.insert(first, Position::Before(function.terminator(preheader).ok_or("a preheader without a branch")?))?;

    let from_latch = match last {
        Held::Value(value) => value,
        Held::Merge(at) => made[&at],
    };
    let from_preheader = Operand::Value(function.instruction(first).result.ok_or("a load without a value")?);
    let arms = vec![from_preheader, Operand::Block(preheader), from_latch, Operand::Block(latch)];
    let phi = function.create_instruction(Opcode::Phi, ty, arms, Flags::default(), name.as_deref());
    function.insert(phi, Position::Before(function.block(header).instructions()[0]))?;
    let carried = Operand::Value(function.instruction(phi).result.ok_or("a phi without a value")?);
    function.replace_all_uses_with(result, carried);
    function.erase(load)
}

#[cfg(test)]
#[path = "looploads_tests.rs"]
mod tests;
