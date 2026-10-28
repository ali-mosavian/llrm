//! InstCombine's `visitTrunc`, `visitZExt`, `visitSExt` and the helpers they
//! call, from
//! [`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp)
//! at d1106deb71cc.
//!
//! [`visit_trunc`] is
//! [`visitTrunc`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L1045),
//! [`visit_zext`] is
//! [`visitZExt`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L1593),
//! [`visit_sext`] is
//! [`visitSExt`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L1915):
//! the same steps in the same order, each helper named for the C++ one with
//! its own permalink. A step that returns `nullptr` there returns `None` here,
//! one that returns an instruction returns `Some(Visit)`.
//!
//! MIRROR: the order of the steps and each condition.
//!
//! DEVIATE 1: MIR has no vectors and no `vscale`: `foldVecTruncToExtElt`,
//! `foldVecExtTruncToExtElt`, `shrinkSplatShuffle`, `shrinkInsertElt`,
//! `foldExtractionOfVectorDeinterleave`, the shuffle step of
//! `commonCastTransforms` and the `vscale` folds are not translated.
//!
//! DEVIATE 4 (temporary): the folds that build an intrinsic call (`uadd.sat`,
//! `usub.sat`, `smin`, `smax`, `ctlz`, `scmp`, `ucmp`, `fshl`, `fshr`) are
//! left out until the Builder can declare an intrinsic (`combine_build`
//! DEVIATE 2). So is the `llvm.abs` and min/max call case of
//! `canEvaluateTruncated` and `EvaluateInDifferentType`: a call is not
//! evaluable in the narrow type here.
//!
//! DEVIATE 5: integers wider than 128 bits are left alone (masks are `u128`);
//! a constant `ConstantExpr` would keep as an expression, or a shift amount
//! that is not an integer constant, skips the fold.
//!
//! # Used by
//! `combine::visit`, once the casts family's slot is flipped.
//!
//! # How it works
//! Each `visit_*` returns the first step that changes the cast. A tree of
//! one-use integer operations under a cast is rebuilt in the cast's type
//! (`canEvaluate*` asks, `EvaluateInDifferentType` builds), which removes the
//! cast; a cast of a cast, of a compare or of a shift pair folds to a mask,
//! a shift or a compare.
//!
//! # Example
//! `trunc (add (zext x), 1)` is computed in the narrow type:
//!
//! ```text
//! %a = zext i8 %x to i32
//! %b = add i32 %a, 1
//! %r = trunc i32 %b to i8   ; %r = add i8 %x, 1
//! ```

use llrm_mir::apfloat;
use llrm_mir::constantfolding;
use llrm_mir::context::{mask, signed};
use llrm_mir::dense::IdMap;
use llrm_mir::hash::HashMap;
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::simplify::simplify_instruction_in;
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::pattern::{self, IntrinsicId};
use llrm_mir::valuetracking::signbits::{compute_max_significant_bits, compute_num_sign_bits};
use llrm_mir::valuetracking::{Query, known, nonzero};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{self as shared, query};

/// The width of the integer type `ty`.
fn bits_of(
    c: &Combiner,
    ty: TypeId,
) -> u32 {
    c.context.types.int_bits(ty).expect("an integer")
}

/// The width of the integer `v` (a scalar: MIR has no vectors).
fn width(
    c: &Combiner,
    v: Operand,
) -> u32 {
    bits_of(c, c.type_of(v))
}

/// The instruction that defines `v`.
fn def_of(
    c: &Combiner,
    v: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = v else { return None };
    match c.function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        _ => None,
    }
}

/// `V->user_back()`: the newest user of the value `inst` makes.
fn user_back(
    c: &Combiner,
    inst: InstId,
) -> Option<InstId> {
    let result = c.function.instruction(inst).result?;
    c.function.users(result).first().map(|one| one.user)
}

/// The value the instruction `inst` makes.
fn value_of(
    c: &Combiner,
    inst: InstId,
) -> Operand {
    Operand::Value(c.function.instruction(inst).result.expect("a value"))
}

/// `m_BinOp(op, a, b)`: the operands and flags of `v` if it is that operation.
fn binop(
    q: &Query,
    v: Operand,
    op: BinaryOp,
) -> Option<(Operand, Operand, Flags)> {
    pattern::binary(q, v, op).map(|(_, instruction, a, b)| (a, b, instruction.flags))
}

/// `m_Shr(a, b)`: `lshr` or `ashr`.
fn shr(
    q: &Query,
    v: Operand,
) -> Option<(Operand, Operand, Flags)> {
    binop(q, v, BinaryOp::LShr).or_else(|| binop(q, v, BinaryOp::AShr))
}

/// `m_Trunc(m_SpecificType(ty, X))`.
fn trunc_from(
    c: &Combiner,
    q: &Query,
    v: Operand,
    ty: TypeId,
) -> Option<Operand> {
    pattern::cast(q, v, CastOp::Trunc).map(|(_, _, x)| x).filter(|&x| c.type_of(x) == ty)
}

/// `m_ZExtOrSExt(m_SpecificType(ty, X))`.
fn ext_from(
    c: &Combiner,
    q: &Query,
    v: Operand,
    ty: TypeId,
) -> Option<Operand> {
    [CastOp::ZExt, CastOp::SExt]
        .into_iter()
        .find_map(|op| pattern::cast(q, v, op))
        .map(|(_, _, x)| x)
        .filter(|&x| c.type_of(x) == ty)
}

/// `m_c_BitwiseLogic(pick, m_Value(Y))`: the operation, what `pick` took from
/// the operand it matches (tried first on the left), and the other.
fn bitwise_logic_with(
    q: &Query,
    v: Operand,
    pick: impl Fn(Operand) -> Option<Operand>,
) -> Option<(BinaryOp, Operand, Operand)> {
    let (_, instruction) = q.instruction_of(v)?;
    let Opcode::Binary(op @ (BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)) = instruction.opcode else { return None };
    let [a, b] = instruction.operands[..] else { return None };
    pick(a).map(|x| (op, x, b)).or_else(|| pick(b).map(|x| (op, x, a)))
}

/// `ConstantExpr::getSub` and the like of two constants (DEVIATE 5: none where
/// it would stay an expression).
fn fold_constants(
    c: &Combiner,
    op: BinaryOp,
    a: Operand,
    b: Operand,
) -> Option<Operand> {
    let (Operand::Constant(x), Operand::Constant(y)) = (a, b) else { return None };
    let env = constantfolding::Env { globals: c.env.globals, layout: c.env.layout };
    constantfolding::constant_fold_binary_op_operands(c.context, env, op, x, y).map(Operand::Constant)
}

/// `ConstantExpr::getTrunc(C, ty)` (DEVIATE 5).
fn fold_trunc(
    c: &Combiner,
    v: Operand,
    ty: TypeId,
) -> Option<Operand> {
    let Operand::Constant(x) = v else { return None };
    constantfolding::constant_fold_cast_operand(c.context, c.env.layout, CastOp::Trunc, x, ty).map(Operand::Constant)
}

/// The `nuw` and `nsw` that are set.
fn wrap_flags(
    nuw: bool,
    nsw: bool,
) -> Flags {
    let mut flags = Flags::default();
    if nuw {
        flags.insert(Flags::NUW);
    }
    if nsw {
        flags.insert(Flags::NSW);
    }
    flags
}

/// The cast `CastInst::CreateIntegerCast` picks.
fn int_cast_op(
    from: u32,
    to: u32,
    is_signed: bool,
) -> CastOp {
    match from.cmp(&to) {
        std::cmp::Ordering::Greater => CastOp::Trunc,
        std::cmp::Ordering::Less if is_signed => CastOp::SExt,
        std::cmp::Ordering::Less => CastOp::ZExt,
        std::cmp::Ordering::Equal => CastOp::BitCast,
    }
}

/// `Builder.CreateIntCast(v, ty, is_signed)`.
fn create_int_cast(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    ty: TypeId,
    is_signed: bool,
) -> Operand {
    if c.type_of(v) == ty {
        return v;
    }
    let op = int_cast_op(width(c, v), bits_of(c, ty), is_signed);
    c.create_cast(at, op, v, ty, Flags::default())
}

/// What a `visit*` returns for `CastInst::CreateIntegerCast(v, ty, is_signed)`.
fn replace_with_int_cast(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    ty: TypeId,
    is_signed: bool,
    flags: Flags,
) -> Visit {
    let op = int_cast_op(width(c, v), bits_of(c, ty), is_signed);
    c.replace_with_new(at, Opcode::Cast(op), ty, vec![v], flags)
}

/// What a `visit*` returns for `new ICmpInst(pred, a, b)`.
fn replace_with_icmp(
    c: &mut Combiner,
    at: InstId,
    pred: IntPredicate,
    a: Operand,
    b: Operand,
) -> Visit {
    let ty = c.context.types.int(1);
    c.replace_with_new(at, Opcode::ICmp(pred), ty, vec![a, b], Flags::default())
}

/// Sets `flag` on the instruction `inst`.
fn set_flag(
    c: &mut Combiner,
    inst: InstId,
    flag: Flags,
) {
    let mut flags = c.function.instruction(inst).flags;
    flags.insert(flag);
    c.function.set_flags(inst, flags);
}

/// `shouldChangeType(from, to)` with the combiner's data layout.
fn should_change_type(
    c: &Combiner,
    from: u32,
    to: u32,
) -> bool {
    shared::should_change_type(c.env.layout, from, to)
}

/// Whether the cast `inst` has a type wider than `u128` holds (DEVIATE 5).
fn too_wide(
    c: &Combiner,
    inst: InstId,
) -> bool {
    let instruction = c.function.instruction(inst);
    width(c, instruction.operands[0]).max(bits_of(c, instruction.ty)) > 128
}

/// The evaluator's own rule for what is free in `ty`:
/// `TypeEvaluationHelper::canAlwaysEvaluateInType` (:476). Constants and
/// extensions or truncates from the destination type.
fn can_always_evaluate_in_type(
    c: &Combiner,
    v: Operand,
    ty: TypeId,
) -> bool {
    let q = query(c, None);
    if matches!(v, Operand::Constant(_)) {
        return pattern::imm_constant(&q, v);
    }
    [CastOp::ZExt, CastOp::SExt, CastOp::Trunc]
        .into_iter()
        .any(|op| pattern::cast(&q, v, op).is_some_and(|(_, _, x)| c.type_of(x) == ty))
}

/// Which predicate a `canEvaluate` asks.
#[derive(Clone, Copy)]
enum Pred {
    Truncated,
    SExtd,
}

/// `TypeEvaluationHelper` (:303): what has been decided, and the users of the
/// many-use values that must all be part of the tree.
struct Evaluator<'a, 'b> {
    c: &'a Combiner<'b>,
    /// `CxtI`.
    cxt: InstId,
    visited: IdMap<InstId, bool>,
    pending: Vec<InstId>,
}

impl Evaluator<'_, '_> {
    /// `allPendingVisited`.
    fn all_pending_visited(&self) -> bool {
        self.pending.iter().all(|user| self.visited.contains_key(user))
    }

    /// `canEvaluate` (:338): the wrapper that tracks visits and many-use
    /// values around `pred`.
    fn can_evaluate(
        &mut self,
        v: Operand,
        ty: TypeId,
        pred: Pred,
    ) -> bool {
        let c = self.c;
        if can_always_evaluate_in_type(c, v, ty) {
            return true;
        }
        let Some(i) = def_of(c, v) else { return false };
        // False by default, so a loop of users answers false.
        if let Some(&known) = self.visited.get(&i) {
            return known;
        }
        self.visited.insert(i, false);
        // A many-use value is decided here only if all its users are in the
        // tree: they are pending until the end.
        if let Operand::Value(value) = v
            && !c.function.users(value).has_one_use()
        {
            for one in c.function.users(value).iter() {
                // The select condition keeps its type and so its use.
                if one.index == 0 && c.function.instruction(one.user).opcode == Opcode::Select {
                    return false;
                }
                self.pending.push(one.user);
            }
        }
        let result = match pred {
            Pred::Truncated => self.truncated_pred(i, ty),
            Pred::SExtd => self.sextd_pred(i, ty),
        };
        self.visited.insert(i, result);
        result
    }

    /// Every operand of `ops` can be evaluated, left to right.
    fn all(
        &mut self,
        ops: &[Operand],
        ty: TypeId,
        pred: Pred,
    ) -> bool {
        ops.iter().all(|&op| self.can_evaluate(op, ty, pred))
    }

    /// `canEvaluateTruncatedPred` (:530).
    fn truncated_pred(
        &mut self,
        i: InstId,
        ty: TypeId,
    ) -> bool {
        let c = self.c;
        let inst = c.function.instruction(i);
        let orig_bits = bits_of(c, inst.ty);
        let bw = bits_of(c, ty);
        let ops = &inst.operands[..];
        match inst.opcode {
            // These operators can all arbitrarily be extended or truncated.
            Opcode::Binary(
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor,
            ) => self.all(ops, ty, Pred::Truncated),
            // UDiv and URem can be truncated if all the truncated bits are
            // zero.
            Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem) => {
                let high = mask(orig_bits) & !mask(bw);
                // Not the original context instruction: simplifying div/rem
                // based on later context may introduce a trap.
                let q = query(c, Some(i));
                known::masked_value_is_zero(&q, ops[0], high, 0)
                    && known::masked_value_is_zero(&q, ops[1], high, 0)
                    && self.all(ops, ty, Pred::Truncated)
            }
            // A shift of an in-range amount can be done in the smaller type.
            Opcode::Binary(BinaryOp::Shl) => {
                let amount = known::compute_known_bits(&Query::bare(c.context, c.function, c.env.layout), ops[1], 0);
                amount.max_value() < u128::from(bw) && self.all(ops, ty, Pred::Truncated)
            }
            // A logical shift right narrows if the bits shifted in are zeros.
            Opcode::Binary(BinaryOp::LShr) => {
                let q = query(c, Some(self.cxt));
                let max_shift = known::compute_known_bits(&q, ops[1], 0).max_value();
                if max_shift >= u128::from(bw) {
                    return false;
                }
                // If the only user is a trunc then we can narrow the shift if
                // any new MSBs are not going to be used.
                if let Some(user) = user_back(c, i)
                    && c.function.instruction(user).opcode == Opcode::Cast(CastOp::Trunc)
                {
                    let demanded = u128::from(bits_of(c, c.function.instruction(user).ty));
                    if (max_shift + demanded) & mask(orig_bits) <= u128::from(bw) {
                        return self.all(ops, ty, Pred::Truncated);
                    }
                }
                known::masked_value_is_zero(&q, ops[0], mask(orig_bits) & !mask(bw), 0)
                    && self.all(ops, ty, Pred::Truncated)
            }
            // An arithmetic shift right narrows if the sign bit of the original
            // type and of the truncated one are copies.
            Opcode::Binary(BinaryOp::AShr) => {
                let amount = known::compute_known_bits(&Query::bare(c.context, c.function, c.env.layout), ops[1], 0);
                amount.max_value() < u128::from(bw)
                    && orig_bits - bw < compute_num_sign_bits(&query(c, Some(self.cxt)), ops[0], 0)
                    && self.all(ops, ty, Pred::Truncated)
            }
            // trunc(trunc(x)) is trunc(x); trunc(ext(x)) is ext(x) or trunc(x).
            Opcode::Cast(CastOp::Trunc | CastOp::ZExt | CastOp::SExt) => true,
            Opcode::Select => self.all(&ops[1..], ty, Pred::Truncated),
            // Cyclic phis are no trouble: `can_evaluate` stops use-chain loops.
            Opcode::Phi => {
                let incoming: Vec<Operand> = pattern::phi_incoming(inst).map(|(value, _)| value).collect();
                self.all(&incoming, ty, Pred::Truncated)
            }
            // If the integer type can hold the max FP value, it is safe to cast
            // directly to that type.
            Opcode::Cast(op @ (CastOp::FPToUI | CastOp::FPToSI)) => {
                let Type::Float(kind) = c.context.types.get(c.type_of(ops[0])) else { return false };
                let min = apfloat::semantics_max_exponent(apfloat::semantics_of(*kind)) as u32
                    + 1
                    + u32::from(op == CastOp::FPToSI);
                bw >= min
            }
            // DEVIATE 4: llvm.abs and the min/max intrinsics.
            _ => false,
        }
    }

    /// `canEvaluateSExtdPred` (:1870).
    fn sextd_pred(
        &mut self,
        i: InstId,
        ty: TypeId,
    ) -> bool {
        let c = self.c;
        let inst = c.function.instruction(i);
        let ops = &inst.operands[..];
        match inst.opcode {
            // sext(sext(x)), sext(zext(x)), sext(trunc(x)).
            Opcode::Cast(CastOp::SExt | CastOp::ZExt | CastOp::Trunc) => true,
            Opcode::Binary(
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor,
            ) => self.all(ops, ty, Pred::SExtd),
            Opcode::Select => self.all(&ops[1..], ty, Pred::SExtd),
            Opcode::Phi => {
                let incoming: Vec<Operand> = pattern::phi_incoming(inst).map(|(value, _)| value).collect();
                self.all(&incoming, ty, Pred::SExtd)
            }
            _ => false,
        }
    }
}

/// `canEvaluateTruncated` (:512): `v` can be computed in `ty`, narrower, with
/// the same low bits.
fn can_evaluate_truncated(
    c: &Combiner,
    v: Operand,
    ty: TypeId,
    cxt: InstId,
) -> bool {
    let mut evaluator = Evaluator { c, cxt, visited: IdMap::new(), pending: Vec::new() };
    // Whether all users of the many-use values were visited is asked last,
    // outside the recursion.
    evaluator.can_evaluate(v, ty, Pred::Truncated) && evaluator.all_pending_visited()
}

/// `canEvaluateSExtd` (:1859): `v` can be returned as `ty`, wider, without a
/// new cast and with the same low bits.
fn can_evaluate_sextd(
    c: &Combiner,
    v: Operand,
    ty: TypeId,
) -> bool {
    // The context instruction is read by the truncating predicates only.
    let Some(cxt) = def_of(c, v) else { return can_always_evaluate_in_type(c, v, ty) };
    let mut evaluator = Evaluator { c, cxt, visited: IdMap::new(), pending: Vec::new() };
    evaluator.can_evaluate(v, ty, Pred::SExtd) && evaluator.all_pending_visited()
}

/// `canEvaluateZExtd` (:1469): `v` can be computed in `ty`, wider, with the
/// same low bits; the number of high bits of `v`'s own width that the result
/// must clear (`BitsToClear`).
fn can_evaluate_zextd(
    c: &Combiner,
    v: Operand,
    ty: TypeId,
    cxt: InstId,
) -> Option<u32> {
    if can_always_evaluate_in_type(c, v, ty) {
        return Some(0);
    }
    // One user only, as the predicate answers with two values.
    let i = def_of(c, v)?;
    if !shared::has_one_use(c, v) {
        return None;
    }
    let inst = c.function.instruction(i);
    let ops = &inst.operands[..];
    let vsize = bits_of(c, inst.ty);
    match inst.opcode {
        // zext(zext(x)), zext(sext(x)), zext(trunc(x)).
        Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc) => Some(0),
        Opcode::Binary(
            op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor),
        ) => {
            let to_clear = can_evaluate_zextd(c, ops[0], ty, cxt)?;
            let other = can_evaluate_zextd(c, ops[1], ty, cxt)?;
            // These can all be promoted if neither operand has bits to clear.
            if to_clear == 0 && other == 0 {
                return Some(0);
            }
            // If the operation is an AND/OR/XOR and the bits to clear are zero
            // in the other side, `to_clear` is ok.
            if other == 0 && matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) {
                let high = mask(vsize) & !mask(vsize.saturating_sub(to_clear));
                if known::masked_value_is_zero(&query(c, Some(cxt)), ops[1], high, 0) {
                    // If this is an And and all of the bits to clear are known
                    // zero we can reset them.
                    return Some(if op == BinaryOp::And { 0 } else { to_clear });
                }
            }
            None
        }
        // shl(x, cst) overwrites the upper bits, so the bits to clear shrink.
        Opcode::Binary(BinaryOp::Shl) => {
            let shift = pattern::constant_int_u64(&query(c, None), ops[1])?;
            let to_clear = can_evaluate_zextd(c, ops[0], ty, cxt)?;
            Some(if shift < u64::from(to_clear) { to_clear - shift as u32 } else { 0 })
        }
        // lshr(x, cst): the ultimate `and` clears the bits shifted in.
        Opcode::Binary(BinaryOp::LShr) => {
            let shift = pattern::constant_int_u64(&query(c, None), ops[1])?;
            let to_clear = can_evaluate_zextd(c, ops[0], ty, cxt)?;
            Some((u64::from(to_clear).wrapping_add(shift) as u32).min(vsize))
        }
        Opcode::Select => {
            let on_true = can_evaluate_zextd(c, ops[1], ty, cxt)?;
            let on_false = can_evaluate_zextd(c, ops[2], ty, cxt)?;
            (on_true == on_false).then_some(on_false)
        }
        Opcode::Phi => {
            let mut incoming = pattern::phi_incoming(inst).map(|(value, _)| value);
            let to_clear = can_evaluate_zextd(c, incoming.next()?, ty, cxt)?;
            incoming.all(|value| can_evaluate_zextd(c, value, ty, cxt) == Some(to_clear)).then_some(to_clear)
        }
        // DEVIATE 1: llvm.vscale.
        _ => None,
    }
}

/// `EvaluateInDifferentTypeImpl` (:38): the instructions that compute `v` in
/// `ty`, built before the ones they replace.
fn evaluate_in_different_type_impl(
    c: &mut Combiner,
    v: Operand,
    ty: TypeId,
    is_signed: bool,
    processed: &mut HashMap<Operand, Operand>,
) -> Operand {
    // The same node may come by several paths; it gets one replacement.
    if let Some(&done) = processed.get(&v) {
        return done;
    }
    if let Operand::Constant(id) = v {
        let folded = constantfolding::constant_fold_integer_cast(c.context, c.env.layout, id, ty, is_signed)
            .expect("an immediate integer constant casts");
        return Operand::Constant(folded);
    }
    let i = def_of(c, v).expect("an instruction");
    let (opcode, flags, operands, old_ty) = {
        let inst = c.function.instruction(i);
        (inst.opcode.clone(), inst.flags, inst.operands.clone(), inst.ty)
    };
    let (new_opcode, new_flags, new_operands) = match opcode {
        Opcode::Binary(op) => {
            let lhs = evaluate_in_different_type_impl(c, operands[0], ty, is_signed, processed);
            let rhs = evaluate_in_different_type_impl(c, operands[1], ty, is_signed, processed);
            let exact = matches!(op, BinaryOp::LShr | BinaryOp::AShr) && flags.contains(Flags::EXACT);
            (Opcode::Binary(op), if exact { Flags::EXACT } else { Flags::default() }, vec![lhs, rhs])
        }
        Opcode::Cast(cast @ (CastOp::Trunc | CastOp::ZExt | CastOp::SExt)) => {
            // If the source type of the cast is the type we're trying for then
            // we can just return the source: it is not new.
            if c.type_of(operands[0]) == ty {
                return operands[0];
            }
            // Otherwise, must be the same type of cast, so just reinsert a new
            // one. This also handles the case of zext(trunc(x)) -> zext(x).
            let to = bits_of(c, ty);
            let new = int_cast_op(width(c, operands[0]), to, cast == CastOp::SExt);
            let mut new_flags = Flags::default();
            if cast == CastOp::Trunc {
                if new == CastOp::Trunc && bits_of(c, old_ty) <= to {
                    new_flags = wrap_flags(flags.contains(Flags::NUW), flags.contains(Flags::NSW));
                } else if new == CastOp::ZExt && flags.contains(Flags::NUW) {
                    new_flags.insert(Flags::NNEG);
                }
            }
            (Opcode::Cast(new), new_flags, vec![operands[0]])
        }
        Opcode::Select => {
            let on_true = evaluate_in_different_type_impl(c, operands[1], ty, is_signed, processed);
            let on_false = evaluate_in_different_type_impl(c, operands[2], ty, is_signed, processed);
            (Opcode::Select, Flags::default(), vec![operands[0], on_true, on_false])
        }
        Opcode::Phi => {
            let mut new_operands = Vec::with_capacity(operands.len());
            for pair in operands.chunks(2) {
                new_operands.push(evaluate_in_different_type_impl(c, pair[0], ty, is_signed, processed));
                new_operands.push(pair[1]);
            }
            (Opcode::Phi, Flags::default(), new_operands)
        }
        Opcode::Cast(cast @ (CastOp::FPToUI | CastOp::FPToSI)) => {
            (Opcode::Cast(cast), Flags::default(), vec![operands[0]])
        }
        // DEVIATE 4: calls (llvm.abs and the min/max intrinsics).
        other => unreachable!("`can_evaluate` accepted {other:?}"),
    };
    let new = c.insert_before(i, new_opcode, ty, new_operands, new_flags);
    let result = value_of(c, new);
    // There is no need to keep track of the old and new value of a one-use
    // node: we came from its only user.
    if !shared::has_one_use(c, v) {
        processed.insert(v, result);
    }
    result
}

/// `EvaluateInDifferentType` (:191): given an expression that `canEvaluate*`
/// accepted, the code that evaluates it in `ty`.
fn evaluate_in_different_type(
    c: &mut Combiner,
    v: Operand,
    ty: TypeId,
    is_signed: bool,
) -> Operand {
    evaluate_in_different_type_impl(c, v, ty, is_signed, &mut HashMap::default())
}

/// `InstCombinerImpl::isEliminableCastPair` (:198): the one cast that is the
/// pair `first`, `second`, if there is one.
fn is_eliminable_cast_pair(
    c: &Combiner,
    first: InstId,
    second: InstId,
) -> Option<CastOp> {
    let (Opcode::Cast(first_op), Opcode::Cast(second_op)) =
        (c.function.instruction(first).opcode.clone(), c.function.instruction(second).opcode.clone())
    else {
        return None;
    };
    let src = c.type_of(c.function.instruction(first).operands[0]);
    let mid = c.function.instruction(first).ty;
    let dst = c.function.instruction(second).ty;
    let types = &c.context.types;
    let int_ptr = |ty: TypeId| match types.get(ty) {
        Type::Pointer(space) => Some(types.int(c.env.layout.pointer(*space).bits)),
        _ => None,
    };
    let result = constantfolding::is_eliminable_cast_pair(c.context, c.env.layout, first_op, second_op, src, mid, dst)?;
    // We don't want to form an inttoptr or ptrtoint that converts to an
    // integer type that differs from the pointer size.
    if (result == CastOp::IntToPtr && Some(src) != int_ptr(dst))
        || (result == CastOp::PtrToInt && Some(dst) != int_ptr(src))
    {
        return None;
    }
    Some(result)
}

/// `commonCastTransforms` (:223): the transforms common to all cast visitors.
pub fn common_cast_transforms(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let Opcode::Cast(_) = c.function.instruction(inst).opcode else { return None };
    let src = c.function.instruction(inst).operands[0];
    let ty = c.function.instruction(inst).ty;
    {
        let env = c.env;
        if let Some(found) =
            simplify_instruction_in(c.context, env.globals, env.layout, Some(&env.analyses), c.function, inst)
            && Some(found) != c.function.instruction(inst).result.map(Operand::Value)
        {
            return Some(Visit::Replace(found));
        }
    }
    // Try to eliminate a cast of a cast: A->B->C.
    if let Some(inner) = def_of(c, src)
        && matches!(c.function.instruction(inner).opcode, Opcode::Cast(_))
        && let Some(new_op) = is_eliminable_cast_pair(c, inner, inst)
    {
        let from = c.function.instruction(inner).operands[0];
        return Some(c.replace_with_new(inst, Opcode::Cast(new_op), ty, vec![from], Flags::default()));
    }
    // We are casting a select. Try to fold the cast into the select if the
    // select does not have a compare instruction with matching operand types
    // or the select is likely better done in a narrow type. Creating a select
    // with operands that are different sizes than its condition may inhibit
    // other folds and lead to worse codegen.
    if let Some(select) = def_of(c, src).filter(|&def| c.function.instruction(def).opcode == Opcode::Select) {
        let cmp = def_of(c, c.function.instruction(select).operands[0])
            .filter(|&def| matches!(
                c.function.instruction(def).opcode,
                Opcode::ICmp(_) | Opcode::FCmp(_)
            ));
        let compares_select_type =
            cmp.is_some_and(|cmp| c.type_of(c.function.instruction(cmp).operands[0]) == c.type_of(src));
        let narrows = matches!(
            c.function.instruction(inst).opcode,
            Opcode::Cast(CastOp::Trunc)
        )
            && matches!(
                (c.context.types.int_bits(c.type_of(src)), c.context.types.int_bits(ty)),
                (Some(from), Some(to)) if should_change_type(c, from, to)
            );
        if !compares_select_type || narrows {
            let found = c.fold_op_into_select(inst, select, false, false);
            if !matches!(found, Visit::None) {
                return Some(found);
            }
        }
    }

    // If we are casting a PHI, then fold the cast into the PHI.
    if let Some(phi) = def_of(c, src)
        && c.function.instruction(phi).opcode == Opcode::Phi
    {
        let (from, to) = (c.context.types.int_bits(c.type_of(src)), c.context.types.int_bits(ty));
        // Not if it creates a phi with an illegal type from a legal one.
        if let (Some(from), Some(to)) = (from, to)
            && !should_change_type(c, from, to)
        {
            // DEVIATE 1: the unary shuffle step follows here.
            return None;
        }
        return match c.fold_op_into_phi(inst, phi, false) {
            Visit::None => None,
            found => Some(found),
        };
    }
    // DEVIATE 1: cast (shuffle X, Mask) --> shuffle (cast X), Mask.
    None
}

/// `narrowBinOp` (:914): pull a truncate ahead of a math or bitwise logic
/// instruction.
fn narrow_bin_op(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let src = c.function.instruction(inst).operands[0];
    let dest_ty = c.function.instruction(inst).ty;
    let (src_w, dest_w) = (width(c, src), bits_of(c, dest_ty));
    if !should_change_type(c, src_w, dest_w) {
        return None;
    }
    if !shared::has_one_use(c, src) {
        return None;
    }
    let (op, op0, op1, old_flags) = {
        let (_, binary) = query(c, Some(inst)).instruction_of(src)?;
        let Opcode::Binary(op) = binary.opcode else { return None };
        let [a, b] = binary.operands[..] else { return None };
        (op, a, b, binary.flags)
    };
    match op {
        BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
            // trunc (binop C, X) --> binop (trunc C', X)
            if matches!(op0, Operand::Constant(_))
                && let Some(narrow_c) = fold_trunc(c, op0, dest_ty)
            {
                let trunc_x = c.create_cast(inst, CastOp::Trunc, op1, dest_ty, Flags::default());
                return Some(c.replace_with_binop(inst, op, narrow_c, trunc_x, Flags::default()));
            }
            // trunc (binop X, C) --> binop (trunc X, C')
            if matches!(op1, Operand::Constant(_))
                && let Some(narrow_c) = fold_trunc(c, op1, dest_ty)
            {
                let trunc_x = c.create_cast(inst, CastOp::Trunc, op0, dest_ty, Flags::default());
                return Some(c.replace_with_binop(inst, op, trunc_x, narrow_c, Flags::default()));
            }
            let (x0, x1) = {
                let q = query(c, Some(inst));
                (ext_from(c, &q, op0, dest_ty), ext_from(c, &q, op1, dest_ty))
            };
            // trunc (binop (ext X), Y) --> binop X, (trunc Y)
            if let Some(x) = x0 {
                let narrow = c.create_cast(inst, CastOp::Trunc, op1, dest_ty, Flags::default());
                return Some(c.replace_with_binop(inst, op, x, narrow, Flags::default()));
            }
            // trunc (binop Y, (ext X)) --> binop (trunc Y), X
            if let Some(x) = x1 {
                let narrow = c.create_cast(inst, CastOp::Trunc, op0, dest_ty, Flags::default());
                return Some(c.replace_with_binop(inst, op, narrow, x, Flags::default()));
            }
        }
        BinaryOp::LShr | BinaryOp::AShr => {
            // trunc (*shr (trunc A), C) --> trunc(*shr A, C)
            let found = {
                let q = query(c, Some(inst));
                pattern::cast(&q, op0, CastOp::Trunc).map(|(_, _, a)| a).zip(q.int_constant(op1))
            };
            if let Some((a, (amount, _))) = found {
                // If the shift is small enough, all zero/sign bits created by
                // the shift are removed by the trunc.
                if amount <= u128::from(src_w - dest_w)
                    && let Operand::Constant(amount_c) = op1
                    && let Some(wide) = constantfolding::constant_fold_integer_cast(
                        c.context,
                        c.env.layout,
                        amount_c,
                        c.type_of(a),
                        true,
                    )
                {
                    let exact = if old_flags.contains(Flags::EXACT) { Flags::EXACT } else { Flags::default() };
                    let shift = c.create_binop(inst, op, a, Operand::Constant(wide), exact);
                    return Some(replace_with_int_cast(c, inst, shift, dest_ty, false, Flags::default()));
                }
            }
        }
        _ => {}
    }
    // DEVIATE 4: narrowFunnelShift (fshl, fshr).
    None
}

/// `visitTrunc`'s steps for a truncate to `i1` (:1102).
fn trunc_to_bool(
    c: &mut Combiner,
    inst: InstId,
    src: Operand,
    flags: Flags,
) -> Option<Visit> {
    let src_ty = c.type_of(src);
    let src_w = bits_of(c, src_ty);
    let wrapped = flags.contains(Flags::NUW) || flags.contains(Flags::NSW);
    let zero = c.int(src_ty, 0);
    // trunc ((C1 << X) >> C2) to i1 --> X == (C2 - cttz(C1)), where C1 is pow2
    let shifted = {
        let q = query(c, Some(inst));
        if shared::has_one_use(c, src) {
            shr(&q, src).and_then(|(shl, c2, _)| {
                let (c1, x, _) = binop(&q, shl, BinaryOp::Shl)?;
                let c1 = pattern::power2_constant(&q, c1)?;
                pattern::imm_constant(&q, c2).then_some((x, c2, c1))
            })
        } else {
            None
        }
    };
    if let Some((x, c2, c1)) = shifted {
        let log2 = c.int(src_ty, u128::from(c1.trailing_zeros()));
        if let Some(cmp_c) = fold_constants(c, BinaryOp::Sub, c2, log2) {
            return Some(replace_with_icmp(c, inst, IntPredicate::Eq, x, cmp_c));
        }
    }
    // trunc (ashr X, BW-1) to i1 --> icmp slt X, 0 (and for lshr)
    let sign = {
        let q = query(c, Some(inst));
        shr(&q, src).filter(|&(_, amount, _)| pattern::apint(&q, amount) == Some((u128::from(src_w - 1), src_w)))
    };
    if let Some((x, ..)) = sign {
        return Some(replace_with_icmp(c, inst, IntPredicate::Slt, x, zero));
    }
    // trunc (lshr X, C) to i1 --> icmp ne (and X, C'), 0
    let lshr = {
        let q = query(c, Some(inst));
        if shared::has_one_use(c, src) {
            binop(&q, src, BinaryOp::LShr).filter(|&(_, amount, _)| pattern::imm_constant(&q, amount))
        } else {
            None
        }
    };
    if let Some((x, amount, _)) = lshr {
        let one = c.int(src_ty, 1);
        let mask_c = c.create_shl(inst, one, amount);
        let and = c.create_binop(inst, BinaryOp::And, x, mask_c, Flags::default());
        return Some(replace_with_icmp(c, inst, IntPredicate::Ne, and, zero));
    }
    // trunc (or (lshr X, C), X) to i1 --> icmp ne (and X, C'), 0
    let or = {
        let q = query(c, Some(inst));
        if shared::has_one_use(c, src) {
            pattern::binary(&q, src, BinaryOp::Or).and_then(|(_, _, a, b)| {
                let lshr_of = |lhs: Operand, other: Operand| {
                    binop(&q, lhs, BinaryOp::LShr)
                        .filter(|&(x, amount, _)| x == other && pattern::imm_constant(&q, amount))
                };
                lshr_of(a, b).or_else(|| lshr_of(b, a))
            })
        } else {
            None
        }
    };
    if let Some((x, amount, _)) = or {
        let one = c.int(src_ty, 1);
        let mask_c = c.create_shl(inst, one, amount);
        let both = c.create_binop(inst, BinaryOp::Or, mask_c, one, Flags::default());
        let and = c.create_binop(inst, BinaryOp::And, x, both, Flags::default());
        return Some(replace_with_icmp(c, inst, IntPredicate::Ne, and, zero));
    }
    // trunc (C << X) to i1 --> X == 0, where C is odd
    let odd = {
        let q = query(c, Some(inst));
        binop(&q, src, BinaryOp::Shl).filter(|&(a, ..)| q.int_constant(a).is_some_and(|(bits, _)| bits & 1 == 1))
    };
    if let Some((_, x, _)) = odd {
        let zero = c.int(c.type_of(x), 0);
        return Some(replace_with_icmp(c, inst, IntPredicate::Eq, x, zero));
    }
    if wrapped {
        let xor = {
            let q = query(c, Some(inst));
            binop(&q, src, BinaryOp::Xor)
        };
        if let Some((x, y, _)) = xor {
            return Some(replace_with_icmp(c, inst, IntPredicate::Ne, x, y));
        }
    }
    // trunc (usub.sat (1, X)) to i1 --> icmp eq X, 0
    let usub_sat = {
        let q = query(c, Some(inst));
        if shared::has_one_use(c, src) {
            pattern::intrinsic(&q, src, IntrinsicId::UsubSat).and_then(|(_, call)| {
                let [a, b] = pattern::call_arguments(call)[..] else { return None };
                pattern::one(&q, a).then_some(b)
            })
        } else {
            None
        }
    };
    if let Some(x) = usub_sat {
        return Some(replace_with_icmp(c, inst, IntPredicate::Eq, x, zero));
    }
    None
}

/// `InstCombinerImpl::visitTrunc` (:1045).
pub fn visit_trunc(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if too_wide(c, inst) {
        return Visit::None;
    }
    if let Some(r) = common_cast_transforms(c, inst) {
        return r;
    }
    let src = c.function.instruction(inst).operands[0];
    let dest_ty = c.function.instruction(inst).ty;
    let flags = c.function.instruction(inst).flags;
    let (nuw, nsw) = (flags.contains(Flags::NUW), flags.contains(Flags::NSW));
    let src_ty = c.type_of(src);
    let (src_w, dest_w) = (bits_of(c, src_ty), bits_of(c, dest_ty));

    // Attempt to truncate the entire input expression tree to the destination
    // type. Only if the dest type is a simple type: not i93 unless the source
    // is also strange.
    if should_change_type(c, src_w, dest_w) && can_evaluate_truncated(c, src, dest_ty, inst) {
        // Evaluating in a different type always eliminates the cast.
        let res = evaluate_in_different_type(c, src, dest_ty, false);
        return Visit::Replace(res);
    }

    // For integer types, check if we can shorten the entire input expression to
    // DestWidth * 2, which won't allow removing the truncate, but reducing the
    // width may enable further optimizations.
    if dest_w * 2 < src_w {
        let new_dest_ty = c.context.types.int(dest_w * 2);
        if should_change_type(c, src_w, dest_w * 2) && can_evaluate_truncated(c, src, new_dest_ty, inst) {
            let res = evaluate_in_different_type(c, src, new_dest_ty, false);
            return c.replace_with_new(inst, Opcode::Cast(CastOp::Trunc), dest_ty, vec![res], Flags::default());
        }
    }
    if dest_w == 1 && (nuw || nsw) {
        let exact_shr = {
            let q = query(c, Some(inst));
            shr(&q, src).filter(|&(.., f)| f.contains(Flags::EXACT)).map(|(x, ..)| x)
        };
        if let Some(x) = exact_shr {
            let zero = c.int(src_ty, 0);
            return replace_with_icmp(c, inst, IntPredicate::Ne, x, zero);
        }
    }

    // See if we can simplify any instructions used by the input whose sole
    // purpose is to compute bits we don't care about.
    let found = c.simplify_demanded_instruction_bits(inst);
    if !matches!(found, Visit::None) {
        return found;
    }

    if dest_w == 1
        && let Some(r) = trunc_to_bool(c, inst, src, flags)
    {
        return r;
    }

    // DEVIATE 4: trunc(u/smin(zext(a) + zext(b), MAX)) --> uadd.sat(a, b)
    // DEVIATE 4: trunc(smax(zext(a) - zext(b), 0)) --> usub.sat(a, b)

    let lshr_sext = {
        let q = query(c, Some(inst));
        binop(&q, src, BinaryOp::LShr).and_then(|(x, amount, f)| {
            let (_, _, a) = pattern::cast(&q, x, CastOp::SExt)?;
            let (amount, _) = q.int_constant(amount)?;
            Some((a, amount, f.contains(Flags::EXACT)))
        })
    };
    if let Some((a, amount, exact)) = lshr_sext {
        let a_w = width(c, a);
        let max_shift = src_w - dest_w.max(a_w);
        // If the shift is small enough, all zero bits created by the shift are
        // removed by the trunc.
        if amount <= u128::from(max_shift) {
            let exact = if exact { Flags::EXACT } else { Flags::default() };
            let new_amount = |c: &Combiner, w: u32| c.int(c.type_of(a), amount.min(u128::from(w - 1)));
            // trunc (lshr (sext A), C) --> ashr A, C
            if c.type_of(a) == dest_ty {
                let amount_c = new_amount(c, dest_w);
                return c.replace_with_binop(inst, BinaryOp::AShr, a, amount_c, exact);
            }
            // The types are mismatched, so create a cast after shifting:
            // trunc (lshr (sext A), C) --> sext/trunc (ashr A, C)
            if shared::has_one_use(c, src) {
                let amount_c = new_amount(c, a_w);
                let shift = c.create_binop(inst, BinaryOp::AShr, a, amount_c, exact);
                return replace_with_int_cast(c, inst, shift, dest_ty, true, Flags::default());
            }
        }
        // TODO in C++: mask high bits with 'and'.
    }

    if let Some(r) = narrow_bin_op(c, inst) {
        return r;
    }

    // DEVIATE 1: shrinkSplatShuffle, shrinkInsertElt.

    if shared::has_one_use(c, src) && should_change_type(c, src_w, dest_w) {
        // Transform "trunc (shl X, cst)" -> "shl (trunc X), cst" so long as the
        // dest type is native and cst < dest size. Skip shifts of shift by
        // constants: it undoes a combine in FoldShiftByConstant and is the
        // extend in reg pattern.
        let shl = {
            let q = query(c, Some(inst));
            binop(&q, src, BinaryOp::Shl).and_then(|(a, amount, f)| {
                if shr(&q, a).is_some_and(|(_, y, _)| matches!(y, Operand::Constant(_))) {
                    return None;
                }
                let (bits, _) = q.int_constant(amount)?;
                (bits < u128::from(dest_w)).then_some((a, amount, f))
            })
        };
        if let Some((a, amount, wide_flags)) = shl
            && let Some(narrow_amount) = fold_trunc(c, amount, dest_ty)
        {
            // If neither the wide shift nor the truncate wrap, propagate the
            // wrap flags on the new truncate and shift.
            let wrap = wrap_flags(nuw && wide_flags.contains(Flags::NUW), nsw && wide_flags.contains(Flags::NSW));
            let new_trunc = c.create_cast(inst, CastOp::Trunc, a, dest_ty, wrap);
            return c.replace_with_binop(inst, BinaryOp::Shl, new_trunc, narrow_amount, wrap);
        }
    }

    // DEVIATE 4: trunc (select(icmp_ult(A, DestTy_umax+1), A, sext(icmp_sgt(A,
    // 0)))) --> trunc (smin(smax(0, A), DestTy_umax))
    // DEVIATE 1: foldVecTruncToExtElt, foldVecExtTruncToExtElt.
    // DEVIATE 4: trunc (ctlz_i32(zext(A), B) --> add(ctlz_i16(A, B), C)
    // DEVIATE 1: trunc (vscale).
    // DEVIATE 4: trunc(scmp(x, y)), trunc(ucmp(x, y)).

    if dest_w == 1 && (nuw || nsw) && nonzero::is_known_non_zero(&query(c, Some(inst)), src, 0) {
        return Visit::Replace(c.int(dest_ty, 1));
    }

    let mut changed = false;
    {
        let q = query(c, Some(inst));
        let fits = !nsw && compute_max_significant_bits(&q, src, 0) <= dest_w;
        let high_zero = !nuw && known::masked_value_is_zero(&q, src, mask(src_w) & !mask(dest_w), 0);
        if fits {
            set_flag(c, inst, Flags::NSW);
            changed = true;
        }
        if high_zero {
            set_flag(c, inst, Flags::NUW);
            changed = true;
        }
    }

    if dest_w == 1 {
        let shifted_constant = {
            let q = query(c, Some(inst));
            shr(&q, src).and_then(|(a, v1, _)| q.int_constant(a).map(|(bits, w)| (bits, w, v1)))
        };
        if let Some((c1, w, v1)) = shifted_constant {
            let v1_ty = c.type_of(v1);
            // trunc (OP i8 C1, V1) to i1 -> icmp eq V1, log_2(C1) iff C1 is a
            // power of 2
            if c1.is_power_of_two() {
                let right = c.int(v1_ty, u128::from(c1.trailing_zeros()));
                return replace_with_icmp(c, inst, IntPredicate::Eq, v1, right);
            }
            // ... icmp ult V1, log_2(C1 + 1) iff (C1 + 1) is a power of 2
            if c1 != 0 && c1 & c1.wrapping_add(1) == 0 {
                let right = c.int(v1_ty, u128::from(c1.trailing_ones()));
                return replace_with_icmp(c, inst, IntPredicate::Ult, v1, right);
            }
            // ... icmp ugt V1, cttz(C1) - 1 iff C1 is a negative power of 2
            if (c1.wrapping_neg() & mask(w)).is_power_of_two() {
                let right = c.int(v1_ty, u128::from(c1.trailing_zeros()));
                return replace_with_icmp(c, inst, IntPredicate::Uge, v1, right);
            }
        }
    }

    if changed { Visit::Changed } else { Visit::None }
}

/// `transformZExtICmp` (:1361): `zext (icmp ...)` as bitwise operations.
fn transform_zext_icmp(
    c: &mut Combiner,
    inst: InstId,
    cmp: InstId,
) -> Option<Visit> {
    let Opcode::ICmp(pred) = c.function.instruction(cmp).opcode else { return None };
    let [op0, op1] = c.function.instruction(cmp).operands[..] else { return None };
    let zext_ty = c.function.instruction(inst).ty;
    let op1_const = query(c, Some(inst)).int_constant(op1);
    // If we are just checking for a icmp eq of a single bit and zext'ing it to
    // an integer, then shift the bit to the appropriate place and then cast to
    // integer to avoid the comparison.
    if let Some((value, _)) = op1_const {
        // zext (x <s 0) to i32 --> x>>u31 (true if the sign bit is set).
        if pred == IntPredicate::Slt && value == 0 {
            let amount = c.int(c.type_of(op0), u128::from(width(c, op0) - 1));
            let shifted = c.create_binop(inst, BinaryOp::LShr, op0, amount, Flags::default());
            let result = create_int_cast(c, inst, shifted, zext_ty, false);
            return Some(Visit::Replace(result));
        }
        // zext (X == 0) to i32 --> X^1 iff X has only the low bit set.
        // zext (X == 0) to i32 --> (X>>1)^1 iff X has only the 2nd bit set.
        // zext (X != 0) to i32 --> X iff X has only the low bit set.
        // zext (X != 0) to i32 --> X>>1 iff X has only the 2nd bit set.
        if value == 0 && matches!(pred, IntPredicate::Eq | IntPredicate::Ne) {
            // Exactly 1 possible 1? But not the high-bit because that is
            // canonicalized to this form.
            let known = known::compute_known_bits(&query(c, Some(inst)), op0, 0);
            let possible = !known.zero & mask(width(c, op0));
            if possible.is_power_of_two() {
                let shift = possible.trailing_zeros();
                if bits_of(c, zext_ty) != shift + 1
                    && (c.type_of(op0) == zext_ty || pred == IntPredicate::Ne || shift == 0)
                {
                    let mut value = op0;
                    // Insert the shift to put the result in the low bit.
                    if shift != 0 {
                        let amount = c.int(c.type_of(value), u128::from(shift));
                        value = c.create_binop(inst, BinaryOp::LShr, value, amount, Flags::default());
                    }
                    // Toggle the low bit for "X == 0".
                    if pred == IntPredicate::Eq {
                        let one = c.int(c.type_of(value), 1);
                        value = c.create_binop(inst, BinaryOp::Xor, value, one, Flags::default());
                    }
                    return Some(Visit::Replace(create_int_cast(c, inst, value, zext_ty, false)));
                }
            }
        }
    }
    if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) {
        // Test if a bit is clear/set using a shifted-one mask:
        // zext (icmp eq (and X, (1 << ShAmt)), 0) --> and (lshr (not X),
        // ShAmt), 1 zext (icmp ne (and X, (1 << ShAmt)), 0) --> and
        // (lshr X, ShAmt), 1
        let found = {
            let q = query(c, Some(inst));
            let cmp_value = value_of(c, cmp);
            if shared::has_one_use(c, cmp_value) && pattern::zero_int(&q, op1) && shared::has_one_use(c, op0) {
                pattern::binary(&q, op0, BinaryOp::And).and_then(|(_, _, a, b)| {
                    let shifted_one = |v: Operand| {
                        let (one, amount, _) = binop(&q, v, BinaryOp::Shl)?;
                        pattern::one(&q, one).then_some(amount)
                    };
                    let (amount, x, shift) =
                        shifted_one(a).map(|s| (s, b, a)).or_else(|| shifted_one(b).map(|s| (s, a, b)))?;
                    Some((amount, x, shift))
                })
            } else {
                None
            }
        };
        if let Some((amount, x, shift)) = found
            && (zext_ty == c.type_of(op0) || pred != IntPredicate::Eq || shared::has_one_use(c, shift))
        {
            let x = if pred == IntPredicate::Eq { c.create_not(inst, x) } else { x };
            let lshr = c.create_binop(inst, BinaryOp::LShr, x, amount, Flags::default());
            let one = c.int(c.type_of(x), 1);
            let and = c.create_binop(inst, BinaryOp::And, lshr, one, Flags::default());
            return Some(Visit::Replace(create_int_cast(c, inst, and, zext_ty, false)));
        }
    }
    None
}

/// `InstCombinerImpl::visitZExt` (:1593).
pub fn visit_zext(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if too_wide(c, inst) {
        return Visit::None;
    }
    let src = c.function.instruction(inst).operands[0];
    // If this zero extend is only used by a truncate, let the truncate be
    // eliminated before we try to optimize this zext.
    if let Some(result) = c.function.instruction(inst).result
        && c.function
            .users(result)
            .only()
            .is_some_and(|one| c.function.instruction(one.user).opcode == Opcode::Cast(CastOp::Trunc))
        && !matches!(src, Operand::Constant(_))
    {
        return Visit::None;
    }
    // If one of the common conversion will work, do it.
    if let Some(r) = common_cast_transforms(c, inst) {
        return r;
    }
    // DEVIATE 1: foldExtractionOfVectorDeinterleave.
    let dest_ty = c.function.instruction(inst).ty;
    let flags = c.function.instruction(inst).flags;
    let nneg = flags.contains(Flags::NNEG);
    let src_ty = c.type_of(src);
    let (src_w, dest_w) = (bits_of(c, src_ty), bits_of(c, dest_ty));

    // zext nneg bool x -> 0
    if src_w == 1 && nneg {
        return Visit::Replace(c.int(dest_ty, 0));
    }

    // zext nneg means Src is non-negative and we can treat this as an sext.
    // Evaluating as a signed type means that any constant operands will be
    // sign-extended instead of zero-extended, so the sign bits in the final
    // result should be enough that we avoid having to clear the high bits.
    let evaluate_as_signed = nneg && can_evaluate_sextd(c, src, dest_ty);

    // Try to extend the entire expression tree to the wide destination type.
    let mut bits_to_clear = 0;
    let evaluable = should_change_type(c, src_w, dest_w)
        && (evaluate_as_signed
            || can_evaluate_zextd(c, src, dest_ty, inst).inspect(|&clear| bits_to_clear = clear).is_some());
    if evaluable {
        let res = evaluate_in_different_type(c, src, dest_ty, evaluate_as_signed);
        let kept = src_w - bits_to_clear;
        // If the high bits are already filled with zeros, just replace this
        // cast with the result. If we've evaluated as a signed expression then
        // instead check that the high bits are the sign bit, which we know is
        // zero.
        let filled = {
            let q = query(c, Some(inst));
            if evaluate_as_signed {
                compute_num_sign_bits(&q, res, 0) > dest_w - kept
            } else {
                known::masked_value_is_zero(&q, res, mask(dest_w) & !mask(kept), 0)
            }
        };
        if filled {
            return Visit::Replace(res);
        }
        // We need to emit an AND to clear the high bits.
        let keep = c.int(c.type_of(res), mask(kept));
        return c.replace_with_binop(inst, BinaryOp::And, res, keep, Flags::default());
    }

    // If this is a TRUNC followed by a ZEXT then we are dealing with integral
    // types and if the sizes are just right we can convert this into a logical
    // 'and' which will be much cheaper than the pair of casts.
    let trunc_src = {
        let q = query(c, Some(inst));
        pattern::cast(&q, src, CastOp::Trunc).map(|(_, _, a)| a)
    };
    if let Some(a) = trunc_src {
        // The intermediate type is smaller than A or C; the relation between A
        // and C is not known.
        let (src_size, mid_size, dst_size) = (width(c, a), src_w, dest_w);
        // If we're actually extending zero bits, then if
        // SrcSize <  DstSize: zext(a & mask)
        // SrcSize == DstSize: a & mask
        // SrcSize  > DstSize: trunc(a) & mask
        if src_size < dst_size {
            let and_c = c.int(c.type_of(a), mask(mid_size));
            let and = c.create_binop(inst, BinaryOp::And, a, and_c, Flags::default());
            return c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), dest_ty, vec![and], Flags::default());
        }
        if src_size == dst_size {
            let and_c = c.int(c.type_of(a), mask(mid_size));
            return c.replace_with_binop(inst, BinaryOp::And, a, and_c, Flags::default());
        }
        let trunc = c.create_cast(inst, CastOp::Trunc, a, dest_ty, Flags::default());
        let and_c = c.int(c.type_of(trunc), mask(mid_size));
        return c.replace_with_binop(inst, BinaryOp::And, trunc, and_c, Flags::default());
    }

    if let Some(cmp) = def_of(c, src)
        && matches!(c.function.instruction(cmp).opcode, Opcode::ICmp(_))
    {
        return transform_zext_icmp(c, inst, cmp).unwrap_or(Visit::None);
    }

    // zext((trunc(X) & C) ^ C) -> ((X & zext(C)) ^ zext(C)).
    let xor_trunc = {
        let q = query(c, Some(inst));
        if shared::has_one_use(c, src) {
            binop(&q, src, BinaryOp::Xor).and_then(|(and, k, _)| {
                if !matches!(k, Operand::Constant(_)) || !shared::has_one_use(c, and) {
                    return None;
                }
                let (t, k2, _) = binop(&q, and, BinaryOp::And)?;
                (k2 == k).then(|| trunc_from(c, &q, t, dest_ty).map(|x| (x, k))).flatten()
            })
        } else {
            None
        }
    };
    if let Some((x, k)) = xor_trunc {
        let zc = c.create_cast(inst, CastOp::ZExt, k, dest_ty, Flags::default());
        let and = c.create_binop(inst, BinaryOp::And, x, zc, Flags::default());
        return c.replace_with_binop(inst, BinaryOp::Xor, and, zc, Flags::default());
    }

    // zext(sub(0, trunc(X))) -> and(sub(0, X), mask)
    let neg_trunc = {
        let q = query(c, Some(inst));
        binop(&q, src, BinaryOp::Sub)
            .and_then(|(zero, t, _)| pattern::zero(&q, zero).then(|| trunc_from(c, &q, t, dest_ty)).flatten())
    };
    if let Some(x) = neg_trunc {
        let zero = c.int(dest_ty, 0);
        let neg = c.create_sub(inst, zero, x);
        let mask_c = c.int(dest_ty, mask(src_w));
        return c.replace_with_binop(inst, BinaryOp::And, neg, mask_c, Flags::default());
    }

    // If we are truncating, masking, and then zexting back to the original
    // type, that's just a mask. This is not handled by canEvaluateZextd if the
    // intermediate values have extra uses.
    // zext (and (trunc X), C) --> and X, (zext C)
    let and_trunc = {
        let q = query(c, Some(inst));
        binop(&q, src, BinaryOp::And).and_then(|(t, k, _)| {
            if !matches!(k, Operand::Constant(_)) {
                return None;
            }
            trunc_from(c, &q, t, dest_ty).map(|x| (x, k))
        })
    };
    if let Some((x, k)) = and_trunc {
        let zext_c = c.create_cast(inst, CastOp::ZExt, k, dest_ty, Flags::default());
        return c.replace_with_binop(inst, BinaryOp::And, x, zext_c, Flags::default());
    }

    let nuw_trunc = {
        let q = query(c, Some(inst));
        if shared::has_one_use(c, src) {
            bitwise_logic_with(&q, src, |v| {
                pattern::cast(&q, v, CastOp::Trunc)
                    .filter(|(_, trunc, _)| trunc.flags.contains(Flags::NUW))
                    .map(|(_, _, x)| x)
                    .filter(|&x| c.type_of(x) == dest_ty)
            })
        } else {
            None
        }
    };
    if let Some((op, x, y)) = nuw_trunc {
        let zext_y = c.create_cast(inst, CastOp::ZExt, y, dest_ty, Flags::default());
        return c.replace_with_binop(inst, op, x, zext_y, Flags::default());
    }

    // DEVIATE 1: zext (vscale).

    if !nneg {
        // If this zero extend is only used by a shift, add nneg flag.
        let shifted = c
            .function
            .instruction(inst)
            .result
            .is_some_and(
                |result| c.function
                    .users(result)
                    .only()
                    .is_some_and(
                        |one| matches!(
                            c.function.instruction(one.user).opcode,
                            Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)
                        ) && one.index == 1,
                    ),
            );
        let log2_ceil = 64 - (u64::from(dest_w) - 1).leading_zeros();
        if shifted && src_w > log2_ceil {
            set_flag(c, inst, Flags::NNEG);
            return Visit::Changed;
        }
        if known::is_known_non_negative(&query(c, Some(inst)), src, 0) {
            set_flag(c, inst, Flags::NNEG);
            return Visit::Changed;
        }
    }
    Visit::None
}

/// `transformSExtICmp` (:1774): `sext (icmp ...)` as bitwise and integer
/// operations.
fn transform_sext_icmp(
    c: &mut Combiner,
    inst: InstId,
    cmp: InstId,
) -> Option<Visit> {
    let Opcode::ICmp(pred) = c.function.instruction(cmp).opcode else { return None };
    let [op0, op1] = c.function.instruction(cmp).operands[..] else { return None };
    let sext_ty = c.function.instruction(inst).ty;
    // Don't bother if Op1 isn't of integer type.
    c.context.types.int_bits(c.type_of(op1))?;
    if pred == IntPredicate::Slt && pattern::zero_int(&query(c, Some(inst)), op1) {
        // sext (x <s 0) --> ashr x, 31 (all ones if negative)
        let amount = c.int(c.type_of(op0), u128::from(width(c, op0) - 1));
        let shifted = c.create_binop(inst, BinaryOp::AShr, op0, amount, Flags::default());
        let result = create_int_cast(c, inst, shifted, sext_ty, true);
        return Some(Visit::Replace(result));
    }
    let (value, w) = query(c, Some(inst)).int_constant(op1)?;
    // If we know that only one bit of the LHS of the icmp can be set and we
    // have an equality comparison with zero or a power of 2, we can transform
    // the icmp and sext into bitwise/integer operations.
    let cmp_value = value_of(c, cmp);
    if shared::has_one_use(c, cmp_value)
        && matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
        && (value == 0 || value.is_power_of_two())
    {
        let known = known::compute_known_bits(&query(c, Some(inst)), op0, 0);
        let possible = !known.zero & mask(w);
        if possible.is_power_of_two() {
            // If the icmp tests for a known zero bit we can constant fold it.
            if value != 0 && value != possible {
                let folded = if pred == IntPredicate::Ne { c.all_ones(sext_ty) } else { c.int(sext_ty, 0) };
                return Some(Visit::Replace(folded));
            }
            let mut in_value = op0;
            if (value != 0) == (pred == IntPredicate::Ne) {
                // sext ((x & 2^n) == 0)   -> (x >> n) - 1
                // sext ((x & 2^n) != 2^n) -> (x >> n) - 1
                let shift = possible.trailing_zeros();
                // Perform a right shift to place the desired bit in the LSB.
                if shift != 0 {
                    let amount = c.int(c.type_of(in_value), u128::from(shift));
                    in_value = c.create_binop(inst, BinaryOp::LShr, in_value, amount, Flags::default());
                }
                // At this point "In" is either 1 or 0. Subtract 1 to turn
                // {1, 0} -> {0, -1}.
                let ones = c.all_ones(c.type_of(in_value));
                in_value = c.create_add(inst, in_value, ones);
            } else {
                // sext ((x & 2^n) != 0)   -> (x << bitwidth-n) a>> bitwidth-1
                // sext ((x & 2^n) == 2^n) -> (x << bitwidth-n) a>> bitwidth-1
                let shift = possible.leading_zeros() - (128 - w);
                // Perform a left shift to place the desired bit in the MSB.
                if shift != 0 {
                    let amount = c.int(c.type_of(in_value), u128::from(shift));
                    in_value = c.create_shl(inst, in_value, amount);
                }
                // Distribute the bit over the whole bit width.
                let amount = c.int(c.type_of(in_value), u128::from(w - 1));
                in_value = c.create_binop(inst, BinaryOp::AShr, in_value, amount, Flags::default());
            }
            if c.type_of(in_value) == sext_ty {
                return Some(Visit::Replace(in_value));
            }
            return Some(replace_with_int_cast(c, inst, in_value, sext_ty, true, Flags::default()));
        }
    }
    None
}

/// `InstCombinerImpl::visitSExt` (:1915).
pub fn visit_sext(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if too_wide(c, inst) {
        return Visit::None;
    }
    // If this sign extend is only used by a truncate, let the truncate be
    // eliminated before we try to optimize this sext.
    if let Some(result) = c.function.instruction(inst).result
        && c.function
            .users(result)
            .only()
            .is_some_and(|one| c.function.instruction(one.user).opcode == Opcode::Cast(CastOp::Trunc))
    {
        return Visit::None;
    }
    if let Some(r) = common_cast_transforms(c, inst) {
        return r;
    }
    let src = c.function.instruction(inst).operands[0];
    let dest_ty = c.function.instruction(inst).ty;
    let src_ty = c.type_of(src);
    let (src_bits, dest_bits) = (bits_of(c, src_ty), bits_of(c, dest_ty));

    // If the value being extended is zero or positive, use a zext instead.
    if known::is_known_non_negative(&query(c, Some(inst)), src, 0) {
        return c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), dest_ty, vec![src], Flags::NNEG);
    }

    // Try to extend the entire expression tree to the wide destination type.
    // It is not desirable to extend expression in the trunc + sext pattern when
    // the destination type is narrower than the original (pre-trunc) type.
    let trunc_src = {
        let q = query(c, Some(inst));
        pattern::cast(&q, src, CastOp::Trunc).map(|(_, trunc, x)| (x, trunc.flags.contains(Flags::NSW)))
    };
    let should_extend_expression = !trunc_src.is_some_and(|(x, _)| width(c, x) > dest_bits);
    if should_extend_expression && should_change_type(c, src_bits, dest_bits) && can_evaluate_sextd(c, src, dest_ty) {
        let res = evaluate_in_different_type(c, src, dest_ty, true);
        // If the high bits are already filled with sign bit, just replace this
        // cast with the result.
        if compute_num_sign_bits(&query(c, Some(inst)), res, 0) > dest_bits - src_bits {
            return Visit::Replace(res);
        }
        // We need to emit a shl + ashr to do the sign extend.
        let amount = c.int(dest_ty, u128::from(dest_bits - src_bits));
        let shl = c.create_shl(inst, res, amount);
        return c.replace_with_binop(inst, BinaryOp::AShr, shl, amount, Flags::default());
    }

    if let Some((x, has_nsw)) = trunc_src {
        // If the input has more sign bits than bits truncated, then convert
        // directly to final type.
        let x_bits = width(c, x);
        if has_nsw || compute_num_sign_bits(&query(c, Some(inst)), x, 0) > x_bits - src_bits {
            let flags = if has_nsw && x_bits > dest_bits { Flags::NSW } else { Flags::default() };
            return replace_with_int_cast(c, inst, x, dest_ty, true, flags);
        }

        // If input is a trunc from the destination type, then convert into
        // shifts.
        if shared::has_one_use(c, src) && c.type_of(x) == dest_ty {
            // sext (trunc X) --> ashr (shl X, C), C
            let amount = c.int(dest_ty, u128::from(dest_bits - src_bits));
            let shl = c.create_shl(inst, x, amount);
            return c.replace_with_binop(inst, BinaryOp::AShr, shl, amount, Flags::default());
        }

        // If we are replacing shifted-in high zero bits with sign bits, convert
        // the logic shift to arithmetic shift and eliminate the cast to
        // intermediate type:
        // sext (trunc (lshr Y, C)) --> sext/trunc (ashr Y, C)
        let lshr = {
            let q = query(c, Some(inst));
            binop(&q, x, BinaryOp::LShr)
                .filter(|&(_, amount, _)| pattern::apint(&q, amount) == Some((u128::from(x_bits - src_bits), x_bits)))
        };
        if shared::has_one_use(c, src)
            && let Some((y, ..)) = lshr
        {
            let amount = c.int(c.type_of(y), u128::from(x_bits - src_bits));
            let ashr = c.create_binop(inst, BinaryOp::AShr, y, amount, Flags::default());
            return replace_with_int_cast(c, inst, ashr, dest_ty, true, Flags::default());
        }
    }

    if let Some(cmp) = def_of(c, src)
        && matches!(c.function.instruction(cmp).opcode, Opcode::ICmp(_))
    {
        return transform_sext_icmp(c, inst, cmp).unwrap_or(Visit::None);
    }

    // If the input is a shl/ashr pair of a same constant, then this is a sign
    // extension from a smaller value. If the source and dest are the same type,
    // eliminate the trunc and extend and just do shifts. For example, turn:
    //   %a = trunc i32 %i to i8
    //   %b = shl i8 %a, C
    //   %c = ashr i8 %b, C
    //   %d = sext i8 %c to i32
    // into:
    //   %a = shl i32 %i, 32-(8-C)
    //   %d = ashr i32 %a, 32-(8-C)
    let shift_pair = {
        let q = query(c, Some(inst));
        binop(&q, src, BinaryOp::AShr).and_then(|(shl, ca, _)| {
            let (inner, ba, _) = binop(&q, shl, BinaryOp::Shl)?;
            let a = trunc_from(c, &q, inner, dest_ty)?;
            // BA->isElementWiseEqual(CA), DEVIATE 5: an integer amount.
            let (amount, w) = q.int_constant(ca)?;
            (ba == ca).then_some((a, amount, w))
        })
    };
    if let Some((a, amount, w)) = shift_pair {
        let wide_amount = signed(amount, w) as u128 & mask(dest_bits);
        let low_bits_left = u128::from(src_bits).wrapping_sub(wide_amount) & mask(dest_bits);
        let new_amount = u128::from(dest_bits).wrapping_sub(low_bits_left) & mask(dest_bits);
        let amount_c = c.int(dest_ty, new_amount);
        let shl = c.create_shl(inst, a, amount_c);
        return c.replace_with_binop(inst, BinaryOp::AShr, shl, amount_c, Flags::default());
    }

    // Splatting a bit of constant-index across a value:
    // sext (ashr (trunc iN X to iM), M-1) to iN --> ashr (shl X, N-M), N-1
    // If the dest type is different, use a cast (adjust use check).
    let splat = {
        let q = query(c, Some(inst));
        if shared::has_one_use(c, src) {
            binop(&q, src, BinaryOp::AShr).and_then(|(t, amount, _)| {
                if pattern::apint(&q, amount) != Some((u128::from(src_bits - 1), src_bits)) {
                    return None;
                }
                pattern::cast(&q, t, CastOp::Trunc).map(|(_, _, x)| (x, t))
            })
        } else {
            None
        }
    };
    if let Some((x, t)) = splat {
        let x_ty = c.type_of(x);
        let x_bits = bits_of(c, x_ty);
        let shl_amount = c.int(x_ty, u128::from(x_bits - src_bits));
        let ashr_amount = c.int(x_ty, u128::from(x_bits - 1));
        if x_ty == dest_ty {
            let shl = c.create_shl(inst, x, shl_amount);
            return c.replace_with_binop(inst, BinaryOp::AShr, shl, ashr_amount, Flags::default());
        }
        if shared::has_one_use(c, t) {
            let shl = c.create_shl(inst, x, shl_amount);
            let ashr = c.create_binop(inst, BinaryOp::AShr, shl, ashr_amount, Flags::default());
            return replace_with_int_cast(c, inst, ashr, dest_ty, true, Flags::default());
        }
    }

    // DEVIATE 1: sext (vscale).
    // DEVIATE 4: sext(scmp(x, y)), sext(ucmp(x, y)).

    let nsw_trunc = {
        let q = query(c, Some(inst));
        if shared::has_one_use(c, src) {
            bitwise_logic_with(&q, src, |v| {
                pattern::cast(&q, v, CastOp::Trunc)
                    .filter(|(_, trunc, _)| trunc.flags.contains(Flags::NSW))
                    .map(|(_, _, x)| x)
                    .filter(|&x| c.type_of(x) == dest_ty)
            })
        } else {
            None
        }
    };
    if let Some((op, x, y)) = nsw_trunc {
        let sext_y = c.create_cast(inst, CastOp::SExt, y, dest_ty, Flags::default());
        return c.replace_with_binop(inst, op, x, sext_y, Flags::default());
    }
    Visit::None
}
