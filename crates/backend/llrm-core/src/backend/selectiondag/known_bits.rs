//! Which bits of a value are known (the pin's `SelectionDAG::computeKnownBits`),
//! for the nodes the combiner's folds ask about.
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

    pub fn constant(value: u128, width: u32) -> KnownBits {
        let all = mask(width);
        KnownBits { zero: !value & all, one: value & all, width }
    }

    /// `KnownBits::isConstant`.
    pub fn is_constant(&self) -> bool {
        (self.zero | self.one) == mask(self.width)
    }

    fn intersect(self, other: KnownBits) -> KnownBits {
        KnownBits { zero: self.zero & other.zero, one: self.one & other.one, width: self.width }
    }
}

/// `computeKnownBits`.
pub fn known_bits(dag: &SelectionDAG, value: SDValue) -> KnownBits {
    walk(dag, value, 0)
}

fn walk(dag: &SelectionDAG, value: SDValue, depth: u32) -> KnownBits {
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
    match dag.opcode(value) {
        ISD::SetCC if value.res == 0 => {
            // A compare is 0 or 1 where the target's boolean contents say so.
            let zero_or_one = dag.config().boolean_contents == BooleanContents::ZeroOrOne && width > 1;
            KnownBits { zero: if zero_or_one { all & !1 } else { 0 }, one: 0, width }
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
            KnownBits { zero: (lhs.zero & rhs.zero) | (lhs.one & rhs.one), one: (lhs.zero & rhs.one) | (lhs.one & rhs.zero), width }
        }
        ISD::Select => operand(1).intersect(operand(2)),
        ISD::ZeroExtend => {
            let inner = operand(0);
            // The new high bits are zero.
            KnownBits { zero: inner.zero | (all & !mask(inner.width)), one: inner.one, width }
        }
        ISD::Truncate => {
            let inner = operand(0);
            KnownBits { zero: inner.zero & all, one: inner.one & all, width }
        }
        ISD::Srl => match dag.constant(dag.operand(value, 1)) {
            Some(by) if by < u128::from(width) => {
                let inner = operand(0);
                KnownBits { zero: ((inner.zero >> by) | (all & !(all >> by))) & all, one: (inner.one >> by) & all, width }
            }
            _ => KnownBits::unknown(width),
        },
        _ => KnownBits::unknown(width),
    }
}
