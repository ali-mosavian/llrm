//! `visitSelectInst`: the canonical forms of a `select`.
//!
//! Translation of
//! [`InstCombinerImpl::visitSelectInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4598)
//! at d1106deb71cc. `visit` has already asked `simplifySelectInst`.
//!
//! MIRROR: the folds below in the pin's order, each its own function in
//! [`FOLDS`]: a new one is a function and an entry.
//!
//! DEVIATE 1: no vectors or `shufflevector` (MIR has none):
//! `canonicalizeSelectToShuffle`, `canonicalizeScalarSelectOfVecs` and
//! `foldVectorSelect` are not made.
//!
//! DEVIATE 2 (temporary): the folds that call a helper not yet translated are
//! left out, each named where it stood: `foldSelectOfBools`,
//! `foldSelectNegNot`, `foldSelectAndOrPowerOfTwo`, the floating-point folds
//! (`fabs`, `minnum`, `canonicalize`...: need #2209),
//! `foldSelectValueEquivalence`, the rest of `foldSelectInstWithICmp`
//! (its `canonicalizeSPF` step is `combine_select_spf`),
//! `foldAddSubSelect`,
//! `foldOverflowingAddSubSelect`, `foldSetClearBits`,
//! `foldSelectZeroOrFixedOp`, `foldSelectIntrinsic`,
//! `foldSelectWithSRem`, the
//! `foldSPFofSPF`, and everything after the not-condition
//! (`foldSelectCmpBitcasts` ... `FoldSelectWithAndOrCond`).
//!
//! DEVIATE 3: `replaceInInstruction` (the arms' instructions that read the
//! condition get its value) is not made:
//! `isSafeToSpeculativelyExecuteWithVariableReplaced` is private to
//! `lazyvalueinfo.rs`.
//!
//! DEVIATE 4: when the operands of a select swap, its `prof` branch weights
//! are dropped, not swapped.
//!
//! # Example
//! ```text
//! %r = select i1 %c, i16 1, i16 0        %r = zext i1 %c to i16
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::simplify::simplify_with_op_replaced_at;
use llrm_mir::valuetracking::context::is_implied_condition;
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_phi::UB_IMPLYING;
use crate::combine_select_bit_test::{select_bit_test, select_icmp_and_and};
use crate::combine_select_bools::select_of_bools;
use crate::combine_select_ext_const::select_ext_const;
use crate::combine_select_into_op::select_into_op;
use crate::combine_select_op_op::select_op_op;
use crate::combine_select_spf::select_with_icmp;
use crate::combine_shared::{has_one_use, query, replace_operand};

/// The parts of a `select`.
pub(crate) struct Sel {
    pub cond: Operand,
    pub if_true: Operand,
    pub if_false: Operand,
    pub ty: llrm_mir::types::TypeId,
}

pub(crate) type Fold = fn(&mut Combiner, InstId, &Sel) -> Option<Visit>;

/// The rewrites in the pin's order (:4598-5429), the translated ones.
const FOLDS: &[Fold] = &[
    ctpop_guard,
    cond_known_in_arms,
    select_of_bools,
    bool_to_extension,
    select_with_icmp,
    select_icmp_and_and,
    select_bit_test,
    select_op_op,
    select_ext_const,
    select_of_gep,
    select_into_op,
    select_of_phi_condition,
    nested_select_implied,
    nested_select_merged,
    binop_between_selects,
    absorb_not_condition,
];

impl Combiner<'_> {
    fn sel_of(
        &self,
        inst: InstId,
    ) -> Option<Sel> {
        let instruction = self.function.instruction(inst);
        let [cond, if_true, if_false] = instruction.operands[..] else { return None };
        (instruction.opcode == Opcode::Select).then_some(Sel { cond, if_true, if_false, ty: instruction.ty })
    }

    /// `visitSelectInst`: the first of `FOLDS` that applies.
    pub fn visit_select(
        &mut self,
        inst: InstId,
    ) -> Visit {
        let Some(sel) = self.sel_of(inst) else { return Visit::None };
        FOLDS.iter().find_map(|fold| fold(self, inst, &sel)).unwrap_or(Visit::None)
    }

    /// `Builder.CreateLogicalAnd(a, b, "", &SI)`: `select a, b, false`.
    pub(crate) fn create_logical_and(
        &mut self,
        at: InstId,
        select: InstId,
        a: Operand,
        b: Operand,
    ) -> Operand {
        let zero = self.int(self.type_of(a), 0);
        self.select_like(at, select, [a, b, zero], Flags::default())
    }

    /// `Builder.CreateLogicalOr(a, b, "", &SI)`: `select a, true, b`.
    pub(crate) fn create_logical_or(
        &mut self,
        at: InstId,
        select: InstId,
        a: Operand,
        b: Operand,
    ) -> Operand {
        let one = self.all_ones(self.type_of(a));
        self.select_like(at, select, [a, one, b], Flags::default())
    }
}

/// `select (icmp ult X, 2), X, ctpop(X)` is `ctpop(X)`: the guard is
/// redundant, ctpop(0) is 0 and ctpop(1) is 1.
fn ctpop_guard(
    c: &mut Combiner,
    _: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let q = query(c, None);
    let (call, args) = pattern::intrinsic_call(&q, sel.if_false, "llvm.ctpop")?;
    if args != [sel.if_true] {
        return None;
    }
    let (_, _, pred, lhs, rhs) = pattern::icmp(&q, sel.cond)?;
    if pred.pred != llrm_mir::opcode::IntPredicate::Ult
        || lhs != sel.if_true
        || q.int_constant(rhs) != Some((2, q.scalar_bits(rhs)))
    {
        return None;
    }
    let flags = c.function.instruction(call).flags.without(Flags::POISON_GENERATING);
    c.function.set_flags(call, flags);
    for kind in UB_IMPLYING {
        c.function.unannotate(call, kind);
    }
    c.worklist.add(call);
    Some(Visit::Replace(sel.if_false))
}

/// The arms know the condition: an arm that simplifies with the condition
/// true (or false) is replaced.
fn cond_known_in_arms(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if matches!(sel.cond, Operand::Constant(_)) || c.context.types.int_bits(sel.ty).is_none() {
        return None;
    }
    let env = c.env;
    let ty = c.type_of(sel.cond);
    let (yes, no) = (Operand::Constant(c.context.int(ty, 1)), Operand::Constant(c.context.int(ty, 0)));
    for (slot, arm, known) in [(1, sel.if_true, yes), (2, sel.if_false, no)] {
        if let Some(simpler) = simplify_with_op_replaced_at(
            c.context,
            env.globals,
            env.layout,
            Some(&env.analyses),
            c.function,
            inst,
            arm,
            sel.cond,
            known,
        ) {
            replace_operand(c, inst, slot, simpler);
            return Some(Visit::Changed);
        }
    }
    None
}

/// `select c, 1, 0` is `zext c`, `select c, -1, 0` is `sext c`, and with the
/// arms swapped the same of `!c`.
fn bool_to_extension(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if c.context.types.int_bits(sel.ty).is_none_or(|bits| bits == 1) {
        return None;
    }
    let (one, zero, ones) = (c.int(sel.ty, 1), c.int(sel.ty, 0), c.all_ones(sel.ty));
    let same = |c: &Combiner, a: Operand, b: Operand| c.same_constant(a, b);
    let (cond, extension) = if same(c, sel.if_true, one) && same(c, sel.if_false, zero) {
        (sel.cond, CastOp::ZExt)
    } else if same(c, sel.if_true, ones) && same(c, sel.if_false, zero) {
        (sel.cond, CastOp::SExt)
    } else if same(c, sel.if_true, zero) && same(c, sel.if_false, one) {
        (c.create_not(inst, sel.cond), CastOp::ZExt)
    } else if same(c, sel.if_true, zero) && same(c, sel.if_false, ones) {
        (c.create_not(inst, sel.cond), CastOp::SExt)
    } else {
        return None;
    };
    Some(c.replace_with_new(inst, Opcode::Cast(extension), sel.ty, vec![cond], Flags::default()))
}

/// `select c, (gep p, i), p` is `gep p, (select c, i, 0)` (and with the arms
/// swapped): the GEP is made once.
fn select_of_gep(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    for (gep_arm, base, swap) in [(sel.if_true, sel.if_false, false), (sel.if_false, sel.if_true, true)] {
        let Some(gep) = c.definition_of(gep_arm).and_then(|def| c.gep_of(def)) else { continue };
        if gep.indices.len() != 1 || gep.ptr != base || !has_one_use(c, gep_arm) {
            continue;
        }
        let index = gep.indices[0];
        let zero = c.int(c.type_of(index), 0);
        let (on_true, on_false) = if swap { (zero, index) } else { (index, zero) };
        let select = c.select_like(inst, inst, [sel.cond, on_true, on_false], Flags::default());
        let operands = vec![gep.ptr, select];
        return Some(c.replace_with_new(
            inst,
            Opcode::GetElementPtr { source: gep.source },
            gep.ty,
            operands,
            gep.flags,
        ));
    }
    None
}

/// A select on a phi: the select folds into the phi.
fn select_of_phi_condition(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let phi = c.definition_of(sel.cond).filter(|&def| c.function.instruction(def).opcode == Opcode::Phi)?;
    Some(c.fold_op_into_phi(inst, phi, false)).filter(|found| !matches!(found, Visit::None))
}

/// `simplifyNestedSelectsUsingImpliedCond`: a nested select whose condition
/// the outer one decides is its arm.
fn nested_select_implied(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    for (slot, arm, cond_is_true) in [(1, sel.if_true, true), (2, sel.if_false, false)] {
        let Some((_, [inner_cond, inner_true, inner_false])) = c.select_of(arm) else { continue };
        if c.type_of(inner_cond) != c.type_of(sel.cond) {
            continue;
        }
        let q = query(c, None);
        if let Some(implied) = is_implied_condition(&q, sel.cond, inner_cond, cond_is_true, 0) {
            let value = if implied { inner_true } else { inner_false };
            replace_operand(c, inst, slot, value);
            return Some(Visit::Changed);
        }
    }
    None
}

/// `select c0, (select c1, a, b), b` is `select (c0 && c1), a, b`, and the
/// mirror `select c0, a, (select c1, a, b)` is `select (c0 || c1), a, b`.
fn nested_select_merged(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if let Some((inner, [inner_cond, inner_true, inner_false])) = c.select_of(sel.if_true)
        && c.type_of(inner_cond) == c.type_of(sel.cond)
        && has_one_use(c, sel.if_true)
    {
        let _ = inner;
        let merged = if inner_false == sel.if_false {
            Some((c.create_logical_and(inst, inst, sel.cond, inner_cond), inner_true))
        } else if inner_true == sel.if_false {
            let not = c.create_not(inst, inner_cond);
            Some((c.create_logical_and(inst, inst, sel.cond, not), inner_false))
        } else {
            None
        };
        if let Some((cond, other)) = merged {
            replace_operand(c, inst, 0, cond);
            replace_operand(c, inst, 1, other);
            c.function.set_metadata(inst, "prof", None);
            return Some(Visit::Changed);
        }
    }
    if let Some((_, [inner_cond, inner_true, inner_false])) = c.select_of(sel.if_false)
        && c.type_of(inner_cond) == c.type_of(sel.cond)
        && has_one_use(c, sel.if_false)
    {
        let merged = if inner_true == sel.if_true {
            Some((c.create_logical_or(inst, inst, sel.cond, inner_cond), inner_false))
        } else if inner_false == sel.if_true {
            let not = c.create_not(inst, inner_cond);
            Some((c.create_logical_or(inst, inst, sel.cond, not), inner_true))
        } else {
            None
        };
        if let Some((cond, other)) = merged {
            replace_operand(c, inst, 0, cond);
            replace_operand(c, inst, 2, other);
            c.function.set_metadata(inst, "prof", None);
            return Some(Visit::Changed);
        }
    }
    None
}

/// `select(C, binop(select(C, X, Y), W), Z)` is `select(C, binop(X, W), Z)`:
/// the inner select is decided by the outer (not for div/rem, which the
/// select may keep from dividing by zero).
fn binop_between_selects(
    c: &mut Combiner,
    _: InstId,
    sel: &Sel,
) -> Option<Visit> {
    for (arm, keep_true) in [(sel.if_true, true), (sel.if_false, false)] {
        if !has_one_use(c, arm) {
            continue;
        }
        let Some(def) = c.definition_of(arm) else { continue };
        let binary = c.function.instruction(def);
        let Opcode::Binary(op) = binary.opcode else { continue };
        if matches!(
            op,
            BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem
        ) {
            continue;
        }
        let operands = binary.operands.to_vec();
        for (slot, &operand) in operands.iter().enumerate() {
            let Some((_, [cond, inner_true, inner_false])) = c.select_of(operand) else { continue };
            if cond == sel.cond {
                replace_operand(c, def, slot, if keep_true { inner_true } else { inner_false });
                c.worklist.push(def);
                return Some(Visit::Changed);
            }
        }
    }
    None
}

/// `select (not c), a, b` is `select c, b, a`, except for the logical and/or
/// forms.
fn absorb_not_condition(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let q = query(c, None);
    let not_cond = pattern::not(&q, sel.cond)?;
    // `a ? b : false` and `a ? true : b` are the logical operations: keep them.
    let selected = Operand::Value(c.function.instruction(inst).result?);
    if pattern::logical_and(&q, selected).is_some() || pattern::logical_or(&q, selected).is_some() {
        return None;
    }
    replace_operand(c, inst, 0, not_cond);
    c.function.set_operand(inst, 1, sel.if_false);
    c.function.set_operand(inst, 2, sel.if_true);
    c.function.set_metadata(inst, "prof", None);
    Some(Visit::Changed)
}

#[cfg(test)]
#[path = "combine_visit_select_tests.rs"]
mod tests;
