//! HashRecognize: a translation of LLVM's `HashRecognize.cpp` at
//! `d1106deb71cc`, the CRC recognizer `LoopIdiomRecognize` asks about a
//! loop, and the Sarwate table it builds.
//!
//! # How it works
//! [`recognize_crc`] accepts an innermost one-block loop with a canonical
//! induction variable, a small constant trip count and a conditional
//! recurrence on one bit of the running value: `crc = bit ? (crc >> 1) ^ poly :
//! crc >> 1` (little-endian) or the `<< 1` form on the top bit (big-endian),
//! with an optional second recurrence for the data. Every instruction of the
//! block must be reached from the computed value, the induction step, the
//! latch compare and the latch branch.
//!
//! ```text
//!   h: %i   = phi [0, pre], [%i.next, h]      %crc = select (bit %crc),
//!      %crc = phi [%init, pre], [%step, h]            (%crc >> 1) ^ poly,
//!      ...                                            (%crc >> 1)
//! ```
//!
//! # Deviations
//! None; the C++ `print`/`dump` methods and the Barrett constants (carry-less
//! multiply, which no target here has) are not translated.
//!
//! # See also
//! [`HashRecognize.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/HashRecognize.cpp).

use llrm_analysis::scev::expr::Type as ScevType;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::constantrange::Range;
use llrm_mir::context::ConstantKind;
use llrm_mir::dense::IdSet;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::pattern::match_simple_recurrence;

/// `PolynomialInfo`.
#[derive(Clone, Debug)]
pub struct PolynomialInfo {
    pub trip_count: u32,
    pub lhs: Operand,
    /// The generating polynomial, `rhs` bits wide (the CRC's width).
    pub rhs: u128,
    pub rhs_width: u32,
    pub computed_value: InstId,
    pub is_big_endian: bool,
    pub lhs_aux: Option<Operand>,
}

/// `CRCTable`: 256 entries of the CRC's width.
pub type CrcTable = [u128; 256];

fn mask(width: u32) -> u128 {
    if width >= 128 { u128::MAX } else { (1u128 << width) - 1 }
}

/// `HashRecognize::genSarwateTable`: the table of 256 entries made by
/// interleaving the generating polynomial.
pub fn gen_sarwate_table(
    gen_poly: u128,
    width: u32,
    is_big_endian: bool,
) -> CrcTable {
    let m = mask(width);
    let mut table = [0u128; 256];
    if is_big_endian {
        let mut crc_init = 1u128 << (width - 1);
        let mut i = 1usize;
        while i < 256 {
            let sign = (crc_init >> (width - 1)) & 1 == 1;
            crc_init = ((crc_init << 1) & m) ^ if sign { gen_poly } else { 0 };
            for j in 0..i {
                table[i + j] = crc_init ^ table[j];
            }
            i <<= 1;
        }
        return table;
    }
    let mut crc_init = 1u128;
    let mut i = 128usize;
    while i != 0 {
        crc_init = (crc_init >> 1) ^ if crc_init & 1 == 1 { gen_poly } else { 0 };
        let mut j = 0usize;
        while j < 256 {
            table[i + j] = crc_init ^ table[j];
            j += i << 1;
        }
        i >>= 1;
    }
    table
}

/// The state of `RecurrenceInfo`: `phi` is set by a match even when it fails,
/// and `extra_const` stays across the attempts, as in C++.
#[derive(Clone, Debug, Default)]
struct Recurrence {
    phi: Option<InstId>,
    bo: Option<InstId>,
    start: Option<Operand>,
    step: Option<Operand>,
    extra_const: Option<(u128, u32)>,
}

struct Ctx<'a, 'b> {
    cx: &'a Cx<'b>,
    l: Loop<'a>,
}

impl Ctx<'_, '_> {
    fn function(&self) -> &Function {
        self.cx.function
    }

    fn inst_of(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = operand else { return None };
        match self.function().value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        }
    }

    fn result_of(
        &self,
        inst: InstId,
    ) -> Operand {
        Operand::Value(self.function().instruction(inst).result.expect("a value"))
    }

    fn int_const(
        &self,
        operand: Operand,
    ) -> Option<(u128, u32)> {
        let Operand::Constant(id) = operand else { return None };
        let constant = self.cx.context.get(id);
        let width = self.cx.context.types.int_bits(constant.ty)?;
        match constant.kind {
            ConstantKind::Int(value) => Some((value & mask(width), width)),
            ConstantKind::Zero => Some((0, width)),
            _ => None,
        }
    }

    fn opcode(
        &self,
        inst: InstId,
    ) -> &Opcode {
        &self.function().instruction(inst).opcode
    }

    fn operands(
        &self,
        inst: InstId,
    ) -> &[Operand] {
        &self.function().instruction(inst).operands
    }

    /// `m_ZExtOrTruncOrSelf(m_Specific(phi))` against `operand`.
    fn is_phi_through_cast(
        &self,
        operand: Operand,
        phi: InstId,
    ) -> bool {
        let phi = self.result_of(phi);
        if operand == phi {
            return true;
        }
        let Some(inst) = self.inst_of(operand) else { return false };
        matches!(self.opcode(inst), Opcode::Cast(CastOp::ZExt | CastOp::Trunc)) && self.operands(inst)[0] == phi
    }

    /// `m_c_Xor(m_ZExtOrTruncOrSelf(m_Specific(a)),
    /// m_ZExtOrTruncOrSelf(m_Specific(b)))`.
    fn is_xor_of_phis(
        &self,
        operand: Operand,
        a: InstId,
        b: InstId,
    ) -> bool {
        let Some(inst) = self.inst_of(operand) else { return false };
        self.inst_is_xor_of_phis(inst, a, b)
    }

    fn inst_is_xor_of_phis(
        &self,
        inst: InstId,
        a: InstId,
        b: InstId,
    ) -> bool {
        if *self.opcode(inst) != Opcode::Binary(BinaryOp::Xor) {
            return false;
        }
        let [x, y] = self.operands(inst)[..] else { return false };
        (self.is_phi_through_cast(x, a) && self.is_phi_through_cast(y, b))
            || (self.is_phi_through_cast(y, a) && self.is_phi_through_cast(x, b))
    }

    /// `m_c_Xor(m_Specific(bit_shift), m_SpecificInt(gen_poly))` against
    /// `operand`.
    fn is_bit_shift_xor_gen_poly(
        &self,
        operand: Operand,
        bit_shift: InstId,
        gen_poly: (u128, u32),
    ) -> bool {
        let Some(inst) = self.inst_of(operand) else { return false };
        if *self.opcode(inst) != Opcode::Binary(BinaryOp::Xor) {
            return false;
        }
        let [x, y] = self.operands(inst)[..] else { return false };
        let shift = self.result_of(bit_shift);
        (x == shift && self.int_const(y) == Some(gen_poly)) || (y == shift && self.int_const(x) == Some(gen_poly))
    }

    /// `RecurrenceInfo::matchSimpleRecurrence`.
    fn match_simple_recurrence(
        &self,
        rec: &mut Recurrence,
        phi: InstId,
    ) -> bool {
        let query = Query::bare(self.cx.context, self.cx.function, self.cx.layout);
        if let Some((bo, start, step)) = match_simple_recurrence(&query, phi) {
            rec.bo = Some(bo);
            rec.start = Some(start);
            rec.step = Some(step);
            rec.phi = Some(phi);
            return true;
        }
        false
    }

    /// `RecurrenceInfo::digRecurrence`.
    fn dig_recurrence(
        &self,
        rec: &mut Recurrence,
        v: InstId,
        bo_with_const_op: Option<BinaryOp>,
    ) -> Option<InstId> {
        let phi = self.result_of(rec.phi?);
        let mut worklist = vec![v];
        let mut visited: IdSet<InstId> = IdSet::default();
        while let Some(inst) = worklist.pop() {
            // Skip this instruction if we have already visited it before.
            if !visited.insert(inst) {
                continue;
            }
            // Don't add a PHI's operands to the Worklist.
            if *self.opcode(inst) == Opcode::Phi {
                continue;
            }
            // Find a recurrence over a BinOp, by matching either of its
            // operands with the PHINode.
            if matches!(self.opcode(inst), Opcode::Binary(_)) && self.operands(inst).contains(&phi) {
                return Some(inst);
            }
            // Bind to ExtraConst, if we match exactly one.
            if let (Opcode::Binary(op), Some(want)) = (self.opcode(inst), bo_with_const_op)
                && *op == want
            {
                if rec.extra_const.is_some() {
                    return None;
                }
                let [x, y] = self.operands(inst)[..] else { return None };
                if let Some(c) = self.int_const(x).or_else(|| self.int_const(y)) {
                    rec.extra_const = Some(c);
                }
            }
            // Continue along the use-def chain.
            for &operand in self.operands(inst) {
                if let Some(next) = self.inst_of(operand)
                    && self.l.contains_instruction(self.function(), next)
                {
                    worklist.push(next);
                }
            }
        }
        None
    }

    /// `RecurrenceInfo::matchConditionalRecurrence`.
    fn match_conditional_recurrence(
        &self,
        rec: &mut Recurrence,
        phi: InstId,
        bo_with_const_op: Option<BinaryOp>,
    ) -> bool {
        rec.phi = Some(phi);
        let function = self.function();
        let operands = self.operands(phi);
        if operands.len() != 4 {
            return false;
        }
        // Step comes from the loop latch, start comes from the other incoming
        // value.
        let Some(latch) = self.l.get_loop_latch(function) else { return false };
        let Some(latch_at) = operands.chunks(2).position(|pair| pair[1] == Operand::Block(latch)) else {
            return false;
        };
        let found_step = operands[2 * latch_at];
        let found_start = operands[2 * (1 - latch_at)];

        let Some(select) = self.inst_of(found_step) else { return false };
        if *self.opcode(select) != Opcode::Select {
            return false;
        }
        let [cond, tv, fv] = self.operands(select)[..] else { return false };
        let is_cmp_or_trunc = self
            .inst_of(cond)
            .is_some_and(
                |inst| matches!(
                    self.opcode(inst),
                    Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::Cast(CastOp::Trunc)
                ),
            );
        let (Some(tv), Some(fv)) = (self.inst_of(tv), self.inst_of(fv)) else { return false };
        if !is_cmp_or_trunc {
            return false;
        }
        // For a conditional recurrence, both the true and false values of the
        // select must ultimately end up in the same recurrent BinOp.
        let found_bo = self.dig_recurrence(rec, tv, bo_with_const_op);
        let alt_bo = self.dig_recurrence(rec, fv, bo_with_const_op);
        let (Some(found_bo), Some(alt_bo)) = (found_bo, alt_bo) else { return false };
        if found_bo != alt_bo {
            return false;
        }
        if bo_with_const_op.is_some() && rec.extra_const.is_none() {
            return false;
        }
        rec.bo = Some(found_bo);
        rec.start = Some(found_start);
        rec.step = Some(found_step);
        true
    }

    /// `getRecurrences`: the simple and the conditional recurrence of the
    /// latch's phis.
    fn get_recurrences(
        &self,
        latch: BlockId,
        ind_var: InstId,
    ) -> Option<(Recurrence, Recurrence)> {
        let function = self.function();
        let phis: Vec<InstId> =
            function.iter_block(latch).take_while(|&inst| *self.opcode(inst) == Opcode::Phi).collect();
        if phis.len() != 2 && phis.len() != 3 {
            return None;
        }
        let mut simple = Recurrence::default();
        let mut conditional = Recurrence::default();
        for phi in phis.iter().copied() {
            if phi == ind_var {
                continue;
            }
            if simple.bo.is_none() {
                self.match_simple_recurrence(&mut simple, phi);
            }
            if conditional.bo.is_none() {
                self.match_conditional_recurrence(&mut conditional, phi, Some(BinaryOp::Xor));
            }
        }
        if phis.len() == 3 && (simple.bo.is_none() || conditional.bo.is_none()) {
            return None;
        }
        Some((simple, conditional))
    }

    /// `containsUnreachable`.
    fn contains_unreachable(
        &self,
        roots: &[InstId],
    ) -> bool {
        let function = self.function();
        let Some(latch) = self.l.get_loop_latch(function) else { return true };
        let mut visited: IdSet<InstId> = IdSet::default();
        let mut worklist: Vec<InstId> = roots.to_vec();
        while let Some(inst) = worklist.pop() {
            if !visited.insert(inst) {
                continue;
            }
            if *self.opcode(inst) == Opcode::Phi {
                continue;
            }
            for &operand in self.operands(inst) {
                if let Some(next) = self.inst_of(operand) {
                    if !self.l.contains_instruction(function, next) {
                        return true;
                    }
                    worklist.push(next);
                }
            }
        }
        function.iter_block(latch).count() != visited.len()
    }

    /// `isConditionalOnXorOfPHIs`.
    fn is_conditional_on_xor_of_phis(
        &self,
        select: InstId,
        p1: InstId,
        p2: InstId,
    ) -> bool {
        let Some(cond) = self.inst_of(self.operands(select)[0]) else { return false };
        let mut worklist = vec![cond];
        let mut visited: IdSet<InstId> = IdSet::default();
        while let Some(inst) = worklist.pop() {
            if !visited.insert(inst) {
                continue;
            }
            if *self.opcode(inst) == Opcode::Phi {
                continue;
            }
            // If we match an XOR of the two PHIs ignoring casts, we're done.
            if self.inst_is_xor_of_phis(inst, p1, p2) {
                return true;
            }
            for &operand in self.operands(inst) {
                if let Some(next) = self.inst_of(operand)
                    && self.l.contains_instruction(self.function(), next)
                {
                    worklist.push(next);
                }
            }
        }
        false
    }

    /// `isSignificantBitCheckWellFormed`.
    fn is_significant_bit_check_well_formed(
        &self,
        conditional: &Recurrence,
        simple: &Recurrence,
        is_big_endian: bool,
    ) -> bool {
        let (Some(step), Some(cphi), Some(bit_shift), Some(gen_poly)) =
            (conditional.step, conditional.phi, conditional.bo, conditional.extra_const)
        else {
            return false;
        };
        let Some(select) = self.inst_of(step) else { return false };
        let [cond, tv, fv] = self.operands(select)[..] else { return false };

        // Match predicate with or without a SimpleRecurrence (the
        // corresponding data is LHSAux).
        let match_pred = |operand: Operand| {
            operand == self.result_of(cphi) || simple.phi.is_some_and(|sphi| self.is_xor_of_phis(operand, cphi, sphi))
        };

        let shift = self.result_of(bit_shift);
        if !is_big_endian
            && let Some(cond_inst) = self.inst_of(cond)
            && *self.opcode(cond_inst) == Opcode::Cast(CastOp::Trunc)
            && match_pred(self.operands(cond_inst)[0])
            && self.is_bit_shift_xor_gen_poly(tv, bit_shift, gen_poly)
            && fv == shift
        {
            return true;
        }

        let Some(cmp) = self.inst_of(cond) else { return false };
        let Opcode::ICmp(pred) = *self.opcode(cmp) else { return false };
        let [l, r] = self.operands(cmp)[..] else { return false };
        let Some((r, bw)) = self.int_const(r) else { return false };
        let (Some(tv_inst), Some(fv_inst)) = (self.inst_of(tv), self.inst_of(fv)) else { return false };

        let l_well_formed = if is_big_endian {
            match_pred(l)
        } else {
            // m_c_And(MatchPred, m_One())
            self.inst_of(l).is_some_and(|and| {
                *self.opcode(and) == Opcode::Binary(BinaryOp::And)
                    && matches!(
                        self.operands(and)[..],
                        [x, y] if (match_pred(x) && self.int_const(y) == Some((1, bw)))
                            || (match_pred(y) && self.int_const(x) == Some((1, bw)))
                    )
            })
        };
        if !l_well_formed {
            return false;
        }

        let rcr = Range::<u128>::single(bw, r);
        let allowed_by_r = Range::<u128>::make_allowed_icmp_region(pred, &rcr);
        let check_allowed_by_r = Range::<u128>::new(bw, 0, if is_big_endian { 1u128 << (bw - 1) } else { 1 });

        if allowed_by_r == check_allowed_by_r {
            return tv_inst == bit_shift && self.is_bit_shift_xor_gen_poly(fv, bit_shift, gen_poly);
        }
        if allowed_by_r.inverse() == check_allowed_by_r {
            return fv_inst == bit_shift && self.is_bit_shift_xor_gen_poly(tv, bit_shift, gen_poly);
        }
        false
    }
}

/// `isBigEndianBitShift`: a multiplication or division by the constant two,
/// by SCEV, so that `mul`/`udiv` and `shl`/`lshr` are alike. `Some(false)`
/// for a division, `Some(true)` for a multiplication.
fn is_big_endian_bit_shift(
    cx: &Cx,
    se: &mut ScalarEvolution,
    value: Operand,
) -> Option<bool> {
    let ty = cx.function.operand_type(cx.context, value)?;
    cx.context.types.int_bits(ty)?;
    let e = se.get_scev(cx, value);
    let two = |se: &ScalarEvolution, s| se.as_constant(s).is_some_and(|c| c.zext_value() == 2);
    if let Some((_, divisor)) = se.match_binary(e, ScevType::UDiv, 0)
        && two(se, divisor)
    {
        return Some(false);
    }
    if let Some((factor, _)) = se.match_binary(e, ScevType::Mul, 0)
        && two(se, factor)
    {
        return Some(true);
    }
    None
}

/// `HashRecognize::recognizeCRC`: the polynomial of `l`'s CRC, or why not.
pub fn recognize_crc(
    cx: &Cx,
    se: &mut ScalarEvolution,
    l: Loop,
) -> Result<PolynomialInfo, &'static str> {
    let c = Ctx { cx, l };
    let function = cx.function;
    if !l.is_innermost() {
        return Err("Loop is not innermost");
    }
    let latch = l.get_loop_latch(function);
    let exit = l.get_exit_block(function);
    let ind_var = l.get_canonical_induction_variable(cx.context, function);
    let latch_cmp = l.get_latch_cmp_inst(function);
    let (Some(latch), Some(exit), Some(ind_var), Some(latch_cmp)) = (latch, exit, ind_var, latch_cmp) else {
        return Err("Loop not in canonical form");
    };
    if l.get_num_blocks() != 1 {
        return Err("Loop not in canonical form");
    }
    let tc = se.get_small_constant_trip_count(cx, l.get_header());
    if tc == 0 {
        return Err("Unable to find a small constant trip count");
    }

    let Some((simple, conditional)) = c.get_recurrences(latch, ind_var) else { return Err("Found stray PHI") };
    let Some(conditional_bo) = conditional.bo else { return Err("Unable to find conditional recurrence") };

    // Make sure that all recurrences are either all SCEVMul with two or
    // SCEVDiv with two, or in other words, that they're single bit-shifts.
    let Some(is_big_endian) = is_big_endian_bit_shift(cx, se, c.result_of(conditional_bo)) else {
        return Err("Loop with non-unit bitshifts");
    };
    if let Some(simple_bo) = simple.bo {
        if is_big_endian_bit_shift(cx, se, c.result_of(simple_bo)) != Some(is_big_endian) {
            return Err("Loop with non-unit bitshifts");
        }
        // Ensure that the PHIs have exactly two uses: the bit-shift, and the
        // XOR (or a cast feeding into the XOR). Also ensure that the
        // SimpleRecurrence's evolution doesn't have stray users.
        let uses = |inst: InstId| function.users(function.instruction(inst).result.expect("a phi")).into_iter().count();
        let (Some(cphi), Some(sphi)) = (conditional.phi, simple.phi) else { return Err("Found stray PHI") };
        let unique_user = {
            let mut users = function.users(function.instruction(simple_bo).result.expect("a value")).into_iter();
            match (users.next(), users.next()) {
                (Some(only), None) => Some(only.user),
                _ => None,
            }
        };
        if uses(cphi) != 2 || uses(sphi) != 2 || unique_user != Some(sphi) {
            return Err("Recurrences have stray uses");
        }
        // Check that the SelectInst ConditionalRecurrence.Step is conditional
        // on the XOR of SimpleRecurrence.Phi and ConditionalRecurrence.Phi.
        let select = c.inst_of(conditional.step.expect("a step")).expect("a select");
        if !c.is_conditional_on_xor_of_phis(select, sphi, cphi) {
            return Err("Recurrences not intertwined with XOR");
        }
    }

    let lhs = conditional.start.expect("a start");
    let mut lhs_aux = simple.bo.and(simple.start);

    let width_of = |operand: Operand| {
        function.operand_type(cx.context, operand).and_then(|ty| cx.context.types.int_bits(ty)).unwrap_or(0)
    };
    // In the big-endian case where LHSAux is narrower than LHS, the most
    // significant bit check will never be influenced by LHSAux.
    if is_big_endian
        && let Some(aux) = lhs_aux
        && width_of(aux) < width_of(lhs)
    {
        lhs_aux = None;
    }

    // Make sure that the TC doesn't exceed the bitwidth of LHSAux, or LHS.
    if tc > lhs_aux.map_or_else(|| width_of(lhs), width_of) {
        return Err("Loop iterations exceed bitwidth of data");
    }

    // Ensure nothing other than the computed value makes its way out of the
    // loop. Since the loop is in LCSSA form, this is as simple as checking the
    // PHI nodes in the exit block.
    let computed_value = c.inst_of(conditional.step.expect("a step")).expect("a select");
    let computed = c.result_of(computed_value);
    let stray = function.iter_block(exit).take_while(|&inst| *c.opcode(inst) == Opcode::Phi).any(|phi| {
        c.operands(phi).chunks(2).find(|pair| pair[1] == Operand::Block(latch)).is_none_or(|pair| pair[0] != computed)
    });
    if stray {
        return Err("Found stray incoming values in loop exit block");
    }

    let gen_poly = conditional.extra_const.expect("a polynomial");
    if !c.is_significant_bit_check_well_formed(&conditional, &simple, is_big_endian) {
        return Err("Malformed significant-bit check");
    }

    let ind_next = {
        let incoming = c.operands(ind_var).chunks(2).find(|pair| pair[1] == Operand::Block(latch)).map(|pair| pair[0]);
        c.inst_of(incoming.expect("a backedge value")).expect("the step")
    };
    let mut roots = vec![computed_value, ind_next, latch_cmp, function.terminator(latch).expect("a terminator")];
    if let Some(simple_bo) = simple.bo {
        roots.push(simple_bo);
    }
    if c.contains_unreachable(&roots) {
        return Err("Found stray unvisited instructions");
    }

    Ok(PolynomialInfo {
        trip_count: tc,
        lhs,
        rhs: gen_poly.0,
        rhs_width: gen_poly.1,
        computed_value,
        is_big_endian,
        lhs_aux,
    })
}

#[cfg(test)]
#[path = "hashrecognize_tests.rs"]
mod tests;
