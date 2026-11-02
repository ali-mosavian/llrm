//! Which SSA values are live at each block's entry and exit, and how many are
//! live at once.
//!
//! # How it works
//! `live` returns `Liveness { live_in, live_out }`, keyed by `cfg::id`. It is
//! the least solution of
//!
//! ```text
//!  out(B)   = union of in(S) over successors S, plus the arms of S's live
//!             phis that come from B
//!  after(B) = out(B) - defs(B) + exposed(B)   (exposed: read before written)
//!  in(B)    = after(B) - phis(B)
//! ```
//!
//! `solved` is that fixed point on rows of bits, one row per block and a bit
//! for each value a block boundary can carry (a value read in a block it is
//! not made in, or taken by a phi): `out` is a union of the successors' rows
//! and `after` two word-wise operations, passes over the blocks from the last
//! until nothing changes. The work is the blocks times the words of a row times
//! the passes; following each value from its reads back to its definition (a
//! map lookup and a set insert for each value in each block it is live in, the
//! solver that stood here, kept as the oracle `LLRM_CHECK_LIVENESS` holds it
//! to) cost ~670 instructions a pair on `joins`. A phi's arm is live out of the
//! predecessor it comes from, not in the phi's block.
//!
//! `live_of` follows only the values it is given (a manager analysis brought up
//! to date for what a change named), from their reads, and gives their sets
//! alone.
//!
//! ```text
//!    B0:  %a = ...                 %a live_out(B0), live_in(B1) (read in B1)
//!    B1:  %p = phi [%a, B0], [%b, B2]    arm %a is live_out(B0) only
//!         ...                      %b live_out(B2)
//!    B2:  %b = ...
//! ```
//!
//! `live_where` solves for a chosen set of values only, plus the phis that take
//! them as arms, so a caller asking about a few values (the phis a pass takes
//! for cells) does not pay for the whole function. `entry_values` are the
//! values used and defined nowhere in the body: the parameters.
//!
//! Pressure. `pressure` is the most values live at once, over the function or
//! over the blocks `inside`; `pressure_of` counts only values a predicate
//! accepts. It starts from `live_out` and walks each block backward.
//! Per-instruction views: `live_points` (live before and across each
//! instruction), `each_across` (the same over one set updated as it goes,
//! avoiding a copy per instruction), `live_steps` (the changes between
//! neighbouring instructions) and `pressure_points`. `phi_inputs` gives the
//! edge operands of phis whose results are live. `solves` counts the solves on
//! this thread, so a test can check that a caller reuses the one it holds.
//!
//! Under `cfg(test)`, `live` is checked against a bit-set fixed point
//! (`live_dense`) on every function a test gives it.
//!
//! # Example
//! A phi arm is live out of the predecessor it comes from:
//!
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %incoming = add i16 %x, 1
//!   br label %b1
//!
//! b1:
//!   %merged = phi i16 [ %incoming, %b0 ]
//!   ret i16 %merged
//! }
//! ```
//!
//! `%incoming` is in `live_out(b0)`. With `ret void` in place of `ret i16
//! %merged` the phi is dead and `%incoming` is not in `live_out(b0)`.
//!
//! From unit test `test_a_dead_phi_does_not_keep_its_edge_operand_live` in
//! `liveness.rs`.
//!
//! # Provides
//! `Liveness`, `live`, `live_where`, `pressure`, `pressure_of`, `entry_values`,
//! `live_points`, `each_across`, `live_steps`, `pressure_points`, `phi_inputs`.
//! Consumers in llrm-transforms: `homes`, `spill`, `lsr`, `indvars`, `inline`.
//! The machine phases use `backend/liveness.rs`, over physical register lanes.
//!
//! # See also
//! GCC `calculate_live_on_exit` (`tree-ssa-live.cc`); LLVM `LiveVariables`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueId};
use llrm_mir::opcode::Opcode;

use crate::cfg::id;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Liveness {
    pub live_in: BTreeMap<i64, IdSet<ValueId>>,
    pub live_out: BTreeMap<i64, IdSet<ValueId>>,
}

/// The local values an instruction reads.
fn reads(instruction: &Instruction) -> impl Iterator<Item = ValueId> + '_ {
    instruction
        .operands
        .iter()
        .filter_map(
            |operand| match operand {
                Operand::Value(value) => Some(*value),
                _ => None,
            },
        )
}

fn is_phi(instruction: &Instruction) -> bool {
    instruction.opcode == Opcode::Phi
}

/// A block's instructions: its phis, then its ordinary operations.
fn split(
    function: &Function,
    block: BlockId,
) -> (Vec<&Instruction>, Vec<&Instruction>) {
    function.iter_block(block).map(|one| function.instruction(one)).partition(|one| is_phi(one))
}

/// A phi's value on the edge from `predecessor`, when it is a local value.
fn arm(
    phi: &Instruction,
    predecessor: BlockId,
) -> Option<ValueId> {
    phi.operands
        .chunks(2)
        .find(|pair| pair[1] == Operand::Block(predecessor))
        .and_then(
            |pair| match pair[0] {
                Operand::Value(value) => Some(value),
                _ => None,
            },
        )
}

fn _defines(
    function: &Function,
    block: BlockId,
) -> IdSet<ValueId> {
    function.iter_block(block).filter_map(|one| function.instruction(one).result).collect()
}

/// Values the ordinary operations read before writing.
fn _exposed(
    function: &Function,
    block: BlockId,
) -> IdSet<ValueId> {
    let mut live = IdSet::new();
    for op in split(function, block).1.into_iter().rev() {
        if let Some(one) = op.result {
            live.remove(&one);
        }
        live.extend(reads(op));
    }
    live
}

/// Values the caller supplied: used somewhere, defined by nothing here.
pub fn entry_values(function: &Function) -> IdSet<ValueId> {
    let mut defined = IdSet::new();
    for &block in function.layout() {
        defined.extend(_defines(function, block).iter());
    }
    let mut arriving = IdSet::new();
    for (_, one) in function.walk() {
        for value in reads(function.instruction(one)) {
            if !defined.contains(&value) {
                arriving.insert(value);
            }
        }
    }
    arriving
}

/// The most values live at once -- anywhere, or in `inside`.
pub fn pressure(
    function: &Function,
    found: Option<&Liveness>,
    inside: Option<&llrm_mir::forest::Body>,
) -> usize {
    pressure_of(function, found, inside, &|_| true)
}

/// `pressure`, counting only the values `counted` says.
pub fn pressure_of(
    function: &Function,
    found: Option<&Liveness>,
    inside: Option<&llrm_mir::forest::Body>,
    counted: &dyn Fn(ValueId) -> bool,
) -> usize {
    let owned;
    let found = match found {
        Some(found) => found,
        None => {
            owned = live(function);
            &owned
        }
    };
    let mut peak = 0;
    for &block in function.layout() {
        if inside.is_some_and(|inside| !inside.contains(&id(block))) {
            continue;
        }
        let mut alive = found.live_out[&id(block)].clone();
        let size = |alive: &IdSet<ValueId>| alive.iter().filter(|&one| counted(one)).count();
        peak = peak.max(size(&alive));
        for op in split(function, block).1.into_iter().rev() {
            if let Some(one) = op.result {
                alive.remove(&one);
            }
            alive.extend(reads(op));
            peak = peak.max(size(&alive));
        }
    }
    peak
}

/// `pressure_of` over the `blocks` alone, solving liveness for the counted
/// values only.
pub fn pressure_in(
    function: &Function,
    blocks: &[BlockId],
    counted: &dyn Fn(ValueId) -> bool,
) -> usize {
    let found = live_where(function, counted);
    let mut peak = 0;
    for &block in blocks {
        let mut alive = found.live_out[&id(block)].clone();
        let size = |alive: &IdSet<ValueId>| alive.iter().filter(|&one| counted(one)).count();
        peak = peak.max(size(&alive));
        for op in split(function, block).1.into_iter().rev() {
            if let Some(one) = op.result {
                alive.remove(&one);
            }
            alive.extend(reads(op));
            peak = peak.max(size(&alive));
        }
    }
    peak
}

/// An instruction, the values live before it and those live across it.
pub struct LivePoint {
    pub inst: InstId,
    pub before: IdSet<ValueId>,
    pub across: IdSet<ValueId>,
}

/// The values live before each instruction of `block` but its phis, and
/// those live across it, in order.
pub fn live_points(
    function: &Function,
    found: &Liveness,
    block: BlockId,
) -> Vec<LivePoint> {
    let mut alive = found.live_out[&id(block)].clone();
    let mut points = Vec::new();
    for inst in function.iter_block_rev(block) {
        let op = function.instruction(inst);
        if is_phi(op) {
            continue;
        }
        if let Some(one) = op.result {
            alive.remove(&one);
        }
        let across = alive.clone();
        alive.extend(reads(op));
        points.push(LivePoint { inst, before: alive.clone(), across });
    }
    points.reverse();
    points
}

/// `f(instruction, whether a value is live across it)` for each instruction of
/// `block` but its phis, last first, over one set changed as the walk goes:
/// `live_points` copies the live set twice for each instruction, and a caller
/// that asked it of every block for each of many values paid the live set's
/// size times the function's for each (`homes` on N cells and N joins: cubic).
pub fn each_across(
    function: &Function,
    found: &Liveness,
    block: BlockId,
    f: &mut dyn FnMut(InstId, &dyn Fn(ValueId) -> bool),
) {
    let mut alive = found.live_out[&id(block)].clone();
    for inst in function.iter_block_rev(block) {
        let op = function.instruction(inst);
        if is_phi(op) {
            continue;
        }
        if let Some(one) = op.result {
            alive.remove(&one);
        }
        f(inst, &|value| alive.contains(&value));
        alive.extend(reads(op));
    }
}

/// What changes in the live set across each instruction of `block` but its
/// phis, in order, and what is live before the first: `live_points` without a
/// set copied for every instruction. An instruction's `read` values are those
/// live before it that were not live across it; `made` is its result where
/// that is live after it. The live set before an instruction is the one
/// after the one before it, less what it made, plus what it read.
pub struct Steps {
    pub first: Vec<ValueId>,
    pub steps: Vec<Step>,
}

pub struct Step {
    pub inst: InstId,
    pub read: Vec<ValueId>,
    pub made: Option<ValueId>,
}

pub fn live_steps(
    function: &Function,
    found: &Liveness,
    block: BlockId,
) -> Steps {
    let mut alive = found.live_out[&id(block)].clone();
    let mut steps = Vec::new();
    for inst in function.iter_block_rev(block) {
        let op = function.instruction(inst);
        if is_phi(op) {
            continue;
        }
        let made = op.result.filter(|one| alive.remove(one));
        let read = reads(op).filter(|one| alive.insert(*one)).collect();
        steps.push(Step { inst, read, made });
    }
    steps.reverse();
    Steps { first: alive.iter().collect(), steps }
}

/// How many values `counted` says are live before each instruction of
/// `block` but its phis, in order.
pub fn pressure_points(
    function: &Function,
    found: &Liveness,
    block: BlockId,
    counted: &dyn Fn(ValueId) -> bool,
) -> Vec<(InstId, usize)> {
    live_points(function, found, block)
        .into_iter()
        .map(|point| (point.inst, point.before.iter().filter(|&one| counted(one)).count()))
        .collect()
}

/// The edge operands of phis whose results are actually live.
pub fn phi_inputs(
    function: &Function,
    found: Option<&Liveness>,
) -> BTreeSet<ValueId> {
    let owned;
    let found = match found {
        Some(found) => found,
        None => {
            owned = live(function);
            &owned
        }
    };
    let mut inputs = BTreeSet::new();
    for &block in function.layout() {
        let (phis, ops) = split(function, block);
        let mut alive = found.live_out[&id(block)].clone();
        for op in ops.into_iter().rev() {
            if let Some(one) = op.result {
                alive.remove(&one);
            }
            alive.extend(reads(op));
        }
        for phi in phis {
            if phi.result.is_some_and(|result| alive.contains(&result)) {
                inputs.extend(reads(phi));
            }
        }
    }
    inputs
}

// What is live at each block's entry and exit, to a fixed point.
thread_local! {
    static SOLVES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has solved a function's liveness, for a test that
/// a caller asks the one it holds.
pub fn solves() -> usize {
    SOLVES.with(std::cell::Cell::get)
}

// Slots the tables of a block's instruction positions took, for a test that a
// table is the block's size and not the function's.
#[cfg(test)]
thread_local! {
    pub(crate) static POSITION_SLOTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// Facts `live` follows, for a test that its work is the size of the live sets.
#[cfg(test)]
thread_local! {
    pub(crate) static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// What is live at each block's entry and exit: the least solution of
/// `out(B) = U in(S) + arms of S's live phis`, `after(B) = out(B) - defs(B) +
/// exposed(B)`, `in(B) = after(B) - phis(B)`, on rows of bits over the values a
/// block boundary can carry (the module header).
pub fn live(function: &Function) -> Liveness {
    let found = solved(function, &|_| true);
    #[cfg(test)]
    assert_eq!(found, live_dense(function), "liveness from the uses is not the bit-set fixed point");
    found
}

/// `live` of the values `wanted` says only: each is live where it is in the
/// whole, the others are in no set. A caller that asks of a few values (the
/// phis a pass takes for cells) is not charged for the live sets of every value
/// in the function.
pub fn live_where(
    function: &Function,
    wanted: &dyn Fn(ValueId) -> bool,
) -> Liveness {
    // A phi's arm is live where the phi is: following a wanted arm needs the
    // liveness of the phi that takes it, and of the phis that take that
    // one.
    let mut needed: IdSet<ValueId> = function
        .walk()
        .filter_map(|(_, inst)| function.instruction(inst).result)
        .chain(function.parameters().iter().copied())
        .filter(|value| wanted(*value))
        .collect();
    let phis: Vec<&Instruction> =
        function.walk().map(|(_, inst)| function.instruction(inst)).filter(|one| one.opcode == Opcode::Phi).collect();
    let mut grown = true;
    while grown {
        grown = false;
        for phi in &phis {
            let Some(result) = phi.result else { continue };
            if !needed.contains(&result)
                && phi.operands.chunks(2).any(|pair| matches!(pair[0], Operand::Value(arm) if needed.contains(&arm)))
            {
                needed.insert(result);
                grown = true;
            }
        }
    }
    let mut found = solved(function, &|value| needed.contains(&value));
    for set in found.live_in.values_mut().chain(found.live_out.values_mut()) {
        set.retain(|value| wanted(value));
    }
    found
}

/// The blocks one value is live into and out of, in ascending order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Range {
    pub into: Vec<i64>,
    pub out: Vec<i64>,
}

/// `live` of the values in `wanted` alone, followed from their reads without
/// the setup of a solve of the whole function: the work is the blocks those
/// values are live in. The sets are named for the blocks a value is live in
/// only. A phi's arm is live where the phi is, so `wanted` holds the arms of
/// its phis and the phis that take its values.
pub fn live_of(
    function: &Function,
    wanted: &IdSet<ValueId>,
) -> Liveness {
    let mut found = Liveness { live_in: BTreeMap::new(), live_out: BTreeMap::new() };
    let ranges = ranges_of(function, wanted, None);
    for (value, range) in ranges.iter() {
        for &at in &range.into {
            found.live_in.entry(at).or_default().insert(value);
        }
        for &at in &range.out {
            found.live_out.entry(at).or_default().insert(value);
        }
    }
    found
}

/// `live_of` by value: where each is live, as sorted lists, with no set made
/// per block. A value live nowhere is not in the result.
///
/// A phi's arm is live out of the predecessor it comes from where the phi's
/// value is live out of its block or read in it. A phi `wanted` does not hold
/// is, with `settled`, taken as `settled` says of it (was it live out of its
/// block) or read in its block as the function now has it; with `None` it is
/// not followed.
pub fn ranges_of(
    function: &Function,
    wanted: &IdSet<ValueId>,
    settled: Option<&dyn Fn(ValueId, BlockId) -> bool>,
) -> IdMap<ValueId, Range> {
    use llrm_mir::module::ValueDef;
    enum Reach {
        After(BlockId, ValueId),
        Out(BlockId, ValueId),
    }
    let mut found: IdMap<ValueId, Range> = IdMap::new();
    let Some(&entry) = function.layout().first() else { return found };
    // Where a value is made: a block, and whether by a phi. A value made by
    // nothing here arrives at the entry.
    let made = |value: ValueId| match function.value(value).def {
        ValueDef::Instruction(inst) => {
            function.parent(inst).map(|block| (block, function.instruction(inst).opcode == Opcode::Phi))
        }
        ValueDef::Argument(_) => None,
    };
    // Each block's instructions by position, worked out once for the blocks a
    // read is asked about in: a scan of the block for every read is the block
    // squared.
    let mut positions: IdMap<BlockId, Vec<(llrm_mir::module::InstId, usize)>> = IdMap::new();
    // A block's predecessors, asked once for all the values live into it.
    let mut before = IdMap::<BlockId, Vec<BlockId>>::new();
    let mut pending = Vec::new();
    for value in wanted.iter() {
        let at = made(value);
        for user in function.users(value) {
            let Some(block) = function.parent(user.user) else { continue };
            if function.instruction(user.user).opcode == Opcode::Phi {
                continue;
            }
            // Read before it is written in the block: the operation that makes
            // it comes first.
            let exposed = match at {
                Some((home, false)) if home == block => {
                    let places = positions.get_or_insert_with(block, || {
                        let mut table: Vec<(llrm_mir::module::InstId, usize)> =
                            function.iter_block(block).enumerate().map(|(at, one)| (one, at)).collect();
                        table.sort_unstable();
                        #[cfg(test)]
                        POSITION_SLOTS.with(|slots| slots.set(slots.get() + table.len()));
                        table
                    });
                    let place = |inst| places.binary_search_by_key(&inst, |&(one, _)| one).ok().map(|at| &places[at].1);
                    let maker = match function.value(value).def {
                        ValueDef::Instruction(inst) => place(inst),
                        ValueDef::Argument(_) => None,
                    };
                    place(user.user).zip(maker).is_some_and(|(&read, &made)| read <= made)
                }
                _ => true,
            };
            if exposed && (block != entry || at.is_some()) {
                pending.push(Reach::After(block, value));
            }
        }
    }
    if let Some(settled) = settled {
        for value in wanted.iter() {
            for user in function.users(value) {
                let phi = function.instruction(user.user);
                let (is_phi, Some(home), Some(result)) =
                    (phi.opcode == Opcode::Phi, function.parent(user.user), phi.result)
                else {
                    continue;
                };
                if !is_phi || wanted.contains(&result) {
                    continue;
                }
                let read = function.users(result).iter().any(|one| {
                    function.instruction(one.user).opcode != Opcode::Phi && function.parent(one.user) == Some(home)
                });
                if !(read || settled(result, home)) {
                    continue;
                }
                for from in before.get_or_insert_with(home, || function.predecessors(home).into()).clone() {
                    if arm(phi, from) == Some(value) {
                        pending.push(Reach::Out(from, value));
                    }
                }
            }
        }
    }
    let mut seen_phis = BTreeSet::<(i64, ValueId)>::new();
    let mut seen_out = IdMap::<ValueId, IdSet<BlockId>>::new();
    let mut seen_in = IdMap::<ValueId, IdSet<BlockId>>::new();
    while let Some(reach) = pending.pop() {
        match reach {
            Reach::Out(block, value) => {
                let by_op = match made(value) {
                    Some((home, phi)) => home == block && !phi,
                    None => block == entry,
                };
                if seen_out.get_or_insert_with(value, IdSet::new).insert(block) {
                    found.get_or_insert_with(value, Range::default).out.push(id(block));
                    if !by_op {
                        pending.push(Reach::After(block, value));
                    }
                }
            }
            Reach::After(block, value) => {
                if made(value).is_some_and(|(home, phi)| home == block && phi) {
                    if !seen_phis.insert((id(block), value)) {
                        continue;
                    }
                    for one in function.iter_block(block) {
                        let phi = function.instruction(one);
                        if phi.opcode != Opcode::Phi {
                            break;
                        }
                        if phi.result != Some(value) {
                            continue;
                        }
                        for from in function.predecessors(block) {
                            if let Some(arm) = arm(phi, from).filter(|arm| wanted.contains(arm)) {
                                pending.push(Reach::Out(from, arm));
                            }
                        }
                    }
                } else {
                    if !seen_in.get_or_insert_with(value, IdSet::new).insert(block) {
                        continue;
                    }
                    found.get_or_insert_with(value, Range::default).into.push(id(block));
                    let from = before.get_or_insert_with(block, || function.predecessors(block).into());
                    pending.extend(from.iter().map(|&from| Reach::Out(from, value)));
                }
            }
        }
    }
    for (_, range) in found.iter_mut() {
        range.into.sort_unstable();
        range.out.sort_unstable();
    }
    found
}

fn solved(
    function: &Function,
    wanted: &dyn Fn(ValueId) -> bool,
) -> Liveness {
    SOLVES.with(|solves| solves.set(solves.get() + 1));
    let found = solved_dataflow(function, wanted);
    if llrm_support::env_flag!("LLRM_CHECK_LIVENESS") {
        assert!(
            found == solved_by_reach(function, wanted),
            "the word-parallel liveness is not the one followed from the uses"
        );
    }
    found
}

/// `solved` as a dataflow over rows of bits, a bit for each value a block
/// boundary can carry (a value read in a block it is not made in, or taken by a
/// phi): a block is a union of its successors' rows, so the work is the blocks
/// times the words of a row times the passes to a fixed point, not a step for
/// each value in each block it is live in.
fn solved_dataflow(
    function: &Function,
    wanted: &dyn Fn(ValueId) -> bool,
) -> Liveness {
    let layout = function.layout();
    let count = layout.len();
    let mut at_index = IdMap::<BlockId, usize>::new();
    for (position, &block) in layout.iter().enumerate() {
        at_index.insert(block, position);
    }
    let blocks: Vec<(Vec<&Instruction>, Vec<&Instruction>)> =
        layout.iter().map(|&block| split(function, block)).collect();
    let arriving = entry_values(function);
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (position, &block) in layout.iter().enumerate() {
        for successor in function.successors(block) {
            if let Some(&to) = at_index.get(&successor) {
                if !successors[position].contains(&to) {
                    successors[position].push(to);
                }
            }
        }
    }
    // The values a boundary can carry, each with a slot.
    let mut slots = IdMap::<ValueId, usize>::new();
    let mut values: Vec<ValueId> = Vec::new();
    let mut slot = |value: ValueId, slots: &mut IdMap<ValueId, usize>| -> usize {
        if let Some(&at) = slots.get(&value) {
            return at;
        }
        values.push(value);
        slots.insert(value, values.len() - 1);
        values.len() - 1
    };
    let mut used: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (position, &block) in layout.iter().enumerate() {
        for value in _exposed(function, block).iter() {
            if wanted(value) && (position != 0 || !arriving.contains(&value)) {
                used[position].push(slot(value, &mut slots));
            }
        }
    }
    // Per block, its phis: the slot of the result when it can be live, and each
    // predecessor's arm.
    struct Taken {
        result: Option<usize>,
        arms: Vec<(usize, usize)>,
    }
    let mut taken: Vec<Vec<Taken>> = Vec::new();
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (from, to) in successors.iter().enumerate().flat_map(|(from, tos)| tos.iter().map(move |&to| (from, to))) {
        predecessors[to].push(from);
    }
    for (position, (phis, _)) in blocks.iter().enumerate() {
        let mut here = Vec::new();
        for phi in phis {
            let result = phi.result.filter(|value| wanted(*value)).map(|value| slot(value, &mut slots));
            let mut arms = Vec::new();
            for &from in &predecessors[position] {
                if let Some(value) = arm(phi, layout[from]).filter(|value| wanted(*value)) {
                    arms.push((from, slot(value, &mut slots)));
                }
            }
            here.push(Taken { result, arms });
        }
        taken.push(here);
    }
    let width = values.len().div_ceil(64).max(1);
    let row = |rows: &Vec<u64>, at: usize| -> std::ops::Range<usize> {
        let _ = rows;
        at * width..(at + 1) * width
    };
    let mut uses = vec![0u64; count * width];
    let mut made = vec![0u64; count * width];
    let mut phi_results = vec![0u64; count * width];
    for position in 0..count {
        for &at in &used[position] {
            uses[position * width + at / 64] |= 1 << (at % 64);
        }
        for op in &blocks[position].1 {
            if let Some(&at) = op.result.and_then(|result| slots.get(&result)) {
                made[position * width + at / 64] |= 1 << (at % 64);
            }
        }
        for phi in &taken[position] {
            if let Some(at) = phi.result {
                phi_results[position * width + at / 64] |= 1 << (at % 64);
            }
        }
    }
    for value in arriving.iter() {
        if let Some(&at) = slots.get(&value) {
            made[at / 64] |= 1 << (at % 64);
        }
    }
    let mut after = vec![0u64; count * width];
    let mut out = vec![0u64; count * width];
    let mut scratch = vec![0u64; width];
    let mut changed = true;
    while changed {
        changed = false;
        for position in (0..count).rev() {
            #[cfg(test)]
            STEPS.with(|steps| steps.set(steps.get() + 1));
            scratch.fill(0);
            for &to in &successors[position] {
                for word in 0..width {
                    scratch[word] |= after[to * width + word] & !phi_results[to * width + word];
                }
                for phi in &taken[to] {
                    let Some(result) = phi.result else { continue };
                    if after[to * width + result / 64] & (1 << (result % 64)) == 0 {
                        continue;
                    }
                    for &(from, at) in &phi.arms {
                        if from == position {
                            scratch[at / 64] |= 1 << (at % 64);
                        }
                    }
                }
            }
            let range = row(&out, position);
            if out[range.clone()] != scratch[..] {
                out[range].copy_from_slice(&scratch);
                changed = true;
            }
            for word in 0..width {
                let now = uses[position * width + word] | (scratch[word] & !made[position * width + word]);
                if after[position * width + word] != now {
                    after[position * width + word] = now;
                    changed = true;
                }
            }
        }
    }
    let make = |rows: &Vec<u64>, position: usize, drop: &[u64]| -> IdSet<ValueId> {
        let mut set = IdSet::new();
        for word in 0..width {
            let mut left = rows[position * width + word] & !drop[word];
            while left != 0 {
                let bit = left.trailing_zeros() as usize;
                left &= left - 1;
                set.insert(values[word * 64 + bit]);
            }
        }
        set
    };
    let none = vec![0u64; width];
    let mut found = Liveness { live_in: BTreeMap::new(), live_out: BTreeMap::new() };
    for (position, &block) in layout.iter().enumerate() {
        let dropped = &phi_results[position * width..(position + 1) * width];
        found.live_in.insert(id(block), make(&after, position, dropped));
        found.live_out.insert(id(block), make(&out, position, &none));
    }
    found
}

/// `solved` followed from the uses, each value back to where it is made: the
/// oracle of `solved_dataflow` (`LLRM_CHECK_LIVENESS`), and cheap where the
/// live sets are small against blocks times values.
fn solved_by_reach(
    function: &Function,
    wanted: &dyn Fn(ValueId) -> bool,
) -> Liveness {
    let layout = function.layout();
    let mut at_index = IdMap::<BlockId, usize>::new();
    for (position, &block) in layout.iter().enumerate() {
        at_index.insert(block, position);
    }
    let blocks: Vec<(Vec<&Instruction>, Vec<&Instruction>)> =
        layout.iter().map(|&block| split(function, block)).collect();
    // Where each value is made: a block's position, and whether by a phi. The
    // values the caller supplies are made at the entry.
    let mut made = IdMap::<ValueId, (usize, bool)>::new();
    for (position, (phis, ops)) in blocks.iter().enumerate() {
        for (instruction, phi) in phis.iter().map(|one| (one, true)).chain(ops.iter().map(|one| (one, false))) {
            if let Some(result) = instruction.result {
                made.insert(result, (position, phi));
            }
        }
    }
    let arriving = entry_values(function);
    for value in arriving.iter() {
        made.insert(value, (0, false));
    }
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); layout.len()];
    for (position, &block) in layout.iter().enumerate() {
        for successor in function.successors(block) {
            if let Some(&to) = at_index.get(&successor) {
                predecessors[to].push(position);
            }
        }
    }
    // `after` is what is live once a block's phis have run; the phis' own
    // results come out of it at the end.
    let empty = || layout.iter().map(|&block| (id(block), Default::default())).collect();
    let mut found = Liveness { live_in: empty(), live_out: empty() };
    let key = |position: usize| id(layout[position]);

    enum Reach {
        /// Live once `block`'s phis have run.
        After(usize, ValueId),
        /// Live at the end of `block`.
        Out(usize, ValueId),
    }
    let mut pending = Vec::new();
    for (position, &block) in layout.iter().enumerate() {
        for value in _exposed(function, block).iter() {
            if wanted(value) && (position != 0 || !arriving.contains(&value)) {
                pending.push(Reach::After(position, value));
            }
        }
    }
    while let Some(reach) = pending.pop() {
        #[cfg(test)]
        STEPS.with(|steps| steps.set(steps.get() + 1));
        match reach {
            Reach::Out(position, value) => {
                let by_op = made.get(&value).is_some_and(|&(at, phi)| at == position && !phi);
                if found.live_out.get_mut(&key(position)).expect("a block").insert(value) && !by_op {
                    pending.push(Reach::After(position, value));
                }
            }
            Reach::After(position, value) => {
                if !found.live_in.get_mut(&key(position)).expect("a block").insert(value) {
                    continue;
                }
                if made.get(&value).is_some_and(|&(at, phi)| at == position && phi) {
                    // Each edge brings the phi's arm from its own block.
                    for phi in blocks[position].0.iter().filter(|phi| phi.result == Some(value)) {
                        for &from in &predecessors[position] {
                            if let Some(arm) = arm(phi, layout[from]).filter(|arm| wanted(*arm)) {
                                pending.push(Reach::Out(from, arm));
                            }
                        }
                    }
                } else {
                    pending.extend(predecessors[position].iter().map(|&from| Reach::Out(from, value)));
                }
            }
        }
    }
    for (position, (phis, _)) in blocks.iter().enumerate() {
        let after = found.live_in.get_mut(&key(position)).expect("a block");
        for result in phis.iter().filter_map(|phi| phi.result) {
            after.remove(&result);
        }
    }
    found
}

/// The fixed point on bit sets over dense value indices, which `live` replaced:
/// its oracle. `live` is held to it on every function a test of this crate
/// gives it.
#[cfg(test)]
pub(crate) fn live_dense(function: &Function) -> Liveness {
    use llrm_support::bits::Bits;
    use llrm_support::hash::HashMap;
    let layout = function.layout();
    let mut index: HashMap<ValueId, usize> = HashMap::default();
    let mut values: Vec<ValueId> = Vec::new();
    for (_, one) in function.walk() {
        let instruction = function.instruction(one);
        for value in instruction.result.into_iter().chain(reads(instruction)) {
            index
                .entry(value)
                .or_insert_with(
                    || {
                        values.push(value);
                        values.len() - 1
                    },
                );
        }
    }
    let bits = |ones: &mut dyn Iterator<Item = ValueId>| {
        let mut set = Bits::new(values.len());
        for one in ones {
            set.insert(index[&one]);
        }
        set
    };
    let at_index: HashMap<BlockId, usize> =
        layout.iter().enumerate().map(|(position, &block)| (block, position)).collect();
    let blocks: Vec<(Vec<&Instruction>, Vec<&Instruction>)> =
        layout.iter().map(|&block| split(function, block)).collect();

    let mut op_defines: Vec<Bits> =
        blocks.iter().map(|(_, ops)| bits(&mut ops.iter().filter_map(|op| op.result))).collect();
    let phi_defines: Vec<Bits> =
        blocks.iter().map(|(phis, _)| bits(&mut phis.iter().filter_map(|phi| phi.result))).collect();
    let mut exposed: Vec<Bits> = layout.iter().map(|&block| bits(&mut _exposed(function, block).iter())).collect();
    if !layout.is_empty() {
        let arriving = bits(&mut entry_values(function).iter());
        op_defines[0].union_with(&arriving);
        exposed[0].subtract(&arriving);
    }
    // Each successor as its position and, per phi, (result, this block's arm).
    let successors: Vec<Vec<(usize, Vec<(usize, usize)>)>> = layout
        .iter()
        .map(|&block| {
            function
                .successors(block)
                .into_iter()
                .filter_map(|successor| at_index.get(&successor))
                .map(|&position| {
                    let arms = blocks[position]
                        .0
                        .iter()
                        .filter_map(|phi| Some((index[&phi.result?], index[&arm(phi, block)?])))
                        .collect();
                    (position, arms)
                })
                .collect()
        })
        .collect();
    let empty = Bits::new(values.len());
    let mut live_in = vec![empty.clone(); layout.len()];
    let mut live_out = vec![empty.clone(); layout.len()];
    let mut after_phis = vec![empty.clone(); layout.len()];

    let mut changing = true;
    while changing {
        changing = false;
        // Last block first: what a block makes live comes from the blocks
        // after it, so a chain settles in one round and a check, not in one
        // round per block.
        for position in (0..layout.len()).rev() {
            let mut out = empty.clone();
            for (successor, arms) in &successors[position] {
                out.union_with(&live_in[*successor]);
                for &(result, arm) in arms {
                    if after_phis[*successor].contains(result) {
                        out.insert(arm);
                    }
                }
            }
            let mut after = out.clone();
            after.subtract(&op_defines[position]);
            after.union_with(&exposed[position]);
            let mut inside = after.clone();
            inside.subtract(&phi_defines[position]);
            if out != live_out[position] || inside != live_in[position] || after != after_phis[position] {
                live_out[position] = out;
                live_in[position] = inside;
                after_phis[position] = after;
                changing = true;
            }
        }
    }
    let sets = |found: &[Bits]| -> BTreeMap<i64, IdSet<ValueId>> {
        layout.iter().zip(found).map(|(&block, set)| (id(block), set.iter().map(|one| values[one]).collect())).collect()
    };
    Liveness { live_in: sets(&live_in), live_out: sets(&live_out) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{block, function, parsed, value};

    /// PARITYCONTROL's dead join-flag phis kept ADD/ADC live as word
    /// operations.
    #[test]
    fn test_a_dead_phi_does_not_keep_its_edge_operand_live() {
        let module = parsed(
            "define void @f(i16 %x) {
b0:
  %incoming = add i16 %x, 1
  br label %b1

b1:
  %merged = phi i16 [ %incoming, %b0 ]
  ret void
}
",
        );
        let function = function(&module, "f");
        let found = live(function);
        assert!(!found.live_out[&id(block(function, "b0"))].contains(&value(function, "incoming")));

        let module = parsed(
            "define i16 @f(i16 %x) {
b0:
  %incoming = add i16 %x, 1
  br label %b1

b1:
  %merged = phi i16 [ %incoming, %b0 ]
  ret i16 %merged
}
",
        );
        let function = crate::testing::function(&module, "f");
        let found = live(function);
        assert!(
            found.live_out[&id(block(function, "b0"))].contains(&value(function, "incoming")),
            "a live phi keeps it"
        );
    }

    /// `@f` and its liveness.
    fn facts(text: &str) -> (llrm_mir::module::Module, Liveness) {
        let module = parsed(text);
        let found = live(function(&module, "f"));
        (module, found)
    }

    fn live_in(
        module: &llrm_mir::module::Module,
        found: &Liveness,
        at: &str,
        name: &str,
    ) -> bool {
        let f = function(module, "f");
        found.live_in[&id(block(f, at))].contains(&value(f, name))
    }

    fn live_out(
        module: &llrm_mir::module::Module,
        found: &Liveness,
        at: &str,
        name: &str,
    ) -> bool {
        let f = function(module, "f");
        found.live_out[&id(block(f, at))].contains(&value(f, name))
    }

    const LOOP: &str = "declare void @use(i16)

define i16 @f(i16 %n, i16 %x) {
pre:
  %before = add i16 %x, 1
  %across = add i16 %x, 2
  call void @use(i16 %before)
  br label %head

head:
  %i = phi i16 [ 0, %pre ], [ %next, %body ]
  %done = icmp eq i16 %i, %n
  br i1 %done, label %out, label %body

body:
  %next = add i16 %i, 1
  br label %head

out:
  %late = add i16 %across, %i
  ret i16 %late
}
";

    #[test]
    fn a_value_used_only_before_the_loop_is_not_live_inside_it() {
        let (module, found) = facts(LOOP);
        for at in ["head", "body"] {
            assert!(!live_in(&module, &found, at, "before"), "{at}");
        }
    }

    #[test]
    fn a_value_used_after_the_loop_is_live_throughout_it() {
        let (module, found) = facts(LOOP);
        for at in ["head", "body"] {
            assert!(live_in(&module, &found, at, "across") && live_out(&module, &found, at, "across"), "{at}");
        }
    }

    #[test]
    fn a_value_defined_after_the_loop_is_not_live_inside_it() {
        let (module, found) = facts(LOOP);
        for at in ["pre", "head", "body"] {
            assert!(!live_out(&module, &found, at, "late"), "{at}");
        }
    }

    #[test]
    fn a_loop_carried_value_is_live_out_of_the_latch_but_not_into_the_header() {
        let (module, found) = facts(LOOP);
        assert!(live_out(&module, &found, "body", "next"));
        assert!(!live_in(&module, &found, "head", "next"), "the phi reads it on the edge");
        assert!(!live_in(&module, &found, "head", "i"), "the phi defines it at the top");
        assert!(live_in(&module, &found, "body", "i"));
    }

    #[test]
    fn a_parameter_read_in_the_loop_is_live_across_the_loop_but_not_into_the_entry() {
        let (module, found) = facts(LOOP);
        assert!(live_out(&module, &found, "pre", "n"));
        assert!(live_out(&module, &found, "body", "n"), "the next trip compares it again");
        assert!(!live_in(&module, &found, "pre", "n"), "the caller supplies it");
        assert!(!live_out(&module, &found, "out", "n"));
    }

    #[test]
    fn a_value_read_on_one_arm_of_a_diamond_is_live_only_into_that_arm() {
        let (module, found) = facts(
            "define i16 @f(i1 %c, i16 %x) {
top:
  %y = add i16 %x, 1
  br i1 %c, label %left, label %right

left:
  br label %join

right:
  %z = add i16 %y, 1
  br label %join

join:
  %r = phi i16 [ 0, %left ], [ %z, %right ]
  ret i16 %r
}
",
        );
        assert!(live_in(&module, &found, "right", "y"));
        assert!(!live_in(&module, &found, "left", "y"));
        assert!(!live_in(&module, &found, "join", "y"));
        assert!(live_out(&module, &found, "right", "z"));
        assert!(!live_out(&module, &found, "left", "z"), "only the right edge carries it");
    }

    /// `homes` took the liveness of every value in a function to read that of
    /// its phis: the sets of every value built (43% of `mir homes` on a
    /// function of 256 joins). Asked of some values, each is live where it
    /// is in the whole and no other is in any set.
    #[test]
    fn test_the_liveness_of_some_values_is_theirs_in_the_whole_and_nothing_else() {
        let (module, whole) = facts(
            "define i16 @f(i1 %c, i16 %x) {
top:
  %y = add i16 %x, 1
  br i1 %c, label %left, label %right

left:
  br label %join

right:
  %z = add i16 %y, 1
  br label %join

join:
  %r = phi i16 [ 0, %left ], [ %z, %right ]
  %w = add i16 %r, %y
  ret i16 %w
}
",
        );
        let f = function(&module, "f");
        let (y, z) = (value(f, "y"), value(f, "z"));
        let some = live_where(f, &|one| one == y || one == z);
        for (blocks, kept) in [(&whole.live_in, &some.live_in), (&whole.live_out, &some.live_out)] {
            for (at, live) in blocks {
                let wanted: Vec<ValueId> = live.iter().filter(|one| *one == y || *one == z).collect();
                assert_eq!(kept[at].iter().collect::<Vec<_>>(), wanted, "block {at}");
            }
        }
        assert!(
            some.live_in
                .values()
                .chain(some.live_out.values())
                .flat_map(|set| set.iter())
                .all(|one| one == y || one == z)
        );
    }

    #[test]
    fn a_use_in_an_unreachable_block_keeps_nothing_live_in_the_reachable_ones() {
        let (module, found) = facts(
            "define i16 @f(i16 %x) {
top:
  %y = add i16 %x, 1
  ret i16 %x

dead:
  ret i16 %y
}
",
        );
        assert!(!live_out(&module, &found, "top", "y"));
    }

    #[test]
    fn a_phi_with_a_repeated_predecessor_keeps_its_input_live_on_that_edge() {
        let (module, found) = facts(
            "define i16 @f(i16 %s, i16 %x) {
top:
  %y = add i16 %x, 1
  switch i16 %s, label %join [ i16 1, label %join ]

join:
  %r = phi i16 [ %y, %top ], [ %y, %top ]
  ret i16 %r
}
",
        );
        assert!(live_out(&module, &found, "top", "y"));
    }

    #[test]
    fn a_declaration_has_nothing_live_and_no_pressure() {
        let module = parsed("declare i16 @f(i16)\n");
        let f = function(&module, "f");
        assert_eq!(live(f), Liveness { live_in: BTreeMap::new(), live_out: BTreeMap::new() });
        assert_eq!(pressure(f, None, None), 0);
    }

    #[test]
    fn pressure_counts_the_most_values_live_at_once_and_can_look_only_inside_a_loop() {
        let (module, found) = facts(LOOP);
        let f = function(&module, "f");
        let inside = llrm_mir::forest::Body::from_blocks([id(block(f, "head")), id(block(f, "body"))]);
        // %n, %across, %i and %done at the header's branch.
        assert_eq!(pressure(f, Some(&found), Some(&inside)), 4);
        assert_eq!(pressure(f, Some(&found), None), 4);
        assert_eq!(pressure(f, Some(&found), Some(&llrm_mir::forest::Body::from_blocks([id(block(f, "out"))]))), 2);
    }

    #[test]
    fn phi_inputs_are_the_edge_operands_of_live_phis_only() {
        let module = parsed(
            "define i16 @f(i1 %c, i16 %a, i16 %b) {
top:
  br i1 %c, label %left, label %join

left:
  br label %join

join:
  %used = phi i16 [ %a, %top ], [ 0, %left ]
  %unused = phi i16 [ %b, %top ], [ %b, %left ]
  ret i16 %used
}
",
        );
        let f = function(&module, "f");
        assert_eq!(phi_inputs(f, None), BTreeSet::from([value(f, "a")]));
    }

    #[test]
    fn entry_values_are_the_parameters_the_body_reads() {
        let module = parsed(
            "define i16 @f(i16 %read, i16 %ignored) {
top:
  %y = add i16 %read, 1
  ret i16 %y
}
",
        );
        let f = function(&module, "f");
        assert_eq!(entry_values(f), [value(f, "read")].into_iter().collect::<IdSet<_>>());
    }

    /// The bit-set fixed point made every block's sets as wide as the
    /// function's values: `branches` at N=1024 spent 82.7 G in gvn, 39% of
    /// it in liveness (4.3x, 5.2x, 5.7x per doubling). The work is now the
    /// live sets': a chain of N blocks that each make a value for the next
    /// costs steps in N, and doubling the chain doubles them.
    #[test]
    fn test_liveness_work_follows_the_live_sets_not_blocks_times_values() {
        let steps = |blocks: usize| {
            let mut text = String::from("define i16 @f(i16 %x) {\nb0:\n  %v0 = add i16 %x, 1\n  br label %b1\n\n");
            for at in 1..blocks {
                text += &format!("b{at}:\n  %v{at} = add i16 %v{}, 1\n  br label %b{}\n\n", at - 1, at + 1);
            }
            text += &format!("b{blocks}:\n  ret i16 %v{}\n}}\n", blocks - 1);
            let module = parsed(&text);
            STEPS.with(|steps| steps.set(0));
            live(function(&module, "f"));
            STEPS.with(|steps| steps.get())
        };
        let (small, large) = (steps(100), steps(200));
        assert!(small > 0 && large <= 2 * small + 8, "{small} steps for 100 blocks, {large} for 200");
    }

    /// The position table of a block was a slot for every instruction id up to
    /// the block's last (about 2,000 for a block at the end of a 2,000
    /// instruction function), made at every `ranges_of` of a value read in the
    /// block it is made in: 6.6 M instructions a call, 3.6% of an unrolled
    /// compile.
    #[test]
    fn test_ranges_of_position_tables_are_the_size_of_the_block() {
        let mut text = String::from("define i16 @f(i16 %x) {\nb0:\n  %v0 = add i16 %x, 1\n");
        for at in 1..2000 {
            text += &format!("  %v{at} = add i16 %v{}, 1\n", at - 1);
        }
        text += "  br label %b1\n\nb1:\n  %t = add i16 %v1999, 1\n  %u = add i16 %t, %t\n  ret i16 %u\n}\n";
        let module = parsed(&text);
        let function = function(&module, "f");
        let wanted: IdSet<ValueId> = [value(function, "t")].into_iter().collect();
        POSITION_SLOTS.with(|slots| slots.set(0));
        ranges_of(function, &wanted, None);
        let slots = POSITION_SLOTS.with(std::cell::Cell::get);
        assert!(slots <= 4, "{slots} slots for a block of 3 instructions");
    }

    /// The liveness of the values asked for, followed from their reads without
    /// a solve of the body, is what the solve gives for them: of all of
    /// them, and of each alone with the phis that take its value.
    #[test]
    fn the_liveness_of_some_values_is_what_the_solve_of_all_of_them_gives() {
        let texts = [
            super::tests::LOOP,
            "define i16 @f(i16 %x) {\nb0:\n  %incoming = add i16 %x, 1\n  br label %b1\n\nb1:\n  %merged = phi i16 [ %incoming, %b0 ]\n  ret i16 %merged\n}\n",
        ];
        for text in texts {
            let module = parsed(text);
            let function = function(&module, "f");
            let whole = live(function);
            let mut every = IdSet::<ValueId>::new();
            every.extend(function.parameters().iter().copied());
            every.extend(function.walk().filter_map(|(_, inst)| function.instruction(inst).result));
            let found = live_of(function, &every);
            for &block in function.layout() {
                let at = id(block);
                assert_eq!(found.live_in.get(&at).cloned().unwrap_or_default(), whole.live_in[&at], "in {at}");
                assert_eq!(found.live_out.get(&at).cloned().unwrap_or_default(), whole.live_out[&at], "out {at}");
            }
        }
    }

    /// A pass that touched a phi's arm named the phi, and the phis that take
    /// it, and their arms and so on: 19 values became 108 in quicksort -O3
    /// (13 in 14 before the body grew 4.4 times). A phi the pass left alone
    /// is taken as the model had it, live or not at its block, and is not
    /// followed.
    #[test]
    fn test_an_arm_is_followed_through_a_phi_that_is_not_wanted() {
        let module = parsed(
            "define i16 @f(i16 %x) {
b0:
  %arm = add i16 %x, 1
  br label %b1

b1:
  %merged = phi i16 [ %arm, %b0 ]
  ret i16 %merged
}
",
        );
        let function = function(&module, "f");
        let (b0, arm) = (id(block(function, "b0")), value(function, "arm"));
        let wanted: IdSet<ValueId> = [arm].into_iter().collect();
        let live_out = |settled: Option<&dyn Fn(ValueId, BlockId) -> bool>| {
            ranges_of(function, &wanted, settled).get(&arm).is_some_and(|range| range.out.contains(&b0))
        };
        assert!(!live_out(None), "no oracle: the phi is not followed");
        assert!(live_out(Some(&|_, _| false)), "the phi is read in its block (ret): the arm is live out of b0");
        let module = parsed(
            "define i16 @g(i16 %x) {
b0:
  %arm = add i16 %x, 1
  br label %b1

b1:
  %merged = phi i16 [ %arm, %b0 ]
  ret i16 0
}
",
        );
        let dead = crate::testing::function(&module, "g");
        let (b0, arm) = (id(block(dead, "b0")), value(dead, "arm"));
        let wanted: IdSet<ValueId> = [arm].into_iter().collect();
        let range = |settled: bool| {
            let oracle = move |_: ValueId, _: BlockId| settled;
            ranges_of(dead, &wanted, Some(&oracle)).get(&arm).cloned()
        };
        assert!(range(false).is_none(), "a dead phi keeps nothing live");
        assert!(range(true).is_some_and(|range| range.out.contains(&b0)), "a phi the model had live out keeps its arm");
    }
}
