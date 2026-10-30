//! `LazyValueInfo`: the range, constant or non-null fact a value has at a
//! point or on an edge of the CFG, computed on demand from the branches,
//! `switch`es, `llvm.assume`s, `!range` metadata and `range` attributes
//! around it.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LazyValueInfo.cpp` and
//! `LazyValueInfo.h`, one Rust function per C++ function in file order, each
//! linking its line. A query pushes `(block, value)` pairs on a work stack
//! (`solve`); a pair is solved once and kept in a per-block cache, so a
//! function of N instructions costs O(N) solves (`steps` counts them; the
//! slope test in `lazyvalueinfo_tests.rs` doubles a function and checks the
//! count does not more than double).
//!
//! The analysis holds no reference to the function: every call takes an
//! [`Env`] (the context, the function, the layout, the assumption cache), so a
//! pass may edit the function between queries and tell the analysis through
//! [`LazyValueInfo::forget_value`], [`LazyValueInfo::erase_block`] and
//! [`LazyValueInfo::thread_edge`], as it does with LLVM's.
//!
//! DEVIATE, permanent (MIR or Rust lacks the construct):
//! 1. No vectors: `solveBlockValueInsertElement` (lines 1208-1232), the vector
//!    arms of `getValueFromICmpCondition`, `isNotCrossLaneOperation` in
//!    `getValueAtUse` (line 1929). MIR types vectors but has no lane operation
//!    and nothing produces one (#2201).
//! 3. No `TargetLibraryInfo`: LVI's only use is the wrapper pass dependency.
//! 4. `LVIValueHandle` (a `CallbackVH` that erases a deleted value) has no Rust
//!    equivalent; a pass calls [`LazyValueInfo::forget_value`] where LLVM's
//!    value would die (MIR never reuses an id, so a stale entry is harmless).
//!    `BlockNumberEpoch` is not needed for the same reason.
//! 5. `assumeBundleImpliesNonNull` and operand-bundle assumes (`AssumeVH.Index
//!    != ExprResultIdx`, lines 856-863): MIR has no operand bundles (#2203).
//! 6. The LVI pass wrappers (`LazyValueInfoWrapperPass`, `LazyValueAnalysis`,
//!    `invalidate`, the printer pass) are pass-manager glue. `printLVI` prints
//!    the annotations alone, not the function around them.
//! 7. `getConstant`, `getConstantOnEdge` and `getPredicate*` answer in Rust
//!    values (`Option<LatticeConstant>`, `Option<bool>`) where the C++ returns
//!    a `Constant *`; for an `i1` compare it is `true`, `false` or null in
//!    both.
//! 8. `Function::predecessors` lists a block once where LLVM's `predecessors`
//!    repeats it per edge (a `switch` naming it twice); the merge is
//!    idempotent, only the 500-step throttle could tell.
//!
//! Not a difference: the guard loop (line 868) runs when the module declares
//! `llvm.experimental.guard`, whether or not a call uses it; the result is the
//! same.
//!
//! OWED (MIR can represent it; each has an issue and its exit):
//! 9. `PerPredRanges` (the hidden `-lvi-per-pred-ranges` option, off by
//!    default) and its predecessor maps (lines 63, 143-260, 792-837,
//!    1100-1165): #2341.
//! 10. (closed: `ConstantFoldCompareInstOperands` is `constantfolding.rs`.)
//! 11. LLVM folds a constant expression when it makes one (`ptrtoint (ptr
//!     inttoptr (i32 20 to ptr) to i32)` is `i32 20`); MIR keeps the
//!     expression, which the lattice then holds as a non-integer constant
//!     (#2252).
//! 12. `getUnderlyingObject`, `stripInBoundsOffsets`, `stripPointerCasts`,
//!     `simplifyExtractValueInst` and `isSafeToSpeculativelyExecuteWith-
//!     VariableReplaced` are copied here (the GEP, bitcast, addrspacecast,
//!     single-input phi and `returned` cases; MIR has no `GlobalAlias`): #2342.
//! 13. Intrinsic attributes that LLVM's generated tables give an intrinsic (the
//!     `range` of `llvm.nvvm.read.ptx.sreg.nctaid.x`, the `returned` argument
//!     of `llvm.ssa.copy`) are unknown to MIR, which has the `declare`'s own
//!     attributes; `Call->getRange()` and `getReturnedArgOperand()` answer less
//!     than LLVM's for those. Exit: #2142 (generated attributes) and the
//!     verifier accepting unenumerated `llvm.*` names.
//!
//! # Example
//! After `br i1 (icmp ult i32 %x, 10), label %then, label %else`, `%x` on the
//! edge to `%then` has range `[0, 10)`; in `%then`, `getConstantRange(%x)` is
//! the same.
//!
//! # See also
//! - Uses: [`crate::valuelattice`], [`crate::valuetracking`],
//!   [`crate::constantrange`]

use crate::apint;
use crate::constantrange::{ConstantRange, NO_SIGNED_WRAP, NO_UNSIGNED_WRAP, PreferredRangeType, RangeIntrinsic};
use crate::context::{Constant, ConstantId, ConstantKind, Context, mask};
use crate::datalayout::DataLayout;
use crate::domtree::DomTree;
use crate::facts::Fact;
use crate::hash::{HashMap, HashSet};
use crate::knownbits::KnownBits;
use crate::module::{
    BlockId, Function, GlobalKind, GlobalValue, InstId, MetadataNode, MetadataOperand, Operand, Use, ValueDef,
};
use crate::opcode::{Attribute, BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use crate::types::{Type, TypeId};
use crate::valuelattice::{LatticeConstant, MergeOptions, ValueLatticeElement};
use crate::valuetracking::context::is_valid_assume_for_context;
use crate::valuetracking::guarantee::{is_guaranteed_not_to_be_undef, strip_pointer_casts_same_representation};
use crate::valuetracking::nonzero::{is_known_non_zero, null_pointer_is_defined, param_has_non_null_attr};
use crate::valuetracking::pattern::{
    self, IntrinsicId, add_like, argument_range, binary, call_arguments, call_range, cast, commutative_with,
    intrinsic_call, intrinsic_id, logical_and, logical_or, not,
};
use crate::valuetracking::range::{SelectOperand, SelectPatternFlavor, match_select_pattern};
use crate::valuetracking::{AssumptionCache, MAX_ANALYSIS_RECURSION_DEPTH, Query};

type Lattice = ValueLatticeElement;

/// This is the number of worklist items we will process to try to discover an
/// answer for a given value.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L52>
const MAX_PROCESSED_PER_VALUE: u32 = 500;

/// What a query reads of the program: LLVM's `F`, `AC` and `DL` (and the
/// module the function is in), with the context mutably, for the constants
/// `constantFoldUser` makes.
pub struct Env<'a> {
    pub context: &'a mut Context,
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
    pub function: &'a Function,
    pub layout: &'a DataLayout,
    pub assumptions: &'a AssumptionCache,
}

impl<'a> Env<'a> {
    pub fn new(
        context: &'a mut Context,
        globals: &'a [GlobalValue],
        metadata: &'a [MetadataNode],
        function: &'a Function,
        layout: &'a DataLayout,
        assumptions: &'a AssumptionCache,
    ) -> Self {
        Self { context, globals, metadata, function, layout, assumptions }
    }

    /// A `SimplifyQuery(DL)`: no assumptions, no dominator tree.
    fn query(&self) -> Query<'_> {
        Query {
            context: &*self.context,
            globals: self.globals,
            metadata: self.metadata,
            function: self.function,
            layout: self.layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        }
    }

    /// A `SimplifyQuery` with `AC` (`isGuaranteedNotToBeUndef(V, AC)`).
    fn query_with_assumptions(&self) -> Query<'_> {
        Query { assumptions: Some(self.assumptions), ..self.query() }
    }

    /// `Type::getScalarSizeInBits()` of an operand's type, 0 for a non-integer.
    fn scalar_size_in_bits(
        &self,
        op: Operand,
    ) -> u32 {
        match self.function.operand_type(self.context, op).map(|ty| self.context.types.get(ty)) {
            Some(Type::Int(bits)) => *bits,
            _ => 0,
        }
    }

    fn is_integer(
        &self,
        op: Operand,
    ) -> bool {
        self.scalar_size_in_bits(op) != 0
    }

    fn is_pointer(
        &self,
        op: Operand,
    ) -> bool {
        self.query().is_pointer(op)
    }
}

/// Returns true if this lattice value represents at most one possible value.
/// This is as precise as any lattice value can get while still representing
/// reachable code.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L79>
fn has_single_value(val: &Lattice) -> bool {
    if val.is_constant_range(true) && val.get_constant_range(true).is_single_element() {
        // Integer constants are single element ranges
        return true;
    }
    if val.is_constant() {
        // Non integer constants
        return true;
    }
    false
}

type NonNullPointerSet = HashSet<Operand>;

/// This is all of the cached information for one basic block. It contains
/// the per-value lattice elements, as well as a separate set for
/// overdefined values to reduce memory usage. Additionally pointers
/// dereferenced in the block are cached for nullability queries.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L124>
#[derive(Default)]
struct BlockCacheEntry {
    lattice_elements: HashMap<Operand, Lattice>,
    over_defined: HashSet<Operand>,
    // None indicates that the nonnull pointers for this basic block
    // block have not been computed yet.
    non_null_pointers: Option<NonNullPointerSet>,
}

/// This is the cache kept by LazyValueInfo which
/// maintains information about queries across the clients' queries.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L119>
#[derive(Default)]
struct LazyValueInfoCache {
    /// Cached information per basic block, indexed by block number.
    block_cache: Vec<Option<Box<BlockCacheEntry>>>,
    /// Set of values the cache has an entry of, to erase them on deletion.
    value_handles: HashSet<Operand>,
}

impl LazyValueInfoCache {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L143>
    fn get_block_entry(
        &self,
        bb: BlockId,
    ) -> Option<&BlockCacheEntry> {
        self.block_cache.get(bb.0 as usize).and_then(|entry| entry.as_deref())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L150>
    fn get_or_create_block_entry(
        &mut self,
        bb: BlockId,
        max_block_number: usize,
    ) -> &mut BlockCacheEntry {
        let number = bb.0 as usize;
        if number >= self.block_cache.len() {
            self.block_cache.resize_with(max_block_number.max(number + 1), || None);
        }
        self.block_cache[number].get_or_insert_with(Default::default)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L167>
    fn add_value_handle(
        &mut self,
        val: Operand,
    ) {
        self.value_handles.insert(val);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L177>
    fn insert_result(
        &mut self,
        val: Operand,
        bb: BlockId,
        max_block_number: usize,
        result: &Lattice,
    ) {
        let entry = self.get_or_create_block_entry(bb, max_block_number);

        // Insert over-defined values into their own cache to reduce memory
        // overhead.
        if result.is_overdefined() {
            entry.over_defined.insert(val);
        } else {
            entry.lattice_elements.entry(val).or_insert_with(|| result.clone());
        }

        self.add_value_handle(val);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L213>
    fn get_cached_value_info(
        &self,
        v: Operand,
        bb: BlockId,
    ) -> Option<Lattice> {
        let entry = self.get_block_entry(bb)?;

        if entry.over_defined.contains(&v) {
            return Some(Lattice::get_overdefined());
        }

        entry.lattice_elements.get(&v).cloned()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L229>
    fn is_non_null_at_end_of_block(
        &mut self,
        v: Operand,
        bb: BlockId,
        max_block_number: usize,
        init_fn: impl FnOnce(BlockId) -> NonNullPointerSet,
    ) -> bool {
        let entry = self.get_or_create_block_entry(bb, max_block_number);
        if entry.non_null_pointers.is_none() {
            let set = init_fn(bb);
            let added: Vec<Operand> = set.iter().copied().collect();
            self.get_or_create_block_entry(bb, max_block_number).non_null_pointers = Some(set);
            for v in added {
                self.add_value_handle(v);
            }
        }

        self.get_or_create_block_entry(bb, max_block_number).non_null_pointers.as_ref().is_some_and(|s| s.contains(&v))
    }

    /// clear - Empty the cache.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L243>
    fn clear(&mut self) {
        self.block_cache.clear();
        self.value_handles.clear();
    }

    /// Inform the cache that a given value has been deleted.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L262>
    fn erase_value(
        &mut self,
        v: Operand,
    ) {
        for elem in self.block_cache.iter_mut().flatten() {
            elem.lattice_elements.remove(&v);
            elem.over_defined.remove(&v);
            if let Some(set) = &mut elem.non_null_pointers {
                set.remove(&v);
            }
        }

        self.value_handles.remove(&v);
    }

    /// This is part of the update interface to inform the cache
    /// that a block has been deleted.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L286>
    fn erase_block(
        &mut self,
        bb: BlockId,
    ) {
        if let Some(entry) = self.block_cache.get_mut(bb.0 as usize) {
            *entry = None;
        }
    }

    /// Updates the cache to remove any influence an overdefined value in
    /// OldSucc might have (unless also overdefined in NewSucc).  This just
    /// flushes elements from the cache and does not add any.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L297>
    fn thread_edge_impl(
        &mut self,
        function: &Function,
        old_succ: BlockId,
        new_succ: BlockId,
    ) {
        // When an edge in the graph has been threaded, values that we could not
        // determine a value for before (i.e. were marked overdefined) may be
        // possible to solve now. We do NOT try to proactively update these
        // values. Instead, we clear their entries from the cache, and
        // allow lazy updating to recompute them when needed.

        // The updating process is fairly simple: we need to drop cached info
        // for all values that were marked overdefined in OldSucc, and for those
        // same values in any successor of OldSucc (except NewSucc) in
        // which they were also marked overdefined.
        let mut worklist = vec![old_succ];

        let Some(entry) = self.get_block_entry(old_succ) else { return };
        if entry.over_defined.is_empty() {
            return; // Nothing to process here.
        }
        let vals_to_clear: Vec<Operand> = entry.over_defined.iter().copied().collect();

        // Use a worklist to perform a depth-first search of OldSucc's
        // successors. NOTE: We do not need a visited list since any
        // blocks we have already visited will have had their
        // overdefined markers cleared already, and we thus won't loop
        // to their successors.
        while let Some(to_update) = worklist.pop() {
            // Skip blocks only accessible through NewSucc.
            if to_update == new_succ {
                continue;
            }

            // If a value was marked overdefined in OldSucc, and is here too...
            let Some(Some(worklist_entry)) = self.block_cache.get_mut(to_update.0 as usize) else { continue };
            if worklist_entry.over_defined.is_empty() {
                continue;
            }
            let value_set = &mut worklist_entry.over_defined;

            let mut changed = false;
            for v in &vals_to_clear {
                if !value_set.remove(v) {
                    continue;
                }

                // If we removed anything, then we potentially need to update
                // blocks successors too.
                changed = true;
            }

            if !changed {
                continue;
            }

            worklist.extend(function.successors(to_update).iter().copied());
        }
    }
}

/// The work counters of a [`LazyValueInfoImpl`]: what the slope test reads.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Steps {
    /// Iterations of the `solve` loop.
    pub solve: u64,
    /// `(block, value)` pairs solved and cached.
    pub solved: u64,
}

/// What a cache was asked or told, for [`LazyValueInfo::disagreements`].
#[derive(Clone, Debug)]
enum Event {
    Block(Operand, BlockId, Option<InstId>),
    Edge(Operand, BlockId, BlockId, Option<InstId>),
    Forget(Operand),
    EraseBlock(BlockId),
    ThreadEdge(BlockId, BlockId, BlockId),
    Clear,
}

/// The actual implementation of the lazy analysis and update.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L376>
pub struct LazyValueInfoImpl {
    /// Every query and update made, in order, under `LLRM_CHECK_CAPTURES`:
    /// the audit replays them on the body as it stands.
    log: Option<Vec<Event>>,
    /// Cached results from previous queries
    the_cache: LazyValueInfoCache,
    /// This stack holds the state of the value solver during a query.
    /// It basically emulates the callstack of the naive
    /// recursive value lookup process.
    block_value_stack: Vec<(BlockId, Operand)>,
    /// Keeps track of which block-value pairs are in BlockValueStack.
    block_value_set: HashSet<(BlockId, Operand)>,
    /// Whether the module declares the `llvm.experimental.guard()` intrinsic.
    guard_decl: bool,
    pub steps: Steps,
}

/// Which range of its operands `ConstantRange::intrinsic` reads, if any.
fn range_intrinsic(id: IntrinsicId) -> Option<RangeIntrinsic> {
    Some(match id {
        IntrinsicId::UaddSat => RangeIntrinsic::UAddSat,
        IntrinsicId::UsubSat => RangeIntrinsic::USubSat,
        IntrinsicId::SaddSat => RangeIntrinsic::SAddSat,
        IntrinsicId::SsubSat => RangeIntrinsic::SSubSat,
        IntrinsicId::Umin => RangeIntrinsic::UMin,
        IntrinsicId::Umax => RangeIntrinsic::UMax,
        IntrinsicId::Smin => RangeIntrinsic::SMin,
        IntrinsicId::Smax => RangeIntrinsic::SMax,
        IntrinsicId::Abs => RangeIntrinsic::Abs,
        IntrinsicId::Ctlz => RangeIntrinsic::Ctlz,
        IntrinsicId::Cttz => RangeIntrinsic::Cttz,
        IntrinsicId::Ctpop => RangeIntrinsic::Ctpop,
        _ => return None,
    })
}

/// `ConstantRange` of `!range` metadata of an integer of `width` bits.
pub fn constant_range_from_metadata(
    context: &Context,
    node: &MetadataNode,
    width: u32,
) -> Option<ConstantRange> {
    let bound = |at: usize| match node.operands.get(at) {
        Some(MetadataOperand::Constant(id)) => match context.get(*id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    let pairs: Vec<(u128, u128)> =
        (0..node.operands.len() / 2).filter_map(|one| Some((bound(2 * one)?, bound(2 * one + 1)?))).collect();
    if pairs.is_empty() {
        return None;
    }
    Some(ConstantRange::from_range_pairs(width, &pairs))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L605>
fn get_from_range_metadata(
    env: &Env,
    bbi: InstId,
) -> Lattice {
    let instruction = env.function.instruction(bbi);
    let q = env.query();
    let from_call = matches!(instruction.opcode, Opcode::Call(_) | Opcode::Invoke(_));
    if from_call && let Some(range) = call_range(&q, instruction) {
        return Lattice::get_range(range, false);
    }
    if from_call || matches!(instruction.opcode, Opcode::Load { .. }) {
        if let Some(ranges) = q.metadata_node(bbi, "range")
            && let Some(Type::Int(width)) = Some(env.context.types.get(instruction.ty))
            && let Some(range) = constant_range_from_metadata(env.context, ranges, *width)
        {
            return Lattice::get_range(range, false);
        }
    }
    // Nothing known - will be intersected with other facts
    Lattice::get_overdefined()
}

/// The instruction that defines `v`, with the lifetime of the function.
fn def_of<'f>(
    function: &'f Function,
    v: Operand,
) -> Option<(InstId, &'f crate::module::Instruction)> {
    let Operand::Value(value) = v else { return None };
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    Some((inst, function.instruction(inst)))
}

fn value_of(inst: &crate::module::Instruction) -> Operand {
    Operand::Value(inst.result.expect("an instruction with a result"))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L689>
fn add_non_null_pointer(
    env: &Env,
    ptr: Operand,
    ptr_set: &mut NonNullPointerSet,
    is_dereferenced: bool,
) {
    // TODO: Use NullPointerIsDefined instead.
    let space = match env.function.operand_type(env.context, ptr).map(|ty| env.context.types.get(ty)) {
        Some(Type::Pointer(space)) => *space,
        _ => return,
    };
    if space == 0 {
        ptr_set.insert(if is_dereferenced {
            get_underlying_object(env, ptr)
        } else {
            strip_in_bounds_offsets(env, ptr)
        });
    }
}

/// The `MemIntrinsic` a call is: `memcpy`, `memmove`, `memset` and their
/// `.inline` forms (not the `element.unordered.atomic` ones), and whether it is
/// a `MemTransferInst`.
fn mem_intrinsic(
    env: &Env,
    inst: InstId,
) -> Option<bool> {
    let name = env.query().callee_name(inst)?.strip_prefix("llvm.")?;
    let mut parts = name.split('.');
    let transfer = match parts.next()? {
        "memcpy" | "memmove" => true,
        "memset" => false,
        _ => return None,
    };
    (parts.next() != Some("element")).then_some(transfer)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L697>
fn add_non_null_pointers_by_instruction(
    env: &Env,
    inst: InstId,
    ptr_set: &mut NonNullPointerSet,
) {
    let instruction = env.function.instruction(inst);
    match &instruction.opcode {
        Opcode::Load { .. } => add_non_null_pointer(env, instruction.operands[0], ptr_set, true),
        Opcode::Store { .. } => add_non_null_pointer(env, instruction.operands[1], ptr_set, true),
        Opcode::Call(_) | Opcode::Invoke(_) => {
            if let Some(transfer) = mem_intrinsic(env, inst) {
                let arguments = call_arguments(instruction);
                if arguments.get(3).and_then(|v| env.query().int_constant(*v)).is_some_and(|(bits, _)| bits != 0) {
                    return;
                }

                // FIXME: check whether it has a valuerange that excludes zero?
                let length = arguments.get(2).and_then(|v| env.query().int_constant(*v));
                if length.is_none_or(|(bits, _)| bits == 0) {
                    return;
                }

                add_non_null_pointer(env, arguments[0], ptr_set, true);
                if transfer {
                    add_non_null_pointer(env, arguments[1], ptr_set, true);
                }
            } else {
                let q = env.query();
                for (arg_no, &arg) in call_arguments(instruction).iter().enumerate() {
                    if q.is_pointer(arg) && param_has_non_null_attr(&q, instruction, arg_no, false) {
                        add_non_null_pointer(env, arg, ptr_set, false);
                    }
                }
            }
        }
        _ => {}
    }
}

/// `Value::stripInBoundsOffsets()`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Value.cpp#L642>
fn strip_in_bounds_offsets(
    env: &Env,
    ptr: Operand,
) -> Operand {
    let q = env.query();
    if !q.is_pointer(ptr) {
        return ptr;
    }

    // Even though we don't look through PHI nodes, we could be called on an
    // instruction in an unreachable block, which may be on a cycle.
    let mut visited = HashSet::default();
    let mut v = ptr;
    visited.insert(v);
    loop {
        let next = match q.instruction_of(v) {
            Some((inst, instruction)) => match &instruction.opcode {
                Opcode::GetElementPtr { .. } => {
                    if !instruction.flags.contains(Flags::INBOUNDS) {
                        return v;
                    }
                    instruction.operands[0]
                }
                Opcode::Cast(CastOp::BitCast) => {
                    let new_v = instruction.operands[0];
                    if !q.is_pointer(new_v) {
                        return v;
                    }
                    new_v
                }
                Opcode::Cast(CastOp::AddrSpaceCast) => instruction.operands[0],
                Opcode::Call(_) | Opcode::Invoke(_) => match pattern::returned_argument_operand(&q, instruction) {
                    Some(returned) => returned,
                    None => {
                        let _ = inst;
                        return v;
                    }
                },
                _ => return v,
            },
            None => return v,
        };
        v = next;
        if !visited.insert(v) {
            return v;
        }
    }
}

/// `getUnderlyingObject(V)`, `MaxLookup` 6.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7289>
fn get_underlying_object(
    env: &Env,
    ptr: Operand,
) -> Operand {
    let q = env.query();
    let mut v = ptr;
    for _ in 0..MAX_LOOKUP {
        let Some((inst, instruction)) = q.instruction_of(v) else { return v };
        match &instruction.opcode {
            Opcode::GetElementPtr { .. } => {
                let ptr_op = instruction.operands[0];
                if !q.is_pointer(ptr_op) {
                    // Only handle scalar pointer base.
                    return v;
                }
                v = ptr_op;
            }
            Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => {
                let new_v = instruction.operands[0];
                if !q.is_pointer(new_v) {
                    return v;
                }
                v = new_v;
            }
            Opcode::Phi if instruction.operands.len() == 2 => {
                // Look through single-arg phi nodes created by LCSSA.
                v = instruction.operands[0];
            }
            Opcode::Call(_) | Opcode::Invoke(_) => {
                match get_argument_aliasing_to_returned_pointer(env, inst, instruction) {
                    Some(returned) => v = returned,
                    None => return v,
                }
            }
            _ => return v,
        }
    }
    v
}

/// `MaxLookupSearchDepth` of `getUnderlyingObject`.
const MAX_LOOKUP: u32 = 6;

/// `getArgumentAliasingToReturnedPointer(Call, MustPreserveOffset=false,
/// MustPreserveProvenance=false)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7219>
fn get_argument_aliasing_to_returned_pointer(
    env: &Env,
    inst: InstId,
    instruction: &crate::module::Instruction,
) -> Option<Operand> {
    let q = env.query();
    if let Some(returned) = pattern::returned_argument_operand(&q, instruction) {
        return Some(returned);
    }
    // This can be used only as a aliasing property.
    match intrinsic_id(&q, inst) {
        Some(
            IntrinsicId::LaunderInvariantGroup
            | IntrinsicId::Aarch64Irg
            | IntrinsicId::Aarch64Tagp
            | IntrinsicId::AmdgcnMakeBufferRsrc
            | IntrinsicId::Ptrmask
            | IntrinsicId::ThreadlocalAddress,
        ) => call_arguments(instruction).first().copied(),
        _ => None,
    }
}

impl LazyValueInfoImpl {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L495>
    pub fn new(env: &Env) -> Self {
        let guard_decl = env
            .globals
            .iter()
            .any(|g| matches!(g.kind, GlobalKind::Function(_)) && g.name.as_deref() == Some("llvm.experimental.guard"));
        Self {
            log: crate::passes::check_captures().then(Vec::new),
            the_cache: LazyValueInfoCache::default(),
            block_value_stack: Vec::new(),
            block_value_set: HashSet::default(),
            guard_decl,
            steps: Steps::default(),
        }
    }

    fn max_block_number(env: &Env) -> usize {
        env.function.block_capacity()
    }

    /// Push BV onto BlockValueStack unless it's already in there.
    /// Returns true on success.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L391>
    fn push_block_value(
        &mut self,
        bv: (BlockId, Operand),
    ) -> bool {
        if !self.block_value_set.insert(bv) {
            return false; // It's already in the stack.
        }

        self.block_value_stack.push(bv);
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L527>
    fn solve(
        &mut self,
        env: &mut Env,
    ) {
        let mut starting_stack = self.block_value_stack.clone();

        let mut processed_count = 0;
        while let Some(&e) = self.block_value_stack.last() {
            processed_count += 1;
            self.steps.solve += 1;
            // Abort if we have to process too many values to get a result for
            // this one. Because of the design of the overdefined
            // cache currently being per-block
            // to avoid naming-related issues (IE it wants to try to give
            // different results for the same name in different
            // blocks), overdefined results don't get cached
            // globally, which in turn means we will often try to rediscover
            // the same overdefined result again and again.  Once something like
            // PredicateInfo is used in LVI or CVP, we should be able to make
            // the overdefined cache global, and remove this
            // throttle.
            if processed_count > MAX_PROCESSED_PER_VALUE {
                // Fill in the original values
                let max = Self::max_block_number(env);
                while let Some(e) = starting_stack.pop() {
                    self.the_cache.insert_result(e.1, e.0, max, &Lattice::get_overdefined());
                }
                self.block_value_set.clear();
                self.block_value_stack.clear();
                return;
            }
            debug_assert!(self.block_value_set.contains(&e), "Stack value should be in BlockValueSet!");

            if self.solve_block_value(env, e.1, e.0) {
                // The work item was completely processed.
                self.block_value_stack.pop();
                self.block_value_set.remove(&e);
            }
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L585>
    fn get_block_value(
        &mut self,
        env: &mut Env,
        val: Operand,
        bb: BlockId,
        cxt_i: Option<InstId>,
    ) -> Option<Lattice> {
        // If already a constant, there is nothing to compute.
        if let Operand::Constant(vc) = val {
            return Some(Lattice::get(&LatticeConstant::of(env.context, vc)));
        }

        if let Some(mut opt_lattice_val) = self.the_cache.get_cached_value_info(val, bb) {
            self.intersect_assume_or_guard_block_value_constant_range(env, val, &mut opt_lattice_val, cxt_i);
            return Some(opt_lattice_val);
        }

        // We have hit a cycle, assume overdefined.
        if !self.push_block_value((bb, val)) {
            return Some(Lattice::get_overdefined());
        }

        // Yet to be resolved.
        None
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L626>
    fn solve_block_value(
        &mut self,
        env: &mut Env,
        val: Operand,
        bb: BlockId,
    ) -> bool {
        debug_assert!(!matches!(val, Operand::Constant(_)), "Value should not be constant");
        debug_assert!(self.the_cache.get_cached_value_info(val, bb).is_none(), "Value should not be in cache");

        // Hold off inserting this value into the Cache in case we have to
        // return false and come back later.
        let Some(res) = self.solve_block_value_impl(env, val, bb) else {
            // Work pushed, will revisit
            return false;
        };

        self.steps.solved += 1;
        let max = Self::max_block_number(env);
        self.the_cache.insert_result(val, bb, max, &res);
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L643>
    fn solve_block_value_impl(
        &mut self,
        env: &mut Env,
        val: Operand,
        bb: BlockId,
    ) -> Option<Lattice> {
        let function = env.function;
        let bbi = def_of(env.function, val).filter(|(inst, _)| function.parent(*inst) == Some(bb));
        let Some((bbi, instruction)) = bbi else {
            return self.solve_block_value_non_local(env, val, bb);
        };

        if instruction.opcode == Opcode::Phi {
            return self.solve_block_value_phi_node(env, bbi, bb);
        }

        if instruction.opcode == Opcode::Select {
            return self.solve_block_value_select(env, bbi, bb);
        }

        // If this value is a nonnull pointer, record it's range and bailout.
        // Note that for all other pointer typed values, we terminate
        // the search at the definition.  We could easily extend this to
        // look through geps, bitcasts, and the like to prove
        // non-nullness, but it's not clear that's worth it compile time
        // wise.  The context-insensitive value walk done inside
        // isKnownNonZero gets most of the profitable cases at much less
        // expense. This does mean that we have a sensitivity to where
        // the defining instruction is placed, even if it could legally
        // be hoisted much higher. That is unfortunate.
        if matches!(env.context.types.get(instruction.ty), Type::Pointer(_)) {
            let bbi_value = value_of(instruction);
            if is_known_non_zero(&env.query(), bbi_value, 0) {
                return Some(Lattice::get_not(&LatticeConstant::null_value(env.context, instruction.ty)));
            }
        }

        if matches!(env.context.types.get(instruction.ty), Type::Int(_)) {
            match &instruction.opcode {
                Opcode::Cast(_) => return self.solve_block_value_cast(env, bbi, bb),
                Opcode::Binary(_) => return self.solve_block_value_binary_op(env, bbi, bb),
                Opcode::ExtractValue(_) => return self.solve_block_value_extract_value(env, bbi, bb),
                Opcode::Call(_) if pattern::call_intrinsic(&env.query(), bbi).is_some() => {
                    return self.solve_block_value_intrinsic(env, bbi, bb);
                }
                _ => {}
            }
        }

        Some(get_from_range_metadata(env, bbi))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L723>
    fn is_non_null_at_end_of_block(
        &mut self,
        env: &Env,
        val: Operand,
        bb: BlockId,
    ) -> bool {
        let space = match env.function.operand_type(env.context, val).map(|ty| env.context.types.get(ty)) {
            Some(Type::Pointer(space)) => *space,
            _ => 0,
        };
        if null_pointer_is_defined(Some(env.function), space) {
            return false;
        }

        let val = strip_in_bounds_offsets(env, val);
        let max = Self::max_block_number(env);
        self.the_cache.is_non_null_at_end_of_block(val, bb, max, |bb| {
            let mut non_null_pointers = NonNullPointerSet::default();
            for i in env.function.iter_block(bb) {
                add_non_null_pointers_by_instruction(env, i, &mut non_null_pointers);
            }
            non_null_pointers
        })
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L738>
    fn solve_block_value_non_local(
        &mut self,
        env: &mut Env,
        val: Operand,
        bb: BlockId,
    ) -> Option<Lattice> {
        let mut result = Lattice::default(); // Start Undefined.

        // If this is the entry block, we must be asking about an argument.
        if env.function.entry() == Some(bb) {
            if let Operand::Value(v) = val
                && let ValueDef::Argument(index) = env.function.value(v).def
                && let Some(range) = argument_range(&env.query(), index)
            {
                return Some(Lattice::get_range(range, false));
            }
            return Some(Lattice::get_overdefined());
        }

        // Loop over all of our predecessors, merging what we know from them
        // into result.  If we encounter an unexplored predecessor, we
        // eagerly explore it in a depth first manner.  In practice,
        // this has the effect of discovering paths we can't analyze
        // eagerly without spending compile times analyzing other paths.
        // This heuristic benefits from the fact that predecessors are
        // frequently arranged such that dominating ones come first and we
        // quickly find a path to function entry.  TODO: We should
        // consider explicitly canonicalizing to make this true rather
        // than relying on this happy accident.
        // The use list in place, as the pin walks it: a block named twice by
        // one terminator is visited twice, and merges to the same value.
        let function = env.function;
        for one in function.block_users(bb) {
            if !function.instruction(one.user).opcode.is_terminator() {
                continue;
            }
            let Some(pred) = function.parent(one.user) else { continue };
            // Skip self loops.
            if pred == bb {
                continue;
            }
            let edge_result = self.get_edge_value(env, val, pred, bb, None)?;

            result.merge_in(&edge_result, MergeOptions::default());

            // If we hit overdefined, exit early.  The BlockVals entry is
            // already set to overdefined.
            if result.is_overdefined() {
                return Some(result);
            }
        }

        // Return the merged value, which is more precise than 'overdefined'.
        debug_assert!(!result.is_overdefined());
        Some(result)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L793>
    fn solve_block_value_phi_node(
        &mut self,
        env: &mut Env,
        pn: InstId,
        bb: BlockId,
    ) -> Option<Lattice> {
        let mut result = Lattice::default(); // Start Undefined.
        let function = env.function;
        let phi = function.instruction(pn);

        // Loop over all of our predecessors, merging what we know from them
        // into result.  See the comment about the chosen traversal
        // order in solveBlockValueNonLocal; the same reasoning applies
        // here.
        for (phi_val, phi_bb) in pattern::phi_incoming(phi) {
            // Note that we can provide PN as the context value to getEdgeValue,
            // even though the results will be cached, because PN is
            // the value being used as the cache key in the caller.
            let edge_result = self.get_edge_value(env, phi_val, phi_bb, bb, Some(pn))?;

            result.merge_in(&edge_result, MergeOptions::default());

            // If we hit overdefined, exit early.  The BlockVals entry is
            // already set to overdefined.
            if result.is_overdefined() {
                return Some(result);
            }
        }

        // Return the merged value, which is more precise than 'overdefined'.
        debug_assert!(!result.is_overdefined(), "Possible PHI in entry block?");
        Some(result)
    }

    /// If we can determine a constraint on the value given conditions assumed
    /// by the program, intersect those constraints with BBLV
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L839>
    fn intersect_assume_or_guard_block_value_constant_range(
        &mut self,
        env: &mut Env,
        val: Operand,
        bblv: &mut Lattice,
        bbi: Option<InstId>,
    ) {
        let function = env.function;
        let Some(bbi) = bbi.or_else(|| def_of(env.function, val).map(|(inst, _)| inst)) else { return };

        let Some(bb) = function.parent(bbi) else { return };
        for &assume in env.assumptions.assumptions_for(val) {
            // Only check assumes in the block of the context instruction. Other
            // assumes will have already been taken into account when the value
            // was propagated from predecessor blocks.
            if function.parent(assume) != Some(bb) || !is_valid_assume_for_context(&env.query(), assume, bbi, false) {
                continue;
            }

            let condition = function.instruction(assume).operands[0];
            *bblv = bblv.intersect(
                &self
                    .get_value_from_condition(env, val, condition, true, false, 0)
                    .expect("no block value is asked for"),
            );
        }

        // If guards are not used in the module, don't spend time looking for
        // them
        if self.guard_decl && function.block(bb).first() != Some(bbi) {
            for i in std::iter::successors(function.prev_in_block(bbi), |&one| function.prev_in_block(one)) {
                let q = env.query();
                if intrinsic_id(&q, i) == Some(IntrinsicId::ExperimentalGuard)
                    && matches!(function.instruction(i).opcode, Opcode::Call(_))
                {
                    let cond = function.instruction(i).operands[0];
                    *bblv = bblv.intersect(
                        &self
                            .get_value_from_condition(env, val, cond, true, false, 0)
                            .expect("no block value is asked for"),
                    );
                }
            }
        }

        if bblv.is_overdefined() {
            // Check whether we're checking at the terminator, and the pointer
            // has been dereferenced in this block.
            let pointer_ty = match env.function.operand_type(env.context, val) {
                Some(ty) if matches!(env.context.types.get(ty), Type::Pointer(_)) => Some(ty),
                _ => None,
            };
            if let Some(pty) = pointer_ty
                && function.terminator(bb) == Some(bbi)
                && self.is_non_null_at_end_of_block(env, val, bb)
            {
                *bblv = Lattice::get_not(&LatticeConstant::null_value(env.context, pty));
            }
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L894>
    fn solve_block_value_select(
        &mut self,
        env: &mut Env,
        si: InstId,
        bb: BlockId,
    ) -> Option<Lattice> {
        let function = env.function;
        let select = function.instruction(si);
        let (cond, true_value, false_value) = (select.operands[0], select.operands[1], select.operands[2]);

        // Recurse on our inputs if needed
        let mut true_val = self.get_block_value(env, true_value, bb, Some(si))?;
        let mut false_val = self.get_block_value(env, false_value, bb, Some(si))?;

        if true_val.is_constant_range(true) || false_val.is_constant_range(true) {
            let width = env.scalar_size_in_bits(value_of(select));
            let true_cr = true_val.as_constant_range_of_type(width, false);
            let false_cr = false_val.as_constant_range_of_type(width, false);
            let spr = match_select_pattern(&env.query(), value_of(select));
            let is_same = |side: Option<SelectOperand>, want: Operand| match side {
                Some(SelectOperand::Value(op)) => op == want,
                Some(SelectOperand::Int { ty, bits }) => {
                    let q = env.query();
                    q.type_of(want) == Some(ty) && q.int_constant(want).is_some_and(|(b, _)| b == bits)
                }
                _ => false,
            };
            let (lhs, rhs) = (spr.lhs, spr.rhs);
            // Is this a min specifically of our two inputs?  (Avoid the risk of
            // ValueTracking getting smarter looking back past our immediate
            // inputs.)
            if spr.flavor.is_min_or_max()
                && ((is_same(lhs, true_value) && is_same(rhs, false_value))
                    || (is_same(rhs, true_value) && is_same(lhs, false_value)))
            {
                let result_cr = match spr.flavor {
                    SelectPatternFlavor::SMin => true_cr.smin(&false_cr),
                    SelectPatternFlavor::UMin => true_cr.umin(&false_cr),
                    SelectPatternFlavor::SMax => true_cr.smax(&false_cr),
                    SelectPatternFlavor::UMax => true_cr.umax(&false_cr),
                    _ => unreachable!("unexpected minmax type!"),
                };
                return Some(Lattice::get_range(
                    result_cr,
                    true_val.is_constant_range_including_undef() || false_val.is_constant_range_including_undef(),
                ));
            }

            if spr.flavor == SelectPatternFlavor::Abs {
                if is_same(lhs, true_value) {
                    return Some(Lattice::get_range(true_cr.abs(false), true_val.is_constant_range_including_undef()));
                }
                if is_same(lhs, false_value) {
                    return Some(Lattice::get_range(
                        false_cr.abs(false),
                        false_val.is_constant_range_including_undef(),
                    ));
                }
            }

            if spr.flavor == SelectPatternFlavor::NAbs {
                let zero = ConstantRange::single(true_cr.width, 0);
                if is_same(lhs, true_value) {
                    return Some(Lattice::get_range(
                        zero.sub(&true_cr.abs(false)),
                        false_val.is_constant_range_including_undef(),
                    ));
                }
                if is_same(lhs, false_value) {
                    return Some(Lattice::get_range(
                        zero.sub(&false_cr.abs(false)),
                        false_val.is_constant_range_including_undef(),
                    ));
                }
            }
        }

        // Can we constrain the facts about the true and false values by using
        // the condition itself?  This shows up with idioms like e.g.
        // select(a > 5, a, 5). TODO: We could potentially refine an
        // overdefined true value above. If the value is undef, a
        // different value may be chosen in the select condition.
        if is_guaranteed_not_to_be_undef(&env.query_with_assumptions(), cond, 0) {
            let from_true = self
                .get_value_from_condition(env, true_value, cond, true, false, 0)
                .expect("no block value is asked for");
            true_val = true_val.intersect(&from_true);
            let from_false = self
                .get_value_from_condition(env, false_value, cond, false, false, 0)
                .expect("no block value is asked for");
            false_val = false_val.intersect(&from_false);
        }

        true_val.merge_in(&false_val, MergeOptions::default());
        Some(true_val)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L980>
    fn get_range_for(
        &mut self,
        env: &mut Env,
        v: Operand,
        cxt_i: InstId,
        bb: BlockId,
    ) -> Option<ConstantRange> {
        let opt_val = self.get_block_value(env, v, bb, Some(cxt_i))?;
        Some(opt_val.as_constant_range_of_type(env.scalar_size_in_bits(v), false))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L988>
    fn solve_block_value_cast(
        &mut self,
        env: &mut Env,
        ci: InstId,
        bb: BlockId,
    ) -> Option<Lattice> {
        let instruction = env.function.instruction(ci);
        let Opcode::Cast(op) = instruction.opcode else { unreachable!("a cast") };
        // Filter out casts we don't know how to reason about before attempting
        // to recurse on our operand.  This can cut a long search short
        // if we know we're not going to be able to get any useful
        // information anways.
        match op {
            CastOp::Trunc | CastOp::SExt | CastOp::ZExt => {}
            _ => {
                // Unhandled instructions are overdefined.
                return Some(Lattice::get_overdefined());
            }
        }

        // Figure out the range of the LHS.  If that fails, we still apply the
        // transfer rule on the full set since we may be able to locally infer
        // interesting facts.
        // More work to do before applying this transfer rule.
        let lhs_range = self.get_range_for(env, instruction.operands[0], ci, bb)?;

        let result_bit_width = env.scalar_size_in_bits(value_of(instruction));

        // NOTE: We're currently limited by the set of operations that
        // ConstantRange can evaluate symbolically.  Enhancing that set
        // will allows us to analyze more definitions.
        let res = if op == CastOp::Trunc {
            lhs_range.truncate(result_bit_width, no_wrap_kind(instruction.flags))
        } else {
            lhs_range.cast_op(op, result_bit_width)
        };

        Some(Lattice::get_range(res, false))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1028>
    fn solve_block_value_binary_op_impl(
        &mut self,
        env: &mut Env,
        i: InstId,
        bb: BlockId,
        op_fn: &dyn Fn(&ConstantRange, &ConstantRange) -> ConstantRange,
    ) -> Option<Lattice> {
        let function = env.function;
        let operands = &function.instruction(i).operands;
        let (lhs, rhs) = (operands[0], operands[1]);

        // Figure out the ranges of the operands.  If that fails, use a
        // conservative range, but apply the transfer rule anyways.  This
        // lets us pick up facts from expressions like "and i32 (call i32
        // @foo()), 32"
        let lhs_res = self.get_range_for(env, lhs, i, bb)?;

        // Try to thread binop over rhs select
        if let Some((si, _)) = def_of(env.function, rhs).filter(|(_, x)| x.opcode == Opcode::Select)
            && let Some(res) = self.thread_bin_op_over_select(env, lhs, &lhs_res, si, true, op_fn)
        {
            return Some(res);
        }

        let rhs_res = self.get_range_for(env, rhs, i, bb)?;

        // Try to thread binop over lhs select
        if let Some((si, _)) = def_of(env.function, lhs).filter(|(_, x)| x.opcode == Opcode::Select)
            && let Some(res) = self.thread_bin_op_over_select(env, rhs, &rhs_res, si, false, op_fn)
        {
            return Some(res);
        }

        Some(Lattice::get_range(op_fn(&lhs_res, &rhs_res), false))
    }

    /// `ThreadBinOpOverSelect`, the lambda of `solveBlockValueBinaryOpImpl`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1036>
    fn thread_bin_op_over_select(
        &mut self,
        env: &mut Env,
        x: Operand,
        crx: &ConstantRange,
        y: InstId,
        x_is_lhs: bool,
        op_fn: &dyn Fn(&ConstantRange, &ConstantRange) -> ConstantRange,
    ) -> Option<Lattice> {
        let select = env.function.instruction(y);
        let cond = select.operands[0];
        // Only handle selects with constant values.
        let Operand::Constant(true_c) = select.operands[1] else { return None };
        let Operand::Constant(false_c) = select.operands[2] else { return None };
        if !is_guaranteed_not_to_be_undef(&env.query_with_assumptions(), cond, 0) {
            return None;
        }

        let width = env.scalar_size_in_bits(x);
        let range_of = |lat: Lattice| lat.as_constant_range_of_type(width, false);
        let on_true = self.get_value_from_condition(env, x, cond, true, false, 0).expect("no block value is asked for");
        let true_x = crx.intersect_with(&range_of(on_true), PreferredRangeType::Smallest);
        let on_false =
            self.get_value_from_condition(env, x, cond, false, false, 0).expect("no block value is asked for");
        let false_x = crx.intersect_with(&range_of(on_false), PreferredRangeType::Smallest);
        let true_y = constant_to_range(env, true_c);
        let false_y = constant_to_range(env, false_c);

        if x_is_lhs {
            return Some(Lattice::get_range(
                op_fn(&true_x, &true_y).union_with(&op_fn(&false_x, &false_y), PreferredRangeType::Smallest),
                false,
            ));
        }
        Some(Lattice::get_range(
            op_fn(&true_y, &true_x).union_with(&op_fn(&false_y, &false_x), PreferredRangeType::Smallest),
            false,
        ))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1167>
    fn solve_block_value_binary_op(
        &mut self,
        env: &mut Env,
        bo: InstId,
        bb: BlockId,
    ) -> Option<Lattice> {
        let instruction = env.function.instruction(bo);
        let Opcode::Binary(op) = instruction.opcode else { unreachable!("a binary operator") };
        let flags = instruction.flags;

        self.solve_block_value_binary_op_impl(env, bo, bb, &move |cr1, cr2| {
            cr1.binary_op_with_flags(
                op,
                cr2,
                flags.contains(Flags::NUW),
                flags.contains(Flags::NSW),
                flags.contains(Flags::DISJOINT),
            )
        })
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1178>
    fn solve_block_value_overflow_intrinsic(
        &mut self,
        env: &mut Env,
        wo: InstId,
        bb: BlockId,
    ) -> Option<Lattice> {
        let Some(crate::intrinsics::Intrinsic::WithOverflow { op, .. }) = env.query().intrinsic(wo) else {
            unreachable!("a with.overflow intrinsic")
        };
        self.solve_block_value_binary_op_impl(env, wo, bb, &move |cr1, cr2| cr1.binary_op(op, cr2))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1187>
    fn solve_block_value_intrinsic(
        &mut self,
        env: &mut Env,
        ii: InstId,
        bb: BlockId,
    ) -> Option<Lattice> {
        let metadata_val = get_from_range_metadata(env, ii);
        let id = intrinsic_id(&env.query(), ii);
        let Some(range_intrinsic) = id.and_then(range_intrinsic) else {
            return Some(metadata_val);
        };

        let function = env.function;
        let mut op_ranges = Vec::new();
        for &op in call_arguments(function.instruction(ii)) {
            let range = self.get_range_for(env, op, ii, bb)?;
            op_ranges.push(range);
        }

        Some(Lattice::get_range(ConstantRange::intrinsic(range_intrinsic, &op_ranges), false).intersect(&metadata_val))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1234>
    fn solve_block_value_extract_value(
        &mut self,
        env: &mut Env,
        evi: InstId,
        bb: BlockId,
    ) -> Option<Lattice> {
        let instruction = env.function.instruction(evi);
        let Opcode::ExtractValue(indices) = &instruction.opcode else { unreachable!("an extractvalue") };
        let aggregate = instruction.operands[0];
        if let Some((wo, _)) = pattern::with_overflow_inst(&env.query(), aggregate)
            && indices.len() == 1
            && indices[0] == 0
        {
            return self.solve_block_value_overflow_intrinsic(env, wo, bb);
        }

        // Handle extractvalue of insertvalue to allow further simplification
        // based on replaced with.overflow intrinsics.
        if let Some(v) = simplify_extract_value_inst(env, aggregate, indices) {
            return self.get_block_value(env, v, bb, Some(evi));
        }

        Some(Lattice::get_overdefined())
    }

    /// Counts one pattern of an `icmp` operand tried; true, to sit in a
    /// condition.
    fn pattern_tried() -> bool {
        crate::counts::bump(crate::counts::Event::LviIcmpPatterns);
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1252>
    fn match_icmp_operand(
        env: &Env,
        offset: &mut u128,
        lhs: Operand,
        val: Operand,
        pred: IntPredicate,
        val_add: Option<(Operand, Operand)>,
    ) -> bool {
        if lhs == val {
            return true;
        }
        let q = env.query();

        // Every pattern of `lhs` below is an add, an or or an and: the kind of
        // its defining instruction says which can match, and a load, a phi,
        // a constant or an argument matches none.
        let (is_add, is_or, is_and) = q
            .instruction_of(lhs)
            .map_or(
                (false, false, false),
                |(_, instruction)| {
                    let or = instruction.opcode == Opcode::Binary(BinaryOp::Or);
                    (
                        or || instruction.opcode == Opcode::Binary(BinaryOp::Add),
                        or,
                        instruction.opcode == Opcode::Binary(BinaryOp::And),
                    )
                },
            );

        // Handle range checking idiom produced by InstCombine. We will subtract
        // the offset from the allowed range for RHS in this case.
        if is_add
            && Self::pattern_tried()
            && let Some((a, b)) = add_like(&q, lhs, Flags::default())
            && a == val
            && let Some((c, _)) = q.int_constant(b)
        {
            *offset = c;
            return true;
        }

        // Handle the symmetric case. This appears in saturation patterns like
        // (x == 16) ? 16 : (x + 1). `val_add` is `val` as an add, found once
        // for the condition.
        if let Some((a, b)) = val_add
            && a == lhs
            && let Some((c, width)) = q.int_constant(b)
        {
            *offset = c.wrapping_neg() & mask(width);
            return true;
        }

        // If (x | y) < C, then (x < C) && (y < C).
        if is_or
            && Self::pattern_tried()
            && commutative_with(&q, lhs, BinaryOp::Or, |x| x == val).is_some()
            && (pred == IntPredicate::Ult || pred == IntPredicate::Ule)
        {
            return true;
        }

        // If (x & y) > C, then (x > C) && (y > C).
        if is_and
            && Self::pattern_tried()
            && commutative_with(&q, lhs, BinaryOp::And, |x| x == val).is_some()
            && (pred == IntPredicate::Ugt || pred == IntPredicate::Uge)
        {
            return true;
        }

        false
    }

    /// Get value range for a "(Val + Offset) Pred RHS" condition.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1286>
    fn get_value_from_simple_icmp_condition(
        &mut self,
        env: &mut Env,
        pred: IntPredicate,
        rhs: Operand,
        offset: u128,
        cxt_i: InstId,
        use_block_value: bool,
    ) -> Option<Lattice> {
        let width = env.scalar_size_in_bits(rhs);
        let mut rhs_range = ConstantRange::full(width);
        if let Operand::Constant(c) = rhs {
            rhs_range = constant_to_range(env, c);
        } else if use_block_value {
            let bb = env.function.parent(cxt_i).expect("a placed instruction");
            let r = self.get_block_value(env, rhs, bb, Some(cxt_i))?;
            rhs_range = r.as_constant_range_of_type(width, false);
        }

        let true_values = ConstantRange::make_allowed_icmp_region(pred, &rhs_range);
        Some(Lattice::get_range(true_values.subtract(offset), false))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1309>
    fn get_range_via_slt(
        mut pred: IntPredicate,
        mut rhs: u128,
        width: u32,
        fn_: &dyn Fn(u128) -> Option<ConstantRange>,
    ) -> Option<ConstantRange> {
        let mut invert = false;
        if pred == IntPredicate::Sgt || pred == IntPredicate::Sge {
            pred = pred.inverse();
            invert = true;
        }
        if pred == IntPredicate::Sle {
            pred = IntPredicate::Slt;
            if rhs == mask(width) >> 1 {
                return None; // Could also return full/empty here, if we wanted.
            }
            rhs = rhs.wrapping_add(1) & mask(width);
        }
        debug_assert!(pred == IntPredicate::Slt, "Must be signed predicate");
        let cr = fn_(rhs)?;
        Some(if invert { cr.inverse() } else { cr })
    }

    /// Get value range for a "ctpop(Val) Pred RHS" condition.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1329>
    fn get_value_from_icmp_ctpop(
        env: &Env,
        pred: IntPredicate,
        rhs: Operand,
    ) -> Lattice {
        let width = env.scalar_size_in_bits(rhs);

        let Some((rhs_const, _)) = env.query().int_constant(rhs) else { return Lattice::get_overdefined() };

        let res_val_range = ConstantRange::make_exact_icmp_region(pred, width, rhs_const);

        let res_min = apint::limited(res_val_range.get_unsigned_min(), width);
        let res_max = apint::limited(res_val_range.get_unsigned_max(), width);

        let val_min = mask(res_min);
        let val_max = apint::set_high(0, width, res_max);
        Lattice::get_range(ConstantRange::non_empty(width, val_min, val_max.wrapping_add(1) & mask(width)), false)
    }

    /// Get the unsigned range for \p V from a `mul nuw V, V` comparison.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1350>
    fn get_range_for_nuw_mul_square(
        env: &Env,
        v: Operand,
        mut pred: IntPredicate,
        lhs: Operand,
        mut rhs: Operand,
    ) -> Option<ConstantRange> {
        if !env.is_integer(v) {
            return None;
        }
        let q = env.query();
        let is_square = |x: Operand| {
            binary(&q, x, BinaryOp::Mul).is_some_and(|(_, i, a, b)| i.flags.contains(Flags::NUW) && a == v && b == v)
        };

        if !is_square(lhs) {
            if !is_square(rhs) {
                return None;
            }

            pred = pred.swapped();
            rhs = lhs;
        }

        let width = env.scalar_size_in_bits(v);
        let mut mul_cr = ConstantRange::full(width);
        if let Some((c, _)) = q.int_constant(rhs) {
            mul_cr = ConstantRange::make_exact_icmp_region(pred, width, c);
        }

        let res = mul_cr.sqrt_floor();
        if res.is_full_set() {
            return None;
        }
        Some(res)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1377>
    fn get_value_from_icmp_condition(
        &mut self,
        env: &mut Env,
        val: Operand,
        ici: InstId,
        is_true_dest: bool,
        use_block_value: bool,
    ) -> Option<Lattice> {
        let function = env.function;
        let icmp_inst = function.instruction(ici);
        let Opcode::ICmp(predicate) = icmp_inst.opcode else { unreachable!("an icmp") };
        let (lhs, rhs) = (icmp_inst.operands[0], icmp_inst.operands[1]);

        // Get the predicate that must hold along the considered edge.
        let edge_pred = if is_true_dest { predicate } else { predicate.inverse() };
        let is_equality = matches!(predicate, IntPredicate::Eq | IntPredicate::Ne);

        if let Operand::Constant(rhs_c) = rhs
            && is_equality
            && lhs == val
        {
            if edge_pred == IntPredicate::Eq {
                return Some(Lattice::get(&LatticeConstant::of(env.context, rhs_c)));
            } else if !matches!(
                env.context.get(rhs_c).kind,
                ConstantKind::Poison | ConstantKind::Undef
            ) {
                return Some(Lattice::get_not(&LatticeConstant::of(env.context, rhs_c)));
            }
        }

        if !env.is_integer(val) {
            return Some(Lattice::get_overdefined());
        }

        if let Some(range) = Self::get_range_for_nuw_mul_square(env, val, edge_pred, lhs, rhs) {
            return Some(Lattice::get_range(range, false));
        }

        let mut offset = 0u128;
        let val_add = if matches!(
            env.query().instruction_of(val).map(|(_, instruction)| &instruction.opcode),
            Some(Opcode::Binary(BinaryOp::Add | BinaryOp::Or))
        ) {
            Self::pattern_tried();
            add_like(&env.query(), val, Flags::default())
        } else {
            None
        };
        if Self::match_icmp_operand(env, &mut offset, lhs, val, edge_pred, val_add) {
            return self.get_value_from_simple_icmp_condition(env, edge_pred, rhs, offset, ici, use_block_value);
        }

        let swapped_pred = edge_pred.swapped();
        if Self::match_icmp_operand(env, &mut offset, rhs, val, swapped_pred, val_add) {
            return self.get_value_from_simple_icmp_condition(env, swapped_pred, lhs, offset, ici, use_block_value);
        }

        let bit_width = env.scalar_size_in_bits(val);
        let q = env.query();
        // The probes of `lhs` below each match one kind of instruction: its
        // defining opcode is read once, and a constant or an argument (the
        // usual left side here) matches none.
        let lhs_op = q.instruction_of(lhs).map(|(_, instruction)| &instruction.opcode);
        let lhs_is = |wanted: fn(&Opcode) -> bool| lhs_op.is_some_and(wanted);
        if lhs_is(|op| matches!(op, Opcode::Call(_)))
            && Self::pattern_tried()
            && let Some((_, args)) = intrinsic_call(&q, lhs, "llvm.ctpop")
            && args.len() == 1
            && args[0] == val
        {
            return Some(Self::get_value_from_icmp_ctpop(env, edge_pred, rhs));
        }

        if lhs_is(|op| matches!(op, Opcode::Binary(BinaryOp::And)))
            && Self::pattern_tried()
            && let Some((_, _, a, b)) = binary(&q, lhs, BinaryOp::And)
            && a == val
            && let Some((mask_bits, _)) = q.int_constant(b)
            && let Some((c, _)) = q.int_constant(rhs)
        {
            // If (Val & Mask) == C then all the masked bits are known and we
            // can compute a value range based on that.
            if edge_pred == IntPredicate::Eq {
                let known = KnownBits { zero: !c & mask_bits & mask(bit_width), one: c & mask_bits, width: bit_width };
                return Some(Lattice::get_range(ConstantRange::from_known_bits(&known, false), false));
            }

            if edge_pred == IntPredicate::Ne {
                return Some(Lattice::get_range(
                    ConstantRange::make_mask_not_equal_range(bit_width, mask_bits, c),
                    false,
                ));
            }
        }

        // If (X urem Modulus) >= C, then X >= C.
        // If trunc X >= C, then X >= C.
        // TODO: An upper bound could be computed as well.
        let urem_or_trunc = lhs_is(|op| matches!(op, Opcode::Binary(BinaryOp::URem)))
            && Self::pattern_tried()
            && binary(&q, lhs, BinaryOp::URem).is_some_and(|(_, _, a, _)| a == val)
            || lhs_is(|op| matches!(op, Opcode::Cast(CastOp::Trunc)))
                && Self::pattern_tried()
                && cast(&q, lhs, CastOp::Trunc).is_some_and(|(_, _, x)| x == val);
        if urem_or_trunc && let Some((c, c_width)) = q.int_constant(rhs) {
            // Use the icmp region so we don't have to deal with different
            // predicates.
            let cr = ConstantRange::make_exact_icmp_region(edge_pred, c_width, c);
            if !cr.is_empty_set() {
                return Some(Lattice::get_range(ConstantRange::non_empty(bit_width, cr.get_unsigned_min(), 0), false));
            }
        }

        // Recognize:
        // icmp slt (ashr X, ShAmtC), C --> icmp slt X, C << ShAmtC
        // Preconditions: (C << ShAmtC) >> ShAmtC == C
        if matches!(
            edge_pred,
            IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
        )
            && lhs_is(|op| matches!(op, Opcode::Binary(BinaryOp::AShr)))
            && Self::pattern_tried()
            && let Some((_, _, a, b)) = binary(&q, lhs, BinaryOp::AShr)
            && a == val
            && let Some((sh_amt, _)) = q.int_constant(b)
            && let Some((c, _)) = q.int_constant(rhs)
        {
            let cr = Self::get_range_via_slt(edge_pred, c, bit_width, &|rhs: u128| -> Option<ConstantRange> {
                let amount = apint::limited(sh_amt, bit_width);
                let new = apint::shl(rhs, bit_width, amount);
                if apint::ashr(new, bit_width, amount.min(bit_width)) != rhs {
                    return None;
                }
                Some(ConstantRange::non_empty(bit_width, apint::sign_bit::<u128>(bit_width), new))
            });
            if let Some(cr) = cr {
                return Some(Lattice::get_range(cr, false));
            }
        }

        // a - b or ptrtoint(a) - ptrtoint(b) ==/!= 0 if a ==/!= b
        if is_equality
            && q.instruction_of(val).is_some_and(|(_, instruction)| instruction.opcode == Opcode::Binary(BinaryOp::Sub))
            && Self::pattern_tried()
            && let Some((_, _, mut x, mut y)) = binary(&q, val, BinaryOp::Sub)
        {
            // Peek through ptrtoints
            if let Some(a) = ptr_to_int_same_size(env, x) {
                x = a;
            }
            if let Some(a) = ptr_to_int_same_size(env, y) {
                y = a;
            }
            if (x == lhs && y == rhs) || (x == rhs && y == lhs) {
                let null_val = LatticeConstant::null_value(env.context, function.operand_type(env.context, val)?);
                if edge_pred == IntPredicate::Eq {
                    return Some(Lattice::get(&null_val));
                }
                return Some(Lattice::get_not(&null_val));
            }
        }

        Some(Lattice::get_overdefined())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1483>
    fn get_value_from_trunc(
        env: &Env,
        val: Operand,
        trunc: InstId,
        is_true_dest: bool,
    ) -> Lattice {
        let instruction = env.function.instruction(trunc);
        debug_assert!(env.scalar_size_in_bits(value_of(instruction)) == 1);

        if instruction.operands[0] != val {
            return Lattice::get_overdefined();
        }

        let width = env.scalar_size_in_bits(val);
        let ty = env.function.operand_type(env.context, val).expect("an integer");
        let _ = ty;

        if instruction.flags.contains(Flags::NUW) {
            if is_true_dest {
                return Lattice::get(&LatticeConstant::Int { width, bits: 1 });
            }
            return Lattice::get(&LatticeConstant::Int { width, bits: 0 });
        }

        if is_true_dest {
            return Lattice::get_not(&LatticeConstant::Int { width, bits: 0 });
        }
        Lattice::get_not(&LatticeConstant::Int { width, bits: mask(width) })
    }

    /// Handle conditions of the form
    /// extractvalue(op.with.overflow(%x, C), 1).
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1506>
    fn get_value_from_overflow_condition(
        env: &Env,
        val: Operand,
        wo: InstId,
        is_true_dest: bool,
    ) -> Lattice {
        // TODO: This only works with a constant RHS for now. We could also
        // compute the range of the RHS, but this doesn't fit into the
        // current structure of the edge value calculation.
        let instruction = env.function.instruction(wo);
        let q = env.query();
        let Some(crate::intrinsics::Intrinsic::WithOverflow { op, signed }) = q.intrinsic(wo) else {
            unreachable!("a with.overflow intrinsic")
        };
        let (wo_lhs, wo_rhs) = (instruction.operands[0], instruction.operands[1]);
        let Some((c, width)) = q.int_constant(wo_rhs).filter(|_| wo_lhs == val) else {
            return Lattice::get_overdefined();
        };

        // Calculate the possible values of %x for which no overflow occurs.
        let mut nwr = ConstantRange::make_exact_no_wrap_region(
            op,
            width,
            c,
            if signed { NO_SIGNED_WRAP } else { NO_UNSIGNED_WRAP },
        );

        // If overflow is false, %x is constrained to NWR. If overflow is true,
        // %x is constrained to it's inverse (all values that might
        // cause overflow).
        if is_true_dest {
            nwr = nwr.inverse();
        }
        Lattice::get_range(nwr, false)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1526>
    fn get_value_from_condition(
        &mut self,
        env: &mut Env,
        val: Operand,
        cond: Operand,
        is_true_dest: bool,
        use_block_value: bool,
        depth: u32,
    ) -> Option<Lattice> {
        let found = def_of(env.function, cond);
        if let Some((ici, instruction)) = found {
            match &instruction.opcode {
                Opcode::ICmp(_) => {
                    return self.get_value_from_icmp_condition(env, val, ici, is_true_dest, use_block_value);
                }
                Opcode::Cast(CastOp::Trunc) => return Some(Self::get_value_from_trunc(env, val, ici, is_true_dest)),
                Opcode::ExtractValue(indices) => {
                    if let Some((wo, _)) = pattern::with_overflow_inst(&env.query(), instruction.operands[0])
                        && indices.len() == 1
                        && indices[0] == 1
                    {
                        return Some(Self::get_value_from_overflow_condition(env, val, wo, is_true_dest));
                    }
                }
                _ => {}
            }
        }

        let depth = depth + 1;
        if depth == MAX_ANALYSIS_RECURSION_DEPTH {
            return Some(Lattice::get_overdefined());
        }

        let q = env.query();
        if let Some(n) = not(&q, cond) {
            return self.get_value_from_condition(env, val, n, !is_true_dest, use_block_value, depth);
        }

        let (is_and, l, r) = if let Some((l, r)) = logical_and(&q, cond) {
            (true, l, r)
        } else if let Some((l, r)) = logical_or(&q, cond) {
            (false, l, r)
        } else {
            return Some(Lattice::get_overdefined());
        };

        let mut lv = self.get_value_from_condition(env, val, l, is_true_dest, use_block_value, depth)?;
        let rv = self.get_value_from_condition(env, val, r, is_true_dest, use_block_value, depth)?;

        // if (L && R) -> intersect L and R
        // if (!(L || R)) -> intersect !L and !R
        // if (L || R) -> union L and R
        // if (!(L && R)) -> union !L and !R
        if is_true_dest ^ is_and {
            lv.merge_in(&rv, MergeOptions::default());
            return Some(lv);
        }

        Some(lv.intersect(&rv))
    }

    /// Compute the value of Val on the edge BBFrom -> BBTo.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1627>
    fn get_edge_value_local(
        &mut self,
        env: &mut Env,
        val: Operand,
        bb_from: BlockId,
        bb_to: BlockId,
        use_block_value: bool,
    ) -> Option<Lattice> {
        let function = env.function;
        let terminator = function.terminator(bb_from)?;
        let term = function.instruction(terminator);
        // TODO: Handle more complex conditionals. If (v == 0 || v2 < 1) is
        // false, we know that v != 0.
        if term.opcode == Opcode::Br && term.operands.len() == 3 {
            let (Operand::Block(succ0), Operand::Block(succ1)) = (term.operands[1], term.operands[2]) else {
                unreachable!("a conditional branch names two blocks")
            };
            // If this is a conditional branch and only one successor goes to
            // BBTo, then we may be able to infer something from the
            // condition.
            if succ0 != succ1 {
                let is_true_dest = succ0 == bb_to;
                assert!(if is_true_dest { succ0 } else { succ1 } == bb_to, "BBTo isn't a successor of BBFrom");
                let condition = term.operands[0];

                // If V is the condition of the branch itself, then we know
                // exactly what it is.
                // NB: The condition on a `br` can't be a vector type.
                if condition == val {
                    return Some(Lattice::get(&LatticeConstant::Int { width: 1, bits: u128::from(is_true_dest) }));
                }

                // If the condition of the branch is an equality comparison, we
                // may be able to infer the value.
                let mut result =
                    self.get_value_from_condition(env, val, condition, is_true_dest, use_block_value, 0)?;

                if !result.is_overdefined() {
                    return Some(result);
                }

                if let Some((usr, user)) = def_of(env.function, val) {
                    debug_assert!(result.is_overdefined(), "Result isn't overdefined");
                    // Check with isOperationFoldable() first to avoid linearly
                    // iterating over the operands
                    // unnecessarily which can be expensive for
                    // instructions with many operands.
                    if matches!(env.context.types.get(user.ty), Type::Int(_)) && is_operation_foldable(user) {
                        if uses_operand(user, condition) {
                            // If Val has Condition as an operand and Val can be
                            // folded into a
                            // constant with either Condition == true or
                            // Condition == false,
                            // propagate the constant.
                            // eg.
                            //   ; %Val is true on the edge to %then.
                            //   %Val = and i1 %Condition, true.
                            //   br %Condition, label %then, label %else
                            let condition_val = u128::from(is_true_dest);
                            result = constant_fold_user(env, usr, condition, condition_val);
                        } else if matches!(
                            user.opcode,
                            Opcode::Cast(CastOp::Trunc | CastOp::ZExt | CastOp::SExt)
                        ) {
                            let op_lattice_val = self
                                .get_value_from_condition(env, user.operands[0], condition, is_true_dest, false, 0)
                                .expect("no block value is asked for");

                            if op_lattice_val.is_constant_range(true) {
                                let result_bit_width = env.scalar_size_in_bits(value_of(user));
                                let range = op_lattice_val.get_constant_range(true);
                                let Opcode::Cast(cast_op) = user.opcode else { unreachable!("a cast") };
                                if cast_op == CastOp::Trunc {
                                    return Some(Lattice::get_range(
                                        range.truncate(result_bit_width, no_wrap_kind(user.flags)),
                                        false,
                                    ));
                                }

                                return Some(Lattice::get_range(range.cast_op(cast_op, result_bit_width), false));
                            }
                            if op_lattice_val.is_constant() {
                                // A constant here is not an integer: the cast
                                // of it is
                                // not an integer cast MIR folds.
                                let _ = op_lattice_val.get_constant();
                            }
                            return Some(Lattice::get_overdefined());
                        } else {
                            // If one of Val's operand has an inferred value, we
                            // may be able to
                            // infer the value of Val.
                            // eg.
                            //    ; %Val is 94 on the edge to %then.
                            //    %Val = add i8 %Op, 1
                            //    %Condition = icmp eq i8 %Op, 93
                            //    br i1 %Condition, label %then, label %else
                            for i in 0..user.operands.len() {
                                let op = user.operands[i];
                                let op_lattice_val = self
                                    .get_value_from_condition(env, op, condition, is_true_dest, false, 0)
                                    .expect("no block value is asked for");
                                if let Some(op_const) = op_lattice_val.as_constant_integer() {
                                    result = constant_fold_user(env, usr, op, op_const);
                                    break;
                                }
                            }
                        }
                    }
                }
                if !result.is_overdefined() {
                    return Some(result);
                }
            }
        }

        // If the edge was formed by a switch on the value, then we may know
        // exactly what it is.
        if term.opcode == Opcode::Switch {
            let condition = term.operands[0];
            if !env.is_integer(val) {
                return Some(Lattice::get_overdefined());
            }
            let mut val_uses_condition_and_may_be_foldable = false;
            if condition != val {
                // Check if Val has Condition as an operand.
                if let Some((_, usr)) = def_of(env.function, val) {
                    val_uses_condition_and_may_be_foldable = is_operation_foldable(usr) && uses_operand(usr, condition);
                }
                if !val_uses_condition_and_may_be_foldable {
                    return Some(Lattice::get_overdefined());
                }
            }
            debug_assert!(
                condition == val || val_uses_condition_and_may_be_foldable,
                "Condition != Val nor Val doesn't use Condition"
            );

            let Operand::Block(default_dest) = term.operands[1] else { unreachable!("a default destination") };
            let default_case = default_dest == bb_to;
            let bit_width = env.scalar_size_in_bits(val);
            let mut edges_vals =
                if default_case { ConstantRange::full(bit_width) } else { ConstantRange::empty(bit_width) };

            for pair in term.operands[2..].chunks(2) {
                let (Operand::Constant(case_value), Operand::Block(case_successor)) = (pair[0], pair[1]) else {
                    unreachable!("a case is a constant and a block")
                };
                let (case_bits, case_width) =
                    env.query().int_constant(Operand::Constant(case_value)).expect("an integer case");
                let mut edge_val = ConstantRange::single(case_width, case_bits);
                if val_uses_condition_and_may_be_foldable {
                    let (usr, _) = def_of(env.function, val).expect("an instruction");
                    let edge_lattice_val = constant_fold_user(env, usr, condition, case_bits);
                    if edge_lattice_val.is_overdefined() {
                        return Some(Lattice::get_overdefined());
                    }
                    edge_val = *edge_lattice_val.get_constant_range(true);
                }
                if default_case {
                    // It is possible that the default destination is the
                    // destination of some cases. We cannot
                    // perform difference for those cases.
                    // We know Condition != CaseValue in BBTo.  In some cases we
                    // can use this to infer Val ==
                    // f(Condition) is != f(CaseValue).  For now, we
                    // only do this when f is identity (i.e. Val == Condition),
                    // but we should be able to do this for
                    // any injective f.
                    if case_successor != bb_to && condition == val {
                        edges_vals = edges_vals.difference(&edge_val);
                    }
                } else if case_successor == bb_to {
                    edges_vals = edges_vals.union_with(&edge_val, PreferredRangeType::Smallest);
                }
            }
            return Some(Lattice::get_range(edges_vals, false));
        }
        Some(Lattice::get_overdefined())
    }

    /// Compute the value of Val on the edge BBFrom -> BBTo or the value at
    /// the basic block if the edge does not constrain Val.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1778>
    fn get_edge_value(
        &mut self,
        env: &mut Env,
        val: Operand,
        bb_from: BlockId,
        bb_to: BlockId,
        cxt_i: Option<InstId>,
    ) -> Option<Lattice> {
        // If already a constant, there is nothing to compute.
        if let Operand::Constant(vc) = val {
            return Some(Lattice::get(&LatticeConstant::of(env.context, vc)));
        }

        let local_result = self.get_edge_value_local(env, val, bb_from, bb_to, true)?;

        if has_single_value(&local_result) {
            // Can't get any more precise here
            return Some(local_result);
        }

        let mut in_block = self.get_block_value(env, val, bb_from, env.function.terminator(bb_from))?;

        // We can use the context instruction (generically the ultimate
        // instruction the calling pass is trying to simplify) here,
        // even though the result of this function is generally cached
        // when called from the solve* functions (and that cached result
        // might be used with queries using a different
        // context instruction), because when this function is called from the
        // solve* functions, the context instruction is not provided.
        // When called from LazyValueInfoImpl::getValueOnEdge, the
        // context instruction is provided, but then the result is not
        // cached.
        self.intersect_assume_or_guard_block_value_constant_range(env, val, &mut in_block, cxt_i);

        Some(local_result.intersect(&in_block))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1813>
    pub fn get_value_in_block(
        &mut self,
        env: &mut Env,
        v: Operand,
        bb: BlockId,
        cxt_i: Option<InstId>,
    ) -> Lattice {
        debug_assert!(self.block_value_stack.is_empty() && self.block_value_set.is_empty());
        if let Some(log) = &mut self.log {
            log.push(Event::Block(v, bb, cxt_i));
        }
        let mut opt_result = self.get_block_value(env, v, bb, cxt_i);
        if opt_result.is_none() {
            self.solve(env);
            opt_result = self.get_block_value(env, v, bb, cxt_i);
        }

        opt_result.expect("Value not available after solving")
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1830>
    pub fn get_value_at(
        &mut self,
        env: &mut Env,
        v: Operand,
        cxt_i: InstId,
    ) -> Lattice {
        if let Operand::Constant(c) = v {
            return Lattice::get(&LatticeConstant::of(env.context, c));
        }

        let mut result = Lattice::get_overdefined();
        if let Some((i, _)) = def_of(env.function, v) {
            result = get_from_range_metadata(env, i);
        }
        self.intersect_assume_or_guard_block_value_constant_range(env, v, &mut result, Some(cxt_i));

        result
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1846>
    pub fn get_value_on_edge(
        &mut self,
        env: &mut Env,
        v: Operand,
        from_bb: BlockId,
        to_bb: BlockId,
        cxt_i: Option<InstId>,
    ) -> Lattice {
        if let Some(log) = &mut self.log {
            log.push(Event::Edge(v, from_bb, to_bb, cxt_i));
        }
        let mut result = self.get_edge_value(env, v, from_bb, to_bb, cxt_i);
        while result.is_none() {
            // As the worklist only explicitly tracks block values (but not edge
            // values) we may have to call solve() multiple times,
            // as the edge value calculation may request additional
            // block values.
            self.solve(env);
            result = self.get_edge_value(env, v, from_bb, to_bb, cxt_i);
        }

        result.expect("solved")
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1867>
    pub fn get_value_at_use(
        &mut self,
        env: &mut Env,
        u: Use,
    ) -> Lattice {
        let function = env.function;
        let v = function.instruction(u.user).operands[u.index as usize];
        let cxt_i = u.user;
        let mut vl = self.get_value_in_block(env, v, function.parent(cxt_i).expect("a placed user"), Some(cxt_i));
        let mut last_queried_bb = function.parent(cxt_i).expect("a placed user");

        // Check whether the only (possibly transitive) use of the value is in a
        // position where V can be constrained by a select or branch condition.
        let mut curr_u = u;
        // TODO: Increase limit?
        const MAX_USES_TO_INSPECT: u32 = 3;
        for i in 0..MAX_USES_TO_INSPECT {
            let mut cond_val: Option<Lattice> = None;
            let curr_i = curr_u.user;
            let curr_inst = function.instruction(curr_i);
            let curr_parent = function.parent(curr_i).expect("a placed user");

            // All instructions on the one-use chain between the original use
            // and CurrI are speculatable and have a single user
            // each, so they could be sunk to CurrI. This means
            // information that holds at CurrI also holds for the
            // original use and can be used to refine it. Skip phis, as V may
            // not dominate their block.
            if i != 0 && curr_inst.opcode != Opcode::Phi && curr_parent != last_queried_bb {
                last_queried_bb = curr_parent;
                vl = vl.intersect(&self.get_value_in_block(env, v, last_queried_bb, Some(curr_i)));
            }

            if curr_inst.opcode == Opcode::Select {
                // If the value is undef, a different value may be chosen in
                // the select condition and at use.
                if !is_guaranteed_not_to_be_undef(&env.query_with_assumptions(), curr_inst.operands[0], 0) {
                    break;
                }
                if curr_u.index == 1 {
                    cond_val = Some(
                        self.get_value_from_condition(env, v, curr_inst.operands[0], true, false, 0)
                            .expect("no block value is asked for"),
                    );
                } else if curr_u.index == 2 {
                    cond_val = Some(
                        self.get_value_from_condition(env, v, curr_inst.operands[0], false, false, 0)
                            .expect("no block value is asked for"),
                    );
                }
            } else if curr_inst.opcode == Opcode::Phi {
                // TODO: Use non-local query?
                let Operand::Block(incoming) = curr_inst.operands[curr_u.index as usize + 1] else {
                    unreachable!("a phi's block")
                };
                cond_val = Some(
                    self.get_edge_value_local(env, v, incoming, curr_parent, false)
                        .expect("no block value is asked for"),
                );
            }
            if let Some(cond_val) = cond_val {
                vl = vl.intersect(&cond_val);
            }

            // Only follow one-use chain, to allow direct intersection of
            // conditions. If there are multiple uses, we would have
            // to intersect with the union of all conditions at
            // different uses. Stop walking if we hit a
            // non-speculatable instruction. Even if the
            // result is only used under a specific condition, executing the
            // instruction itself may cause side effects or UB already.
            // This also disallows looking through phi nodes: If the phi node is
            // part of a cycle, we might end up reasoning about
            // values from different cycle iterations (PR60629).
            let single_use = curr_inst.result.and_then(|r| function.users(r).only().copied());
            let Some(next_use) = single_use else { break };
            if !is_safe_to_speculatively_execute_with_variable_replaced(env, curr_i, false) {
                break;
            }
            curr_u = next_use;
        }
        vl
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1936>
    pub fn thread_edge(
        &mut self,
        env: &Env,
        _pred_bb: BlockId,
        old_succ: BlockId,
        new_succ: BlockId,
    ) {
        if let Some(log) = &mut self.log {
            log.push(Event::ThreadEdge(_pred_bb, old_succ, new_succ));
        }
        self.the_cache.thread_edge_impl(env.function, old_succ, new_succ);
    }
}

/// `predecessors(BB)`, newest use first as LLVM walks a use list.
pub(crate) fn predecessors(
    function: &Function,
    bb: BlockId,
) -> crate::blocklist::BlockList {
    crate::counts::bump(crate::counts::Event::LviPredecessorLists);
    function.predecessors(bb)
}

/// `Constant::toConstantRange` of an integer constant.
fn constant_to_range(
    env: &Env,
    c: ConstantId,
) -> ConstantRange {
    match LatticeConstant::of(env.context, c) {
        LatticeConstant::Int { width, bits } => ConstantRange::single(width, bits),
        _ => {
            let ty = env.context.get(c).ty;
            ConstantRange::full(env.context.types.int_bits(ty).unwrap_or(1))
        }
    }
}

/// `getNoWrapKind()` of a `trunc`.
fn no_wrap_kind(flags: Flags) -> u32 {
    (if flags.contains(Flags::NUW) { NO_UNSIGNED_WRAP } else { 0 })
        | (if flags.contains(Flags::NSW) { NO_SIGNED_WRAP } else { 0 })
}

/// `m_PtrToIntSameSize(DL, m_Value(X))`: the operand of a `ptrtoint` whose
/// result is as wide as the pointer.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/PatternMatch.h#L2155>
fn ptr_to_int_same_size(
    env: &Env,
    v: Operand,
) -> Option<Operand> {
    let q = env.query();
    let (_, instruction, x) = cast(&q, v, CastOp::PtrToInt)?;
    (q.scalar_bits(value_of(instruction)) == q.scalar_bits(x)).then_some(x)
}

/// Return true if Usr has Op as an operand, otherwise false.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1579>
fn uses_operand(
    usr: &crate::module::Instruction,
    op: Operand,
) -> bool {
    usr.operands.contains(&op)
}

/// Return true if the instruction type of Val is supported by
/// constantFoldUser(). Currently CastInst, BinaryOperator and FreezeInst only.
/// Call this before calling constantFoldUser() to find out if it's even worth
/// attempting to call it.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1587>
fn is_operation_foldable(usr: &crate::module::Instruction) -> bool {
    matches!(
        usr.opcode,
        Opcode::Cast(_) | Opcode::Binary(_) | Opcode::Freeze
    )
}

/// Check if Usr can be simplified to an integer constant when the value of one
/// of its operands Op is an integer constant OpConstVal. If so, return it as an
/// lattice value range with a single element or otherwise return an overdefined
/// lattice value.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1595>
fn constant_fold_user(
    env: &mut Env,
    usr: InstId,
    op: Operand,
    op_const_val: u128,
) -> Lattice {
    let function = env.function;
    let user = function.instruction(usr);
    debug_assert!(is_operation_foldable(user), "Precondition");
    let op_ty = function.operand_type(env.context, op).expect("an integer operand");
    let op_width = env.context.types.int_bits(op_ty).expect("an integer operand");
    let op_const = Operand::Constant(
        env.context.constant(Constant { ty: op_ty, kind: ConstantKind::Int(op_const_val & mask(op_width)) }),
    );
    let as_range = |env: &Env, folded: Option<Operand>| -> Option<Lattice> {
        let Operand::Constant(c) = folded? else { return None };
        match LatticeConstant::of(env.context, c) {
            LatticeConstant::Int { width, bits } => Some(Lattice::get_range(ConstantRange::single(width, bits), false)),
            _ => None,
        }
    };
    // Check if Usr can be simplified to a constant.
    match &user.opcode {
        Opcode::Cast(cast_op) => {
            debug_assert!(user.operands[0] == op, "Operand 0 isn't Op");
            let folded =
                crate::simplify::simplify_cast_operand(env.context, env.layout, function, *cast_op, op_const, user.ty);
            if let Some(lattice) = as_range(env, folded) {
                return lattice;
            }
        }
        Opcode::Binary(binary_op) => {
            let op0_match = user.operands[0] == op;
            let op1_match = user.operands[1] == op;
            debug_assert!(op0_match || op1_match, "Operand 0 nor Operand 1 isn't a match");
            let lhs = if op0_match { op_const } else { user.operands[0] };
            let rhs = if op1_match { op_const } else { user.operands[1] };
            let folded = crate::simplify::simplify_binop_operands(
                env.context,
                env.layout,
                function,
                *binary_op,
                Flags::default(),
                lhs,
                rhs,
            );
            if let Some(lattice) = as_range(env, folded) {
                return lattice;
            }
        }
        Opcode::Freeze => {
            debug_assert!(user.operands[0] == op, "Operand 0 isn't Op");
            return Lattice::get_range(ConstantRange::single(op_width, op_const_val & mask(op_width)), false);
        }
        _ => {}
    }
    Lattice::get_overdefined()
}

/// `simplifyExtractValueInst(Agg, Idxs, Q)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/InstructionSimplify.cpp#L5526>
fn simplify_extract_value_inst(
    env: &mut Env,
    agg: Operand,
    idxs: &[u32],
) -> Option<Operand> {
    if let Operand::Constant(c_agg) = agg {
        return constant_fold_extract_value_instruction(env, c_agg, idxs).map(Operand::Constant);
    }

    // extractvalue x, (insertvalue y, elt, n), n -> elt
    let function = env.function;
    let num_idxs = idxs.len();
    let mut visited = HashSet::default();
    let mut current = def_of(env.function, agg).filter(|(_, i)| matches!(i.opcode, Opcode::InsertValue(_)));
    while let Some((ivi, insert)) = current {
        // Protect against insertvalue cycles in unreachable code.
        if !visited.insert(ivi) {
            break;
        }

        let Opcode::InsertValue(insert_value_idxs) = &insert.opcode else { unreachable!("an insertvalue") };
        let num_common_idxs = insert_value_idxs.len().min(num_idxs);
        if insert_value_idxs[..num_common_idxs] == idxs[..num_common_idxs] {
            if num_idxs == insert_value_idxs.len() {
                return Some(insert.operands[1]);
            }
            break;
        }
        current = def_of(env.function, insert.operands[0]).filter(|(_, i)| matches!(i.opcode, Opcode::InsertValue(_)));
    }

    // Simplify umul_with_overflow where one operand is 1.
    if idxs.len() == 1
        && let Some((_, args)) = intrinsic_call(&env.query(), agg, "llvm.umul.with.overflow")
        && let [a, b] = *args
    {
        let q = env.query();
        let is_one = |x: Operand| q.int_constant(x).is_some_and(|(bits, _)| bits == 1);
        let v = if is_one(b) {
            Some(a)
        } else if is_one(a) {
            Some(b)
        } else {
            None
        };
        if let Some(v) = v {
            if idxs[0] == 0 {
                return Some(v);
            }
            debug_assert!(idxs[0] == 1, "invalid index");
            let i1 = env.context.types.int(1);
            return Some(Operand::Constant(env.context.int(i1, 0)));
        }
    }
    let _ = function;

    None
}

/// `ConstantFoldExtractValueInstruction`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFold.cpp#L538>
fn constant_fold_extract_value_instruction(
    env: &mut Env,
    agg: ConstantId,
    idxs: &[u32],
) -> Option<ConstantId> {
    // Base case: no indices, so return the entire value.
    let Some((&first, rest)) = idxs.split_first() else { return Some(agg) };

    let Constant { ty, kind } = env.context.get(agg).clone();
    let element_ty = env.context.types.member(ty, u64::from(first))?;
    let element = match kind {
        ConstantKind::Aggregate(members) => *members.get(first as usize)?,
        ConstantKind::Zero => zero_value(env.context, element_ty),
        ConstantKind::Poison => env.context.constant(Constant { ty: element_ty, kind: ConstantKind::Poison }),
        ConstantKind::Undef => env.context.constant(Constant { ty: element_ty, kind: ConstantKind::Undef }),
        _ => return None,
    };
    constant_fold_extract_value_instruction(env, element, rest)
}

/// `Constant::getNullValue(Ty)`.
fn zero_value(
    context: &Context,
    ty: TypeId,
) -> ConstantId {
    let kind = match context.types.get(ty) {
        Type::Int(_) => ConstantKind::Int(0),
        Type::Pointer(_) => ConstantKind::Null,
        Type::Float(_) => ConstantKind::Float(0),
        _ => ConstantKind::Zero,
    };
    context.constant(Constant { ty, kind })
}

/// `isSafeToSpeculativelyExecuteWithVariableReplaced(I,
/// IgnoreUBImplyingAttrs)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7575>
fn is_safe_to_speculatively_execute_with_variable_replaced(
    env: &Env,
    inst: InstId,
    ignore_ub_implying_attrs: bool,
) -> bool {
    let instruction = env.function.instruction(inst);
    let q = env.query();
    match &instruction.opcode {
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem) => {
            // x / y is undefined if y == 0.
            q.int_constant(instruction.operands[1]).is_some_and(|(bits, _)| bits != 0)
        }
        Opcode::Binary(BinaryOp::SDiv | BinaryOp::SRem) => {
            // x / y is undefined if y == 0 or x == INT_MIN and y == -1
            let Some((denominator, width)) = q.int_constant(instruction.operands[1]) else { return false };
            // We cannot hoist this division if the denominator is 0.
            if denominator == 0 {
                return false;
            }
            // It's safe to hoist if the denominator is not 0 or -1.
            if denominator != mask(width) {
                return true;
            }
            // At this point we know that the denominator is -1.  It is safe to
            // hoist as long we know that the numerator is not
            // INT_MIN.
            if let Some((numerator, _)) = q.int_constant(instruction.operands[0]) {
                return numerator != apint::sign_bit::<u128>(width);
            }
            // The numerator *might* be MinSignedValue.
            false
        }
        // !UseVariableInfo
        Opcode::Load { .. } => false,
        Opcode::Call(_) => {
            let Some(callee) = pattern::called_function(&q, instruction) else { return false };
            // The called function could have undefined behavior or
            // side-effects, even if marked readnone nounwind.
            if !callee.attrs.iter().any(|a| matches!(a, Attribute::Flag(name) if name == "speculatable")) {
                return false;
            }
            // Since the operands may be changed after hoisting, undefined
            // behavior may be triggered by some UB-implying
            // attributes.
            ignore_ub_implying_attrs || !has_ub_implying_attrs(env, instruction)
        }
        Opcode::Alloca { .. }
        | Opcode::Invoke(_)
        | Opcode::Phi
        | Opcode::Store { .. }
        | Opcode::Ret
        | Opcode::Br
        | Opcode::Switch
        | Opcode::Unreachable
        | Opcode::LandingPad { .. }
        | Opcode::Resume => false, // Misc instructions which have effects
        _ => true,
    }
}

/// `Instruction::hasUBImplyingAttrs()` of a call.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L595>
fn has_ub_implying_attrs(
    env: &Env,
    instruction: &crate::module::Instruction,
) -> bool {
    let q = env.query();
    let Some(info) = pattern::call_info(instruction) else { return false };
    let named = |attrs: &[Attribute]| {
        attrs
            .iter()
            .any(
                |a| match a {
                    Attribute::Flag(_) => Fact::of_attribute(a) == Some(Fact::NoUndef),
                    Attribute::Int(name, _) => name == "dereferenceable" || name == "dereferenceable_or_null",
                    _ => false,
                },
            )
    };
    let callee = pattern::called_function(&q, instruction);
    for arg_no in 0..call_arguments(instruction).len() {
        // CallBase::isPassingUndefUB
        if info.argument_attrs.get(arg_no).is_some_and(|a| named(a))
            || callee.is_some_and(|f| f.parameter_attrs.get(arg_no).is_some_and(|a| named(a)))
        {
            return true;
        }
    }
    named(&info.return_attrs) || callee.is_some_and(|f| named(&f.return_attrs))
}

/// `LazyValueInfo`: the analysis a pass holds; the cache is built on the first
/// query.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/LazyValueInfo.h#L35>
#[derive(Default)]
pub struct LazyValueInfo {
    imp: Option<LazyValueInfoImpl>,
}

/// Returns true if we can statically tell that this value will never be a
/// "useful" constant.  In practice, this means we've got something like an
/// alloca or a malloc call for which a comparison against a constant can
/// only be guarding dead code.  Note that we are potentially giving up some
/// precision in dead code (a constant result) in favour of avoiding a
/// expensive search for a easily answered common query.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2013>
fn is_known_non_constant(
    env: &Env,
    v: Operand,
) -> bool {
    let v = strip_pointer_casts(env, v);
    // The return val of alloc cannot be a Constant.
    def_of(env.function, v).is_some_and(|(_, i)| matches!(i.opcode, Opcode::Alloca { .. }))
}

/// `Value::stripPointerCasts()`: casts and GEPs with all-zero indices.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Value.cpp#L712>
fn strip_pointer_casts(
    env: &Env,
    ptr: Operand,
) -> Operand {
    let q = env.query();
    if !q.is_pointer(ptr) {
        return ptr;
    }
    let mut visited = HashSet::default();
    let mut v = ptr;
    visited.insert(v);
    loop {
        let Some((_, instruction)) = q.instruction_of(v) else { return v };
        let next = match &instruction.opcode {
            Opcode::GetElementPtr { .. } => {
                let all_zero =
                    instruction.operands[1..].iter().all(|one| q.int_constant(*one).is_some_and(|(bits, _)| bits == 0));
                if !all_zero {
                    return v;
                }
                instruction.operands[0]
            }
            Opcode::Cast(CastOp::BitCast) => {
                let new_v = instruction.operands[0];
                if !q.is_pointer(new_v) {
                    return v;
                }
                new_v
            }
            Opcode::Cast(CastOp::AddrSpaceCast) => instruction.operands[0],
            Opcode::Call(_) | Opcode::Invoke(_) => match pattern::returned_argument_operand(&q, instruction) {
                Some(returned) => returned,
                None => return v,
            },
            _ => return v,
        };
        v = next;
        if !visited.insert(v) {
            return v;
        }
    }
}

/// The LVI of a function, kept from one pass to the next while nothing edits
/// the function: LLVM's `LazyValueAnalysis` result, whose value cache lives
/// until a pass that changes the function does not preserve it. A pass takes
/// the cache for its run and puts it back.
pub struct SharedLvi(std::rc::Rc<std::cell::RefCell<LazyValueInfo>>);

impl SharedLvi {
    pub fn take(&self) -> LazyValueInfo {
        std::mem::take(&mut *self.0.borrow_mut())
    }

    pub fn put(
        &self,
        lvi: LazyValueInfo,
    ) {
        *self.0.borrow_mut() = lvi;
    }
}

/// All caches are equal: `PartialEq` cannot see the function. A held cache is
/// judged by its answers, in `LviCache::audit` (`LLRM_CHECK_CAPTURES`), and
/// kept true by the passes that edit (`forget_value`, `erase_block`,
/// `thread_edge`) and by dropping it when a pass does not preserve it, as
/// LLVM's `LazyValueInfo::invalidate` does
/// (`LazyValueInfo.cpp:1987`).
/// Compared by identity with a fresh, empty cache, every cache a pass left
/// standing was stale (6 Nib programs, `fixednarrow left lvi held but stale`).
impl PartialEq for SharedLvi {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl std::fmt::Debug for SharedLvi {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str("SharedLvi")
    }
}

/// `LazyValueAnalysis`: an empty [`SharedLvi`], filled by the passes that ask.
pub struct LviCache;

impl crate::passes::Analysis for LviCache {
    type Result = SharedLvi;
    const NAME: &'static str = "lvi";

    fn run(
        _: &Context,
        _: &DataLayout,
        _: &Function,
        _: &mut crate::passes::Analyses,
    ) -> SharedLvi {
        SharedLvi(Default::default())
    }

    const AUDITED: bool = true;

    /// Every `(value, block)` the cache holds answers as a fresh one does:
    /// fewer entries is laziness, a different answer is one an edit left
    /// behind without `forget_value`.
    fn audit(
        result: &SharedLvi,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut crate::passes::Analyses,
    ) {
        let outer = std::rc::Rc::clone(analyses.outer());
        let assumptions = AssumptionCache::of(context, &outer.globals, &outer.metadata, function);
        let mut scratch = context.clone();
        let mut env = Env::new(&mut scratch, &outer.globals, &outer.metadata, function, layout, &assumptions);
        let stale = result.0.borrow_mut().disagreements(&mut env);
        assert!(stale.is_empty(), "lvi held but stale: {stale:?}");
    }
}

impl LazyValueInfo {
    /// The memoised `(block, value)` pairs whose answer differs from the one
    /// a fresh `LazyValueInfo` gives when asked what this one was asked and
    /// told, in that order, on the body as it stands. The cache is path
    /// dependent (a range holds one of several gaps by which question came
    /// first), so a fresh lone query is not what a sound cache holds; the same
    /// questions in the same order are.
    pub fn disagreements(
        &mut self,
        env: &mut Env,
    ) -> Vec<(BlockId, Operand)> {
        let Some(imp) = self.get_impl() else { return Vec::new() };
        let held = imp.memoised();
        let Some(events) = imp.log.clone() else { return Vec::new() };
        let function = env.function;
        let mut blocks = crate::dense::IdSet::<BlockId>::new();
        for &bb in function.layout() {
            blocks.insert(bb);
        }
        let placed = |inst: InstId| !function.is_erased(inst) && function.parent(inst).is_some();
        let alive = |v: Operand, cxt: Option<InstId>| {
            let value = match v {
                Operand::Value(value) => match function.value(value).def {
                    ValueDef::Instruction(inst) => placed(inst),
                    ValueDef::Argument(_) => true,
                },
                _ => true,
            };
            value && cxt.is_none_or(placed)
        };
        let mut fresh = LazyValueInfo::new();
        let replay = fresh.get_or_create_impl(env);
        for event in events {
            match event {
                Event::Block(v, bb, cxt) if blocks.contains(&bb) && alive(v, cxt) => {
                    replay.get_value_in_block(env, v, bb, cxt);
                }
                Event::Edge(v, from, to, cxt) if blocks.contains(&from) && blocks.contains(&to) && alive(v, cxt) => {
                    replay.get_value_on_edge(env, v, from, to, cxt);
                }
                Event::Forget(v) => replay.forget_value(v),
                Event::EraseBlock(bb) => replay.erase_block(bb),
                Event::ThreadEdge(pred, old, new) if [pred, old, new].iter().all(|bb| blocks.contains(bb)) => {
                    replay.thread_edge(env, pred, old, new);
                }
                Event::Clear => replay.clear(),
                _ => {}
            }
        }
        let replayed: HashMap<(BlockId, Operand), Lattice> =
            replay.memoised().into_iter().map(|(bb, v, answer)| ((bb, v), answer)).collect();
        // An entry the replay did not make is laziness: a question about a
        // value or block the body no longer holds.
        held.into_iter()
            .filter(|(bb, v, answer)| replayed.get(&(*bb, *v)).is_some_and(|again| again != answer))
            .map(|(bb, v, _)| (bb, v))
            .collect()
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// This lazily constructs the LazyValueInfoImpl.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1965>
    pub fn get_or_create_impl(
        &mut self,
        env: &Env,
    ) -> &mut LazyValueInfoImpl {
        self.imp.get_or_insert_with(|| LazyValueInfoImpl::new(env))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1975>
    pub fn get_impl(&mut self) -> Option<&mut LazyValueInfoImpl> {
        self.imp.as_mut()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L1979>
    pub fn release_memory(&mut self) {
        // If the cache was allocated, free it.
        self.imp = None;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2021>
    pub fn get_constant(
        &mut self,
        env: &mut Env,
        v: Operand,
        cxt_i: InstId,
    ) -> Option<LatticeConstant> {
        // Bail out early if V is known not to be a Constant.
        if is_known_non_constant(env, v) {
            return None;
        }

        let bb = env.function.parent(cxt_i).expect("a placed instruction");
        let result = self.get_or_create_impl(env).get_value_in_block(env, v, bb, Some(cxt_i));

        if result.is_constant() {
            return Some(result.get_constant().clone());
        }
        if result.is_constant_range(true)
            && let Some(single_val) = result.get_constant_range(true).get_single_element()
        {
            return Some(LatticeConstant::Int { width: result.get_constant_range(true).width, bits: single_val });
        }
        None
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2039>
    pub fn get_constant_range(
        &mut self,
        env: &mut Env,
        v: Operand,
        cxt_i: InstId,
        undef_allowed: bool,
    ) -> ConstantRange {
        let bb = env.function.parent(cxt_i).expect("a placed instruction");
        let result = self.get_or_create_impl(env).get_value_in_block(env, v, bb, Some(cxt_i));
        result.as_constant_range_of_type(env.scalar_size_in_bits(v), undef_allowed)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2046>
    pub fn get_constant_range_at_use(
        &mut self,
        env: &mut Env,
        u: Use,
        undef_allowed: bool,
    ) -> ConstantRange {
        let v = env.function.instruction(u.user).operands[u.index as usize];
        let result = self.get_or_create_impl(env).get_value_at_use(env, u);
        result.as_constant_range_of_type(env.scalar_size_in_bits(v), undef_allowed)
    }

    /// Determine whether the specified value is known to be a
    /// constant on the specified edge. Return null if not.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2054>
    pub fn get_constant_on_edge(
        &mut self,
        env: &mut Env,
        v: Operand,
        from_bb: BlockId,
        to_bb: BlockId,
        cxt_i: Option<InstId>,
    ) -> Option<LatticeConstant> {
        let result = self.get_or_create_impl(env).get_value_on_edge(env, v, from_bb, to_bb, cxt_i);

        if result.is_constant() {
            return Some(result.get_constant().clone());
        }
        if result.is_constant_range(true)
            && let Some(single_val) = result.get_constant_range(true).get_single_element()
        {
            return Some(LatticeConstant::Int { width: result.get_constant_range(true).width, bits: single_val });
        }
        None
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2070>
    pub fn get_constant_range_on_edge(
        &mut self,
        env: &mut Env,
        v: Operand,
        from_bb: BlockId,
        to_bb: BlockId,
        cxt_i: Option<InstId>,
    ) -> ConstantRange {
        let result = self.get_or_create_impl(env).get_value_on_edge(env, v, from_bb, to_bb, cxt_i);
        // TODO: Should undef be allowed here?
        result.as_constant_range_of_type(env.scalar_size_in_bits(v), true)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2080>
    fn get_predicate_result(
        env: &mut Env,
        pred: IntPredicate,
        c: ConstantId,
        val: &Lattice,
    ) -> Option<bool> {
        let c = LatticeConstant::of(env.context, c);
        // If we know the value is a constant, evaluate the conditional.
        if val.is_constant() {
            return constant_fold_compare_inst_operands(env, pred, val.get_constant(), &c);
        }

        if val.is_constant_range(true) {
            let cr = val.get_constant_range(true);
            let rhs = match &c {
                LatticeConstant::Int { width, bits } => ConstantRange::single(*width, *bits),
                _ => ConstantRange::full(cr.width),
            };
            if cr.icmp(pred, &rhs) {
                return Some(true);
            }
            if cr.icmp(pred.inverse(), &rhs) {
                return Some(false);
            }
            return None;
        }

        if val.is_not_constant() {
            // If this is an equality comparison, we can try to fold it knowing
            // that "V != C1".
            if pred == IntPredicate::Eq {
                // !C1 == C -> false iff C1 == C.
                let res = constant_fold_compare_inst_operands(env, IntPredicate::Ne, val.get_not_constant(), &c);
                if res == Some(false) {
                    return Some(false);
                }
            } else if pred == IntPredicate::Ne {
                // !C1 != C -> true iff C1 == C.
                let res = constant_fold_compare_inst_operands(env, IntPredicate::Ne, val.get_not_constant(), &c);
                if res == Some(false) {
                    return Some(true);
                }
            }
            return None;
        }

        None
    }

    /// Determine whether the specified value comparison with a constant is
    /// known to be true or false on the specified CFG edge. Pred is a
    /// CmpInst predicate.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2122>
    pub fn get_predicate_on_edge(
        &mut self,
        env: &mut Env,
        pred: IntPredicate,
        v: Operand,
        c: ConstantId,
        from_bb: BlockId,
        to_bb: BlockId,
        cxt_i: Option<InstId>,
    ) -> Option<bool> {
        let result = self.get_or_create_impl(env).get_value_on_edge(env, v, from_bb, to_bb, cxt_i);

        Self::get_predicate_result(env, pred, c, &result)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2132>
    pub fn get_predicate_at(
        &mut self,
        env: &mut Env,
        pred: IntPredicate,
        v: Operand,
        c: ConstantId,
        cxt_i: InstId,
        use_block_value: bool,
    ) -> Option<bool> {
        // Is or is not NonNull are common predicates being queried. If
        // isKnownNonZero can tell us the result of the predicate, we can
        // return it quickly. But this is only a fastpath, and falling
        // through would still be correct.
        // NOTE: This check is meant to determine whether a pointer is
        // semantically a null pointer, not just whether its value
        // equals ConstantPointerNull. If the semantics of
        // ConstantPointerNull change in the future, this should be
        // updated to use a semantic check (e.g. isKnownNonNull).
        if env.is_pointer(v) && LatticeConstant::of(env.context, c).is_null_value() {
            let stripped = strip_pointer_casts_same_representation(&env.query(), v);
            if is_known_non_zero(&env.query(), stripped, 0) {
                if pred == IntPredicate::Eq {
                    return Some(false);
                } else if pred == IntPredicate::Ne {
                    return Some(true);
                }
            }
        }

        let function = env.function;
        let bb = function.parent(cxt_i).expect("a placed instruction");
        let imp = self.get_or_create_impl(env);
        let result = if use_block_value {
            imp.get_value_in_block(env, v, bb, Some(cxt_i))
        } else {
            imp.get_value_at(env, v, cxt_i)
        };
        let ret = Self::get_predicate_result(env, pred, c, &result);
        if ret.is_some() {
            return ret;
        }

        // Note: The following bit of code is somewhat distinct from the rest of
        // LVI; LVI as a whole tries to compute a lattice value which is
        // conservatively correct at a given location.  In this case, we
        // have a predicate which we weren't able to prove about the
        // merged result, and we're pushing that predicate back along
        // each incoming edge to see if we can prove it separately for
        // each input.  As a motivating example, consider: bb1:
        //   %v1 = ... ; constantrange<1, 5>
        //   br label %merge
        // bb2:
        //   %v2 = ... ; constantrange<10, 20>
        //   br label %merge
        // merge:
        //   %phi = phi [%v1, %v2] ; constantrange<1,20>
        //   %pred = icmp eq i32 %phi, 8
        // We can't tell from the lattice value for '%phi' that '%pred' is false
        // along each path, but by checking the predicate over each input
        // separately, we can.
        // We limit the search to one step backwards from the current BB and
        // value. We could consider extending this to search further
        // backwards through the CFG and/or value graph, but there are
        // non-obvious compile time vs quality tradeoffs.

        // Function entry or an unreachable block.  Bail to avoid confusing
        // analysis below.
        let preds = predecessors(function, bb);
        let first = *preds.first()?;

        // If V is a PHI node in the same block as the context, we need to ask
        // questions about the predicate as applied to the incoming value along
        // each edge. This is useful for eliminating cases where the predicate
        // is known along all incoming edges.
        if let Some((_, phi)) = def_of(env.function, v).filter(|(_, i)| i.opcode == Opcode::Phi)
            && let Some((phi_inst, _)) = def_of(env.function, v)
            && function.parent(phi_inst) == Some(bb)
        {
            let mut baseline: Option<bool> = None;
            for (i, (incoming, pred_bb)) in pattern::phi_incoming(phi).enumerate() {
                // Note that PredBB may be BB itself.
                let result = self.get_predicate_on_edge(env, pred, incoming, c, pred_bb, bb, Some(cxt_i));

                // Keep going as long as we've seen a consistent known result
                // for all inputs.
                baseline = if i == 0 {
                    result // First iteration
                } else if baseline == result {
                    baseline
                } else {
                    None // All others
                };
                if baseline.is_none() {
                    break;
                }
            }
            if baseline.is_some() {
                return baseline;
            }
        }

        // For a comparison where the V is outside this block, it's possible
        // that we've branched on it before. Look to see if the value is known
        // on all incoming edges.
        let defined_here = env.query().instruction_of(v).is_some_and(|(inst, _)| function.parent(inst) == Some(bb));
        if !defined_here {
            // For predecessor edge, determine if the comparison is true or
            // false on that edge. If they're all true or all false,
            // we can conclude the value of the comparison in this
            // block.
            let baseline = self.get_predicate_on_edge(env, pred, v, c, first, bb, Some(cxt_i));
            if baseline.is_some() {
                // Check that all remaining incoming values match the first one.
                let mut all_match = true;
                for &p in &preds[1..] {
                    let ret = self.get_predicate_on_edge(env, pred, v, c, p, bb, Some(cxt_i));
                    if ret != baseline {
                        all_match = false;
                        break;
                    }
                }
                // If we terminated early, then one of the values didn't match.
                if all_match {
                    return baseline;
                }
            }
        }

        None
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2242>
    pub fn get_predicate_at_values(
        &mut self,
        env: &mut Env,
        pred: IntPredicate,
        lhs: Operand,
        rhs: Operand,
        cxt_i: InstId,
        use_block_value: bool,
    ) -> Option<bool> {
        if let Operand::Constant(c) = rhs {
            return self.get_predicate_at(env, pred, lhs, c, cxt_i, use_block_value);
        }
        if let Operand::Constant(c) = lhs {
            return self.get_predicate_at(env, pred.swapped(), rhs, c, cxt_i, use_block_value);
        }

        // Got two non-Constant values. Try to determine the comparison results
        // based on the block values of the two operands, e.g. because
        // they have non-overlapping ranges.
        if use_block_value {
            let bb = env.function.parent(cxt_i).expect("a placed instruction");
            let l = self.get_or_create_impl(env).get_value_in_block(env, lhs, bb, Some(cxt_i));
            if l.is_overdefined() {
                return None;
            }

            let r = self.get_or_create_impl(env).get_value_in_block(env, rhs, bb, Some(cxt_i));
            let mut fold = |pred: IntPredicate, a: &LatticeConstant, b: &LatticeConstant| {
                constant_fold_compare_inst_operands(env, pred, a, b)
            };
            return l.get_compare(pred, &r, &mut fold);
        }
        None
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2268>
    pub fn thread_edge(
        &mut self,
        env: &Env,
        pred_bb: BlockId,
        old_succ: BlockId,
        new_succ: BlockId,
    ) {
        if let Some(imp) = self.get_impl() {
            imp.thread_edge(env, pred_bb, old_succ, new_succ);
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2274>
    pub fn forget_value(
        &mut self,
        v: Operand,
    ) {
        if let Some(imp) = self.get_impl() {
            imp.forget_value(v);
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2279>
    pub fn erase_block(
        &mut self,
        bb: BlockId,
    ) {
        if let Some(imp) = self.get_impl() {
            imp.erase_block(bb);
        }
    }

    /// Complete flush all previously computed values
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2284>
    pub fn clear(&mut self) {
        if let Some(imp) = self.get_impl() {
            imp.clear();
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2289>
    pub fn print_lvi(
        &mut self,
        env: &mut Env,
        dtree: &DomTree,
    ) -> String {
        match self.get_impl() {
            Some(imp) => imp.print_lvi(env, dtree),
            None => String::new(),
        }
    }
}

impl LazyValueInfoImpl {
    /// What the cache holds: each block's solved values and overdefined ones.
    fn memoised(&self) -> Vec<(BlockId, Operand, Lattice)> {
        let mut held = Vec::new();
        for (number, entry) in self.the_cache.block_cache.iter().enumerate() {
            let Some(entry) = entry else { continue };
            let bb = BlockId(number as _);
            held.extend(entry.lattice_elements.iter().map(|(v, answer)| (bb, *v, answer.clone())));
            held.extend(entry.over_defined.iter().map(|v| (bb, *v, Lattice::get_overdefined())));
        }
        held
    }

    /// Complete flush all previously computed values
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L497>
    pub fn clear(&mut self) {
        if let Some(log) = &mut self.log {
            log.push(Event::Clear);
        }
        self.the_cache.clear();
    }

    /// This is part of the update interface to remove information related to
    /// this value from the cache.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L489>
    pub fn forget_value(
        &mut self,
        v: Operand,
    ) {
        if let Some(log) = &mut self.log {
            log.push(Event::Forget(v));
        }
        self.the_cache.erase_value(v);
    }

    /// This is part of the update interface to inform the cache
    /// that a block has been deleted.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L493>
    pub fn erase_block(
        &mut self,
        bb: BlockId,
    ) {
        if let Some(log) = &mut self.log {
            log.push(Event::EraseBlock(bb));
        }
        self.the_cache.erase_block(bb);
    }

    /// Printing the LazyValueInfo Analysis.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L482>
    pub fn print_lvi(
        &mut self,
        env: &mut Env,
        dtree: &DomTree,
    ) -> String {
        let mut out = String::new();
        for annotation in self.annotations(env, dtree) {
            out += &annotation.text(env);
        }
        out
    }

    /// What `F.print(OS, &Writer)` asks `LazyValueInfoAnnotatedWriter` for, in
    /// the order it asks.
    pub fn annotations(
        &mut self,
        env: &mut Env,
        dtree: &DomTree,
    ) -> Vec<Annotation> {
        let function = env.function;
        let mut out = Vec::new();
        for &bb in function.layout() {
            self.emit_basic_block_start_annot(env, bb, &mut out);
            for i in function.iter_block(bb) {
                self.emit_instruction_annot(env, dtree, i, &mut out);
            }
        }
        out
    }

    /// Print the LVI for the function arguments at the start of each basic
    /// block.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2295>
    fn emit_basic_block_start_annot(
        &mut self,
        env: &mut Env,
        bb: BlockId,
        os: &mut Vec<Annotation>,
    ) {
        // Find if there are latticevalues defined for arguments of the
        // function.
        let function = env.function;
        for &arg in function.parameters() {
            let result = self.get_value_in_block(env, Operand::Value(arg), bb, None);
            if result.is_unknown() {
                continue;
            }
            os.push(Annotation {
                subject: Subject::Argument(arg),
                block: bb,
                lattice: result.print(env.context, env.globals),
            });
        }
    }

    /// This function prints the LVI analysis for the instruction I at the
    /// beginning of various basic blocks. It relies on calculated values
    /// that are stored in the LazyValueInfoCache, and in the absence of
    /// cached values, recalculate the LazyValueInfo for `I`, and print that
    /// info.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LazyValueInfo.cpp#L2312>
    fn emit_instruction_annot(
        &mut self,
        env: &mut Env,
        dt: &DomTree,
        i: InstId,
        os: &mut Vec<Annotation>,
    ) {
        let function = env.function;
        let result_id = function.instruction(i).result;
        let parent_bb = function.parent(i).expect("a placed instruction");
        let mut blocks_containing_lvi = crate::dense::IdSet::<BlockId>::new();
        // We can generate (solve) LVI values only for blocks that are dominated
        // by the I's parent. However, to avoid generating LVI for all
        // dominating blocks, that contain redundant/uninteresting
        // information, we print LVI for blocks that may use this LVI
        // information (such as immediate successor blocks, and blocks
        // that contain uses of `I`).
        let mut print_result = |this: &mut Self, env: &mut Env, bb: BlockId, os: &mut Vec<Annotation>| {
            if !blocks_containing_lvi.insert(bb) {
                return;
            }
            // An instruction with no result is no value MIR can name; LVI
            // gives it nothing to go on (`getFromRangeMetadata`) in its own
            // block and in every block its block dominates (DEVIATE 6).
            let lattice = match result_id {
                Some(result) => this.get_value_in_block(env, Operand::Value(result), bb, None),
                None => Lattice::get_overdefined(),
            };
            os.push(Annotation {
                subject: Subject::Instruction(i),
                block: bb,
                lattice: lattice.print(env.context, env.globals),
            });
        };

        print_result(self, env, parent_bb, os);
        // Print the LVI analysis results for the immediate successor blocks,
        // that are dominated by `ParentBB`.
        for bb_succ in function.successors(parent_bb) {
            if dt.dominates(parent_bb, bb_succ) {
                print_result(self, env, bb_succ, os);
            }
        }

        // Print LVI in blocks where `I` is used.
        for u in result_id.into_iter().flat_map(|result| function.users(result).iter()) {
            let use_i = function.instruction(u.user);
            let use_bb = function.parent(u.user).expect("a placed user");
            if use_i.opcode != Opcode::Phi || dt.dominates(parent_bb, use_bb) {
                print_result(self, env, use_bb, os);
            }
        }
    }
}

/// What a `; LatticeVal for:` line is about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Subject {
    Argument(crate::module::ValueId),
    Instruction(InstId),
}

/// One `; LatticeVal for:` line of `printLVI`: the value, the block it is
/// asked in, and the lattice element as `operator<<` prints it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Annotation {
    pub subject: Subject,
    pub block: BlockId,
    pub lattice: String,
}

impl Annotation {
    /// The line as LLVM writes it, an instruction named by its result or its
    /// opcode where LLVM writes the whole instruction (DEVIATE 6).
    fn text(
        &self,
        env: &Env,
    ) -> String {
        match self.subject {
            Subject::Argument(arg) => {
                format!("; LatticeVal for: '{}' is: {}\n", value_text(env, Operand::Value(arg)), self.lattice)
            }
            Subject::Instruction(i) => {
                let instruction = env.function.instruction(i);
                let subject = match instruction.result {
                    Some(result) => value_text(env, Operand::Value(result)),
                    None => format!("{:?}", instruction.opcode)
                        .split(['(', ' ', '{'])
                        .next()
                        .unwrap_or_default()
                        .to_lowercase(),
                };
                format!(
                    "; LatticeVal for: '{subject}' in BB: '%{}' is: {}\n",
                    env.function.block(self.block).name.clone().unwrap_or_default(),
                    self.lattice
                )
            }
        }
    }
}

/// `*V` of a value: its type and name.
fn value_text(
    env: &Env,
    v: Operand,
) -> String {
    let ty = env.function.operand_type(env.context, v).map(|ty| env.context.types.display(ty)).unwrap_or_default();
    match v {
        Operand::Value(id) => format!("{ty} %{}", env.function.value(id).name.clone().unwrap_or_default()),
        _ => ty,
    }
}

/// `ConstantFoldCompareInstOperands(Pred, C1, C2, DL)`: true, false, or
/// unfolded.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1332>
fn constant_fold_compare_inst_operands(
    env: &mut Env,
    pred: IntPredicate,
    c1: &LatticeConstant,
    c2: &LatticeConstant,
) -> Option<bool> {
    crate::valuelattice::fold_compare(env.context, env.globals, env.layout, pred, c1, c2)
}
