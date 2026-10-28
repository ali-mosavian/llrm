//! Exit counts found by running the loop: `computeShiftCompareExitLimit` and
//! `computeExitCountExhaustively`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ScalarEvolution.cpp`. A shift
//! recurrence (`%iv = phi [start, %pre], [%iv >> c, %loop]`) reaches 0 or -1
//! in at most `bitwidth` iterations; if the compare is false there, the
//! backedge is taken a bounded number of times.
//!
//! # Example
//! `while (x >>= 1)` over an `i32` is taken at most 32 times.

use llrm_mir::constantfolding;
use llrm_mir::context::{ConstantId, ConstantKind};
use llrm_mir::dense::IdMap;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, GlobalKind, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, IntPredicate, Opcode};
use llrm_mir::valuetracking::known::compute_known_bits_at;
use llrm_mir::valuetracking::pattern;

use super::ap::Ap;
use super::expr::{Scev, Ty};
use super::trip::ExitLimit;
use super::{Cx, ScalarEvolution};

/// `ConstantFoldCompareInstOperands` of two integers.
fn compare(
    pred: IntPredicate,
    x: Ap,
    y: Ap,
) -> bool {
    match pred {
        IntPredicate::Eq => x == y,
        IntPredicate::Ne => x != y,
        IntPredicate::Ugt => y.ult(x),
        IntPredicate::Uge => y.ule(x),
        IntPredicate::Ult => x.ult(y),
        IntPredicate::Ule => x.ule(y),
        IntPredicate::Sgt => y.slt(x),
        IntPredicate::Sge => y.sle(x),
        IntPredicate::Slt => x.slt(y),
        IntPredicate::Sle => x.sle(y),
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9594
    /// `computeShiftCompareExitLimit`.
    pub(super) fn compute_shift_compare_exit_limit(
        &mut self,
        cx: &Cx,
        header: BlockId,
        lhs: Operand,
        rhs_v: Operand,
        pred: IntPredicate,
    ) -> ExitLimit {
        let query = cx.query(None);
        let loop_ = cx.loops.get_loop_for(header).expect("the header of a loop");
        let Some((rhs_bits, width)) = pattern::apint(&query, rhs_v) else {
            return ExitLimit::could_not_compute(self);
        };
        let rhs = Ap::new(width, rhs_bits);

        let Some(latch) = loop_.get_loop_latch(cx.function) else { return ExitLimit::could_not_compute(self) };
        let Some(predecessor) = loop_.get_loop_predecessor(cx.function) else {
            return ExitLimit::could_not_compute(self);
        };

        // Return true if V is of the form "LHS `shift_op` <positive
        // constant>". Return LHS in OutLHS, shift_op in OutOpCode, and the
        // shift amount in OutShiftAmt.
        let match_positive_shift = |v: Operand| -> Option<(Operand, BinaryOp, u32)> {
            let (_, instruction) = query.instruction_of(v)?;
            let Opcode::Binary(op @ (BinaryOp::LShr | BinaryOp::AShr | BinaryOp::Shl)) = instruction.opcode else {
                return None;
            };
            let (amount, _) = pattern::apint(&query, instruction.operands[1])?;
            let limited = u32::try_from(amount).unwrap_or(u32::MAX);
            let out_lhs = instruction.operands[0];
            let scalar_bits = query.scalar_bits(out_lhs);
            if limited == 0 || limited >= scalar_bits {
                return None;
            }
            Some((out_lhs, op, limited))
        };

        // Recognize a "shift recurrence" either of the form %iv or of
        // %iv.shifted in
        //
        // loop:
        //   %iv = phi i32 [ %iv.shifted, %loop ], [ %val, %preheader ]
        //   %iv.shifted = lshr i32 %iv, <positive constant>
        //
        // Return true on a successful match. Return the corresponding PHI node
        // (%iv above) in PNOut, the opcode of the shift operation in
        // OpCodeOut, and the shift amount in ShiftAmtOut.
        let mut lhs = lhs;
        let mut post_shift_op: Option<BinaryOp> = None;
        // If we encounter a shift instruction, "peel off" the shift operation,
        // and remember that we did so. Later when we inspect %iv's backedge
        // value, we will make sure that the backedge value uses the same
        // operation.
        if let Some((inner, op, _)) = match_positive_shift(lhs) {
            post_shift_op = Some(op);
            lhs = inner;
        }

        let Some((phi_inst, phi)) = query.instruction_of(lhs) else { return ExitLimit::could_not_compute(self) };
        if phi.opcode != Opcode::Phi || cx.function.parent(phi_inst) != Some(header) {
            return ExitLimit::could_not_compute(self);
        }
        let pn = lhs;

        let Some(be_value) = pattern::phi_incoming(phi).find(|&(_, from)| from == latch).map(|(value, _)| value) else {
            return ExitLimit::could_not_compute(self);
        };
        let Some((op_lhs, op_code, shift_amt)) = match_positive_shift(be_value) else {
            // The backedge value for the PHI node must be a shift by a
            // positive amount.
            return ExitLimit::could_not_compute(self);
        };
        // of the PHI node itself, and the kind of shift should be match the
        // kind of shift we peeled off, if any.
        if op_lhs != pn || post_shift_op.is_some_and(|op| op != op_code) {
            return ExitLimit::could_not_compute(self);
        }

        // The key rationale for this optimization is that for some kinds of
        // shift recurrences, the value of the recurrence "stabilizes" to
        // either 0 or -1 within a finite number of iterations. If the
        // condition guarding the backedge (in the sense that the backedge is
        // taken if the condition is true) is false for the value the shift
        // recurrence stabilizes to, then we know that the backedge is taken
        // only a finite number of times.
        let stable_value = match op_code {
            BinaryOp::AShr => {
                // {K,ashr,<positive-constant>} stabilizes to signum(K) in at
                // most bitwidth(K) iterations.
                let Some(first_value) =
                    pattern::phi_incoming(phi).find(|&(_, from)| from == predecessor).map(|(value, _)| value)
                else {
                    return ExitLimit::could_not_compute(self);
                };
                let known = compute_known_bits_at(&query, first_value, cx.function.terminator(predecessor), 0);
                if known.is_non_negative() {
                    Ap::zero(width)
                } else if known.is_negative() {
                    Ap::all_ones(width)
                } else {
                    return ExitLimit::could_not_compute(self);
                }
            }
            // Both {K,lshr,<positive-constant>} and {K,shl,<positive-constant>}
            // stabilize to 0 in at most bitwidth(K) iterations.
            _ => Ap::zero(width),
        };

        let result = compare(pred, stable_value, rhs);

        if !result {
            let bit_width = self.get_type_size_in_bits(cx, self.ty_of_operand(cx, rhs_v));
            let mut max_btc = bit_width;

            // For right-shift recurrences (lshr/ashr with non-negative start),
            // we can compute a tighter max backedge-taken count from the range
            // of the start value. After k shifts of ShiftAmt, value =
            // start >> (k * ShiftAmt). The value reaches 0 (the stable value)
            // when k * ShiftAmt >= activeBits(start), so max BTC =
            // ceil(activeBits(maxStart) / ShiftAmt).
            if matches!(op_code, BinaryOp::LShr | BinaryOp::AShr)
                && let Some(start_value) =
                    pattern::phi_incoming(phi).find(|&(_, from)| from == predecessor).map(|(value, _)| value)
            {
                let start_scev = self.get_scev(cx, start_value);
                let max_start = self.get_unsigned_range_max(cx, start_scev);
                if !max_start.is_zero() {
                    let active_bits = max_start.active_bits();
                    let range_btc = active_bits.div_ceil(shift_amt);
                    max_btc = max_btc.min(range_btc);
                }
            }

            let ty = self.ty_of_operand(cx, rhs_v);
            let ty = self.get_effective_scev_type(cx, ty);
            let upper_bound = self.get_constant_of_type(cx, ty, i128::from(max_btc));
            let unknown = self.get_could_not_compute();
            return ExitLimit::new(self, unknown, upper_bound, upper_bound, false);
        }

        ExitLimit::could_not_compute(self)
    }

    /// The SCEV type of a value's IR type.
    fn ty_of_operand(
        &self,
        cx: &Cx,
        v: Operand,
    ) -> super::expr::Ty {
        let ty = cx.function.operand_type(cx.context, v).expect("a typed value");
        self.ty_of(cx, ty).expect("a SCEVable value")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10012
    /// `computeExitCountExhaustively`: run the loop on constants until `cond`
    /// is `exit_when`.
    pub(super) fn compute_exit_count_exhaustively(
        &mut self,
        cx: &Cx,
        header: BlockId,
        cond: Operand,
        exit_when: bool,
    ) -> Scev {
        let Some(l) = cx.loops.get_loop_for(header) else { return self.get_could_not_compute() };
        let Some(pn) = get_constant_evolving_phi(cx, &l, cond) else { return self.get_could_not_compute() };

        // If the loop is canonicalized, the PHI will have exactly two entries.
        // That's the only form we support here.
        if cx.function.instruction(pn).operands.len() != 4 {
            return self.get_could_not_compute();
        }

        let mut current: IdMap<InstId, ConstantId> = IdMap::new();
        let Some(latch) = l.get_loop_latch(cx.function) else { return self.get_could_not_compute() };
        let phis: Vec<InstId> = header_phis(cx, header);
        for &phi in &phis {
            if let Some(start) = get_other_incoming_value(cx, phi, latch) {
                current.insert(phi, start);
            }
        }
        if !current.contains_key(&pn) {
            return self.get_could_not_compute();
        }

        // Okay, we find a PHI node that defines the trip count of this loop.
        // Execute the loop symbolically to determine when the condition gets a
        // value of "ExitWhen".
        for iteration in 0..MAX_BRUTE_FORCE_ITERATIONS {
            let value = evaluate_expression(cx, &l, cond, &mut current).map(|c| cx.context.get(c).kind.clone());
            let Some(ConstantKind::Int(bits)) = value else {
                // Couldn't symbolically evaluate.
                return self.get_could_not_compute();
            };
            if bits == u128::from(exit_when) {
                return self.get_constant_of_type(cx, Ty::Int(32), i128::from(iteration));
            }

            // Update all the PHI nodes for the next iteration.
            let mut next: IdMap<InstId, ConstantId> = IdMap::new();
            let to_compute: Vec<InstId> = phis.iter().copied().filter(|phi| current.contains_key(phi)).collect();
            for phi in to_compute {
                let be_value = incoming_for_block(cx, phi, latch);
                if let Some(c) = be_value.and_then(|v| evaluate_expression(cx, &l, v, &mut current)) {
                    next.insert(phi, c);
                }
            }
            current = next;
        }

        // Too many iterations were needed to evaluate.
        self.get_could_not_compute()
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9919
    /// `getConstantEvolutionLoopExitValue`: the value `pn`, a phi of the header
    /// of `l`, has when `l` exits after `bes` backedges, by running the
    /// loop on constants; none where it cannot be.
    ///
    /// DEVIATE: no cache per phi (`ConstantEvolutionLoopExitValue`);
    /// `getSCEVAtScope` memoizes the result per value and scope.
    pub(super) fn get_constant_evolution_loop_exit_value(
        &mut self,
        cx: &Cx,
        pn: InstId,
        bes: u128,
        l: &Loop,
    ) -> Option<ConstantId> {
        // Not going to evaluate it.
        if bes > u128::from(MAX_BRUTE_FORCE_ITERATIONS) {
            return None;
        }
        let header = l.get_header();
        let latch = l.get_loop_latch(cx.function)?;
        let phis = header_phis(cx, header);
        let mut current: IdMap<InstId, ConstantId> = IdMap::new();
        for &phi in &phis {
            if let Some(start) = get_other_incoming_value(cx, phi, latch) {
                current.insert(phi, start);
            }
        }
        if !current.contains_key(&pn) {
            return None;
        }
        let back_edge_value = incoming_for_block(cx, pn, latch)?;
        // Execute the loop symbolically to determine the exit value.
        let iterations = bes as u32;
        let mut iteration = 0;
        loop {
            if iteration == iterations {
                // Got exit value!
                return current.get(&pn).copied();
            }
            // Compute the value of the PHIs for the next iteration.
            // `evaluate_expression` adds the non-phi values to
            // `current`.
            let mut next: IdMap<InstId, ConstantId> = IdMap::new();
            let next_phi = evaluate_expression(cx, l, back_edge_value, &mut current)?;
            next.insert(pn, next_phi);
            let mut stopped_evolving = Some(&next_phi) == current.get(&pn);
            // Also evaluate the other phis. We do not get to stop if we find
            // that all of them are constant evolving.
            let others: Vec<(InstId, ConstantId)> = phis
                .iter()
                .filter(|&&phi| phi != pn)
                .filter_map(|&phi| current.get(&phi).map(|&value| (phi, value)))
                .collect();
            for (phi, previous) in others {
                let value = match next.get(&phi) {
                    Some(&done) => Some(done),
                    None => {
                        incoming_for_block(cx, phi, latch).and_then(|v| evaluate_expression(cx, l, v, &mut current))
                    }
                };
                if let Some(value) = value {
                    next.insert(phi, value);
                }
                if value != Some(previous) {
                    stopped_evolving = false;
                }
            }
            // If all the entries of `current` equal those of `next`, the loop
            // cannot continue to change.
            if stopped_evolving {
                return current.get(&pn).copied();
            }
            current = next;
            iteration += 1;
        }
    }
}

/// `MaxBruteForceIterations`: the `cl::opt` default.
const MAX_BRUTE_FORCE_ITERATIONS: u32 = 100;

/// `MaxConstantEvolvingDepth`.
const MAX_CONSTANT_EVOLVING_DEPTH: u32 = 100;

fn header_phis(
    cx: &Cx,
    header: BlockId,
) -> Vec<InstId> {
    cx.function.iter_block(header).take_while(|&inst| cx.function.instruction(inst).opcode == Opcode::Phi).collect()
}

/// The instruction that defines `v`.
fn instruction_of(
    cx: &Cx,
    v: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = v else { return None };
    match cx.function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// `PHI->getIncomingValueForBlock(bb)`.
fn incoming_for_block(
    cx: &Cx,
    phi: InstId,
    bb: BlockId,
) -> Option<Operand> {
    let want = Operand::Block(bb);
    cx.function.instruction(phi).operands.chunks(2).find(|pair| pair[1] == want).map(|pair| pair[0])
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9904
/// `getOtherIncomingValue`: if every incoming value of `phi` except the one
/// for `bb` is one specific constant, that constant.
fn get_other_incoming_value(
    cx: &Cx,
    phi: InstId,
    bb: BlockId,
) -> Option<ConstantId> {
    let mut incoming: Option<ConstantId> = None;
    for pair in cx.function.instruction(phi).operands.chunks(2) {
        if pair[1] == Operand::Block(bb) {
            continue;
        }
        let Operand::Constant(current) = pair[0] else { return None };
        if incoming != Some(current) {
            if incoming.is_some() {
                return None;
            }
            incoming = Some(current);
        }
    }
    incoming
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9764
/// `canConstantFold`: an instruction of this kind folds when its operands
/// are constants.
pub(super) fn can_constant_fold(
    cx: &Cx,
    inst: InstId,
) -> bool {
    let instruction = cx.function.instruction(inst);
    match &instruction.opcode {
        Opcode::Binary(_)
        | Opcode::FNeg
        | Opcode::GetElementPtr { .. }
        | Opcode::Freeze
        | Opcode::ICmp(_)
        | Opcode::FCmp(_)
        | Opcode::Select
        | Opcode::Cast(_)
        | Opcode::Load { .. }
        | Opcode::ExtractValue(_)
        | Opcode::InsertValue(_) => true,
        Opcode::Call(info) => {
            let Some(&Operand::Constant(callee)) = instruction.operands.last() else { return false };
            let ConstantKind::Global(id) = cx.context.get(callee).kind else { return false };
            let Some(global) = cx.globals.get(id.0 as usize) else { return false };
            let (GlobalKind::Function(function), Some(name)) = (&global.kind, global.name.as_deref()) else {
                return false;
            };
            constantfolding::can_constant_fold_call_to(info, function, name)
        }
        _ => false,
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9780
/// `canConstantEvolve`.
fn can_constant_evolve(
    cx: &Cx,
    l: &Loop,
    inst: InstId,
) -> bool {
    // An instruction outside of the loop can't be derived from a loop PHI.
    if !l.contains_instruction(cx.function, inst) {
        return false;
    }
    if cx.function.instruction(inst).opcode == Opcode::Phi {
        // We don't currently keep track of the control flow needed to evaluate
        // PHIs, so we cannot handle PHIs inside of loops.
        return cx.function.parent(inst) == Some(l.get_header());
    }
    // If we won't be able to constant fold this expression even if the
    // operands are constants, bail early.
    can_constant_fold(cx, inst)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9800
/// `getConstantEvolvingPHIOperands`.
fn get_constant_evolving_phi_operands(
    cx: &Cx,
    l: &Loop,
    use_inst: InstId,
    phi_map: &mut IdMap<InstId, Option<InstId>>,
    depth: u32,
) -> Option<InstId> {
    if depth > MAX_CONSTANT_EVOLVING_DEPTH {
        return None;
    }

    // Otherwise, we can evaluate this instruction if all of its operands are
    // constant or derived from a PHI node themselves.
    let mut phi: Option<InstId> = None;
    for &op in &cx.function.instruction(use_inst).operands {
        if matches!(op, Operand::Constant(_)) {
            continue;
        }
        let op_inst = instruction_of(cx, op).filter(|&one| can_constant_evolve(cx, l, one))?;

        let mut p = if cx.function.instruction(op_inst).opcode == Opcode::Phi {
            Some(op_inst)
        } else {
            // If this operand is already visited, reuse the prior result. We
            // may have P != PHI if this is the deepest point at which the
            // inconsistent paths meet.
            phi_map.get(&op_inst).copied().flatten()
        };
        if p.is_none() {
            // Recurse and memoize the results, whether a phi is found or not.
            p = get_constant_evolving_phi_operands(cx, l, op_inst, phi_map, depth + 1);
            phi_map.insert(op_inst, p);
        }
        // Not evolving from PHI.
        let p = p?;
        // Evolving from multiple different PHIs.
        if phi.is_some_and(|one| one != p) {
            return None;
        }
        phi = Some(p);
    }
    // This is a expression evolving from a constant PHI!
    phi
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9843
/// `getConstantEvolvingPHI`: a phi in the loop that `v` is derived from,
/// through operations whose other operands are constants.
fn get_constant_evolving_phi(
    cx: &Cx,
    l: &Loop,
    v: Operand,
) -> Option<InstId> {
    let inst = instruction_of(cx, v).filter(|&one| can_constant_evolve(cx, l, one))?;
    if cx.function.instruction(inst).opcode == Opcode::Phi {
        return Some(inst);
    }
    // Record non-constant instructions contained by the loop.
    let mut phi_map = IdMap::new();
    get_constant_evolving_phi_operands(cx, l, inst, &mut phi_map, 0)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9861
/// `EvaluateExpression`: the value of `v` given the values of the loop's
/// phis in `vals`, if it folds.
fn evaluate_expression(
    cx: &Cx,
    l: &Loop,
    v: Operand,
    vals: &mut IdMap<InstId, ConstantId>,
) -> Option<ConstantId> {
    // Convenient constant check, but redundant for recursive calls.
    if let Operand::Constant(c) = v {
        return Some(c);
    }
    let inst = instruction_of(cx, v)?;

    if let Some(&c) = vals.get(&inst) {
        return Some(c);
    }

    // An instruction inside the loop depends on a value outside the loop that
    // we weren't given a mapping for, or a value such as a call inside the
    // loop.
    if !can_constant_evolve(cx, l, inst) {
        return None;
    }

    // An unmapped PHI can be due to a branch or another loop inside this loop,
    // or due to this not being the initial iteration through a loop where we
    // couldn't compute the evolution of this particular PHI last time.
    if cx.function.instruction(inst).opcode == Opcode::Phi {
        return None;
    }

    let operands = cx.function.instruction(inst).operands.clone();
    let mut folded: Vec<ConstantId> = Vec::with_capacity(operands.len());
    for operand in operands {
        let c = match instruction_of(cx, operand) {
            Some(op_inst) => {
                let c = evaluate_expression(cx, l, Operand::Value(cx.function.instruction(op_inst).result?), vals)?;
                vals.insert(op_inst, c);
                c
            }
            None => match operand {
                Operand::Constant(c) => c,
                _ => return None,
            },
        };
        folded.push(c);
    }

    let env = constantfolding::Env { globals: cx.globals, layout: cx.layout };
    constantfolding::constant_fold_inst_operands_at(cx.context, env, cx.function, inst, &folded)
}
