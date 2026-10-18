//! The helpers [`crate::gvn`], [`crate::dead`], [`crate::decide`],
//! [`crate::unswitch`] and [`crate::loopexit`] share: value reuse, liveness of
//! values, and cleanup of phis after an edge goes.
//!
//! # How it works
//! `subexpressions` reuses a load: blocks are taken in layout order, and a
//! non-volatile load gets a key (`_computation`): its opcode, flags and type,
//! and its pointer as earlier replacements leave it. A load whose key an
//! earlier one has in its block, with nothing between the two that may write
//! its bytes (`_undisturbed`, from `memoryssa::Accesses`), reads the earlier
//! result and is erased; with `avoid_store_crossing` a store between them
//! refuses it too. Arithmetic, comparisons, addresses and casts are numbered
//! by `gvn::table`, as LLVM's GVN does; `joined` keys them with `_computation`
//! until scalar PRE replaces it.
//!
//! `equal_calls` makes two calls with one callee and arguments one when the
//! caller's `silent` says they touch no memory: the later takes the earlier's
//! result where the earlier has certainly run.
//!
//! `forwarded` is store-to-load forwarding across blocks: each load that
//! `avail::forwardable_walk` says a known value serves becomes that value.
//! `Crossings` answers whether a store lies on some path between the value's
//! definition and the load, for `avoid_store_crossing`; `Forwarding` keeps the
//! walk for the second numbering of one function.
//!
//! ```text
//!   before                        after
//!   %b = load %p                  %b = load %p
//!   store %v, %q                  store %v, %q   ; may not write %p's bytes
//!   %d = load %p                  (uses of %d read %b)
//! ```
//!
//! `live` marks, to a fixed point, every value that something staying reads;
//! `dead` removes the rest. `_comparison` finds the `icmp` in a block that
//! defines a branch's condition. `_trivial_phis` resolves joins after an edge
//! disappears: each phi keeps only the inputs of its remaining predecessors, a
//! phi with one other value is that value, and a phi with the arms of an
//! earlier phi in its block is that phi (LLVM's `EliminateDuplicatePHINodes`).
//!
//! # Example
//! No honest example: these are helpers shared by passes, and none changes a
//! function on its own. Read [`crate::gvn`] and its tests for them in use.
//!
//! # Provides
//! `subexpressions`, `equal_calls`, `forwarded` ([`crate::gvn`]); `live` and
//! `live_except` ([`crate::dead`], [`crate::floatloop`]); `_comparison`
//! ([`crate::decide`], [`crate::unswitch`]); `_trivial_phis`
//! ([`crate::decide`], [`crate::loopexit`], [`crate::unswitch`]).
//!
//! # Used by
//! [`crate::gvn`] calls `subexpressions`, `equal_calls` and `forwarded` on each
//! function it numbers; [`crate::dead`] and [`crate::floatloop`] call `live`
//! to decide what to delete; [`crate::decide`], [`crate::loopexit`] and
//! [`crate::unswitch`] call `_trivial_phis` after they drop an edge. Not a
//! pass: it runs only inside those.
//!
//! # Needs
//! `memoryssa::Accesses`, `avail` and `cfg::Dominance` (`llrm_analysis`).

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::memory;
use llrm_analysis::memory::MemRef;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::{avail, cfg, ssa};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::program::ProgramProxy;
use llrm_mir::types::TypeId;
use llrm_support::hash::IndexMap;

use crate::edges;
use crate::lcssa::{arms, from_arms};

/// Values something that stays reads, to a fixed point.
pub fn live(
    context: &Context,
    callees: &Callees,
    function: &Function,
) -> IdSet<ValueId> {
    live_except(context, callees, function, |_| false)
}

/// `live`, as if the instructions `skipped` picks did not stay.
pub fn live_except(
    context: &Context,
    callees: &Callees,
    function: &Function,
    skipped: impl Fn(InstId) -> bool,
) -> IdSet<ValueId> {
    let mut alive = IdSet::new();
    let mut pending = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| !skipped(inst) && crate::dead::_kept(context, callees, function, inst))
        .collect::<Vec<_>>();
    while let Some(inst) = pending.pop() {
        for &operand in &function.instruction(inst).operands {
            if let Operand::Value(value) = operand
                && alive.insert(value)
                && let ValueDef::Instruction(defining) = function.value(value).def
            {
                pending.push(defining);
            }
        }
    }
    alive
}

/// The comparison supplying `branch`'s condition: the `icmp` in `block`
/// that defines it.
///
/// The old one found the flag-setting `cmp`, or a `test`-like and/or/xor
/// read for equality, whose flags the branch read; both are an `icmp` here.
/// A float compare was refused as floating work, so `fcmp` is not one.
pub fn _comparison(
    function: &Function,
    block: BlockId,
    branch: InstId,
) -> Option<InstId> {
    let instruction = function.instruction(branch);
    if instruction.opcode != Opcode::Br || instruction.operands.len() != 3 {
        return None;
    }
    let Operand::Value(condition) = instruction.operands[0] else {
        return None;
    };
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .find(|&one| function.instruction(one).result == Some(condition))
        .filter(|&one| matches!(function.instruction(one).opcode, Opcode::ICmp(_)))
}

/// Blocks `_trivial_phis` looked at on this thread, for a test that a pass
/// calling it does not call it once per change.
#[cfg(test)]
thread_local! {
    pub(crate) static BLOCKS_VISITED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Resolve single-valued joins after an edge disappears: each phi keeps
/// only its predecessors' inputs, one naming a single other value is that
/// value, and one with an earlier phi's arms is that phi (LLVM's
/// `EliminateDuplicatePHINodes`).
pub fn _trivial_phis(function: &mut Function) -> Result<(), String> {
    let predecessors =
        function.layout().iter().map(|&block| (block, function.predecessors(block))).collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in function.layout().to_vec() {
            #[cfg(test)]
            BLOCKS_VISITED.with(|visited| visited.set(visited.get() + 1));
            for phi in edges::phis(function, block) {
                let result = function.instruction(phi).result.expect("a phi's value");
                let incoming = arms(function, phi)
                    .into_iter()
                    .filter(|(_, at)| predecessors[&block].contains(at))
                    .collect::<Vec<_>>();
                let mut values = incoming
                    .iter()
                    .map(|&(value, _)| value)
                    .filter(|&value| value != Operand::Value(result))
                    .collect::<Vec<_>>();
                values.dedup();
                if let [value] = values[..] {
                    function.replace_value(result, value);
                    function.set_operands(phi, Vec::new());
                    function.erase(phi)?;
                    changed = true;
                } else if from_arms(&incoming) != function.instruction(phi).operands {
                    function.set_operands(phi, from_arms(&incoming));
                }
            }
        }
        if !_duplicate_phis(function)? && !changed {
            return Ok(());
        }
    }
}

/// Each phi whose arms, edge for edge, are an earlier phi's in its block
/// replaced by that phi. Whether any was.
fn _duplicate_phis(function: &mut Function) -> Result<bool, String> {
    let mut changed = false;
    for block in function.layout().to_vec() {
        changed |= duplicate_phis_in(function, block)?;
    }
    Ok(changed)
}

/// `_duplicate_phis` of one `block`: LLVM's `EliminateDuplicatePHINodes(BB)`.
pub fn duplicate_phis_in(
    function: &mut Function,
    block: BlockId,
) -> Result<bool, String> {
    let mut changed = false;
    let mut seen = llrm_support::hash::HashMap::<(TypeId, Vec<(BlockId, Operand)>), ValueId>::default();
    for phi in edges::phis(function, block) {
        let op = function.instruction(phi);
        let (ty, result) = (op.ty, op.result.expect("a phi's value"));
        let mut incoming = arms(function, phi).into_iter().map(|(value, from)| (from, value)).collect::<Vec<_>>();
        incoming.sort_by_key(|&(from, _)| from);
        match seen.entry((ty, incoming)) {
            std::collections::hash_map::Entry::Occupied(earlier) => {
                function.replace_value(result, Operand::Value(*earlier.get()));
                function.set_operands(phi, Vec::new());
                function.erase(phi)?;
                changed = true;
            }
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(result);
            }
        }
    }
    Ok(changed)
}

// CSE only ever removes an operation whose whole answer is in its operands.
//
// By mnemonic, each old kind's instruction: `sub` is also Neg, `xor` Not,
// `getelementptr` PtrOffset and Address, `icmp` each comparison, the casts
// Convert, SignExtend and Extract. FixedMul, Smulhi and Concat were
// machine idioms with no one instruction; Copy has none. The pointer casts
// are the rich MIR's own: a segment made a pointer twice is one pointer.
pub const _PURE: [&str; 22] = [
    "add",
    "sub",
    "mul",
    "getelementptr",
    "udiv",
    "sdiv",
    "urem",
    "srem",
    "and",
    "or",
    "xor",
    "shl",
    "lshr",
    "ashr",
    "trunc",
    "zext",
    "sext",
    "icmp",
    "inttoptr",
    "ptrtoint",
    "bitcast",
    "addrspacecast",
];

/// The old `op.floating` kinds `_computation` took: arithmetic and the
/// conversions to and from integers. Fsqrt is an intrinsic call here.
pub fn _floating(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv)
            | Opcode::Cast(
                CastOp::FPTrunc
                    | CastOp::FPExt
                    | CastOp::FPRound
                    | CastOp::FPToUI
                    | CastOp::FPToSI
                    | CastOp::UIToFP
                    | CastOp::SIToFP
            )
    )
}

/// How far past a load the writes were looked at when a later load of the same
/// bytes was found equal to it: the next such load looks at the rest, not at
/// the whole stretch again (a block of N loads of one cell with a store
/// between each was N^2/2 asks).
struct Verified {
    upto: usize,
    read: MemRef,
    stored: bool,
}

/// One operation where two computed the same thing from the same values;
/// whether any went.
///
/// `avoid_store_crossing` keeps a load from being served across a store.
pub fn subexpressions(
    function: &mut Function,
    accesses: &Accesses,
    avoid_store_crossing: bool,
    program: Option<&ProgramProxy>,
    crossed: &std::cell::Cell<bool>,
) -> Result<bool, String> {
    let dominance = cfg::Dominance::of(function);
    let order: IndexMap<BlockId, usize> =
        function.layout().iter().enumerate().map(|(index, &block)| (block, index)).collect();

    let mut seen: IndexMap<_Computation, Vec<(usize, usize, InstId)>> = IndexMap::default();
    // What a name is rewritten to, and so what it numbers as.
    let mut swap: BTreeMap<ValueId, Operand> = BTreeMap::new();
    let mut gone: BTreeSet<InstId> = BTreeSet::new();
    let mut verified: llrm_mir::dense::IdMap<InstId, Verified> = llrm_mir::dense::IdMap::new();
    for block in dominators_first(function, &dominance) {
        let here = order[&block];
        let instructions = function.block(block).instructions();
        for (index, &inst) in instructions.iter().enumerate() {
            let op = function.instruction(inst);
            // Arithmetic is `gvn::table`'s; `joined` still keys it, with
            // `_computation`, until scalar PRE replaces it.
            if !matches!(op.opcode, Opcode::Load { .. }) {
                continue;
            }
            let Some(key) = _computation(op, &swap) else {
                continue;
            };
            let candidates = seen.entry(key).or_default();
            let first = candidates
                .iter()
                .rev()
                .find(|candidate| {
                    _reaches(candidate.0, candidate.1, here, index, &dominance, function, function.layout(), block)
                })
                .copied();
            let Some((at, where_, earlier)) = first else {
                candidates.push((here, index, inst));
                continue;
            };
            let loads = matches!(op.opcode, Opcode::Load { .. });
            if loads && at != here {
                candidates.push((here, index, inst));
                continue;
            }
            if loads {
                // The writes between the earlier load and this one: those up
                // to the last load it served were looked at then.
                let read = accesses.references.get(&inst);
                let kept = verified.get(&earlier).filter(|kept| kept.upto <= index && Some(&kept.read) == read);
                let (from, mut stored) = kept.map_or((where_ + 1, false), |kept| (kept.upto, kept.stored));
                let between = &instructions[from..index];
                if !_undisturbed(inst, between, accesses, program) {
                    candidates.push((here, index, inst));
                    continue;
                }
                stored |= between.iter().any(|&b| matches!(function.instruction(b).opcode, Opcode::Store { .. }));
                if let Some(read) = read {
                    verified.insert(earlier, Verified { upto: index, read: read.clone(), stored });
                }
                if stored {
                    // The one place `avoid_store_crossing` changes what is
                    // numbered: told, so a caller that runs this both
                    // ways can see when the second way is the first.
                    crossed.set(true);
                    if avoid_store_crossing {
                        candidates.push((here, index, inst));
                        continue;
                    }
                }
            }
            let (Some(mine), Some(theirs)) = (op.result, function.instruction(earlier).result) else {
                continue;
            };
            swap.insert(mine, Operand::Value(theirs));
            gone.insert(inst);
        }
    }

    if gone.is_empty() {
        return Ok(false);
    }
    // A leader is never itself swapped, so the order is free.
    for (&value, &with) in &swap {
        function.replace_value(value, with);
    }
    // Each block is rewritten once: erasing k of the n instructions of a block
    // one at a time is O(kn).
    function.erase_all(&gone.into_iter().collect::<Vec<_>>())?;
    Ok(true)
}

/// Two calls with the same callee and arguments that touch no memory (`silent`
/// says which) are one: the later one is the earlier's value, where the earlier
/// has certainly run. LLVM's GVN numbers a call that `doesNotAccessMemory` and
/// gcc's FRE a call to a const function. `silent` is asked only of calls that
/// have an equal. Whether anything changed.
pub fn equal_calls(
    function: &mut Function,
    silent: &dyn Fn(&Function, InstId) -> bool,
) -> Result<bool, String> {
    // By callee and arguments, in layout order: only groups of two or more
    // are looked at again.
    let mut groups: IndexMap<(TypeId, &[Operand]), Vec<(usize, usize, InstId)>> = IndexMap::default();
    for (at, &block) in function.layout().iter().enumerate() {
        for (index, &inst) in function.block(block).instructions().iter().enumerate() {
            let op = function.instruction(inst);
            if matches!(op.opcode, Opcode::Call(_)) && op.result.is_some() {
                groups.entry((op.ty, &op.operands[..])).or_default().push((at, index, inst));
            }
        }
    }
    let groups: Vec<Vec<(usize, usize, InstId)>> = groups
        .into_values()
        .filter(|calls| calls.len() > 1)
        .map(|calls| calls.into_iter().filter(|&(_, _, inst)| silent(function, inst)).collect::<Vec<_>>())
        .filter(|calls| calls.len() > 1)
        .collect();
    if groups.is_empty() {
        return Ok(false);
    }
    let dominance = cfg::Dominance::of(function);
    let layout = function.layout().to_vec();
    let rank: IndexMap<BlockId, usize> =
        dominators_first(function, &dominance).into_iter().enumerate().map(|(rank, block)| (block, rank)).collect();
    let mut groups = groups;
    for calls in &mut groups {
        calls.sort_by_key(|&(at, index, _)| (rank[&layout[at]], index));
    }
    let mut swap: Vec<(ValueId, ValueId)> = Vec::new();
    for calls in &groups {
        let mut leaders: Vec<(usize, usize, InstId)> = Vec::new();
        for &(at, index, inst) in calls {
            let earlier = leaders
                .iter()
                .rev()
                .find(|leader| _reaches(leader.0, leader.1, at, index, &dominance, function, &layout, layout[at]))
                .copied();
            match earlier {
                Some((_, _, leader)) => {
                    let (mine, theirs) = (function.instruction(inst).result, function.instruction(leader).result);
                    if let (Some(mine), Some(theirs)) = (mine, theirs) {
                        swap.push((mine, theirs));
                    }
                }
                None => leaders.push((at, index, inst)),
            }
        }
    }
    if swap.is_empty() {
        return Ok(false);
    }
    for &(mine, theirs) in &swap {
        function.replace_value(mine, Operand::Value(theirs));
        if let ValueDef::Instruction(inst) = function.value(mine).def {
            function.erase(inst)?;
        }
    }
    Ok(true)
}

/// Python's `_computation` key tuple.
///
/// `name` was the old operation's variant; it is the opcode, whose
/// predicate, cast or element type is part of it, with the flags. The
/// result widths are the result type. `operands` is `(unordered, named)`:
/// a commutative pair is sorted and deduplicated.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct _Computation {
    pub opcode: Opcode,
    pub flags: Flags,
    pub ty: TypeId,
    pub operands: (bool, Vec<Operand>),
}

/// What this operation computes, or None where that is not only its
/// operands. Each operand is named as `stands` rewrites it.
///
/// A load is its pointer's bytes, as an old load was its cell's; a
/// volatile one is the old barrier.
pub fn _computation(
    op: &Instruction,
    stands: &BTreeMap<ValueId, Operand>,
) -> Option<_Computation> {
    let floating = _floating(&op.opcode);
    let load = matches!(op.opcode, Opcode::Load { volatile: false, .. });
    if !(_PURE.contains(&op.opcode.mnemonic()) || load) && !floating {
        return None;
    }
    if op.result.is_none() || op.operands.is_empty() {
        return None;
    }
    let mut named = Vec::new();
    for &one in &op.operands {
        match one {
            Operand::Value(_) => named.push(ssa::provider(one, stands).ok()?),
            Operand::Constant(_) => named.push(one),
            Operand::Block(_) => return None,
        }
    }
    let unordered = named.len() == 2
        && matches!(
            op.opcode,
            Opcode::Binary(BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
                | Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne)
        );
    if unordered {
        // Any total order will do.
        if named[0] > named[1] {
            named.swap(0, 1);
        }
        if named[0] == named[1] {
            named.pop();
        }
    }
    Some(_Computation { opcode: op.opcode.clone(), flags: op.flags, ty: op.ty, operands: (unordered, named) })
}

/// The blocks with each before the ones it dominates: by dominator depth, not
/// by the layout, which a removed join leaves behind the blocks it came to
/// dominate. A numbering that serves a later computation from an earlier one
/// meets the dominating one first this way.
pub fn dominators_first(
    function: &Function,
    dominance: &cfg::Dominance,
) -> Vec<BlockId> {
    let depths = dominance.depths(function);
    let mut blocks = function.layout().to_vec();
    blocks.sort_by_key(|&block| depths.get(&cfg::id(block)).copied().filter(|&depth| depth > 0).unwrap_or(usize::MAX));
    blocks
}

/// Whether the earlier operation has certainly run by the later one.
pub fn _reaches(
    at: usize,
    where_: usize,
    then: usize,
    index: usize,
    dominance: &cfg::Dominance,
    function: &Function,
    layout: &[BlockId],
    block: BlockId,
) -> bool {
    if at == then {
        return where_ < index;
    }
    dominance.dominates(function, cfg::id(layout[at]), cfg::id(block))
}

thread_local! {
    static UNDISTURBED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static LOOKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many writes this thread has looked at for those asks, for a test that a
/// stretch of a block is looked at once.
pub fn undisturbed_looked() -> usize {
    LOOKED.with(std::cell::Cell::get)
}

/// How many times this thread has asked whether a load is undisturbed by a set
/// of writes, for a test that a pass asks it once.
pub fn undisturbed_asked() -> usize {
    UNDISTURBED.with(std::cell::Cell::get)
}

/// Whether the load `one` still reads what the load before it read, with
/// `between` run in between: nothing there may write its bytes.
///
/// `accesses` says what each writes; a call writes its footprint.
/// `regions::overlapping` decides against each write, on `program`, and an
/// answer it cannot give overlaps.
pub fn _undisturbed(
    one: InstId,
    between: &[InstId],
    accesses: &Accesses,
    program: Option<&ProgramProxy>,
) -> bool {
    UNDISTURBED.with(|asked| asked.set(asked.get() + 1));
    LOOKED.with(|looked| looked.set(looked.get() + between.len()));
    let Some(read) = accesses.references.get(&one) else {
        return false;
    };
    between.iter().all(|&other| llrm_analysis::memoryssa::spares(accesses, program, read, other))
}

/// The walk's answer for every load in one gvn run, kept for the run's second
/// numbering of the same function: it does not depend on
/// `avoid_store_crossing`.
#[derive(Default)]
pub struct Forwarding {
    walked: std::cell::OnceCell<Vec<avail::Forward>>,
}

impl Forwarding {
    fn served(
        &self,
        unit: &memory::Unit,
        accesses: &Accesses,
        want: &IdSet<InstId>,
    ) -> Vec<avail::Forward> {
        self.walked.get_or_init(|| avail::forwardable_walk(unit, accesses, want)).clone()
    }
}

/// Store-to-load forwarding: each load a known value serves becomes that
/// value; whether any did. `accesses` are `function`'s as it stands.
///
/// `crossed` is set when a value serves a load across a store, the one thing
/// `avoid_store_crossing` changes: where it is not set the run is the same
/// either way.
///
/// `avoid_store_crossing` keeps a value from serving a load when a store
/// lies on a path from its definition to the load.
pub fn forwarded(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    accesses: &Accesses,
    registers: &IndexMap<ValueId, llrm_analysis::consts::Known>,
    shape: &llrm_analysis::cfg::Shape,
    avoid_store_crossing: bool,
    forwarding: &Forwarding,
    crossed: &std::cell::Cell<bool>,
) -> Result<bool, String> {
    let want = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Load { .. }))
        .collect::<IdSet<_>>();
    if want.is_empty() {
        return Ok(false);
    }
    let unit = memory::Unit::within(context, layout, function, outer).with_registers(registers).with_shape(shape);
    let crossings = Crossings::of(function, &shape.reaching);
    // What each block holds is a fact of the instructions `function` has now,
    // which a caller that runs this twice on one function works out once.
    let served = forwarding
        .served(&unit, accesses, &want)
        .into_iter()
        .filter(|one| {
            let crosses = crossings.crosses(one.value, one.at);
            crossed.set(crossed.get() || crosses);
            !avoid_store_crossing || !crosses
        })
        .collect::<Vec<_>>();
    if served.is_empty() {
        return Ok(false);
    }
    // A load may be served by another served load.
    let replacements = served
        .iter()
        .map(|one| (function.instruction(one.at).result.expect("a load's value"), one.value))
        .collect::<BTreeMap<_, _>>();
    let ends = ssa::providers(replacements.values().copied(), |id| replacements.get(&id).copied())
        .map_err(|error| error.to_string())?;
    for (&value, end) in replacements.keys().zip(ends) {
        function.replace_value(value, end);
    }
    function.erase_all(&served.iter().map(|one| one.at).collect::<Vec<_>>())?;
    Ok(true)
}

/// Whether stores lie on paths between instructions, asked many times of one
/// function: where each instruction is and how many stores each block holds
/// before each position are found once, and the blocks that reach a block once
/// for each block asked of. Each ask was a scan of a block for the instruction,
/// twice, and of the function for what reaches the load: quadratic in a large
/// function.
struct Crossings<'f> {
    function: &'f Function,
    places: BTreeMap<InstId, (BlockId, i64)>,
    /// Per block, the stores among its first `n` instructions, at `n`.
    stores: BTreeMap<BlockId, Vec<u32>>,
    reaching: &'f cfg::Reaching,
}

impl<'f> Crossings<'f> {
    fn of(
        function: &'f Function,
        reaching: &'f cfg::Reaching,
    ) -> Self {
        let mut places = BTreeMap::new();
        let mut stores = BTreeMap::new();
        for &block in function.layout() {
            let mut counted = vec![0u32];
            for (index, &inst) in function.block(block).instructions().iter().enumerate() {
                places.insert(inst, (block, index as i64));
                counted.push(
                    counted[index] + u32::from(matches!(function.instruction(inst).opcode, Opcode::Store { .. })),
                );
            }
            stores.insert(block, counted);
        }
        Self { function, places, stores, reaching }
    }

    /// Whether a store lies on some path from `holder`'s definition to `load`,
    /// or `holder` is defined where no path from it reaches `load` first.
    fn crosses(
        &self,
        holder: Operand,
        load: InstId,
    ) -> bool {
        let function = self.function;
        let Operand::Value(holder) = holder else {
            return false;
        };
        let source = match function.value(holder).def {
            ValueDef::Instruction(inst) => self.places[&inst],
            _ => (function.entry().expect("a body"), -1),
        };
        let destination = self.places[&load];
        let stores_in = |block: BlockId, low: i64, high: i64| {
            let counted = &self.stores[&block];
            let (low, high) = (low.max(0) as usize, (high.max(0) as usize).min(counted.len() - 1));
            low < high && counted[high] > counted[low]
        };
        if source.0 == destination.0 {
            return source.1 >= destination.1 || stores_in(source.0, source.1 + 1, destination.1);
        }
        let reaching = self.reaching.to(function, destination.0);
        if !reaching.contains(&source.0) {
            return true;
        }
        let mut seen = IdSet::new();
        let mut work = vec![source.0];
        let mut arrived = false;
        while let Some(at) = work.pop() {
            if !reaching.contains(&at) || !seen.insert(at) {
                continue;
            }
            let low = if at == source.0 { source.1 + 1 } else { 0 };
            let high = if at == destination.0 { destination.1 } else { function.block(at).instructions().len() as i64 };
            if stores_in(at, low, high) {
                return true;
            }
            if at == destination.0 {
                arrived = true;
            } else {
                work.extend(function.successors(at));
            }
        }
        !arrived
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use llrm_analysis::memory::Unit;
    use llrm_analysis::memoryssa::Accesses;
    use llrm_mir::datalayout::DataLayout;
    use llrm_mir::module::Module;
    use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
    use llrm_mir::opcode::Opcode;
    use llrm_mir::passes::Outer;
    use llrm_support::hash::IndexMap;

    use super::{_trivial_phis, Crossings, forwarded, subexpressions};
    use crate::testing::{f, parsed, printed, results};

    /// `module`'s @f numbered.
    fn subexpressions_of(module: &mut Module) -> bool {
        let layout = DataLayout::default();
        let (_, _, function) =
            module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
        let accesses = Accesses::resolved(
            &llrm_analysis::testing::with_registers(Unit::of(module, &layout, function)),
            &IndexMap::default(),
        )
        .unwrap();
        subexpressions(f(module), &accesses, false, None, &std::cell::Cell::new(false)).unwrap()
    }

    /// `text` numbered: its printed form, and whether anything went. What
    /// `@f` returns for `inputs` is what it returned before.
    fn numbered(
        text: &str,
        inputs: &[&[i128]],
    ) -> (String, bool) {
        let before = parsed(text);
        let mut module: Module = before.clone();
        let changed = subexpressions_of(&mut module);
        let text = printed(&module);
        assert_eq!(results(&module, inputs), results(&before, inputs), "{text}");
        (text, changed)
    }

    /// `text`, left as it was.
    fn kept(text: &str) {
        let mut module = parsed(text);
        let before = printed(&module);
        assert!(!subexpressions_of(&mut module), "{before}");
        assert_eq!(printed(&module), before);
    }

    const XY: &[&[i128]] = &[&[0, 0], &[3, 5], &[-7, 2], &[0x7fff, 1]];

    /// A store to text memory misses @g on real-mode DOS: a load of @g is
    /// reused across it where the target is asked. It was not asked.
    #[test]
    fn test_a_load_is_reused_across_a_store_to_foreign_memory() {
        let text = format!(
            "{}@g = global i16 0

define i16 @f() {{
b0:
  %a = load i16, ptr @g
  %s = inttoptr i16 -18432 to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  store i16 1, ptr addrspace(1) %far
  %b = load i16, ptr @g
  %r = add i16 %a, %b
  ret i16 %r
}}
",
            llrm_analysis::testing::DOS
        );
        let reused = |dos: bool| {
            let mut module = parsed(&text);
            let layout = llrm_analysis::testing::layout(&module);
            let program = llrm_mir::program::ProgramProxy::of(&module, std::rc::Rc::new(llrm_x86_m16::Dos::default()));
            let program = dos.then_some(&*program);
            let (_, _, function) =
                module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
            let accesses = Accesses::resolved(
                &Unit { program, ..llrm_analysis::testing::with_registers(Unit::of(&module, &layout, function)) },
                &IndexMap::default(),
            )
            .unwrap();
            subexpressions(f(&mut module), &accesses, false, program, &std::cell::Cell::new(false)).unwrap()
        };
        assert!(reused(true) && !reused(false));
    }

    /// `text`'s @f forwarded, printed; what it returns for `XY` stays.
    fn forwarded_of(
        text: &str,
        avoid_store_crossing: bool,
    ) -> String {
        let before = parsed(text);
        let mut module = before.clone();
        let (layout, outer) = (llrm_analysis::testing::layout(&module), Outer::of(&module, None));
        let accesses = {
            let (_, _, function) =
                module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
            Accesses::resolved(
                &llrm_analysis::testing::with_registers(Unit::within(&module.context, &layout, function, &outer)),
                &IndexMap::default(),
            )
            .unwrap()
        };
        let (context, function) = module.function_mut("f").expect("@f");
        let registers = llrm_analysis::consts::known(&llrm_analysis::testing::with_registers(Unit::within(
            context, &layout, function, &outer,
        )));
        let shape = llrm_analysis::cfg::Shape::of(function);
        let changed = forwarded(
            context,
            &layout,
            function,
            &outer,
            &accesses,
            &registers,
            &shape,
            avoid_store_crossing,
            &super::Forwarding::default(),
            &std::cell::Cell::new(false),
        )
        .unwrap();
        let text = printed(&module);
        assert_eq!(changed, text != printed(&before), "{text}");
        assert_eq!(results(&module, XY), results(&before, XY), "{text}");
        text
    }

    /// The stored value serves the load of its cell; the load of the cell
    /// 8 bytes on stays.
    #[test]
    fn test_forwarding_extends_lifetime_without_conflating_shared_addresses() {
        let text = forwarded_of(
            "@g = global [8 x i16] zeroinitializer

define i16 @f(i16 %x, i16 %y) {
b0:
  store i16 %x, ptr @g
  %a = load i16, ptr @g
  %o = getelementptr i8, ptr @g, i16 8
  %b = load i16, ptr %o
  %r = add i16 %a, %b
  ret i16 %r
}
",
            false,
        );
        assert!(
            text.contains(
                "  store i16 %x, ptr @g
  %o = getelementptr i8, ptr @g, i16 8
  %b = load i16, ptr %o
  %r = add i16 %x, %b
"
            ),
            "{text}"
        );
    }

    const CROSSING: &str = "@g = global [8 x i16] zeroinitializer

define i16 @f(i16 %x, i16 %y) {
b0:
  store i16 %x, ptr @g
  %o = getelementptr i8, ptr @g, i16 8
  STORE
  %a = load i16, ptr @g
  ret i16 %a
}
";

    /// A store to another cell leaves the value held; `avoid_store_crossing`
    /// declines to hold it across one.
    #[test]
    fn test_a_value_crosses_a_store_only_when_allowed() {
        for store in ["store i16 %y, ptr %o", "store volatile i16 %y, ptr %o"] {
            let text = CROSSING.replace("STORE", store);
            assert!(
                forwarded_of(&text, false).contains(
                    "  ret i16 %x
"
                ),
                "{store}"
            );
            assert_eq!(forwarded_of(&text, true), printed(&parsed(&text)), "{store}");
        }
    }

    /// What may write the cell between stops the value.
    #[test]
    fn test_a_value_is_not_forwarded_past_what_may_write_its_cell() {
        for between in [
            "%v = getelementptr i8, ptr @g, i16 %y\n  store i16 %y, ptr %v",
            "store i8 1, ptr @g",
            "call void @h()",
            "store volatile i8 1, ptr @g",
        ] {
            let text = format!(
                "{}\ndefine void @h() {{\nb0:\n  store i16 3, ptr @g\n  ret void\n}}\n",
                CROSSING.replace("STORE", between)
            );
            assert_eq!(forwarded_of(&text, false), printed(&parsed(&text)), "{between}");
        }
    }

    /// `%a` and `%b` load `%p` with `between` in between, in @f of `head`,
    /// which starts with `pointers`.
    fn loads(
        head: &str,
        pointers: &str,
        between: &str,
    ) -> String {
        format!(
            "{head}
define i16 @f(i16 %x, i16 %y{pointers}) {{
b0:
  %a = load i16, ptr %p
{between}  %b = load i16, ptr %p
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        )
    }

    const LOCAL: &str = "  %p = alloca [2 x i16]\n  %q = alloca i16\n  store i16 %x, ptr %p\n";

    fn reused(text: String) {
        let (text, changed) = numbered(&text.replace("i16 %y) {\nb0:\n", &format!("i16 %y) {{\nb0:\n{LOCAL}")), XY);
        assert!(changed, "{text}");
        assert!(text.contains("%r = add i16 %a, %a"), "{text}");
    }

    #[test]
    fn test_a_load_is_reused_across_what_touches_no_memory() {
        reused(loads("", "", "  %n = add i16 %x, 1\n"));
    }

    #[test]
    fn test_a_load_is_reused_across_a_store_to_another_offset() {
        reused(loads("", "", "  %s = getelementptr i8, ptr %p, i16 2\n  store i16 %y, ptr %s\n"));
    }

    #[test]
    fn test_a_load_is_reused_across_a_store_to_another_alloca() {
        reused(loads("", "", "  store i16 %y, ptr %q\n"));
    }

    /// A volatile access touches only its own bytes: one of another alloca
    /// leaves the first load's value. Both were a barrier (#257).
    #[test]
    fn test_a_load_is_reused_across_a_volatile_access_of_another_alloca() {
        reused(loads("", "", "  store volatile i16 %y, ptr %q\n"));
        reused(loads("", "", "  %v = load volatile i16, ptr %q\n"));
    }

    /// Two noalias pointers' objects are apart; alias proves nothing of one
    /// noalias pointer against a plain one.
    #[test]
    fn test_a_load_through_a_noalias_pointer_is_reused_across_a_store_through_another() {
        let mut module = parsed(
            &loads("", "", "  store i16 %y, ptr %q\n")
                .replace("i16 %y) {", "i16 %y, ptr noalias %p, ptr noalias %q) {"),
        );
        assert!(subexpressions_of(&mut module), "{}", printed(&module));
        assert!(printed(&module).contains("%r = add i16 %a, %a"));
    }

    /// Neither alloca escaped, so the callee cannot reach them.
    #[test]
    fn test_a_load_is_reused_across_a_call_that_cannot_reach_it() {
        reused(loads(
            "define void @g(ptr %s) {\nb0:\n  store i16 9, ptr %s\n  ret void\n}\n",
            "",
            "  call void @g(ptr %q)\n",
        ));
    }

    #[test]
    fn test_a_load_is_refused_across_what_may_write_it() {
        let pointers = |between: &str| loads("declare void @g(ptr)\n", ", ptr %p, ptr %q", between);
        for between in [
            "  store i16 %y, ptr %q\n",
            "  call void @g(ptr %q)\n",
            "  call void @g(ptr %p)\n",
            "  store volatile i16 %y, ptr %q\n",
        ] {
            kept(&pointers(between));
        }
        kept(&pointers("").replace("%b = load i16", "%b = load volatile i16"));
    }

    /// The first load has not certainly run, nor did the old one look.
    #[test]
    fn test_a_load_in_another_block_is_loaded_again() {
        kept(
            "define i16 @f(ptr %p) {
b0:
  %a = load i16, ptr %p
  br label %b1

b1:
  %b = load i16, ptr %p
  %r = add i16 %a, %b
  ret i16 %r
}
",
        );
    }

    /// A dead arm and the block it came from go; the join it leaves with one
    /// value is that value.
    #[test]
    fn test_a_dead_arm_leaves_a_join_that_is_its_one_value() {
        let mut module = parsed(
            "define i16 @f(i16 %x) {
b0:
  br label %b2

b1:
  %y = add i16 %x, 1
  br label %b2

b2:
  %r = phi i16 [ %x, %b0 ], [ %y, %b1 ]
  %s = add i16 %r, 2
  ret i16 %s
}
",
        );
        let mut function = module.function_mut("f").unwrap().1.clone();
        llrm_analysis::cfg::_unreachable(&mut module.context, &mut function);
        _trivial_phis(&mut function).unwrap();
        *module.function_mut("f").unwrap().1 = function;
        assert_eq!(
            printed(&module),
            "define i16 @f(i16 %x) {
b0:
  br label %b2

b2:
  %s = add i16 %x, 2
  ret i16 %s
}
"
        );
    }

    /// A phi that reads only itself and one other value is that value.
    #[test]
    fn test_a_loop_phi_of_one_outside_value_is_that_value() {
        let mut module = parsed(
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  %r = phi i16 [ %x, %b0 ], [ %r, %b1 ]
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %r
}
",
        );
        _trivial_phis(module.function_mut("f").unwrap().1).unwrap();
        assert_eq!(
            printed(&module),
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %x
}
"
        );
    }

    /// SROA built a loop phi per slice beside promote's identical ones,
    /// and nbody carried twelve i32 values twice.
    #[test]
    fn a_phi_with_an_earlier_phis_arms_is_that_phi() {
        let mut module = parsed(
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  %r = phi i16 [ %x, %b0 ], [ %t, %b1 ]
  %s = phi i16 [ %t, %b1 ], [ %x, %b0 ]
  %t = add i16 %r, 1
  %u = add i16 %s, %t
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %u
}
",
        );
        _trivial_phis(module.function_mut("f").unwrap().1).unwrap();
        let after = printed(&module);
        assert!(after.matches("phi").count() == 1 && after.contains("add i16 %r, %t"), "{after}");
    }
    /// The scan the pass made for every served load (a block searched for the
    /// instruction twice, the function for what reaches the load), kept
    /// here as the reference: `Crossings` answers the same for every definition
    /// and load of a function with a diamond, a loop and stores in every
    /// block, and finds each position once.
    fn reference_crosses_store(
        function: &Function,
        holder: Operand,
        load: InstId,
    ) -> bool {
        let Operand::Value(holder) = holder else {
            return false;
        };
        let place = |inst: InstId| {
            let block = function.parent(inst).expect("placed");
            (
                block,
                function.block(block).instructions().iter().position(|&one| one == inst).expect("in its block") as i64,
            )
        };
        let source = match function.value(holder).def {
            ValueDef::Instruction(inst) => place(inst),
            _ => (function.entry().expect("a body"), -1),
        };
        let destination = place(load);
        let stores_in = |block: BlockId, low: i64, high: i64| {
            let instructions = function.block(block).instructions();
            let range = low.max(0) as usize..(high.max(0) as usize).min(instructions.len());
            instructions
                .get(range)
                .is_some_and(
                    |run| run.iter().any(|&one| matches!(function.instruction(one).opcode, Opcode::Store { .. })),
                )
        };
        if source.0 == destination.0 {
            return source.1 >= destination.1 || stores_in(source.0, source.1 + 1, destination.1);
        }
        let mut reaching = BTreeSet::from([destination.0]);
        let mut work = vec![destination.0];
        while let Some(at) = work.pop() {
            for parent in function.predecessors(at) {
                if reaching.insert(parent) {
                    work.push(parent);
                }
            }
        }
        if !reaching.contains(&source.0) {
            return true;
        }
        let mut seen = BTreeSet::new();
        let mut work = vec![source.0];
        let mut arrived = false;
        while let Some(at) = work.pop() {
            if !reaching.contains(&at) || !seen.insert(at) {
                continue;
            }
            let low = if at == source.0 { source.1 + 1 } else { 0 };
            let high = if at == destination.0 { destination.1 } else { function.block(at).instructions().len() as i64 };
            if stores_in(at, low, high) {
                return true;
            }
            if at == destination.0 {
                arrived = true;
            } else {
                work.extend(function.successors(at));
            }
        }
        !arrived
    }

    #[test]
    fn test_crossings_answer_as_the_scan_for_each_load_did() {
        let mut module = parsed(
            "@g = global i16 0
@h = global i16 0

define i16 @f(i16 %x, i1 %c, i16 %n) {
b0:
  %a = load i16, ptr @g
  store i16 %x, ptr @h
  br i1 %c, label %b1, label %b2

b1:
  %b = load i16, ptr @g
  store i16 %b, ptr @g
  br label %b3

b2:
  %d = load i16, ptr @h
  br label %b3

b3:
  %i = phi i16 [ 0, %b1 ], [ 0, %b2 ], [ %i2, %b3 ]
  %e = load i16, ptr @g
  %i2 = add i16 %i, 1
  %m = icmp slt i16 %i2, %n
  store i16 %e, ptr @h
  br i1 %m, label %b3, label %b4

b4:
  %r = load i16, ptr @h
  ret i16 %r
}
",
        );
        let function = f(&mut module);
        let reaching = llrm_analysis::cfg::Reaching::default();
        let crossings = Crossings::of(function, &reaching);
        let values: Vec<_> = function.walk().filter_map(|(_, inst)| function.instruction(inst).result).collect();
        let loads: Vec<_> = function
            .walk()
            .map(|(_, inst)| inst)
            .filter(|&inst| matches!(
                function.instruction(inst).opcode,
                llrm_mir::opcode::Opcode::Load { .. }
            ))
            .collect();
        assert!(loads.len() >= 5 && values.len() >= 8);
        for &holder in &values {
            for &load in &loads {
                let operand = llrm_mir::module::Operand::Value(holder);
                assert_eq!(
                    crossings.crosses(operand, load),
                    reference_crosses_store(function, operand, load),
                    "{holder:?} to {load:?}"
                );
            }
        }
    }

    /// The equal loads of a block of N went one at a time, each erase taking
    /// the block's list apart to find it: 351 M of 2.7 G on 800 statements
    /// (mem), 13% of the compile. They go together.
    #[test]
    fn test_equal_loads_are_erased_with_one_scan_of_the_block() {
        let mut text = String::from(
            "define i16 @f(i16 %n) {\nb0:\n  %i = alloca i16\n  store i16 0, ptr %i\n  br label %b1\n\nb1:\n",
        );
        for k in 0..100 {
            text += &format!("  %l{k} = load i16, ptr %i\n  %m{k} = add i16 %l{k}, %n\n");
        }
        text += "  store i16 %n, ptr %i\n  %c = icmp slt i16 %n, 0\n  br i1 %c, label %b1, label %b2\n\nb2:\n  ret i16 %n\n}\n";
        let mut module = parsed(&text);
        let before = llrm_mir::edit::instructions_scanned();
        subexpressions_of(&mut module);
        let scanned = llrm_mir::edit::instructions_scanned() - before;
        assert_eq!(printed(&module).matches("load").count(), 1, "{}", printed(&module));
        assert!(scanned <= 4 * 210, "{scanned} instructions scanned to erase 99 loads of a block of 210");
    }

    /// A block of N loads of one slot with a store to another between each
    /// looked at every store between the first load and each later one: N^2/2
    /// looks, a third of gvn on 800 statements of a loop (`spares`, 7.4 M
    /// calls). A stretch is looked at once.
    #[test]
    fn test_equal_loads_look_at_each_write_between_them_once() {
        let mut text = String::from(
            "define i16 @f(i16 %n) {\nb0:\n  %i = alloca i16\n  %s = alloca i16\n  store i16 0, ptr %i\n  br label %b1\n\nb1:\n",
        );
        for k in 0..40 {
            text += &format!("  %l{k} = load i16, ptr %i\n  %m{k} = add i16 %l{k}, %n\n  store i16 %m{k}, ptr %s\n");
        }
        text += "  store i16 %n, ptr %i\n  %c = icmp slt i16 %n, 0\n  br i1 %c, label %b1, label %b2\n\nb2:\n  ret i16 %n\n}\n";
        let mut module = parsed(&text);
        let looked = super::undisturbed_looked();
        subexpressions_of(&mut module);
        let looked = super::undisturbed_looked() - looked;
        assert_eq!(printed(&module).matches("load").count(), 1, "{}", printed(&module));
        assert!(looked <= 4 * 40, "{looked} writes looked at for 40 loads");
    }
}
