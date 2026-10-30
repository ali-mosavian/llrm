//! LoopIdiomRecognize: a translation of LLVM's `LoopIdiomRecognize.cpp` at
//! `d1106deb71cc`, the countable half (memset, memset.pattern and memcpy/
//! memmove from a strided store), function for function.
//!
//! # How it works
//! [`LoopIdiomRecognize`] holds what the C++ class holds besides the analyses:
//! the options, the library's answers and the stores `collect_stores` found in
//! the block being scanned. Each method takes the [`Cx`] of the function, its
//! loops and dominators, and the [`ScalarEvolution`], as the translation of
//! ScalarEvolution does.
//!
//! # Deviations
//! - D9a: CRC loops are the table strategy only; see [`crc`].
//! - D10: see [`process`].
//! Absent in MIR, so skipped (facts, not choices):
//! - Atomics: a store or load is simple and unordered unless `volatile`, so
//!   `UnorderedAtomicMemcpy` has no producer.
//! - `hasUnstableRepresentation` and `hasExternalState` of the datalayout
//!   (non-integral pointers, CHERI): MIR's datalayout has neither, so they are
//!   false.
//! - Scalable vectors.
//!
//! # See also
//! [`LoopIdiomRecognize.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp),
//! `docs/architecture/loopidiom-port.md`.

use llrm_analysis::aa::{AAQueryInfo, AAResults, LocationSize, MemoryLocation, ModRefInfo};
use llrm_analysis::scev::ap::Ap;
use llrm_analysis::scev::expr::{Scev, Ty, Use, flags};
use llrm_analysis::scev::{Cx, Flags as ScevFlags, ScalarEvolution};
use llrm_mir::context::{ConstantId, ConstantKind};
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::types::Type;
use llrm_mir::valuetracking::bytewise::{Byte, is_bytewise_value};
use llrm_mir::valuetracking::underlying::{MAX_LOOKUP_SEARCH_DEPTH, get_underlying_object};

/// The `cl::opt`s of the file, at their defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Options {
    /// `disable-loop-idiom-all`.
    pub disable_all: bool,
    /// `disable-loop-idiom-memset`.
    pub disable_memset: bool,
    /// `disable-loop-idiom-memcpy`.
    pub disable_memcpy: bool,
    /// `disable-loop-idiom-hashrecognize`.
    pub disable_hash_recognize: bool,
    /// `use-lir-code-size-heurs`.
    pub use_lir_code_size_heurs: bool,
    /// `loop-idiom-force-memset-pattern-intrinsic`.
    pub force_memset_pattern_intrinsic: bool,
    /// `loop-idiom-crc-strategy`.
    pub crc_strategy: CrcStrategy,
}

/// `CRCStrategyKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrcStrategy {
    Disable,
    Auto,
    Table,
    Clmul,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            disable_all: false,
            disable_memset: false,
            disable_memcpy: false,
            disable_hash_recognize: false,
            use_lir_code_size_heurs: true,
            force_memset_pattern_intrinsic: false,
            crc_strategy: CrcStrategy::Auto,
        }
    }
}

/// What `TargetLibraryInfo::has` says of the three functions the countable
/// half forms calls to.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Library {
    pub memset: bool,
    pub memset_pattern16: bool,
    pub memcpy: bool,
}

/// `LegalStoreKind`, less `UnorderedAtomicMemcpy` (no atomics in MIR).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegalStoreKind {
    None,
    Memset,
    MemsetPattern,
    Memcpy,
}

/// The stores of one block by the object they write, in the order found
/// (`MapVector<Value *, StoreList>`).
pub type StoreListMap = Vec<(Operand, Vec<InstId>)>;

/// `LoopIdiomRecognize`.
pub struct LoopIdiomRecognize {
    pub options: Options,
    library: Library,
    /// The routines the compile states the program links (`strlen`).
    pub routines: Vec<llrm_mir::libfunc::Routine>,
    /// The compile is -Os or -Oz: the pipeline's stand-in for the `optsize`
    /// attribute clang puts on every function there. Read by the CRC table
    /// only; the stores keep their own reading of the attribute.
    pub for_size: bool,
    apply_code_size_heuristics: bool,
    /// The function being run has `"no-builtins"`: the pin's TLI then states
    /// every library function unavailable (`libcall`).
    no_builtins: bool,
    has_memset: bool,
    has_memset_pattern: bool,
    has_memcpy: bool,
    pub(crate) store_refs_for_memset: StoreListMap,
    pub(crate) store_refs_for_memset_pattern: StoreListMap,
    pub(crate) store_refs_for_memcpy: Vec<InstId>,
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L427
/// `getStoreStride`: the constant stride of a store's add recurrence.
pub fn get_store_stride(
    se: &ScalarEvolution,
    store_ev: Scev,
) -> Ap {
    let (_, step, _) = se.match_affine_add_rec(store_ev).expect("a store's address is an add recurrence");
    se.match_apint(step).expect("a store's stride is a constant")
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L440
/// `getMemSetPatternValue`: the constant a strided store of `v` writes as a
/// pattern, if it is safe to turn into a memset.pattern.
pub fn get_mem_set_pattern_value(
    cx: &Cx,
    v: Operand,
) -> Option<ConstantId> {
    // If the value isn't a constant, we can't promote it to being in a
    // constant array.
    let Operand::Constant(id) = v else { return None };
    let constant = cx.context.get(id);
    if matches!(constant.kind, ConstantKind::Expr(_)) {
        return None;
    }

    // Only handle simple values that are a power of two bytes in size.
    let size = cx.layout.size_bits(&cx.context.types, constant.ty);
    if size == 0 || (size & 7) != 0 || (size & (size - 1)) != 0 {
        return None;
    }

    // Don't care enough about darwin/ppc to implement this.
    if cx.layout.big_endian {
        return None;
    }

    // TODO (LLVM): If CI is larger than 16-bytes, we can try slicing it in
    // half to see if the top and bottom are the same.
    if size / 8 > 16 {
        return None;
    }

    // For now, don't handle types that aren't int, floats, or pointers.
    matches!(
        cx.context.types.get(constant.ty),
        Type::Int(_) | Type::Pointer(_) | Type::Float(_)
    ).then_some(id)
}

impl LoopIdiomRecognize {
    /// The pin's per-function `TargetLibraryInfo::has`: a library function
    /// the target states (`stated`) is unavailable in a function with
    /// `"no-builtins"`. Every idiom that forms a libcall asks here.
    pub(crate) fn libcall(
        &self,
        stated: bool,
    ) -> bool {
        stated && !self.no_builtins
    }

    pub fn new(
        options: Options,
        library: Library,
    ) -> Self {
        Self {
            options,
            library,
            routines: Vec::new(),
            for_size: false,
            apply_code_size_heuristics: false,
            no_builtins: false,
            has_memset: library.memset,
            has_memset_pattern: library.memset_pattern16,
            has_memcpy: library.memcpy,
            store_refs_for_memset: Vec::new(),
            store_refs_for_memset_pattern: Vec::new(),
            store_refs_for_memcpy: Vec::new(),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L476
    /// `isLegalStore`.
    pub fn is_legal_store(
        &self,
        cx: &Cx,
        se: &mut ScalarEvolution,
        cur_loop: Loop,
        store: InstId,
    ) -> LegalStoreKind {
        let function = cx.function;
        let context = &cx.context;
        let instruction = function.instruction(store);
        let Opcode::Store { volatile, .. } = instruction.opcode else { return LegalStoreKind::None };
        // Don't touch volatile stores. MIR has no atomic ordering, so an
        // unordered store is any other.
        if volatile {
            return LegalStoreKind::None;
        }

        // Avoid merging nontemporal stores.
        if instruction.metadata.iter().any(|(kind, _)| kind == "nontemporal") {
            return LegalStoreKind::None;
        }

        let stored_val = instruction.operands[0];
        let store_ptr = instruction.operands[1];
        let Some(stored_ty) = function.operand_type(context, stored_val) else { return LegalStoreKind::None };

        // Reject stores that are so large that they overflow an unsigned.
        let size_in_bits = cx.layout.size_bits(&context.types, stored_ty);
        if (size_in_bits & 7) != 0 || (size_in_bits >> 32) != 0 {
            return LegalStoreKind::None;
        }

        // See if the pointer expression is an AddRec like {base,+,1} on the
        // current loop, which indicates a strided store. If we have something
        // else, it's a random store we can't handle.
        let store_ev = se.get_scev(cx, store_ptr);
        let Some((stride_scev, stride)) = constant_stride(se, store_ev, cur_loop) else { return LegalStoreKind::None };

        // See if the store can be turned into a memset.

        // If the stored value is a byte-wise value (like i32 -1), then it may
        // be turned into a memset of i8 -1, assuming that all the consecutive
        // bytes are stored. A store of i32 0x01020304 can never be turned
        // into a memset, but it can be turned into memset_pattern if the
        // target supports it.
        let query = cx.query(Some(store));
        let splat_value = is_bytewise_value(&query, stored_val);

        // If we're allowed to form a memset, and the stored value would be
        // acceptable for memset, use it.
        if self.has_memset
            && !self.options.disable_memset
            && let Some(splat) = splat_value
            // Verify that the stored value is loop invariant. If not, we
            // can't promote the memset.
            && match splat {
                Byte::Value(value) => cur_loop.is_loop_invariant(function, value),
                _ => true,
            }
        {
            // It looks like we can use SplatValue.
            return LegalStoreKind::Memset;
        }
        if (self.has_memset_pattern || self.options.force_memset_pattern_intrinsic)
            && !self.options.disable_memset
            // Don't create memset_pattern16s with address spaces.
            && matches!(
                query.type_of(store_ptr).map(|ty| context.types.get(ty)),
                Some(Type::Pointer(0))
            )
            && get_mem_set_pattern_value(cx, stored_val).is_some()
        {
            // It looks like we can use PatternValue!
            return LegalStoreKind::MemsetPattern;
        }

        // Otherwise, see if the store can be turned into a memcpy.
        if self.has_memcpy && !self.options.disable_memcpy {
            // Check to see if the stride matches the size of the store. If so,
            // then we know that every byte is touched in the loop.
            let store_size = cx.layout.store_size(&context.types, stored_ty);
            if !(equals(stride, store_size) || equals(stride.neg(), store_size)) {
                return LegalStoreKind::None;
            }

            // The store must be feeding a non-volatile load.
            let Some((_, load)) = query.instruction_of(stored_val) else { return LegalStoreKind::None };
            let Opcode::Load { volatile, .. } = load.opcode else { return LegalStoreKind::None };
            if volatile {
                return LegalStoreKind::None;
            }

            // See if the pointer expression is an AddRec like {base,+,1} on
            // the current loop, which indicates a strided load. If we have
            // something else, it's a random load we can't handle.
            let load_ev = se.get_scev(cx, load.operands[0]);

            // The store and load must share the same stride.
            let same = se
                .match_affine_add_rec(load_ev)
                .is_some_and(|(_, step, header)| step == stride_scev && header == cur_loop.get_header());
            if !same {
                return LegalStoreKind::None;
            }

            // Success. This store can be converted into a memcpy.
            return LegalStoreKind::Memcpy;
        }
        // This store can't be transformed into a memset/memcpy.
        LegalStoreKind::None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L591
    /// `collectStores`.
    pub fn collect_stores(
        &mut self,
        cx: &Cx,
        se: &mut ScalarEvolution,
        cur_loop: Loop,
        block: llrm_mir::module::BlockId,
    ) {
        self.store_refs_for_memset.clear();
        self.store_refs_for_memset_pattern.clear();
        self.store_refs_for_memcpy.clear();
        let function = cx.function;
        for inst in function.iter_block(block) {
            if !matches!(function.instruction(inst).opcode, Opcode::Store { .. }) {
                continue;
            }

            // Make sure this is a strided store with a constant stride.
            match self.is_legal_store(cx, se, cur_loop, inst) {
                LegalStoreKind::None => {}
                LegalStoreKind::Memset => {
                    // Find the base pointer.
                    let ptr = underlying(cx, inst);
                    push(&mut self.store_refs_for_memset, ptr, inst);
                }
                LegalStoreKind::MemsetPattern => {
                    let ptr = underlying(cx, inst);
                    push(&mut self.store_refs_for_memset_pattern, ptr, inst);
                }
                LegalStoreKind::Memcpy => self.store_refs_for_memcpy.push(inst),
            }
        }
    }

    /// The stores `collect_stores` found for memset, by base pointer.
    pub fn store_refs_for_memset(&self) -> &StoreListMap {
        &self.store_refs_for_memset
    }

    /// The stores found for memset.pattern, by base pointer.
    pub fn store_refs_for_memset_pattern(&self) -> &StoreListMap {
        &self.store_refs_for_memset_pattern
    }

    /// The stores found for memcpy.
    pub fn store_refs_for_memcpy(&self) -> &[InstId] {
        &self.store_refs_for_memcpy
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1001
/// `isSameByteValueStore`: whether `inst` is a simple store of a loop-invariant
/// value whose bytewise value is `splat_byte`.
pub fn is_same_byte_value_store(
    cx: &Cx,
    cur_loop: Loop,
    inst: InstId,
    splat_byte: Byte,
) -> bool {
    let instruction = cx.function.instruction(inst);
    let Opcode::Store { volatile: false, .. } = instruction.opcode else { return false };
    let value = instruction.operands[0];
    if !cur_loop.is_loop_invariant(cx.function, value) {
        return false;
    }
    is_bytewise_value(&cx.query(Some(inst)), value) == Some(splat_byte)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1017
/// `mayLoopAccessLocation`: whether the loop might access the location `ptr`
/// strides through. `access` says what the verboten forms of access are (read
/// or write). When the access size cannot be bounded, fall back to allow
/// stores writing the same byte value `splat_byte`.
pub fn may_loop_access_location(
    cx: &Cx,
    se: &ScalarEvolution,
    aa: &mut dyn AAResults,
    ptr: Operand,
    access: ModRefInfo,
    cur_loop: Loop,
    be_count: Scev,
    store_size_scev: Scev,
    ignored_insts: &[InstId],
    splat_byte: Option<Byte>,
) -> bool {
    // Get the location that may be stored across the loop. Since the access
    // is strided positively through memory, we say that the modified location
    // starts at the pointer and has infinite size.
    let mut access_size = LocationSize::AfterPointer;

    // If the loop iterates a fixed number of times, we can refine the access
    // size to be exactly the size of the memset, which is
    // (BECount+1)*StoreSize.
    if let (Some(be_cst), Some(const_size)) = (se.match_apint(be_count), se.match_apint(store_size_scev)) {
        let try_zext = |value: Ap| (value.active_bits() <= 64).then(|| value.zext_value() as u64);
        // FIXME (LLVM): Should this check for overflow?
        if let (Some(be_int), Some(size_int)) = (try_zext(be_cst), try_zext(const_size)) {
            access_size = LocationSize::Precise(be_int.wrapping_add(1).wrapping_mul(size_int));
        }
    }

    // TODO (LLVM): For this to be really effective, we have to dive into the
    // pointer operand in the store.
    let store_loc = MemoryLocation::new(ptr, access_size);

    // Only consult the same-byte-value fallback when the access size stayed
    // infinite (non-constant trip count); with a precise size AA is accurate.
    let try_same_byte_value = !matches!(access_size, LocationSize::Precise(_)) && splat_byte.is_some();

    for &block in cur_loop.get_blocks() {
        for inst in cx.function.iter_block(block) {
            if ignored_insts.contains(&inst) {
                continue;
            }
            let mod_ref = aa.get_mod_ref_info(inst, Some(&store_loc), &mut AAQueryInfo::default());
            if !mod_ref.intersect(access).is_no_mod_ref() {
                if try_same_byte_value
                    && let Some(splat) = splat_byte
                    && is_same_byte_value_store(cx, cur_loop, inst, splat)
                {
                    continue;
                }
                return true;
            }
        }
    }
    false
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1064
/// `getStartForNegStride`: if we have a negative stride, `start` refers to
/// the end of the memory location we're trying to memset. Therefore, we need
/// to recompute the base pointer, which is just `start - be_count * size`.
pub fn get_start_for_neg_stride(
    cx: &Cx,
    se: &mut ScalarEvolution,
    start: Scev,
    be_count: Scev,
    int_ptr: Ty,
    store_size_scev: Scev,
) -> Scev {
    let mut index = se.get_truncate_or_zero_extend(cx, be_count, int_ptr, 0);
    if !se.match_one(store_size_scev) {
        // index = back edge count * store size
        let size = se.get_truncate_or_zero_extend(cx, store_size_scev, int_ptr, 0);
        index = se.get_mul_expr(cx, vec![Use::of(index), Use::of(size)], ScevFlags::expr(flags::NUW), 0).scev;
    }
    // base pointer = start - index * store size
    se.get_minus_scev(cx, Use::of(start), Use::of(index), 0, 0)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1082
/// `getNumBytes`: the number of bytes as a SCEV from the backedge taken
/// count, mapped into `int_ptr` and handled in a way that will fold cleanly.
pub fn get_num_bytes(
    cx: &Cx,
    se: &mut ScalarEvolution,
    be_count: Scev,
    int_ptr: Ty,
    store_size_scev: Scev,
    cur_loop: Loop,
) -> Scev {
    let trip_count_scev = se.get_trip_count_from_exit_count(cx, be_count, int_ptr, Some(cur_loop.get_header()));
    let size = se.get_truncate_or_zero_extend(cx, store_size_scev, int_ptr, 0);
    se.get_mul_expr(cx, vec![Use::of(trip_count_scev), Use::of(size)], ScevFlags::expr(flags::NUW), 0).scev
}

/// `match(StoreEv, m_scev_AffineAddRec(m_SCEV(), m_SCEVConstant(Stride),
/// m_SpecificLoop(CurLoop)))`: the step as an expression and its value.
fn constant_stride(
    se: &ScalarEvolution,
    ev: Scev,
    cur_loop: Loop,
) -> Option<(Scev, Ap)> {
    let (_, step, header) = se.match_affine_add_rec(ev)?;
    (header == cur_loop.get_header()).then(|| se.match_apint(step).map(|stride| (step, stride)))?
}

/// `APInt == uint64_t`.
pub(crate) fn equals(
    value: Ap,
    other: u64,
) -> bool {
    value.active_bits() <= 64 && value.zext_value() == u128::from(other)
}

/// `getUnderlyingObject(SI->getPointerOperand())`.
fn underlying(
    cx: &Cx,
    store: InstId,
) -> Operand {
    let query = cx.query(Some(store));
    get_underlying_object(&query, cx.function.instruction(store).operands[1], MAX_LOOKUP_SEARCH_DEPTH)
}

fn push(
    map: &mut StoreListMap,
    key: Operand,
    store: InstId,
) {
    match map.iter_mut().find(|(one, _)| *one == key) {
        Some((_, list)) => list.push(store),
        None => map.push((key, vec![store])),
    }
}

pub mod build;
pub mod crc;
pub mod driver;
pub mod hashrecognize;
pub mod pass;
pub mod process;
pub mod strlen;

#[cfg(test)]
mod crc_tests;
#[cfg(test)]
mod tests;
