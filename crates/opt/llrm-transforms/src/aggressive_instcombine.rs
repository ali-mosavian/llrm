//! `AggressiveInstCombine`: `TruncInstCombine`, then the folds of
//! `foldUnusualPatterns`, here `foldConsecutiveLoads`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `AggressiveInstCombine.cpp`, each
//! function linking the lines it translates. `run_impl` is `runImpl`
//! ([:2674](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L2674)):
//! the truncs first (`trunc_inst_combine`), then every instruction of every
//! reachable block, bottom up, is tried against the folds.
//! `fold_consecutive_loads` finds `(zext(load p) << s1) | (zext(load p+n) <<
//! s2) ...`, a byte-assembly of adjacent loads, and makes it one wider load.
//!
//! ```text
//!   before                           after
//!   %a = load i8, ptr %p             %a = load i16, ptr %p, align 1
//!   %q = gep i8, ptr %p, 1           (the or's uses take %a zext'd)
//!   %b = load i8, ptr %q
//!   %x = zext i8 %a to i16
//!   %y = zext i8 %b to i16
//!   %s = shl i16 %y, 8
//!   %r = or i16 %x, %s
//! ```
//!
//! DEVIATE (anything MIR or Rust lacks), numbered after `trunc_inst_combine`'s:
//! 38. The scan between the two loads asks `mayWriteToMemory` and no alias
//!     analysis: any instruction that may write memory between them stops the
//!     merge (the pin stops only if it may write the loaded bytes). Sound, and
//!     it merges less. Owner: w2. Exit: the `AliasQuery` of `memcpyopt`,
//!     2026-11-21.
//! 39. The target's `allowsMisalignedMemoryAccesses` is taken to be true and
//!     fast for every size the data layout has as a legal integer (the X86
//!     answer, `X86ISelLoweringCall.cpp#L356`). Owner: w2. Exit: a
//!     target-supplied fact, 2026-11-28.
//!
//! 40. The pin ends `foldUnusualPatterns` with `SimplifyInstructionsInBlock`;
//!     there is no public equivalent here, so the dead chain a fold leaves is
//!     the next `dead` pass's. Owner: w2. Exit: 2026-11-21, expose the helper.
//! 41. `foldConsecutiveStores` ends a run at any instruction that may read or
//!     write memory, or throw; the pin asks `BatchAA` whether it touches the
//!     run's base. The pass `Unit` carries no module globals, so a callee's own
//!     attributes are not seen either. Sound, merges less. Owner: w2. Exit:
//!     2026-11-21, with deviation 38's `AliasQuery`.
//! 42. `tryToRecognizeTableBasedCttz` marks the select it makes for a table
//!     whose zero entry is not the width with unlikely branch weights; MIR has
//!     no `!prof` on a select here, so none is set. Owner: w2. Exit:
//!     2026-11-21, with select weights.
//! 43. `tryToRecognizeTableBasedLog2` asks the target's `getIntrinsicInstrCost`
//!     for `ctlz`; a pass run with no `tti` has no answer and does not fold.
//!     Owner: w2. Exit: 2026-11-21.
//! 49. `tryToFPToSat` is not translated: MIR has no `llvm.fptosi.sat` (no
//!     interpreter or lowering) and no target prices it. Owner: w2. Exit:
//!     2026-11-30, when a corpus cell needs it.
//!
//! # Example
//! The pair above is one `load i16`; see `aggressive_instcombine_tests.rs`.
//!
//! # Runs
//! As `aggressive-instcombine@678`, after `instcombine` and before `tailrec`
//! (`PassBuilderPipelines.cpp#L678`); not at -O1.
//!
//! # Needs
//! The dominator tree, to leave unreachable blocks alone.
//!
//! # Keeps
//! The dominator tree and the loops: no block or edge changes.

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, IntPredicate, Opcode, Tail};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::guarantee::{may_read_from_memory, may_throw, may_write_to_memory};
use llrm_mir::valuetracking::nonzero::strip_and_accumulate_constant_offsets;
use llrm_mir::valuetracking::{Query, pattern};

use crate::trunc_inst_combine::TruncInstCombine;

/// `MaxInstrsToScan`
/// ([:62](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L62)).
const MAX_INSTRS_TO_SCAN: u32 = 64;

pub struct AggressiveInstCombine;

impl FunctionPass for AggressiveInstCombine {
    fn name(&self) -> &'static str {
        "aggressive-instcombine"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("aggressive-instcombine")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if unit.function.is_declaration() {
            return PreservedAnalyses::all();
        }
        let truncs = TruncInstCombine.run(unit, analyses);
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(&*unit.context, unit.layout, &*unit.function);
        let reachable: IdSet<BlockId> = unit
            .function
            .layout()
            .iter()
            .copied()
            .filter(|&block| shape.dominance.tree(unit.function).is_reachable_from_entry(block))
            .collect();
        drop(shape);
        let outer = std::rc::Rc::clone(analyses.outer());
        let tti = outer.target().tti();
        let info = crate::simplifylibcalls::target_library_info(&outer, unit.layout.pointer(0).index_bits);
        let mut cfg_changed = false;
        let folded = fold_unusual_patterns(unit, &reachable, &outer.globals, &info, tti.as_deref(), &mut cfg_changed);
        if truncs.are_all_preserved() && !folded {
            return PreservedAnalyses::all();
        }
        if cfg_changed {
            return PreservedAnalyses::none();
        }
        PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>().preserve::<llrm_analysis::cfg::Shape>()
    }
}

/// `foldUnusualPatterns`
/// ([:2620](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L2620)),
/// of the folds this module has.
fn fold_unusual_patterns(
    unit: &mut Unit,
    reachable: &IdSet<BlockId>,
    globals: &[llrm_mir::module::GlobalValue],
    info: &llrm_mir::libfunc::TargetLibraryInfo,
    tti: Option<&dyn llrm_mir::tti::Tti>,
    cfg_changed: &mut bool,
) -> bool {
    let mut changed = false;
    let blocks: Vec<BlockId> =
        unit.function.layout().iter().copied().filter(|&block| reachable.contains(&block)).collect();
    for block in blocks {
        // Bottom up: a chain is matched from its end, and a partial pattern
        // is not matched.
        let insts: Vec<InstId> = unit.function.iter_block(block).collect();
        for &inst in insts.iter().rev() {
            changed |=
                fold_select_split_ctlz_cttz(unit.context, unit.layout, unit.function, unit.declared, globals, inst);
            changed |= fold_any_or_all_bits_set(unit.context, unit.layout, unit.function, inst);
            changed |= fold_guarded_funnel_shift(unit.context, unit.layout, unit.function, unit.declared, inst);
            changed |= try_to_recognize_pop_count(unit.context, unit.layout, unit.function, unit.declared, inst);
            changed |= try_to_recognize_table_based_patterns(
                unit.context,
                unit.layout,
                unit.function,
                unit.declared,
                globals,
                tti,
                inst,
            );
            changed |= fold_consecutive_loads(unit.context, unit.layout, unit.function, inst);
            changed |= fold_patterned_loads(unit.context, unit.layout, unit.function, globals, inst);
            changed |= fold_icmp_or_chain(unit.context, unit.layout, unit.function, inst);
            changed |= fold_mul_high(unit.context, unit.layout, unit.function, inst);
            changed |= fold_lib_calls(unit, globals, info, tti, cfg_changed, inst);
        }
        // Done separately to scan the stores once.
        changed |= fold_consecutive_stores(unit.context, unit.layout, unit.function, block);
    }
    changed
}

/// A `Query` over `function` alone: no module, so no callee is named.
fn query<'a>(
    context: &'a llrm_mir::context::Context,
    layout: &'a DataLayout,
    function: &'a Function,
) -> Query<'a> {
    Query {
        context,
        globals: &[],
        metadata: &[],
        function,
        layout,
        dt: None,
        assumptions: None,
        dom_conditions: None,
        cond_context: None,
        cxt: None,
        pending_intrinsics: &[],
    }
}

/// `LoadOps`
/// ([:1371](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1371)),
/// less the AA tags: MIR carries none to concatenate.
#[derive(Default)]
struct LoadOps {
    root: Option<InstId>,
    root_insert: Option<InstId>,
    found_root: bool,
    load_size: u64,
    shift: u64,
    zext_type: Option<TypeId>,
}

/// A `ZExt(load)` that has one use, with the shift it is under, or `None`
/// without a shift: `m_OneUse(m_ShlOrSelf(m_OneUse(m_ZExt(m_Instruction(L))),
/// Shift))`.
fn shifted_extended_instruction(
    q: &Query,
    v: Operand,
) -> Option<(InstId, u64)> {
    let one_use = |operand: Operand| {
        matches!(
            operand,
            Operand::Value(value) if q.function.users(value).has_one_use()
        )
    };
    if !one_use(v) {
        return None;
    }
    let extended = |operand: Operand| -> Option<InstId> {
        if !one_use(operand) {
            return None;
        }
        let (_, _, source) = llrm_mir::valuetracking::pattern::cast(q, operand, CastOp::ZExt)?;
        q.instruction_of(source).map(|(inst, _)| inst)
    };
    if let Some((_, _, lhs, rhs)) = llrm_mir::valuetracking::pattern::binary(q, v, BinaryOp::Shl)
        && let Some((amount, _)) = q.int_constant(rhs)
        && let Some(load) = extended(lhs)
    {
        return Some((load, u64::try_from(amount).ok()?));
    }
    extended(v).map(|load| (load, 0))
}

/// `foldLoadsRecursive`
/// ([:1385](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1385)).
fn fold_loads_recursive(
    q: &Query,
    v: Operand,
    lops: &mut LoadOps,
    is_root: bool,
) -> bool {
    // The root may have several uses; an intermediate value needs one.
    if !is_root
        && !matches!(
            v,
            Operand::Value(value) if q.function.users(value).has_one_use()
        )
    {
        return false;
    }
    // `m_c_Or(m_Value(X), <shifted extended load>)`, either order.
    let Some((_, _, a, b)) = llrm_mir::valuetracking::pattern::binary(q, v, BinaryOp::Or) else { return false };
    let (x, l2, shift2) = if let Some((l2, shift)) = shifted_extended_instruction(q, b) {
        (a, l2, shift)
    } else if let Some((l2, shift)) = shifted_extended_instruction(q, a) {
        (b, l2, shift)
    } else {
        return false;
    };

    if !fold_loads_recursive(q, x, lops, false) && lops.found_root {
        // Avoid a partial chain merge.
        return false;
    }

    let mut li1 = lops.root;
    let mut shift1 = lops.shift;
    if !lops.found_root
        && let Some((l1, shift)) = shifted_extended_instruction(q, x)
    {
        li1 = Some(l1).filter(|&inst| matches!(q.function.instruction(inst).opcode, Opcode::Load { .. }));
        shift1 = shift;
    }
    let li2 = Some(l2).filter(|&inst| matches!(q.function.instruction(inst).opcode, Opcode::Load { .. }));
    let (Some(mut li1), Some(mut li2)) = (li1, li2) else { return false };

    // The same load twice, a volatile one, or a different block or address
    // space.
    let simple = |inst: InstId| matches!(
        q.function.instruction(inst).opcode,
        Opcode::Load { volatile: false, .. }
    );
    if li1 == li2 || !simple(li1) || !simple(li2) {
        return false;
    }
    let space =
        |inst: InstId| match q.type_of(q.function.instruction(inst).operands[0]).map(|ty| q.context.types.get(ty)) {
            Some(Type::Pointer(space)) => Some(*space),
            _ => None,
        };
    if space(li1) != space(li2) || space(li1).is_none() {
        return false;
    }
    if q.function.parent(li1) != q.function.parent(li2) {
        return false;
    }

    let big_endian = q.layout.big_endian;
    let index_bits = q.layout.pointer(space(li1).unwrap_or(0)).index_bits;
    let (mut ptr1, mut offset1) =
        strip_and_accumulate_constant_offsets(q, q.function.instruction(li1).operands[0], index_bits, true);
    let (mut ptr2, mut offset2) =
        strip_and_accumulate_constant_offsets(q, q.function.instruction(li2).operands[0], index_bits, true);
    let signed = |offset: u128| -> i128 {
        let shift = 128 - index_bits;
        ((offset << shift) as i128) >> shift
    };
    let bits_of =
        |inst: InstId| u64::from(q.scalar_bits(Operand::Value(q.function.instruction(inst).result.expect("a load"))));
    let (mut load_size1, mut load_size2) = (bits_of(li1), bits_of(li2));
    if ptr1 != ptr2 {
        return false;
    }
    // No padding bits.
    for inst in [li1, li2] {
        let ty = q.function.instruction(inst).ty;
        if q.layout.size_bits(&q.context.types, ty) != 8 * q.layout.store_size(&q.context.types, ty) {
            return false;
        }
    }

    // Stores between the loads (DEVIATE 38: any write).
    let (mut start, mut end) = (if lops.found_root { lops.root_insert.expect("a root") } else { li1 }, li2);
    if !q.function.comes_before(start, end) {
        std::mem::swap(&mut start, &mut end);
    }
    let mut scanned = 0;
    let mut here = start;
    while here != end {
        if may_write_to_memory(q, here) {
            return false;
        }
        scanned += 1;
        if scanned > MAX_INSTRS_TO_SCAN {
            return false;
        }
        let Some(next) = q.function.next_in_block(here) else { return false };
        here = next;
    }

    // The load with the lower offset is LI1.
    let (mut shift2, mut reverse) = (shift2, false);
    if signed(offset2) < signed(offset1) {
        std::mem::swap(&mut li1, &mut li2);
        std::mem::swap(&mut shift1, &mut shift2);
        std::mem::swap(&mut offset1, &mut offset2);
        std::mem::swap(&mut ptr1, &mut ptr2);
        std::mem::swap(&mut load_size1, &mut load_size2);
        reverse = true;
    }
    if big_endian {
        std::mem::swap(&mut shift1, &mut shift2);
    }
    // The merged size of the loads found so far stands for LI1 (forward) or
    // LI2 (reversed).
    if lops.found_root {
        if !reverse {
            load_size1 = lops.load_size;
        } else {
            load_size2 = lops.load_size;
        }
    }
    let shift_diff = if big_endian { load_size2 } else { load_size1 };
    let prev_size = load_size1.div_ceil(8);
    if shift2.wrapping_sub(shift1) != shift_diff || signed(offset2).wrapping_sub(signed(offset1)) != prev_size as i128 {
        return false;
    }
    // The combined size must fit the extended type.
    let Some(x_ty) = q.type_of(x) else { return false };
    let x_bits = match q.context.types.get(x_ty) {
        Type::Int(bits) => u64::from(*bits),
        _ => return false,
    };
    if load_size1 + load_size2 > x_bits {
        return false;
    }

    lops.found_root = true;
    lops.load_size = load_size1 + load_size2;
    lops.root_insert = Some(start);
    lops.root = Some(li1);
    lops.shift = shift1;
    lops.zext_type = Some(x_ty);
    true
}

/// `DT.dominates(Value, Instruction)` for an address: an argument or constant
/// always does; an instruction does when it is before `at` in its block, or
/// its block dominates `at`'s.
fn dominates_at(
    function: &Function,
    value: Operand,
    at: InstId,
) -> bool {
    let Operand::Value(id) = value else { return true };
    let llrm_mir::module::ValueDef::Instruction(def) = function.value(id).def else { return true };
    let (Some(def_block), Some(at_block)) = (function.parent(def), function.parent(at)) else { return false };
    if def_block == at_block {
        return function.comes_before(def, at);
    }
    llrm_mir::domtree::DomTree::of(function).dominates(def_block, at_block)
}

/// `foldConsecutiveLoads`
/// ([:1540](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1540)).
fn fold_consecutive_loads(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    inst: InstId,
) -> bool {
    // Scalars only.
    let Some(result) = function.instruction(inst).result else { return false };
    if !matches!(context.types.get(function.value(result).ty), Type::Int(_)) {
        return false;
    }
    let mut lops = LoadOps::default();
    {
        let q = query(context, layout, function);
        if !fold_loads_recursive(&q, Operand::Value(result), &mut lops, true) || !lops.found_root {
            return false;
        }
    }
    let (Some(li1), Some(root_insert), Some(zext_type)) = (lops.root, lops.root_insert, lops.zext_type) else {
        return false;
    };
    // A power of two number of bits, at least 16, that a legal integer holds.
    let allowed = lops.load_size >= 16
        && lops.load_size.is_power_of_two()
        && u32::try_from(lops.load_size).is_ok_and(|bits| layout.fits_in_legal_integer(bits));
    if !allowed {
        return false;
    }
    let Opcode::Load { align, volatile } = function.instruction(li1).opcode.clone() else { return false };
    let mut pointer = function.instruction(li1).operands[0];
    // The wide load sits at `root_insert`, which may be the other load: the
    // first load's address is used only if its definition dominates that
    // point, else the stripped base plus the constant offset is (the pin's
    // `CreatePtrAdd`, :1569).
    if !dominates_at(function, pointer, root_insert) {
        let space = match context.types.get(function.operand_type(context, pointer).expect("a typed pointer")) {
            Type::Pointer(space) => *space,
            _ => return false,
        };
        let index_bits = layout.pointer(space).index_bits;
        let (base, offset) =
            strip_and_accumulate_constant_offsets(&query(context, layout, function), pointer, index_bits, true);
        let shift = 128 - index_bits;
        let offset = ((offset << shift) as i128) >> shift;
        let i8 = context.types.int(8);
        let index_ty = context.types.int(index_bits);
        let index = Operand::Constant(context.int(index_ty, offset));
        let ptr_ty = function.operand_type(context, pointer).expect("a typed pointer");
        let add = function.create_instruction(
            Opcode::GetElementPtr { source: i8 },
            ptr_ty,
            vec![base, index],
            Flags::default(),
            None,
        );
        function.insert(add, Position::Before(root_insert)).expect("a placed instruction");
        pointer = Operand::Value(function.instruction(add).result.expect("a value"));
    }
    let wider = context.types.int(lops.load_size as u32);
    let new_load =
        function.create_instruction(Opcode::Load { align, volatile }, wider, vec![pointer], Flags::default(), None);
    function.insert(new_load, Position::Before(root_insert)).expect("a placed instruction");
    let mut value = Operand::Value(function.instruction(new_load).result.expect("a value"));
    if context.types.int_bits(zext_type) != Some(lops.load_size as u32) {
        let extended =
            function.create_instruction(Opcode::Cast(CastOp::ZExt), zext_type, vec![value], Flags::default(), None);
        function.insert(extended, Position::Before(root_insert)).expect("a placed instruction");
        value = Operand::Value(function.instruction(extended).result.expect("a value"));
    }
    if lops.shift != 0 {
        let amount = Operand::Constant(context.int(zext_type, i128::from(lops.shift)));
        let shifted = function.create_instruction(
            Opcode::Binary(BinaryOp::Shl),
            zext_type,
            vec![value, amount],
            Flags::default(),
            None,
        );
        function.insert(shifted, Position::Before(root_insert)).expect("a placed instruction");
        value = Operand::Value(function.instruction(shifted).result.expect("a value"));
    }
    function.replace_all_uses_with(result, value);
    true
}

/// `PartStore`
/// ([:1597](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1597)):
/// `width` bits starting at `val_offset` of `val`, stored at `base + offset`.
struct PartStore {
    base: Operand,
    offset: i128,
    val: Operand,
    val_offset: u64,
    width: u64,
    store: InstId,
    space: u32,
}

impl PartStore {
    /// `isCompatibleWith`.
    fn is_compatible_with(
        &self,
        other: &Self,
    ) -> bool {
        self.base == other.base && self.val == other.val && self.space == other.space
    }
}

/// `matchPartStore`
/// ([:1618](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1618)).
fn match_part_store(
    q: &Query,
    inst: InstId,
) -> Option<PartStore> {
    let instruction = q.function.instruction(inst);
    let Opcode::Store { volatile: false, .. } = instruction.opcode else { return None };
    let stored = instruction.operands[0];
    let pointer = instruction.operands[1];
    let ty = q.type_of(stored)?;
    if !matches!(q.context.types.get(ty), Type::Int(_))
        || q.layout.size_bits(&q.context.types, ty) != 8 * q.layout.store_size(&q.context.types, ty)
    {
        return None;
    }
    let width = u64::from(q.scalar_bits(stored));
    // `m_Trunc(m_LShrOrSelf(m_Value(Val), ValOffset))`
    let (_, _, source) = llrm_mir::valuetracking::pattern::cast(q, stored, CastOp::Trunc)?;
    let (val, val_offset) = match llrm_mir::valuetracking::pattern::binary(q, source, BinaryOp::LShr) {
        Some((_, _, lhs, rhs)) => match q.int_constant(rhs) {
            Some((amount, _)) => (lhs, u64::try_from(amount).ok()?),
            None => (source, 0),
        },
        None => (source, 0),
    };
    let Some(Type::Pointer(space)) = q.type_of(pointer).map(|ty| q.context.types.get(ty).clone()) else { return None };
    let index_bits = q.layout.pointer(space).index_bits;
    let (base, offset) = strip_and_accumulate_constant_offsets(q, pointer, index_bits, true);
    let shift = 128 - index_bits;
    Some(PartStore { base, offset: ((offset << shift) as i128) >> shift, val, val_offset, width, store: inst, space })
}

/// `mergeConsecutivePartStores`
/// ([:1644](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1644)):
/// whether `parts` become one store of `width` bits, and the store it
/// replaces them with.
fn merge_consecutive_part_stores(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    parts: &[PartStore],
    width: u64,
) -> bool {
    if parts.len() < 2 {
        return false;
    }
    // Profitable? (DEVIATE 39: a misaligned access of a legal width is fast.)
    let allowed = width >= 16
        && width.is_power_of_two()
        && u32::try_from(width).is_ok_and(|bits| layout.fits_in_legal_integer(bits));
    if !allowed {
        return false;
    }
    let first = &parts[0];
    let Opcode::Store { align, .. } = function.instruction(first.store).opcode.clone() else { return false };
    let pointer = function.instruction(first.store).operands[1];
    let new_ty = context.types.int(width as u32);
    let void = function.instruction(first.store).ty;
    let mut value = first.val;
    let value_ty = query(context, layout, function).type_of(value).expect("a typed value");
    let value_bits = u64::from(context.types.int_bits(value_ty).unwrap_or(0));
    let at = Position::Before(first.store);
    if first.val_offset != 0 {
        let amount = Operand::Constant(context.int(value_ty, i128::from(first.val_offset)));
        let shifted = function.create_instruction(
            Opcode::Binary(BinaryOp::LShr),
            value_ty,
            vec![value, amount],
            Flags::default(),
            None,
        );
        function.insert(shifted, at).expect("a placed instruction");
        value = Operand::Value(function.instruction(shifted).result.expect("a value"));
    }
    // `CreateZExtOrTrunc`
    if value_bits != width {
        let op = if value_bits < width { CastOp::ZExt } else { CastOp::Trunc };
        let cast = function.create_instruction(Opcode::Cast(op), new_ty, vec![value], Flags::default(), None);
        function.insert(cast, at).expect("a placed instruction");
        value = Operand::Value(function.instruction(cast).result.expect("a value"));
    }
    let store = function.create_instruction(
        Opcode::Store { align, volatile: false },
        void,
        vec![value, pointer],
        Flags::default(),
        None,
    );
    function.insert(store, at).expect("a placed instruction");
    for part in parts {
        function.erase(part.store).expect("a store nothing reads");
    }
    true
}

/// `mergePartStores`
/// ([:1696](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1696)).
fn merge_part_stores(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    mut parts: Vec<PartStore>,
) -> bool {
    if parts.len() < 2 {
        return false;
    }
    // Sorted by pointer offset, consistent with the value offsets, and
    // covering the value with no overlap.
    parts.sort_by_key(|part| part.offset);
    let mut changed = false;
    let mut last_end_from_first: i128 = 0;
    let mut first = 0;
    for at in 0..parts.len() {
        let part = &parts[at];
        let ptr_from_first = part.offset - parts[first].offset;
        let val_from_first = i128::from(part.val_offset) - i128::from(parts[first].val_offset);
        if ptr_from_first * 8 != val_from_first || last_end_from_first != val_from_first {
            changed |=
                merge_consecutive_part_stores(context, layout, function, &parts[first..at], last_end_from_first as u64);
            first = at;
            last_end_from_first = i128::from(part.width);
            continue;
        }
        last_end_from_first = val_from_first + i128::from(part.width);
    }
    changed |= merge_consecutive_part_stores(context, layout, function, &parts[first..], last_end_from_first as u64);
    changed
}

/// `foldConsecutiveStores`
/// ([:1762](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1762)):
/// stores of the parts of one value to adjacent bytes become one wider store.
fn fold_consecutive_stores(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    block: BlockId,
) -> bool {
    // FIXME (pin): big endian.
    if layout.big_endian {
        return false;
    }
    // The runs are found over the block as it is, then merged: a merge only
    // erases stores already passed and adds one before them.
    let mut runs: Vec<Vec<PartStore>> = Vec::new();
    {
        let q = query(context, layout, function);
        let mut parts: Vec<PartStore> = Vec::new();
        for inst in function.iter_block(block) {
            if let Some(part) = match_part_store(&q, inst) {
                if parts.is_empty() || part.is_compatible_with(&parts[0]) {
                    parts.push(part);
                    continue;
                }
                runs.push(std::mem::take(&mut parts));
                parts.push(part);
                continue;
            }
            if parts.is_empty() {
                continue;
            }
            // DEVIATE 41.
            if may_throw(&q, inst) || may_read_from_memory(&q, inst) || may_write_to_memory(&q, inst) {
                runs.push(std::mem::take(&mut parts));
            }
        }
        runs.push(parts);
    }
    let mut changed = false;
    for run in runs {
        changed |= merge_part_stores(context, layout, function, run);
    }
    changed
}

/// `m_c_Add`-style: both orders of the operands of `v`, an `op`.
fn operand_pairs(
    q: &Query,
    v: Operand,
    op: BinaryOp,
) -> Vec<(Operand, Operand)> {
    match llrm_mir::valuetracking::pattern::binary(q, v, op) {
        Some((_, _, a, b)) => vec![(a, b), (b, a)],
        None => Vec::new(),
    }
}

/// `m_And(X, m_SpecificInt(mask))`: `X`.
fn and_with(
    q: &Query,
    v: Operand,
    mask: u128,
) -> Option<Operand> {
    let (_, _, x, m) = llrm_mir::valuetracking::pattern::binary(q, v, BinaryOp::And)?;
    q.int_constant(m).is_some_and(|(bits, _)| bits == mask).then_some(x)
}

/// `m_LShr(X, m_SpecificInt(amount))`: `X`.
fn lshr_by(
    q: &Query,
    v: Operand,
    amount: u128,
) -> Option<Operand> {
    let (_, _, x, a) = llrm_mir::valuetracking::pattern::binary(q, v, BinaryOp::LShr)?;
    q.int_constant(a).is_some_and(|(bits, _)| bits == amount).then_some(x)
}

/// `APInt::getSplat(len, APInt(8, byte))`.
fn splat(
    len: u32,
    byte: u8,
) -> u128 {
    (0..len / 8).fold(0u128, |all, at| all | (u128::from(byte) << (8 * at)))
}

/// `matchPopCountBytes`
/// ([:492](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L492)):
/// the root of the per-byte population count steps that end at `v`.
fn match_pop_count_bytes(
    q: &Query,
    v: Operand,
    len: u32,
) -> Option<Operand> {
    let (mask55, mask33, mask0f) = (splat(len, 0x55), splat(len, 0x33), splat(len, 0x0f));
    // "((x + (x >> 4)) & 0x0F...)"
    let sum = and_with(q, v, mask0f)?;
    let add2 = operand_pairs(q, sum, BinaryOp::Add)
        .into_iter()
        .find_map(|(shifted, same)| lshr_by(q, shifted, 4).filter(|&x| x == same))?;
    // "x = (x & 0x33..) + ((x >> 2) & 0x33..)" or "x = x - 3*((x >> 2) &
    // 0x33..)"
    let sub1 = operand_pairs(q, add2, BinaryOp::Add)
        .into_iter()
        .find_map(|(shifted, plain)| {
            let x = lshr_by(q, and_with(q, shifted, mask33)?, 2)?;
            (and_with(q, plain, mask33)? == x).then_some(x)
        })
        .or_else(|| {
            let (_, _, times, sub1) = llrm_mir::valuetracking::pattern::binary(q, add2, BinaryOp::Add)?;
            let (_, _, anded, minus_three) = llrm_mir::valuetracking::pattern::binary(q, times, BinaryOp::Mul)?;
            let minus_three_bits = splat_neg(len, 3);
            q.int_constant(minus_three).is_some_and(|(bits, _)| bits == minus_three_bits).then_some(())?;
            let x = lshr_by(q, and_with(q, anded, mask33)?, 2)?;
            (x == sub1).then_some(x)
        })?;
    // "x - ((x >> 1) & 0x55...)"
    let (_, _, root, and) = llrm_mir::valuetracking::pattern::binary(q, sub1, BinaryOp::Sub)?;
    let (_, _, shifted, and_mask) = llrm_mir::valuetracking::pattern::binary(q, and, BinaryOp::And)?;
    if lshr_by(q, shifted, 1)? != root {
        return None;
    }
    let (and_mask, _) = q.int_constant(and_mask)?;
    if and_mask != mask55 {
        // A narrowed mask is accepted if the missing bits are known zero in
        // `Root >> 1`.
        if and_mask & !mask55 != 0 {
            return None;
        }
        let needed = mask55 & !and_mask;
        if !llrm_mir::valuetracking::known::masked_value_is_zero(q, shifted, needed, 0) {
            return None;
        }
    }
    Some(root)
}

/// `-n` as a `len`-bit constant.
fn splat_neg(
    len: u32,
    n: u128,
) -> u128 {
    let mask = if len >= 128 { u128::MAX } else { (1u128 << len) - 1 };
    n.wrapping_neg() & mask
}

/// `replaceWithPopCount`
/// ([:470](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L470)),
/// of a root of the instruction's own type: a call of `llvm.ctpop.iN`, its
/// declaration made through `Unit::declared`.
fn replace_with_pop_count(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    inst: InstId,
    root: Operand,
) {
    let ty = function.instruction(inst).ty;
    let width = context.types.int_bits(ty).expect("an integer");
    let function_type = context.types.intern(Type::Function { returns: ty, parameters: vec![ty], variadic: false });
    let callee = declared.declare(&format!("llvm.ctpop.i{width}"), function_type);
    let pointer = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new()],
        attrs: Vec::new(),
        tail: Tail::default(),
    };
    let call =
        function.create_instruction(Opcode::Call(Box::new(info)), ty, vec![root, callee], Flags::default(), None);
    function.insert(call, Position::Before(inst)).expect("a placed instruction");
    let result = function.instruction(inst).result.expect("a value");
    function.replace_all_uses_with(result, Operand::Value(function.instruction(call).result.expect("a value")));
}

/// `matchFunnelShift`
/// ([:243](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L243)):
/// `v` as `fshl` (`true`) or `fshr` of `(shval0, shval1, amount)`.
fn match_funnel_shift(
    function: &Function,
    q: &Query,
    v: Operand,
    width: u128,
) -> Option<(bool, Operand, Operand, Operand)> {
    use llrm_mir::valuetracking::pattern;
    let Operand::Value(result) = v else { return None };
    let (_, _, a, b) = pattern::binary(q, v, BinaryOp::Or)?;
    if !function.users(result).has_one_use() {
        return None;
    }
    let width_minus = |sub: Operand, amount: Operand| {
        pattern::binary(q, sub, BinaryOp::Sub)
            .is_some_and(|(_, _, w, x)| q.int_constant(w).is_some_and(|(bits, _)| bits == width) && x == amount)
    };
    // `m_c_Or`: the shifts in either order.
    for (left, right) in [(a, b), (b, a)] {
        let (Some((_, _, shl_value, shl_amount)), Some((_, _, lshr_value, lshr_amount))) =
            (pattern::binary(q, left, BinaryOp::Shl), pattern::binary(q, right, BinaryOp::LShr))
        else {
            continue;
        };
        // fshl(v0, v1, n) == (v0 << n) | (v1 >> (width - n))
        if width_minus(lshr_amount, shl_amount) {
            return Some((true, shl_value, lshr_value, shl_amount));
        }
        // fshr(v0, v1, n) == (v1 >> n) | (v0 << (width - n))
        if width_minus(shl_amount, lshr_amount) {
            return Some((false, shl_value, lshr_value, lshr_amount));
        }
    }
    None
}

/// `foldGuardedFunnelShift`
/// ([:236](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L236)):
/// a phi of a funnel shift and its source, the branch around the shift when
/// the amount is zero, is `llvm.fshl`/`llvm.fshr`.
fn fold_guarded_funnel_shift(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    inst: InstId,
) -> bool {
    use llrm_mir::valuetracking::pattern;
    if function.instruction(inst).opcode != Opcode::Phi || function.instruction(inst).operands.len() != 4 {
        return false;
    }
    let ty = function.instruction(inst).ty;
    let Some(bits) = context.types.int_bits(ty) else { return false };
    // Not strictly necessary, but careful of targets with no funnel shift.
    if !bits.is_power_of_two() {
        return false;
    }
    let arms = llrm_analysis::occurrence::arms(function, inst);
    let (found, phi_block) = {
        let q = query(context, layout, function);
        let width = u128::from(bits);
        let mut found = None;
        // One arm is the funnel shift, the other its source.
        for (funnel, guard) in [(0usize, 1usize), (1, 0)] {
            let Some((is_fshl, shval0, shval1, amount)) = match_funnel_shift(function, &q, arms[funnel].0, width)
            else {
                continue;
            };
            let source = if is_fshl { shval0 } else { shval1 };
            if source == arms[guard].0 {
                found = Some((funnel, guard, is_fshl, shval0, shval1, amount));
                break;
            }
        }
        (found, function.parent(inst))
    };
    let (Some((funnel, guard, is_fshl, mut shval0, mut shval1, amount)), Some(phi_block)) = (found, phi_block) else {
        return false;
    };
    let guard_block = arms[guard].1;
    let funnel_block = arms[funnel].1;
    let Some(term) = function.terminator(guard_block) else { return false };
    // The shifted values dominate the guard's branch.
    if !dominates_at(function, shval0, term) || !dominates_at(function, shval1, term) {
        return false;
    }
    // `br (icmp eq amount, 0), PhiBB, FunnelBB`.
    {
        let q = query(context, layout, function);
        let branch = function.instruction(term);
        let (Opcode::Br, [cond, Operand::Block(then), Operand::Block(otherwise)]) =
            (&branch.opcode, &branch.operands[..])
        else {
            return false;
        };
        let Some((_, _, pred, lhs, rhs)) = pattern::icmp(&q, *cond) else { return false };
        if pred.pred != llrm_mir::opcode::IntPredicate::Eq
            || lhs != amount
            || !q.int_constant(rhs).is_some_and(|(bits, _)| bits == 0)
            || *then != phi_block
            || *otherwise != funnel_block
        {
            return false;
        }
    }
    // The first non-phi instruction of the phi's block.
    let Some(insert) = function.iter_block(phi_block).find(|&one| function.instruction(one).opcode != Opcode::Phi)
    else {
        return false;
    };
    // The select blocked poison from the shift-by-zero arm; a funnel shift
    // does not, so freeze that value unless this is a rotate.
    if shval0 != shval1 {
        let source = if is_fshl { &mut shval1 } else { &mut shval0 };
        let poison_free = {
            let q = query(context, layout, function);
            llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_poison(&q, *source, 0)
        };
        if !poison_free {
            let freeze = function.create_instruction(Opcode::Freeze, ty, vec![*source], Flags::default(), None);
            function.insert(freeze, Position::Before(insert)).expect("a placed instruction");
            *source = Operand::Value(function.instruction(freeze).result.expect("a value"));
        }
    }
    let name = if is_fshl { "fshl" } else { "fshr" };
    let value = call_intrinsic_before(
        context,
        function,
        declared,
        insert,
        &format!("llvm.{name}.i{bits}"),
        ty,
        vec![shval0, shval1, amount],
        vec![ty, ty, ty],
    );
    let result = function.instruction(inst).result.expect("a value");
    function.replace_all_uses_with(result, value);
    true
}

/// `optimizeShiftInOrChain`
/// ([:1762](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1762)):
/// the chain of one-use ors `v` with the `shl nsw`/`shl nuw` operands taken
/// out, built before `before`; none where nothing changes. A shifted value is
/// zero exactly when the value is, so the chain compared with zero is the
/// same.
fn optimize_shift_in_or_chain(
    context: &llrm_mir::context::Context,
    function: &mut Function,
    v: Operand,
    before: InstId,
) -> Option<Operand> {
    let Operand::Value(result) = v else { return None };
    let llrm_mir::module::ValueDef::Instruction(def) = function.value(result).def else { return None };
    let instruction = function.instruction(def);
    if instruction.opcode != Opcode::Binary(BinaryOp::Or) || !function.users(result).has_one_use() {
        return None;
    }
    let ty = instruction.ty;
    let operands = [instruction.operands[0], instruction.operands[1]];
    let mut changed = false;
    let mut new = operands;
    for (at, &op) in operands.iter().enumerate() {
        let shifted = match op {
            Operand::Value(value) => match function.value(value).def {
                llrm_mir::module::ValueDef::Instruction(shl) => {
                    let one = function.instruction(shl);
                    (one.opcode == Opcode::Binary(BinaryOp::Shl)
                        && (one.flags.contains(Flags::NSW) || one.flags.contains(Flags::NUW)))
                    .then(|| one.operands[0])
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(inner) = shifted {
            new[at] = inner;
            changed = true;
        } else if let Some(deeper) = optimize_shift_in_or_chain(context, function, op, before) {
            new[at] = deeper;
            changed = true;
        }
    }
    if !changed {
        return None;
    }
    let or = function.create_instruction(Opcode::Binary(BinaryOp::Or), ty, new.to_vec(), Flags::default(), None);
    function.insert(or, Position::Before(before)).expect("a placed instruction");
    Some(Operand::Value(function.instruction(or).result.expect("a value")))
}

/// `foldICmpOrChain`
/// ([:1798](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1798)):
/// `icmp eq/ne (or ...), 0`: loads in the chain are merged first, then the
/// no-wrap shifts are dropped.
fn fold_icmp_or_chain(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    inst: InstId,
) -> bool {
    use llrm_mir::valuetracking::pattern;
    let (op0, zero, equality) = {
        let q = query(context, layout, function);
        let Some(result) = function.instruction(inst).result else { return false };
        let Some((_, _, pred, op0, zero)) = pattern::icmp(&q, Operand::Value(result)) else { return false };
        if !q.int_constant(zero).is_some_and(|(bits, _)| bits == 0)
            || !matches!(
                pred.pred,
                llrm_mir::opcode::IntPredicate::Eq | llrm_mir::opcode::IntPredicate::Ne
            )
        {
            return false;
        }
        (op0, zero, pred.pred)
    };
    // If the chain of ors matches loads, combine to that before removing
    // shifts.
    if let Operand::Value(value) = op0
        && let llrm_mir::module::ValueDef::Instruction(def) = function.value(value).def
        && function.instruction(def).opcode == Opcode::Binary(BinaryOp::Or)
        && fold_consecutive_loads(context, layout, function, def)
    {
        return true;
    }
    let Some(chain) = optimize_shift_in_or_chain(context, function, op0, inst) else { return false };
    let i1 = function.instruction(inst).ty;
    let cmp = function.create_instruction(Opcode::ICmp(equality), i1, vec![chain, zero], Flags::default(), None);
    function.insert(cmp, Position::Before(inst)).expect("a placed instruction");
    let result = function.instruction(inst).result.expect("a value");
    function.replace_all_uses_with(result, Operand::Value(function.instruction(cmp).result.expect("a value")));
    true
}

/// The matcher of `foldMulHigh`'s lambdas: what the pin's `PatternMatch`
/// forms ask of the operands, over the function as it stands.
struct MulHigh<'a> {
    function: &'a Function,
    q: Query<'a>,
    half: u32,
    mask: u128,
}

impl MulHigh<'_> {
    fn bin(
        &self,
        v: Operand,
        op: BinaryOp,
    ) -> Option<(Operand, Operand)> {
        llrm_mir::valuetracking::pattern::binary(&self.q, v, op).map(|(_, _, a, b)| (a, b))
    }

    /// `m_c_<op>(L, R)`: the first order `f` takes.
    fn c_bin<T>(
        &self,
        v: Operand,
        op: BinaryOp,
        f: impl Fn(Operand, Operand) -> Option<T>,
    ) -> Option<T> {
        let (a, b) = self.bin(v, op)?;
        f(a, b).or_else(|| f(b, a))
    }

    fn is_inst(
        &self,
        v: Operand,
    ) -> bool {
        self.q.instruction_of(v).is_some()
    }

    fn one_use(
        &self,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Value(value) if self.function.users(value).has_one_use()
        )
    }

    /// `hasNUsesOrMore(3)`.
    fn three_uses(
        &self,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Value(value) if self.function.users(value).has_n_uses_or_more(3)
        )
    }

    fn is_const(
        &self,
        v: Operand,
        value: u128,
    ) -> bool {
        self.q.int_constant(v).is_some_and(|(bits, _)| bits == value)
    }

    /// `m_LShr(m_Value(x), m_SpecificInt(BitWidth / 2))`: `x`.
    fn lshr_half(
        &self,
        v: Operand,
    ) -> Option<Operand> {
        let (value, amount) = self.bin(v, BinaryOp::LShr)?;
        self.is_const(amount, u128::from(self.half)).then_some(value)
    }

    /// `m_And(m_Value(x), m_SpecificInt(LowMask))`: `x`.
    fn and_mask(
        &self,
        v: Operand,
    ) -> Option<Operand> {
        let (value, mask) = self.bin(v, BinaryOp::And)?;
        self.is_const(mask, self.mask).then_some(value)
    }

    /// `CheckLoLo`: `xlyl` is `(x & mask) * (y & mask)`.
    fn check_lo_lo(
        &self,
        xlyl: Operand,
        x: Operand,
        y: Operand,
    ) -> bool {
        self.c_bin(xlyl, BinaryOp::Mul, |a, b| {
            (self.and_mask(a) == Some(x) && self.and_mask(b) == Some(y)).then_some(())
        })
        .is_some()
    }

    /// `CheckHiLo`: `xhyl` is `(x >> half) * (y & mask)`.
    fn check_hi_lo(
        &self,
        xhyl: Operand,
        x: Operand,
        y: Operand,
    ) -> bool {
        self.c_bin(xhyl, BinaryOp::Mul, |a, b| {
            (self.lshr_half(a) == Some(x) && self.and_mask(b) == Some(y)).then_some(())
        })
        .is_some()
    }

    /// `HiHi`: a one-use `(x >> half) * (y >> half)`.
    fn hi_hi(
        &self,
        v: Operand,
    ) -> Option<(Operand, Operand)> {
        if !self.one_use(v) {
            return None;
        }
        let (a, b) = self.bin(v, BinaryOp::Mul)?;
        Some((self.lshr_half(a)?, self.lshr_half(b)?))
    }

    /// A one-use `select (icmp ult s, t), 1 << half, 0`: `(s, t)`.
    fn carry_select(
        &self,
        v: Operand,
    ) -> Option<(Operand, Operand)> {
        if !self.one_use(v) {
            return None;
        }
        let (_, select) = self.q.instruction_of(v)?;
        let (Opcode::Select, [cond, then, otherwise]) = (&select.opcode, &select.operands[..]) else { return None };
        if !self.one_use(*cond) || !self.is_const(*then, 1u128 << self.half) || !self.is_const(*otherwise, 0) {
            return None;
        }
        let (_, _, pred, lhs, rhs) = llrm_mir::valuetracking::pattern::icmp(&self.q, *cond)?;
        (pred.pred == llrm_mir::opcode::IntPredicate::Ult).then_some((lhs, rhs))
    }

    fn is_select(
        &self,
        v: Operand,
    ) -> bool {
        self.q.instruction_of(v).is_some_and(|(_, one)| one.opcode == Opcode::Select)
    }

    /// `FoldMulHighCarry`: `xh*yh + carry + lowsum >> half`, the carry a
    /// select on `lowsum < xh*yl`.
    fn carry(
        &self,
        mut x: Operand,
        mut y: Operand,
        mut carry: Operand,
        mut b: Operand,
    ) -> Option<(Operand, Operand)> {
        if !self.is_select(carry) {
            std::mem::swap(&mut carry, &mut b);
        }
        let (low_sum, xh_yl) = self.carry_select(carry)?;
        // `xh_yl` can be `xh*yl` or `xl*yh`.
        if !self.check_hi_lo(xh_yl, x, y) {
            if self.check_hi_lo(xh_yl, y, x) {
                std::mem::swap(&mut x, &mut y);
            } else {
                return None;
            }
        }
        if self.three_uses(xh_yl) {
            return None;
        }
        // `b` is `lowsum >> half`.
        if !self.one_use(b) || self.lshr_half(b) != Some(low_sum) || self.three_uses(low_sum) {
            return None;
        }
        // `lowsum = xh*yl + xl*yh + (xl*yl >> half)`, in any grouping.
        let parts = |xl_yh: Operand, shifted: Operand| -> Option<(Operand, Operand)> {
            Some((xl_yh, self.lshr_half(shifted)?))
        };
        let (xl_yh, xl_yl) = self
            .c_bin(low_sum, BinaryOp::Add, |a, rest| {
                if a != xh_yl || !self.one_use(rest) {
                    return None;
                }
                self.c_bin(rest, BinaryOp::Add, |c, d| if self.one_use(c) { parts(c, d) } else { None })
            })
            .or_else(|| {
                self.c_bin(low_sum, BinaryOp::Add, |a, rest| {
                    if !self.one_use(a) || !self.one_use(rest) {
                        return None;
                    }
                    self.c_bin(rest, BinaryOp::Add, |c, d| if c == xh_yl { parts(a, d) } else { None })
                })
            })
            .or_else(|| {
                self.c_bin(low_sum, BinaryOp::Add, |shifted, rest| {
                    let xl_yl = self.lshr_half(shifted)?;
                    if !self.one_use(rest) {
                        return None;
                    }
                    self.c_bin(rest, BinaryOp::Add, |c, d| (c == xh_yl && self.one_use(d)).then_some((d, xl_yl)))
                })
            })?;
        if !self.check_lo_lo(xl_yl, x, y) || !self.check_hi_lo(xl_yh, y, x) {
            return None;
        }
        Some((x, y))
    }

    /// `FoldMulHighLadder`: `xh*yh + c2 >> half + c3 >> half`.
    fn ladder(
        &self,
        x: Operand,
        y: Operand,
        a: Operand,
        b: Operand,
    ) -> Option<(Operand, Operand)> {
        let (mut c2, mut c3) = (self.lshr_half(a)?, self.lshr_half(b)?);
        // `m_c_Add(m_Add(), m_Value())`.
        let add_of_add =
            |v: Operand| self.c_bin(v, BinaryOp::Add, |l, _| self.bin(l, BinaryOp::Add).map(|_| ())).is_some();
        if add_of_add(c3) {
            std::mem::swap(&mut c2, &mut c3);
        }
        // `c2 = (xl*yh & mask) + xh*yl + (xl*yl >> half)`, `c3 = xh*yl`.
        let first = |c2: Operand, c3: Operand| -> Option<(Operand, Operand)> {
            let and_c3 = |v: Operand| self.and_mask(v) == Some(c3);
            // (and + XlYh) + lshr(XlYl)
            let form1 = self.c_bin(c2, BinaryOp::Add, |inner, shifted| {
                let xl_yl = self.lshr_half(shifted)?;
                self.c_bin(inner, BinaryOp::Add, |l, r| and_c3(l).then_some((r, xl_yl)))
            });
            // (and + lshr(XlYl)) + XlYh
            let form2 =
                || {
                    self.c_bin(c2, BinaryOp::Add, |inner, xl_yh| {
                        self.c_bin(inner, BinaryOp::Add, |l, r| {
                            if and_c3(l) { Some((xl_yh, self.lshr_half(r)?)) } else { None }
                        })
                    })
                };
            // (lshr(XlYl) + XlYh) + and
            let form3 = || {
                self.c_bin(c2, BinaryOp::Add, |inner, and| {
                    if !and_c3(and) {
                        return None;
                    }
                    self.c_bin(inner, BinaryOp::Add, |l, r| Some((r, self.lshr_half(l)?)))
                })
            };
            form1.or_else(form2).or_else(form3)
        };
        let (xl_yh, xh_yl, xl_yl);
        if let Some((found_yh, found_yl)) = first(c2, c3) {
            (xl_yh, xh_yl, xl_yl) = (found_yh, c3, found_yl);
        } else {
            // `c3 = c2 & mask + xl*yh`.
            let c3_form = |c2: Operand, c3: Operand| {
                self.c_bin(c3, BinaryOp::Add, |l, r| (self.and_mask(l) == Some(c2)).then_some(r))
            };
            if c3_form(c2, c3).is_none() {
                std::mem::swap(&mut c2, &mut c3);
            }
            let found_yh =
                self.c_bin(c3, BinaryOp::Add, |l, r| (self.one_use(l) && self.and_mask(l) == Some(c2)).then_some(r))?;
            if !self.one_use(c3) || self.three_uses(c2) {
                return None;
            }
            // `c2 = xh*yl + (xl*yl >> half)`.
            let (found_yl, found_hl) =
                self.c_bin(c2, BinaryOp::Add, |l, r| self.lshr_half(l).map(|inner| (inner, r)))?;
            (xl_yh, xh_yl, xl_yl) = (found_yh, found_hl, found_yl);
        }
        // `xl_yh` and `xh_yl` can be either way round.
        let (mut xl_yh, mut xh_yl) = (xl_yh, xh_yl);
        if !self.check_hi_lo(xl_yh, y, x) {
            std::mem::swap(&mut xl_yh, &mut xh_yl);
        }
        if !self.check_hi_lo(xl_yh, y, x) || !self.check_hi_lo(xh_yl, x, y) || !self.check_lo_lo(xl_yl, x, y) {
            return None;
        }
        Some((x, y))
    }

    /// The first two shapes of the root: `xh*yh` and two more terms.
    fn two_terms(
        &self,
        root: Operand,
    ) -> Option<(Operand, Operand, Operand, Operand)> {
        let first = self.c_bin(root, BinaryOp::Add, |p, rest| {
            let (x, y) = self.hi_hi(p)?;
            if !self.one_use(rest) {
                return None;
            }
            let (a, b) = self.bin(rest, BinaryOp::Add)?;
            (self.is_inst(a) && self.is_inst(b)).then_some((x, y, a, b))
        });
        let (x, y, a, b) = first.or_else(|| {
            self.c_bin(root, BinaryOp::Add, |a, rest| {
                if !self.is_inst(a) || !self.one_use(rest) {
                    return None;
                }
                self.c_bin(rest, BinaryOp::Add, |p, b| {
                    let (x, y) = self.hi_hi(p)?;
                    self.is_inst(b).then_some((x, y, a, b))
                })
            })
        })?;
        (self.one_use(a) && self.one_use(b)).then_some((x, y, a, b))
    }
}

/// `foldMulHigh`
/// ([:2264](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L2264)):
/// the high half of a long multiply built from half-width parts is a
/// widening multiply, a shift and a truncate.
///
/// DEVIATE 44: the `Carry4` and `Ladder4` forms (four terms after `xh*yh`) are
/// not translated. Owner: w2. Exit: 2026-11-21.
fn fold_mul_high(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let ty = function.instruction(inst).ty;
    let Some(bits) = context.types.int_bits(ty) else { return false };
    let Some(result) = function.instruction(inst).result else { return false };
    if bits % 2 != 0 || bits < 4 {
        return false;
    }
    let found = {
        let matcher =
            MulHigh { function, q: query(context, layout, function), half: bits / 2, mask: low_mask(bits / 2) };
        let Some((x, y, a, b)) = matcher.two_terms(Operand::Value(result)) else { return false };
        matcher.carry(x, y, a, b).or_else(|| matcher.ladder(x, y, a, b))
    };
    let Some((x, y)) = found else { return false };
    let wide = context.types.int(bits * 2);
    let mut build = |function: &mut Function, op: Opcode, ty: TypeId, operands: Vec<Operand>, flags: Flags| {
        let one = function.create_instruction(op, ty, operands, flags, None);
        function.insert(one, Position::Before(inst)).expect("a placed instruction");
        Operand::Value(function.instruction(one).result.expect("a value"))
    };
    let x_wide = build(function, Opcode::Cast(CastOp::ZExt), wide, vec![x], Flags::default());
    let y_wide = build(function, Opcode::Cast(CastOp::ZExt), wide, vec![y], Flags::default());
    let product = build(function, Opcode::Binary(BinaryOp::Mul), wide, vec![x_wide, y_wide], Flags::NUW);
    let amount = Operand::Constant(context.int(wide, i128::from(bits)));
    let high = build(function, Opcode::Binary(BinaryOp::LShr), wide, vec![product, amount], Flags::default());
    let narrow = build(function, Opcode::Cast(CastOp::Trunc), ty, vec![high], Flags::NUW);
    function.replace_all_uses_with(result, narrow);
    true
}

/// `foldSqrt`
/// ([:855](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L855)):
/// a `sqrt` libcall whose argument cannot be negative or NaN, on a target with
/// a sqrt instruction, is `llvm.sqrt`: it sets no `errno`.
///
/// DEVIATE 45: the query has no dominator tree or assumption cache, so a fact
/// that only an `llvm.assume` or a dominating branch states is not seen.
/// Owner: w2. Exit: 2026-11-21.
fn fold_sqrt(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    tti: Option<&dyn llrm_mir::tti::Tti>,
    inst: InstId,
) -> bool {
    use llrm_mir::types::FloatKind;
    let Some(tti) = tti else { return false };
    let ty = function.instruction(inst).ty;
    let (suffix, tti_ty) = match context.types.get(ty) {
        Type::Float(FloatKind::Float) => ("f32", llrm_mir::tti::Ty::Float),
        Type::Float(FloatKind::Double) => ("f64", llrm_mir::tti::Ty::Double),
        Type::Float(FloatKind::X86Fp80) => ("f80", llrm_mir::tti::Ty::X87),
        _ => return false,
    };
    let Some(result) = function.instruction(inst).result else { return false };
    let flags = function.instruction(inst).flags;
    let argument = function.instruction(inst).operands[0];
    if !tti.have_fast_sqrt(tti_ty) {
        return false;
    }
    let never_negative = flags.contains(Flags::NNAN) || {
        let q = query(context, layout, function);
        llrm_mir::valuetracking::fp::compute_known_fp_class(
            &q,
            argument,
            llrm_mir::knownfpclass::KnownFpClass::ORDERED_LESS_THAN_ZERO_MASK,
            0,
        )
        .cannot_be_ordered_less_than_zero()
    };
    if !never_negative {
        return false;
    }
    let name = format!("llvm.sqrt.{suffix}");
    // `CreateIntrinsic(..., Call)` copies the call's fast-math flags.
    let value =
        call_intrinsic_with_flags_before(context, function, declared, inst, &name, ty, vec![argument], vec![ty], flags);
    function.replace_all_uses_with(result, value);
    function.erase(inst).expect("a call nothing uses");
    true
}

/// `foldLibCalls`
/// ([:2192](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L2192)).
///
/// DEVIATE 46: `memchr`, `strcmp` and `strncmp` read a pointer that is a
/// constant global itself, not a constant offset into one, and their
/// branches have no weights (as DEVIATE 42). Owner: w2. Exit: 2026-11-21.
fn fold_lib_calls(
    unit: &mut Unit,
    globals: &[llrm_mir::module::GlobalValue],
    info: &llrm_mir::libfunc::TargetLibraryInfo,
    tti: Option<&dyn llrm_mir::tti::Tti>,
    cfg_changed: &mut bool,
    inst: InstId,
) -> bool {
    use llrm_mir::libfunc::LibFunc;
    if !matches!(unit.function.instruction(inst).opcode, Opcode::Call(_)) {
        return false;
    }
    let Some(global) = llrm_mir::memory::callee(unit.context, unit.function, inst) else { return false };
    let Some(func) = globals.get(global.0 as usize).and_then(|one| info.get_lib_func_of(&unit.context.types, one))
    else {
        return false;
    };
    // `isLibFuncEmittable`.
    if !info.has(func) {
        return false;
    }
    match func {
        LibFunc::Sqrt | LibFunc::Sqrtf | LibFunc::Sqrtl => {
            fold_sqrt(unit.context, unit.layout, unit.function, unit.declared, tti, inst)
        }
        LibFunc::Strcmp | LibFunc::Strncmp => {
            let folded = fold_str_n_cmp(unit.context, unit.layout, unit.function, globals, func, inst);
            *cfg_changed |= folded;
            folded
        }
        LibFunc::Memchr => {
            let folded = fold_mem_chr(unit.context, unit.layout, unit.function, globals, inst);
            *cfg_changed |= folded;
            folded
        }
        _ => false,
    }
}

/// `StrNCmpInlineThreshold`.
const STR_N_CMP_INLINE_THRESHOLD: u64 = 3;

/// `StrNCmpInliner::optimizeStrNCmp` and `inlineCompare`
/// ([:1970](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1970),
/// [:2040](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L2040)):
/// a `strcmp` or `strncmp` against a constant string of two or three bytes,
/// used only in comparisons with zero, is a chain of byte differences that
/// stops at the first one that is not zero.
fn fold_str_n_cmp(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    globals: &[llrm_mir::module::GlobalValue],
    func: llrm_mir::libfunc::LibFunc,
    inst: InstId,
) -> bool {
    use llrm_mir::libfunc::LibFunc;
    let operands = function.instruction(inst).operands.clone();
    let (first, second, limit) = match (func, &operands[..]) {
        (LibFunc::Strcmp, [first, second, _callee]) => (*first, *second, None),
        (LibFunc::Strncmp, [first, second, limit, _callee]) => (*first, *second, Some(*limit)),
        _ => return false,
    };
    let q = query(context, layout, function);
    if !llrm_mir::valuetracking::known::is_only_used_in_zero_comparison(&q, inst) || first == second {
        return false;
    }
    let env = llrm_mir::constantfolding::Env { globals, layout };
    let known_first = crate::simplifylibcalls::constant_array_in(context, env, first);
    let known_second = crate::simplifylibcalls::constant_array_in(context, env, second);
    if known_first.is_some() == known_second.is_some() {
        return false;
    }
    // The NUL and what follows it are not trimmed.
    let swapped = known_first.is_some();
    let (string, other) =
        if let Some(string) = known_first { (string, second) } else { (known_second.expect("one is known"), first) };
    let mut n = string.iter().position(|&byte| byte == 0).map_or(u128::MAX, |at| at as u128 + 1);
    if let Some(limit) = limit {
        let Some((limit, _)) = q.int_constant(limit) else { return false };
        n = n.min(limit);
    }
    if n > string.len() as u128 || n < 2 || n > u128::from(STR_N_CMP_INLINE_THRESHOLD) {
        return false;
    }
    // A pointer with two or more dereferenceable bytes is better served
    // elsewhere.
    if llrm_mir::valuetracking::known::pointer_dereferenceable_bytes(&q, other).0 > 1 {
        return false;
    }
    let n = n as usize;
    let call_ty = function.instruction(inst).ty;
    let Some(result) = function.instruction(inst).result else { return false };
    let Some(block) = function.parent(inst) else { return false };
    let i8 = context.types.int(8);
    let index_ty = context.types.int(layout.pointer(0).index_bits);
    let void = context.types.void();

    let tail = crate::edges::split_block_at(function, block, inst);
    let jump = function.terminator(block).expect("the split's branch");
    function.erase(jump).expect("a terminator defines nothing");
    let mut previous = block;
    let mut subs = Vec::new();
    for _ in 0..n {
        let sub = function.create_block(Some("sub"));
        function.insert_block(sub, Some(previous)).expect("a new block");
        previous = sub;
        subs.push(sub);
    }
    let ne = function.create_block(Some("ne"));
    function.insert_block(ne, Some(previous)).expect("a new block");

    let enter = function.create_instruction(Opcode::Br, void, vec![Operand::Block(subs[0])], Flags::default(), None);
    function.insert(enter, Position::End(block)).expect("a placed block");

    let mut incoming = Vec::new();
    for (at, &sub) in subs.iter().enumerate() {
        let mut build = |function: &mut Function, op: Opcode, ty, operands: Vec<Operand>, flags| {
            let one = function.create_instruction(op, ty, operands, flags, None);
            function.insert(one, Position::End(sub)).expect("a placed block");
            function.instruction(one).result.map(Operand::Value)
        };
        let offset = Operand::Constant(context.int(index_ty, at as i128));
        let place = build(
            function,
            Opcode::GetElementPtr { source: i8 },
            function.operand_type(context, other).expect("a typed pointer"),
            vec![other, offset],
            Flags::INBOUNDS,
        )
        .expect("a value");
        let byte = build(function, Opcode::Load { align: None, volatile: false }, i8, vec![place], Flags::default())
            .expect("a value");
        let ours = build(function, Opcode::Cast(CastOp::ZExt), call_ty, vec![byte], Flags::default()).expect("a value");
        let theirs = Operand::Constant(context.int(call_ty, i128::from(string[at])));
        let operands = if swapped { vec![theirs, ours] } else { vec![ours, theirs] };
        let difference =
            build(function, Opcode::Binary(BinaryOp::Sub), call_ty, operands, Flags::default()).expect("a value");
        if at + 1 < n {
            let zero = Operand::Constant(context.int(call_ty, 0));
            let i1 = context.types.int(1);
            let differs = build(
                function,
                Opcode::ICmp(llrm_mir::opcode::IntPredicate::Ne),
                i1,
                vec![difference, zero],
                Flags::default(),
            )
            .expect("a value");
            build(
                function,
                Opcode::Br,
                void,
                vec![differs, Operand::Block(ne), Operand::Block(subs[at + 1])],
                Flags::default(),
            );
        } else {
            build(function, Opcode::Br, void, vec![Operand::Block(ne)], Flags::default());
        }
        incoming.push((difference, sub));
    }
    let difference = function.create_instruction(
        Opcode::Phi,
        call_ty,
        llrm_analysis::occurrence::from_arms(&incoming),
        Flags::default(),
        None,
    );
    function.insert(difference, Position::End(ne)).expect("a placed block");
    let difference = Operand::Value(function.instruction(difference).result.expect("a value"));
    let out = function.create_instruction(Opcode::Br, void, vec![Operand::Block(tail)], Flags::default(), None);
    function.insert(out, Position::End(ne)).expect("a placed block");
    function.replace_all_uses_with(result, difference);
    function.erase(inst).expect("a call nothing uses");
    true
}

/// `MemChrInlineThreshold`.
const MEM_CHR_INLINE_THRESHOLD: u64 = 3;

/// `foldMemChr`
/// ([:2113](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L2113)):
/// `memchr` of a short constant string with a variable byte is a switch on the
/// byte, to the index of its first occurrence.
fn fold_mem_chr(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    globals: &[llrm_mir::module::GlobalValue],
    inst: InstId,
) -> bool {
    let operands = function.instruction(inst).operands.clone();
    let [base, byte, length, _callee] = operands[..] else { return false };
    if matches!(byte, Operand::Constant(_)) {
        return false;
    }
    let env = llrm_mir::constantfolding::Env { globals, layout };
    let Some(string) = crate::simplifylibcalls::constant_array_in(context, env, base) else { return false };
    let Some((count, _)) = query(context, layout, function).int_constant(length) else { return false };
    // A length past the string is left to the library.
    if count > string.len() as u128 || count > u128::from(MEM_CHR_INLINE_THRESHOLD) {
        return false;
    }
    let n = count as usize;
    let call_ty = function.instruction(inst).ty;
    let Some(result) = function.instruction(inst).result else { return false };
    let Some(block) = function.parent(inst) else { return false };
    let i8 = context.types.int(8);
    let index_ty = context.types.int(layout.pointer(0).index_bits);
    let void = context.types.void();

    let next = crate::edges::split_block_at(function, block, inst);
    let jump = function.terminator(block).expect("the split's branch");
    function.erase(jump).expect("a terminator defines nothing");
    let narrow = function.create_instruction(Opcode::Cast(CastOp::Trunc), i8, vec![byte], Flags::default(), None);
    function.insert(narrow, Position::End(block)).expect("a placed block");
    let narrow = Operand::Value(function.instruction(narrow).result.expect("a value"));

    let success = function.create_block(Some("memchr.success"));
    let mut cases: Vec<(u8, BlockId)> = Vec::new();
    let mut incoming: Vec<(Operand, BlockId)> = Vec::new();
    let mut previous = block;
    for (at, &value) in string[..n].iter().enumerate() {
        if cases.iter().any(|&(seen, _)| seen == value) {
            continue;
        }
        let case = function.create_block(Some("memchr.case"));
        function.insert_block(case, Some(previous)).expect("a new block");
        previous = case;
        let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(success)], Flags::default(), None);
        function.insert(jump, Position::End(case)).expect("a placed block");
        incoming.push((Operand::Constant(context.int(index_ty, at as i128)), case));
        cases.push((value, case));
    }
    function.insert_block(success, Some(previous)).expect("a new block");

    let mut switch = vec![narrow, Operand::Block(next)];
    for &(value, case) in &cases {
        switch.extend([Operand::Constant(context.int(i8, i128::from(value))), Operand::Block(case)]);
    }
    let switch = function.create_instruction(Opcode::Switch, void, switch, Flags::default(), None);
    function.insert(switch, Position::End(block)).expect("a placed block");

    let index = function.create_instruction(
        Opcode::Phi,
        index_ty,
        llrm_analysis::occurrence::from_arms(&incoming),
        Flags::default(),
        Some("memchr.idx"),
    );
    function.insert(index, Position::End(success)).expect("a placed block");
    let index = Operand::Value(function.instruction(index).result.expect("a value"));
    let place = function.create_instruction(
        Opcode::GetElementPtr { source: i8 },
        call_ty,
        vec![base, index],
        Flags::INBOUNDS,
        None,
    );
    function.insert(place, Position::End(success)).expect("a placed block");
    let place = Operand::Value(function.instruction(place).result.expect("a value"));
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(next)], Flags::default(), None);
    function.insert(jump, Position::End(success)).expect("a placed block");

    let null = Operand::Constant(context.constant(Constant { ty: call_ty, kind: ConstantKind::Null }));
    let found = function.create_instruction(
        Opcode::Phi,
        call_ty,
        llrm_analysis::occurrence::from_arms(&[(null, block), (place, success)]),
        Flags::default(),
        None,
    );
    function.insert(found, Position::Before(inst)).expect("a placed instruction");
    let found = Operand::Value(function.instruction(found).result.expect("a value"));
    function.replace_all_uses_with(result, found);
    function.erase(inst).expect("a call nothing uses");
    true
}

/// `MaskOps`: the source and the bit indexes of a chain of shifted bits.
struct MaskOps {
    root: Option<Operand>,
    mask: u128,
    bits: u32,
    match_and_chain: bool,
    found_and_1: bool,
}

/// `matchAndOrChain`
/// ([:373](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L373)).
fn match_and_or_chain(
    q: &Query,
    v: Operand,
    ops: &mut MaskOps,
) -> bool {
    let binary = |op: BinaryOp| pattern::binary(q, v, op).map(|(_, _, a, b)| (a, b));
    if ops.match_and_chain {
        if let Some((first, one)) = binary(BinaryOp::And)
            && q.int_constant(one).is_some_and(|(bits, _)| bits == 1)
        {
            ops.found_and_1 = true;
            return match_and_or_chain(q, first, ops);
        }
        if let Some((a, b)) = binary(BinaryOp::And) {
            return match_and_or_chain(q, a, ops) && match_and_or_chain(q, b, ops);
        }
    } else if let Some((a, b)) = binary(BinaryOp::Or) {
        return match_and_or_chain(q, a, ops) && match_and_or_chain(q, b, ops);
    }
    // A shift right, or a bare value standing for bit 0 of the source.
    let shifted = binary(BinaryOp::LShr).and_then(|(value, amount)| Some((value, q.int_constant(amount)?.0)));
    let (candidate, index) = match shifted {
        Some((value, index)) => (value, Some(index)),
        None => (v, None),
    };
    let root = *ops.root.get_or_insert(candidate);
    // A shift out of range was not simplified.
    if index.is_some_and(|index| index >= u128::from(ops.bits)) {
        return false;
    }
    ops.mask |= 1u128 << index.unwrap_or(0);
    root == candidate
}

/// `foldAnyOrAllBitsSet`
/// ([:420](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L420)):
/// `and (or (lshr X, C), ...), 1` is `(X & mask) != 0`, and the `and` chain
/// is `(X & mask) == mask`.
///
/// DEVIATE 47: integers of up to 128 bits; no vectors. Owner: w2. Exit:
/// 2026-11-21.
fn fold_any_or_all_bits_set(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let ty = function.instruction(inst).ty;
    let Some(result) = function.instruction(inst).result else { return false };
    let (x, all_bits_set, trunc) = {
        let q = query(context, layout, function);
        let one_use = |v: Operand| {
            matches!(
                v,
                Operand::Value(value) if function.users(value).has_one_use()
            )
        };
        let one_use_binary = |v: Operand, op: BinaryOp| one_use(v) && pattern::binary(&q, v, op).is_some();
        let this = function.instruction(inst);
        if context.types.int_bits(ty) == Some(1) {
            let Opcode::Cast(CastOp::Trunc) = this.opcode else { return false };
            let source = this.operands[0];
            if one_use_binary(source, BinaryOp::And) {
                (source, true, true)
            } else if one_use_binary(source, BinaryOp::Or) {
                (source, false, true)
            } else {
                return false;
            }
        } else {
            let Opcode::Binary(BinaryOp::And) = this.opcode else { return false };
            let [a, b, ..] = this.operands[..] else { return false };
            if one_use_binary(a, BinaryOp::And) || one_use_binary(b, BinaryOp::And) {
                (Operand::Value(result), true, false)
            } else if one_use_binary(a, BinaryOp::Or) && q.int_constant(b).is_some_and(|(bits, _)| bits == 1) {
                (a, false, false)
            } else {
                return false;
            }
        }
    };
    let Some(x_ty) = function.operand_type(context, x) else { return false };
    let Some(bits) = context.types.int_bits(x_ty) else { return false };
    if bits > 128 {
        return false;
    }
    let mut ops = MaskOps { root: None, mask: 0, bits, match_and_chain: all_bits_set, found_and_1: false };
    let matched = match_and_or_chain(&query(context, layout, function), x, &mut ops);
    if !matched || (all_bits_set && !trunc && !ops.found_and_1) {
        return false;
    }
    let root = ops.root.expect("a chain has a root");
    let mask = Operand::Constant(context.int(x_ty, ops.mask as i128));
    let mut build = |function: &mut Function, op: Opcode, ty: TypeId, operands: Vec<Operand>| {
        let one = function.create_instruction(op, ty, operands, Flags::default(), None);
        function.insert(one, Position::Before(inst)).expect("a placed instruction");
        Operand::Value(function.instruction(one).result.expect("a value"))
    };
    let and = build(function, Opcode::Binary(BinaryOp::And), x_ty, vec![root, mask]);
    let i1 = context.types.int(1);
    let compare = if all_bits_set {
        build(function, Opcode::ICmp(llrm_mir::opcode::IntPredicate::Eq), i1, vec![and, mask])
    } else {
        let zero = Operand::Constant(context.int(x_ty, 0));
        build(function, Opcode::ICmp(llrm_mir::opcode::IntPredicate::Ne), i1, vec![and, zero])
    };
    let replacement = if trunc { compare } else { build(function, Opcode::Cast(CastOp::ZExt), x_ty, vec![compare]) };
    function.replace_all_uses_with(result, replacement);
    true
}

/// `foldSelectSplitCTLZCTTZ`
/// ([:204](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L204)),
/// with `foldSelectSplitCTTZ` (:95) and `foldSelectSplitCTLZ` (:152): the
/// trailing or leading zeros of a value counted in halves, the second half
/// only if the first is zero, are one full-width count.
fn fold_select_split_ctlz_cttz(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    globals: &[llrm_mir::module::GlobalValue],
    inst: InstId,
) -> bool {
    let ty = function.instruction(inst).ty;
    let Some(result) = function.instruction(inst).result else { return false };
    let Some(half) = context.types.int_bits(ty) else { return false };
    // The full-width count can exceed what the half type holds.
    if half <= 2 {
        return false;
    }
    let full = half * 2;
    let found = {
        // Calls are named through the globals.
        let q = Query { globals, ..query(context, layout, function) };
        let Some((_, _, cond, mut true_value, mut false_value)) = pattern::select(&q, Operand::Value(result)) else {
            return false;
        };
        let Some((_, _, pred, compared, zero)) = pattern::icmp(&q, cond) else { return false };
        if !q.int_constant(zero).is_some_and(|(bits, _)| bits == 0)
            || !matches!(pred.pred, IntPredicate::Eq | IntPredicate::Ne)
        {
            return false;
        }
        if pred.pred == IntPredicate::Ne {
            std::mem::swap(&mut true_value, &mut false_value);
        }
        let one_use = |v: Operand| {
            matches!(
                v,
                Operand::Value(value) if function.users(value).has_one_use()
            )
        };
        let is_const = |v: Operand, want: u128| q.int_constant(v).is_some_and(|(bits, _)| bits == want);
        let of_width = |v: Operand, bits: u32| {
            function.operand_type(context, v).and_then(|t| context.types.int_bits(t)) == Some(bits)
        };
        // `m_Trunc(m_LShr(m_Specific(src), m_SpecificInt(half)))`.
        let high_half = |v: Operand, src: Operand| {
            pattern::cast(&q, v, CastOp::Trunc)
                .and_then(|(_, _, shifted)| pattern::binary(&q, shifted, BinaryOp::LShr))
                .is_some_and(|(_, _, value, amount)| value == src && is_const(amount, u128::from(half)))
        };
        // `m_OneUse(m_AddLike(m_OneUse(m_Intrinsic(base)),
        // m_SpecificInt(half)))`: the count's argument.
        let plus_half = |v: Operand, base: &str| {
            if !one_use(v) {
                return None;
            }
            let (count, amount) = pattern::add_like(&q, v, Flags::default())?;
            if !is_const(amount, u128::from(half)) || !one_use(count) {
                return None;
            }
            let (_, args) = pattern::intrinsic_call(&q, count, base)?;
            args.first().copied()
        };
        let counted = |v: Operand, base: &str| {
            if !one_use(v) {
                return None;
            }
            let (_, args) = pattern::intrinsic_call(&q, v, base)?;
            args.first().copied()
        };
        // cttz: `lo == 0 ? cttz(hi) + half : cttz(lo)`.
        let cttz = (|| {
            let (_, _, src) = pattern::cast(&q, compared, CastOp::Trunc)?;
            if !of_width(src, full) || counted(false_value, "llvm.cttz") != Some(compared) {
                return None;
            }
            let hi_arg = plus_half(true_value, "llvm.cttz")?;
            high_half(hi_arg, src).then_some((src, "cttz"))
        })();
        // ctlz: `hi == 0 ? ctlz(lo) + half : ctlz(hi)`.
        let ctlz = || {
            let hi_part = match pattern::cast(&q, compared, CastOp::Trunc) {
                Some((_, _, src)) => src,
                None => compared,
            };
            let (_, _, src, amount) = pattern::binary(&q, hi_part, BinaryOp::LShr)?;
            if !of_width(src, full) || !is_const(amount, u128::from(half)) {
                return None;
            }
            let hi_arg = counted(false_value, "llvm.ctlz")?;
            if !high_half(hi_arg, src) {
                return None;
            }
            let lo_arg = plus_half(true_value, "llvm.ctlz")?;
            pattern::cast(&q, lo_arg, CastOp::Trunc).is_some_and(|(_, _, value)| value == src).then_some((src, "ctlz"))
        };
        cttz.or_else(ctlz)
    };
    let Some((src, count)) = found else { return false };
    let wide = function.operand_type(context, src).expect("a typed value");
    let i1 = context.types.int(1);
    let no_poison = Operand::Constant(context.int(i1, 0));
    let wide_count = call_intrinsic_before(
        context,
        function,
        declared,
        inst,
        &format!("llvm.{count}.i{full}"),
        wide,
        vec![src, no_poison],
        vec![wide, i1],
    );
    let narrow = function.create_instruction(Opcode::Cast(CastOp::Trunc), ty, vec![wide_count], Flags::default(), None);
    function.insert(narrow, Position::Before(inst)).expect("a placed instruction");
    let narrow = Operand::Value(function.instruction(narrow).result.expect("a value"));
    function.replace_all_uses_with(result, narrow);
    true
}

/// `getStrideAndModOffsetOfGEP`
/// ([:1823](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1823)):
/// the smallest step of the variable indices of the GEPs under `pointer`, and
/// the constant offset modulo it; `(1, 0)` where it does not reach a global.
fn stride_and_mod_offset(
    q: &Query,
    layout: &DataLayout,
    mut pointer: Operand,
) -> (u128, u128) {
    let index_bits = layout.pointer(0).index_bits;
    let mask = low_mask(index_bits);
    let mut stride: Option<u128> = None;
    let mut mod_offset: u128 = 0;
    while let Some((_, gep)) = q.instruction_of(pointer)
        && let Opcode::GetElementPtr { source } = gep.opcode
    {
        let indices: Vec<Option<i128>> =
            gep.operands[1..].iter().map(|&index| q.int_constant(index).map(|(bits, _)| bits as i128)).collect();
        let (constant, variable) = layout.collect_offset(&q.context().types, source, &indices);
        mod_offset = mod_offset.wrapping_add(constant as u128) & mask;
        for (_, scale) in variable {
            let mut scale = u128::from(scale) & mask;
            if scale == 0 {
                continue;
            }
            // Only a power of two factor is kept without `nusw`.
            if !gep.flags.contains(Flags::NUSW) && !gep.flags.contains(Flags::INBOUNDS) {
                scale = 1u128 << scale.trailing_zeros();
            }
            stride = Some(stride.map_or(scale, |stride| gcd(stride, scale)));
        }
        pointer = gep.operands[0];
    }
    let reaches_global = matches!(
        pointer,
        Operand::Constant(id) if matches!(q.context().get(id).kind, ConstantKind::Global(_))
    );
    let Some(stride) = stride.filter(|_| reaches_global) else { return (1, 0) };
    // A negative offset is the remainder of a division by the stride.
    let negative = mod_offset >> (index_bits - 1) & 1 == 1;
    let signed = if negative { (mod_offset | !mask) as i128 } else { mod_offset as i128 };
    let remainder = signed.rem_euclid(stride as i128) as u128;
    (stride, remainder)
}

fn gcd(
    mut a: u128,
    mut b: u128,
) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `foldPatternedLoads`
/// ([:1868](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1868)):
/// a load from a constant array whose every reachable element is the same is
/// that constant.
///
/// DEVIATE 48: a GEP that is a constant expression is not looked through;
/// only GEP instructions are. Owner: w2. Exit: 2026-11-21.
fn fold_patterned_loads(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    globals: &[llrm_mir::module::GlobalValue],
    inst: InstId,
) -> bool {
    let Opcode::Load { align, volatile: false } = function.instruction(inst).opcode else { return false };
    let load_ty = function.instruction(inst).ty;
    let Some(result) = function.instruction(inst).result else { return false };
    let pointer = function.instruction(inst).operands[0];
    let q = Query { globals, ..query(context, layout, function) };
    let Operand::Constant(object) = llrm_mir::valuetracking::underlying::get_underlying_object(&q, pointer, 6) else {
        return false;
    };
    let ConstantKind::Global(table) = context.get(object).kind else { return false };
    let Some(global) = globals.get(table.0 as usize) else { return false };
    let llrm_mir::module::GlobalKind::Variable(variable) = &global.kind else { return false };
    // A definitive initializer: the global is not interposable.
    let (true, Some(initializer), false) = (variable.constant, variable.initializer, global.linkage.is_interposable())
    else {
        return false;
    };
    // Bail for large initializers in excess of 4K to avoid too many scans.
    let size = layout.alloc_size(&context.types, context.get(initializer).ty);
    if size == 0 || size > 4096 {
        return false;
    }
    let (mut stride, mut offset) = stride_and_mod_offset(&q, layout, pointer);
    // Any valid offset is a multiple of the load's alignment, so checking the
    // multiples of the bigger of it and the stride is enough.
    let load_align = align.unwrap_or_else(|| layout.align(&context.types, load_ty));
    if load_align <= variable.align.unwrap_or(1) && u64::try_from(stride).unwrap_or(u64::MAX) < load_align {
        offset = 0;
        stride = u128::from(load_align);
    }
    let fold = |at: u128| {
        llrm_mir::constantfolding::constant_fold_load_from_const(context, layout, initializer, load_ty, at as u64)
    };
    let Some(constant) = fold(offset) else { return false };
    let last = size.wrapping_sub(layout.store_size(&context.types, load_ty));
    while offset <= u128::from(last) {
        if fold(offset) != Some(constant) {
            return false;
        }
        offset += stride;
    }
    function.replace_all_uses_with(result, Operand::Constant(constant));
    true
}

/// `tryToRecognizePopCount`
/// ([:551](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L551)):
/// `(i * 0x01010101) >> 24` of the parallel bit count.
fn try_to_recognize_pop_count(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    inst: InstId,
) -> bool {
    let root = {
        let q = query(context, layout, function);
        let Some(result) = function.instruction(inst).result else { return false };
        let value = Operand::Value(result);
        let Some((_, _, op0, op1)) = llrm_mir::valuetracking::pattern::binary(&q, value, BinaryOp::LShr) else {
            return false;
        };
        let Some(Type::Int(len)) = q.type_of(value).map(|ty| q.context.types.get(ty).clone()) else { return false };
        // Len == 8 is `tryToRecognizePopCount2n3`'s.
        if len > 128 || len <= 8 || len % 8 != 0 {
            return false;
        }
        let Some((_, _, mul_op0, mask01)) = llrm_mir::valuetracking::pattern::binary(&q, op0, BinaryOp::Mul) else {
            return false;
        };
        if !q.int_constant(mask01).is_some_and(|(bits, _)| bits == splat(len, 0x01))
            || !q.int_constant(op1).is_some_and(|(bits, _)| bits == u128::from(len - 8))
        {
            return false;
        }
        let Some(root) = match_pop_count_bytes(&q, mul_op0, len) else { return false };
        root
    };
    replace_with_pop_count(context, function, declared, inst, root);
    true
}

/// `isCTTZTable`
/// ([:887](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L887)):
/// every one-bit input, through the multiply, the shift and the mask, indexes
/// the table at its own bit position.
#[allow(clippy::too_many_arguments)]
fn is_cttz_table(
    context: &llrm_mir::context::Context,
    layout: &DataLayout,
    table: llrm_mir::context::ConstantId,
    mul: u128,
    shift: u128,
    and_mask: u128,
    access: TypeId,
    input_bits: u32,
    scale: u128,
) -> bool {
    let all = if input_bits == 128 { u128::MAX } else { (1u128 << input_bits) - 1 };
    (0..input_bits).all(|at| {
        let index = ((1u128 << at).wrapping_mul(mul) & all).checked_shr(shift as u32).unwrap_or(0) & and_mask;
        let offset = index.wrapping_mul(scale) & all;
        llrm_mir::constantfolding::constant_fold_load_from_const(context, layout, table, access, offset as u64)
            .and_then(|folded| match context.get(folded).kind {
                ConstantKind::Int(bits) => Some(bits),
                _ => None,
            })
            == Some(u128::from(at))
    })
}

/// A load from a constant global table at one variable index: what the three
/// table recognizers share (`tryToRecognizeTableBasedPatterns`
/// ([:1329](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1329))).
struct TableLoad {
    result: llrm_mir::module::ValueId,
    access: TypeId,
    initializer: llrm_mir::context::ConstantId,
    index: Operand,
    /// The index as the GEP has it, before `m_CastOrSelf`.
    raw_index: Operand,
    scale: u128,
    inbounds: bool,
    /// The global's value type.
    table_type: TypeId,
}

fn match_table_load(
    context: &llrm_mir::context::Context,
    layout: &DataLayout,
    function: &Function,
    globals: &[llrm_mir::module::GlobalValue],
    inst: InstId,
) -> Option<TableLoad> {
    use llrm_mir::module::GlobalKind;
    let q = query(context, layout, function);
    let instruction = function.instruction(inst);
    let (Opcode::Load { volatile: false, .. }, Some(result)) = (&instruction.opcode, instruction.result) else {
        return None;
    };
    let access = instruction.ty;
    context.types.int_bits(access)?;
    let (_, gep) = q.instruction_of(instruction.operands[0])?;
    let Opcode::GetElementPtr { source } = gep.opcode else { return None };
    if !gep.flags.contains(Flags::NUSW) && !gep.flags.contains(Flags::INBOUNDS) {
        return None;
    }
    let Operand::Constant(base) = gep.operands[0] else { return None };
    let ConstantKind::Global(table) = context.get(base).kind else { return None };
    let GlobalKind::Variable(variable) = &globals.get(table.0 as usize)?.kind else { return None };
    let (true, Some(initializer)) = (variable.constant, variable.initializer) else { return None };
    let indices: Vec<Option<i128>> =
        gep.operands[1..].iter().map(|&index| q.int_constant(index).map(|(bits, _)| bits as i128)).collect();
    let (constant, variable_indices) = layout.collect_offset(&context.types, source, &indices);
    let [(at, scale)] = variable_indices[..] else { return None };
    if constant != 0 {
        return None;
    }
    // `m_CastOrSelf`: the index may be extended or truncated.
    let raw_index = gep.operands[1 + at];
    let mut index = raw_index;
    if let Some((_, cast)) = q.instruction_of(index)
        && matches!(
            cast.opcode,
            Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
        )
    {
        index = cast.operands[0];
    }
    Some(TableLoad {
        result,
        access,
        initializer,
        index,
        raw_index,
        scale: u128::from(scale),
        inbounds: gep.flags.contains(Flags::INBOUNDS),
        table_type: variable.ty,
    })
}

fn low_mask(bits: u32) -> u128 {
    if bits == 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `isLog2Table`
/// ([:1098](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1098)):
/// the run of `at + 1` low ones indexes the table at `at`, and zero at 0.
fn is_log2_table(
    context: &llrm_mir::context::Context,
    layout: &DataLayout,
    table: llrm_mir::context::ConstantId,
    mul: u128,
    shift: u128,
    access: TypeId,
    input_bits: u32,
    scale: u128,
) -> bool {
    let all = low_mask(input_bits);
    let found = (0..input_bits).all(|at| {
        let index = (low_mask(at + 1).wrapping_mul(mul) & all).checked_shr(shift as u32).unwrap_or(0);
        let offset = index.wrapping_mul(scale) & all;
        llrm_mir::constantfolding::constant_fold_load_from_const(context, layout, table, access, offset as u64)
            .and_then(|folded| match context.get(folded).kind {
                ConstantKind::Int(bits) => Some(bits),
                _ => None,
            })
            == Some(u128::from(at))
    });
    // An input of zero selects index 0.
    found && (mul & all).checked_shr(shift as u32).unwrap_or(0) == 0
}

/// Call `name(args...)` with `ret` before `inst`; its value.
fn call_intrinsic_before(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    inst: InstId,
    name: &str,
    ret: TypeId,
    args: Vec<Operand>,
    parameters: Vec<TypeId>,
) -> Operand {
    call_intrinsic_with_flags_before(context, function, declared, inst, name, ret, args, parameters, Flags::default())
}

/// `call_intrinsic_before` with the call's fast-math `flags`.
fn call_intrinsic_with_flags_before(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    inst: InstId,
    name: &str,
    ret: TypeId,
    mut args: Vec<Operand>,
    parameters: Vec<TypeId>,
    flags: Flags,
) -> Operand {
    let function_type = context.types.intern(Type::Function { returns: ret, parameters, variadic: false });
    let callee = declared.declare(name, function_type);
    let pointer = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new(); args.len()],
        attrs: Vec::new(),
        tail: Tail::default(),
    };
    args.push(callee);
    let call = function.create_instruction(Opcode::Call(Box::new(info)), ret, args, flags, None);
    function.insert(call, Position::Before(inst)).expect("a placed instruction");
    Operand::Value(function.instruction(call).result.expect("a value"))
}

/// `CreateZExtOrTrunc` before `inst`.
fn zext_or_trunc_before(
    context: &llrm_mir::context::Context,
    function: &mut Function,
    inst: InstId,
    value: Operand,
    from_bits: u32,
    to: TypeId,
) -> Operand {
    let to_bits = context.types.int_bits(to).expect("an integer");
    if to_bits == from_bits {
        return value;
    }
    let op = if to_bits > from_bits { CastOp::ZExt } else { CastOp::Trunc };
    let cast = function.create_instruction(Opcode::Cast(op), to, vec![value], Flags::default(), None);
    function.insert(cast, Position::Before(inst)).expect("a placed instruction");
    Operand::Value(function.instruction(cast).result.expect("a value"))
}

/// `tryToRecognizeTableBasedCttz`
/// ([:966](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L966)):
/// a load from a constant table at `((x & -x) * mul) >> shift` is `cttz x`.
fn try_to_recognize_table_based_cttz(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    inst: InstId,
    load: &TableLoad,
) -> bool {
    use llrm_mir::valuetracking::pattern;
    let (x, x_ty, input_bits, zero_elem, defined_for_zero) = {
        let q = query(context, layout, function);
        // `((x & -x) * mul) >> shift`, or that masked.
        let mut and_mask = None;
        let mut lshr = load.index;
        if let Some((_, _, a, b)) = pattern::binary(&q, load.index, BinaryOp::And)
            && let Some((bits, _)) = q.int_constant(b)
            && pattern::binary(&q, a, BinaryOp::LShr).is_some()
        {
            and_mask = Some(bits);
            lshr = a;
        }
        let Some((_, _, product, shift)) = pattern::binary(&q, lshr, BinaryOp::LShr) else { return false };
        let Some((shift, _)) = q.int_constant(shift) else { return false };
        let Some((_, _, isolate, mul)) = pattern::binary(&q, product, BinaryOp::Mul) else { return false };
        let Some((mul, _)) = q.int_constant(mul) else { return false };
        // `x & -x`, in either order.
        let Some((_, _, left, right)) = pattern::binary(&q, isolate, BinaryOp::And) else { return false };
        let negated = |neg: Operand, x: Operand| {
            pattern::binary(&q, neg, BinaryOp::Sub).is_some_and(|(_, _, zero, minus)| {
                q.int_constant(zero).is_some_and(|(bits, _)| bits == 0) && minus == x
            })
        };
        let x = if negated(left, right) {
            right
        } else if negated(right, left) {
            left
        } else {
            return false;
        };
        let Some(x_ty) = q.type_of(x) else { return false };
        let Some(input_bits) = context.types.int_bits(x_ty) else { return false };
        if !matches!(input_bits, 16 | 32 | 64 | 128) || load.scale > low_mask(input_bits) {
            return false;
        }
        if !is_cttz_table(
            context,
            layout,
            load.initializer,
            mul,
            shift,
            and_mask.unwrap_or(low_mask(input_bits)),
            load.access,
            input_bits,
            load.scale,
        ) {
            return false;
        }
        let Some(zero_elem) =
            llrm_mir::constantfolding::constant_fold_load_from_const(context, layout, load.initializer, load.access, 0)
        else {
            return false;
        };
        let ConstantKind::Int(zero_bits) = context.get(zero_elem).kind else { return false };
        (x, x_ty, input_bits, zero_elem, zero_bits == u128::from(input_bits))
    };
    let i1 = context.types.int(1);
    let flag = Operand::Constant(context.int(i1, i128::from(!defined_for_zero)));
    let cttz = call_intrinsic_before(
        context,
        function,
        declared,
        inst,
        &format!("llvm.cttz.i{input_bits}"),
        x_ty,
        vec![x, flag],
        vec![x_ty, i1],
    );
    let mut value = zext_or_trunc_before(context, function, inst, cttz, input_bits, load.access);
    if !defined_for_zero {
        // The table's own element for zero; the pin marks this select's
        // branch unlikely (DEVIATE 42).
        value = select_zero_before(context, function, inst, x, x_ty, load.access, zero_elem, value);
    }
    function.replace_all_uses_with(load.result, value);
    true
}

/// `x == 0 ? zero : value` before `inst`.
fn select_zero_before(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    inst: InstId,
    x: Operand,
    x_ty: TypeId,
    access: TypeId,
    zero: llrm_mir::context::ConstantId,
    value: Operand,
) -> Operand {
    let i1 = context.types.int(1);
    let zero_x = Operand::Constant(context.int(x_ty, 0));
    let cmp = function.create_instruction(
        Opcode::ICmp(llrm_mir::opcode::IntPredicate::Eq),
        i1,
        vec![x, zero_x],
        Flags::default(),
        None,
    );
    function.insert(cmp, Position::Before(inst)).expect("a placed instruction");
    let cond = Operand::Value(function.instruction(cmp).result.expect("a value"));
    let select = function.create_instruction(
        Opcode::Select,
        access,
        vec![cond, Operand::Constant(zero), value],
        Flags::default(),
        None,
    );
    function.insert(select, Position::Before(inst)).expect("a placed instruction");
    Operand::Value(function.instruction(select).result.expect("a value"))
}

/// `tryToRecognizeTableBasedLog2`
/// ([:1130](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1130)):
/// a load from a constant table at `(smear(x) * mul) >> shift`, or with the
/// top bit isolated by `v - (v >> 1)`, is `bits - 1 - ctlz x`.
///
/// DEVIATE 43: no target `tti` is no `ctlz` cost, so no fold. Owner: w2.
/// Exit: 2026-11-21.
fn try_to_recognize_table_based_log2(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    tti: Option<&dyn llrm_mir::tti::Tti>,
    inst: InstId,
    load: &TableLoad,
) -> bool {
    use llrm_mir::valuetracking::pattern;
    let (mut x, x_ty, input_bits, zero_elem, isolated) = {
        let q = query(context, layout, function);
        let Some((_, _, product, shift)) = pattern::binary(&q, load.index, BinaryOp::LShr) else { return false };
        let Some((shift, _)) = q.int_constant(shift) else { return false };
        let Some((_, _, mut x, mul)) = pattern::binary(&q, product, BinaryOp::Mul) else { return false };
        let Some((mul, _)) = q.int_constant(mul) else { return false };
        // The most significant bit isolated first: `v - (v >> 1)`.
        let mut isolated = false;
        if let Some((_, _, v, half)) = pattern::binary(&q, x, BinaryOp::Sub)
            && let Some((_, _, v2, one)) = pattern::binary(&q, half, BinaryOp::LShr)
            && v2 == v
            && q.int_constant(one).is_some_and(|(bits, _)| bits == 1)
        {
            isolated = true;
            x = v;
        }
        let Some(x_ty) = q.type_of(x) else { return false };
        let Some(input_bits) = context.types.int_bits(x_ty) else { return false };
        if !matches!(input_bits, 16 | 32 | 64 | 128) || shift != u128::from(input_bits - input_bits.ilog2()) {
            return false;
        }
        // The or cascade: `v | v >> 64`, ..., `v | v >> 1`.
        let mut amount = input_bits / 2;
        while amount != 0 {
            let Some((_, _, a, b)) = pattern::binary(&q, x, BinaryOp::Or) else { return false };
            let step = |shifted: Operand, y: Operand| {
                pattern::binary(&q, shifted, BinaryOp::LShr).is_some_and(|(_, _, base, by)| {
                    base == y && q.int_constant(by).is_some_and(|(bits, _)| bits == u128::from(amount))
                })
            };
            x = if let Some((_, _, base, _)) = pattern::binary(&q, a, BinaryOp::LShr).filter(|_| step(a, b)) {
                let _ = base;
                b
            } else if step(b, a) {
                a
            } else {
                return false;
            };
            amount /= 2;
        }
        if load.scale > low_mask(input_bits) {
            return false;
        }
        let table_ok = if isolated {
            is_cttz_table(
                context,
                layout,
                load.initializer,
                mul,
                shift,
                low_mask(input_bits),
                load.access,
                input_bits,
                load.scale,
            )
        } else {
            is_log2_table(context, layout, load.initializer, mul, shift, load.access, input_bits, load.scale)
        };
        if !table_ok {
            return false;
        }
        let Some(zero_elem) =
            llrm_mir::constantfolding::constant_fold_load_from_const(context, layout, load.initializer, load.access, 0)
        else {
            return false;
        };
        (x, x_ty, input_bits, zero_elem, isolated)
    };
    let _ = isolated;
    // The target needs an efficient ctlz (a lzcnt, not a bsr and a fix-up).
    let Some(tti) = tti else { return false };
    let cost = tti.zero_poison_intrinsic_cost(
        &format!("llvm.ctlz.i{input_bits}"),
        Some(llrm_mir::tti::Ty::Int(input_bits)),
        &[llrm_mir::tti::Ty::Int(input_bits), llrm_mir::tti::Ty::Int(1)],
        llrm_mir::tti::CostKind::SizeAndLatency,
    );
    if !cost.valid || cost.value > llrm_mir::tti::Cost::BASIC.value {
        return false;
    }
    let ConstantKind::Int(zero_bits) = context.get(zero_elem).kind else { return false };
    let i1 = context.types.int(1);
    let bits_m1 = Operand::Constant(context.int(x_ty, i128::from(input_bits - 1)));
    let result = if zero_bits == u128::from(input_bits - 1) {
        let no = Operand::Constant(context.int(i1, 0));
        let ctlz = call_intrinsic_before(
            context,
            function,
            declared,
            inst,
            &format!("llvm.ctlz.i{input_bits}"),
            x_ty,
            vec![x, no],
            vec![x_ty, i1],
        );
        // `~ctlz & (bits - 1)`.
        let ones = Operand::Constant(context.int(x_ty, -1));
        let not =
            function.create_instruction(Opcode::Binary(BinaryOp::Xor), x_ty, vec![ctlz, ones], Flags::default(), None);
        function.insert(not, Position::Before(inst)).expect("a placed instruction");
        let not = Operand::Value(function.instruction(not).result.expect("a value"));
        let and = function.create_instruction(
            Opcode::Binary(BinaryOp::And),
            x_ty,
            vec![not, bits_m1],
            Flags::default(),
            None,
        );
        function.insert(and, Position::Before(inst)).expect("a placed instruction");
        Operand::Value(function.instruction(and).result.expect("a value"))
    } else {
        let yes = Operand::Constant(context.int(i1, 1));
        let ctlz = call_intrinsic_before(
            context,
            function,
            declared,
            inst,
            &format!("llvm.ctlz.i{input_bits}"),
            x_ty,
            vec![x, yes],
            vec![x_ty, i1],
        );
        let sub = function.create_instruction(
            Opcode::Binary(BinaryOp::Sub),
            x_ty,
            vec![bits_m1, ctlz],
            Flags::default(),
            None,
        );
        function.insert(sub, Position::Before(inst)).expect("a placed instruction");
        let sub = Operand::Value(function.instruction(sub).result.expect("a value"));
        // The table gives the zero element for 0.
        let zero_x = select_zero_before_ext(context, function, inst, x, x_ty, zero_bits, sub);
        zero_x
    };
    let value = zext_or_trunc_before(context, function, inst, result, input_bits, load.access);
    function.replace_all_uses_with(load.result, value);
    true
}

/// `x == 0 ? zero : value` in `x`'s own type, before `inst`.
fn select_zero_before_ext(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    inst: InstId,
    x: Operand,
    x_ty: TypeId,
    zero_bits: u128,
    value: Operand,
) -> Operand {
    let zero_const = context.constant(Constant { ty: x_ty, kind: ConstantKind::Int(zero_bits) });
    select_zero_before(context, function, inst, x, x_ty, x_ty, zero_const, value)
}

/// `tryToRecognizeTableBasedLowBitsMask`
/// ([:1255](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1255)):
/// a load of `tbl[i]` where `tbl[j] == (1 << j) - 1` is `(1 << i) - 1`.
fn try_to_recognize_table_based_low_bits_mask(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    inst: InstId,
    load: &TableLoad,
) -> bool {
    // The table is `[K x AccessType]` of integers.
    let Type::Array { element, count } = context.types.get(load.table_type).clone() else { return false };
    let Some(bits) = context.types.int_bits(load.access) else { return false };
    if element != load.access {
        return false;
    }
    let bytes = layout.alloc_size(&context.types, load.access);
    // The element fits a legal integer, so the shift is cheap and no libcall.
    if !layout.fits_in_legal_integer(bits) {
        return false;
    }
    // `nusw` keeps the address from wrapping but not inside the object; with
    // `inbounds` every executed load has 0 <= i < K.
    if !load.inbounds {
        return false;
    }
    // K <= bits then keeps i < bits: `1 << i` never shifts out of range.
    if count == 0 || count > u64::from(bits) {
        return false;
    }
    // The index steps by one element, so it is the shift amount.
    if load.scale != u128::from(bytes) {
        return false;
    }
    // Every element is the low-bits mask of its position.
    for at in 0..count {
        let folded = llrm_mir::constantfolding::constant_fold_load_from_const(
            context,
            layout,
            load.initializer,
            load.access,
            at * bytes,
        );
        let want = if at == 0 { 0 } else { low_mask(at as u32) };
        if folded.map(|folded| context.get(folded).kind.clone()) != Some(ConstantKind::Int(want)) {
            return false;
        }
    }
    // `(1 << i) - 1` in the element type.
    let index = {
        let index_bits = function.operand_type(context, load.raw_index).and_then(|ty| context.types.int_bits(ty));
        let Some(index_bits) = index_bits else { return false };
        zext_or_trunc_before(context, function, inst, load.raw_index, index_bits, load.access)
    };
    let one = Operand::Constant(context.int(load.access, 1));
    let shifted = function.create_instruction(
        Opcode::Binary(BinaryOp::Shl),
        load.access,
        vec![one, index],
        Flags::default(),
        None,
    );
    function.insert(shifted, Position::Before(inst)).expect("a placed instruction");
    let shifted = Operand::Value(function.instruction(shifted).result.expect("a value"));
    let mask = function.create_instruction(
        Opcode::Binary(BinaryOp::Sub),
        load.access,
        vec![shifted, one],
        Flags::default(),
        None,
    );
    function.insert(mask, Position::Before(inst)).expect("a placed instruction");
    let mask = Operand::Value(function.instruction(mask).result.expect("a value"));
    function.replace_all_uses_with(load.result, mask);
    true
}

/// `tryToRecognizeTableBasedPatterns`
/// ([:1329](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombine.cpp#L1329)).
fn try_to_recognize_table_based_patterns(
    context: &mut llrm_mir::context::Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    globals: &[llrm_mir::module::GlobalValue],
    tti: Option<&dyn llrm_mir::tti::Tti>,
    inst: InstId,
) -> bool {
    let Some(load) = match_table_load(context, layout, function, globals, inst) else { return false };
    try_to_recognize_table_based_cttz(context, layout, function, declared, inst, &load)
        || try_to_recognize_table_based_log2(context, layout, function, declared, tti, inst, &load)
        || try_to_recognize_table_based_low_bits_mask(context, layout, function, inst, &load)
}

#[cfg(test)]
#[path = "aggressive_instcombine_tests.rs"]
mod tests;
