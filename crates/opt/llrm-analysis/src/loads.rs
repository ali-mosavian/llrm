//! The value a load can reuse from an access before it in its block.
//!
//! A translation of the available-value half of LLVM's
//! [`Loads.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/Loads.cpp)
//! at d1106deb71cc: `FindAvailableLoadedValue` (both overloads, :552 and
//! :798), `findAvailablePtrLoadStore` (:705), `getAvailableLoadStore` (:617)
//! and `AreEquivalentAddressValues` (:285).
//!
//! # How it works
//! `find_available_loaded_value` scans back from a cursor in one block, at most
//! `max_insts_to_scan` instructions (`DEF_MAX_INSTS_TO_SCAN`, 6, as LLVM's
//! `available-load-scan-limit`; 0 is no limit). The first load of the same
//! address, store to it or constant `memset` that covers it gives the value.
//! A store to a different alloca or global is skipped; any other store, or
//! anything else that may write memory, ends the scan, unless the alias
//! analysis says it does not modify the location. Without one, a store with
//! the same base, constant offsets and no overlap is skipped. The cursor is
//! left at the access that gave the value, or after the one that ended the
//! scan, so a caller can scan on.
//!
//! ```text
//! store i16 %v, ptr %p
//! store i16 0, ptr %q      ; %q and %p are different allocas: skipped
//! %x = load i16, ptr %p    ; %x is %v
//! ```
//!
//! MIRROR: every check and its order, the limit, the cursor.
//!
//! DEVIATE 1: no atomics, volatile aside: MIR has none, so an access is never
//! "at least atomic", and a volatile load or store is not CSE'd (a volatile
//! load of the pointer still gives its value, as LLVM's).
//!
//! DEVIATE 2: no debug instructions to skip, and no element-wise atomics.
//!
//! DEVIATE 3: a `memset` forwarded to a load wider than 128 bits gives
//! nothing (a constant integer here has at most 128 bits).
//!
//! DEVIATE 4: the alias analysis is a `ModRefOracle`, which `BatchAAResults`
//! is, so a caller holding an analysis of its own passes it without naming
//! `BatchAAResults`.
//!
//! # Used by
//! InstCombine's `visitLoadInst`, JumpThreading's load PRE, Promote.
//!
//! # Example
//! ```mir
//! %a = load i16, ptr %p
//! %b = load i16, ptr %p    ; %a, with is_load_cse
//! ```

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::guarantee::strip_pointer_casts_same_representation;
use llrm_mir::valuetracking::nonzero::strip_and_accumulate_constant_offsets;
use llrm_mir::valuetracking::{Query, pattern};

use crate::aa::{AAResults, BatchAAResults, LocationSize, MemoryLocation, ModRefInfo};
use crate::memory::Unit;

/// `DefMaxInstsToScan`.
pub const DEF_MAX_INSTS_TO_SCAN: u32 = 6;

/// The value a scan found and what gave it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Available {
    pub value: Operand,
    /// The value is an earlier load of the address, not a store or a
    /// `memset` to it (`IsLoadCSE`).
    pub is_load_cse: bool,
}

/// What a scan asks of an alias analysis: whether an instruction may modify a
/// location.
pub trait ModRefOracle {
    fn get_mod_ref_info(
        &mut self,
        inst: InstId,
        loc: &MemoryLocation,
    ) -> ModRefInfo;
}

impl<A: AAResults> ModRefOracle for BatchAAResults<'_, A> {
    fn get_mod_ref_info(
        &mut self,
        inst: InstId,
        loc: &MemoryLocation,
    ) -> ModRefInfo {
        BatchAAResults::get_mod_ref_info(self, inst, Some(loc))
    }
}

/// A ValueTracking query over the unit's body, with its globals (a callee is
/// found by name).
fn query<'a>(unit: &Unit<'a>) -> Query<'a> {
    Query { globals: unit.globals, ..Query::bare(unit.context, unit.function, unit.layout) }
}

/// What the load being answered is: the type it reads and the pointer.
#[derive(Clone, Copy)]
struct Access {
    ty: TypeId,
    ptr: Operand,
}

/// `AreEquivalentAddressValues`: the same value, or a cast, phi or GEP that is
/// the same instruction as the other.
fn are_equivalent_address_values(
    function: &Function,
    a: Operand,
    b: Operand,
) -> bool {
    if a == b {
        return true;
    }
    let (Operand::Value(a), Operand::Value(b)) = (a, b) else { return false };
    let (llrm_mir::module::ValueDef::Instruction(a), llrm_mir::module::ValueDef::Instruction(b)) =
        (function.value(a).def, function.value(b).def)
    else {
        return false;
    };
    let (a, b) = (function.instruction(a), function.instruction(b));
    matches!(
        a.opcode,
        Opcode::Cast(_) | Opcode::Phi | Opcode::GetElementPtr { .. }
    )
        && a.is_identical_to_when_defined(b)
}

/// `getAvailableLoadStore`: what `inst` makes available at `access`, if it is
/// a load or a store of the address, or a `memset` that covers it.
fn get_available_load_store(
    unit: &Unit,
    q: &Query,
    inst: InstId,
    access: Access,
) -> Option<Available> {
    let function = unit.function;
    let (types, layout) = (&unit.context.types, unit.layout);
    let instruction = function.instruction(inst);
    let strip = |v| strip_pointer_casts_same_representation(q, v);
    match &instruction.opcode {
        // If this is a load of Ptr, the loaded value is available.
        Opcode::Load { .. } => {
            let same_address = are_equivalent_address_values(function, strip(instruction.operands[0]), access.ptr);
            let value = Operand::Value(instruction.result?);
            (same_address && layout.is_bit_or_noop_pointer_castable(types, instruction.ty, access.ty))
                .then_some(Available { value, is_load_cse: true })
        }
        // If this is a store through Ptr, the value is available.
        Opcode::Store { .. } => {
            if !are_equivalent_address_values(function, strip(instruction.operands[1]), access.ptr) {
                return None;
            }
            let value = instruction.operands[0];
            let value_ty = function.operand_type(unit.context, value)?;
            let found = |value| Some(Available { value, is_load_cse: false });
            if layout.is_bit_or_noop_pointer_castable(types, value_ty, access.ty) {
                return found(value);
            }
            if layout.size_bits(types, access.ty) <= layout.size_bits(types, value_ty)
                && let Operand::Constant(c) = value
                && let Some(folded) =
                    llrm_mir::constantfolding::constant_fold_load_from_const(unit.context, layout, c, access.ty, 0)
            {
                return found(Operand::Constant(folded));
            }
            None
        }
        Opcode::Call(_) if pattern::call_intrinsic(q, inst) == Some(pattern::IntrinsicId::Memset) => {
            memset_available(unit, q, instruction, access).map(|value| Available { value, is_load_cse: false })
        }
        _ => None,
    }
}

/// The constant a `memset` of a number of bytes to a byte gives a load of
/// `access` it covers.
fn memset_available(
    unit: &Unit,
    q: &Query,
    memset: &llrm_mir::module::Instruction,
    access: Access,
) -> Option<Operand> {
    let [dest, value, len, ..] = memset.operands[..] else { return None };
    // Only handle constant memsets.
    let (value, _) = q.int_constant(value)?;
    let (len, _) = q.int_constant(len)?;
    // Handle offsets.
    let width = unit.layout.pointer(pointer_space(unit, dest)?).index_bits;
    let (store_base, store_offset) = base_with_constant_offset(q, dest, width);
    let (load_base, load_offset) = base_with_constant_offset(q, access.ptr, width);
    if store_base != load_base || load_offset < store_offset {
        return None;
    }
    let load_size = unit.layout.size_bits(&unit.context.types, access.ty);
    // Make sure the read bytes are contained in the memset.
    let needed = u128::from(load_size) + ((load_offset - store_offset) as u128) * 8;
    if len * 8 < needed || load_size > 128 {
        return None;
    }
    let byte = value & 0xff;
    let splat = if load_size >= 8 {
        (0..load_size / 8).fold(0u128, |bits, at| bits | byte << (8 * at))
    } else {
        byte & ((1u128 << load_size) - 1)
    };
    let ty = unit.context.types.int(load_size as u32);
    let found = unit.context.types.get(access.ty) == &Type::Int(load_size as u32) || {
        unit.layout.is_bit_or_noop_pointer_castable(&unit.context.types, ty, access.ty)
    };
    found.then(|| Operand::Constant(unit.context.int(ty, splat as i128)))
}

fn pointer_space(
    unit: &Unit,
    ptr: Operand,
) -> Option<u32> {
    let ty = unit.function.operand_type(unit.context, ptr)?;
    match unit.context.types.get(ty) {
        Type::Pointer(space) => Some(*space),
        _ => None,
    }
}

/// `GetPointerBaseWithConstantOffset`: the base of `ptr` and the signed bytes
/// from it.
fn base_with_constant_offset(
    q: &Query,
    ptr: Operand,
    width: u32,
) -> (Operand, i64) {
    let (base, offset) = strip_and_accumulate_constant_offsets(q, ptr, width, true);
    (base, offset as i64)
}

/// `areNonOverlapSameBaseLoadAndStore`: the load and the store have the same
/// base, constant offsets and ranges that do not meet.
fn are_non_overlap_same_base_load_and_store(
    unit: &Unit,
    q: &Query,
    load_ptr: Operand,
    load_ty: TypeId,
    store_ptr: Operand,
    store_ty: TypeId,
) -> bool {
    let Some(space) = pointer_space(unit, load_ptr) else { return false };
    let width = unit.layout.pointer(space).index_bits;
    if pointer_space(unit, store_ptr).map(|space| unit.layout.pointer(space).index_bits) != Some(width) {
        return false;
    }
    let (load_base, load_offset) = strip_and_accumulate_constant_offsets(q, load_ptr, width, false);
    let (store_base, store_offset) = strip_and_accumulate_constant_offsets(q, store_ptr, width, false);
    if load_base != store_base {
        return false;
    }
    let types = &unit.context.types;
    let (load_end, store_end) = (
        load_offset.wrapping_add(unit.layout.store_size(types, load_ty) as u128),
        store_offset.wrapping_add(unit.layout.store_size(types, store_ty) as u128),
    );
    // Two ranges of `width` bits do not meet when one ends where the other
    // starts or before (a range that wraps meets the other).
    let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
    let (load_offset, store_offset, load_end, store_end) =
        (load_offset & mask, store_offset & mask, load_end & mask, store_end & mask);
    let wraps = |start: u128, end: u128| end < start;
    if wraps(load_offset, load_end) || wraps(store_offset, store_end) {
        return false;
    }
    load_end <= store_offset || store_end <= load_offset
}

/// Whether `ptr` is an alloca or a global.
fn is_alloca_or_global(
    function: &Function,
    unit: &Unit,
    ptr: Operand,
) -> bool {
    match ptr {
        Operand::Constant(id) => matches!(unit.context.get(id).kind, ConstantKind::Global(_)),
        Operand::Value(value) => match function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(inst) => {
                matches!(function.instruction(inst).opcode, Opcode::Alloca { .. })
            }
            llrm_mir::module::ValueDef::Argument(_) => false,
        },
        Operand::Block(_) => false,
    }
}

/// `findAvailablePtrLoadStore`: scan back in `scan_bb` from `scan_from` (the
/// instruction before which to start; none for the end of the block).
#[allow(clippy::too_many_arguments)]
pub fn find_available_ptr_load_store(
    unit: &Unit,
    loc: &MemoryLocation,
    access_ty: TypeId,
    scan_bb: BlockId,
    scan_from: &mut Option<InstId>,
    max_insts_to_scan: u32,
    mut aa: Option<&mut dyn ModRefOracle>,
    mut num_scanned: Option<&mut u32>,
) -> Option<Available> {
    let function = unit.function;
    let q = query(unit);
    let mut remaining = if max_insts_to_scan == 0 { u32::MAX } else { max_insts_to_scan };
    let stripped = strip_pointer_casts_same_representation(&q, loc.ptr);
    let access = Access { ty: access_ty, ptr: stripped };
    loop {
        let before = *scan_from;
        let inst = match before {
            Some(at) => function.prev_in_block(at),
            None => function.block(scan_bb).last(),
        };
        // Got to the start of the block, we didn't find it, but are done for
        // this block.
        let Some(inst) = inst else { return None };
        if let Some(count) = num_scanned.as_deref_mut() {
            *count += 1;
        }
        // Don't scan huge blocks.
        let over = remaining == 0;
        remaining = remaining.wrapping_sub(1);
        if over {
            return None;
        }
        *scan_from = Some(inst);
        if let Some(found) = get_available_load_store(unit, &q, inst, access) {
            return Some(found);
        }
        let instruction = function.instruction(inst);
        // Try to get the store size for the type.
        if let Opcode::Store { .. } = instruction.opcode {
            let store_ptr = strip_pointer_casts_same_representation(&q, instruction.operands[1]);
            // If both reach all the way to an alloca or global and they are
            // different, ignore the store.
            if is_alloca_or_global(function, unit, stripped)
                && is_alloca_or_global(function, unit, store_ptr)
                && stripped != store_ptr
            {
                continue;
            }
            let store_ty = function.operand_type(unit.context, instruction.operands[0]);
            match aa.as_deref_mut() {
                // Without alias analysis, the same base, constant offsets
                // and no overlap is a store to skip.
                None => {
                    if let Some(store_ty) = store_ty
                        && are_non_overlap_same_base_load_and_store(
                            unit,
                            &q,
                            loc.ptr,
                            access_ty,
                            instruction.operands[1],
                            store_ty,
                        )
                    {
                        continue;
                    }
                }
                // If it says the store won't modify the loaded value, ignore
                // the store.
                Some(aa) => {
                    if !aa.get_mod_ref_info(inst, loc).is_mod_set() {
                        continue;
                    }
                }
            }
            // Otherwise the store that may or may not alias the pointer, bail
            // out.
            *scan_from = before;
            return None;
        }
        // If this is some other instruction that may clobber Ptr, bail out.
        if llrm_mir::valuetracking::guarantee::may_write_to_memory(&q, inst) {
            // If alias analysis claims that it really won't modify the load,
            // ignore it.
            if let Some(aa) = aa.as_deref_mut()
                && !aa.get_mod_ref_info(inst, loc).is_mod_set()
            {
                continue;
            }
            // May modify the pointer, bail out.
            *scan_from = before;
            return None;
        }
    }
}

/// `FindAvailableLoadedValue` (the `ScanBB` overload): what `load` would read,
/// from the accesses before `scan_from` in `scan_bb`. A volatile load has
/// none.
#[allow(clippy::too_many_arguments)]
pub fn find_available_loaded_value(
    unit: &Unit,
    load: InstId,
    scan_bb: BlockId,
    scan_from: &mut Option<InstId>,
    max_insts_to_scan: u32,
    aa: Option<&mut dyn ModRefOracle>,
    num_scanned: Option<&mut u32>,
) -> Option<Available> {
    let instruction = unit.function.instruction(load);
    // Don't CSE load that is volatile or anything stronger than unordered.
    let Opcode::Load { volatile: false, .. } = instruction.opcode else { return None };
    let size = unit.layout.store_size(&unit.context.types, instruction.ty);
    let loc = MemoryLocation::new(instruction.operands[0], LocationSize::Precise(size));
    find_available_ptr_load_store(unit, &loc, instruction.ty, scan_bb, scan_from, max_insts_to_scan, aa, num_scanned)
}

/// `FindAvailableLoadedValue` (the `BatchAAResults` overload): what `load`
/// reads, from the accesses before it in its own block. An available value is
/// found first, and the alias analysis asked only then, of what lies between.
pub fn find_available_loaded_value_before(
    unit: &Unit,
    load: InstId,
    aa: &mut dyn ModRefOracle,
    max_insts_to_scan: u32,
) -> Option<Available> {
    let function = unit.function;
    let instruction = function.instruction(load);
    let Opcode::Load { volatile: false, .. } = instruction.opcode else { return None };
    let q = query(unit);
    let ptr = strip_pointer_casts_same_representation(&q, instruction.operands[0]);
    let access = Access { ty: instruction.ty, ptr };
    let mut remaining = max_insts_to_scan;
    // Try to find an available value first, and delay expensive alias
    // analysis queries until later.
    let mut must_not_alias = Vec::new();
    let mut available = None;
    let mut at = function.prev_in_block(load);
    while let Some(inst) = at {
        let over = remaining == 0;
        remaining = remaining.wrapping_sub(1);
        if over {
            return None;
        }
        available = get_available_load_store(unit, &q, inst, access);
        if available.is_some() {
            break;
        }
        if llrm_mir::valuetracking::guarantee::may_write_to_memory(&q, inst) {
            must_not_alias.push(inst);
        }
        at = function.prev_in_block(inst);
    }
    // If we found an available value, ensure that the instructions in between
    // did not modify the memory location.
    if available.is_some() {
        let size = unit.layout.store_size(&unit.context.types, instruction.ty);
        let loc = MemoryLocation::new(instruction.operands[0], LocationSize::Precise(size));
        if must_not_alias.into_iter().any(|inst| aa.get_mod_ref_info(inst, &loc).is_mod_set()) {
            return None;
        }
    }
    available
}

#[cfg(test)]
#[path = "loads_tests.rs"]
mod tests;
