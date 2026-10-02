//! Port of `qbopt/frontend/raising_floats.py`: translate decoded floating
//! shapes into explicit evaluation semantics.

use crate::model::floating::{Format, Precision, Rounding, Semantics};
use crate::model::ir::{Loc, Operation};
use crate::model::mir::{Arg, Op, OpCode};

fn _format(width: u32, integer: bool) -> Option<Format> {
    if integer {
        return match width {
            2 => Some(Format::Signed16),
            4 => Some(Format::Signed32),
            8 => Some(Format::Signed64),
            _ => None,
        };
    }
    match width {
        4 => Some(Format::Binary32),
        8 => Some(Format::Binary64),
        10 => Some(Format::Extended80),
        _ => None,
    }
}

fn register_or_value(arg: &Arg) -> bool {
    match arg {
        Arg::Opaque(one) => matches!(one.machine_payload(), Some(Loc::St(_))),
        Arg::Held(one) => one.width == 10,
        _ => false,
    }
}

pub fn semantics(op: &Op) -> Option<Semantics> {
    let x87 = [Format::Extended80];
    let pair = [Format::Extended80, Format::Extended80];
    let name = op.name.as_str();
    match op.op {
        Some(OpCode::Operation(Operation::FloatLoad)) => {
            if name == "fild" && op.loads.is_empty() && op.stores.is_empty() && op.args.len() == 1 {
                let width = match &op.args[0] {
                    Arg::Held(one) => Some(one.width),
                    Arg::Const(one) => Some(one.width),
                    _ => None,
                };
                if let Some(source) = width.and_then(|width| _format(width, true)) {
                    return Some(Semantics::new([source], Format::Extended80, Precision::Exact, Rounding::None));
                }
            }
            if !matches!(name, "fld" | "fild") || op.loads.len() != 1 || !op.stores.is_empty() {
                return None;
            }
            let source = _format(op.loads[0].width, name == "fild")?;
            Some(Semantics::new([source], Format::Extended80, Precision::Exact, Rounding::None))
        }
        Some(OpCode::Operation(Operation::FloatStore)) => {
            if matches!(name, "fistp" | "fisttp") && op.stores.is_empty() && op.loads.is_empty() && op.results.len() == 1
            {
                if let Arg::Held(result) = &op.results[0] {
                    if let Some(target) = _format(result.width, true) {
                        let rounding = if name == "fisttp" { Rounding::TowardZero } else { Rounding::Dynamic };
                        return Some(Semantics::new(x87, target, Precision::Destination, rounding));
                    }
                }
            }
            if !matches!(name, "fstp" | "fistp" | "fisttp") || op.stores.len() != 1 || !op.loads.is_empty() {
                return None;
            }
            let target = _format(op.stores[0].width, name != "fstp")?;
            let rounding = match (name, target) {
                (_, Format::Extended80) => Rounding::None,
                ("fisttp", _) => Rounding::TowardZero,
                _ => Rounding::Dynamic,
            };
            Some(Semantics::new(x87, target, Precision::Destination, rounding))
        }
        Some(OpCode::Operation(Operation::FloatArith)) => {
            if matches!(name, "fadd" | "fsub" | "fmul" | "fdiv")
                && op.loads.is_empty()
                && op.stores.is_empty()
                && op.args.len() == 2
                && op.results.len() == 1
                && op.args.iter().chain(&op.results).all(register_or_value)
            {
                return Some(Semantics::new(pair, Format::Extended80, Precision::Dynamic, Rounding::Dynamic));
            }
            if !matches!(name, "fadd" | "fsub" | "fmul" | "fdiv" | "fidiv" | "fisub")
                || op.loads.len() != 1
                || !op.stores.is_empty()
            {
                return None;
            }
            let source = _format(op.loads[0].width, matches!(name, "fidiv" | "fisub"))?;
            Some(Semantics::new([Format::Extended80, source], Format::Extended80, Precision::Dynamic, Rounding::Dynamic))
        }
        Some(OpCode::Operation(Operation::FloatArithPop)) => {
            if !matches!(name, "faddp" | "fsubp" | "fmulp" | "fdivp") || !op.loads.is_empty() || !op.stores.is_empty() {
                return None;
            }
            Some(Semantics::new(pair, Format::Extended80, Precision::Dynamic, Rounding::Dynamic))
        }
        Some(OpCode::Operation(Operation::FloatUnary)) => {
            if !matches!(name, "fchs" | "fabs" | "fsqrt") || !op.loads.is_empty() || !op.stores.is_empty() {
                return None;
            }
            let exact = matches!(name, "fchs" | "fabs");
            Some(Semantics::new(
                x87,
                Format::Extended80,
                if exact { Precision::Exact } else { Precision::Dynamic },
                if exact { Rounding::None } else { Rounding::Dynamic },
            ))
        }
        _ => None,
    }
}
