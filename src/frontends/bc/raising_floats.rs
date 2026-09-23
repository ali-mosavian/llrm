//! Port of `qbopt/frontend/raising_floats.py`: translate decoded floating
//! shapes into explicit evaluation semantics.

use crate::model::floating::{Format, Precision, Rounding, Semantics};
use crate::model::ir::{Loc, Operation};
use crate::model::mir::{Arg, Kind, Op, OpCode, RaisedBody};

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
            if !matches!(name, "fstp" | "fistp") || op.stores.len() != 1 || !op.loads.is_empty() {
                return None;
            }
            let target = _format(op.stores[0].width, name == "fistp")?;
            let rounding = if target == Format::Extended80 { Rounding::None } else { Rounding::Dynamic };
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

pub fn annotated(body: RaisedBody) -> RaisedBody {
    let annotated_op = |op: &Op| {
        if op.op == Some(OpCode::Operation(Operation::Nothing))
            && matches!(op.name.as_str(), "wait" | "fwait")
            && op.defines.is_empty()
            && op.uses.is_empty()
            && op.loads.is_empty()
            && op.stores.is_empty()
        {
            let mut op = op.clone();
            op.kind = Kind::Fcheck;
            op.name = String::new();
            return op;
        }
        let mut made = op.clone();
        made.floating = semantics(op);
        made
    };
    let blocks = body.blocks.iter().map(|block| block.with_ops(block.ops.iter().map(annotated_op).collect())).collect();
    body.with_blocks(blocks)
}

#[cfg(test)]
mod tests {
    //! The `raising_floats` tests of `tests/test_floating.py`.
    //!
    //! Skipped, needing `mir.bodies` and `tools/stages.py`:
    //! `test_dump_exposes_single_rounding`.
    use super::*;
    use crate::model::floating::Exceptions;
    use crate::model::mir::MemRef;

    /// The same four bytes mean signed integer for FILD, binary32 for FLD.
    #[test]
    fn test_conversion_formats_do_not_confuse_integer_and_real() {
        let cases: [(&str, u32, Option<Format>); 15] = [
            ("fld", 4, Some(Format::Binary32)),
            ("fld", 8, Some(Format::Binary64)),
            ("fld", 10, Some(Format::Extended80)),
            ("fild", 2, Some(Format::Signed16)),
            ("fild", 4, Some(Format::Signed32)),
            ("fild", 8, Some(Format::Signed64)),
            ("fstp", 4, Some(Format::Binary32)),
            ("fstp", 8, Some(Format::Binary64)),
            ("fstp", 10, Some(Format::Extended80)),
            ("fistp", 2, Some(Format::Signed16)),
            ("fistp", 4, Some(Format::Signed32)),
            ("fistp", 8, Some(Format::Signed64)),
            ("fld", 2, None),
            ("fild", 10, None),
            ("unknown", 4, None),
        ];
        for (name, width, expected) in cases {
            let store = matches!(name, "fstp" | "fistp");
            let reference = MemRef::new(None, width);
            let operation = if store { Operation::FloatStore } else { Operation::FloatLoad };
            let mut op = Op::new(0, OpCode::Operation(operation), name, vec![], vec![]);
            op.kind = if store { Kind::Fstore } else { Kind::Fload };
            if store {
                op.stores = vec![reference];
            } else {
                op.loads = vec![reference];
            }
            let rule = semantics(&op);
            let Some(expected) = expected else {
                assert!(rule.is_none(), "{name} {width}");
                continue;
            };
            let rule = rule.unwrap();
            assert_eq!(if store { rule.result } else { rule.inputs[0] }, expected);
            assert_eq!(rule.exceptions, Exceptions::Strict);
            let rounding = if store && width != 10 { Rounding::Dynamic } else { Rounding::None };
            assert_eq!(rule.rounding, rounding);
        }
    }

    #[test]
    fn test_unary_precision_is_explicit() {
        for (name, precision, rounding) in [
            ("fchs", Precision::Exact, Rounding::None),
            ("fabs", Precision::Exact, Rounding::None),
            ("fsqrt", Precision::Dynamic, Rounding::Dynamic),
        ] {
            let op = Op::new(0, OpCode::Operation(Operation::FloatUnary), name, vec![], vec![]);
            let rule = semantics(&op).unwrap();
            assert!(rule.precision == precision && rule.rounding == rounding);
            assert_eq!(rule.exceptions, Exceptions::Strict);
        }
    }
}
