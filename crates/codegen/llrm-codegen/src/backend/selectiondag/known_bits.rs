//! Which bits of a value are known (the pin's
//! `SelectionDAG::computeKnownBits`), for the nodes the combiner's folds ask
//! about.
//!
//! # How it works
//! [`known_bits`] returns the bits known zero and known one. It walks the
//! operands to the pin's `MaxRecursionDepth` (6) and knows constants, a
//! `setcc` (0 or 1 under `ZeroOrOne` boolean contents), `and`/`or`/`xor`,
//! `select`, zero extension, truncation and a right shift by a constant;
//! anything else is unknown.
//!
//! # Runs
//! Called by the combiner's folds (`visitAND`). This is a subset of the
//! pin's list; the others are added when a fold needs them (SD202).
//!
//! # See also
//! LLVM's `SelectionDAG::computeKnownBits`
//! ([L3349](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L3349)).

use llrm_mir::knownbits::Bits;

use super::dag::{BooleanContents, SDValue, SelectionDAG};
use super::fold::mask;
use super::isd::ISD;

/// `MaxRecursionDepth`.
const MAX_DEPTH: u32 = 6;

/// Bits known zero and known one, at the value's width; no bit is both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownBits {
    pub zero: u128,
    pub one: u128,
    pub width: u32,
}

impl KnownBits {
    pub fn unknown(width: u32) -> KnownBits {
        KnownBits { zero: 0, one: 0, width }
    }

    pub fn constant(
        value: u128,
        width: u32,
    ) -> KnownBits {
        let all = mask(width);
        KnownBits { zero: !value & all, one: value & all, width }
    }

    /// The same bits as [`llrm_mir::knownbits::KnownBits`].
    fn to_mir(self) -> llrm_mir::knownbits::KnownBits {
        llrm_mir::knownbits::KnownBits::from_masks(self.width, self.zero, self.one)
    }

    /// The same bits as this module's [`KnownBits`].
    fn from_mir(known: llrm_mir::knownbits::KnownBits) -> KnownBits {
        KnownBits { zero: known.zero, one: known.one, width: known.width }
    }

    /// `KnownBits::isConstant`.
    pub fn is_constant(&self) -> bool {
        (self.zero | self.one) == mask(self.width)
    }

    pub fn intersect(
        self,
        other: KnownBits,
    ) -> KnownBits {
        KnownBits { zero: self.zero & other.zero, one: self.one & other.one, width: self.width }
    }

    /// `KnownBits::getMaxValue`.
    fn max_value(&self) -> u128 {
        !self.zero & mask(self.width)
    }

    /// `KnownBits::makeGE`: the bits `self` has when it is known to be at least
    /// `value`.
    fn make_ge(
        &self,
        value: u128,
    ) -> KnownBits {
        let all = mask(self.width);
        // The leading bit positions where the value is known to be less than or
        // equal to `value`.
        let leading = (self.zero | value) & all;
        let n = (leading << (128 - self.width)).leading_ones().min(self.width);
        let low = self.width - n;
        let masked = value & !mask(low) & all;
        KnownBits { zero: self.zero, one: self.one | masked, width: self.width }
    }

    /// `KnownBits::umax`.
    pub fn umax(
        self,
        other: KnownBits,
    ) -> KnownBits {
        if self.one >= other.max_value() {
            return self;
        }
        if other.one >= self.max_value() {
            return other;
        }
        self.make_ge(other.one).intersect(other.make_ge(self.one))
    }

    /// `KnownBits::umin`: `umax` of the flipped ranges.
    pub fn umin(
        self,
        other: KnownBits,
    ) -> KnownBits {
        let flip = |k: KnownBits| KnownBits { zero: k.one, one: k.zero, width: k.width };
        flip(flip(self).umax(flip(other)))
    }
}

/// `computeKnownBits`.
pub fn known_bits(
    dag: &SelectionDAG,
    value: SDValue,
) -> KnownBits {
    walk(dag, value, 0)
}

/// `computeKnownBits(Op, Depth)`: the walk starts `depth` operands down, so it
/// stops at `MaxRecursionDepth` sooner (the pin's callers pass their own depth).
pub fn known_bits_at(
    dag: &SelectionDAG,
    value: SDValue,
    depth: u32,
) -> KnownBits {
    walk(dag, value, depth)
}

#[cfg(test)]
thread_local! {
    /// How many values `known_bits` looked at.
    pub(super) static KNOWN_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// How many values `num_sign_bits` looked at (what a test counts to see the
    /// search is bounded by `MAX_DEPTH`).
    pub(super) static SIGN_BITS_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Applies a `KnownBits` operation to two walked operands.
fn binary(
    lhs: KnownBits,
    rhs: KnownBits,
    op: impl Fn(&Bits, &Bits) -> Bits,
) -> KnownBits {
    KnownBits::from_mir(op(&lhs.to_mir(), &rhs.to_mir()))
}

fn walk(
    dag: &SelectionDAG,
    value: SDValue,
    depth: u32,
) -> KnownBits {
    #[cfg(test)]
    KNOWN_VISITS.with(|visits| visits.set(visits.get() + 1));
    let vt = dag.value_type(value);
    if !vt.is_integer() {
        return KnownBits::unknown(1);
    }
    let width = vt.size_in_bits();
    let all = mask(width);
    if let Some(bits) = dag.constant(value) {
        return KnownBits::constant(bits, width);
    }
    if depth >= MAX_DEPTH {
        return KnownBits::unknown(width);
    }
    let operand = |at: usize| walk(dag, dag.operand(value, at), depth + 1);
    let opcode = dag.opcode(value);
    match opcode {
        ISD::SetCC if value.res == 0 => {
            // A compare is 0 or 1 where the target's boolean contents say so.
            let zero_or_one = dag.config().boolean_contents == BooleanContents::ZeroOrOne && width > 1;
            KnownBits { zero: if zero_or_one { all & !1 } else { 0 }, one: 0, width }
        }
        // `X86TargetLowering::computeKnownBitsForTargetNode`:
        // `Known.Zero.setBitsFrom(1)`.
        op if Some(op) == dag.config().nodes.flag_boolean && value.res == 0 => {
            KnownBits { zero: all & !1, one: 0, width }
        }
        ISD::And => {
            let (rhs, lhs) = (operand(1), operand(0));
            KnownBits { zero: lhs.zero | rhs.zero, one: lhs.one & rhs.one, width }
        }
        ISD::Or => {
            let (rhs, lhs) = (operand(1), operand(0));
            KnownBits { zero: lhs.zero & rhs.zero, one: lhs.one | rhs.one, width }
        }
        ISD::Xor => {
            let (rhs, lhs) = (operand(1), operand(0));
            KnownBits {
                zero: (lhs.zero & rhs.zero) | (lhs.one & rhs.one),
                one: (lhs.zero & rhs.one) | (lhs.one & rhs.zero),
                width,
            }
        }
        ISD::Select => operand(1).intersect(operand(2)),
        // `computeKnownBits` of the bit counts (SelectionDAG.cpp#L3921-L3955):
        // the count is at most what the operand's known bits leave possible,
        // so the bits above that count's width are zero.
        ISD::CtTz | ISD::CtTzZeroPoison => counted(width, operand(0).one.trailing_zeros().min(width)),
        ISD::CtLz | ISD::CtLzZeroPoison => counted(width, operand(0).one.leading_zeros().saturating_sub(128 - width)),
        ISD::CtPop => counted(width, width - operand(0).zero.count_ones().min(width)),
        // `Known.Zero.setBitsFrom(1)` (SelectionDAG.cpp#L3957).
        ISD::Parity => KnownBits { zero: all & !1, one: 0, width },
        ISD::UMax => operand(0).umax(operand(1)),
        ISD::UMin => operand(0).umin(operand(1)),
        // `AssertZext`: the bits at and above the asserted type are zero.
        ISD::AssertZext => {
            let inner = operand(0);
            let asserted = match dag.node(dag.operand(value, 1)).data {
                super::dag::NodeData::ValueType(asserted) => asserted.size_in_bits(),
                _ => width,
            };
            let zero = inner.zero | (all & !mask(asserted.min(width)));
            KnownBits { zero, one: inner.one & !zero, width }
        }
        // `computeKnownBits`, `BUILD_PAIR`: the low half's bits below the high half's.
        ISD::BuildPair if dag.config().known_bits_of_build_pair && width % 2 == 0 => {
            let (low, high) = (operand(0), operand(1));
            let half = width / 2;
            KnownBits {
                zero: (low.zero & mask(half)) | ((high.zero & mask(half)) << half),
                one: (low.one & mask(half)) | ((high.one & mask(half)) << half),
                width,
            }
        }
        ISD::ZeroExtend => {
            let inner = operand(0);
            // The new high bits are zero.
            KnownBits { zero: inner.zero | (all & !mask(inner.width)), one: inner.one, width }
        }
        // `computeKnownBitsForStackObjectPointer`: the low bits of an aligned
        // object's address are zero.
        ISD::FrameIndex | ISD::TargetFrameIndex => {
            let aligned = match dag.node(value).data {
                super::dag::NodeData::FrameIndex(index) => {
                    usize::try_from(index).ok().and_then(|at| dag.frame_alignments.get(at))
                }
                _ => None,
            };
            KnownBits { zero: aligned.map_or(0, |align| mask(align.trailing_zeros()) & all), one: 0, width }
        }
        ISD::Add => {
            // `KnownBits::computeForAddSub` without a carry in.
            let (lhs, rhs) = (operand(0), operand(1));
            let max = |k: KnownBits| !k.zero & all;
            let sum_zero = max(lhs).wrapping_add(max(rhs)).wrapping_add(0) & all;
            let sum_one = lhs.one.wrapping_add(rhs.one) & all;
            let carry_zero = !(sum_zero ^ lhs.zero ^ rhs.zero);
            let carry_one = sum_one ^ lhs.one ^ rhs.one;
            let known = (lhs.zero | lhs.one) & (rhs.zero | rhs.one) & (carry_zero | carry_one) & all;
            KnownBits { zero: !sum_zero & known, one: sum_one & known, width }
        }
        ISD::SignExtend => {
            let inner = operand(0);
            let high = all & !mask(inner.width);
            let sign = 1u128 << (inner.width - 1);
            // The new high bits copy the sign bit when it is known.
            KnownBits {
                zero: inner.zero | if inner.zero & sign != 0 { high } else { 0 },
                one: inner.one | if inner.one & sign != 0 { high } else { 0 },
                width,
            }
        }
        ISD::Truncate => {
            let inner = operand(0);
            KnownBits { zero: inner.zero & all, one: inner.one & all, width }
        }
        // The pin's `SHL`: `KnownBits::shl`, then the minimum shift amount's
        // low bits (which `KnownBits::shl` already sets). Node flags are not
        // modelled, so no-wrap is false.
        ISD::Shl => binary(operand(0), operand(1), |a, b| Bits::shl(a, b, false, false, b.is_non_zero())),
        // `computeKnownBits` of a `ZEXTLOAD`: the bits above the memory type.
        ISD::Load if value.res == 0 => match &dag.node(value).data {
            super::dag::NodeData::Mem(info) if info.ext == Some(super::actions::LoadExt::ZExtLoad) => {
                KnownBits { zero: all & !mask(info.mem_vt.size_in_bits()), one: 0, width }
            }
            _ => KnownBits::unknown(width),
        },
        ISD::AnyExtend => {
            let inner = operand(0);
            KnownBits { zero: inner.zero, one: inner.one, width }
        }
        ISD::Srl => binary(operand(0), operand(1), |a, b| Bits::lshr(a, b, false, false)),
        ISD::Sra => binary(operand(0), operand(1), |a, b| Bits::ashr(a, b, false, false)),
        ISD::Mul => {
            // The pin sets SelfMultiply only for a self-product that
            // `isGuaranteedNotToBeUndefOrPoison`; there is no such query here,
            // so it is never set.
            binary(operand(1), operand(0), |a, b| Bits::mul(a, b, false))
        }
        ISD::MulHU => binary(operand(1), operand(0), Bits::mulhu),
        ISD::MulHS => binary(operand(1), operand(0), Bits::mulhs),
        ISD::Sub => binary(operand(0), operand(1), |a, b| Bits::sub(a, b, false, false)),
        ISD::UDiv => binary(operand(0), operand(1), |a, b| Bits::udiv(a, b, false)),
        ISD::SDiv => binary(operand(0), operand(1), |a, b| Bits::sdiv(a, b, false)),
        ISD::URem => binary(operand(0), operand(1), Bits::urem),
        ISD::SRem => binary(operand(0), operand(1), Bits::srem),
        ISD::SMax => binary(operand(0), operand(1), Bits::smax),
        ISD::SMin => binary(operand(0), operand(1), Bits::smin),
        ISD::BSwap | ISD::BitReverse => {
            let inner = operand(0);
            let flip = |v: u128| {
                let flipped = if opcode == ISD::BSwap { v.swap_bytes() } else { v.reverse_bits() };
                flipped >> (128 - width)
            };
            KnownBits { zero: flip(inner.zero), one: flip(inner.one), width }
        }
        _ => dag.config().known_bits_for_target_node.and_then(|known| known(dag, value, &operand)).map_or_else(
            || KnownBits::unknown(width),
            |mut found| {
                found.width = width;
                found
            },
        ),
    }
}

/// `MaskedValueIsZero`: every bit of `bits` is known zero.
pub fn masked_value_is_zero(
    dag: &SelectionDAG,
    value: SDValue,
    bits: u128,
) -> bool {
    known_bits(dag, value).zero & bits == bits
}

/// `ComputeNumSignBits`, from the sign extension and the known bits.
pub fn num_sign_bits(
    dag: &SelectionDAG,
    value: SDValue,
) -> u32 {
    num_sign_bits_at(dag, value, 0)
}

/// `ComputeNumSignBits(Op, Depth)`: a value `MAX_DEPTH` operands down is not
/// looked at (the pin's `Depth >= MaxRecursionDepth`), and the known bits of
/// an operand are asked for at the depth below.
fn num_sign_bits_at(
    dag: &SelectionDAG,
    value: SDValue,
    depth: u32,
) -> u32 {
    #[cfg(test)]
    SIGN_BITS_VISITS.with(|visits| visits.set(visits.get() + 1));
    let width = dag.value_type(value).size_in_bits();
    if let Some(constant) = dag.constant(value) {
        // `Val.getNumSignBits()`, before the depth is looked at.
        let sign = 1u128 << (width - 1);
        let run = |bits: u128| (0..width).take_while(|i| bits & (sign >> i) != 0).count() as u32;
        return if constant & sign != 0 { run(constant) } else { run(!constant & mask(width)) }.max(1);
    }
    if depth >= MAX_DEPTH {
        return 1;
    }
    if dag.opcode(value) == ISD::SignExtend {
        let inner = dag.operand(value, 0);
        return num_sign_bits_at(dag, inner, depth + 1) + width - dag.value_type(inner).size_in_bits();
    }
    // `ComputeNumSignBits` of `AssertSext` (`VTBits - Bits + 1`) and
    // `AssertZext` (`VTBits - Bits`).
    if matches!(dag.opcode(value), ISD::AssertSext | ISD::AssertZext)
        && let super::dag::NodeData::ValueType(asserted) = dag.node(dag.operand(value, 1)).data
    {
        let extra = u32::from(dag.opcode(value) == ISD::AssertSext);
        return width - asserted.size_in_bits() + extra;
    }
    // `ComputeNumSignBits` of `SETCC`: all bits where it returns 0 or -1.
    if dag.opcode(value) == ISD::SetCC
        && value.res == 0
        && dag.config().boolean_contents == BooleanContents::ZeroOrNegativeOne
    {
        return width;
    }
    // `ComputeNumSignBits` of `SRA` by a constant: the operand's sign bits and
    // the amount, up to the width.
    if dag.opcode(value) == ISD::Sra
        && let Some(amount) = dag.constant(dag.operand(value, 1))
        && (0..u128::from(width)).contains(&amount)
    {
        return (num_sign_bits_at(dag, dag.operand(value, 0), depth + 1) + amount as u32).min(width);
    }
    if dag.opcode(value) == ISD::SignExtendInReg
        && let super::dag::NodeData::ValueType(inner) = dag.node(dag.operand(value, 1)).data
    {
        // `ComputeNumSignBits` of `SIGN_EXTEND_INREG`: the larger of what the
        // extension makes and what the operand has.
        let made = width - inner.size_in_bits() + 1;
        return made.max(num_sign_bits_at(dag, dag.operand(value, 0), depth + 1));
    }
    // `ComputeNumSignBits` of a `SEXTLOAD` (`VTBits - MemBits + 1`) and a
    // `ZEXTLOAD` (`VTBits - MemBits`).
    if dag.opcode(value) == ISD::Load
        && value.res == 0
        && let super::dag::NodeData::Mem(info) = &dag.node(value).data
    {
        match info.ext {
            Some(super::actions::LoadExt::SExtLoad) => return width - info.mem_vt.size_in_bits() + 1,
            Some(super::actions::LoadExt::ZExtLoad) => return width - info.mem_vt.size_in_bits(),
            _ => {}
        }
    }
    let signs = |index| num_sign_bits_at(dag, dag.operand(value, index), depth + 1);
    let mut first = 1;
    match dag.opcode(value) {
        // Logical operations keep the operands' sign bits at the worst; the
        // known bits below may do better.
        ISD::And | ISD::Or | ISD::Xor => {
            let left = signs(0);
            if left != 1 {
                first = left.min(signs(1));
            }
        }
        ISD::Select => {
            let (left, right) = (signs(1), signs(2));
            return if left == 1 { 1 } else { left.min(right) };
        }
        ISD::SelectCC => {
            let (left, right) = (signs(2), signs(3));
            return if left == 1 { 1 } else { left.min(right) };
        }
        ISD::UMin | ISD::UMax => {
            let left = signs(0);
            return if left == 1 { 1 } else { left.min(signs(1)) };
        }
        // The remainder is no larger than the dividend, and has its sign.
        ISD::SRem => return signs(0),
        ISD::Add => {
            let left = signs(0);
            if left == 1 {
                return 1;
            }
            // Decrementing: a 0/1 input gives 0/-1, a positive one no borrow.
            if dag.constant(dag.operand(value, 1)) == Some(mask(width)) {
                let known = walk(dag, dag.operand(value, 0), depth + 1);
                if (known.zero | 1) == mask(width) {
                    return width;
                }
                if known.zero & (1u128 << (width - 1)) != 0 {
                    return left;
                }
            }
            let right = signs(1);
            if right == 1 {
                return 1;
            }
            // One carry bit at most.
            return left.min(right) - 1;
        }
        ISD::Sub => {
            let right = signs(1);
            if right == 1 {
                return 1;
            }
            if dag.constant(dag.operand(value, 0)) == Some(0) {
                let known = walk(dag, dag.operand(value, 1), depth + 1);
                if (known.zero | 1) == mask(width) {
                    return width;
                }
                if known.zero & (1u128 << (width - 1)) != 0 {
                    return right;
                }
            }
            let left = signs(0);
            if left == 1 {
                return 1;
            }
            return left.min(right) - 1;
        }
        // The product has at most twice the operands' valid bits.
        ISD::Mul => {
            let left = signs(0);
            let right = if left == 1 { 1 } else { signs(1) };
            if left != 1 && right != 1 {
                let valid = (width - left + 1) + (width - right + 1);
                return if valid > width { 1 } else { width - valid + 1 };
            }
        }
        ISD::Truncate => {
            let source = dag.value_type(dag.operand(value, 0)).size_in_bits();
            let source_signs = signs(0);
            if source_signs > source - width {
                return source_signs - (source - width);
            }
        }
        _ => {}
    }
    // Finally what the known bits prove about the top bits.
    let known = walk(dag, value, depth);
    let sign = 1u128 << (width - 1);
    let run = |bits: u128| (0..width).take_while(|i| bits & (sign >> i) != 0).count() as u32;
    let proven = if known.zero & sign != 0 {
        run(known.zero)
    } else if known.one & sign != 0 {
        run(known.one)
    } else {
        1
    };
    first.max(proven)
}

/// `ComputeMaxSignificantBits`.
pub fn max_significant_bits(
    dag: &SelectionDAG,
    value: SDValue,
) -> u32 {
    dag.value_type(value).size_in_bits() - num_sign_bits(dag, value) + 1
}

/// A count of at most `most` in `width` bits: the bits from `bit_width(most)`
/// up are zero (`Known.Zero.setBitsFrom(bit_width(most))`).
fn counted(
    width: u32,
    most: u32,
) -> KnownBits {
    let used = 32 - most.leading_zeros();
    KnownBits { zero: mask(width) & !mask(used), one: 0, width }
}
