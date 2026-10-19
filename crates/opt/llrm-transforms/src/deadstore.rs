//! Dead store elimination: LLVM's `DSEPass`, stage 1 (`DSEState`,
//! `eliminateDeadDefs`, `getDomMemoryDef`,
//! `eliminateDeadWritesAtEndOfFunction`, `storeIsNoop`).
//!
//! # How it works
//! MemorySSA is built once. Every store and every `llvm.memset` of a constant
//! length is a candidate killing def. From a killing def the walk goes up its
//! defining accesses (`getDomMemoryDef`) to the nearest earlier def that it
//! completely overwrites, with nothing between that reads the bytes (a use of
//! the dead def that may read them ends the walk); a dead def reached through a
//! `MemoryPhi` is searched from each incoming access. The dead def is removed
//! (`deleteDeadInstruction`: its access leaves MemorySSA, its operands that
//! became trivially dead go with it). A def to a location that the caller
//! cannot see after the function returns (a local object) is killed also by
//! the function's exits: no path from it to a return reads it
//! (`eliminateDeadWritesAtEndOfFunction`, `isWriteAtEndOfFunction`), or every
//! path to an exit passes a block that overwrites it (the common
//! post-dominator check at the end of `getDomMemoryDef`). A store that writes
//! back the value it just loaded from the same place, or the byte the object
//! already holds, is a no-op (`storeIsNoop`).
//!
//! ```text
//!   store i16 1, ptr %p        (overwritten before any read)
//!   store i16 2, ptr %p   ==>  store i16 2, ptr %p
//!   %v = load i16, ptr %q
//!   ret i16 %v                 (a store to a local nothing reads is gone too)
//! ```
//!
//! MIRROR: `eliminateDeadStores`, `DSEState::eliminateDeadDefs` (both),
//! `getDomMemoryDef`, `isOverwrite` (without the masked and scalable cases),
//! `isWriteAtEndOfFunction`, `eliminateDeadWritesAtEndOfFunction`,
//! `isReadClobber`, `isCompleteOverwrite`, `isGuaranteedLoopIndependent`,
//! `isGuaranteedLoopInvariant`, `canSkipDef`, `isRemovable`, `isDSEBarrier`,
//! `deleteDeadInstruction`, `storeIsNoop`, `pushMemUses`.
//!
//! DEVIATE 1: alias questions are `mssa::alias` (llrm's far-pointer
//! semantics), not BasicAA's address-space view; the underlying object of a
//! location is the one object of its provenance, and "must alias with an
//! offset" is the same address with sizes (`AliasResult::Partial`). Exit:
//! none; the model is llrm's.
//! DEVIATE 2: nothing throws (no unwinding in MIR), so `ThrowingBlocks` is
//! empty and every local object is invisible on unwind. Exit: when MIR has EH.
//! DEVIATE 3: an object is invisible after the function returns when it is
//! a stack or frame object (`isa<AllocaInst>`); `dead_on_return` and byval
//! arguments (stage 3), and non-alloca objects with no capture (malloc) are
//! absent. Exit: stage 3.
//! DEVIATE 4: memory terminators (`lifetime.end`, `free`), the `initializes`
//! attribute, `tryFoldIntoCalloc` and
//! `eliminateRedundantStoresViaDominatingConditions` are stage 2 and 3. Partial
//! overwrites (`isPartialOverwrite`, `IOLs`,
//! `tryToMergePartialOverlappingStores`, `tryToShorten` of memsets) and
//! `eliminateRedundantStoresOfExistingValues` are in. Exit: stage 2.
//! DEVIATE 5: only stores and constant-length memsets are candidates, and
//! `storeIsNoop` knows no `getInitialValueOfAllocation` (it needs `calloc`
//! and friends). Exit: stage 2.
//!
//! # Example
//! A local written and never read:
//!
//! ```mir
//! %a = alloca i16
//! store i16 7, ptr %a     ; gone: nothing reads %a before the return
//! ret i16 0
//! ```
//!
//! # Runs
//! In the slot of `dse@820` (LLVM's order); the older `dse.rs` runs elsewhere.
//!
//! # Needs
//! MemorySSA (`mssa`), its alias query, dominators and post-dominators, the
//! loops.
//!
//! # Keeps
//! The blocks and edges: only instructions are removed.
//!
//! # See also
//! LLVM
//! [`DeadStoreElimination.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DeadStoreElimination.cpp).

use std::rc::Rc;

use llrm_analysis::memory::{MemoryKind, Unit as View};
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::mssa::alias::{AliasQuery, Location};
use llrm_analysis::mssa::updater::MemorySsaUpdater;
use llrm_analysis::mssa::{AccessId, Kind, Mssa, shadow};
use llrm_mir::context::Context;
use llrm_mir::domtree::{DomTree, PostDomTree};
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_support::hash::{HashMap, HashSet};

/// `MemorySSAScanLimit`.
const SCAN_LIMIT: u32 = 150;
/// `MemorySSAUpwardsStepLimit`.
const UPWARDS_STEP_LIMIT: u32 = 90;
/// `MemorySSAPartialStoreLimit`.
const PARTIAL_STORE_LIMIT: u32 = 5;
/// `MemorySSADefsPerBlockLimit`.
const DEFS_PER_BLOCK_LIMIT: usize = 5000;
/// `MemorySSASameBBStepCost`.
const SAME_BLOCK_STEP_COST: u32 = 1;
/// `MemorySSAOtherBBStepCost`.
const OTHER_BLOCK_STEP_COST: u32 = 5;
/// `MemorySSAPathCheckLimit`.
const PATH_CHECK_LIMIT: usize = 50;

thread_local! {
    static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The upward steps this thread's walks have taken, for a test that a chain of
/// overwrites is not walked once for each store.
pub fn steps() -> usize {
    STEPS.with(std::cell::Cell::get)
}

pub struct DeadStoreElimination;

impl FunctionPass for DeadStoreElimination {
    fn name(&self) -> &'static str {
        "dse"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let accesses = match Accesses::managed(unit.context, unit.layout, unit.function, analyses) {
            Ok(accesses) => accesses,
            Err(error) => panic!("dse: {error}"),
        };
        // MemorySSA describes the function as it is now; the removals go to
        // `unit.function` at the end, and the graph follows them through its
        // updater.
        let before = unit.function.fork();
        let frozen = unit.context.clone();
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
        let view = View::within(&frozen, unit.layout, &before, &outer).with_shape(&shape);
        let shape = view.shape();
        let mssa = Mssa::build(&before, &shape.dominance, &shadow::effect_of(&accesses, &before));
        let tree = DomTree::of(&before);
        let mut post = PostDomTree::new();
        post.recalculate(&before);
        let mut state = Dse::new(&frozen, &before, &view, &accesses, outer.callees(), mssa, &tree, &post, &shape.loops);
        let changed = state.run();
        let gone = std::mem::take(&mut state.to_remove);
        let shortened = std::mem::take(&mut state.shortened);
        let merged = std::mem::take(&mut state.merged);
        for (memset, (length, skip)) in shortened {
            if unit.function.is_erased(memset) || gone.contains(&memset) {
                continue;
            }
            let operands = unit.function.instruction(memset).operands.clone();
            let length_ty = unit.function.operand_type(unit.context, operands[2]).expect("a typed length");
            let new_length = Operand::Constant(unit.context.int(length_ty, i128::from(length)));
            let mut now = operands.clone();
            now[2] = new_length;
            if skip != 0 {
                let i8 = unit.context.types.int(8);
                let offset = Operand::Constant(unit.context.int(length_ty, i128::from(skip)));
                let ptr_ty = unit.function.operand_type(unit.context, operands[0]).expect("a typed pointer");
                let gep = unit
                    .function
                    .create_instruction(
                        Opcode::GetElementPtr { source: i8 },
                        ptr_ty,
                        vec![operands[0], offset],
                        llrm_mir::opcode::Flags::default(),
                        None,
                    );
                unit.function.insert(gep, llrm_mir::edit::Position::Before(memset)).expect("a placed instruction");
                now[0] = Operand::Value(unit.function.instruction(gep).result.expect("an address"));
            }
            unit.function.set_operands(memset, now);
        }
        for (store, value) in merged {
            if unit.function.is_erased(store) || gone.contains(&store) {
                continue;
            }
            let ty = unit
                .function
                .operand_type(unit.context, unit.function.instruction(store).operands[0])
                .expect("a typed operand");
            let constant = unit.context.int(ty, value as i128);
            let pointer = unit.function.instruction(store).operands[1];
            unit.function.set_operands(store, vec![Operand::Constant(constant), pointer]);
        }
        for inst in gone {
            if !unit.function.is_erased(inst) {
                unit.function.erase(inst).unwrap_or_else(|error| panic!("dse: {error}"));
            }
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// `OverwriteResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Overwrite {
    Complete,
    MaybePartial,
    None,
    Unknown,
    /// `OW_PartialEarlierWithFullLater`: a dead store overwritten by a smaller
    /// killing store inside it.
    PartialEarlierWithFullLater,
}

/// What `getDomMemoryDef` found: nothing, or an access (a def, or a phi the
/// caller searches through).
type Found = Option<AccessId>;

struct Dse<'a> {
    context: &'a Context,
    function: &'a Function,
    callees: &'a Callees,
    accesses: &'a Accesses,
    aa: AliasQuery<'a>,
    mssa: Mssa,
    updater: MemorySsaUpdater,
    dt: &'a DomTree,
    pdt: &'a PostDomTree,
    loops: &'a [llrm_analysis::graph::loops::Loop],
    /// Post-order numbers of the reachable blocks.
    post_order: HashMap<BlockId, u32>,
    /// All the defs that could kill others.
    mem_defs: Vec<AccessId>,
    /// Defs already deleted.
    skip_stores: HashSet<AccessId>,
    any_unreachable_exit: bool,
    /// Dead instructions to be removed at the end.
    to_remove: Vec<InstId>,
    /// `IOLs`: per partly overwritten instruction, the bytes the killing writes
    /// found so far cover, as end -> start of disjoint intervals.
    overlaps: HashMap<InstId, std::collections::BTreeMap<i64, i64>>,
    /// Stores whose constant became a merge with a killing store's: the
    /// store, and its new value.
    merged: HashMap<InstId, u128>,
    /// The order the partly overwritten instructions were first seen in.
    overlap_order: Vec<InstId>,
    /// The memsets to shorten: the new length, and the bytes taken off the
    /// front.
    shortened: HashMap<InstId, (i64, i64)>,
}

impl<'a> Dse<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        context: &'a Context,
        function: &'a Function,
        view: &'a View<'a>,
        accesses: &'a Accesses,
        callees: &'a Callees,
        mssa: Mssa,
        dt: &'a DomTree,
        pdt: &'a PostDomTree,
        loops: &'a [llrm_analysis::graph::loops::Loop],
    ) -> Self {
        let mut state = Self {
            context,
            function,
            callees,
            accesses,
            aa: AliasQuery::new(view, accesses),
            mssa,
            updater: MemorySsaUpdater::new(),
            dt,
            pdt,
            loops,
            post_order: HashMap::default(),
            mem_defs: Vec::new(),
            skip_stores: HashSet::default(),
            any_unreachable_exit: false,
            to_remove: Vec::new(),
            overlaps: HashMap::default(),
            merged: HashMap::default(),
            overlap_order: Vec::new(),
            shortened: HashMap::default(),
        };
        // Post-order numbers, and the defs that could kill.
        for (number, block) in post_order_of(function).into_iter().enumerate() {
            state.post_order.insert(block, number as u32);
            for inst in function.iter_block(block) {
                let Some(access) = state.mssa.get_memory_access(inst) else { continue };
                if state.mssa.access(access).kind == Kind::Def
                    && state.mem_defs.len() < DEFS_PER_BLOCK_LIMIT
                    && state.loc_for_write(inst).is_some()
                {
                    state.mem_defs.push(access);
                }
            }
        }
        state.any_unreachable_exit = pdt
            .roots()
            .iter()
            .any(
                |&root| function
                    .terminator(root)
                    .is_some_and(|last| matches!(function.instruction(last).opcode, Opcode::Unreachable)),
            );
        state
    }

    // ---- instructions -------------------------------------------------------

    fn inst_of(
        &self,
        access: AccessId,
    ) -> InstId {
        self.mssa.access(access).inst.expect("a def or use has an instruction")
    }

    /// `mayWriteToMemory`.
    fn may_write(
        &self,
        inst: InstId,
    ) -> bool {
        self.accesses.writes(inst).is_none_or(|writes| !writes.is_empty())
    }

    /// `mayReadFromMemory`.
    fn may_read(
        &self,
        inst: InstId,
    ) -> bool {
        self.accesses.reads(inst).is_none_or(|reads| !reads.is_empty())
    }

    fn is_store(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(self.function.instruction(inst).opcode, Opcode::Store { .. })
    }

    /// A `llvm.memset` call: its destination, byte and length.
    fn memset(
        &self,
        inst: InstId,
    ) -> Option<(Operand, Operand, Operand)> {
        memory::memset(self.context, self.callees, self.function, inst)
    }

    /// `getLocForWrite`: a store's place, or a memset's destination of a
    /// constant length.
    fn loc_for_write(
        &self,
        inst: InstId,
    ) -> Option<Rc<Location>> {
        if !self.may_write(inst) {
            return None;
        }
        if self.is_store(inst) {
            return self.aa.location_of(inst);
        }
        self.memset(inst)?;
        let filled = llrm_analysis::memory::MemRef::filled(self.aa.unit(), inst)?;
        // The provenance the accesses annotated, with the bytes the memset
        // fills.
        let provenance = match self.accesses.writes(inst) {
            Some([written]) => written.provenance.clone(),
            _ => filled.provenance.clone(),
        };
        let mut reference = llrm_analysis::memory::MemRef { provenance, ..filled };
        // A memset already shortened writes fewer bytes (LLVM changes the call
        // at once; the edit here is made at the end).
        if let Some(&(length, skip)) = self.shortened.get(&inst) {
            reference.disp += skip;
            reference.width = length as u32;
        }
        Some(self.aa.location_for(&reference, false))
    }

    /// `isRemovable`: not a volatile store or memset.
    fn is_removable(
        &self,
        inst: InstId,
    ) -> bool {
        match self.function.instruction(inst).opcode {
            Opcode::Store { volatile, .. } => !volatile,
            Opcode::Call(_) => self.memset(inst).is_some_and(|(_, _, _)| !self.memset_volatile(inst)),
            _ => false,
        }
    }

    fn memset_volatile(
        &self,
        inst: InstId,
    ) -> bool {
        let volatile = self.function.instruction(inst).operands[3];
        !matches!(
            volatile,
            Operand::Constant(id) if matches!(self.context.get(id).kind, llrm_mir::context::ConstantKind::Int(0))
        )
    }

    /// `isNoopIntrinsic`: lifetime markers.
    fn is_noop_intrinsic(
        &self,
        inst: InstId,
    ) -> bool {
        memory::lifetime(self.context, self.callees, self.function, inst).is_some()
    }

    // ---- objects ------------------------------------------------------------

    /// `getUnderlyingObject`: the one object of the location's provenance.
    fn object_of(
        &self,
        loc: &Location,
    ) -> Option<llrm_analysis::memory::ObjectRef> {
        let provenance = loc.reference.provenance.as_ref()?;
        let mut slices = provenance.slices.iter();
        let first = slices.next()?;
        slices.next().is_none().then_some(first.object)
    }

    /// The same object (`getUnderlyingObject` equal).
    fn same_object(
        &self,
        one: &Location,
        other: &Location,
    ) -> bool {
        match (self.object_of(one), self.object_of(other)) {
            (Some(a), Some(b)) => a.key == b.key && a.kind == b.kind,
            _ => false,
        }
    }

    /// `isIdentifiedObject`: a local or a global object.
    fn is_identified(object: &llrm_analysis::memory::ObjectRef) -> bool {
        matches!(
            object.kind,
            MemoryKind::Stack | MemoryKind::Frame | MemoryKind::Global
        )
    }

    /// `isInvisibleToCallerAfterRet`: a local (an alloca).
    fn invisible_after_return(
        &self,
        loc: &Location,
    ) -> bool {
        self.object_of(loc).is_some_and(|object| matches!(object.kind, MemoryKind::Stack | MemoryKind::Frame))
    }

    /// `isInvisibleToCallerOnUnwind`: nothing unwinds, so the same.
    fn invisible_on_unwind(
        &self,
        loc: &Location,
    ) -> bool {
        self.invisible_after_return(loc)
    }

    // ---- loops --------------------------------------------------------------

    /// The innermost loop of `block`, by its header (`CycleInfo::getCycle`).
    fn cycle_of(
        &self,
        block: BlockId,
    ) -> Option<i64> {
        self.loops
            .iter()
            .filter(|one| one.body.contains(&llrm_analysis::cfg::id(block)))
            .min_by_key(|one| one.body.len())
            .map(|one| one.header)
    }

    /// `isGuaranteedLoopInvariant`: the pointer is made outside every loop.
    fn loop_invariant(
        &self,
        loc: &Location,
    ) -> bool {
        let reference = &loc.reference;
        let defined_in_loop = |operand: Option<Operand>| {
            let Some(Operand::Value(value)) = operand else { return false };
            let ValueDef::Instruction(def) = self.function.value(value).def else { return false };
            let Some(block) = self.function.parent(def) else { return false };
            self.function.entry() != Some(block) && self.cycle_of(block).is_some()
        };
        !defined_in_loop(reference.root)
            && reference.base.is_none_or(|base| !defined_in_loop(Some(Operand::Value(base))))
    }

    /// `isGuaranteedLoopIndependent`.
    fn loop_independent(
        &self,
        current: InstId,
        killing: InstId,
        loc: &Location,
    ) -> bool {
        let (a, b) = (self.function.parent(current), self.function.parent(killing));
        if a == b {
            return true;
        }
        let (Some(a), Some(b)) = (a, b) else { return false };
        match self.cycle_of(a) {
            Some(cycle) if Some(cycle) == self.cycle_of(b) => true,
            _ => self.loop_invariant(loc),
        }
    }

    // ---- overwrite ----------------------------------------------------------

    /// `isOverwrite`: whether the store of `killing` completely overwrites the
    /// write of `dead`.
    fn is_overwrite(
        &self,
        killing_inst: InstId,
        dead_inst: InstId,
        killing: &Location,
        dead: &Location,
    ) -> Overwrite {
        self.is_overwrite_at(killing_inst, dead_inst, killing, dead).0
    }

    /// `isOverwrite` with the offsets of the two writes it found from their
    /// common base (`KillingOff`, `DeadOff`; 0 where it found none).
    fn is_overwrite_at(
        &self,
        killing_inst: InstId,
        dead_inst: InstId,
        killing: &Location,
        dead: &Location,
    ) -> (Overwrite, i64, i64) {
        let result = self.is_overwrite_inner(killing_inst, dead_inst, killing, dead);
        let (d, k) = (&dead.reference, &killing.reference);
        if matches!(result, Overwrite::Complete | Overwrite::MaybePartial)
            && d.root == k.root
            && d.base.is_none()
            && k.base.is_none()
        {
            (result, k.disp, d.disp)
        } else {
            (result, 0, 0)
        }
    }

    fn is_overwrite_inner(
        &self,
        killing_inst: InstId,
        dead_inst: InstId,
        killing: &Location,
        dead: &Location,
    ) -> Overwrite {
        if !self.loop_independent(dead_inst, killing_inst, dead) {
            return Overwrite::Unknown;
        }
        let (dead_object, killing_object) = (self.object_of(dead), self.object_of(killing));
        let same_object = self.same_object(dead, killing);
        // The killing store overwrites the whole object.
        if same_object {
            if let Some(object) = killing_object.filter(Self::is_identified) {
                if object.extent == Some(i64::from(killing.reference.width)) {
                    return Overwrite::Complete;
                }
            }
        }
        let (killing_size, dead_size) = (u64::from(killing.reference.width), u64::from(dead.reference.width));
        let result = self.aa.alias(killing, dead);
        // The same start: sizes decide.
        if result == llrm_analysis::mssa::alias::AliasResult::Must && killing_size >= dead_size {
            return Overwrite::Complete;
        }
        if result == llrm_analysis::mssa::alias::AliasResult::Partial && killing_size >= dead_size {
            return Overwrite::Complete;
        }
        // Different objects that cannot be told to alias, or overlap.
        if !same_object {
            return if result == llrm_analysis::mssa::alias::AliasResult::No {
                Overwrite::None
            } else {
                Overwrite::Unknown
            };
        }
        let _ = (dead_object, killing_object);
        // A base plus constant offset on both sides.
        let (d, k) = (&dead.reference, &killing.reference);
        if d.root != k.root || d.base.is_some() || k.base.is_some() || d.segment != k.segment {
            return Overwrite::Unknown;
        }
        let (dead_off, killing_off) = (d.disp, k.disp);
        if dead_off >= killing_off {
            if (dead_off - killing_off) as u64 + dead_size <= killing_size {
                return Overwrite::Complete;
            } else if ((dead_off - killing_off) as u64) < killing_size {
                return Overwrite::MaybePartial;
            }
        } else if ((killing_off - dead_off) as u64) < dead_size {
            return Overwrite::MaybePartial;
        }
        Overwrite::None
    }

    /// `isCompleteOverwrite`: `use_inst` completely overwrites `def_loc`.
    fn is_complete_overwrite(
        &self,
        def_loc: &Location,
        def_inst: InstId,
        use_inst: InstId,
    ) -> bool {
        // A def that does not write memory (a volatile load).
        if !self.may_write(use_inst) {
            return false;
        }
        match self.loc_for_write(use_inst) {
            Some(written) => self.is_overwrite(use_inst, def_inst, &written, def_loc) == Overwrite::Complete,
            None => false,
        }
    }

    /// `isReadClobber`: `use_inst` may read `def_loc`.
    fn is_read_clobber(
        &self,
        def_loc: &Location,
        use_inst: InstId,
    ) -> bool {
        if self.is_noop_intrinsic(use_inst) {
            return false;
        }
        // A store reads nothing (atomic orderings stronger than monotonic do
        // not exist in MIR).
        if self.is_store(use_inst) {
            return false;
        }
        if !self.may_read(use_inst) {
            return false;
        }
        self.aa.get_mod_ref_info(use_inst, def_loc).is_ref()
    }

    // ---- walks --------------------------------------------------------------

    /// `pushMemUses`.
    fn push_mem_uses(
        &self,
        access: AccessId,
        work: &mut Vec<AccessId>,
        visited: &mut HashSet<AccessId>,
    ) {
        let one = self.mssa.access(access);
        for &user in one.users.iter().chain(&one.phi_users) {
            if visited.insert(user) {
                work.push(user);
            }
        }
    }

    /// `canSkipDef`.
    fn can_skip_def(
        &self,
        def: AccessId,
    ) -> bool {
        self.is_noop_intrinsic(self.inst_of(def))
    }

    /// `isDSEBarrier`: nothing is atomic or throws.
    fn is_barrier(&self) -> bool {
        false
    }

    /// `isWriteAtEndOfFunction`.
    fn is_write_at_end_of_function(
        &self,
        def: AccessId,
        def_loc: &Location,
    ) -> bool {
        let mut work = Vec::new();
        let mut visited = HashSet::default();
        self.push_mem_uses(def, &mut work, &mut visited);
        let mut at = 0;
        while at < work.len() {
            if work.len() >= SCAN_LIMIT as usize {
                return false;
            }
            let use_access = work[at];
            at += 1;
            if self.mssa.access(use_access).kind == Kind::Phi {
                // AliasAnalysis does not account for loops.
                if !self.loop_invariant(def_loc) {
                    return false;
                }
                self.push_mem_uses(use_access, &mut work, &mut visited);
                continue;
            }
            let use_inst = self.inst_of(use_access);
            if self.is_read_clobber(def_loc, use_inst) {
                return false;
            }
            if self.mssa.access(use_access).kind == Kind::Def {
                self.push_mem_uses(use_access, &mut work, &mut visited);
            }
        }
        true
    }

    /// `getDomMemoryDef`.
    #[allow(clippy::too_many_arguments)]
    fn get_dom_memory_def(
        &mut self,
        killing_def: AccessId,
        start_access: AccessId,
        killing_loc: &Rc<Location>,
        scan_limit: &mut u32,
        walker_step_limit: &mut u32,
        partial_limit: &mut u32,
    ) -> Found {
        if *scan_limit == 0 || *walker_step_limit == 0 {
            return None;
        }
        let killing_inst = self.inst_of(killing_def);
        let mut current = start_access;
        // Only optimize the defining access of the killing def when directly
        // starting at it; the defining access must only access the location.
        let mut can_optimize =
            self.mssa.access(killing_def).defining == Some(start_access) && !self.may_read(killing_inst);
        let mut current_loc: Option<Rc<Location>>;
        loop {
            STEPS.with(|steps| steps.set(steps.get() + 1));
            // Reached the top.
            if self.mssa.is_live_on_entry_def(current) {
                if can_optimize && Some(current) != self.mssa.access(killing_def).defining {
                    self.mssa.set_optimized(killing_def, current);
                }
                return None;
            }
            let step_cost = if self.mssa.access(killing_def).block == self.mssa.access(current).block {
                SAME_BLOCK_STEP_COST
            } else {
                OTHER_BLOCK_STEP_COST
            };
            if *walker_step_limit <= step_cost {
                return None;
            }
            *walker_step_limit -= step_cost;
            // A phi: the caller searches through it.
            if self.mssa.access(current).kind == Kind::Phi {
                return Some(current);
            }
            let current_inst = self.inst_of(current);
            if self.can_skip_def(current) {
                can_optimize = false;
                current = self.mssa.access(current).defining.expect("a defining access");
                continue;
            }
            // Anything that looks like a barrier to further removal.
            if self.is_barrier() {
                return None;
            }
            // A read clobber on the path ends it (intrinsic calls are
            // understood by the caller).
            if self.may_read(current_inst) && self.is_read_clobber(killing_loc, current_inst) {
                return None;
            }
            // Quick check for direct uses that are read clobbers.
            let users: Vec<AccessId> = self.mssa.access(current).users.clone();
            for user in users {
                let one = self.mssa.access(user);
                if matches!(one.kind, Kind::Use | Kind::Def)
                    && !self.mssa.dominates(start_access, user)
                    && one.inst.is_some_and(|inst| self.is_read_clobber(killing_loc, inst))
                {
                    return None;
                }
            }
            // The write location, if the current def is removable.
            current_loc = self.loc_for_write(current_inst);
            let analyzable = current_loc.is_some() && self.is_removable(current_inst);
            if !analyzable {
                can_optimize = false;
                current = self.mssa.access(current).defining.expect("a defining access");
                continue;
            }
            let loc = current_loc.clone().expect("analyzable");
            if !self.loop_independent(current_inst, killing_inst, &loc) {
                can_optimize = false;
                current = self.mssa.access(current).defining.expect("a defining access");
                continue;
            }
            let result = self.is_overwrite(killing_inst, current_inst, killing_loc, &loc);
            if can_optimize {
                if Some(current) != self.mssa.access(killing_def).defining
                    && matches!(result, Overwrite::Complete | Overwrite::MaybePartial)
                {
                    self.mssa.set_optimized(killing_def, current);
                }
                if result != Overwrite::None {
                    can_optimize = false;
                }
            }
            if matches!(result, Overwrite::Unknown | Overwrite::None) {
                current = self.mssa.access(current).defining.expect("a defining access");
                continue;
            } else if result == Overwrite::MaybePartial {
                if *partial_limit <= 1 {
                    *walker_step_limit -= 1;
                    current = self.mssa.access(current).defining.expect("a defining access");
                    continue;
                }
                *partial_limit -= 1;
            }
            break;
        }
        // Accesses to objects accessible after the function returns can only be
        // eliminated if dead along all paths to the exit.
        let mut killing_defs: Vec<InstId> = vec![killing_inst];
        let maybe_dead_access = current;
        let maybe_dead_loc = current_loc.expect("a location");
        let maybe_dead_inst = self.inst_of(maybe_dead_access);
        let mut work: Vec<AccessId> = Vec::new();
        let mut visited: HashSet<AccessId> = HashSet::default();
        self.push_mem_uses(maybe_dead_access, &mut work, &mut visited);
        // Check if the dead def may be read.
        let mut at = 0;
        while at < work.len() {
            let use_access = work[at];
            // Bail out if the number of accesses to check exceeds the scan
            // limit.
            if *scan_limit < (work.len() - at) as u32 {
                return None;
            }
            *scan_limit -= 1;
            at += 1;
            if self.mssa.access(use_access).kind == Kind::Phi {
                let phi_block = self.mssa.access(use_access).block;
                if killing_defs.iter().any(|&inst| {
                    self.function
                        .parent(inst)
                        .is_some_and(|block| block != phi_block && self.dt.dominates(block, phi_block))
                }) {
                    continue;
                }
                self.push_mem_uses(use_access, &mut work, &mut visited);
                continue;
            }
            let use_inst = self.inst_of(use_access);
            if killing_defs.iter().any(|&inst| self.instruction_dominates(inst, use_inst)) {
                continue;
            }
            if self.is_noop_intrinsic(use_inst) {
                self.push_mem_uses(use_access, &mut work, &mut visited);
                continue;
            }
            // Uses which may read the original def mean it cannot be
            // eliminated.
            if self.is_read_clobber(&maybe_dead_loc, use_inst) {
                return None;
            }
            // If the walk comes back to the original access (and the pointer is
            // not loop invariant), a store does not kill itself.
            if maybe_dead_access == use_access && !self.loop_invariant(&maybe_dead_loc) {
                return None;
            }
            if killing_def == use_access || maybe_dead_access == use_access {
                continue;
            }
            // Check all uses of defs, except for defs completely overwriting
            // the original location.
            if self.mssa.access(use_access).kind == Kind::Def {
                if self.is_complete_overwrite(&maybe_dead_loc, maybe_dead_inst, use_inst) {
                    let killing_block = self.function.parent(use_inst).expect("a placed instruction");
                    let dead_block = self.mssa.access(maybe_dead_access).block;
                    if self.post_order[&killing_block] < self.post_order[&dead_block] {
                        if !self.invisible_after_return(killing_loc) {
                            killing_defs.push(use_inst);
                        }
                    } else {
                        return None;
                    }
                } else {
                    self.push_mem_uses(use_access, &mut work, &mut visited);
                }
            }
        }
        // For locations visible after the function returns, the location must
        // be overwritten along all paths from the dead def to the exit.
        if !self.invisible_after_return(killing_loc) {
            let mut killing_blocks: Vec<BlockId> = Vec::new();
            for &inst in &killing_defs {
                let block = self.function.parent(inst).expect("a placed instruction");
                if !killing_blocks.contains(&block) {
                    killing_blocks.push(block);
                }
            }
            assert!(!killing_blocks.is_empty(), "Expected at least a single killing block");
            let mut common = Some(killing_blocks[0]);
            for &block in &killing_blocks[1..] {
                let Some(so_far) = common else { break };
                common = self.pdt.find_nearest_common_dominator(self.function, so_far, block);
            }
            let dead_block = self.mssa.access(maybe_dead_access).block;
            // Every path from the dead def to an exit goes through a killing
            // block.
            let post_dominates = common.is_some_and(|block| self.pdt.dominates(block, dead_block));
            if !post_dominates {
                if !self.any_unreachable_exit {
                    return None;
                }
                common = None;
            }
            if common.is_some_and(|block| killing_blocks.contains(&block)) {
                return Some(maybe_dead_access);
            }
            let mut blocks: Vec<BlockId> = Vec::new();
            match common {
                Some(block) => blocks.push(block),
                None => {
                    for &root in self.pdt.roots() {
                        let unreachable = self
                            .function
                            .terminator(root)
                            .is_some_and(|last| matches!(self.function.instruction(last).opcode, Opcode::Unreachable));
                        if !unreachable && !blocks.contains(&root) {
                            blocks.push(root);
                        }
                    }
                }
            }
            let mut at = 0;
            while at < blocks.len() {
                let block = blocks[at];
                at += 1;
                if killing_blocks.contains(&block) {
                    continue;
                }
                if block == dead_block {
                    return None;
                }
                if !self.dt.is_reachable_from_entry(block) {
                    continue;
                }
                for predecessor in self.function.predecessors(block) {
                    if !blocks.contains(&predecessor) {
                        blocks.push(predecessor);
                    }
                }
                if blocks.len() >= PATH_CHECK_LIMIT {
                    return None;
                }
            }
        }
        // No aliasing uses of the dead def: it is potentially dead.
        Some(maybe_dead_access)
    }

    /// `DT.dominates(Instruction, Instruction)`.
    fn instruction_dominates(
        &self,
        def: InstId,
        user: InstId,
    ) -> bool {
        if def == user {
            return true;
        }
        let (Some(a), Some(b)) = (self.function.parent(def), self.function.parent(user)) else { return false };
        if a != b {
            return self.dt.dominates(a, b);
        }
        self.function.comes_before(def, user)
    }

    // ---- removal ------------------------------------------------------------

    /// `deleteDeadInstruction`: `inst` leaves MemorySSA and the function, and
    /// the operands that were only its own go with it.
    fn delete_dead_instruction(
        &mut self,
        inst: InstId,
        deleted: Option<&mut HashSet<AccessId>>,
    ) {
        let mut now_dead = vec![inst];
        let mut deleted = deleted;
        while let Some(dead) = now_dead.pop() {
            if let Some(access) = self.mssa.get_memory_access(dead) {
                if self.mssa.access(access).kind == Kind::Def {
                    self.skip_stores.insert(access);
                    if let Some(deleted) = deleted.as_deref_mut() {
                        deleted.insert(access);
                    }
                }
                self.updater.remove_memory_access(&mut self.mssa, self.function, access, true);
            }
            self.overlaps.remove(&dead);
            // Its operands that were only its own.
            let operands: Vec<Operand> = self.function.instruction(dead).operands.clone();
            self.to_remove.push(dead);
            for operand in operands {
                let Operand::Value(value) = operand else { continue };
                let ValueDef::Instruction(producer) = self.function.value(value).def else { continue };
                if self.to_remove.contains(&producer) || now_dead.contains(&producer) {
                    continue;
                }
                if self.trivially_dead(producer, dead) {
                    now_dead.push(producer);
                }
            }
        }
    }

    /// `isInstructionTriviallyDead`, after `gone` is out: its result is used by
    /// nothing else, and it has no effects.
    fn trivially_dead(
        &self,
        inst: InstId,
        gone: InstId,
    ) -> bool {
        let instruction = self.function.instruction(inst);
        let Some(result) = instruction.result else { return false };
        let pure = matches!(
            instruction.opcode,
            Opcode::GetElementPtr { .. } | Opcode::Cast(_) | Opcode::Binary(..) | Opcode::Select
        );
        pure && self.function.users(result).iter().all(|one| one.user == gone || self.to_remove.contains(&one.user))
    }

    // ---- driver -------------------------------------------------------------

    /// `tryToMergePartialOverlappingStores`: the constant the earlier, larger
    /// store holds once the killing store's bytes are in it.
    fn try_to_merge_partial_overlapping_stores(
        &self,
        killing: InstId,
        dead: InstId,
        killing_off: i64,
        dead_off: i64,
    ) -> Option<u128> {
        let unit = self.aa.unit();
        let (dead_value, killing_value) =
            (self.function.instruction(dead).operands[0], self.function.instruction(killing).operands[0]);
        let dead_bits = unit.int_bits(dead_value)?;
        let killing_bits = unit.int_bits(killing_value)?;
        // Both stores hold constants of whole bytes, and neither is volatile.
        if dead_bits % 8 != 0 || killing_bits % 8 != 0 || dead_bits > 128 {
            return None;
        }
        let dead_constant = self.merged.get(&dead).copied().or_else(|| unit.int_constant(dead_value))?;
        let killing_constant = self.merged.get(&killing).copied().or_else(|| unit.int_constant(killing_value))?;
        if !self.is_removable(killing) || !self.is_removable(dead) {
            return None;
        }
        if !self.memory_is_not_modified_between(dead, killing) {
            return None;
        }
        let shift = ((killing_off - dead_off) * 8) as u32;
        let mask_of = |bits: u32| if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
        let mask = mask_of(killing_bits) << shift;
        Some(((dead_constant & !mask) | ((killing_constant & mask_of(killing_bits)) << shift)) & mask_of(dead_bits))
    }

    /// `isModSet(AA.getModRefInfo(inst, loc))`: a memset writes the bytes it
    /// fills (`MemoryLocation::getForDest`), not its destination object.
    fn writes_location(
        &self,
        inst: InstId,
        loc: &Location,
    ) -> bool {
        if self.memset(inst).is_some() {
            if let Some(written) = self.loc_for_write(inst) {
                return self.aa.alias(&written, loc) != llrm_analysis::mssa::alias::AliasResult::No;
            }
        }
        self.aa.get_mod_ref_info(inst, loc).is_mod()
    }

    /// `memoryIsNotModifiedBetween`: nothing between `first` and `second`
    /// (`second` dominated by `first`) writes the bytes `second` writes. The
    /// address is not translated through phis: a pointer made by a phi ends the
    /// walk (DEVIATE).
    fn memory_is_not_modified_between(
        &self,
        first: InstId,
        second: InstId,
    ) -> bool {
        let Some(loc) = self.loc_for_write(second) else { return false };
        let (first_block, second_block) = (
            self.function.parent(first).expect("a placed instruction"),
            self.function.parent(second).expect("a placed instruction"),
        );
        let mut visited: Vec<BlockId> = Vec::new();
        let mut work: Vec<(BlockId, bool)> = vec![(second_block, true)];
        while let Some((block, is_first)) = work.pop() {
            // The block after `first` where it holds it, up to `second` where
            // that is the block's end of the walk; empty if `first` is not
            // before.
            let backwards = block == first_block && is_first && !self.function.comes_before(first, second);
            let rest: Box<dyn Iterator<Item = InstId> + '_> = match (backwards, block == first_block) {
                (true, _) => Box::new(std::iter::empty()),
                (false, true) => Box::new(self.function.iter_after(first)),
                (false, false) => Box::new(self.function.iter_block(block)),
            };
            let until = is_first.then_some(second);
            for inst in rest.take_while(|&one| Some(one) != until) {
                if inst != second
                    && !self.to_remove.contains(&inst)
                    && self.may_write(inst)
                    && self.writes_location(inst, &loc)
                {
                    return false;
                }
            }
            if block != first_block {
                for predecessor in self.function.predecessors(block) {
                    if !visited.contains(&predecessor) {
                        visited.push(predecessor);
                        work.push((predecessor, false));
                    }
                }
            }
        }
        true
    }

    /// `isPartialOverwrite` with `EnablePartialOverwriteTracking`: the part of
    /// the dead write the killing write covers joins the intervals found for
    /// it, and together they may cover all of it.
    /// (`OW_PartialEarlierWithFullLater` and the shortening of a store are
    /// stage 2.)
    fn is_partial_overwrite(
        &mut self,
        killing: &Location,
        dead: &Location,
        killing_off: i64,
        dead_off: i64,
        dead_inst: InstId,
    ) -> Overwrite {
        let (killing_size, dead_size) = (i64::from(killing.reference.width), i64::from(dead.reference.width));
        if killing_off < dead_off + dead_size && killing_off + killing_size >= dead_off {
            if !self.overlaps.contains_key(&dead_inst) {
                self.overlap_order.push(dead_inst);
            }
            let map = self.overlaps.entry(dead_inst).or_default();
            let (mut start, mut end) = (killing_off, killing_off + killing_size);
            // Intervals ending at or after the start that begin before the end.
            let mut merging: Vec<i64> = Vec::new();
            for (&interval_end, &interval_start) in map.range(start..) {
                if interval_start <= end {
                    start = start.min(interval_start);
                    end = end.max(interval_end);
                    merging.push(interval_end);
                } else {
                    break;
                }
            }
            for key in merging {
                map.remove(&key);
            }
            map.insert(end, start);
            let (&first_end, &first_start) = map.iter().next().expect("one interval");
            if first_start <= dead_off && first_end >= dead_off + dead_size {
                return Overwrite::Complete;
            }
        }
        // The dead store writes every byte the killing store writes.
        if killing_off >= dead_off
            && dead_off + dead_size > killing_off
            && ((killing_off - dead_off) + killing_size) <= dead_size
        {
            return Overwrite::PartialEarlierWithFullLater;
        }
        Overwrite::Unknown
    }

    /// `tryToShorten`: `dead` (a memset) loses the bytes the killing intervals
    /// cover at its end or its start. (A memset has no destination alignment
    /// in MIR: the alignment of the remaining part is 1.)
    fn try_to_shorten(
        &mut self,
        dead: InstId,
        dead_start: &mut i64,
        dead_size: &mut u64,
        killing_start: i64,
        killing_size: u64,
        overwrite_end: bool,
    ) -> bool {
        let (remove_size, begin) = if overwrite_end {
            let remove_start = killing_start;
            if *dead_size <= (remove_start - *dead_start) as u64 {
                return false;
            }
            (*dead_size - (remove_start - *dead_start) as u64, false)
        } else {
            (killing_size - (*dead_start - killing_start) as u64, true)
        };
        let new_size = *dead_size - remove_size;
        let entry = self.shortened.entry(dead).or_insert((*dead_size as i64, 0));
        entry.0 = new_size as i64;
        if begin {
            entry.1 += remove_size as i64;
            *dead_start += remove_size as i64;
        }
        *dead_size = new_size;
        true
    }

    /// `removePartiallyOverlappedStores`: for each partly overwritten memset,
    /// the part its killing intervals cover at the end, then at the start.
    fn remove_partially_overlapped_stores(&mut self) -> bool {
        let mut changed = false;
        for dead in self.overlap_order.clone() {
            let Some(map) = self.overlaps.get(&dead).cloned() else { continue };
            let Some(loc) = self.loc_for_write(dead) else { continue };
            if self.is_store(dead) || self.memset(dead).is_none() {
                continue;
            }
            let (mut dead_start, mut dead_size) = (loc.reference.disp, u64::from(loc.reference.width));
            let mut map = map;
            // The end.
            if let Some((&end, &start)) = map.iter().next_back() {
                let killing_size = (end - start) as u64;
                if start > dead_start
                    && ((start - dead_start) as u64) < dead_size
                    && killing_size >= dead_size - (start - dead_start) as u64
                    && self.try_to_shorten(dead, &mut dead_start, &mut dead_size, start, killing_size, true)
                {
                    map.remove(&end);
                    changed = true;
                }
            }
            if map.is_empty() {
                continue;
            }
            // The start.
            if let Some((&end, &start)) = map.iter().next() {
                let killing_size = (end - start) as u64;
                if start <= dead_start
                    && killing_size > (dead_start - start) as u64
                    && self.try_to_shorten(dead, &mut dead_start, &mut dead_size, start, killing_size, false)
                {
                    map.remove(&end);
                    changed = true;
                }
            }
        }
        changed
    }

    /// `eliminateDeadDefs` for one killing def: whether anything changed.
    fn eliminate_dead_defs_of(
        &mut self,
        killing_def: AccessId,
        killing_loc: Rc<Location>,
    ) -> (bool, bool) {
        let mut changed = false;
        let mut scan_limit = SCAN_LIMIT;
        let mut walker_step_limit = UPWARDS_STEP_LIMIT;
        let mut partial_limit = PARTIAL_STORE_LIMIT;
        let mut to_check: Vec<AccessId> = vec![self.mssa.access(killing_def).defining.expect("a defining access")];
        let mut deleted: HashSet<AccessId> = HashSet::default();
        let killing_inst = self.inst_of(killing_def);
        let mut at = 0;
        while at < to_check.len() {
            let current = to_check[at];
            at += 1;
            if deleted.contains(&current) {
                continue;
            }
            let Some(dead_access) = self.get_dom_memory_def(
                killing_def,
                current,
                &killing_loc,
                &mut scan_limit,
                &mut walker_step_limit,
                &mut partial_limit,
            ) else {
                continue;
            };
            if self.mssa.access(dead_access).kind == Kind::Phi {
                let phi_block = self.mssa.access(dead_access).block;
                let incoming: Vec<(BlockId, AccessId)> = self.mssa.access(dead_access).incoming.clone();
                for (_, incoming_access) in incoming {
                    let incoming_block = self.mssa.access(incoming_access).block;
                    // Only incoming accesses that come before the phi.
                    if self.post_order.get(&incoming_block) > self.post_order.get(&phi_block)
                        && !to_check.contains(&incoming_access)
                    {
                        to_check.push(incoming_access);
                    }
                }
                continue;
            }
            let dead_inst = self.inst_of(dead_access);
            let dead_loc = self.loc_for_write(dead_inst).expect("a removable def has a location");
            let defining = self.mssa.access(dead_access).defining.expect("a defining access");
            if !to_check.contains(&defining) {
                to_check.push(defining);
            }
            let (mut result, killing_off, dead_off) =
                self.is_overwrite_at(killing_inst, dead_inst, &killing_loc, &dead_loc);
            if result == Overwrite::MaybePartial {
                result = self.is_partial_overwrite(&killing_loc, &dead_loc, killing_off, dead_off, dead_inst);
            }
            if result == Overwrite::PartialEarlierWithFullLater
                && self.is_store(dead_inst)
                && self.is_store(killing_inst)
                && self.instruction_dominates(dead_inst, killing_inst)
            {
                if let Some(value) =
                    self.try_to_merge_partial_overlapping_stores(killing_inst, dead_inst, killing_off, dead_off)
                {
                    // The earlier store takes the merged constant; the killing
                    // store goes.
                    self.merged.insert(dead_inst, value);
                    changed = true;
                    self.delete_dead_instruction(killing_inst, Some(&mut deleted));
                    self.overlaps.remove(&dead_inst);
                    return (changed, true);
                }
            }
            if result == Overwrite::Complete {
                self.delete_dead_instruction(dead_inst, Some(&mut deleted));
                changed = true;
            }
        }
        (changed, false)
    }

    /// `storeIsNoop`: a store of the value just loaded from the same place.
    fn store_is_noop(
        &mut self,
        def: AccessId,
    ) -> bool {
        let def_inst = self.inst_of(def);
        if !self.is_removable(def_inst) {
            return false;
        }
        let Opcode::Store { .. } = self.function.instruction(def_inst).opcode else { return false };
        let operands = &self.function.instruction(def_inst).operands;
        let (Operand::Value(stored), pointer) = (operands[0], operands[1]) else { return false };
        let ValueDef::Instruction(load) = self.function.value(stored).def else { return false };
        let Opcode::Load { volatile: false, .. } = self.function.instruction(load).opcode else { return false };
        if self.function.instruction(load).operands[0] != pointer {
            return false;
        }
        let Some(load_access) = self.mssa.get_memory_access(load) else { return false };
        let load_defining = self.mssa.access(load_access).defining.expect("a defining access");
        // Fast path: the defining accesses are the same.
        if Some(load_defining) == self.mssa.access(def).defining {
            return true;
        }
        // Look through phi accesses.
        let mut to_check: Vec<AccessId> = Vec::new();
        let current = self.mssa.get_clobbering_memory_access(&self.aa, def);
        to_check.push(def);
        to_check.push(current);
        let mut at = 1;
        while at < to_check.len() {
            let current = to_check[at];
            at += 1;
            if self.mssa.access(current).kind == Kind::Phi {
                let incoming: Vec<AccessId> = self.mssa.access(current).incoming.iter().map(|(_, one)| *one).collect();
                for one in incoming {
                    if !to_check.contains(&one) {
                        to_check.push(one);
                    }
                }
                continue;
            }
            // An unrelated write between ends it.
            if load_defining != current {
                return false;
            }
        }
        true
    }

    /// `eliminateRedundantStoresOfExistingValues`: a store of what the
    /// clobbering def before it wrote already (an identical instruction, or
    /// a memset of the byte) is removed.
    fn eliminate_redundant_stores_of_existing_values(&mut self) -> bool {
        let mut changed = false;
        let defs = self.mem_defs.clone();
        for def in defs {
            if self.skip_stores.contains(&def) || self.mssa.is_live_on_entry_def(def) {
                continue;
            }
            let def_inst = self.inst_of(def);
            let Some(def_loc) = self.loc_for_write(def_inst) else { continue };
            if !self.is_removable(def_inst) {
                continue;
            }
            // To conserve compile time the walk to the next clobbering def is
            // not made: the optimized access, if there is one.
            let upper =
                if self.mssa.is_optimized(def) { self.mssa.get_optimized(def) } else { self.mssa.access(def).defining };
            let Some(upper) = upper else { continue };
            if self.mssa.is_live_on_entry_def(upper) || self.mssa.access(upper).kind != Kind::Def {
                continue;
            }
            let upper_inst = self.inst_of(upper);
            let identical = {
                let (a, b) = (self.function.instruction(def_inst), self.function.instruction(upper_inst));
                a.opcode == b.opcode && a.operands == b.operands
            };
            let redundant = identical || self.memset_covers_store(upper_inst, def_inst, &def_loc);
            if !redundant || self.is_read_clobber(&def_loc, def_inst) {
                continue;
            }
            self.delete_dead_instruction(def_inst, None);
            changed = true;
        }
        changed
    }

    /// `upper` is a memset of the byte `store` stores, covering it.
    fn memset_covers_store(
        &self,
        upper: InstId,
        store: InstId,
        store_loc: &Location,
    ) -> bool {
        let Some((_, byte, _)) = self.memset(upper) else { return false };
        if !self.is_store(store) {
            return false;
        }
        let Some(upper_loc) = self.loc_for_write(upper) else { return false };
        let unit = self.aa.unit();
        let value = self.function.instruction(store).operands[0];
        let Some(bits) = unit.int_bits(value) else { return false };
        let Some(constant) = self.merged.get(&store).copied().or_else(|| unit.int_constant(value)) else {
            return false;
        };
        let stored_byte = if bits % 8 == 0 && bits > 0 {
            let first = constant & 0xFF;
            (0..bits / 8).all(|at| (constant >> (8 * at)) & 0xFF == first).then_some(first)
        } else {
            None
        };
        let Some(stored_byte) = stored_byte else { return false };
        unit.int_constant(byte) == Some(stored_byte)
            && self.is_overwrite(upper, store, &upper_loc, store_loc) == Overwrite::Complete
    }

    /// `eliminateDeadWritesAtEndOfFunction`.
    fn eliminate_dead_writes_at_end_of_function(&mut self) -> bool {
        let mut changed = false;
        let defs = self.mem_defs.clone();
        for &def in defs.iter().rev() {
            if self.skip_stores.contains(&def) {
                continue;
            }
            let def_inst = self.inst_of(def);
            let Some(def_loc) = self.loc_for_write(def_inst) else { continue };
            if !self.is_removable(def_inst) {
                continue;
            }
            if !self.invisible_after_return(&def_loc) {
                continue;
            }
            if self.is_write_at_end_of_function(def, &def_loc) {
                self.delete_dead_instruction(def_inst, None);
                changed = true;
            }
        }
        changed
    }

    /// `eliminateDeadStores`.
    fn run(&mut self) -> bool {
        let mut changed = false;
        let mut at = 0;
        while at < self.mem_defs.len() {
            let killing_def = self.mem_defs[at];
            at += 1;
            if self.skip_stores.contains(&killing_def) {
                continue;
            }
            let killing_inst = self.inst_of(killing_def);
            let Some(killing_loc) = self.loc_for_write(killing_inst) else { continue };
            let (did, deleted_killing) = self.eliminate_dead_defs_of(killing_def, Rc::clone(&killing_loc));
            changed |= did;
            if !deleted_killing && self.store_is_noop(killing_def) {
                self.delete_dead_instruction(killing_inst, None);
                changed = true;
            }
        }
        changed |= self.remove_partially_overlapped_stores();
        changed |= self.eliminate_redundant_stores_of_existing_values();
        changed |= self.eliminate_dead_writes_at_end_of_function();
        changed
    }
}

/// The reachable blocks in post-order.
fn post_order_of(function: &Function) -> Vec<BlockId> {
    let Some(entry) = function.entry() else { return Vec::new() };
    let mut seen: HashSet<BlockId> = HashSet::default();
    let mut order = Vec::new();
    // A block's first successor is visited first, as LLVM's `post_order`: the
    // lists are popped from the back.
    let successors_of = |block| -> Vec<BlockId> { function.successors(block).iter().rev().copied().collect() };
    let mut stack: Vec<(BlockId, Vec<BlockId>)> = vec![(entry, successors_of(entry))];
    seen.insert(entry);
    while let Some((block, successors)) = stack.last_mut() {
        if let Some(next) = successors.pop() {
            if seen.insert(next) {
                stack.push((next, successors_of(next)));
            }
        } else {
            order.push(*block);
            stack.pop();
        }
    }
    order
}

#[cfg(test)]
#[path = "deadstore_tests.rs"]
mod tests;
