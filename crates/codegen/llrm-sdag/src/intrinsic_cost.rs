//! What an integer intrinsic costs on a target, by whether the target's own
//! lowering table makes its operation legal: `BasicTTIImplBase::
//! getTypeBasedIntrinsicInstrCost` (BasicTTIImpl.h:2730-2830 in the pin). The
//! operation legal (or promoted) costs 1, 2 where the type is split; custom
//! lowered, twice that; otherwise the intrinsic is an expansion or a call,
//! `SingleCallCost`.
//!
//! DEVIATE (M68K-INTRINSIC-EXPANSION, owner m68k, review 2026-12-20): the
//! pin's `SingleCallCost` is 1 under `TCK_CodeSize`, the size of one call; the
//! loop unroller asks that kind, so a `smul.fix` that expands to dozens of
//! instructions read as one and a 3x3 loop of them unrolled (fixed_point -O2
//! 1644 -> 3604 B). Here an expansion costs the same under every kind. The
//! saturating fixed-point and shift intrinsics, which the pin's switch leaves
//! to its fallback `SingleCallCost`, have rows of their own. Only integer
//! intrinsics are priced; the float rows, and the intrinsics with no row,
//! keep the base class's price.

use super::actions::{Action, TargetLowering};
use super::isd::ISD;
use super::mvt::{EVT, MVT};

/// `SingleCallCost` of an expansion or a call, whatever the kind.
pub const EXPANSION: i64 = 10;
/// `ctpop`'s: "cheaper than a library call but still not a cheap instruction"
/// (`TCC_Expensive`).
pub const POPULATION_EXPANSION: i64 = 4;

/// The operation of the integer intrinsic `name` in the pin's switch.
pub fn operation_of(name: &str) -> Option<ISD> {
    // the longer name first: a row matches a name that continues with `.`
    const ROWS: [(&str, ISD); 26] = [
        ("smul.fix.sat", ISD::SMulFixSat),
        ("umul.fix.sat", ISD::UMulFixSat),
        ("sshl.sat", ISD::SShlSat),
        ("ushl.sat", ISD::UShlSat),
        ("abs", ISD::Abs),
        ("fshl", ISD::FShl),
        ("fshr", ISD::FShr),
        ("smax", ISD::SMax),
        ("smin", ISD::SMin),
        ("umax", ISD::UMax),
        ("umin", ISD::UMin),
        ("sadd.sat", ISD::SAddSat),
        ("ssub.sat", ISD::SSubSat),
        ("uadd.sat", ISD::UAddSat),
        ("usub.sat", ISD::USubSat),
        ("smul.fix", ISD::SMulFix),
        ("umul.fix", ISD::UMulFix),
        ("ctpop", ISD::CtPop),
        ("ctlz", ISD::CtLz),
        ("cttz", ISD::CtTz),
        ("bswap", ISD::BSwap),
        ("bitreverse", ISD::BitReverse),
        ("ucmp", ISD::UCmp),
        ("scmp", ISD::SCmp),
        ("smulh", ISD::MulHS),
        ("umulh", ISD::MulHU),
    ];
    let bare = name.strip_prefix("llvm.")?;
    ROWS.iter()
        .find(|(row, _)| bare.strip_prefix(row).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
        .map(|(_, op)| *op)
}

/// The price of the integer intrinsic `name` of `bits` bits on `tli`: none for
/// an intrinsic the pin's switch has no integer row for, or a width that is
/// not 8 to 64 bits.
pub fn cost(
    tli: &TargetLowering,
    name: &str,
    bits: u32,
) -> Option<i64> {
    let op = operation_of(name)?;
    let (vt, parts) = match bits {
        1..=8 => (MVT::I8, 1),
        9..=16 => (MVT::I16, 1),
        17..=32 => (MVT::I32, 1),
        33..=64 => (MVT::I32, i64::from(bits.div_ceil(32))),
        _ => return None,
    };
    Some(match tli.operation_action(op, EVT::Simple(vt)) {
        Action::Legal | Action::Promote => parts * if parts > 1 { 2 } else { 1 },
        Action::Custom => parts * 2,
        Action::Expand | Action::LibCall if op == ISD::CtPop => POPULATION_EXPANSION,
        Action::Expand | Action::LibCall => EXPANSION,
    })
}
