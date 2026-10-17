//! A translation of LLVM's `ConstraintElimination`
//! (`llvm/lib/Transforms/Scalar/ConstraintElimination.cpp` at d1106deb71cc),
//! for the branches of one function: the conditions that hold on entry to a
//! block are facts of a system of linear constraints
//! (`constraints::ConstraintSystem`), pushed and popped in dominator-tree
//! order, and each compare feeding a conditional branch is asked of it.
//!
//! The entry point is [`decisions`]: the branches the system decides, with the
//! way they go. A spike: nothing calls it but the shadow in `decide`.
//!
//! DEVIATE (not translated; each a fact or a check LLVM has and this does not):
//! - `decomposeGEP`, `collectOffsets`, and every pointer compare (no GEP
//!   decomposition);
//! - `addBoundsForHeaderInductions`, `addInfoForInductions`,
//!   `getMonotonicityInfo` (the facts of loop inductions, which need
//!   `ScalarEvolution`);
//! - `getConstraintFromMemoryAccess` (facts from loads and stores through a
//!   `getelementptr`), the facts of `assume` and of the min, max, abs,
//!   saturating, `urem`, `udiv`, `lshr`, `srem`, `sdiv` instructions;
//! - the checks of overflow intrinsics, flags strengthening
//!   (`tryToStrengthenFlags`), `checkOrAndOpImpliedByOther`, `samesign`;
//! - `::isKnownNonNegative` (ValueTracking) is a small stand-in: constants, a
//!   zero extension, a mask by a non-negative constant, a division, a remainder
//!   by anything and a logical right shift by a constant above zero;
//! - `ConstantRange::makeExactNoWrapRegion` for `mul` and `shl` (the add and
//!   sub regions are translated).

use std::collections::HashMap;

use llrm_analysis::constraints::{ConstraintSystem, Entry, Row};
use llrm_analysis::manager::Held;
use llrm_analysis::{cfg, memory::Unit};
use llrm_mir::footprint::Footprint;
use llrm_mir::module::{BlockId, InstId, Instruction, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};

/// `MaxConstraintValue`, `MinSignedConstraintValue`.
const MAX_CONSTRAINT_VALUE: i64 = i64::MAX;
const MIN_SIGNED_CONSTRAINT_VALUE: i64 = i64::MIN;
/// `-constraint-elimination-max-rows`.
const MAX_ROWS: usize = 500;

/// A `Value *`: a local value or constant, or an integer the pass makes up
/// (`ConstantInt::get(Ty, 0)` and the like) of a width.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Val {
    Op(Operand),
    Int(i128, u32),
}

/// An integer of `width` bits.
#[derive(Clone, Copy)]
struct Int {
    bits: u128,
    width: u32,
}

impl Int {
    fn mask(width: u32) -> u128 {
        if width >= 128 { u128::MAX } else { (1u128 << width) - 1 }
    }

    /// `getZExtValue`.
    fn zext(self) -> u128 {
        self.bits & Self::mask(self.width)
    }

    /// `getSExtValue`.
    fn sext(self) -> i128 {
        let bits = self.zext();
        if self.width >= 128 || bits & (1u128 << (self.width - 1)) == 0 {
            bits as i128
        } else {
            (bits | !Self::mask(self.width)) as i128
        }
    }

    fn is_negative(self) -> bool {
        self.sext() < 0
    }
}

/// `DecompEntry`/`Decomposition`: `Offset + Coefficient1 * Variable1 + ...`.
#[derive(Clone, Debug, Default)]
struct Decomposition {
    offset: i64,
    vars: Vec<(i64, Operand)>,
}

impl Decomposition {
    fn constant(offset: i64) -> Self {
        Self { offset, vars: Vec::new() }
    }

    fn variable(variable: Operand) -> Self {
        Self { offset: 0, vars: vec![(1, variable)] }
    }

    /// `add(int64_t)`: true if the operation overflows.
    fn add_offset(
        &mut self,
        other: i64,
    ) -> bool {
        match self.offset.checked_add(other) {
            Some(sum) => {
                self.offset = sum;
                false
            }
            None => true,
        }
    }

    /// `add(const Decomposition &)`.
    fn add(
        &mut self,
        other: &Decomposition,
    ) -> bool {
        if self.add_offset(other.offset) {
            return true;
        }
        self.vars.extend(other.vars.iter().copied());
        false
    }

    /// `sub`.
    fn sub(
        &mut self,
        other: &Decomposition,
    ) -> bool {
        let mut tmp = other.clone();
        if tmp.mul(-1) {
            return true;
        }
        if self.add_offset(tmp.offset) {
            return true;
        }
        self.vars.extend(tmp.vars);
        false
    }

    /// `mul`.
    fn mul(
        &mut self,
        factor: i64,
    ) -> bool {
        match self.offset.checked_mul(factor) {
            Some(product) => self.offset = product,
            None => return true,
        }
        for (coefficient, _) in &mut self.vars {
            match coefficient.checked_mul(factor) {
                Some(product) => *coefficient = product,
                None => return true,
            }
        }
        false
    }
}

/// `ConstraintTy`.
#[derive(Clone, Debug, Default)]
struct ConstraintTy {
    coefficients: Row,
    num_vars: usize,
    signed: bool,
    eq: bool,
    ne: bool,
}

impl ConstraintTy {
    fn empty(&self) -> bool {
        self.coefficients.is_empty()
    }

    /// `ConstraintTy::isImpliedBy`: Some(true) or Some(false) where the
    /// constraint is proven to be respectively true or false.
    fn implied_by(
        &self,
        system: &ConstraintSystem,
    ) -> Option<bool> {
        let (sub, new_coefficients) = system.sub_system(&self.coefficients);
        let implied = sub.is_condition_implied(new_coefficients.clone());

        if self.eq || self.ne {
            let negated_or_equal = ConstraintSystem::negate_or_equal(new_coefficients.clone());
            let negated_or_equal_implied = !negated_or_equal.is_empty() && sub.is_condition_implied(negated_or_equal);

            // In order to check that `%a == %b` is true (equality), both
            // conditions `%a >= %b` and `%a <= %b` must hold true.
            if implied && negated_or_equal_implied {
                return Some(self.eq);
            }

            let negated = ConstraintSystem::negate(new_coefficients.clone());
            let negated_implied = !negated.is_empty() && sub.is_condition_implied(negated);

            let strict = ConstraintSystem::to_strict_less_than(new_coefficients);
            let strict_implied = !strict.is_empty() && sub.is_condition_implied(strict);

            // In order to check that `%a != %b` is true (non-equality), either
            // `%a > %b` or `%a < %b` must hold true.
            if negated_implied || strict_implied {
                return Some(self.ne);
            }

            return None;
        }

        if implied {
            return Some(true);
        }

        let negated = ConstraintSystem::negate(new_coefficients);
        if !negated.is_empty() && sub.is_condition_implied(negated) {
            return Some(false);
        }

        // Neither the condition nor its negated holds, did not prove anything.
        None
    }
}

/// `StackEntry`.
struct StackEntry {
    num_in: u32,
    num_out: u32,
    signed: bool,
    /// Variables that can be removed from the system once the entry gets
    /// removed.
    values_to_release: Vec<Operand>,
}

fn is_signed(predicate: IntPredicate) -> bool {
    matches!(
        predicate,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

fn is_unsigned(predicate: IntPredicate) -> bool {
    matches!(
        predicate,
        IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
    )
}

fn is_equality(predicate: IntPredicate) -> bool {
    matches!(predicate, IntPredicate::Eq | IntPredicate::Ne)
}

fn signed_predicate(predicate: IntPredicate) -> IntPredicate {
    match predicate {
        IntPredicate::Ugt => IntPredicate::Sgt,
        IntPredicate::Uge => IntPredicate::Sge,
        IntPredicate::Ult => IntPredicate::Slt,
        IntPredicate::Ule => IntPredicate::Sle,
        other => other,
    }
}

fn unsigned_predicate(predicate: IntPredicate) -> IntPredicate {
    match predicate {
        IntPredicate::Sgt => IntPredicate::Ugt,
        IntPredicate::Sge => IntPredicate::Uge,
        IntPredicate::Slt => IntPredicate::Ult,
        IntPredicate::Sle => IntPredicate::Ule,
        other => other,
    }
}

fn strict_predicate(predicate: IntPredicate) -> IntPredicate {
    match predicate {
        IntPredicate::Uge => IntPredicate::Ugt,
        IntPredicate::Ule => IntPredicate::Ult,
        IntPredicate::Sge => IntPredicate::Sgt,
        IntPredicate::Sle => IntPredicate::Slt,
        other => other,
    }
}

/// `ConstraintInfo`: the unsigned and the signed system, and the decompositions
/// found against their current state.
struct ConstraintInfo<'u, 'a> {
    unit: &'u Unit<'a>,
    unsigned: ConstraintSystem,
    signed: ConstraintSystem,
    decompose_cache: HashMap<(Val, bool), Decomposition>,
}

impl<'u, 'a> ConstraintInfo<'u, 'a> {
    /// `ConstraintInfo(DL, FunctionArgs)`.
    fn new(unit: &'u Unit<'a>) -> Self {
        let arguments: Vec<Operand> = unit.function.parameters().iter().map(|&value| Operand::Value(value)).collect();
        let mut info = Self {
            unit,
            unsigned: ConstraintSystem::with_arguments(&arguments),
            signed: ConstraintSystem::with_arguments(&arguments),
            decompose_cache: HashMap::new(),
        };
        // Add Arg > -1 constraints to unsigned system for all function
        // arguments.
        for argument in &arguments {
            let index = info.unsigned.value2index()[argument];
            let size = info.unsigned.value2index().len();
            info.unsigned.add_row(&[Entry::new(0, 0), Entry::new(-1, index as u16)], size);
        }
        info
    }

    fn system(
        &self,
        signed: bool,
    ) -> &ConstraintSystem {
        if signed { &self.signed } else { &self.unsigned }
    }

    fn system_mut(
        &mut self,
        signed: bool,
    ) -> &mut ConstraintSystem {
        if signed { &mut self.signed } else { &mut self.unsigned }
    }

    fn width(
        &self,
        value: Val,
    ) -> Option<u32> {
        match value {
            Val::Int(_, width) => Some(width),
            Val::Op(operand) => self.unit.int_bits(operand),
        }
    }

    /// The integer `value` is, where it is a constant.
    fn constant(
        &self,
        value: Val,
    ) -> Option<Int> {
        match value {
            Val::Int(number, width) => Some(Int { bits: number as u128, width }),
            Val::Op(operand) => {
                Some(Int { bits: self.unit.int_constant(operand)?, width: self.unit.int_bits(operand)? })
            }
        }
    }

    fn defining(
        &self,
        value: Val,
    ) -> Option<&'a Instruction> {
        match value {
            Val::Op(operand) => self.unit.defining(operand).map(|(_, instruction)| instruction),
            Val::Int(..) => None,
        }
    }

    fn zero(
        &self,
        like: Val,
    ) -> Val {
        Val::Int(0, self.width(like).unwrap_or(1))
    }

    fn is_zero(
        &self,
        value: Val,
    ) -> bool {
        self.constant(value).is_some_and(|number| number.zext() == 0)
    }

    /// `ConstraintInfo::isKnownNonNegative`.
    fn is_known_non_negative(
        &mut self,
        value: Val,
    ) -> bool {
        if let Some(number) = self.constant(value) {
            return !number.is_negative();
        }
        self.value_tracking_non_negative(value) || self.does_hold(IntPredicate::Sge, value, self.zero(value))
    }

    /// `::isKnownNonNegative(V, DL)`, the stand-in (see the header).
    fn value_tracking_non_negative(
        &self,
        value: Val,
    ) -> bool {
        let Some(instruction) = self.defining(value) else { return false };
        let operand = |at: usize| Val::Op(instruction.operands[at]);
        match instruction.opcode {
            Opcode::Cast(CastOp::ZExt) => {
                // A zero extension is non-negative when it widens.
                matches!(
                    (self.width(operand(0)), self.width(value)),
                    (Some(from), Some(to)) if from < to
                )
            }
            Opcode::Binary(BinaryOp::And) => {
                (0..2).any(|at| self.constant(operand(at)).is_some_and(|mask| !mask.is_negative()))
            }
            Opcode::Binary(BinaryOp::UDiv) | Opcode::Binary(BinaryOp::URem) => {
                matches!(
                    (self.constant(operand(1)), self.width(value)),
                    (Some(divisor), Some(_)) if !divisor.is_negative()
                )
            }
            Opcode::Binary(BinaryOp::LShr) => self.constant(operand(1)).is_some_and(|shift| shift.zext() > 0),
            _ => false,
        }
    }

    /// `canUseSExt`.
    fn can_use_sext(number: Int) -> bool {
        let value = number.sext();
        value > i128::from(MIN_SIGNED_CONSTRAINT_VALUE) && value < i128::from(MAX_CONSTRAINT_VALUE)
    }

    /// `doesHoldInRange`: whether the systems imply `op` is in the range
    /// `[low, high]` (read signed if `signed`, unsigned otherwise); None where
    /// the range is empty or wraps.
    fn does_hold_in_range(
        &mut self,
        op: Val,
        low: i128,
        high: i128,
        signed: bool,
    ) -> bool {
        let Some(width) = self.width(op) else { return false };
        if low > high {
            return false;
        }
        let (min_val, max_val) =
            if signed { (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1) } else { (0i128, (1i128 << width) - 1) };
        if low <= min_val && high >= max_val {
            return true;
        }
        let mut high = high;
        // Replace bound too large to be decomposed by the largest usable one.
        if !signed && high >= i128::from(MAX_CONSTRAINT_VALUE) {
            high = i128::from(MAX_CONSTRAINT_VALUE) - 1;
        }
        if low != min_val
            && !self.does_hold(if signed { IntPredicate::Sge } else { IntPredicate::Uge }, op, Val::Int(low, width))
        {
            return false;
        }
        if high != max_val
            && !self.does_hold(if signed { IntPredicate::Sle } else { IntPredicate::Ule }, op, Val::Int(high, width))
        {
            return false;
        }
        true
    }

    /// `isKnownNoWrap(Opcode, Op0, Op1, NoWrapFlags, Info, Signed)`.
    fn is_known_no_wrap_op(
        &mut self,
        opcode: BinaryOp,
        op0: Val,
        op1: Val,
        flags: Flags,
        signed: bool,
    ) -> bool {
        if flags.contains(if signed { Flags::NSW } else { Flags::NUW }) {
            return true;
        }

        if opcode == BinaryOp::Sub {
            // Op0 - Op1 does not wrap unsigned if Op0 >=u Op1.
            if !signed {
                return self.does_hold(IntPredicate::Uge, op0, op1);
            }

            // Op0 - Op1 does not wrap signed if 0 <=s Op1 <=s Op0.
            if self.is_known_non_negative(op1) && self.does_hold(IntPredicate::Sge, op0, op1) {
                return true;
            }
        }

        if !signed
            && flags.contains(Flags::NSW)
            && (opcode == BinaryOp::Shl || self.is_known_non_negative(op1))
            && self.is_known_non_negative(op0)
        {
            return true;
        }

        // For a constant Op1, the ranges of Op0 for which the operation does
        // not wrap are known exactly; check if the systems imply one of
        // them.
        let Some(c) = self.constant(op1) else { return false };
        let Some(width) = self.width(op0) else { return false };
        let (min_s, max_s) = (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1);
        let max_u = (1i128 << width) - 1;
        // `ConstantRange::makeExactNoWrapRegion`, for add and sub.
        let region = match (opcode, signed) {
            (BinaryOp::Add, false) => Some((0, max_u - c.zext() as i128)),
            (BinaryOp::Add, true) => {
                let value = c.sext();
                Some(if value >= 0 { (min_s, max_s - value) } else { (min_s - value, max_s) })
            }
            (BinaryOp::Sub, false) => Some((c.zext() as i128, max_u)),
            (BinaryOp::Sub, true) => {
                let value = c.sext();
                Some(if value >= 0 { (min_s + value, max_s) } else { (min_s, max_s + value) })
            }
            // DEVIATE: mul and shl.
            _ => None,
        };
        match region {
            Some((low, high)) => self.does_hold_in_range(op0, low, high, signed),
            None => false,
        }
    }

    /// `isKnownNoWrap(Value *V, Info, Signed)`.
    fn is_known_no_wrap(
        &mut self,
        value: Val,
        signed: bool,
    ) -> bool {
        let Some(instruction) = self.defining(value) else { return false };
        match instruction.opcode {
            Opcode::Binary(BinaryOp::Or) if instruction.flags.contains(Flags::DISJOINT) => true,
            Opcode::Cast(CastOp::Trunc) => {
                if signed {
                    return instruction.flags.contains(Flags::NSW);
                }
                // A trunc nsw only truncates without unsigned wrap if its
                // operand is non-negative.
                instruction.flags.contains(Flags::NUW)
                    || (instruction.flags.contains(Flags::NSW)
                        && self.is_known_non_negative(Val::Op(instruction.operands[0])))
            }
            Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl)) => self
                .is_known_no_wrap_op(
                    op,
                    Val::Op(instruction.operands[0]),
                    Val::Op(instruction.operands[1]),
                    instruction.flags,
                    signed,
                ),
            _ => false,
        }
    }

    /// `mayLookThrough`.
    fn may_look_through(
        &self,
        value: Val,
    ) -> bool {
        let Some(instruction) = self.defining(value) else { return false };
        matches!(
            instruction.opcode,
            Opcode::Binary(
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl | BinaryOp::Or | BinaryOp::Xor
            ) | Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
        )
    }

    /// `decompose`.
    fn decompose(
        &mut self,
        value: Val,
        signed: bool,
    ) -> Decomposition {
        if !self.may_look_through(value) {
            return self.decompose_impl(value, signed);
        }
        if let Some(found) = self.decompose_cache.get(&(value, signed)) {
            return found.clone();
        }
        let result = self.decompose_impl(value, signed);
        self.decompose_cache.insert((value, signed), result.clone());
        result
    }

    /// The decomposition of `value` as itself: a variable, or its number.
    fn itself(
        &self,
        value: Val,
    ) -> Decomposition {
        match value {
            Val::Op(operand) => Decomposition::variable(operand),
            Val::Int(number, _) => Decomposition::constant(number as i64),
        }
    }

    /// `decomposeImpl`'s `MergeResults`.
    fn merge_results(
        &mut self,
        a: Val,
        b: Val,
        signed: bool,
        signed_b: bool,
    ) -> Option<Decomposition> {
        let mut result_a = self.decompose(a, signed);
        let result_b = self.decompose(b, signed_b);
        if result_a.add(&result_b) {
            return None;
        }
        Some(result_a)
    }

    /// `decomposeImpl`.
    fn decompose_impl(
        &mut self,
        mut value: Val,
        signed: bool,
    ) -> Decomposition {
        // Don't handle integers > 64 bit. Our coefficients are 64-bit large, so
        // coefficient add/mul may wrap, while the operation in the full bit
        // width would not.
        let Some(type_bits) = self.width(value).filter(|&bits| bits <= 64) else { return self.itself(value) };

        if let Some(number) = self.constant(value) {
            if signed {
                if Self::can_use_sext(number) {
                    return Decomposition::constant(number.sext() as i64);
                }
            } else if number.zext() < MAX_CONSTRAINT_VALUE as u128 {
                return Decomposition::constant(number.zext() as i64);
            }
            return self.itself(value);
        }

        let original = value;
        let instruction = match self.defining(value) {
            Some(instruction) => instruction,
            None => return self.itself(value),
        };

        match instruction.opcode {
            Opcode::Cast(CastOp::ZExt) => {
                // In the signed system, the ZExt must be non-negative.
                if signed && !instruction.flags.contains(Flags::NNEG) {
                    return self.itself(value);
                }
                value = Val::Op(instruction.operands[0]);
            }
            Opcode::Cast(CastOp::SExt) => {
                // In the unsigned system, the SExt operand must be
                // non-negative.
                if !signed && !self.is_known_non_negative(Val::Op(instruction.operands[0])) {
                    return self.itself(value);
                }
                value = Val::Op(instruction.operands[0]);
            }
            Opcode::Cast(CastOp::Trunc) => {
                let source = Val::Op(instruction.operands[0]);
                if self.width(source).is_some_and(|bits| bits <= 64) && self.is_known_no_wrap(original, signed) {
                    value = source;
                }
            }
            _ => {}
        }

        // Match on what `value` is now (the operand a cast was looked through),
        // as `decomposeImpl` does.
        let Some(instruction) = self.defining(value) else { return self.itself(value) };
        let op = |at: usize| Val::Op(instruction.operands[at]);
        match instruction.opcode {
            // m_AddLike: an add, or an or of bits that cannot overlap.
            Opcode::Binary(BinaryOp::Add) | Opcode::Binary(BinaryOp::Or)
                if instruction.opcode == Opcode::Binary(BinaryOp::Add)
                    || instruction.flags.contains(Flags::DISJOINT) =>
            {
                if self.is_known_no_wrap(value, signed) {
                    return match self.merge_results(op(0), op(1), signed, signed) {
                        Some(decomposition) => decomposition,
                        None => self.itself(value),
                    };
                }
                // In the unsigned system, adding a negative constant only wraps
                // if Op0 is smaller than it.
                if !signed
                    && let Some(c) = self.constant(op(1))
                    && c.is_negative()
                    && Self::can_use_sext(c)
                    && self.does_hold(
                        IntPredicate::Uge,
                        op(0),
                        Val::Int(-c.sext(), self.width(op(0)).unwrap_or(type_bits)),
                    )
                    && let Some(merged) = self.merge_results(op(0), op(1), signed, true)
                {
                    return merged;
                }
                self.itself(value)
            }
            // `xor %x, -1` is equivalent to `sub nsw -1, %x`.
            Opcode::Binary(BinaryOp::Xor) if signed && self.constant(op(1)).is_some_and(|c| c.sext() == -1) => {
                let mut result = Decomposition::constant(-1);
                let inner = self.decompose(op(0), signed);
                if !result.sub(&inner) {
                    return result;
                }
                self.itself(value)
            }
            Opcode::Binary(BinaryOp::Sub) => {
                if self.is_known_no_wrap(value, signed) {
                    let mut result_a = self.decompose(op(0), signed);
                    let result_b = self.decompose(op(1), signed);
                    if !result_a.sub(&result_b) {
                        return result_a;
                    }
                }
                self.itself(value)
            }
            Opcode::Binary(BinaryOp::Mul) if self.constant(op(1)).is_some() => {
                let c = self.constant(op(1)).expect("a constant");
                // A negative constant is only a valid coefficient in the signed
                // system; in the unsigned system the multiplier is the
                // constant's unsigned value.
                if Self::can_use_sext(c) && (signed || !c.is_negative()) && self.is_known_no_wrap(value, signed) {
                    let mut result = self.decompose(op(0), signed);
                    if !result.mul(c.sext() as i64) {
                        return result;
                    }
                }
                self.itself(value)
            }
            Opcode::Binary(BinaryOp::Shl) if self.constant(op(1)).is_some() => {
                let c = self.constant(op(1)).expect("a constant");
                // (shl x, shift) is (mul x, 1 << shift). The scale must fit in
                // the signed coefficient, so reject shifts >= 63. Also reject a
                // shift of bw-1, for which the product is not representable.
                let max_shift = if signed { i128::from(type_bits) - 1 } else { 63 };
                if !c.is_negative() && c.sext() < max_shift && self.is_known_no_wrap(value, signed) {
                    let mut result = self.decompose(op(0), signed);
                    if !result.mul(1i64 << c.sext()) {
                        return result;
                    }
                }
                self.itself(value)
            }
            _ => self.itself(value),
        }
    }

    /// `getRowForLessEqual`: the row for `ADec <= BDec`, using the indices from
    /// the system's value-to-index map. Variables not in it are appended to
    /// `new_variables` and get the indices following. Empty if the
    /// coefficients overflow.
    fn row_for_less_equal(
        a: &Decomposition,
        b: &Decomposition,
        value2index: &llrm_support::hash::IndexMap<Operand, u32>,
        new_variables: &mut Vec<Operand>,
    ) -> Row {
        // Build the row, by first adding all coefficients from A and then
        // subtracting all coefficients from B.
        let Some(offset_sum) = b.offset.checked_sub(a.offset) else { return Vec::new() };
        let mut row: Row = vec![Entry::new(offset_sum, 0)];
        // The entry for `idx`, or the place to insert it at, is the first entry
        // with an index >= idx.
        fn coefficient(
            row: &mut Row,
            idx: u32,
        ) -> &mut i64 {
            let at =
                row.iter().skip(1).position(|entry| u32::from(entry.id) >= idx).map(|at| at + 1).unwrap_or(row.len());
            if at >= row.len() || u32::from(row[at].id) != idx {
                row.insert(at, Entry::new(0, idx as u16));
            }
            &mut row[at].coefficient
        }
        // First try to look up V in Value2Index and NewVariables. Otherwise add
        // a new entry to NewVariables.
        let mut index_of = |variable: Operand, new_variables: &mut Vec<Operand>| -> u32 {
            if let Some(&index) = value2index.get(&variable) {
                return index;
            }
            let idx = match new_variables.iter().position(|one| *one == variable) {
                Some(at) => at,
                None => {
                    new_variables.push(variable);
                    new_variables.len() - 1
                }
            };
            (value2index.len() + idx + 1) as u32
        };
        for &(factor, variable) in &a.vars {
            let idx = index_of(variable, new_variables);
            *coefficient(&mut row, idx) += factor;
        }

        for &(factor, variable) in &b.vars {
            let idx = index_of(variable, new_variables);
            let slot = coefficient(&mut row, idx);
            match slot.checked_sub(factor) {
                Some(difference) => *slot = difference,
                None => return Vec::new(),
            }
        }

        // Drop coefficients that cancelled out.
        row.retain(|entry| entry.id == 0 || entry.coefficient != 0);
        row
    }

    /// `ConstraintInfo::getConstraint`.
    fn get_constraint(
        &mut self,
        mut predicate: IntPredicate,
        mut op0: Val,
        mut op1: Val,
        new_variables: &mut Vec<Operand>,
        force_signed_system: bool,
    ) -> ConstraintTy {
        debug_assert!(new_variables.is_empty(), "NewVariables must be empty when passed in");
        let (mut is_eq, mut is_ne) = (false, false);

        // Try to convert Pred to one of ULE/ULT/SLE/SLT.
        match predicate {
            IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Sgt | IntPredicate::Sge => {
                predicate = predicate.swapped();
                std::mem::swap(&mut op0, &mut op1);
            }
            IntPredicate::Eq => {
                if !force_signed_system && self.is_zero(op1) {
                    predicate = IntPredicate::Ule;
                } else {
                    is_eq = true;
                    predicate = IntPredicate::Ule;
                }
            }
            IntPredicate::Ne => {
                if !force_signed_system && self.is_zero(op1) {
                    predicate = IntPredicate::Ugt.swapped();
                    std::mem::swap(&mut op0, &mut op1);
                } else {
                    is_ne = true;
                    predicate = IntPredicate::Ule;
                }
            }
            _ => {}
        }

        if !matches!(
            predicate,
            IntPredicate::Ule | IntPredicate::Ult | IntPredicate::Sle | IntPredicate::Slt
        ) {
            return ConstraintTy::default();
        }

        let signed = force_signed_system || is_signed(predicate);
        let a_dec = self.decompose(op0, signed);
        let b_dec = self.decompose(op1, signed);
        let mut row = {
            let value2index = self.system(signed).value2index();
            Self::row_for_less_equal(&a_dec, &b_dec, value2index, new_variables)
        };
        if row.is_empty() {
            return ConstraintTy::default();
        }

        if matches!(predicate, IntPredicate::Slt | IntPredicate::Ult) {
            match row[0].coefficient.checked_add(-1) {
                Some(sum) => row[0].coefficient = sum,
                None => return ConstraintTy::default(),
            }
        }

        // Remove any new variable without a coefficient in the row.
        let num_v2i = self.system(signed).value2index().len() as u32;
        let last_id = u32::from(row.last().expect("a row").id);
        new_variables.truncate(if last_id > num_v2i { (last_id - num_v2i) as usize } else { 0 });

        ConstraintTy {
            coefficients: row,
            num_vars: self.system(signed).value2index().len() + new_variables.len(),
            signed,
            eq: is_eq,
            ne: is_ne,
        }
    }

    /// `ConstraintInfo::getConstraintForSolving`.
    fn get_constraint_for_solving(
        &mut self,
        mut predicate: IntPredicate,
        op0: Val,
        op1: Val,
    ) -> ConstraintTy {
        // Handle trivially true compares directly to avoid adding V UGE 0
        // constraints for all variables in the unsigned system.
        if (predicate == IntPredicate::Ule && self.is_zero(op0))
            || (predicate == IntPredicate::Uge && self.is_zero(op1))
        {
            // Return constraint that's trivially true.
            return ConstraintTy { coefficients: vec![Entry::new(0, 0)], ..ConstraintTy::default() };
        }

        // If both operands are known to be non-negative, change signed
        // predicates to unsigned ones. This increases the reasoning
        // effectiveness in combination with the signed <-> unsigned transfer
        // logic.
        if is_signed(predicate)
            && self.value_tracking_non_negative_or_constant(op0)
            && self.value_tracking_non_negative_or_constant(op1)
        {
            predicate = unsigned_predicate(predicate);
        }

        let mut new_variables = Vec::new();
        let result = self.get_constraint(predicate, op0, op1, &mut new_variables, false);
        if !new_variables.is_empty() {
            return ConstraintTy::default();
        }
        result
    }

    /// `::isKnownNonNegative(Op, DL, Depth)`, as `getConstraintForSolving`
    /// asks.
    fn value_tracking_non_negative_or_constant(
        &self,
        value: Val,
    ) -> bool {
        match self.constant(value) {
            Some(number) => !number.is_negative(),
            None => self.value_tracking_non_negative(value),
        }
    }

    /// `ConstraintInfo::doesHold`.
    fn does_hold(
        &mut self,
        predicate: IntPredicate,
        a: Val,
        b: Val,
    ) -> bool {
        let constraint = self.get_constraint_for_solving(predicate, a, b);
        !constraint.empty()
            && self.system(constraint.signed).is_condition_implied_in_sub_system(&constraint.coefficients)
    }

    /// `ConstraintInfo::addFact`.
    fn add_fact(
        &mut self,
        predicate: IntPredicate,
        a: Val,
        b: Val,
        num_in: u32,
        num_out: u32,
        stack: &mut Vec<StackEntry>,
    ) {
        self.add_fact_impl(predicate, a, b, num_in, num_out, stack, false);
        // If the Pred is eq/ne, also add the fact to signed system.
        if is_equality(predicate) {
            self.add_fact_impl(predicate, a, b, num_in, num_out, stack, true);
        }
        if predicate == IntPredicate::Ne {
            self.tighten_bound_using_ne(a, b, num_in, num_out, stack);
        }
    }

    /// `ConstraintInfo::tightenBoundUsingNe`.
    fn tighten_bound_using_ne(
        &mut self,
        a: Val,
        b: Val,
        num_in: u32,
        num_out: u32,
        stack: &mut Vec<StackEntry>,
    ) {
        if self.width(a).is_none() {
            return;
        }

        for signed in [false, true] {
            // In the unsigned system `A u>= 0` holds for every A, so
            // getConstraint already turned `A != 0` into `A u> 0`.
            if !signed && self.is_zero(b) {
                continue;
            }

            // Skip if there are any unknown variables.
            let decomposition = self.decompose(a, signed);
            if decomposition.vars.iter().any(|(_, variable)| !self.system(signed).value2index().contains_key(variable))
            {
                continue;
            }

            // If the system implies `A >= B` then together with `A != B` we get
            // the strict `A > B`; symmetrically `A <= B` becomes `A < B`.
            let (ge, le) =
                if signed { (IntPredicate::Sge, IntPredicate::Sle) } else { (IntPredicate::Uge, IntPredicate::Ule) };
            for non_strict in [ge, le] {
                if !self.does_hold(non_strict, a, b) {
                    continue;
                }
                let strict = strict_predicate(non_strict);
                self.add_fact_impl(strict, a, b, num_in, num_out, stack, false);
                break;
            }
        }
    }

    /// `ConstraintInfo::addFactImpl`.
    fn add_fact_impl(
        &mut self,
        predicate: IntPredicate,
        a: Val,
        b: Val,
        num_in: u32,
        num_out: u32,
        stack: &mut Vec<StackEntry>,
        force_signed_system: bool,
    ) {
        let mut new_variables = Vec::new();
        let r = self.get_constraint(predicate, a, b, &mut new_variables, force_signed_system);

        // TODO (LLVM): support non-equality for facts as well.
        if r.empty() || r.ne {
            return;
        }

        let added = self.system_mut(r.signed).add_row(&r.coefficients, r.num_vars);
        if !added {
            return;
        }

        self.decompose_cache.clear();

        // If R has been added to the system, add the new variables and queue it
        // for removal once it goes out-of-scope.
        let mut values_to_release = Vec::new();
        for &variable in &new_variables {
            let next = self.system(r.signed).value2index().len() as u32 + 1;
            self.system_mut(r.signed).value2index_mut().entry(variable).or_insert(next);
            values_to_release.push(variable);
        }

        stack.push(StackEntry { num_in, num_out, signed: r.signed, values_to_release });

        if !r.signed {
            for &variable in &new_variables {
                // Add V > -1 constraints for all new variables.
                let index = self.unsigned.value2index()[&variable];
                let size = self.unsigned.value2index().len();
                self.unsigned.add_row(&[Entry::new(0, 0), Entry::new(-1, index as u16)], size);
                stack.push(StackEntry { num_in, num_out, signed: r.signed, values_to_release: Vec::new() });
            }
        }

        if r.eq {
            // Also add the inverted constraint for equality constraints.
            let mut inverted = r.coefficients.clone();
            for entry in &mut inverted {
                match entry.coefficient.checked_mul(-1) {
                    Some(product) => entry.coefficient = product,
                    None => return,
                }
            }
            self.system_mut(r.signed).add_row(&inverted, r.num_vars);

            stack.push(StackEntry { num_in, num_out, signed: r.signed, values_to_release: Vec::new() });
        }
    }

    /// `ConstraintInfo::transferToOtherSystem`.
    fn transfer_to_other_system(
        &mut self,
        predicate: IntPredicate,
        a: Val,
        b: Val,
        num_in: u32,
        num_out: u32,
        stack: &mut Vec<StackEntry>,
    ) {
        // Check if we can combine facts from the signed and unsigned systems to
        // derive additional facts.
        if self.width(a).is_none() {
            return;
        }
        let zero_b = Val::Int(0, self.width(b).unwrap_or(1));
        match predicate {
            IntPredicate::Ult | IntPredicate::Ule => {
                // If B is a signed positive constant, then A >=s 0 and A <s (or
                // <=s) B.
                if self.is_known_non_negative(b) {
                    self.add_fact(IntPredicate::Sge, a, zero_b, num_in, num_out, stack);
                    self.add_fact(signed_predicate(predicate), a, b, num_in, num_out, stack);
                }
            }
            IntPredicate::Uge | IntPredicate::Ugt => {
                // If A is a signed positive constant, then B >=s 0 and A >s (or
                // >=s) B.
                if self.is_known_non_negative(a) {
                    self.add_fact(IntPredicate::Sge, b, zero_b, num_in, num_out, stack);
                    self.add_fact(signed_predicate(predicate), a, b, num_in, num_out, stack);
                }
            }
            IntPredicate::Slt | IntPredicate::Sle => {
                if self.is_known_non_negative(a) {
                    self.add_fact(unsigned_predicate(predicate), a, b, num_in, num_out, stack);
                }
            }
            IntPredicate::Sgt => {
                let all_ones = Val::Int(-1, self.width(b).unwrap_or(1));
                if self.does_hold(IntPredicate::Sge, b, all_ones) {
                    self.add_fact(IntPredicate::Uge, a, zero_b, num_in, num_out, stack);
                }
                if self.is_known_non_negative(b) {
                    self.add_fact(IntPredicate::Ugt, a, b, num_in, num_out, stack);
                }
            }
            IntPredicate::Sge => {
                if self.is_known_non_negative(b) {
                    self.add_fact(IntPredicate::Uge, a, b, num_in, num_out, stack);
                }
            }
            _ => {}
        }
    }

    /// `removeEntryFromStack`.
    fn remove_entry_from_stack(
        &mut self,
        entry: &StackEntry,
    ) {
        self.decompose_cache.clear();
        self.system_mut(entry.signed).pop_last_constraint();
        // Remove variables in the system that went out of scope.
        for variable in &entry.values_to_release {
            self.system_mut(entry.signed).value2index_mut().shift_remove(variable);
        }
        self.system_mut(entry.signed).pop_last_n_variables(entry.values_to_release.len());
    }

    /// `getDecompositionLinkRow`.
    fn decomposition_link_row(
        &mut self,
        value: Val,
        c: &ConstraintTy,
    ) -> Row {
        let Val::Op(operand) = value else { return Vec::new() };
        let Some(&index) = self.system(c.signed).value2index().get(&operand) else { return Vec::new() };
        if c.coefficients.iter().any(|entry| u32::from(entry.id) == index) {
            return Vec::new();
        }
        let mut new_variables = Vec::new();
        let decomposition = self.decompose(value, c.signed);
        let row = Self::row_for_less_equal(
            &Decomposition::variable(operand),
            &decomposition,
            self.system(c.signed).value2index(),
            &mut new_variables,
        );
        if new_variables.is_empty() { row } else { Vec::new() }
    }

    /// `checkCondition`.
    fn check_condition(
        &mut self,
        predicate: IntPredicate,
        a: Val,
        b: Val,
    ) -> Option<bool> {
        let r = self.get_constraint_for_solving(predicate, a, b);
        if let Some(implied) = self.try_with_constraint(&r) {
            return Some(implied);
        }
        if let Some(implied) = self.try_with_linked_decomposition(&r, a, b) {
            return Some(implied);
        }

        // For non-negative operands unsigned queries can also be checked
        // against the signed system.
        if is_unsigned(predicate) && self.width(a).is_some() {
            let mut new_variables = Vec::new();
            let sr = self.get_constraint(signed_predicate(predicate), a, b, &mut new_variables, false);
            if new_variables.is_empty() && !sr.empty() && self.is_known_non_negative(a) && self.is_known_non_negative(b)
            {
                if let Some(implied) = self.try_with_constraint(&sr) {
                    return Some(implied);
                }
            }
        }

        // Additionally, query the signed system for eq/ne predicates if we know
        // about A or B.
        if is_equality(predicate) {
            let known = |system: &ConstraintSystem, value: Val| match value {
                Val::Op(operand) => system.value2index().contains_key(&operand),
                Val::Int(..) => false,
            };
            if !known(&self.signed, a) && !known(&self.signed, b) {
                return None;
            }

            let mut new_variables = Vec::new();
            let sr = self.get_constraint(predicate, a, b, &mut new_variables, true);
            if new_variables.is_empty() {
                if let Some(implied) = self.try_with_constraint(&sr) {
                    return Some(implied);
                }
                if let Some(implied) = self.try_with_linked_decomposition(&sr, a, b) {
                    return Some(implied);
                }
            }
        }
        None
    }

    /// `checkCondition`'s `TryWithConstraint`.
    fn try_with_constraint(
        &self,
        r: &ConstraintTy,
    ) -> Option<bool> {
        if r.empty() {
            return None;
        }
        r.implied_by(self.system(r.signed))
    }

    /// `checkCondition`'s `TryWithLinkedDecomposition`.
    fn try_with_linked_decomposition(
        &mut self,
        c: &ConstraintTy,
        a: Val,
        b: Val,
    ) -> Option<bool> {
        if c.empty() {
            return None;
        }

        let num_vars = self.system(c.signed).value2index().len();
        let mut pushed = 0;
        for value in [a, b] {
            let row = self.decomposition_link_row(value, c);
            let negated = ConstraintSystem::negate_or_equal(row.clone());
            if row.is_empty() || negated.is_empty() {
                continue;
            }
            pushed += usize::from(self.system_mut(c.signed).add_row(&row, num_vars));
            pushed += usize::from(self.system_mut(c.signed).add_row(&negated, num_vars));
        }
        if pushed == 0 {
            return None;
        }

        let result = self.try_with_constraint(c);
        for _ in 0..pushed {
            self.system_mut(c.signed).pop_last_constraint();
        }
        result
    }
}

/// `FactOrCheck`, the two kinds the spike makes: a condition that holds on
/// entry to a block, and a compare to ask about, the branch of `block` using
/// it.
enum Entry_ {
    Fact {
        predicate: IntPredicate,
        a: Val,
        b: Val,
    },
    Check {
        predicate: IntPredicate,
        a: Val,
        b: Val,
        branch_block: BlockId,
    },
    /// `FactOrCheck::getCheck(DTN, I, ContextInst)` of an instruction whose
    /// flags may be strengthened, checked at the point dominating all its uses.
    Strengthen {
        inst: llrm_mir::module::InstId,
    },
}

struct FactOrCheck {
    entry: Entry_,
    num_in: u32,
    num_out: u32,
    /// The order of the instruction in its block, for checks.
    position: usize,
}

impl FactOrCheck {
    fn is_check(&self) -> bool {
        matches!(self.entry, Entry_::Check { .. } | Entry_::Strengthen { .. })
    }
}

/// The depth-first numbers of the dominator tree (`DT.updateDFSNumbers()`).
fn dfs_numbers(unit: &Unit) -> HashMap<i64, (u32, u32)> {
    let function = unit.function;
    let shape = unit.shape();
    let mut children: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut roots = Vec::new();
    for &block in function.layout() {
        let at = cfg::id(block);
        if !shape.dominance.reachable(at) {
            continue;
        }
        match shape.dominance.immediate(at) {
            Some(parent) => children.entry(parent).or_default().push(at),
            None => roots.push(at),
        }
    }
    let mut numbers = HashMap::new();
    let mut counter = 0u32;
    for root in roots {
        let mut stack = vec![(root, false)];
        while let Some((at, done)) = stack.pop() {
            if done {
                counter += 1;
                numbers.get_mut(&at).map(|number: &mut (u32, u32)| number.1 = counter);
                continue;
            }
            counter += 1;
            numbers.insert(at, (counter, 0));
            stack.push((at, true));
            if let Some(below) = children.get(&at) {
                for &child in below.iter().rev() {
                    stack.push((child, false));
                }
            }
        }
    }
    numbers
}

/// `State::canAddSuccessor`: `DT.dominates(BasicBlockEdge(&BB, Succ), Succ)`.
fn can_add_successor(
    unit: &Unit,
    block: BlockId,
    successor: BlockId,
) -> bool {
    let function = unit.function;
    let shape = unit.shape();
    // An edge dominates a block it ends at if it is the only way in, or every
    // other way in is from a block the block dominates; a branch with both its
    // edges to the block has no edge of its own.
    let edges = function.terminator(block).map_or(0, |last| {
        function.instruction(last).operands.iter().filter(|operand| **operand == Operand::Block(successor)).count()
    });
    if edges != 1 {
        return false;
    }
    function
        .predecessors(successor)
        .iter()
        .all(|&from| from == block || shape.dominance.dominates(cfg::id(successor), cfg::id(from)))
}

/// The branches of `unit`'s function that the constraint system decides, with
/// the way each goes (true: the first successor), and the instruction count
/// the walk took.
pub fn decisions(unit: &Unit) -> Vec<(BlockId, bool)> {
    eliminate(unit).decisions
}

/// What the walk found: the branches decided, and the flags to add to
/// instructions (`tryToStrengthenFlags`).
#[derive(Default)]
pub struct Outcome {
    pub decisions: Vec<(BlockId, bool)>,
    pub flags: Vec<(InstId, Flags)>,
}

/// `canStrengthenFlags`.
fn can_strengthen_flags(
    unit: &Unit,
    inst: InstId,
) -> bool {
    let instruction = unit.function.instruction(inst);
    let Some(result) = instruction.result else { return false };
    let Some(width) = unit.int_bits(Operand::Value(result)) else { return false };
    let _ = width;
    let flags = instruction.flags;
    match instruction.opcode {
        Opcode::Cast(CastOp::Trunc) => flags.contains(Flags::NSW) && !flags.contains(Flags::NUW),
        Opcode::Binary(BinaryOp::Sub) => !(flags.contains(Flags::NUW) && flags.contains(Flags::NSW)),
        Opcode::Binary(BinaryOp::Add | BinaryOp::Mul | BinaryOp::Shl) => {
            if flags.contains(Flags::NUW) && flags.contains(Flags::NSW) {
                return false;
            }
            // With a constant second operand, we can use bounds on the first
            // operand to refine no-wrap flags. Independently, nuw can be added
            // for nsw if the operands are non-negative.
            unit.int_constant(instruction.operands[1]).is_some() || flags.contains(Flags::NSW)
        }
        _ => false,
    }
}

/// `findCommonDominatorOfUses`: the closest program point dominating all uses
/// of `inst`'s result, as a block and the position of the instruction in it.
fn common_dominator_of_uses(
    unit: &Unit,
    inst: InstId,
    numbers: &HashMap<i64, (u32, u32)>,
) -> (BlockId, usize) {
    let function = unit.function;
    let shape = unit.shape();
    let own = function.parent(inst).expect("a placed instruction");
    let place = |at: InstId| -> (BlockId, usize) {
        let block = function.parent(at).expect("a placed instruction");
        (block, function.block(block).instructions().iter().position(|one| *one == at).unwrap_or(0))
    };
    let own_place = place(inst);
    let Some(result) = function.instruction(inst).result else { return own_place };
    let mut common: Option<(BlockId, usize)> = None;
    let mut uses = 0;
    for one in function.users(result) {
        // Conservatively use original instruction, if there are too many uses.
        uses += 1;
        if uses == 16 {
            return own_place;
        }
        // `getContextInstForUse`: a phi's use is at the end of the incoming
        // block.
        let context = match function.instruction(one.user).opcode {
            Opcode::Phi => {
                let Operand::Block(from) = function.instruction(one.user).operands[one.index as usize + 1] else {
                    continue;
                };
                let last = function.terminator(from).expect("a terminator");
                place(last)
            }
            _ => place(one.user),
        };
        common = Some(match common {
            None => context,
            Some(have) => {
                // `findNearestCommonDominator(Instruction *, Instruction *)`.
                if have.0 == context.0 {
                    if have.1 <= context.1 { have } else { context }
                } else {
                    let mut chain = std::collections::HashSet::new();
                    let mut above = Some(cfg::id(have.0));
                    while let Some(one) = above {
                        chain.insert(one);
                        above = shape.dominance.immediate(one);
                    }
                    let mut up = Some(cfg::id(context.0));
                    let mut nearest = None;
                    while let Some(one) = up {
                        if chain.contains(&one) {
                            nearest = Some(one);
                            break;
                        }
                        up = shape.dominance.immediate(one);
                    }
                    match nearest {
                        Some(at) if at == cfg::id(have.0) => have,
                        Some(at) if at == cfg::id(context.0) => context,
                        Some(at) => {
                            let block = cfg::block(at);
                            (block, function.block(block).instructions().len().saturating_sub(1))
                        }
                        None => have,
                    }
                }
            }
        });
    }
    // Uses in unreachable blocks are not in the dominator tree.
    match common {
        Some(found) if numbers.contains_key(&cfg::id(found.0)) => found,
        _ => {
            let _ = own;
            own_place
        }
    }
}

/// The walk of `eliminateConstraints`.
pub fn eliminate(unit: &Unit) -> Outcome {
    let function = unit.function;
    let numbers = dfs_numbers(unit);
    let mut info = ConstraintInfo::new(unit);
    let mut work: Vec<FactOrCheck> = Vec::new();

    // `State::addInfoFor`, for the branches and switches.
    for &block in function.layout() {
        let at = cfg::id(block);
        let Some(&(num_in, num_out)) = numbers.get(&at) else { continue };
        let Some(last) = function.terminator(block) else { continue };
        let terminator = function.instruction(last);
        let position = function.block(block).instructions().len();
        match terminator.opcode {
            Opcode::Switch => {
                for pair in terminator.operands[2..].chunks(2) {
                    let [value, Operand::Block(successor)] = pair else { continue };
                    if !can_add_successor(unit, block, *successor) {
                        continue;
                    }
                    let Some(&(s_in, s_out)) = numbers.get(&cfg::id(*successor)) else { continue };
                    work.push(FactOrCheck {
                        entry: Entry_::Fact {
                            predicate: IntPredicate::Eq,
                            a: Val::Op(terminator.operands[0]),
                            b: Val::Op(*value),
                        },
                        num_in: s_in,
                        num_out: s_out,
                        position: 0,
                    });
                }
            }
            Opcode::Br if terminator.operands.len() == 3 => {
                let [condition, Operand::Block(yes), Operand::Block(no)] = terminator.operands[..] else { continue };
                // A check for the compare this branch uses.
                if let Some((_, compare)) = unit.defining(condition)
                    && let Opcode::ICmp(predicate) = compare.opcode
                {
                    work.push(FactOrCheck {
                        entry: Entry_::Check {
                            predicate,
                            a: Val::Op(compare.operands[0]),
                            b: Val::Op(compare.operands[1]),
                            branch_block: block,
                        },
                        num_in,
                        num_out,
                        position,
                    });
                }
                // If the condition is a chain of ORs/AND and the successor only
                // has the current block as predecessor, queue conditions for
                // the successor.
                let join = unit
                    .defining(condition)
                    .and_then(
                        |(_, one)| match one.opcode {
                            Opcode::Binary(BinaryOp::And) => Some((false, one)),
                            Opcode::Binary(BinaryOp::Or) => Some((true, one)),
                            _ => None,
                        },
                    );
                if let Some((is_or, join)) = join {
                    let successor = if is_or { no } else { yes };
                    if can_add_successor(unit, block, successor)
                        && let Some(&(s_in, s_out)) = numbers.get(&cfg::id(successor))
                    {
                        let mut pending = vec![join.operands[1], join.operands[0]];
                        let mut seen = vec![join.operands[1], join.operands[0]];
                        while let Some(current) = pending.pop() {
                            let Some((_, one)) = unit.defining(current) else { continue };
                            match one.opcode {
                                Opcode::ICmp(predicate) => work.push(FactOrCheck {
                                    entry: Entry_::Fact {
                                        predicate: if is_or { predicate.inverse() } else { predicate },
                                        a: Val::Op(one.operands[0]),
                                        b: Val::Op(one.operands[1]),
                                    },
                                    num_in: s_in,
                                    num_out: s_out,
                                    position: 0,
                                }),
                                Opcode::Binary(op @ (BinaryOp::And | BinaryOp::Or))
                                    if (op == BinaryOp::Or) == is_or =>
                                {
                                    for operand in [one.operands[1], one.operands[0]] {
                                        if !seen.contains(&operand) {
                                            seen.push(operand);
                                            pending.push(operand);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    continue;
                }
                let Some((_, compare)) = unit.defining(condition) else { continue };
                let Opcode::ICmp(predicate) = compare.opcode else { continue };
                let (a, b) = (Val::Op(compare.operands[0]), Val::Op(compare.operands[1]));
                for (successor, predicate) in [(yes, predicate), (no, predicate.inverse())] {
                    if can_add_successor(unit, block, successor)
                        && let Some(&(s_in, s_out)) = numbers.get(&cfg::id(successor))
                    {
                        work.push(FactOrCheck {
                            entry: Entry_::Fact { predicate, a, b },
                            num_in: s_in,
                            num_out: s_out,
                            position: 0,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // `State::addInfoFor`: instructions whose flags may be strengthened,
    // checked at the closest point dominating all their uses.
    for &block in function.layout() {
        if !numbers.contains_key(&cfg::id(block)) {
            continue;
        }
        for &inst in function.block(block).instructions() {
            if can_strengthen_flags(unit, inst) {
                let (at, position) = common_dominator_of_uses(unit, inst, &numbers);
                let Some(&(num_in, num_out)) = numbers.get(&cfg::id(at)) else { continue };
                work.push(FactOrCheck { entry: Entry_::Strengthen { inst }, num_in, num_out, position });
            }
        }
    }

    // Next, sort worklist by dominance, so that dominating conditions to check
    // and facts come before conditions and facts dominated by them. If a
    // condition to check and a fact have the same numbers, conditional facts
    // come first. Also make sure conditions with constant operands come before
    // conditions without constant operands.
    let has_no_const_op = |one: &FactOrCheck| match &one.entry {
        Entry_::Fact { a, b, .. } | Entry_::Check { a, b, .. } => {
            ConstraintInfo::constant_static(unit, *a).is_none() && ConstraintInfo::constant_static(unit, *b).is_none()
        }
        Entry_::Strengthen { .. } => true,
    };
    work.sort_by(|a, b| {
        if a.num_in == b.num_in {
            if !a.is_check() && !b.is_check() {
                return has_no_const_op(a).cmp(&has_no_const_op(b));
            }
            if !a.is_check() {
                return std::cmp::Ordering::Less;
            }
            if !b.is_check() {
                return std::cmp::Ordering::Greater;
            }
            return a.position.cmp(&b.position);
        }
        a.num_in.cmp(&b.num_in)
    });

    // Finally, process ordered worklist and eliminate implied conditions.
    let mut stack: Vec<StackEntry> = Vec::new();
    let mut out = Vec::new();
    let mut flags: Vec<(InstId, Flags)> = Vec::new();
    for cb in &work {
        // First, pop entries from the stack that are out-of-scope for CB.
        // Remove the corresponding entry from the constraint system.
        while let Some(top) = stack.last() {
            debug_assert!(top.num_in <= cb.num_in);
            if cb.num_out <= top.num_out {
                break;
            }
            let entry = stack.pop().expect("an entry");
            info.remove_entry_from_stack(&entry);
        }

        match &cb.entry {
            // `tryToStrengthenFlags`.
            Entry_::Strengthen { inst } => {
                let instruction = function.instruction(*inst);
                let Some(result) = instruction.result else { continue };
                let value = Val::Op(Operand::Value(result));
                let mut add = Flags::default();
                if !instruction.flags.contains(Flags::NSW) && info.is_known_no_wrap(value, true) {
                    add.insert(Flags::NSW);
                }
                if !instruction.flags.contains(Flags::NUW) && info.is_known_no_wrap(value, false) {
                    add.insert(Flags::NUW);
                }
                if !add.is_empty() {
                    flags.push((*inst, add));
                }
            }
            Entry_::Check { predicate, a, b, branch_block } => {
                if let Some(implied) = info.check_condition(*predicate, *a, *b) {
                    out.push((*branch_block, implied));
                }
            }
            Entry_::Fact { predicate, a, b } => {
                if info.system(is_signed(*predicate)).size() > MAX_ROWS {
                    continue;
                }
                info.add_fact(*predicate, *a, *b, cb.num_in, cb.num_out, &mut stack);
                if matches!(
                    predicate,
                    IntPredicate::Ugt
                        | IntPredicate::Uge
                        | IntPredicate::Ult
                        | IntPredicate::Ule
                        | IntPredicate::Sgt
                        | IntPredicate::Sge
                        | IntPredicate::Slt
                        | IntPredicate::Sle
                ) {
                    info.transfer_to_other_system(*predicate, *a, *b, cb.num_in, cb.num_out, &mut stack);
                }
            }
        }
    }
    Outcome { decisions: out, flags }
}

impl<'u, 'a> ConstraintInfo<'u, 'a> {
    /// `isa<ConstantInt>` for the sort.
    fn constant_static(
        unit: &Unit,
        value: Val,
    ) -> Option<Int> {
        match value {
            Val::Int(number, width) => Some(Int { bits: number as u128, width }),
            Val::Op(operand) => Some(Int { bits: unit.int_constant(operand)?, width: unit.int_bits(operand)? }),
        }
    }
}

/// `ConstraintEliminationPass::run`, at its slot: the branches the constraint
/// system decides become jumps (the spike's stand-in for replacing the
/// compare's uses with a constant and letting SimplifyCFG fold the branch).
/// With `LLRM_POC_SLOT` set, one line a run: instructions, decisions,
/// instructions the walk took.
pub struct ConstraintElimination;

impl FunctionPass for ConstraintElimination {
    fn name(&self) -> &'static str {
        "constraint-elim"
    }

    fn reads(&self) -> Footprint {
        Footprint::CONTROL | Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let held = Held::of(unit.context, unit.layout, unit.function, analyses, false);
        let outcome = {
            let view = held.unit(unit.context, unit.layout, unit.function, &outer);
            let w0 = llrm_support::debug::work();
            let outcome = eliminate(&view);
            if std::env::var_os("LLRM_POC_SLOT").is_some() {
                eprintln!(
                    "POCSLOT constraint-elim insts={} decisions={} flags={} nsw={} nuw={} work={}",
                    unit.function.walk().count(),
                    outcome.decisions.len(),
                    outcome.flags.len(),
                    outcome.flags.iter().filter(|(_, add)| add.contains(Flags::NSW)).count(),
                    outcome.flags.iter().filter(|(_, add)| add.contains(Flags::NUW)).count(),
                    llrm_support::debug::work() - w0
                );
            }
            outcome
        };
        if outcome.decisions.is_empty() && outcome.flags.is_empty() {
            return PreservedAnalyses::all();
        }
        for (inst, add) in &outcome.flags {
            let mut flags = unit.function.instruction(*inst).flags;
            flags.insert(*add);
            unit.function.set_flags(*inst, flags);
        }
        if outcome.decisions.is_empty() {
            // Blocks and edges are as they were.
            return PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>();
        }
        for (block, way) in outcome.decisions {
            let last = unit.function.terminator(block).expect("a terminator");
            let operands = unit.function.instruction(last).operands.clone();
            let [_, Operand::Block(yes), Operand::Block(no)] = operands[..] else { continue };
            crate::decide::_jump(unit.function, last, if way { yes } else { no });
        }
        cfg::_unreachable(unit.context, unit.function);
        let _ = crate::transform::_trivial_phis(unit.function);
        PreservedAnalyses::none()
    }
}
