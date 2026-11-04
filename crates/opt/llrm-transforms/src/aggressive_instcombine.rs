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

use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, Opcode, Tail};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::guarantee::{may_read_from_memory, may_throw, may_write_to_memory};
use llrm_mir::valuetracking::nonzero::strip_and_accumulate_constant_offsets;

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
        let folded = fold_unusual_patterns(unit, &reachable, &outer.globals, tti.as_deref());
        if truncs.are_all_preserved() && !folded {
            return PreservedAnalyses::all();
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
    tti: Option<&dyn llrm_mir::tti::Tti>,
) -> bool {
    let mut changed = false;
    let blocks: Vec<BlockId> =
        unit.function.layout().iter().copied().filter(|&block| reachable.contains(&block)).collect();
    for block in blocks {
        // Bottom up: a chain is matched from its end, and a partial pattern
        // is not matched.
        let insts: Vec<InstId> = unit.function.iter_block(block).collect();
        for &inst in insts.iter().rev() {
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
    // "x = (x & 0x33..) + ((x >> 2) & 0x33..)" or "x = x - 3*((x >> 2) & 0x33..)"
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
    let call = function.create_instruction(Opcode::Call(Box::new(info)), ty, vec![root, callee], Flags::default(), None);
    function.insert(call, Position::Before(inst)).expect("a placed instruction");
    let result = function.instruction(inst).result.expect("a value");
    function.replace_all_uses_with(result, Operand::Value(function.instruction(call).result.expect("a value")));
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
        && matches!(cast.opcode, Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc))
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
    mut args: Vec<Operand>,
    parameters: Vec<TypeId>,
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
    let call = function.create_instruction(Opcode::Call(Box::new(info)), ret, args, Flags::default(), None);
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
            pattern::binary(&q, neg, BinaryOp::Sub)
                .is_some_and(|(_, _, zero, minus)| q.int_constant(zero).is_some_and(|(bits, _)| bits == 0) && minus == x)
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
    let cost = tti.intrinsic_cost(
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
        let not = function.create_instruction(
            Opcode::Binary(BinaryOp::Xor),
            x_ty,
            vec![ctlz, ones],
            Flags::default(),
            None,
        );
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
    let shifted =
        function.create_instruction(Opcode::Binary(BinaryOp::Shl), load.access, vec![one, index], Flags::default(), None);
    function.insert(shifted, Position::Before(inst)).expect("a placed instruction");
    let shifted = Operand::Value(function.instruction(shifted).result.expect("a value"));
    let mask =
        function.create_instruction(Opcode::Binary(BinaryOp::Sub), load.access, vec![shifted, one], Flags::default(), None);
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
