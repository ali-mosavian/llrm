//! Port of `tests/test_scalar_division.py`.
//!
//! Skipped, monkeypatching `raising_division.scalar` out of `mir.bodies`:
//! `test_stride_division_requires_the_dividends_sign_extension`.

use crate::model::mir::{Arg, Const, Kind, Op};
use crate::optimize::transform;

/// An all-ones divisor is -1, whose minimum-signed quotient can fault; it is
/// not safe to speculate.
#[test]
fn test_division_fault_gate_uses_the_operands_width() {
    for width in [2_u32, 4] {
        for divisor in [-1_i64, 0, 1, 5] {
            let masked = divisor & ((1_i64 << (8 * width)) - 1);
            let mut op = Op::new(1, None, "", Vec::new(), Vec::new());
            op.kind = Kind::Divmod;
            op.args = vec![Arg::Const(Const::new(0, width)), Arg::Const(Const::new(masked, width))];
            assert_eq!(transform::_cannot_fault(&op), !(divisor == -1 || divisor == 0), "{width} {divisor}");
        }
    }
}
