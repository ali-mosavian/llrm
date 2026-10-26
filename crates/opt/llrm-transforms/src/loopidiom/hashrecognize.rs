//! `HashRecognize`: a translation of LLVM's `HashRecognize.cpp` at
//! `d1106deb71cc`, the CRC half (`recognizeCRC`, `genSarwateTable`).
//!
//! # How it works
//! [`recognize_crc`] takes the single block of an innermost counted loop and
//! answers the [`PolynomialInfo`] of the bitwise CRC it computes, or the
//! reason it is not one. The pieces follow the C++: the conditional and the
//! simple recurrence of the latch's phis, the significant-bit check, and the
//! walk that finds an instruction outside the CRC's use-def chains.
//!
//! # Deviations
//! - D13: `isBigEndianBitShift` reads the instruction (`shl x, 1`, `mul x, 2`,
//!   `add x, x`; `lshr x, 1`, `udiv x, 2`), where the pin asks ScalarEvolution
//!   for `udiv x, 2` or `mul 2, x`. The forms differ only in an expression SCEV
//!   folds to those two without an instruction of them.
//! - Widths above 128 bits do not exist in MIR.
//!
//! # Example
//! `c = (c & 1) ? 0xEDB88320 ^ (c >> 1) : c >> 1` for 8 trips is the
//! little-endian CRC-32 with the polynomial `0xEDB88320`.
//!
//! # See also
//! [`HashRecognize.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/HashRecognize.cpp)

use std::collections::HashSet;

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::knownbits::KnownBits;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};

/// `PolynomialInfo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolynomialInfo {
    pub trip_count: u32,
    /// The initial value of the CRC.
    pub lhs: Operand,
    /// The generating polynomial, in `width` bits.
    pub rhs: u128,
    pub width: u32,
    /// The select whose value leaves the loop.
    pub computed_value: InstId,
    pub is_big_endian: bool,
    pub lhs_aux: Option<Operand>,
}

fn mask(width: u32) -> u128 {
    if width >= 128 { u128::MAX } else { (1u128 << width) - 1 }
}

/// `genSarwateTable`: the 256 entries of the byte-at-a-time CRC.
pub fn gen_sarwate_table(
    gen_poly: u128,
    width: u32,
    is_big_endian: bool,
) -> [u128; 256] {
    let m = mask(width);
    let mut table = [0u128; 256];
    if is_big_endian {
        let sign = 1u128 << (width - 1);
        let mut init = sign;
        let mut i = 1usize;
        while i < 256 {
            init = ((init << 1) & m) ^ if init & sign != 0 { gen_poly } else { 0 };
            for j in 0..i {
                table[i + j] = init ^ table[j];
            }
            i <<= 1;
        }
        return table;
    }
    let mut init = 1u128;
    let mut i = 128usize;
    while i != 0 {
        init = (init >> 1) ^ if init & 1 != 0 { gen_poly } else { 0 };
        let mut j = 0;
        while j < 256 {
            table[i + j] = init ^ table[j];
            j += i << 1;
        }
        i >>= 1;
    }
    table
}

/// The loop's one block and the function it is in.
struct Block<'a> {
    context: &'a Context,
    function: &'a Function,
    block: BlockId,
}

/// `RecurrenceInfo`. `phi` and `extra_const` are set by a failed attempt as
/// the C++ sets them, and the next phi's attempt starts from them.
#[derive(Clone, Copy, Debug, Default)]
struct Recurrence {
    phi: Option<InstId>,
    bo: Option<InstId>,
    start: Option<Operand>,
    step: Option<Operand>,
    extra_const: Option<u128>,
}

impl Block<'_> {
    fn instruction_of(
        &self,
        operand: Operand,
    ) -> Option<(InstId, &Instruction)> {
        let Operand::Value(value) = operand else { return None };
        let ValueDef::Instruction(inst) = self.function.value(value).def else { return None };
        Some((inst, self.function.instruction(inst)))
    }

    /// `L.contains(I)`.
    fn contains(
        &self,
        inst: InstId,
    ) -> bool {
        self.function.parent(inst) == Some(self.block)
    }

    fn constant(
        &self,
        operand: Operand,
    ) -> Option<u128> {
        let Operand::Constant(id) = operand else { return None };
        match self.context.get(id).kind {
            ConstantKind::Int(value) => Some(value),
            ConstantKind::Zero => Some(0),
            _ => None,
        }
    }

    fn value_of(
        &self,
        inst: InstId,
    ) -> Operand {
        Operand::Value(self.function.instruction(inst).result.expect("a value"))
    }

    fn bits_of(
        &self,
        operand: Operand,
    ) -> Option<u32> {
        let ty = self.function.operand_type(self.context, operand)?;
        self.context.types.int_bits(ty)
    }

    fn phis(&self) -> Vec<InstId> {
        self.function
            .iter_block(self.block)
            .take_while(|&one| self.function.instruction(one).opcode == Opcode::Phi)
            .collect()
    }

    /// The incoming value of `phi` from the block itself, and the other.
    fn incoming(
        &self,
        phi: InstId,
        latch: bool,
    ) -> Option<Operand> {
        let operands = &self.function.instruction(phi).operands;
        (0..operands.len() / 2)
            .find(|&at| (operands[at * 2 + 1] == Operand::Block(self.block)) == latch)
            .map(|at| operands[at * 2])
    }

    /// `matchSimpleRecurrence` (ValueTracking).
    fn match_simple_recurrence(
        &self,
        phi: InstId,
    ) -> Option<(InstId, Operand, Operand)> {
        let value = self.value_of(phi);
        let operands = &self.function.instruction(phi).operands;
        if operands.len() != 4 {
            return None;
        }
        (0..2).find_map(|at| {
            let (bo, instruction) = self.instruction_of(operands[at * 2])?;
            if !matches!(instruction.opcode, Opcode::Binary(_)) {
                return None;
            }
            let step = if instruction.operands[0] == value {
                instruction.operands[1]
            } else if instruction.operands[1] == value {
                instruction.operands[0]
            } else {
                return None;
            };
            Some((bo, operands[(1 - at) * 2], step))
        })
    }

    /// `RecurrenceInfo::digRecurrence`.
    fn dig_recurrence(
        &self,
        recurrence: &mut Recurrence,
        start: InstId,
    ) -> Option<InstId> {
        let phi = self.value_of(recurrence.phi?);
        let mut worklist = vec![start];
        let mut visited = HashSet::new();
        while let Some(inst) = worklist.pop() {
            if !visited.insert(inst) {
                continue;
            }
            let instruction = self.function.instruction(inst);
            if instruction.opcode == Opcode::Phi {
                continue;
            }
            if matches!(instruction.opcode, Opcode::Binary(_)) && instruction.operands.contains(&phi) {
                return Some(inst);
            }
            if instruction.opcode == Opcode::Binary(BinaryOp::Xor) {
                if recurrence.extra_const.is_some() {
                    return None;
                }
                if let Some(value) = instruction.operands.iter().find_map(|&one| self.constant(one)) {
                    recurrence.extra_const = Some(value);
                }
            }
            for &operand in &instruction.operands {
                if let Some((one, _)) = self.instruction_of(operand)
                    && self.contains(one)
                {
                    worklist.push(one);
                }
            }
        }
        None
    }

    /// `RecurrenceInfo::matchConditionalRecurrence` with `Xor` to match.
    fn match_conditional_recurrence(
        &self,
        recurrence: &mut Recurrence,
        phi: InstId,
    ) -> bool {
        recurrence.phi = Some(phi);
        if self.function.instruction(phi).operands.len() != 4 {
            return false;
        }
        let (Some(step), Some(start)) = (self.incoming(phi, true), self.incoming(phi, false)) else { return false };
        let Some((select, selected)) = self.instruction_of(step) else { return false };
        if selected.opcode != Opcode::Select {
            return false;
        }
        let condition_is_cmp_or_trunc = self
            .instruction_of(selected.operands[0])
            .is_some_and(
                |(_, one)| matches!(
                    one.opcode,
                    Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::Cast(CastOp::Trunc)
                ),
            );
        let (Some((tv, _)), Some((fv, _))) =
            (self.instruction_of(selected.operands[1]), self.instruction_of(selected.operands[2]))
        else {
            return false;
        };
        if !condition_is_cmp_or_trunc {
            return false;
        }
        let found = self.dig_recurrence(recurrence, tv);
        let alt = self.dig_recurrence(recurrence, fv);
        let (Some(found), true) = (found, found == alt) else { return false };
        if recurrence.extra_const.is_none() {
            return false;
        }
        recurrence.bo = Some(found);
        recurrence.start = Some(start);
        recurrence.step = Some(self.value_of(select));
        true
    }

    /// `isBigEndianBitShift`: `Some(false)` for a halving, `Some(true)` for a
    /// doubling.
    fn is_big_endian_bit_shift(
        &self,
        bo: InstId,
    ) -> Option<bool> {
        let instruction = self.function.instruction(bo);
        let Opcode::Binary(op) = instruction.opcode else { return None };
        let (a, b) = (instruction.operands[0], instruction.operands[1]);
        match op {
            BinaryOp::LShr if self.constant(b) == Some(1) => Some(false),
            BinaryOp::UDiv if self.constant(b) == Some(2) => Some(false),
            BinaryOp::Shl if self.constant(b) == Some(1) => Some(true),
            BinaryOp::Mul if self.constant(a) == Some(2) || self.constant(b) == Some(2) => Some(true),
            BinaryOp::Add if a == b => Some(true),
            _ => None,
        }
    }

    /// `m_ZExtOrTruncOrSelf(m_Specific(want))`.
    fn is_cast_of(
        &self,
        operand: Operand,
        want: Operand,
    ) -> bool {
        operand == want
            || self.instruction_of(operand).is_some_and(|(_, one)| {
                matches!(one.opcode, Opcode::Cast(CastOp::ZExt | CastOp::Trunc)) && one.operands[0] == want
            })
    }

    /// `m_c_Xor(m_ZExtOrTruncOrSelf(m_Specific(p1)),
    /// m_ZExtOrTruncOrSelf(m_Specific(p2)))`.
    fn is_xor_of(
        &self,
        operand: Operand,
        p1: Operand,
        p2: Operand,
    ) -> bool {
        let Some((_, one)) = self.instruction_of(operand) else { return false };
        one.opcode == Opcode::Binary(BinaryOp::Xor)
            && ((self.is_cast_of(one.operands[0], p1) && self.is_cast_of(one.operands[1], p2))
                || (self.is_cast_of(one.operands[1], p1) && self.is_cast_of(one.operands[0], p2)))
    }

    /// `isConditionalOnXorOfPHIs`.
    fn is_conditional_on_xor_of_phis(
        &self,
        select: InstId,
        p1: InstId,
        p2: InstId,
    ) -> bool {
        let (p1, p2) = (self.value_of(p1), self.value_of(p2));
        let Some((first, _)) = self.instruction_of(self.function.instruction(select).operands[0]) else {
            return false;
        };
        let mut worklist = vec![first];
        let mut visited = HashSet::new();
        while let Some(inst) = worklist.pop() {
            if !visited.insert(inst) {
                continue;
            }
            let instruction = self.function.instruction(inst);
            if instruction.opcode == Opcode::Phi {
                continue;
            }
            if self.is_xor_of(self.value_of(inst), p1, p2) {
                return true;
            }
            for &operand in &instruction.operands {
                if let Some((one, _)) = self.instruction_of(operand)
                    && self.contains(one)
                {
                    worklist.push(one);
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
        let select =
            self.function.instruction(self.instruction_of(conditional.step.expect("a step")).expect("a select").0);
        let cond_phi = self.value_of(conditional.phi.expect("a phi"));
        let simple_phi = simple.phi.filter(|_| simple.bo.is_some()).map(|one| self.value_of(one));
        let bit_shift = self.value_of(conditional.bo.expect("a shift"));
        let extra = conditional.extra_const.expect("a polynomial");
        let width = self.bits_of(cond_phi).expect("an integer");

        // MatchPred: the conditional phi, or its xor with the simple phi.
        let match_pred = |operand: Operand| {
            operand == cond_phi || simple_phi.is_some_and(|simple_phi| self.is_xor_of(operand, cond_phi, simple_phi))
        };
        // MatchBitShiftXorGenPoly: a commutative xor of the shift and the
        // polynomial.
        let match_xor_poly = |operand: Operand| {
            self.instruction_of(operand).is_some_and(|(_, one)| {
                one.opcode == Opcode::Binary(BinaryOp::Xor)
                    && ((one.operands[0] == bit_shift && self.constant(one.operands[1]) == Some(extra))
                        || (one.operands[1] == bit_shift && self.constant(one.operands[0]) == Some(extra)))
            })
        };
        let (condition, true_value, false_value) = (select.operands[0], select.operands[1], select.operands[2]);
        if !is_big_endian
            && self
                .instruction_of(condition)
                .is_some_and(|(_, one)| one.opcode == Opcode::Cast(CastOp::Trunc) && match_pred(one.operands[0]))
            && match_xor_poly(true_value)
            && false_value == bit_shift
        {
            return true;
        }

        let Some((_, compare)) = self.instruction_of(condition) else { return false };
        let Opcode::ICmp(pred) = compare.opcode else { return false };
        let Some(r) = self.constant(compare.operands[1]) else { return false };
        // The true and the false values are instructions.
        if self.instruction_of(true_value).is_none() || self.instruction_of(false_value).is_none() {
            return false;
        }
        let left = compare.operands[0];
        let well_formed = if is_big_endian {
            match_pred(left)
        } else {
            self.instruction_of(left).is_some_and(|(_, one)| {
                one.opcode == Opcode::Binary(BinaryOp::And)
                    && ((match_pred(one.operands[0]) && self.constant(one.operands[1]) == Some(1))
                        || (match_pred(one.operands[1]) && self.constant(one.operands[0]) == Some(1)))
            })
        };
        if !well_formed {
            return false;
        }
        let known = KnownBits::constant(width, r);
        let allowed = ConstantRange::make_allowed_icmp_region(pred, &ConstantRange::from_known_bits(&known, false));
        let sign_min = 1u128 << (width - 1);
        let check = ConstantRange::new(width, 0, if is_big_endian { sign_min } else { 1 });
        if allowed == check {
            return true_value == bit_shift && match_xor_poly(false_value);
        }
        if allowed.inverse() == check {
            return false_value == bit_shift && match_xor_poly(true_value);
        }
        false
    }

    /// `containsUnreachable`.
    fn contains_unreachable(
        &self,
        roots: &[InstId],
    ) -> bool {
        let mut visited = HashSet::new();
        let mut worklist = roots.to_vec();
        while let Some(inst) = worklist.pop() {
            if !visited.insert(inst) {
                continue;
            }
            let instruction = self.function.instruction(inst);
            if instruction.opcode == Opcode::Phi {
                continue;
            }
            for &operand in &instruction.operands {
                if let Some((one, _)) = self.instruction_of(operand) {
                    if !self.contains(one) {
                        return true;
                    }
                    worklist.push(one);
                }
            }
        }
        self.function.iter_block(self.block).count() != visited.len()
    }

    fn uses(
        &self,
        inst: InstId,
    ) -> Vec<InstId> {
        let value = self.function.instruction(inst).result.expect("a value");
        self.function.users(value).iter().map(|one| one.user).collect()
    }
}

/// `HashRecognize::recognizeCRC` for the innermost loop that is the one block
/// `block`, which branches to itself. `exit` is its one exit block,
/// `indvar` its canonical induction variable, `latch_cmp` the compare of its
/// latch's branch and `trip_count` `getSmallConstantTripCount`.
pub fn recognize_crc(
    context: &Context,
    function: &Function,
    block: BlockId,
    exit: BlockId,
    indvar: InstId,
    latch_cmp: InstId,
    trip_count: u32,
) -> Result<PolynomialInfo, &'static str> {
    let l = Block { context, function, block };
    if trip_count == 0 {
        return Err("Unable to find a small constant trip count");
    }

    // getRecurrences.
    let phis = l.phis();
    if phis.len() != 2 && phis.len() != 3 {
        return Err("Found stray PHI");
    }
    let (mut simple, mut conditional) = (Recurrence::default(), Recurrence::default());
    for &phi in &phis {
        if phi == indvar {
            continue;
        }
        if simple.bo.is_none()
            && let Some((bo, start, step)) = l.match_simple_recurrence(phi)
        {
            simple =
                Recurrence { phi: Some(phi), bo: Some(bo), start: Some(start), step: Some(step), extra_const: None };
        }
        if conditional.bo.is_none() {
            l.match_conditional_recurrence(&mut conditional, phi);
        }
    }
    if phis.len() == 3 && (simple.bo.is_none() || conditional.bo.is_none()) {
        return Err("Found stray PHI");
    }
    let Some(conditional_bo) = conditional.bo else { return Err("Unable to find conditional recurrence") };

    let Some(is_big_endian) = l.is_big_endian_bit_shift(conditional_bo) else {
        return Err("Loop with non-unit bitshifts");
    };
    if let Some(simple_bo) = simple.bo {
        if l.is_big_endian_bit_shift(simple_bo) != Some(is_big_endian) {
            return Err("Loop with non-unit bitshifts");
        }
        let simple_phi = simple.phi.expect("a phi");
        let conditional_phi = conditional.phi.expect("a phi");
        if l.uses(conditional_phi).len() != 2 || l.uses(simple_phi).len() != 2 || l.uses(simple_bo) != [simple_phi] {
            return Err("Recurrences have stray uses");
        }
        let select = l.instruction_of(conditional.step.expect("a step")).expect("a select").0;
        if !l.is_conditional_on_xor_of_phis(select, simple_phi, conditional_phi) {
            return Err("Recurrences not intertwined with XOR");
        }
    }

    let lhs = conditional.start.expect("a start");
    let mut lhs_aux = simple.bo.and(simple.start);
    let width_of = |operand: Operand| l.bits_of(operand).expect("an integer");
    if is_big_endian && lhs_aux.is_some_and(|aux| width_of(aux) < width_of(lhs)) {
        lhs_aux = None;
    }
    if trip_count > lhs_aux.map_or_else(|| width_of(lhs), width_of) {
        return Err("Loop iterations exceed bitwidth of data");
    }

    let computed = l.instruction_of(conditional.step.expect("a step")).expect("a select").0;
    let computed_value = l.value_of(computed);
    let stray_exit_phi =
        function.iter_block(exit).take_while(|&one| function.instruction(one).opcode == Opcode::Phi).any(|phi| {
            let operands = &function.instruction(phi).operands;
            (0..operands.len() / 2)
                .any(|at| operands[at * 2 + 1] == Operand::Block(block) && operands[at * 2] != computed_value)
        });
    if stray_exit_phi {
        return Err("Found stray incoming values in loop exit block");
    }

    let gen_poly = conditional.extra_const.expect("a polynomial");
    if !l.is_significant_bit_check_well_formed(&conditional, &simple, is_big_endian) {
        return Err("Malformed significant-bit check");
    }

    let indvar_next = l.instruction_of(l.incoming(indvar, true).expect("a latch value")).expect("an instruction").0;
    let terminator = function.terminator(block).expect("a terminator");
    let mut roots = vec![computed, indvar_next, latch_cmp, terminator];
    roots.extend(simple.bo);
    if l.contains_unreachable(&roots) {
        return Err("Found stray unvisited instructions");
    }

    let width = width_of(lhs);
    Ok(PolynomialInfo {
        trip_count,
        lhs,
        rhs: gen_poly & mask(width),
        width,
        computed_value: computed,
        is_big_endian,
        lhs_aux,
    })
}
