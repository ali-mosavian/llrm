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
//!     answer, `X86ISelLoweringCall.cpp#L356`). Owner: w2. Exit: a target
//!     fact when a target says otherwise.
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
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::guarantee::may_write_to_memory;
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
        let folded = fold_unusual_patterns(unit, &reachable);
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
) -> bool {
    let mut changed = false;
    let blocks: Vec<BlockId> = unit.function.layout().iter().copied().filter(|&block| reachable.contains(&block)).collect();
    for block in blocks {
        // Bottom up: a chain is matched from its end, and a partial pattern
        // is not matched.
        let insts: Vec<InstId> = unit.function.iter_block(block).collect();
        for &inst in insts.iter().rev() {
            changed |= fold_consecutive_loads(unit.context, unit.layout, unit.function, inst);
        }
    }
    changed
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
    let one_use = |operand: Operand| matches!(operand, Operand::Value(value) if q.function.users(value).has_one_use());
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
    if !is_root && !matches!(v, Operand::Value(value) if q.function.users(value).has_one_use()) {
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
    let simple = |inst: InstId| matches!(q.function.instruction(inst).opcode, Opcode::Load { volatile: false, .. });
    if li1 == li2 || !simple(li1) || !simple(li2) {
        return false;
    }
    let space = |inst: InstId| match q.type_of(q.function.instruction(inst).operands[0]).map(|ty| q.context.types.get(ty)) {
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
    let bits_of = |inst: InstId| u64::from(q.scalar_bits(Operand::Value(q.function.instruction(inst).result.expect("a load"))));
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
        let q = Query {
            context: &*context,
            globals: &[],
            metadata: &[],
            function: &*function,
            layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        };
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
    let pointer = function.instruction(li1).operands[0];
    let wider = context.types.int(lops.load_size as u32);
    let new_load =
        function.create_instruction(Opcode::Load { align, volatile }, wider, vec![pointer], Flags::default(), None);
    function.insert(new_load, Position::Before(root_insert)).expect("a placed instruction");
    let mut value = Operand::Value(function.instruction(new_load).result.expect("a value"));
    if context.types.int_bits(zext_type) != Some(lops.load_size as u32) {
        let extended = function.create_instruction(
            Opcode::Cast(CastOp::ZExt),
            zext_type,
            vec![value],
            Flags::default(),
            None,
        );
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

#[cfg(test)]
#[path = "aggressive_instcombine_tests.rs"]
mod tests;
