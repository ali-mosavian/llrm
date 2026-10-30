//! `constraint-elimination`: the facts a loop's inductions give.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `State::getMonotonicityInfo`,
//! `addBoundsForHeaderInductions` and `addInfoForInductions`, each function
//! linking the lines it translates. A header phi `{Start,+,Step}` that never
//! steps below its start (or above it, for a decrement) gives
//! `PN >= Start` in the loop; a loop exit compared against an invariant `B`
//! gives `PN < B` in the body, given a precondition on `Start` and `B` that
//! the walk checks before it adds the fact. The facts are queued as
//! condition facts of the loop's blocks, as a branch's are.
//!
//! [`Inductions`] holds what LLVM's `State` holds by reference: `LI`, `SE`
//! and (for `SE`) the assumptions.
//!
//! # Used by
//! `constraintelim::State::add_info_for`.
//!
//! # Example
//! `for (j = 1919; j > -1; j--)`: the phi `{1919,+,-1}<nsw>` gives
//! `j <= 1919`, the header test `j > -1`, so `j <u 1920` is true.

use llrm_analysis::scev::expr::Type as ScevType;
use llrm_analysis::scev::predicate::MonotonicPredicateType;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::pattern::{self, CmpPredicate, IntrinsicId, Side};
use llrm_mir::valuetracking::{AssumptionCache, Query};

use crate::constraintelim::{ConditionTy, Entry2, State, is_integer, is_pointer, signed_predicate, unsigned_predicate};

/// `LI`, `SE` and `AC` of the function.
pub struct Inductions {
    loops: std::rc::Rc<LoopInfo>,
    scev: ScalarEvolution,
    assumptions: AssumptionCache,
}

impl Inductions {
    pub fn new(
        q: &Query,
        tree: &DomTree,
    ) -> Self {
        Self {
            loops: std::rc::Rc::new(LoopInfo::analyze(q.function, || tree)),
            scev: ScalarEvolution::new(),
            assumptions: AssumptionCache::of(q.context, q.globals, q.metadata, q.function),
        }
    }
}

/// `MonotonicInfo`.
#[derive(Clone, Copy, Debug, Default)]
struct MonotonicInfo {
    decreasing: bool,
    unsigned: bool,
    signed: bool,
}

/// A constant integer: its bits and width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Imm {
    bits: u128,
    width: u32,
}

impl Imm {
    fn is_negative(self) -> bool {
        self.width != 0 && self.bits >> (self.width - 1) & 1 == 1
    }

    fn is_zero(self) -> bool {
        self.bits == 0
    }

    fn is_one(self) -> bool {
        self.bits == 1
    }

    /// `(-*this).isOne()`.
    fn negated_is_one(self) -> bool {
        self.bits == llrm_mir::context::mask(self.width)
    }
}

fn imm(
    q: &Query,
    v: Operand,
) -> Option<Imm> {
    q.int_constant(v).map(|(bits, width)| Imm { bits, width })
}

/// `m_IncrementOf(m_Value(Var), Off)`: `step` is `Var + Off`, a plain `add`
/// or the sum of an `[u|s]add.with.overflow`: `Var`, `Off` and the `add`'s
/// flags (the `extractvalue` has none).
fn increment_of(
    q: &Query,
    step: Operand,
) -> Option<(Operand, Imm, Option<Flags>)> {
    // `m_c_Add(PhiM, m_APInt)`: the operand that is no constant is the phi.
    let split = |a: Operand, b: Operand| match (imm(q, a), imm(q, b)) {
        (_, Some(off)) => Some((a, off)),
        (Some(off), None) => Some((b, off)),
        (None, None) => None,
    };
    if let Some((_, instruction, a, b)) = pattern::binary(q, step, BinaryOp::Add) {
        let (var, off) = split(a, b)?;
        return Some((var, off, Some(instruction.flags)));
    }
    let (_, instruction, aggregate) = pattern::extract_value(q, step)?;
    if !matches!(
        &instruction.opcode,
        Opcode::ExtractValue(index) if index == &[0]
    ) {
        return None;
    }
    let (call, call_instruction) = q.instruction_of(aggregate)?;
    if !matches!(
        pattern::call_intrinsic(q, call),
        Some(IntrinsicId::UaddWithOverflow | IntrinsicId::SaddWithOverflow)
    )
    {
        return None;
    }
    let arguments = pattern::call_arguments(call_instruction);
    let (var, off) = split(*arguments.first()?, *arguments.get(1)?)?;
    Some((var, off, None))
}

/// `getStartAndBackedgeValue`: the start value, coming from `loop_pred`, and
/// the backedge value of the phi `pn`; None if it has other incoming values.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1103>
fn start_and_backedge(
    q: &Query,
    pn: InstId,
    loop_pred: BlockId,
) -> Option<(Operand, Operand)> {
    let incoming: Vec<(Operand, BlockId)> = pattern::phi_incoming(q.function.instruction(pn)).collect();
    if incoming.len() != 2 {
        return None;
    }
    let start = usize::from(incoming[0].1 != loop_pred);
    Some((incoming[start].0, incoming[1 - start].0))
}

fn result(
    q: &Query,
    inst: InstId,
) -> Operand {
    pattern::result_of(q.function.instruction(inst))
}

/// The header phis of `bb`.
fn phis(
    q: &Query,
    bb: BlockId,
) -> Vec<InstId> {
    q.function.iter_block(bb).take_while(|&inst| q.function.instruction(inst).opcode == Opcode::Phi).collect()
}

impl State<'_> {
    fn fact(
        &mut self,
        block: BlockId,
        pred: CmpPredicate,
        op0: Side,
        op1: Side,
        does_hold: Option<ConditionTy>,
    ) {
        self.push(block, Entry2::ConditionFact { cond: ConditionTy { pred, op0, op1 }, does_hold });
    }

    /// `getMonotonicityInfo`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1127>
    fn monotonicity_info(
        q: &Query,
        tree: &DomTree,
        ind: &mut Inductions,
        pn: InstId,
        step: Operand,
    ) -> MonotonicInfo {
        let pn_value = result(q, pn);
        let mut info = MonotonicInfo::default();
        let mut step_offset = None;
        if let Some((var, off, flags)) = increment_of(q, step)
            && var == pn_value
        {
            step_offset = Some(off);
            info.decreasing = off.is_negative();
            if let Some(flags) = flags {
                info.unsigned = !info.decreasing && flags.contains(Flags::NUW);
                info.signed = flags.contains(Flags::NSW);
            }
        } else if let Some((_, instruction)) = q.instruction_of(step)
            && let Opcode::GetElementPtr { .. } = instruction.opcode
        {
            // TODO in LLVM: the non-increasing direction needs a nusw GEP
            // with a negative constant offset.
            let flags = instruction.flags;
            let no_unsigned_signed_wrap = flags.contains(Flags::NUSW) || flags.contains(Flags::INBOUNDS);
            info.unsigned = instruction.operands[0] == pn_value
                && (flags.contains(Flags::NUW)
                    || (no_unsigned_signed_wrap && crate::constraintelim::constant_gep_offset(q, step) >= Some(0)));
        }

        // Forming the SCEV of a phi is expensive, so only consult it for a PN
        // + C step whose no-wrap flags prove nothing.
        if info.unsigned || info.signed || step_offset.is_none() {
            return info;
        }

        let Inductions { loops, scev, assumptions } = ind;
        let cx = Inductions::cx(q, tree, loops, assumptions);
        if !scev.is_value_scevable(&cx, pn_value) {
            return info;
        }
        let ar = scev.get_scev(&cx, pn_value);
        if scev.scev_type(ar) != ScevType::AddRec {
            return info;
        }
        let expected = if info.decreasing {
            MonotonicPredicateType::MonotonicallyDecreasing
        } else {
            MonotonicPredicateType::MonotonicallyIncreasing
        };
        let mut is_monotonic = |pred| scev.get_monotonic_predicate_type(&cx, ar, pred) == Some(expected);
        info.signed = is_monotonic(IntPredicate::Sgt);
        info.unsigned = !info.decreasing && is_monotonic(IntPredicate::Ugt);
        info
    }

    /// `addBoundsForHeaderInductions`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1167>
    pub(crate) fn add_bounds_for_header_inductions(
        &mut self,
        q: &Query,
        ind: &mut Inductions,
        bb: BlockId,
    ) {
        let loops = std::rc::Rc::clone(&ind.loops);
        let Some(loop_) = loops.get_loop_for(bb).filter(|l| l.get_header() == bb) else { return };
        let Some(loop_pred) = loop_.get_loop_predecessor(q.function) else { return };

        for pn in phis(q, bb) {
            let pn_value = result(q, pn);
            let side = Side::Value(pn_value);
            if !is_integer(q, side) && !is_pointer(q, side) {
                continue;
            }
            let Some((start, step)) = start_and_backedge(q, pn, loop_pred) else { continue };

            let mut info = Self::monotonicity_info(q, self.tree, ind, pn, step);
            // Every variable in the unsigned system already has a `V >= 0`
            // row, so a zero start value would just duplicate it.
            if pattern::zero(q, start) {
                info.unsigned = false;
            }
            if !info.unsigned && !info.signed {
                continue;
            }

            // A non-decreasing induction cannot step below its start value,
            // and a non-increasing one cannot step above it.
            let (mut lhs, mut rhs) = (Side::Value(pn_value), Side::of(q, start));
            if info.decreasing {
                std::mem::swap(&mut lhs, &mut rhs);
            }
            let pred = CmpPredicate {
                pred: if info.unsigned { IntPredicate::Uge } else { IntPredicate::Sge },
                same_sign: info.unsigned && info.signed,
            };
            self.fact(bb, pred, lhs, rhs, None);
        }
    }

    /// `addInfoForInductions`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1203>
    pub(crate) fn add_info_for_inductions(
        &mut self,
        q: &Query,
        ind: &mut Inductions,
        bb: BlockId,
    ) {
        let loops = std::rc::Rc::clone(&ind.loops);
        let Some(loop_) = loops.get_loop_for(bb) else { return };
        let header = loop_.get_header();
        let latch = loop_.get_loop_latch(q.function);
        if header != bb && latch != Some(bb) {
            return;
        }

        let Some(terminator) = q.function.terminator(bb) else { return };
        let br = q.function.instruction(terminator);
        if br.opcode != Opcode::Br || br.operands.len() != 3 {
            return;
        }
        let (Operand::Block(succ0), Operand::Block(succ1)) = (br.operands[1], br.operands[2]) else { return };
        let condition = br.operands[0];

        // `IndValue`: a phi, or a post-increment `PN + C` of one.
        let ind_value = |v: Operand| -> Option<(InstId, Option<Imm>)> {
            if let Some((pn, _)) = pattern::phi_of(q, v) {
                return Some((pn, None));
            }
            let (var, off, _) = increment_of(q, v)?;
            pattern::phi_of(q, var).map(|(pn, _)| (pn, Some(off)))
        };
        // `m_c_ICmp(Pred, IndValue, m_Value(B))`: the predicate with the
        // induction value on the left, `A`, `B`, the phi and the increment.
        let counting_cmp = |v: Operand| {
            let (_, _, pred, lhs, rhs) = pattern::icmp(q, v)?;
            if let Some((pn, inc)) = ind_value(lhs) {
                Some((pred, lhs, rhs, pn, inc))
            } else {
                let (pn, inc) = ind_value(rhs)?;
                Some((pred.swapped(), rhs, lhs, pn, inc))
            }
        };

        let mut peeled_on_edge = None;
        let counting = counting_cmp(condition).or_else(|| {
            let (and, or) = (pattern::logical_and(q, condition), pattern::logical_or(q, condition));
            let ((op0, op1), edge) = match (and, or) {
                (Some(ops), _) => (ops, true),
                (None, Some(ops)) => (ops, false),
                (None, None) => return None,
            };
            peeled_on_edge = Some(edge);
            counting_cmp(op0).or_else(|| counting_cmp(op1))
        });
        let Some((pred, a, b, pn, inc_step)) = counting else { return };

        let pn_value = result(q, pn);
        if q.function.parent(pn) != Some(header)
            || q.function.instruction(pn).operands.len() != 4
            || !ind.is_scevable(q, self.tree, pn_value)
        {
            return;
        }

        // For latch conditions, we need to inject the condition that holds for
        // the next iteration into the header. We limit to post-inc
        // conditions, for which an original PN + Step != B condition results
        // in a PN < B constraint in the header, which also holds for the next
        // loop iteration. This would no longer be correct if the post-inc
        // handling would inject a more precise PN + Step < B constraint
        // instead.
        if latch == Some(bb) && inc_step.is_none() {
            return;
        }

        let p = pred.pred;
        let continue_on_true = matches!(
            p,
            IntPredicate::Ne | IntPredicate::Ult | IntPredicate::Slt | IntPredicate::Ule | IntPredicate::Sle
        );
        let continue_pred = if continue_on_true { p } else { p.inverse() };
        let in_loop_succ = if continue_on_true { succ0 } else { succ1 };

        // The peeled condition only implies the compare on the edge where the
        // combined condition forces its operands, which must be the in-loop
        // edge.
        if peeled_on_edge.is_some_and(|edge| edge != continue_on_true) {
            return;
        }

        if !loop_.contains(in_loop_succ) || !loop_.is_loop_exiting(q.function, bb) {
            return;
        }

        let Some(loop_pred) = loop_.get_loop_predecessor(q.function) else { return };
        if !loop_.is_loop_invariant(q.function, b) {
            return;
        }

        let Some((start_value, backedge)) = start_and_backedge(q, pn, loop_pred) else { return };
        let (pn_side, b_side, start_side) = (Side::Value(pn_value), Side::of(q, b), Side::of(q, start_value));
        let cond_pred = |pred| CmpPredicate::new(pred);

        if is_relational(continue_pred) {
            if a != backedge {
                return;
            }

            // The latch condition ensures ContinuePred holds in the header on
            // each iteration other than the first. Together with a
            // precondition on the start value (StartValue ContinuePred B), we
            // can add B as bound of PN.
            let holds = ConditionTy { pred: cond_pred(continue_pred), op0: start_side, op1: b_side };
            self.fact(in_loop_succ, cond_pred(continue_pred), pn_side, b_side, Some(holds));

            // A signed bound can be translated to the unsigned system if PN is
            // signed non-decreasing (StartValue s<= PN s< B) and StartValue u<
            // B holds. Then StartValue, PN and B must all have the same sign.
            if matches!(continue_pred, IntPredicate::Slt | IntPredicate::Sle) {
                let info = Self::monotonicity_info(q, self.tree, ind, pn, backedge);
                if info.signed && !info.decreasing {
                    let unsigned = unsigned_predicate(continue_pred);
                    let holds = ConditionTy { pred: cond_pred(unsigned), op0: start_side, op1: b_side };
                    self.fact(in_loop_succ, cond_pred(unsigned), pn_side, b_side, Some(holds));
                }
            }

            // A relational latch steps past B rather than landing on it, so
            // none of the reasoning below applies.
            return;
        }

        // `m_c_Add(m_Specific(PN), m_APInt(StepOffset))`, or an affine
        // recurrence of the loop with a constant step.
        let mut start_scev = None;
        let plain = pattern::binary(q, backedge, BinaryOp::Add).and_then(|(_, _, x, y)| {
            if x == pn_value {
                imm(q, y)
            } else if y == pn_value {
                imm(q, x)
            } else {
                None
            }
        });
        let step_offset = match plain {
            Some(off) => {
                if off.is_zero() {
                    return;
                }
                off
            }
            None => {
                let Some((start, step_imm)) = ind.affine_constant_step(q, self.tree, pn_value, header) else { return };
                start_scev = Some(start);
                step_imm
            }
        };

        // If we looked through `PN + C`, only derive facts when that add is
        // really the induction's post-increment or post-decrement.
        if inc_step.is_some_and(|inc| inc != step_offset) {
            return;
        }

        let info = Self::monotonicity_info(q, self.tree, ind, pn, backedge);

        // Handle negative steps.
        if step_offset.is_negative() {
            // TODO in LLVM: extend to allow steps > -1.
            if !step_offset.negated_is_one() {
                return;
            }

            // AR may wrap. The loop exits once the compared value reaches B,
            // that is at PN == B when comparing the phi, and at PN == B + 1
            // for a post-decrement. Use non-strict predicate for the former,
            // and a strict one for the latter to ensure the loop exits before
            // wrapping.
            let u_precond = if inc_step.is_some() { IntPredicate::Ult } else { IntPredicate::Ule };
            let b_before_start_unsigned = ConditionTy { pred: cond_pred(u_precond), op0: b_side, op1: start_side };
            let b_before_start_signed =
                ConditionTy { pred: cond_pred(signed_predicate(u_precond)), op0: b_side, op1: start_side };

            // AR may wrap, so both facts are conditional on B being below
            // StartValue. Add StartValue >= PN, which holds as the loop exits
            // before wrapping.
            self.fact(in_loop_succ, cond_pred(IntPredicate::Uge), start_side, pn_side, Some(b_before_start_unsigned));
            if !(info.decreasing && info.signed) {
                self.fact(in_loop_succ, cond_pred(IntPredicate::Sge), start_side, pn_side, Some(b_before_start_signed));
            }
            // Add PN > B, which holds as the loop exits when reaching B.
            self.fact(in_loop_succ, cond_pred(IntPredicate::Ugt), pn_side, b_side, Some(b_before_start_unsigned));
            self.fact(in_loop_succ, cond_pred(IntPredicate::Sgt), pn_side, b_side, Some(b_before_start_signed));
            return;
        }

        // Make sure AR either steps by 1 or that the value we compare against
        // is a GEP based on the same start value and all offsets are a
        // multiple of the step size, to guarantee that the induction will
        // reach the value.
        if step_offset.is_zero() || step_offset.is_negative() {
            return;
        }

        if !step_offset.is_one()
            && !ind.b_minus_start_is_multiple(q, self.tree, start_scev, start_value, b, step_offset)
        {
            return;
        }

        // We already established that B - Start is a multiple of Step above.
        // The loop exits once the compared value reaches B, that is at PN == B
        // when comparing the phi, and at PN + Step == B for a post-increment.
        // Together with the added precondition StartValue <= B for the former
        // and the strict StartValue < B for the latter (which implies
        // StartValue + Step <= B), neither PN nor the increment can wrap.
        let u_precond = if inc_step.is_some() { IntPredicate::Ult } else { IntPredicate::Ule };
        let start_before_bound_unsigned = ConditionTy { pred: cond_pred(u_precond), op0: start_side, op1: b_side };
        let start_before_bound_signed =
            ConditionTy { pred: cond_pred(signed_predicate(u_precond)), op0: start_side, op1: b_side };

        // Add PN >= StartValue, as the loop exits before wrapping.
        if !info.unsigned {
            self.fact(
                in_loop_succ,
                cond_pred(IntPredicate::Uge),
                pn_side,
                start_side,
                Some(start_before_bound_unsigned),
            );
        }
        if !info.signed {
            self.fact(in_loop_succ, cond_pred(IntPredicate::Sge), pn_side, start_side, Some(start_before_bound_signed));
        }
        // Add PN < B, as the loop exits once the compared value reaches B.
        self.fact(in_loop_succ, cond_pred(IntPredicate::Slt), pn_side, b_side, Some(start_before_bound_signed));
        self.fact(in_loop_succ, cond_pred(IntPredicate::Ult), pn_side, b_side, Some(start_before_bound_unsigned));

        // Try to add condition from the header or latch to the dedicated exit
        // blocks. When exiting either with EQ or NE, we know that the
        // induction value must be u<= B, as other exits may only exit
        // earlier.
        debug_assert!(continue_pred == IntPredicate::Ne, "unsupported predicate");
        for exit in loop_.get_exit_blocks(q.function) {
            // Bail out on non-dedicated exits.
            if self.tree.dominates(bb, exit) && self.tree.contains(exit) {
                self.fact(
                    exit,
                    cond_pred(IntPredicate::Ule),
                    Side::of(q, a),
                    b_side,
                    Some(start_before_bound_unsigned),
                );
            }
        }
    }
}

fn is_relational(pred: IntPredicate) -> bool {
    !matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
}

impl Inductions {
    fn cx<'a>(
        q: &Query<'a>,
        tree: &'a DomTree,
        loops: &'a LoopInfo,
        assumptions: &'a AssumptionCache,
    ) -> Cx<'a> {
        Cx {
            context: q.context,
            globals: q.globals,
            metadata: q.metadata,
            function: q.function,
            layout: q.layout,
            loops,
            tree,
            assumptions,
        }
    }

    /// `SE.isSCEVable(V->getType())`.
    fn is_scevable(
        &mut self,
        q: &Query,
        tree: &DomTree,
        v: Operand,
    ) -> bool {
        let Self { loops, scev, assumptions } = self;
        scev.is_value_scevable(&Self::cx(q, tree, loops, assumptions), v)
    }

    /// `match(SE.getSCEV(PN), m_scev_AffineAddRec(m_SCEV(Start),
    /// m_scev_APInt(Step), m_SpecificLoop(L)))`: the start and the step.
    fn affine_constant_step(
        &mut self,
        q: &Query,
        tree: &DomTree,
        pn: Operand,
        header: BlockId,
    ) -> Option<(llrm_analysis::scev::expr::Scev, Imm)> {
        let Self { loops, scev, assumptions } = self;
        let cx = Self::cx(q, tree, loops, assumptions);
        let expr = scev.get_scev(&cx, pn);
        let (start, step, at) = scev.match_affine_add_rec(expr)?;
        let step = scev.match_apint(step)?;
        (at == header).then_some((start, Imm { bits: step.bits, width: step.width }))
    }

    /// Whether `B - Start` is known to be a multiple of `step_offset`.
    fn b_minus_start_is_multiple(
        &mut self,
        q: &Query,
        tree: &DomTree,
        start_scev: Option<llrm_analysis::scev::expr::Scev>,
        start_value: Operand,
        b: Operand,
        step_offset: Imm,
    ) -> bool {
        use llrm_analysis::scev::ap::Ap;
        use llrm_analysis::scev::expr::Use;
        let Self { loops, scev, assumptions } = self;
        let cx = Self::cx(q, tree, loops, assumptions);
        let start_scev = start_scev.unwrap_or_else(|| scev.get_scev(&cx, start_value));
        let b_scev = scev.get_scev(&cx, b);
        let b_minus_start = scev.get_minus_scev(&cx, Use::of(b_scev), Use::of(start_scev), 0, 0);
        if scev.is_could_not_compute(b_minus_start) {
            return false;
        }
        let step = Ap { width: step_offset.width, bits: step_offset.bits };
        scev.get_constant_multiple(&cx, b_minus_start, None).urem(step).is_zero()
    }
}
