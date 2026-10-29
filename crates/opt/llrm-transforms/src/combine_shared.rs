//! What InstCombine's integer `visit*` functions share from
//! `InstructionCombining.cpp`: the complexity order that puts constants on the
//! right, and the reassociation of an associative or commutative operation.
//!
//! Translations at d1106deb71cc:
//! [`getComplexity`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/InstCombine/InstCombiner.h#L166),
//! [`matchSymmetricPair`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1320),
//! [`simplifyAssocCastAssoc`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L415),
//! [`SimplifyAssociativeOrCommutative`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L500),
//! `maintainNoSignedWrap` (:365) and `replaceOperand`
//! (InstCombineInternal.h).
//!
//! MIRROR: the order of the steps and each step's condition, one for one.
//!
//! DEVIATE 1: `matchSymmetricPair` matches a `select` pair and a min/max pair
//! only; the `phi` pair (`matchSymmetricPhiNodesPair`) belongs to the phi
//! family, whose helpers the `combine_phi` translation owns. Until it lands a
//! pair of phis is no pair.
//!
//! MIRROR: `Instruction::isAssociative` is the integer operations and `fadd`
//! and `fmul` with `reassoc` and `nsz` (`combine_phi::is_associative`); `fadd`
//! and `fmul` are commutative and ordered by complexity.
//!
//! DEVIATE 3: a new instruction does not take the name of the one it stands
//! for (`takeName`): MIR names are not read by any pass.
//!
//! The functions are called by the `visit*` translations; the dispatch in
//! `combine::visit` calls them when the family's slot is flipped.
//!
//! # How it works
//! `get_complexity` ranks a value (undef 0, constant 1, cast or neg or not 2,
//! else 3); a commutative operation lists the lower rank on the right.
//! `simplify_associative_or_commutative` then tries, in LLVM's order, to
//! reassociate through an inner operation of the same opcode wherever the
//! operation of two of the three operands simplifies, and to join two inner
//! operations that each carry a constant. Every step that changes the
//! operation loops again.
//!
//! # Used by
//! `combine_addsub` (`visitAdd`, `visitSub`) and the families after it.
//!
//! # Example
//! A constant moves to the right, then `(x + 1) + 2` reassociates to `x + 3`:
//!
//! ```text
//! %a = add i32 1, %x    ; %a = add i32 %x, 1
//! %b = add i32 %a, 2    ; %b = add i32 %x, 3
//! ```

use llrm_mir::constantfolding;
use llrm_mir::context::{Constant, ConstantId, ConstantKind};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::simplify::{simplify_binop_at, simplify_instruction_in};
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::{Query, pattern};

use crate::combine::{Combiner, Visit, handle_use_count_decrement};
use crate::combine_phi::is_associative;

/// A ValueTracking query over the body being combined, asked at `cxt`
/// (`SQ.getWithInstruction`).
pub fn query<'a>(
    c: &'a Combiner,
    cxt: Option<InstId>,
) -> Query<'a> {
    Query {
        globals: c.env.globals,
        metadata: c.env.metadata,
        dt: Some(c.env.analyses.dominator_tree()),
        assumptions: Some(c.env.analyses.assumption_cache()),
        dom_conditions: c.env.analyses.dom_conditions(),
        cxt,
        ..Query::bare(c.context, c.function, c.env.layout)
    }
}

/// `InstCombiner::getComplexity`: 0 for undef, 1 for a constant, 2 for a cast
/// and a (f)neg or not, 3 for the rest.
pub fn get_complexity(
    q: &Query,
    v: Operand,
) -> u32 {
    if let Operand::Constant(id) = v {
        return if matches!(
            q.context().get(id).kind,
            ConstantKind::Undef | ConstantKind::Poison
        ) { 0 } else { 1 };
    }
    let is_cast = q.instruction_of(v).is_some_and(|(_, instruction)| matches!(instruction.opcode, Opcode::Cast(_)));
    if is_cast || pattern::neg(q, v).is_some() || pattern::not(q, v).is_some() || pattern::fneg(q, v).is_some() {
        return 2;
    }
    3
}

/// `simplify*Inst(..., SQ.getWithInstruction(&I))` at the head of a `visit*`:
/// what the instruction simplifies to, as `replaceInstUsesWith`.
pub fn simplify_visited(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let env = c.env;
    let found = simplify_instruction_in(c.context, env.globals, env.layout, Some(&env.analyses), c.function, inst)?;
    (Some(found) != c.function.instruction(inst).result.map(Operand::Value)).then_some(Visit::Replace(found))
}

/// `replaceOperand`: operand `index` of `inst` reads `with`, and what it read
/// is looked at again.
pub fn replace_operand(
    c: &mut Combiner,
    inst: InstId,
    index: usize,
    with: Operand,
) {
    let old = c.function.instruction(inst).operands[index];
    c.function.set_operand(inst, index, with);
    handle_use_count_decrement(&mut c.worklist, c.function, old);
}

/// `InstCombinerImpl::matchSymmetricPair`: the two values a pair of the same
/// opcode swap between them, `select c, a, b` and `select c, b, a`, or
/// `smin(a, b)` and `smax(a, b)`.
pub fn match_symmetric_pair(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> Option<(Operand, Operand)> {
    let ((_, left), (_, right)) = (q.instruction_of(lhs)?, q.instruction_of(rhs)?);
    if std::mem::discriminant(&left.opcode) != std::mem::discriminant(&right.opcode) {
        return None;
    }
    match left.opcode {
        // DEVIATE 1: the phi pair is the phi family's.
        Opcode::Phi => None,
        Opcode::Select => {
            let [cond, on_true, on_false] = left.operands[..] else { return None };
            let [other_cond, other_true, other_false] = right.operands[..] else { return None };
            (cond == other_cond && on_true == other_false && on_false == other_true).then_some((on_true, on_false))
        }
        Opcode::Call(_) => {
            let (l, r) = (min_max(q, lhs)?, min_max(q, rhs)?);
            let swapped = l.0 == r.0.swapped();
            let same_order = l.1 == r.1 && l.2 == r.2;
            let reversed = l.1 == r.2 && l.2 == r.1;
            (swapped && (same_order || reversed)).then_some((l.1, l.2))
        }
        _ => None,
    }
}

/// A `MinMaxIntrinsic`: its comparison predicate and operands.
fn min_max(
    q: &Query,
    v: Operand,
) -> Option<(llrm_mir::opcode::IntPredicate, Operand, Operand)> {
    use llrm_mir::opcode::IntPredicate;
    use pattern::IntrinsicId;
    let (inst, instruction) = q.instruction_of(v)?;
    let predicate = match pattern::call_intrinsic(q, inst)? {
        IntrinsicId::Smax => IntPredicate::Sgt,
        IntrinsicId::Smin => IntPredicate::Slt,
        IntrinsicId::Umax => IntPredicate::Ugt,
        IntrinsicId::Umin => IntPredicate::Ult,
        _ => return None,
    };
    let [a, b] = pattern::call_arguments(instruction)[..] else { return None };
    Some((predicate, a, b))
}

/// `Instruction::isCommutative`.
pub fn is_commutative(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::FAdd | BinaryOp::FMul
    )
}

/// `isBitwiseLogicOp`.
fn is_bitwise_logic(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
}

/// The operation, operands and flags of `v` if it is a binary operator.
fn binary_operator(
    c: &Combiner,
    v: Operand,
) -> Option<(InstId, BinaryOp, [Operand; 2], Flags)> {
    let Operand::Value(value) = v else { return None };
    let llrm_mir::module::ValueDef::Instruction(inst) = c.function.value(value).def else { return None };
    c.function.parent(inst)?;
    let instruction = c.function.instruction(inst);
    let Opcode::Binary(op) = instruction.opcode else { return None };
    let [a, b] = instruction.operands[..] else { return None };
    Some((inst, op, [a, b], instruction.flags))
}

/// `InstCombinerImpl::isDesirableIntType`
/// ([:310](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L310)):
/// i8, i16 and i32, and the native widths.
fn is_desirable_int_type(
    layout: &DataLayout,
    bits: u32,
) -> bool {
    matches!(bits, 8 | 16 | 32) || layout.legal_integer(bits)
}

/// `InstCombinerImpl::shouldChangeType(FromWidth, ToWidth)`
/// ([:329](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L329)):
/// whether to compute in `to` bits what is computed in `from`.
pub fn should_change_type(
    layout: &DataLayout,
    from: u32,
    to: u32,
) -> bool {
    let from_legal = from == 1 || layout.legal_integer(from);
    let to_legal = to == 1 || layout.legal_integer(to);
    // Only shrink to a desirable width, to prevent infinite loops.
    if to < from && is_desirable_int_type(layout, to) {
        return true;
    }
    // Do not go from a legal or desirable type to an illegal one.
    if (from_legal || is_desirable_int_type(layout, from)) && !to_legal {
        return false;
    }
    // Both illegal: do not grow.
    if !from_legal && !to_legal && to > from {
        return false;
    }
    true
}

/// `m_OneUse` of a value.
pub fn has_one_use(
    c: &Combiner,
    v: Operand,
) -> bool {
    match v {
        Operand::Value(value) => c.function.users(value).has_one_use(),
        _ => false,
    }
}

impl Combiner<'_> {
    /// `Value::hasOneUse`.
    pub(crate) fn has_one_use(
        &self,
        v: Operand,
    ) -> bool {
        has_one_use(self, v)
    }

    /// `isa<UndefValue>`: `undef` or `poison`.
    pub fn is_undef_value(
        &self,
        v: Operand,
    ) -> bool {
        constant(v).is_some_and(|id| matches!(
            self.context.get(id).kind,
            ConstantKind::Undef | ConstantKind::Poison
        ))
    }

    /// `isa<PoisonValue>`.
    pub fn is_poison_value(
        &self,
        v: Operand,
    ) -> bool {
        constant(v).is_some_and(|id| self.context.get(id).kind == ConstantKind::Poison)
    }

    /// `isa<ConstantPointerNull>`.
    pub fn is_null_pointer(
        &self,
        v: Operand,
    ) -> bool {
        constant(v).is_some_and(|id| matches!(
            self.context.get(id).kind,
            ConstantKind::Null | ConstantKind::Zero
        ))
    }

    /// `PoisonValue::get(ty)`.
    pub fn poison(
        &mut self,
        ty: TypeId,
    ) -> Operand {
        Operand::Constant(self.context.constant(Constant { ty, kind: ConstantKind::Poison }))
    }
}

/// `m_Constant`.
fn constant(v: Operand) -> Option<ConstantId> {
    match v {
        Operand::Constant(id) => Some(id),
        _ => None,
    }
}

/// `maintainNoSignedWrap` (:365): `I` has `nsw` and `B op C` of two integer
/// constants does not overflow, for `add`, `sub` and `mul`.
fn maintain_no_signed_wrap(
    c: &Combiner,
    flags: Flags,
    op: BinaryOp,
    b: Operand,
    c_operand: Operand,
) -> bool {
    if !flags.contains(Flags::NSW) {
        return false;
    }
    let q = query(c, None);
    let (Some((b, width)), Some((cc, _))) = (pattern::apint(&q, b), pattern::apint(&q, c_operand)) else {
        return false;
    };
    let signed = |bits: u128| llrm_mir::context::signed(bits, width);
    let (x, y) = (signed(b), signed(cc));
    let lo = -(1_i128 << (width - 1).min(126));
    let hi = (1_i128 << (width - 1).min(126)) - 1;
    let wide = match op {
        BinaryOp::Add => x.checked_add(y),
        BinaryOp::Sub => x.checked_sub(y),
        BinaryOp::Mul => x.checked_mul(y),
        _ => return false,
    };
    wide.is_some_and(|v| width > 126 || (lo..=hi).contains(&v))
}

/// `simplifyAssocCastAssoc` (:415): `(op (zext (op X, C2)), C1)` becomes
/// `(op (zext X), (op C1, zext C2))`.
fn simplify_assoc_cast_assoc(
    c: &mut Combiner,
    bin_op1: InstId,
) -> bool {
    let instruction = c.function.instruction(bin_op1);
    let Opcode::Binary(assoc) = instruction.opcode else { return false };
    let [first, second] = instruction.operands[..] else { return false };
    let Some((cast_inst, cast_instruction)) = ({
        let q = query(c, None);
        q.instruction_of(first)
    }) else {
        return false;
    };
    if !has_one_use(c, first) {
        return false;
    }
    // TODO in C++: other casts and other operations.
    if cast_instruction.opcode != Opcode::Cast(CastOp::ZExt) || !is_bitwise_logic(assoc) {
        return false;
    }
    let cast_source = cast_instruction.operands[0];
    let Some((bin_op2, op2, [x, c2_operand], _)) = binary_operator(c, cast_source) else { return false };
    if !has_one_use(c, cast_source) || op2 != assoc {
        return false;
    }
    let (Some(c1), Some(c2)) = (constant(second), constant(c2_operand)) else { return false };
    let _ = bin_op2;
    let env = constantfolding::Env { globals: c.env.globals, layout: c.env.layout };
    let dest = c.context.get(c1).ty;
    let Some(cast_c2) = constantfolding::constant_fold_cast_operand(c.context, c.env.layout, CastOp::ZExt, c2, dest)
    else {
        return false;
    };
    let Some(folded) = constantfolding::constant_fold_binary_op_operands(c.context, env, assoc, c1, cast_c2) else {
        return false;
    };
    replace_operand(c, cast_inst, 0, x);
    replace_operand(c, bin_op1, 1, Operand::Constant(folded));
    drop_poison_generating_flags(c, bin_op1);
    drop_poison_generating_flags(c, cast_inst);
    true
}

/// `Instruction::dropPoisonGeneratingFlags`.
pub fn drop_poison_generating_flags(
    c: &mut Combiner,
    inst: InstId,
) {
    let flags = c.function.instruction(inst).flags;
    let kept = flags.without(Flags::POISON_GENERATING);
    if kept != flags {
        c.function.set_flags(inst, kept);
    }
}

/// Sets `flag` on `inst` to `on` (`setHasNoUnsignedWrap`,
/// `setHasNoSignedWrap`).
pub(crate) fn set_flag(
    c: &mut Combiner,
    inst: InstId,
    flag: Flags,
    on: bool,
) {
    let flags = c.function.instruction(inst).flags;
    let mut new = flags;
    if on {
        new.insert(flag);
    } else {
        new.remove(flag);
    }
    if new != flags {
        c.function.set_flags(inst, new);
    }
}

/// `simplifyBinOp(Opcode, L, R, SQ.getWithInstruction(&I))`.
pub(crate) fn simplify_binop(
    c: &Combiner,
    inst: InstId,
    op: BinaryOp,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    simplify_binop_at(c.context, c.env.globals, c.env.layout, Some(&c.env.analyses), c.function, inst, op, lhs, rhs)
}

/// `InstCombinerImpl::SimplifyAssociativeOrCommutative` (:500): orders the
/// operands of a commutative `inst` and reassociates an associative one when
/// that makes an operation simplify. Whether `inst` changed.
pub fn simplify_associative_or_commutative(
    c: &mut Combiner,
    inst: InstId,
) -> bool {
    let Opcode::Binary(opcode) = c.function.instruction(inst).opcode else { return false };
    let mut changed = false;
    loop {
        let [lhs, rhs] = c.function.instruction(inst).operands[..] else { return changed };
        let flags = c.function.instruction(inst).flags;
        // Order operands such that they are listed from right (least complex)
        // to left (most complex): constants before unary operators before
        // binary operators.
        if is_commutative(opcode) {
            let q = query(c, Some(inst));
            if get_complexity(&q, lhs) < get_complexity(&q, rhs) {
                c.function.set_operands(inst, vec![rhs, lhs]);
                changed = true;
            }
        }
        let [lhs, rhs] = c.function.instruction(inst).operands[..] else { return changed };
        if is_commutative(opcode) {
            let pair = match_symmetric_pair(&query(c, Some(inst)), lhs, rhs);
            if let Some((first, second)) = pair {
                replace_operand(c, inst, 0, first);
                replace_operand(c, inst, 1, second);
                changed = true;
            }
        }
        let [lhs, rhs] = c.function.instruction(inst).operands[..] else { return changed };
        let op0 = binary_operator(c, lhs);
        let op1 = binary_operator(c, rhs);
        if is_associative(opcode, flags) {
            // "(A op B) op C" ==> "A op (B op C)" if "B op C" simplifies.
            if let Some((_, op, [a, b], op0_flags)) = op0
                && op == opcode
                && let Some(v) = simplify_binop(c, inst, opcode, b, rhs)
            {
                replace_operand(c, inst, 0, a);
                replace_operand(c, inst, 1, v);
                let is_nuw = flags.contains(Flags::NUW) && op0_flags.contains(Flags::NUW);
                let is_nsw = maintain_no_signed_wrap(c, flags, opcode, b, rhs) && op0_flags.contains(Flags::NSW);
                // Conservatively clear all optional flags since they may not
                // be preserved by the reassociation; reset nsw/nuw by the
                // analysis above.
                set_flag(c, inst, Flags::DISJOINT, false);
                if matches!(opcode, BinaryOp::Add | BinaryOp::Mul) {
                    set_flag(c, inst, Flags::NUW, is_nuw);
                    set_flag(c, inst, Flags::NSW, is_nsw);
                }
                changed = true;
                continue;
            }
            // "A op (B op C)" ==> "(A op B) op C" if "A op B" simplifies.
            if let Some((_, op, [b, cc], _)) = op1
                && op == opcode
                && let Some(v) = simplify_binop(c, inst, opcode, lhs, b)
            {
                replace_operand(c, inst, 0, v);
                replace_operand(c, inst, 1, cc);
                drop_poison_generating_flags(c, inst);
                changed = true;
                continue;
            }
        }
        if is_associative(opcode, flags) && is_commutative(opcode) {
            if simplify_assoc_cast_assoc(c, inst) {
                changed = true;
                continue;
            }
            // "(A op B) op C" ==> "(C op A) op B" if "C op A" simplifies.
            if let Some((_, op, [a, b], _)) = op0
                && op == opcode
                && let Some(v) = simplify_binop(c, inst, opcode, rhs, a)
            {
                replace_operand(c, inst, 0, v);
                replace_operand(c, inst, 1, b);
                drop_poison_generating_flags(c, inst);
                changed = true;
                continue;
            }
            // "A op (B op C)" ==> "B op (C op A)" if "C op A" simplifies.
            if let Some((_, op, [b, cc], _)) = op1
                && op == opcode
                && let Some(v) = simplify_binop(c, inst, opcode, cc, lhs)
            {
                replace_operand(c, inst, 0, b);
                replace_operand(c, inst, 1, v);
                drop_poison_generating_flags(c, inst);
                changed = true;
                continue;
            }
            // "(A op C1) op (B op C2)" ==> "(A op B) op (C1 op C2)" if C1 and
            // C2 are constants.
            if let (Some((i0, op0_op, [a, c1], _)), Some((i1, op1_op, [b, c2], _))) = (op0, op1)
                && op0_op == opcode
                && op1_op == opcode
                && has_one_use(c, lhs)
                && has_one_use(c, rhs)
                && let (Some(c1), Some(c2)) = (constant(c1), constant(c2))
            {
                let env = constantfolding::Env { globals: c.env.globals, layout: c.env.layout };
                if let Some(c_res) = constantfolding::constant_fold_binary_op_operands(c.context, env, opcode, c1, c2) {
                    let nuw_of = |c: &Combiner, i: InstId| c.function.instruction(i).flags.contains(Flags::NUW);
                    let is_nuw = flags.contains(Flags::NUW) && nuw_of(c, i0) && nuw_of(c, i1);
                    let new_flags = if is_nuw && opcode == BinaryOp::Add { Flags::NUW } else { Flags::default() };
                    let ty = c.function.instruction(inst).ty;
                    let new = c.insert_before(inst, Opcode::Binary(opcode), ty, vec![a, b], new_flags);
                    let new_value = c.function.instruction(new).result.expect("a value");
                    replace_operand(c, inst, 0, Operand::Value(new_value));
                    replace_operand(c, inst, 1, Operand::Constant(c_res));
                    // Conservatively clear the optional flags, since they may
                    // not be preserved by the reassociation.
                    drop_poison_generating_flags(c, inst);
                    if is_nuw {
                        set_flag(c, inst, Flags::NUW, true);
                    }
                    changed = true;
                    continue;
                }
            }
        }
        // No further simplifications.
        return changed;
    }
}

/// `m_BinOp(op, a, b)`: the operands and flags of `v` if it is that operation.
pub fn binop(
    q: &Query,
    v: Operand,
    op: BinaryOp,
) -> Option<(Operand, Operand, Flags)> {
    pattern::binary(q, v, op).map(|(_, instruction, a, b)| (a, b, instruction.flags))
}

/// `m_c_BinOp(op, P, m_Value(Other))`: `v` is `op a, b` where `a` or `b`
/// satisfies `first`; the one that does, and the other. `a` is tried first.
pub fn commutative(
    q: &Query,
    v: Operand,
    op: BinaryOp,
    first: impl Fn(Operand) -> bool,
) -> Option<(Operand, Operand)> {
    let (a, b, _) = binop(q, v, op)?;
    if first(a) {
        Some((a, b))
    } else if first(b) {
        Some((b, a))
    } else {
        None
    }
}

/// `m_c_<op>(m_Specific(x), m_Value(Y))`: `Y`.
pub fn other_of(
    q: &Query,
    v: Operand,
    op: BinaryOp,
    x: Operand,
) -> Option<Operand> {
    commutative(q, v, op, |s| s == x).map(|(_, other)| other)
}

/// `m_SpecificInt(n)`: an integer constant equal to `n`.
pub fn specific_int(
    q: &Query,
    v: Operand,
    n: u128,
) -> bool {
    pattern::apint(q, v).is_some_and(|(bits, _)| bits == n)
}

/// The visited instruction's operands, flags and result width.
pub fn parts(
    c: &Combiner,
    inst: InstId,
) -> (Operand, Operand, Flags) {
    let instruction = c.function.instruction(inst);
    (instruction.operands[0], instruction.operands[1], instruction.flags)
}

/// The width of the integer `v` (a scalar: MIR has no vectors).
pub fn width(
    c: &Combiner,
    v: Operand,
) -> u32 {
    c.context.types.int_bits(c.type_of(v)).expect("an integer")
}

/// `ConstantExpr::getAdd` and the like of two constants: the folded constant
/// (DEVIATE 4: none where it would stay an expression).
pub fn fold_constants(
    c: &Combiner,
    op: BinaryOp,
    a: Operand,
    b: Operand,
) -> Option<Operand> {
    let (Operand::Constant(x), Operand::Constant(y)) = (a, b) else { return None };
    let env = constantfolding::Env { globals: c.env.globals, layout: c.env.layout };
    constantfolding::constant_fold_binary_op_operands(c.context, env, op, x, y).map(Operand::Constant)
}

/// The `nsw` and `nuw` of `flags`.
pub fn wrap_of(flags: Flags) -> Flags {
    let mut wrap = Flags::default();
    for flag in [Flags::NSW, Flags::NUW] {
        if flags.contains(flag) {
            wrap.insert(flag);
        }
    }
    wrap
}

/// The instruction that defines the value `v`.
pub fn inst_def(
    c: &Combiner,
    v: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = v else { return None };
    match c.function.value(value).def {
        llrm_mir::module::ValueDef::Instruction(inst) => Some(inst),
        _ => None,
    }
}

#[cfg(test)]
#[path = "combine_shared_tests.rs"]
mod tests;
