//! InstCombine's `visitFPTrunc`, from
//! [`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L2256)
//! at d1106deb71cc.
//!
//! [`visit_fptrunc`] is `visitFPTrunc`, the same steps in the same order:
//! `commonCastTransforms`, the narrowing of `fptrunc (binop (fpext x), (fpext
//! y))` (the `FAdd`, `FSub`, `FMul`, `FDiv` and `FRem` cases and their
//! double-rounding bounds), `fptrunc (fneg x)`, `fptrunc (select c, (fpext x),
//! y)`, the unary intrinsics, and `fptrunc (itofp x)` when the conversion is
//! exact. `getMinimumFPType` (:2142) is [`minimum_fp_type`].
//!
//! MIRROR: the order of the steps and each condition, the fast-math flags each
//! new instruction takes.
//!
//! DEVIATE 1: MIR has no vectors (`shrinkInsertElt`, vector constants).
//!
//! DEVIATE 2: MIR has no `half` or `bfloat`. A constant that fits `half` takes
//! `float` (precision 24, not 11). Every pair of MIR types the bounds see
//! (`float`, `double`, `x86_fp80`) gives the same answer for 11 and 24.
//!
//! DEVIATE 3: an `x86_fp80` constant is held as a double, so it shrinks only
//! to `float` or `double`, as the pin shrinks a constant that fits.
//!
//! # Example
//! ```text
//! %a = fpext float %x to double          %r = fadd float %x, %y
//! %b = fpext float %y to double
//! %s = fadd double %a, %b
//! %r = fptrunc double %s to float
//! ```
//!
//! # Used by
//! `combine` (its `visit`, the `FPTrunc` opcode).

use llrm_mir::apfloat::{ApFloat, IEEE_DOUBLE, IEEE_SINGLE, semantics_of};
use llrm_mir::context::ConstantKind;
use llrm_mir::fpclass::RoundingMode;
use llrm_mir::intrinsics::{FloatFunction, Intrinsic};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::{FloatKind, Type, TypeId};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_casts::common_cast_transforms;
use crate::combine_fptoint::{can_be_casted_exactly_int_to_fp, def_of, is_known_exact_cast_int_to_fp, mantissa_width};
use crate::combine_shared::{self as shared, query};

/// `visitFPTrunc`.
pub fn visit_fptrunc(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if let Some(found) = common_cast_transforms(c, inst) {
        return found;
    }
    if let Some(found) = narrow_binop(c, inst) {
        return found;
    }
    if let Some(found) = narrow_one_use_operand(c, inst) {
        return found;
    }
    if let Some(found) = narrow_unary_intrinsic(c, inst) {
        return found;
    }
    // DEVIATE 1: shrinkInsertElt.
    let src = c.function.instruction(inst).operands[0];
    if let Some(from_int) = def_of(c, src)
        && let Opcode::Cast(op @ (CastOp::SIToFP | CastOp::UIToFP)) = c.function.instruction(from_int).opcode
        && is_known_exact_cast_int_to_fp(c, from_int)
    {
        let x = c.function.instruction(from_int).operands[0];
        let ty = c.function.instruction(inst).ty;
        return c.replace_with_new(inst, Opcode::Cast(op), ty, vec![x], Flags::default());
    }
    Visit::None
}

/// The fast-math flags of `flags`.
fn fast_math(flags: Flags) -> Flags {
    flags.intersect(Flags::FAST)
}

/// The float kind of `ty`.
fn float_kind(
    c: &Combiner,
    ty: TypeId,
) -> Option<FloatKind> {
    match c.context.types.get(ty) {
        Type::Float(kind) => Some(*kind),
        _ => None,
    }
}

/// `shrinkFPConstant` (:2093) of a scalar: the smallest type holding `v`
/// exactly, if it is smaller than its own.
fn shrink_fp_constant(
    c: &Combiner,
    v: Operand,
) -> Option<TypeId> {
    let Operand::Constant(id) = v else { return None };
    let constant = c.context.get(id);
    let own = float_kind(c, constant.ty)?;
    let ConstantKind::Float(bits) = constant.kind else { return None };
    // DEVIATE 3: an x86_fp80 constant is a double.
    let held = if own == FloatKind::X86Fp80 { IEEE_DOUBLE } else { semantics_of(own) };
    let fits = |to| {
        let mut value = ApFloat::from_bits(held, u128::from(bits));
        let (_, loses_info) = value.convert(to, RoundingMode::NearestTiesToEven);
        !loses_info
    };
    // DEVIATE 2: half is float.
    if fits(IEEE_SINGLE) {
        return Some(c.context.types.intern(Type::Float(FloatKind::Float)));
    }
    if own == FloatKind::Double {
        return None;
    }
    Some(c.context.types.intern(Type::Float(FloatKind::Double)))
}

/// `getMinimumFPType`: the type `v` can be truncated to without losing
/// anything.
fn minimum_fp_type(
    c: &Combiner,
    v: Operand,
    preferred: TypeId,
) -> TypeId {
    if let Some(def) = def_of(c, v) {
        let instruction = c.function.instruction(def);
        match instruction.opcode {
            Opcode::Cast(CastOp::FPExt) => return c.type_of(instruction.operands[0]),
            Opcode::Cast(op @ (CastOp::SIToFP | CastOp::UIToFP))
                if can_be_casted_exactly_int_to_fp(c, instruction.operands[0], preferred, op == CastOp::SIToFP, def) =>
            {
                return preferred;
            }
            _ => {}
        }
    }
    // (float)((double)X+2.0) --> x+2.0f
    shrink_fp_constant(c, v).unwrap_or_else(|| c.type_of(v))
}

/// `Builder.CreateFPTrunc(v, ty)` (with `flags` as its fast-math flags).
fn create_fptrunc(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    ty: TypeId,
    flags: Flags,
) -> Operand {
    c.create_cast(at, CastOp::FPTrunc, v, ty, flags)
}

/// fptrunc (binop (fpext x), (fpext y)) --> binop x, y, when the result is
/// the same.
fn narrow_binop(
    c: &mut Combiner,
    fpt: InstId,
) -> Option<Visit> {
    let src = c.function.instruction(fpt).operands[0];
    if !shared::has_one_use(c, src) {
        return None;
    }
    let bo = def_of(c, src)?;
    let Opcode::Binary(op @ (BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem)) =
        c.function.instruction(bo).opcode
    else {
        return None;
    };
    let [lhs, rhs] = c.function.instruction(bo).operands[..] else { return None };
    let ty = c.function.instruction(fpt).ty;
    let lhs_min = minimum_fp_type(c, lhs, ty);
    let rhs_min = minimum_fp_type(c, rhs, ty);
    let width = |c: &Combiner, ty| mantissa_width(c, ty).unwrap_or(0);
    let op_width = width(c, c.type_of(src));
    let lhs_width = width(c, lhs_min);
    let rhs_width = width(c, rhs_min);
    let src_width = lhs_width.max(rhs_width);
    let dst_width = width(c, ty);

    // Narrowing recomputes the binop in a smaller type, which can overflow to
    // inf where the wide op was finite. Therefore we can only keep ninf if
    // both the binop and the fptrunc have that flag.
    let bo_flags = fast_math(c.function.instruction(bo).flags);
    let mut narrow = bo_flags;
    if !c.function.instruction(fpt).flags.contains(Flags::NINF) {
        narrow.remove(Flags::NINF);
    }

    let fits = match op {
        // The infinitely precise result can be arbitrarily wide; double
        // rounding is innocuous when OpWidth >= 2*DstWidth+1 and DstWidth is
        // sufficient to represent both sources (Figueroa 2000, p50).
        BinaryOp::FAdd | BinaryOp::FSub => op_width >= 2 * dst_width + 1 && dst_width >= src_width,
        // The infinitely precise result has at most LHSWidth + RHSWidth
        // significant bits.
        BinaryOp::FMul => op_width >= lhs_width + rhs_width && dst_width >= src_width,
        BinaryOp::FDiv => op_width >= 2 * dst_width && dst_width >= src_width,
        BinaryOp::FRem => {
            // Remainder is always exact: evaluate in the larger source type,
            // then convert to the destination.
            if src_width == op_width {
                return None;
            }
            let min = if lhs_width == src_width { lhs_min } else { rhs_min };
            let l = create_fptrunc(c, fpt, lhs, min, Flags::default());
            let r = create_fptrunc(c, fpt, rhs, min, Flags::default());
            let exact = c.create_binop(fpt, BinaryOp::FRem, l, r, bo_flags);
            if c.type_of(exact) == ty {
                return Some(Visit::Replace(exact));
            }
            let cast = if width(c, c.type_of(exact)) > dst_width { CastOp::FPTrunc } else { CastOp::FPExt };
            return Some(c.replace_with_new(fpt, Opcode::Cast(cast), ty, vec![exact], Flags::default()));
        }
        _ => return None,
    };
    if !fits {
        return None;
    }
    let l = create_fptrunc(c, fpt, lhs, ty, Flags::default());
    let r = create_fptrunc(c, fpt, rhs, ty, Flags::default());
    Some(c.replace_with_binop(fpt, op, l, r, narrow))
}

/// The select (or fneg) the one-use operand of `fpt` is, narrowed.
fn narrow_one_use_operand(
    c: &mut Combiner,
    fpt: InstId,
) -> Option<Visit> {
    let src = c.function.instruction(fpt).operands[0];
    if !shared::has_one_use(c, src) {
        return None;
    }
    let op = def_of(c, src)?;
    let ty = c.function.instruction(fpt).ty;
    let flags = fast_math(c.function.instruction(fpt).flags).intersect(fast_math(c.function.instruction(op).flags));

    // (fptrunc (fneg x)) -> (fneg (fptrunc x))
    if let Some(x) = pattern::fneg(&query(c, None), src) {
        let inner = create_fptrunc(c, fpt, x, ty, flags);
        let neg = c.insert_before(fpt, Opcode::FNeg, ty, vec![inner], flags);
        return Some(Visit::Replace(c.value_of(neg)));
    }

    // If we are truncating a select that has an extended operand, we can
    // narrow the other operand and do the select as a narrow op.
    if c.function.instruction(op).opcode != Opcode::Select {
        return None;
    }
    let [cond, if_true, if_false] = c.function.instruction(op).operands[..] else { return None };
    let extended_from = |c: &Combiner, v: Operand| -> Option<Operand> {
        let def = def_of(c, v)?;
        let instruction = c.function.instruction(def);
        (instruction.opcode == Opcode::Cast(CastOp::FPExt) && c.type_of(instruction.operands[0]) == ty)
            .then(|| instruction.operands[0])
    };
    // fptrunc (select Cond, (fpext X), Y --> select Cond, X, (fptrunc Y)
    // fptrunc (select Cond, Y, (fpext X) --> select Cond, (fptrunc Y), X
    let (x, y, extended_is_true) = if let Some(x) = extended_from(c, if_true) {
        (x, if_false, true)
    } else if let Some(x) = extended_from(c, if_false) {
        (x, if_true, false)
    } else {
        return None;
    };
    let narrow_y = create_fptrunc(c, fpt, y, ty, flags);
    let select = if extended_is_true {
        c.create_select(fpt, cond, x, narrow_y)
    } else {
        c.create_select(fpt, cond, narrow_y, x)
    };
    if let Some(select_inst) = def_of(c, select)
        && c.function.instruction(select_inst).opcode == Opcode::Select
    {
        c.function.set_flags(select_inst, flags);
    }
    Some(Visit::Replace(select))
}

/// The intrinsic name of `function` on `kind`.
fn unary_name(
    function: FloatFunction,
    kind: FloatKind,
) -> Option<String> {
    let base = match function {
        FloatFunction::Ceil => "ceil",
        FloatFunction::Fabs => "fabs",
        FloatFunction::Floor => "floor",
        FloatFunction::NearbyInt => "nearbyint",
        FloatFunction::Rint => "rint",
        FloatFunction::Round => "round",
        FloatFunction::RoundEven => "roundeven",
        FloatFunction::Trunc => "trunc",
        _ => return None,
    };
    let suffix = match kind {
        FloatKind::Float => "f32",
        FloatKind::Double => "f64",
        FloatKind::X86Fp80 => "f80",
    };
    Some(format!("llvm.{base}.{suffix}"))
}

/// (fptrunc (fabs x)) -> (fabs (fptrunc x)), and the same for the rounding
/// intrinsics when their input is an fpext from the destination type.
fn narrow_unary_intrinsic(
    c: &mut Combiner,
    fpt: InstId,
) -> Option<Visit> {
    let src = c.function.instruction(fpt).operands[0];
    let call = def_of(c, src)?;
    let Some(Intrinsic::Unary(function)) = c.intrinsic_of(call) else { return None };
    let ty = c.function.instruction(fpt).ty;
    let kind = float_kind(c, ty)?;
    let name = unary_name(function, kind)?;
    let arg = *c.function.instruction(call).operands.first()?;
    if !shared::has_one_use(c, arg) {
        return None;
    }
    // Except for fabs, this transformation requires the input of the unary FP
    // operation to be itself an fpext from the type to which we're truncating.
    if function != FloatFunction::Fabs {
        let extended = def_of(c, arg)?;
        let instruction = c.function.instruction(extended);
        if instruction.opcode != Opcode::Cast(CastOp::FPExt) || c.type_of(instruction.operands[0]) != ty {
            return None;
        }
    }
    let inner = create_fptrunc(c, fpt, arg, ty, Flags::default());
    let narrow = c.create_intrinsic_named(fpt, &name, ty, &[inner]);
    // A normal value may be converted to an infinity. It means that we cannot
    // propagate ninf from the intrinsic. So we propagate FMF from fptrunc.
    if let Some(narrow_inst) = def_of(c, narrow) {
        c.function.set_flags(narrow_inst, fast_math(c.function.instruction(fpt).flags));
    }
    Some(Visit::Replace(narrow))
}
