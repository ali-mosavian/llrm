//! `hoistRegion` and what it asks (`LICM.cpp`): decisions only.

use llrm_analysis::cfg;
use llrm_analysis::mssa::alias::AliasQuery;
use llrm_analysis::mssa::{AccessId, Kind, Mssa};
use llrm_mir::context::ConstantKind;
use llrm_mir::facts::{Fact, Facts};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp, Flags as Wrap, IntPredicate, Opcode};
use llrm_mir::passes::{self, Outer};

use super::{Flags, LoopCtx, LoopSafety, calls};

/// [`LoopBlocksRPO`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/LoopIterator.h#L110):
/// the loop's blocks in reverse postorder from the header, back edges not
/// followed.
fn loop_blocks_rpo(ctx: &LoopCtx) -> Vec<BlockId> {
    let header = cfg::block(ctx.loop_.header);
    let mut seen = llrm_mir::dense::IdSet::new();
    let mut post = Vec::new();
    let mut stack = vec![(header, ctx.function.successors(header), 0usize)];
    seen.insert(header);
    while let Some((block, successors, next)) = stack.last_mut() {
        if let Some(&succ) = successors.get(*next) {
            *next += 1;
            if ctx.contains_block(succ) && seen.insert(succ) {
                let successors = ctx.function.successors(succ);
                stack.push((succ, successors, 0));
            }
        } else {
            post.push(*block);
            stack.pop();
        }
    }
    post.reverse();
    post
}

/// [`hoistRegion`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L645):
/// the instructions it hoists to `preheader`. Not built (L1): the
/// reciprocal of an `fdiv` (MIR has no `arcp`), `hoistInsertPastInsert`
/// (no vectors), the invariant-start and guard intrinsics, and
/// `hoistArithmetics` but `hoistMinMax`, `hoistGEP` and `hoistBOAssociation`
/// (the latter only for integers: a float add or multiply needs `reassoc` and
/// `nsz`, and the intersection of the two fast-math flags).
#[allow(clippy::too_many_arguments)]
pub fn hoist_region(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    preheader: BlockId,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    flags: &mut Flags,
    safety: &LoopSafety,
) -> (Vec<Hoist>, Vec<(usize, Reassociation)>) {
    let mut found = Vec::new();
    let mut reassociated = Vec::new();
    for block in loop_blocks_rpo(ctx) {
        // Only the contents of a block outside the subloops: those have been
        // processed.
        if ctx.in_sub_loop(block) {
            continue;
        }
        for inst in unit.function.iter_block(block).collect::<Vec<_>>() {
            // Erased or sunk by `sinkRegion`.
            if ctx.state.gone.borrow().contains(&inst) {
                continue;
            }
            // Try hoisting the instruction out to the preheader. We can only do
            // this if all of the operands of the instruction are loop invariant
            // and if it is safe to hoist the instruction.
            if ctx.has_loop_invariant_operands(inst)
                && can_sink_or_hoist_inst(unit, outer, ctx, mssa, aa, flags, safety, inst, true)
                && is_safe_to_execute_unconditionally(unit, outer, ctx, safety, preheader, inst)
            {
                ctx.state.moved.borrow_mut().insert(inst, preheader);
                found.push(Hoist { inst, preheader, guaranteed: safety.is_guaranteed_to_execute(inst) });
            // Try to reassociate instructions so that part of computations can
            // be done out of loop.
            } else if let Some(one) = hoist_min_max(unit, ctx, inst)
                .or_else(|| hoist_gep(unit, ctx, inst))
                .or_else(|| hoist_add_sub(unit, ctx, inst))
                .or_else(|| hoist_mul_add_association(unit, ctx, inst))
                .or_else(|| hoist_bo_association(unit, ctx, inst))
                .or_else(|| hoist_sub_add_association(unit, ctx, inst))
            {
                ctx.state.gone.borrow_mut().extend([one.inst()].into_iter().chain(one.erased()));
                one.forward(unit.function, ctx);
                // The preheader gets it where the visit reached it, after the
                // hoists found so far.
                reassociated.push((found.len(), one));
            }
        }
    }
    (found, reassociated)
}

/// An instruction `hoist_region` moves to `preheader`, and whether it was
/// guaranteed to execute in the loop (`LICM.cpp:1478`: a call that was not has
/// its UB-implying attributes dropped).
#[derive(Clone, Copy, Debug)]
pub struct Hoist {
    pub inst: InstId,
    pub preheader: BlockId,
    pub guaranteed: bool,
}

/// Whether `inst` could be a [`hoist_bo_association`]'s `bo`: an operation it
/// handles with an operand the same operation defines.
pub fn may_reassociate(
    function: &Function,
    inst: InstId,
) -> bool {
    let bo = function.instruction(inst);
    if matches!(bo.opcode, Opcode::GetElementPtr { .. }) {
        return matches!(
            bo.operands[0],
            Operand::Value(ptr) if matches!(function.value(ptr).def, ValueDef::Instruction(def) if matches!(function.instruction(def).opcode, Opcode::GetElementPtr { .. }))
        );
    }
    // `hoistSubAddAssociation`: a `sub` of an `add` or `sub`, an `add` of a
    // `sub`.
    let defined_by = |wanted: &[BinaryOp]| {
        bo.operands
            .iter()
            .any(
                |&operand| {
                    let Operand::Value(value) = operand else { return false };
                    matches!(
                        function.value(value).def,
                        ValueDef::Instruction(def) if matches!(function.instruction(def).opcode, Opcode::Binary(op) if wanted.contains(&op))
                    )
                },
            )
    };
    match bo.opcode {
        // `hoistAddSub`: a compare of an `add` or `sub`.
        Opcode::ICmp(_) if defined_by(&[BinaryOp::Add, BinaryOp::Sub]) => return true,
        // `hoistMulAddAssociation`: a product with an operand an instruction
        // defines.
        Opcode::Binary(BinaryOp::Mul) if defined_by(&[BinaryOp::Add, BinaryOp::Mul]) => return true,
        Opcode::Binary(BinaryOp::Sub) if defined_by(&[BinaryOp::Add, BinaryOp::Sub]) => return true,
        Opcode::Binary(BinaryOp::Add) if defined_by(&[BinaryOp::Sub]) => return true,
        _ => {}
    }
    matches!(
        bo.opcode,
        Opcode::Binary(BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
    )
        && bo.operands.iter().any(|&operand| {
            let Operand::Value(value) = operand else { return false };
            matches!(
                function.value(value).def,
                ValueDef::Instruction(def) if function.instruction(def).opcode == bo.opcode
            )
        })
}

/// What `hoistArithmetics` decides for an instruction of the loop.
#[derive(Clone, Debug)]
pub enum Reassociation {
    /// [`hoist_bo_association`]: `bo` becomes `lv op (c1 op c2)`, the inner
    /// `c1 op c2` made in the preheader, and `bo0` goes.
    BinaryOp {
        bo: InstId,
        bo0: InstId,
        op: BinaryOp,
        lv: Operand,
        c1: Operand,
        c2: Operand,
        /// The flags both the new instructions get (`OverflowTracking`).
        flags: Wrap,
        /// `bo0` is what an earlier reassociation of the loop put in place of
        /// the `bo` it replaced: `lv` stands for its variant operand and `c1`
        /// is read from it when this one is applied.
        chained: bool,
    },
    /// [`hoist_sub_add_association`]: `i` becomes `lv result_op (c1 inv_op
    /// c2)`, the inner one made in the preheader, and `bo` goes.
    SubAdd { i: InstId, bo: InstId, inv_op: BinaryOp, result_op: BinaryOp, lv: Operand, c1: Operand, c2: Operand },
    /// [`hoist_add_sub`]: `icmp pred (variant op inv), rhs` becomes `icmp pred
    /// variant, (inv' op' rhs)`, the new operand made in the preheader, and
    /// `dead`, the `add` or `sub`, goes.
    IcmpAddSub {
        icmp: InstId,
        dead: InstId,
        pred: IntPredicate,
        variant: Operand,
        op: BinaryOp,
        a: Operand,
        b: Operand,
    },
    /// [`hoist_mul_add_association`]: `i` is `variant * factor`, the `variant`
    /// a tree of `adds` over products; each product `ins` gets the factor on
    /// its invariant operand `slot` (`factor.op.mul`, made in the preheader).
    MulAdd { i: InstId, variant: Operand, factor: Operand, changes: Vec<(InstId, usize, Operand)>, adds: Vec<InstId> },
    /// [`hoist_gep`]: `gep (gep ptr, inner), outer` becomes
    /// `gep (gep ptr, outer), inner`, the new inner one made in the preheader.
    Gep { gep: InstId, src: InstId, in_bounds: bool },
    /// [`hoist_min_max`]: `x < a && x < b` becomes `x < min(a, b)`, the
    /// `min` made in the preheader, and the two compares go.
    MinMax {
        /// The `and`, `or` or `select` of the two compares.
        logic: InstId,
        cond1: InstId,
        cond2: InstId,
        lhs: Operand,
        rhs1: Operand,
        rhs2: Operand,
        signed: bool,
        use_min: bool,
        /// The predicate of the new compare (inverted for a logical or).
        pred: IntPredicate,
        /// A select: `rhs2` was not necessarily evaluated, so it is frozen.
        freeze_rhs2: bool,
    },
}

impl Reassociation {
    /// The instruction the decision replaces.
    pub fn inst(&self) -> InstId {
        match *self {
            Self::BinaryOp { bo, .. } => bo,
            Self::Gep { gep, .. } => gep,
            Self::MinMax { logic, .. } => logic,
            Self::SubAdd { i, .. } | Self::MulAdd { i, .. } => i,
            Self::IcmpAddSub { dead, .. } => dead,
        }
    }

    /// What the next decisions of the loop read of this one, as the pin's
    /// in-place edit leaves it (`hoistMulAddAssociation`).
    fn forward(
        &self,
        function: &Function,
        ctx: &LoopCtx,
    ) {
        if let Self::BinaryOp { bo, lv, flags, .. } = self {
            ctx.state.chained.borrow_mut().insert(*bo, (*lv, *flags));
            return;
        }
        let Self::MulAdd { i, variant, changes, .. } = self else { return };
        if let Some(result) = function.instruction(*i).result {
            ctx.state.forwarded.borrow_mut().insert(result, *variant);
        }
        let mut factored = ctx.state.factored.borrow_mut();
        for &(ins, slot, _) in changes {
            factored.insert(ins, slot);
        }
    }

    /// The instructions it also erases.
    pub fn erased(&self) -> Vec<InstId> {
        match self {
            Self::BinaryOp { bo0, .. } => vec![*bo0],
            Self::Gep { src, .. } => vec![*src],
            Self::MinMax { cond1, cond2, .. } => vec![*cond1, *cond2],
            Self::SubAdd { bo, .. } => vec![*bo],
            Self::IcmpAddSub { .. } => vec![],
            Self::MulAdd { changes, .. } => changes.iter().map(|&(ins, ..)| ins).collect(),
        }
    }
}

/// `MatchICmpAgainstInvariant`: a one-use relational integer compare `cond` of
/// a loop-variant value against an invariant one, the variant one on the left;
/// its definition, predicate (inverted for a logical or) and both sides.
fn against_invariant(
    function: &Function,
    ctx: &LoopCtx,
    cond: Operand,
    inverse: bool,
) -> Option<(InstId, IntPredicate, Operand, Operand)> {
    let Operand::Value(value) = cond else { return None };
    let ValueDef::Instruction(def) = function.value(value).def else { return None };
    let compare = function.instruction(def);
    let Opcode::ICmp(mut pred) = compare.opcode else { return None };
    if !function.users(value).has_one_use() || ctx.state.gone.borrow().contains(&def) {
        return None;
    }
    let [mut lhs, mut rhs] = compare.operands[..] else { return None };
    if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) {
        return None;
    }
    if ctx.is_loop_invariant(lhs) {
        std::mem::swap(&mut lhs, &mut rhs);
        pred = pred.swapped();
    }
    if ctx.is_loop_invariant(lhs) || !ctx.is_loop_invariant(rhs) {
        return None;
    }
    if inverse {
        pred = pred.inverse();
    }
    Some((def, pred, lhs, rhs))
}

/// Whether `inst` could be a [`hoist_min_max`]'s logical operation: an `and`,
/// `or` or select of two compares [`against_invariant`] with one predicate and
/// one variant side. The cheap look `seeded` takes ahead of the memory graph:
/// a loop with a logical operation of unrelated compares is not seeded by it.
pub fn may_hoist_min_max(
    function: &Function,
    ctx: &LoopCtx,
    inst: InstId,
) -> bool {
    let logic = function.instruction(inst);
    let is_constant = |operand: Operand| matches!(operand, Operand::Constant(_));
    let (a, b, inverse) = match (&logic.opcode, &logic.operands[..]) {
        (Opcode::Binary(BinaryOp::And), &[a, b]) => (a, b, false),
        (Opcode::Binary(BinaryOp::Or), &[a, b]) => (a, b, true),
        // `select a, b, false` and `select a, true, b`.
        (Opcode::Select, &[a, b, c]) if is_constant(c) => (a, b, false),
        (Opcode::Select, &[a, b, c]) if is_constant(b) => (a, c, true),
        _ => return false,
    };
    let (Some((_, pred1, lhs1, _)), Some((_, pred2, lhs2, _))) =
        (against_invariant(function, ctx, a, inverse), against_invariant(function, ctx, b, inverse))
    else {
        return false;
    };
    pred1 == pred2 && lhs1 == lhs2
}

/// [`hoistMinMax`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2256):
/// `x < a && x < b` (or `||`, as the inverse) with `a` and `b` invariant in
/// the loop becomes one compare against `min(a, b)`, made in the preheader.
fn hoist_min_max(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    inst: InstId,
) -> Option<Reassociation> {
    let function = &*unit.function;
    let logic = function.instruction(inst);
    // Only these can be `m_LogicalAnd` or `m_LogicalOr`.
    if !matches!(
        logic.opcode,
        Opcode::Binary(BinaryOp::And | BinaryOp::Or) | Opcode::Select
    ) {
        return None;
    }
    llrm_mir::counts::bump(llrm_mir::counts::Event::HoistMinMaxQueries);
    let result = logic.result?;
    let query = llrm_mir::valuetracking::Query {
        context: unit.context,
        globals: &[],
        metadata: &[],
        function,
        layout: unit.layout,
        dt: None,
        assumptions: None,
        dom_conditions: None,
        cond_context: None,
        cxt: Some(inst),
        pending_intrinsics: &[],
    };
    use llrm_mir::valuetracking::pattern;
    let (inverse, (cond1, cond2)) = if let Some(operands) = pattern::logical_or(&query, Operand::Value(result)) {
        (true, operands)
    } else if let Some(operands) = pattern::logical_and(&query, Operand::Value(result)) {
        (false, operands)
    } else {
        return None;
    };
    let (def1, pred1, lhs1, rhs1) = against_invariant(function, ctx, cond1, inverse)?;
    let (def2, pred2, lhs2, rhs2) = against_invariant(function, ctx, cond2, inverse)?;
    unit.context.types.int_bits(query.type_of(lhs1)?)?;
    if pred1 != pred2 || lhs1 != lhs2 {
        return None;
    }
    let use_min = matches!(
        pred1,
        IntPredicate::Ult | IntPredicate::Ule | IntPredicate::Slt | IntPredicate::Sle
    );
    let pred = if inverse { pred1.inverse() } else { pred1 };
    Some(Reassociation::MinMax {
        logic: inst,
        cond1: def1,
        cond2: def2,
        lhs: lhs1,
        rhs1,
        rhs2,
        signed: pred1.is_signed(),
        use_min,
        pred,
        freeze_rhs2: logic.opcode == Opcode::Select,
    })
}

/// [`hoistGEP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2337):
/// `gep (gep ptr, idx1), idx2` to `gep (gep ptr, idx2), idx1` when the inner
/// one then leaves the loop. DEVIATE: as for [`hoist_bo_association`], a GEP
/// another reassociation has replaced is left alone.
fn hoist_gep(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    inst: InstId,
) -> Option<Reassociation> {
    let function = &*unit.function;
    let gep = function.instruction(inst);
    if !matches!(gep.opcode, Opcode::GetElementPtr { .. }) {
        return None;
    }
    // A constant GEP folds into an addressing mode, and reassociating it may
    // inhibit the CSE of a common base.
    if gep.operands[1..].iter().all(|operand| matches!(operand, Operand::Constant(_))) {
        return None;
    }
    let Operand::Value(base) = gep.operands[0] else { return None };
    let ValueDef::Instruction(src_inst) = function.value(base).def else { return None };
    let src = function.instruction(src_inst);
    if !matches!(src.opcode, Opcode::GetElementPtr { .. })
        || function.users(base).iter().count() != 1
        || !ctx.contains(src_inst)
        || ctx.state.gone.borrow().contains(&src_inst)
    {
        return None;
    }
    let invariant = |operand: &Operand| ctx.is_loop_invariant(*operand);
    if !invariant(&src.operands[0]) || !gep.operands[1..].iter().all(invariant) {
        return None;
    }
    // Only possible where speculation is off: otherwise `src` was hoisted.
    if src.operands[1..].iter().all(invariant) {
        return None;
    }
    // The swapped GEPs are inbounds if both were and every offset is
    // non-negative.
    let query = llrm_mir::valuetracking::Query {
        context: unit.context,
        globals: &[],
        metadata: &[],
        function,
        layout: unit.layout,
        dt: None,
        assumptions: None,
        dom_conditions: None,
        cond_context: None,
        cxt: Some(inst),
        pending_intrinsics: &[],
    };
    let non_negative = |operand: &Operand| llrm_mir::valuetracking::known::is_known_non_negative(&query, *operand, 0);
    let in_bounds = src.flags.contains(Wrap::INBOUNDS)
        && gep.flags.contains(Wrap::INBOUNDS)
        && src.operands[1..].iter().all(non_negative)
        && gep.operands[1..].iter().all(non_negative);
    Some(Reassociation::Gep { gep: inst, src: src_inst, in_bounds })
}

/// [`hoistBOAssociation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2686):
/// `(LV op C1) op C2` and its commuted forms, the integer `add`, `mul`, `and`,
/// `or` and `xor`. The pin edits `bo` in place
/// ([`LICM.cpp#L2718`-`L2740`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2718-L2740)),
/// so the next link of a chain reads `lv op (c1 op c2)`: `State::chained`
/// carries that edit to it. DEVIATE: an instruction another kind of
/// reassociation of the loop has replaced, or one reading a value it erased,
/// is left alone, where LLVM would reassociate the new one in turn (a later
/// round of the pipeline does; `hoist_mul_add_association` follows its edits
/// too). Exit: 2026-11-20, x86-m32-bytes.
fn hoist_bo_association(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    inst: InstId,
) -> Option<Reassociation> {
    let function = &*unit.function;
    let bo = function.instruction(inst);
    let Opcode::Binary(op) = bo.opcode else { return None };
    if !matches!(
        op,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
    ) {
        return None;
    }
    let in_rhs = ctx.is_loop_invariant(bo.operands[0]);
    let Operand::Value(inner) = bo.operands[usize::from(in_rhs)] else { return None };
    let ValueDef::Instruction(bo0_inst) = function.value(inner).def else { return None };
    let bo0 = function.instruction(bo0_inst);
    // `hasNUsesOrMore(2)` for an integer: `bo` is its only user.
    if bo0.opcode != bo.opcode || function.users(inner).iter().count() >= 2 {
        return None;
    }
    // The pin edits `bo0` in place when it reassociates it, so the next link
    // of a chain reads `lv op (c1 op c2)`: `c1` is the invariant made then.
    let prior = ctx.state.chained.borrow().get(&bo0_inst).copied();
    if prior.is_none() && ctx.state.gone.borrow().contains(&bo0_inst) {
        return None;
    }
    let c2 = bo.operands[usize::from(!in_rhs)];
    let (lv, c1, bo0_flags) = match prior {
        Some((lv, flags)) => (lv, lv, flags),
        None => {
            let (mut lv, mut c1) = (bo0.operands[0], bo0.operands[1]);
            if ctx.is_loop_invariant(lv) && !ctx.is_loop_invariant(c1) {
                std::mem::swap(&mut lv, &mut c1);
            }
            if ctx.is_loop_invariant(lv) || !ctx.is_loop_invariant(c1) {
                return None;
            }
            (lv, c1, bo0.flags)
        }
    };
    if !ctx.is_loop_invariant(c2) {
        return None;
    }
    // A value an earlier reassociation of the loop has erased is not read.
    let erased = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => ctx.state.gone.borrow().contains(&def),
            ValueDef::Argument(_) => false,
        },
        _ => false,
    };
    if erased(lv) || (prior.is_none() && erased(c1)) || erased(c2) {
        return None;
    }
    // `OverflowTracking` with `AllKnownNonNegative` and `AllKnownNonZero` off,
    // `applyFlags` after `dropPoisonGeneratingFlags`.
    let both = |flag: Wrap| bo.flags.contains(flag) && bo0_flags.contains(flag);
    let mut flags = Wrap::default();
    match op {
        BinaryOp::Add if both(Wrap::NUW) => {
            flags.insert(Wrap::NUW);
            if both(Wrap::NSW) {
                flags.insert(Wrap::NSW);
            }
        }
        BinaryOp::Or if both(Wrap::DISJOINT) => flags.insert(Wrap::DISJOINT),
        _ => {}
    }
    Some(Reassociation::BinaryOp { bo: inst, bo0: bo0_inst, op, lv, c1, c2, flags, chained: prior.is_some() })
}

/// [`hoistAddSub`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2538)
/// with `hoistAdd` and `hoistSub`: `LV + C1 < C2` to `LV < C2 - C1`,
/// `LV - C1 < C2` to `LV < C1 + C2`, `C1 - LV < C2` to `LV > C1 - C2`, where
/// the overflow checks prove the move. DEVIATE: the operation is an `add` or
/// `sub` with the wrap flag, not `m_NSWAddLike` (`or disjoint`), and the
/// overflow queries have no dominator tree or assumptions (exit 2026-11-20).
fn hoist_add_sub(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    inst: InstId,
) -> Option<Reassociation> {
    use llrm_mir::valuetracking::range::{
        OverflowResult, compute_overflow_for_signed_add, compute_overflow_for_signed_sub,
        compute_overflow_for_unsigned_add, compute_overflow_for_unsigned_sub,
    };
    let function = &*unit.function;
    let icmp = function.instruction(inst);
    let Opcode::ICmp(mut pred) = icmp.opcode else { return None };
    let (mut lhs, mut rhs) = (icmp.operands[0], icmp.operands[1]);
    // Put the variant operand to the left.
    if ctx.is_loop_invariant(lhs) {
        std::mem::swap(&mut lhs, &mut rhs);
        pred = pred.swapped();
    }
    // The initial operation goes after the reassociation: only with no other
    // use.
    let Operand::Value(lhs_value) = lhs else { return None };
    if ctx.is_loop_invariant(lhs) || !ctx.is_loop_invariant(rhs) || function.users(lhs_value).iter().count() != 1 {
        return None;
    }
    let ValueDef::Instruction(dead) = function.value(lhs_value).def else { return None };
    if ctx.state.gone.borrow().contains(&dead) {
        return None;
    }
    let operation = function.instruction(dead);
    // `m_NSWAddLike`/`m_NUWAddLike`: an `or disjoint` is an add that wraps in
    // neither sense.
    let (op, disjoint) = match operation.opcode {
        Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub)) => (op, false),
        Opcode::Binary(BinaryOp::Or) if operation.flags.contains(Wrap::DISJOINT) => (BinaryOp::Add, true),
        _ => return None,
    };
    let signed = pred.is_signed();
    if !disjoint && !operation.flags.contains(if signed { Wrap::NSW } else { Wrap::NUW }) {
        return None;
    }
    let (mut variant_op, mut invariant_op) = (operation.operands[0], operation.operands[1]);
    let mut subtracted = false;
    if ctx.is_loop_invariant(variant_op) {
        std::mem::swap(&mut variant_op, &mut invariant_op);
        // `hoistAdd` swaps the operands of a sum and keeps the predicate; of a
        // difference the variant one is then subtracted.
        if op == BinaryOp::Sub {
            subtracted = true;
            pred = pred.swapped();
        }
    }
    if ctx.is_loop_invariant(variant_op) || !ctx.is_loop_invariant(invariant_op) {
        return None;
    }
    let query = llrm_mir::valuetracking::Query {
        context: unit.context,
        globals: &[],
        metadata: &[],
        function,
        layout: unit.layout,
        dt: None,
        assumptions: None,
        dom_conditions: None,
        cond_context: None,
        cxt: Some(inst),
        pending_intrinsics: &[],
    };
    let (new_op, a, b, overflow) = match (op, subtracted) {
        // LV + C1 < C2: LV < C2 - C1.
        (BinaryOp::Add, _) => (
            BinaryOp::Sub,
            rhs,
            invariant_op,
            if signed {
                compute_overflow_for_signed_sub(&query, rhs, invariant_op)
            } else {
                compute_overflow_for_unsigned_sub(&query, rhs, invariant_op)
            },
        ),
        // C1 - LV < C2: LV > C1 - C2.
        (_, true) => (
            BinaryOp::Sub,
            invariant_op,
            rhs,
            if signed {
                compute_overflow_for_signed_sub(&query, invariant_op, rhs)
            } else {
                compute_overflow_for_unsigned_sub(&query, invariant_op, rhs)
            },
        ),
        // LV - C1 < C2: LV < C1 + C2.
        (_, false) => (
            BinaryOp::Add,
            invariant_op,
            rhs,
            if signed {
                compute_overflow_for_signed_add(&query, invariant_op, rhs)
            } else {
                compute_overflow_for_unsigned_add(&query, invariant_op, rhs)
            },
        ),
    };
    if overflow != OverflowResult::NeverOverflows {
        return None;
    }
    // A value an earlier reassociation of the loop has erased is not read.
    let erased = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => ctx.state.gone.borrow().contains(&def),
            ValueDef::Argument(_) => false,
        },
        _ => false,
    };
    if erased(variant_op) || erased(a) || erased(b) {
        return None;
    }
    Some(Reassociation::IcmpAddSub { icmp: inst, dead, pred, variant: variant_op, op: new_op, a, b })
}

/// [`hoistMulAddAssociation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2583)
/// for integers: `((A1 * B1) + (A2 * B2) + ...) * C` with `A1`, `A2`, ... and
/// `C` invariant becomes `((A1 * C) * B1) + ((A2 * C) * B2) + ...`, the
/// invariant products made in the preheader. At most 5 of them
/// (`licm-max-num-int-reassociations`). The pin edits in place, so a later
/// product of the loop reads the product it replaced as its variant and a
/// leaf it changed as a product with an invariant factor (`State::forwarded`
/// and `State::factored` say so); the sum a long unrolled chain builds is
/// multiplied once.
fn hoist_mul_add_association(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    inst: InstId,
) -> Option<Reassociation> {
    const INT_ASSOCIATION_UPPER_LIMIT: usize = 5;
    let function = &*unit.function;
    let i = function.instruction(inst);
    if i.opcode != Opcode::Binary(BinaryOp::Mul) {
        return None;
    }
    let (mut variant, mut invariant) = (i.operands[0], i.operands[1]);
    if ctx.is_loop_invariant(variant) {
        std::mem::swap(&mut variant, &mut invariant);
    }
    if ctx.is_loop_invariant(variant) || !ctx.is_loop_invariant(invariant) {
        return None;
    }
    let factor = invariant;
    // The instruction `operand` is after the earlier edits of the loop, and
    // how many users it has: a replaced product's users are its variant's.
    let binary_def = |operand: Operand| -> Option<(InstId, Option<usize>)> {
        let mut users = None;
        let mut operand = operand;
        while let Operand::Value(value) = operand
            && let Some(&to) = ctx.state.forwarded.borrow().get(&value)
        {
            users = Some(function.users(value).iter().count());
            operand = to;
        }
        let Operand::Value(value) = operand else { return None };
        let ValueDef::Instruction(def) = function.value(value).def else { return None };
        matches!(function.instruction(def).opcode, Opcode::Binary(_)).then_some((def, users))
    };
    let gone = |def: InstId| ctx.state.gone.borrow().contains(&def);
    let mut changes = Vec::new();
    let mut adds = Vec::new();
    let mut worklist: Vec<(InstId, Option<usize>)> = binary_def(variant).into_iter().collect();
    while let Some((bo, users)) = worklist.pop() {
        let instruction = function.instruction(bo);
        let result = instruction.result?;
        let factored = ctx.state.factored.borrow().get(&bo).copied();
        if users.unwrap_or_else(|| function.users(result).iter().count()) != 1 || (gone(bo) && factored.is_none()) {
            return None;
        }
        if instruction.opcode == Opcode::Binary(BinaryOp::Add)
            && let (Some(a), Some(b)) = (binary_def(instruction.operands[0]), binary_def(instruction.operands[1]))
        {
            worklist.extend([a, b]);
            adds.push(bo);
            continue;
        }
        if instruction.opcode != Opcode::Binary(BinaryOp::Mul) || !ctx.contains(bo) {
            return None;
        }
        let slot = if let Some(slot) = factored {
            slot
        } else if ctx.is_loop_invariant(instruction.operands[0]) {
            0
        } else if ctx.is_loop_invariant(instruction.operands[1]) {
            1
        } else {
            return None;
        };
        changes.push((bo, slot, instruction.operands[slot]));
        if changes.len() > INT_ASSOCIATION_UPPER_LIMIT {
            return None;
        }
    }
    if changes.is_empty() {
        return None;
    }
    // A value an earlier reassociation of the loop has erased is not read.
    let erased = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => gone(def),
            ValueDef::Argument(_) => false,
        },
        _ => false,
    };
    if erased(factor) || changes.iter().any(|&(_, _, inv)| erased(inv)) {
        return None;
    }
    Some(Reassociation::MulAdd { i: inst, variant, factor, changes, adds })
}

/// [`hoistSubAddAssociation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2762):
/// `(LV + C1) - C2` to `LV + (C1 - C2)`, `(LV - C1) - C2` to `LV - (C1 + C2)`,
/// `(LV - C1) + C2` to `LV + (C2 - C1)`; no wrap flags on the new
/// instructions. DEVIATE: as for [`hoist_bo_association`], an instruction
/// another reassociation of the loop has replaced is left alone.
fn hoist_sub_add_association(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    inst: InstId,
) -> Option<Reassociation> {
    let function = &*unit.function;
    let i = function.instruction(inst);
    let Opcode::Binary(op) = i.opcode else { return None };
    // The one-use operand `BO` defined by one of `wanted`, and the operand
    // that is not it.
    let one_use_def = |operand: Operand, wanted: &[BinaryOp]| -> Option<(InstId, BinaryOp)> {
        let Operand::Value(value) = operand else { return None };
        let ValueDef::Instruction(def) = function.value(value).def else { return None };
        let Opcode::Binary(bo_op) = function.instruction(def).opcode else { return None };
        (wanted.contains(&bo_op)
            && function.users(value).iter().count() == 1
            && !ctx.state.gone.borrow().contains(&def))
        .then_some((def, bo_op))
    };
    let (bo, lv, c1, c2, inv_op, result_op);
    match op {
        BinaryOp::Sub => {
            let (def, bo_op) = one_use_def(i.operands[0], &[BinaryOp::Add, BinaryOp::Sub])?;
            let (mut a, mut b) = (function.instruction(def).operands[0], function.instruction(def).operands[1]);
            c2 = i.operands[1];
            if bo_op == BinaryOp::Add {
                // Case 1.
                if ctx.is_loop_invariant(a) && !ctx.is_loop_invariant(b) {
                    std::mem::swap(&mut a, &mut b);
                }
                (inv_op, result_op) = (BinaryOp::Sub, BinaryOp::Add);
            } else {
                // Case 2.
                (inv_op, result_op) = (BinaryOp::Add, BinaryOp::Sub);
            }
            (bo, lv, c1) = (def, a, b);
        }
        BinaryOp::Add => {
            // Case 3, `m_c_Add`: the `sub` on either side.
            let (def, other) = if let Some((def, _)) = one_use_def(i.operands[0], &[BinaryOp::Sub]) {
                (def, i.operands[1])
            } else if let Some((def, _)) = one_use_def(i.operands[1], &[BinaryOp::Sub]) {
                (def, i.operands[0])
            } else {
                return None;
            };
            let sub = function.instruction(def);
            // `C1 = C2; C2 = sub's C1`: the invariant is `C2 - C1`.
            (bo, lv, c1, c2) = (def, sub.operands[0], other, sub.operands[1]);
            (inv_op, result_op) = (BinaryOp::Sub, BinaryOp::Add);
        }
        _ => return None,
    }
    if ctx.is_loop_invariant(lv) || !ctx.is_loop_invariant(c1) || !ctx.is_loop_invariant(c2) {
        return None;
    }
    // A value an earlier reassociation of the loop has erased is not read.
    let erased = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => ctx.state.gone.borrow().contains(&def),
            ValueDef::Argument(_) => false,
        },
        _ => false,
    };
    if erased(lv) || erased(c1) || erased(c2) {
        return None;
    }
    Some(Reassociation::SubAdd { i: inst, bo, inv_op, result_op, lv, c1, c2 })
}

/// [`isSafeToExecuteUnconditionally`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1507)
/// (`AllowSpeculation` is the run's, `LoopSafety::allow_speculation`).
fn is_safe_to_execute_unconditionally(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    safety: &LoopSafety,
    preheader: BlockId,
    inst: InstId,
) -> bool {
    if safety.is_guaranteed_to_execute(inst) {
        return true;
    }
    safety.allow_speculation()
        && (is_safe_to_speculatively_execute(unit, outer, inst) || load_in_range(unit, outer, ctx, inst))
        && !speculation_would_hold_the_frame(unit, outer, ctx, preheader)
}

/// DEVIATE LM2 (`range_deref`): a load whose offset SCEV keeps inside its
/// object.
fn load_in_range(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    inst: InstId,
) -> bool {
    let function = &*unit.function;
    let load = function.instruction(inst);
    let optsize = function
        .attrs
        .iter()
        .any(
            |attr| matches!(
                attr,
                llrm_mir::opcode::Attribute::Flag(flag) if flag == "optsize" || flag == "minsize"
            ),
        );
    if optsize
        || !crate::looprotate::target_tti(outer.target()).licm_range_deref()
        || !matches!(load.opcode, Opcode::Load { volatile: false, .. })
    {
        return false;
    }
    let bytes = unit.layout.store_size(&unit.context.types, load.ty);
    let session = ctx
        .state
        .scev
        .get_or_init(
            || std::cell::RefCell::new(llrm_analysis::scev::analysis::ScevSession::of(unit.context, function, outer)),
        );
    session
        .borrow_mut()
        .ask(
            unit.context,
            unit.layout,
            function,
            outer,
            |se, cx| super::range_deref::dereferenceable_by_range(se, cx, load.operands[0], bytes),
        )
}

/// DEVIATE LICM-FRAME (see `Tti::licm_speculation_keeps_frame_late`): a
/// speculated hoist into the entry block of an `optsize` function whose loop
/// calls.
fn speculation_would_hold_the_frame(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    preheader: BlockId,
) -> bool {
    let function = &*unit.function;
    if function.entry() != Some(preheader)
        || !function.attrs.iter().any(|attr| {
            matches!(
                attr,
                llrm_mir::opcode::Attribute::Flag(flag) if flag == "optsize" || flag == "minsize"
            )
        })
        || !crate::looprotate::target_tti(outer.target()).licm_speculation_keeps_frame_late()
    {
        return false;
    }
    ctx.loop_
        .body
        .iter()
        .any(
            |&block| function
                .iter_block(cfg::block(block))
                .any(|inst| matches!(
                    function.instruction(inst).opcode,
                    Opcode::Call(_) | Opcode::Invoke(_)
                )),
        )
}

/// [`isSafeToSpeculativelyExecuteWithOpcode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7575).
/// DEVIATE LM1: a load is safe where its bytes are dereferenceable or the
/// target does not trap on it at its alignment (`Machine::load_may_trap`), in
/// place of `isDereferenceableAndAlignedPointer` (the same rule as `hoist`'s
/// `_may_fault`). Exit: none; real mode faults only on an access that crosses
/// offset FFFFh (dos.toml), so the pin's rule is for a target that traps.
/// Pin-faithful, 424 bench rows change 2 (nbody_fixed nib -O2/-Os: -16 B,
/// -36,000 memory operands, +24,000 instructions, +60,000 clocks); the rule
/// was added for PLASMABLOBS `fsin3%(y% - k%)` leaving its pixel loop
/// (53b3bb9db8).
pub fn is_safe_to_speculatively_execute(
    unit: &passes::Unit,
    outer: &Outer,
    inst: InstId,
) -> bool {
    is_safe_to_speculatively_execute_with(unit.context, unit.layout, unit.function, outer, inst, true)
}

/// [`is_safe_to_speculatively_execute`] of the parts of a unit, for a caller
/// that holds the function mutably.
pub fn is_safe_to_speculatively_execute_in(
    context: &llrm_mir::context::Context,
    layout: &llrm_mir::datalayout::DataLayout,
    function: &Function,
    outer: &Outer,
    inst: InstId,
) -> bool {
    is_safe_to_speculatively_execute_with(context, layout, function, outer, inst, false)
}

/// [`is_safe_to_speculatively_execute_in`] with `IgnoreUBImplyingAttrs`
/// (`ValueTracking.h:590`: true by default, as `LICM.cpp:1513` asks). A caller
/// that moves a call with it drops the attributes
/// (`Function::drop_ub_implying_attrs`) where the call may not run.
pub fn is_safe_to_speculatively_execute_with(
    context: &llrm_mir::context::Context,
    layout: &llrm_mir::datalayout::DataLayout,
    function: &Function,
    outer: &Outer,
    inst: InstId,
    ignore_ub_implying_attrs: bool,
) -> bool {
    speculatable(context, layout, function, outer, inst, ignore_ub_implying_attrs, false)
}

/// The rule of SimplifyCFG's own speculation (`speculativelyExecuteBB`,
/// `foldTwoEntryPHINode`, `foldBranchToCommonDest`, hoisting and sinking common
/// code): [`is_safe_to_speculatively_execute_in`] where a load is safe only
/// if its bytes are dereferenceable and its pointer is known to be as aligned
/// as the load, as `isDereferenceableAndAlignedPointer` has it. LICM and the
/// other users keep the target's no-trap rule (DEVIATE LM1 of `hoist`).
pub fn is_safe_to_speculatively_execute_for_cfg(
    context: &llrm_mir::context::Context,
    layout: &llrm_mir::datalayout::DataLayout,
    function: &Function,
    outer: &Outer,
    inst: InstId,
) -> bool {
    speculatable(context, layout, function, outer, inst, false, true)
}

/// The alignment `isDereferenceableAndAlignedPointer` knows of `pointer`:
/// `Value::getPointerAlignment` of the object under constant-offset GEPs,
/// less what the offset breaks (a variable offset knows none).
fn pointer_alignment_at(
    context: &llrm_mir::context::Context,
    layout: &llrm_mir::datalayout::DataLayout,
    outer: &Outer,
    function: &Function,
    pointer: Operand,
) -> u64 {
    let query = llrm_mir::valuetracking::Query {
        globals: &outer.globals,
        ..llrm_mir::valuetracking::Query::bare(context, function, layout)
    };
    let mut at = pointer;
    let mut offset: i128 = 0;
    // A GEP is an instruction or a constant expression (`GEPOperator`).
    let gep_parts = |at: Operand| match at {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => match function.instruction(def).opcode {
                Opcode::GetElementPtr { source } => Some((source, function.instruction(def).operands.clone())),
                _ => None,
            },
            _ => None,
        },
        Operand::Constant(id) => match &context.get(id).kind {
            ConstantKind::Expr(llrm_mir::context::ConstantExpr::GetElementPtr { source, operands, .. }) => {
                Some((*source, operands.iter().map(|&one| Operand::Constant(one)).collect()))
            }
            _ => None,
        },
        Operand::Block(_) => None,
    };
    while let Some((source, operands)) = gep_parts(at) {
        let indices: Vec<Option<i128>> = operands[1..]
            .iter()
            .map(|&one| match one {
                Operand::Constant(id) => match context.get(id).kind {
                    ConstantKind::Int(bits) => {
                        Some(llrm_mir::context::signed(bits, context.types.int_bits(context.get(id).ty).unwrap_or(64)))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let (constant, variable) = layout.collect_offset(&context.types, source, &indices);
        if !variable.is_empty() {
            return 1;
        }
        offset += constant as i128;
        at = operands[0];
    }
    let base = llrm_mir::valuetracking::known::pointer_alignment(&query, at);
    if offset == 0 { base } else { base.min(1 << offset.trailing_zeros().min(63)) }
}

fn speculatable(
    context: &llrm_mir::context::Context,
    layout: &llrm_mir::datalayout::DataLayout,
    function: &Function,
    outer: &Outer,
    inst: InstId,
    ignore_ub_implying_attrs: bool,
    pin_loads: bool,
) -> bool {
    let instruction = function.instruction(inst);
    let constant_int = |operand: Operand| match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    match &instruction.opcode {
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem) => {
            constant_int(instruction.operands[1]).is_some_and(|d| d != 0)
        }
        Opcode::Binary(op @ (BinaryOp::SDiv | BinaryOp::SRem)) => {
            let _ = op;
            let width = context.types.int_bits(instruction.ty).expect("an integer division");
            let Some(denominator) = constant_int(instruction.operands[1]) else { return false };
            if denominator == 0 {
                return false;
            }
            if denominator != llrm_mir::context::mask(width) {
                return true;
            }
            // The denominator is -1: safe as long as the numerator is not
            // INT_MIN.
            constant_int(instruction.operands[0]).is_some_and(|numerator| numerator != 1u128 << (width - 1))
        }
        Opcode::Load { align, volatile } => {
            if *volatile {
                return false;
            }
            let (pointer, bytes) = (instruction.operands[0], layout.store_size(&context.types, instruction.ty));
            if pin_loads {
                // `isDereferenceableAndAlignedPointer`: the pointer's known
                // alignment must reach the load's own.
                let wanted = align.unwrap_or_else(|| layout.align(&context.types, instruction.ty));
                return pointer_alignment_at(context, layout, outer, function, pointer) >= wanted
                    && llrm_mir::valuetracking::dereferenceable(
                        context,
                        layout,
                        outer.sizes(),
                        function,
                        pointer,
                        bytes,
                    );
            }
            llrm_mir::valuetracking::dereferenceable(context, layout, outer.sizes(), function, pointer, bytes)
                || !outer.target().load_may_trap(
                    bytes,
                    llrm_mir::valuetracking::alignment(context, layout, &outer.globals, function, pointer),
                )
        }
        Opcode::Call(info) => {
            // The called function could have undefined behavior or
            // side-effects, even if marked readnone nounwind.
            if !calls::has_flag(outer, context, function, inst, "speculatable") {
                return false;
            }
            // Since the operands may be changed after hoisting, undefined
            // behavior may be triggered by some UB-implying attributes.
            let implies_ub = |attrs: &Vec<Attribute>| {
                Facts::of(attrs).iter().any(Fact::implies_ub)
                    || attrs.iter().any(|attr| {
                        matches!(
                            attr,
                            Attribute::Int(name, _) if name == "dereferenceable_or_null"
                        )
                    })
            };
            ignore_ub_implying_attrs || !(implies_ub(&info.return_attrs) || info.argument_attrs.iter().any(implies_ub))
        }
        Opcode::Alloca { .. }
        | Opcode::Invoke(_)
        | Opcode::Phi
        | Opcode::Store { .. }
        | Opcode::Ret
        | Opcode::Br
        | Opcode::Switch
        | Opcode::Unreachable
        | Opcode::LandingPad { .. }
        | Opcode::Resume => false,
        _ => true,
    }
}

/// [`isHoistableAndSinkableInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L890).
fn is_hoistable_and_sinkable_inst(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Load { .. }
            | Opcode::Store { .. }
            | Opcode::Call(_)
            | Opcode::Cast(_)
            | Opcode::FNeg
            | Opcode::Binary(_)
            | Opcode::Select
            | Opcode::GetElementPtr { .. }
            | Opcode::ICmp(_)
            | Opcode::FCmp(_)
            | Opcode::ExtractValue(_)
            | Opcode::InsertValue(_)
            | Opcode::Freeze
    )
}

/// [`canSinkOrHoistInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L973).
#[allow(clippy::too_many_arguments)]
pub fn can_sink_or_hoist_inst(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    flags: &mut Flags,
    safety: &LoopSafety,
    inst: InstId,
    target_executes_once_per_loop: bool,
) -> bool {
    let _ = (safety, target_executes_once_per_loop);
    let function = &*unit.function;
    let instruction = function.instruction(inst);
    // If we don't understand the instruction, bail early.
    if !is_hoistable_and_sinkable_inst(&instruction.opcode) {
        return false;
    }
    match &instruction.opcode {
        // Loads have extra constraints we have to verify before we can hoist
        // them.
        Opcode::Load { .. } => can_hoist_load(unit, ctx, mssa, aa, flags, inst),
        Opcode::Call(_) => {
            // Don't sink calls which can throw.
            if calls::may_throw(outer, unit.context, function, inst) {
                return false;
            }
            // Convergent operations may not move across control flow.
            if calls::has_flag(outer, unit.context, function, inst, "convergent") {
                return false;
            }
            // Handle simple cases by querying alias analysis.
            let (reads, writes) = call_behavior(aa, inst);
            if !reads && !writes {
                return true;
            }
            if !writes {
                // Might have stale MemoryDef for call that was later inferred
                // to be read-only.
                let Some(access) = mssa.get_memory_access(inst) else { return false };
                if mssa.access(access).kind != Kind::Use {
                    return false;
                }
                // If we can prove there are no writes to the memory read by the
                // call, we can hoist or sink.
                return !pointer_invalidated_by_loop(ctx, mssa, aa, access, inst, flags, false);
            }
            if !reads {
                // Can hoist or sink if there are no conflicting read/writes to
                // the memory location written to by the call.
                return no_conflicting_read_writes(ctx, mssa, aa, flags, inst);
            }
            false
        }
        Opcode::Store { volatile, .. } => {
            if *volatile {
                // Don't sink/hoist volatile or ordered atomic store!
                return false;
            }
            // We can only hoist a store that we can prove writes a value which
            // is not read or overwritten within the loop. For those cases, we
            // fall back to load store promotion instead.
            if is_only_memory_access(ctx, mssa, inst) {
                return true;
            }
            no_conflicting_read_writes(ctx, mssa, aa, flags, inst)
        }
        // We've established mechanical ability and aliasing, it's up to the
        // caller to check fault safety.
        _ => true,
    }
}

/// `AA->getMemoryEffects(CI)`: whether the call reads and writes memory.
fn call_behavior(
    aa: &AliasQuery,
    call: InstId,
) -> (bool, bool) {
    aa.call_effects(call)
}

/// `licm-max-num-uses-traversed`.
const MAX_NUM_USES_TRAVERSED: u32 = 8;

/// [`isLoadInvariantInLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1053):
/// an `invariant.start` over the load's address and at least its size, with
/// no use of its own, stands in a block that properly dominates the loop's
/// header.
fn is_load_invariant_in_loop(
    ctx: &LoopCtx,
    aa: &AliasQuery,
    load: InstId,
) -> bool {
    let unit = aa.unit();
    let function = unit.function;
    let instruction = function.instruction(load);
    // A constant address: no use list to walk.
    let Operand::Value(address) = instruction.operands[0] else { return false };
    let bits = unit.layout.size_bits(&unit.context.types, instruction.ty);
    let mut visited = 0;
    for user in function.users(address).iter().map(|one| one.user) {
        // Avoid traversing for a load operand with a high number of users.
        visited += 1;
        if visited > MAX_NUM_USES_TRAVERSED {
            return false;
        }
        if unit.intrinsic(user) != Some(llrm_mir::intrinsics::Intrinsic::InvariantStart) {
            continue;
        }
        // If there are escaping uses of the invariant.start, the load may be
        // non-invariant.
        // DEVIATE: the pin takes any use, an `invariant.end` included, as
        // making the load non-invariant
        // (https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1086
        // `II->use_empty()`). Here a scope whose ends cannot run between the
        // start and the loop header still counts, so a frontend can close a
        // scope after the loop. Exit: the pin gains the same dominance check.
        if !scope_holds_at(ctx, aa, user) {
            continue;
        }
        let Some(size) = llrm_mir::intrinsics::length(unit.context, function.instruction(user).operands[0]) else {
            continue;
        };
        // -1 stands for a variable sized object.
        if size > i64::MAX as u64 {
            continue;
        }
        // The invariant.start should dominate the load, and the load must not
        // be hoisted out of a loop that contains this dominating one.
        let Some(block) = function.parent(user) else { continue };
        if bits <= size * 8
            && cfg::id(block) != ctx.loop_.header
            && ctx.shape.dominance.dominates(function, cfg::id(block), ctx.loop_.header)
        {
            return true;
        }
    }
    false
}

/// Whether the scope `start` opens is still open at the loop's header: every
/// use of it is an `invariant.end` on it, and none of those can run after the
/// start and before the header is reached, on a path that does not restart the
/// scope. With no use at all, that is the pin's `use_empty()`.
fn scope_holds_at(
    ctx: &LoopCtx,
    aa: &AliasQuery,
    start: InstId,
) -> bool {
    let unit = aa.unit();
    let function = unit.function;
    let Some(scope) = function.instruction(start).result else { return true };
    let mut ends = Vec::new();
    for one in function.users(scope).iter() {
        let end = unit.intrinsic(one.user) == Some(llrm_mir::intrinsics::Intrinsic::InvariantEnd) && one.index == 0;
        match function.parent(one.user) {
            Some(block) if end => ends.push((one.user, block)),
            // A phi, a store or any other use may end it where we cannot see.
            _ => return false,
        }
    }
    if ends.is_empty() {
        return true;
    }
    let Some(opened) = function.parent(start) else { return false };
    let (opened, header) = (cfg::id(opened), ctx.loop_.header);
    // The blocks the start reaches, and the blocks that reach the header, each
    // without passing through the start's block.
    let after = walk(opened, opened, |at| cfg::successors_of(function, at).into_iter().collect());
    let before = walk(header, opened, |at| cfg::predecessors_of(function, at).into_iter().collect());
    ends.into_iter().all(|(end, block)| {
        let at = cfg::id(block);
        if at == opened {
            // Ahead of the start in its block, the start reopens the scope.
            return function.comes_before(end, start);
        }
        !(after.contains(&at) && before.contains(&at))
    })
}

/// The blocks `next` reaches from `from` without expanding `stop`.
fn walk(
    from: i64,
    stop: i64,
    next: impl Fn(i64) -> Vec<i64>,
) -> std::collections::BTreeSet<i64> {
    let mut seen = std::collections::BTreeSet::new();
    let mut work = vec![from];
    while let Some(at) = work.pop() {
        for to in next(at) {
            if seen.insert(to) && to != stop {
                work.push(to);
            }
        }
    }
    seen
}

/// [`canHoistLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L932).
/// No `invariant.group` in MIR, no atomic loads.
pub fn can_hoist_load(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    flags: &mut Flags,
    load: InstId,
) -> bool {
    if matches!(
        unit.function.instruction(load).opcode,
        Opcode::Load { volatile: true, .. }
    ) {
        return false; // Don't sink/hoist volatile or ordered atomic loads!
    }
    // Loads from constant memory are always safe to move, even if they end up
    // in the same alias set as something that ends up being modified. Loads
    // marked `invariant` are too.
    if aa.is_use_trivially_optimizable_to_live_on_entry(load) {
        return true;
    }
    // This checks for an invariant.start dominating the load.
    if is_load_invariant_in_loop(ctx, aa, load) {
        return true;
    }
    let Some(access) = mssa.get_memory_access(load) else { return false };
    !pointer_invalidated_by_loop(ctx, mssa, aa, access, load, flags, false)
}

/// [`isOnlyMemoryAccess`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L902):
/// `inst` is the only instruction with a memory access in the loop.
fn is_only_memory_access(
    ctx: &LoopCtx,
    mssa: &Mssa,
    inst: InstId,
) -> bool {
    for &at in ctx.loop_.body.iter() {
        if let Some(accesses) = mssa.get_block_accesses(cfg::block(at)) {
            let mut not_a_phi = 0;
            for &access in accesses {
                let one = mssa.access(access);
                if one.kind == Kind::Phi {
                    continue;
                }
                let first = not_a_phi == 0;
                not_a_phi += 1;
                if one.inst != Some(inst) || !first {
                    return false;
                }
            }
        }
    }
    true
}

/// [`getClobberingMemoryAccess`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L919):
/// the skip-self walk, until the loop has asked `LicmMssaOptCap` of them.
fn get_clobbering_memory_access(
    mssa: &mut Mssa,
    aa: &AliasQuery,
    flags: &mut Flags,
    access: AccessId,
) -> AccessId {
    // See the declaration of `SetLicmMssaOptCap` for usage details.
    if flags.too_many_clobbering_calls() {
        return mssa.get_defining_access(access).expect("a defining access");
    }
    let source = mssa.get_clobbering_memory_access_skip_self(aa, access);
    flags.increment_clobbering_calls();
    source
}

/// [`noConflictingReadWrites`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2153).
fn no_conflicting_read_writes(
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    flags: &mut Flags,
    inst: InstId,
) -> bool {
    // If there are more accesses than the Promotion cap, then give up as we're
    // not walking a list that long.
    if flags.too_many_memory_accesses() {
        return false;
    }
    let imd = mssa.get_memory_access(inst).expect("an access of the instruction");
    let source = get_clobbering_memory_access(mssa, aa, flags, imd);
    // Make sure there are no clobbers inside the loop.
    if !mssa.is_live_on_entry_def(source) && ctx.contains_block(mssa.access(source).block) {
        return false;
    }
    // If there are interfering Uses don't move this store.
    for &at in ctx.loop_.body.iter() {
        let Some(accesses) = mssa.get_block_accesses(cfg::block(at)) else { continue };
        for &access in accesses {
            // Accesses are ordered. If we find one that I dominates we can
            // stop.
            if !flags.is_sink() && mssa.dominates(imd, access) {
                break;
            }
            if let Some(other) = mssa.access(access).inst {
                // Skip unrelated accesses.
                if !aa.mod_ref_instructions(other, inst) {
                    continue;
                }
                return false;
            }
        }
    }
    true
}

/// [`pointerInvalidatedByLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2193).
fn pointer_invalidated_by_loop(
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    mu: AccessId,
    inst: InstId,
    flags: &mut Flags,
    invariant_group: bool,
) -> bool {
    // For hoisting, use the walker to determine safety.
    if !flags.is_sink() {
        let source = get_clobbering_memory_access(mssa, aa, flags, mu);
        return !mssa.is_live_on_entry_def(source)
            && ctx.contains_block(mssa.access(source).block)
            && !(invariant_group
                && cfg::id(mssa.access(source).block) == ctx.loop_.header
                && mssa.access(source).kind == Kind::Phi);
    }
    // For sinking, we'd need to check all Defs below this use. The
    // getClobbering call will look on the backedge of the loop, but will check
    // aliasing with the instructions on the previous iteration. So: only sink
    // if there are no Defs in the loop, and the existing ones precede the use
    // and are in the same block.
    if flags.too_many_memory_accesses() {
        return true;
    }
    for &at in ctx.loop_.body.iter() {
        if pointer_invalidated_by_block(mssa, cfg::block(at), mu) {
            return true;
        }
    }
    // When sinking, the source block may not be part of the loop so check it.
    if !ctx.contains(inst) {
        let parent = ctx.function.parent(inst).expect("an instruction in a block");
        return pointer_invalidated_by_block(mssa, parent, mu);
    }
    false
}

/// [`pointerInvalidatedByBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2243).
fn pointer_invalidated_by_block(
    mssa: &Mssa,
    block: BlockId,
    mu: AccessId,
) -> bool {
    if let Some(defs) = mssa.get_block_defs(block) {
        for &def in defs {
            if mssa.access(def).kind == Kind::Def
                && (mssa.access(mu).block != mssa.access(def).block || !mssa.locally_dominates(def, mu))
            {
                return true;
            }
        }
    }
    false
}
