//! `icmp pred (load (gep @table, i)), C`: the compare of an element of a
//! constant table with a constant, as a compare of the index.
//!
//! Translation, from
//! [`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)
//! at d1106deb71cc, of
//! [`foldCmpLoadFromIndexedGlobal`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L119)
//! and of
//! [`decomposeLinearExpression`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/Loads.cpp#L972).
//!
//! MIRROR: the checks, the scan and the order of the state machines.
//!
//! OWED: the `fcmp` call site (`foldFCmpInstWithConstantNotFloat`).
//!
//! # How it works
//! The table is scanned once: each element is loaded from the initializer
//! (masked by `AndCst` when the load is under an `and`) and compared with the
//! constant. The compare is then rebuilt from the elements it is true (or
//! false) for: one or two indices, a range of them, or a bit of a constant
//! bitvector shifted by the index. A multi-use load stays.
//!
//! # Used by
//! `combine_icmp_const::fold_icmp_and_constant` (the masked load) and
//! `combine_compares::visit_icmp` (the plain load).
//!
//! # Example
//! A table of `{0, 1, 2, 17, 64, 100}` compared `> 9` is true for indices 3
//! to 5:
//!
//! ```text
//! %v = load i16, ptr %p        ; %p = gep [2 x i8], ptr @ns, i32 %i
//! %r = icmp sgt i16 %v, 9      ; %t = sub i32 %i, 3
//!                              ; %r = icmp ult i32 %t, 3
//! ```

use llrm_mir::constantfolding::{
    Env, constant_fold_binary_op_operands, constant_fold_compare_inst_operands, constant_fold_load_from_const,
    definitive_constant_initializer,
};
use llrm_mir::context::mask;
use llrm_mir::context::{ConstantId, ConstantKind};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::underlying::get_underlying_object;

use crate::combine::{Combiner, Visit};
use crate::combine_gep::GepFlags;
use crate::combine_shared::query;

/// `MaxArraySizeForCombine` (`-instcombine-maxarray-size`, default 1024).
const MAX_ARRAY_SIZE_FOR_COMBINE: u64 = 1024;

/// `LinearExpression`: `base + index * scale + offset`, all `width` bits.
struct LinearExpression {
    base: Operand,
    index: Option<Operand>,
    scale: u128,
    offset: u128,
    flags: GepFlags,
    width: u32,
}

/// `decomposeLinearExpression` (Loads.cpp#L972): a pointer as a base, one
/// variable index and a constant offset, looking through several GEPs.
fn decompose_linear_expression(
    c: &Combiner,
    ptr: Operand,
) -> LinearExpression {
    let width = c.context.types.int_bits(c.index_type(c.type_of(ptr))).expect("an integer index");
    let m = mask(width);
    let mut expr = LinearExpression { base: ptr, index: None, scale: 0, offset: 0, flags: GepFlags::ALL, width };
    let q = query(c, None);
    while let Some(gep) = c.defining_gep_ref(expr.base).map(|gep| c.gep_view(gep)) {
        let mut var_index = None;
        for &index in &gep.indices {
            if q.int_constant(index).is_some() {
                continue;
            }
            // Only allow a single variable index.
            if expr.index.is_some() || var_index.is_some() {
                return expr;
            }
            var_index = Some(index);
        }
        // Don't return non-canonical indexes.
        if var_index.is_some_and(|index| c.context.types.int_bits(c.type_of(index)) != Some(width)) {
            return expr;
        }
        // The GEP can be fully handled: update the expression.
        expr.base = gep.ptr;
        expr.flags = expr.flags.intersect_for_offset_add(GepFlags::of(gep.flags));
        let types = &c.context.types;
        let mut current = gep.source;
        for (at, &index) in gep.indices.iter().enumerate() {
            // The type this index steps over, as `gep_type_iterator`.
            let is_struct = at > 0 && types.fields(current).is_some();
            let stride = if at == 0 {
                c.env.layout.alloc_size(types, gep.source)
            } else if is_struct {
                0
            } else {
                current = match types.get(current) {
                    Type::Array { element, .. } | Type::Vector { element, .. } => *element,
                    _ => return expr,
                };
                c.env.layout.alloc_size(types, current)
            };
            if let Some((bits, from)) = q.int_constant(index) {
                if bits == 0 {
                    if is_struct {
                        current = types.member(current, 0).expect("a field");
                    }
                    continue;
                }
                if is_struct {
                    let (_, offsets) = c.env.layout.struct_layout(types, current);
                    expr.offset = expr.offset.wrapping_add(u128::from(offsets[bits as usize])) & m;
                    current = types.member(current, bits as u64).expect("a field");
                    continue;
                }
                let signed_index = llrm_mir::context::signed(bits, from) as u128;
                expr.offset = expr.offset.wrapping_add(signed_index.wrapping_mul(u128::from(stride))) & m;
                continue;
            }
            // FIXME in LLVM: also look through a mul/shl in the index.
            expr.index = Some(index);
            expr.scale = u128::from(stride) & m;
        }
    }
    expr
}

/// Whether a constant folds to `undef` or `poison`.
fn is_undef_or_poison(
    c: &Combiner,
    id: ConstantId,
) -> bool {
    matches!(
        c.context.get(id).kind,
        ConstantKind::Undef | ConstantKind::Poison
    )
}

/// The state machines' "no element yet" and "more than two" markers.
const UNDEFINED: i64 = -2;
const OVERDEFINED: i64 = -3;

/// `foldCmpLoadFromIndexedGlobal`: `cmp pred (load (gep GV, ...)), cmpcst`,
/// with the loaded value masked by `and_cst` when it is `Some`.
pub fn fold_cmp_load_from_indexed_global(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    load: InstId,
    compare_rhs: ConstantId,
    and_cst: Option<ConstantId>,
) -> Option<Visit> {
    let (volatile, load_ty, ptr) = {
        let inst = c.function.instruction(load);
        let Opcode::Load { volatile, .. } = inst.opcode else { return None };
        (volatile, inst.ty, inst.operands[0])
    };
    c.defining_gep(ptr)?;
    let q = query(c, Some(inst));
    let Operand::Constant(gv) = get_underlying_object(&q, ptr, 0) else { return None };
    let env = Env { globals: c.env.globals, layout: c.env.layout };
    let (_, init) = definitive_constant_initializer(env, c.context, gv)?;
    if volatile {
        return None;
    }
    let elt_size = c.env.layout.store_size(&c.context.types, load_ty);

    let expr = decompose_linear_expression(c, ptr);
    let idx = expr.index?;
    if expr.base != Operand::Constant(gv) || expr.width > 64 {
        return None;
    }
    let global_size = c.env.layout.alloc_size(&c.context.types, c.context.get(init).ty);
    let (stride, const_offset) = (expr.scale, expr.offset);

    // Allow an additional context offset, but only within the stride.
    if const_offset >= stride {
        return None;
    }
    // Don't handle overlapping loads for now.
    if stride < u128::from(elt_size) {
        return None;
    }
    // Don't blow up on huge arrays.
    let array_element_count = (u128::from(global_size).checked_sub(const_offset)?).div_ceil(stride);
    if array_element_count > u128::from(MAX_ARRAY_SIZE_FOR_COMBINE) {
        return None;
    }
    let array_element_count = array_element_count as u64;

    // First/second true and false index, the range ends and the bitvector.
    let (mut first_true, mut second_true) = (UNDEFINED, UNDEFINED);
    let (mut first_false, mut second_false) = (UNDEFINED, UNDEFINED);
    let (mut true_range_end, mut false_range_end) = (UNDEFINED, UNDEFINED);
    let mut magic_bitvector: u64 = 0;

    // Scan the array and see if one of our patterns matches.
    let mut offset = const_offset;
    for i in 0..array_element_count as i64 {
        let mut elt = constant_fold_load_from_const(c.context, c.env.layout, init, load_ty, offset as u64)?;
        // If the element is masked, handle it.
        if let Some(and) = and_cst {
            elt = constant_fold_binary_op_operands(c.context, env, BinaryOp::And, elt, and)?;
        }
        // Is the comparison true or false for the i'th element?
        let result =
            constant_fold_compare_inst_operands(c.context, c.env.globals, c.env.layout, pred, elt, compare_rhs)?;
        // An undef result is ignored; extend the range state machines over it.
        if is_undef_or_poison(c, result) {
            if true_range_end == i - 1 {
                true_range_end = i;
            }
            if false_range_end == i - 1 {
                false_range_end = i;
            }
            offset += stride;
            continue;
        }
        // If the result is not computed for any element, give up.
        let (bits, _) = q.int_constant(Operand::Constant(result))?;
        let is_true = bits != 0;

        // State machine for single/double/range index comparison.
        if is_true {
            if first_true == UNDEFINED {
                first_true = i;
                true_range_end = i;
            } else {
                second_true = if second_true == UNDEFINED { i } else { OVERDEFINED };
                true_range_end = if true_range_end == i - 1 { i } else { OVERDEFINED };
            }
        } else if first_false == UNDEFINED {
            first_false = i;
            false_range_end = i;
        } else {
            second_false = if second_false == UNDEFINED { i } else { OVERDEFINED };
            false_range_end = if false_range_end == i - 1 { i } else { OVERDEFINED };
        }

        // If this element is in range, update the magic bitvector.
        if i < 64 && is_true {
            magic_bitvector |= 1u64 << i;
        }

        // Bail out early when everything is overdefined; checked only every
        // 8 elements, and useful for huge arrays only.
        if (i & 8) == 0
            && i >= 64
            && second_true == OVERDEFINED
            && second_false == OVERDEFINED
            && true_range_end == OVERDEFINED
            && false_range_end == OVERDEFINED
        {
            return None;
        }
        offset += stride;
    }

    // Emit the new comparison(s), the state machines in the order of the
    // complexity of the code they generate.
    let ty = c.type_of(idx);
    let i1 = c.context.types.int(1);
    // If inbounds is not present, Idx * Stride can overflow: erase the
    // highest countTrailingZeros(Stride) bits of Idx.
    let mask_idx = |c: &mut Combiner, idx: Operand| -> Operand {
        let shift = stride.trailing_zeros();
        if !expr.flags.in_bounds && shift != 0 {
            let all_ones = c.all_ones(ty);
            let amount = c.int(ty, u128::from(shift));
            let mask_value = c.create_binop(inst, BinaryOp::LShr, all_ones, amount, Flags::default());
            return c.create_binop(inst, BinaryOp::And, idx, mask_value, Flags::default());
        }
        idx
    };
    let eq_ne = |c: &mut Combiner, pred: IntPredicate, idx: Operand, at: i64| -> Operand {
        let at = c.int(ty, at as u128 & mask(expr.width));
        c.create_icmp(inst, pred, idx, at)
    };

    // The comparison is true for only one or two elements: direct compares.
    if second_true != OVERDEFINED {
        let idx = mask_idx(c, idx);
        // None true -> false.
        if first_true == UNDEFINED {
            return Some(Visit::Replace(c.int(i1, 0)));
        }
        let first = c.int(ty, first_true as u128);
        // True for one element -> 'i == 47'.
        if second_true == UNDEFINED {
            return Some(new_icmp(c, inst, IntPredicate::Eq, idx, first));
        }
        // True for two elements -> 'i == 47 | i == 72'.
        let c1 = c.create_icmp(inst, IntPredicate::Eq, idx, first);
        let c2 = eq_ne(c, IntPredicate::Eq, idx, second_true);
        return Some(c.replace_with_binop(inst, BinaryOp::Or, c1, c2, Flags::default()));
    }

    // Likewise for only one or two false elements.
    if second_false != OVERDEFINED {
        let idx = mask_idx(c, idx);
        // None false -> true.
        if first_false == UNDEFINED {
            return Some(Visit::Replace(c.int(i1, 1)));
        }
        let first = c.int(ty, first_false as u128);
        // False for one element -> 'i != 47'.
        if second_false == UNDEFINED {
            return Some(new_icmp(c, inst, IntPredicate::Ne, idx, first));
        }
        // False for two elements -> 'i != 47 & i != 72'.
        let c1 = c.create_icmp(inst, IntPredicate::Ne, idx, first);
        let c2 = eq_ne(c, IntPredicate::Ne, idx, second_false);
        return Some(c.replace_with_binop(inst, BinaryOp::And, c1, c2, Flags::default()));
    }

    // True for a range of elements: (i - FirstTrue) <u (TrueRangeEnd -
    // FirstTrue + 1).
    if true_range_end != OVERDEFINED {
        debug_assert!(true_range_end != first_true, "should emit single compare");
        let mut idx = mask_idx(c, idx);
        if first_true != 0 {
            let offs = c.int(ty, (first_true as u128).wrapping_neg() & mask(expr.width));
            idx = c.create_add(inst, idx, offs);
        }
        let end = c.int(ty, (true_range_end - first_true + 1) as u128);
        return Some(new_icmp(c, inst, IntPredicate::Ult, idx, end));
    }

    // False range check: (i - FirstFalse) >u (FalseRangeEnd - FirstFalse).
    if false_range_end != OVERDEFINED {
        debug_assert!(false_range_end != first_false, "should emit single compare");
        let mut idx = mask_idx(c, idx);
        if first_false != 0 {
            let offs = c.int(ty, (first_false as u128).wrapping_neg() & mask(expr.width));
            idx = c.create_add(inst, idx, offs);
        }
        let end = c.int(ty, (false_range_end - first_false) as u128);
        return Some(new_icmp(c, inst, IntPredicate::Ugt, idx, end));
    }

    // A magic bitvector that holds the whole comparison state of the load:
    // ((magic_cst >> i) & 1) != 0, in the type of Idx if the magic fits, else
    // the smallest fitting legal type.
    let idx_bits = u64::from(expr.width);
    let magic_ty: Option<TypeId> = if array_element_count <= idx_bits {
        Some(ty)
    } else {
        // DataLayout::getSmallestLegalIntType.
        let wanted = array_element_count as u32;
        (1..=64)
            .filter(|&bits| bits >= wanted && c.env.layout.legal_integer(bits))
            .min()
            .map(|bits| c.context.types.int(bits))
    };
    if let Some(magic_ty) = magic_ty {
        let idx = mask_idx(c, idx);
        let v = if magic_ty == ty { idx } else { cast_int(c, inst, idx, magic_ty) };
        let magic = c.int(magic_ty, u128::from(magic_bitvector));
        let shifted = c.create_binop(inst, BinaryOp::LShr, magic, v, Flags::default());
        let one = c.int(magic_ty, 1);
        let bit = c.create_binop(inst, BinaryOp::And, one, shifted, Flags::default());
        let zero = c.int(magic_ty, 0);
        return Some(new_icmp(c, inst, IntPredicate::Ne, bit, zero));
    }
    None
}

/// `Builder.CreateIntCast(v, ty, false)`: a zero extension or truncation.
fn cast_int(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    ty: TypeId,
) -> Operand {
    let from = c.context.types.int_bits(c.type_of(v)).expect("an integer");
    let to = c.context.types.int_bits(ty).expect("an integer");
    let op = if to > from { CastOp::ZExt } else { CastOp::Trunc };
    c.create_cast(at, op, v, ty, Flags::default())
}

/// A new compare in place of `inst`: `new ICmpInst(pred, a, b)`.
fn new_icmp(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    a: Operand,
    b: Operand,
) -> Visit {
    let i1 = c.context.types.int(1);
    c.replace_with_new(inst, Opcode::ICmp(pred), i1, vec![a, b], Flags::default())
}

/// The load a compare's left operand is, when the right one is a constant:
/// the `Instruction::Load` arm of `foldICmpInstWithConstantNotInt`.
pub fn fold_icmp_of_load(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Visit> {
    let Operand::Constant(rhs) = rhs else { return None };
    let load = c.definition_of(lhs).filter(|&def| matches!(c.function.instruction(def).opcode, Opcode::Load { .. }))?;
    fold_cmp_load_from_indexed_global(c, inst, pred, load, rhs, None)
}
