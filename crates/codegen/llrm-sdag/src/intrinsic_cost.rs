//! What an integer intrinsic costs on a target, by whether the target's own
//! lowering table makes its operation legal: `BasicTTIImplBase::
//! getTypeBasedIntrinsicInstrCost`, which the pin's M68k runs
//! (`CodeGenTargetMachineImpl::getTargetTransformInfo` returns `BasicTTIImpl`;
//! M68k does not override it). Legal (or promoted) costs 1 per register the
//! type splits into, doubled where it splits; custom lowered, 2 per register;
//! an operation the target expands costs what the switch says: `SingleCallCost`
//! (10, or 1 under `TCK_CodeSize`) for the bit counts and the saturating
//! fixed-point and shift intrinsics, `TCC_Expensive` (4) for `ctpop`.
//! <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L2301>
//!
//! Not translated: the intrinsics the switch prices by a formula of other
//! costs when the operation is not legal (`smin`, `abs`, `fshl`, `sadd.sat`,
//! `smul.fix`, `ucmp`, `smulh` and the like). They have no answer here when
//! the target expands them; legal or custom, they are priced like the rest.

use super::actions::{Action, TargetLowering};
use super::isd::ISD;
use super::mvt::{EVT, MVT};

/// What the pin's switch does with an operation the target does not make
/// legal or custom.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Expansion {
    /// `SingleCallCost`: "turned into a library call, make it expensive".
    Call,
    /// `TCC_Expensive`, `ctpop`'s: "cheaper than a library call but still
    /// not a cheap instruction".
    Expensive,
    /// A formula over other costs (the min/max, saturating and funnel
    /// intrinsics); not translated.
    Formula,
}

/// `TCC_Expensive`.
const EXPENSIVE: i64 = 4;

/// The row of the integer intrinsic `name` in the pin's switch: its `ISD`
/// opcode (none where the switch leaves `ISD` at 0, so no legality is asked)
/// and what it costs expanded.
fn row_of(name: &str) -> Option<(Option<ISD>, Expansion)> {
    use Expansion::{Call, Expensive, Formula};
    // the longer name first: a row matches a name that continues with `.`
    const ROWS: [(&str, Option<ISD>, Expansion); 26] = [
        ("smul.fix.sat", None, Call),
        ("umul.fix.sat", None, Call),
        ("sshl.sat", None, Call),
        ("ushl.sat", None, Call),
        ("abs", Some(ISD::Abs), Formula),
        ("fshl", Some(ISD::FShl), Formula),
        ("fshr", Some(ISD::FShr), Formula),
        ("smax", Some(ISD::SMax), Formula),
        ("smin", Some(ISD::SMin), Formula),
        ("umax", Some(ISD::UMax), Formula),
        ("umin", Some(ISD::UMin), Formula),
        ("sadd.sat", Some(ISD::SAddSat), Formula),
        ("ssub.sat", Some(ISD::SSubSat), Formula),
        ("uadd.sat", Some(ISD::UAddSat), Formula),
        ("usub.sat", Some(ISD::USubSat), Formula),
        ("smul.fix", Some(ISD::SMulFix), Formula),
        ("umul.fix", Some(ISD::UMulFix), Formula),
        ("ctpop", Some(ISD::CtPop), Expensive),
        ("ctlz", Some(ISD::CtLz), Call),
        ("cttz", Some(ISD::CtTz), Call),
        ("bswap", Some(ISD::BSwap), Call),
        ("bitreverse", Some(ISD::BitReverse), Call),
        ("ucmp", Some(ISD::UCmp), Formula),
        ("scmp", Some(ISD::SCmp), Formula),
        ("smulh", Some(ISD::MulHS), Formula),
        ("umulh", Some(ISD::MulHU), Formula),
    ];
    let bare = name.strip_prefix("llvm.")?;
    ROWS.iter()
        .find(|(row, ..)| bare.strip_prefix(row).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
        .map(|(_, op, expansion)| (*op, *expansion))
}

/// The price of the integer intrinsic `name` of `bits` bits on `tli`, under
/// `TCK_CodeSize` when `code_size`: none for an intrinsic with no row, one the
/// target expands that the pin prices by a formula, or a width that is not 8
/// to 64 bits.
pub fn cost(
    tli: &TargetLowering,
    name: &str,
    bits: u32,
    code_size: bool,
) -> Option<i64> {
    let (op, expansion) = row_of(name)?;
    let (vt, parts) = match bits {
        1..=8 => (MVT::I8, 1),
        9..=16 => (MVT::I16, 1),
        17..=32 => (MVT::I32, 1),
        33..=64 => (MVT::I32, i64::from(bits.div_ceil(32))),
        _ => return None,
    };
    let action = op.map(|op| tli.operation_action(op, EVT::Simple(vt)));
    match action {
        Some(Action::Legal | Action::Promote) => Some(parts * if parts > 1 { 2 } else { 1 }),
        Some(Action::Custom) => Some(parts * 2),
        Some(Action::Expand | Action::LibCall) | None => match expansion {
            Expansion::Call => Some(if code_size { 1 } else { 10 }),
            Expansion::Expensive => Some(EXPENSIVE),
            Expansion::Formula => None,
        },
    }
}
