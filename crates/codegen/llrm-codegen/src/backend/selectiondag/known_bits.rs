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

use super::dag::{BooleanContents, SDValue, SelectionDAG};
use super::fold::mask;
use super::isd::ISD;
use super::x86_lowering::x86isd;

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

    /// `KnownBits::isConstant`.
    pub fn is_constant(&self) -> bool {
        (self.zero | self.one) == mask(self.width)
    }

    fn intersect(
        self,
        other: KnownBits,
    ) -> KnownBits {
        KnownBits { zero: self.zero & other.zero, one: self.one & other.one, width: self.width }
    }
}

/// `computeKnownBits`.
pub fn known_bits(
    dag: &SelectionDAG,
    value: SDValue,
) -> KnownBits {
    walk(dag, value, 0)
}

fn walk(
    dag: &SelectionDAG,
    value: SDValue,
    depth: u32,
) -> KnownBits {
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
        op if op == x86isd::SETCC && value.res == 0 => KnownBits { zero: all & !1, one: 0, width },
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
        ISD::ZeroExtend => {
            let inner = operand(0);
            // The new high bits are zero.
            KnownBits { zero: inner.zero | (all & !mask(inner.width)), one: inner.one, width }
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
        ISD::Shl => match dag.constant(dag.operand(value, 1)) {
            Some(by) if by < u128::from(width) => {
                let inner = operand(0);
                KnownBits { zero: ((inner.zero << by) | mask(by as u32)) & all, one: (inner.one << by) & all, width }
            }
            _ => KnownBits::unknown(width),
        },
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
        ISD::Srl => match dag.constant(dag.operand(value, 1)) {
            Some(by) if by < u128::from(width) => {
                let inner = operand(0);
                KnownBits {
                    zero: ((inner.zero >> by) | (all & !(all >> by))) & all,
                    one: (inner.one >> by) & all,
                    width,
                }
            }
            _ => KnownBits::unknown(width),
        },
        ISD::BSwap | ISD::BitReverse => {
            let inner = operand(0);
            let flip = |v: u128| {
                let flipped = if opcode == ISD::BSwap { v.swap_bytes() } else { v.reverse_bits() };
                flipped >> (128 - width)
            };
            KnownBits { zero: flip(inner.zero), one: flip(inner.one), width }
        }
        _ => KnownBits::unknown(width),
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
    let width = dag.value_type(value).size_in_bits();
    if dag.opcode(value) == ISD::SignExtend {
        let inner = dag.operand(value, 0);
        return num_sign_bits(dag, inner) + width - dag.value_type(inner).size_in_bits();
    }
    // `ComputeNumSignBits` of `AssertSext` (`VTBits - Bits + 1`) and
    // `AssertZext` (`VTBits - Bits`).
    if matches!(dag.opcode(value), ISD::AssertSext | ISD::AssertZext)
        && let super::dag::NodeData::ValueType(asserted) = dag.node(dag.operand(value, 1)).data
    {
        let extra = u32::from(dag.opcode(value) == ISD::AssertSext);
        return width - asserted.size_in_bits() + extra;
    }
    if dag.opcode(value) == ISD::SignExtendInReg
        && let super::dag::NodeData::ValueType(inner) = dag.node(dag.operand(value, 1)).data
    {
        // `ComputeNumSignBits` of `SIGN_EXTEND_INREG`: the larger of what the
        // extension makes and what the operand has.
        let made = width - inner.size_in_bits() + 1;
        return made.max(num_sign_bits(dag, dag.operand(value, 0)));
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
    let known = known_bits(dag, value);
    let sign = 1u128 << (width - 1);
    let run = |bits: u128| (0..width).take_while(|i| bits & (sign >> i) != 0).count() as u32;
    if known.zero & sign != 0 {
        run(known.zero)
    } else if known.one & sign != 0 {
        run(known.one)
    } else {
        1
    }
}

/// `ComputeMaxSignificantBits`.
pub fn max_significant_bits(
    dag: &SelectionDAG,
    value: SDValue,
) -> u32 {
    dag.value_type(value).size_in_bits() - num_sign_bits(dag, value) + 1
}
