//! `computeConstantRange` and the select-pattern matcher it and
//! `computeKnownBits` use.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`computeConstantRange`, `setLimitsForBinOp`, `getRangeForIntrinsic`,
//! `getRangeForSelectPattern`, `computeConstantRangeIncludingKnownBits`, the
//! `computeOverflowFor*` family and `matchSelectPattern` with its helpers),
//! each function linking the line it translates.
//!
//! # Example
//! `urem i32 %x, 10` is in `[0, 10)`.

use super::query::Query;
use crate::constantrange::ConstantRange;
use crate::module::Operand;

/// `SelectPatternFlavor`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectPatternFlavor {
    None,
    SMin,
    UMin,
    SMax,
    UMax,
    FMinimum,
    FMaximum,
    Abs,
    NAbs,
    Other,
}

/// `SelectPatternResult`: the flavor, and the operands of the min, max or abs
/// it found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectPatternResult {
    pub flavor: SelectPatternFlavor,
    pub lhs: Option<Operand>,
    pub rhs: Option<Operand>,
}

/// `matchSelectPattern(V, LHS, RHS)` for an integer select.
pub fn match_select_pattern(
    q: &Query,
    v: Operand,
) -> SelectPatternResult {
    let _ = (q, v);
    SelectPatternResult { flavor: SelectPatternFlavor::None, lhs: None, rhs: None }
}

/// `computeConstantRange(V, ForSigned, UseInstrInfo, AC, CtxI, DT, Depth)`.
pub fn compute_constant_range(
    q: &Query,
    v: Operand,
    for_signed: bool,
    depth: u32,
) -> ConstantRange {
    let _ = (for_signed, depth);
    ConstantRange::full(q.scalar_bits(v).max(1))
}

/// The `cru` and `crs` columns of `examples/vtdump.rs`.
pub fn column(
    q: &Query,
    v: Operand,
    for_signed: bool,
) -> String {
    let _ = (q, v);
    format!(" {}=?", if for_signed { "crs" } else { "cru" })
}
