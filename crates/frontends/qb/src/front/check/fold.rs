//! What BC does with an operator on constants, found by running the three BCs
//! on every operator over the integer-range boundaries (`corpus/programs.py`,
//! group `fold`).
//!
//! A constant is *folded* when BC computes it at compile time, and then a value
//! outside the range of the place it is converted to is "Math overflow" there.
//! BC folds a sign, NOT and a conversion (CINT, CLNG, CSNG, CDBL) of a folded
//! constant, and arithmetic, `/`, `\`, MOD and the logical operators on two
//! folded constants when at least one is wider than INTEGER; two INTEGERs are
//! combined at run time (`x% = 32767 + 1` is BC's to accept), and so is `^`.

use super::*;

/// A constant expression's value: folded, or only known.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Value {
    pub(super) folded: Option<f64>,
    pub(super) known: Option<f64>,
}

impl Value {
    pub(super) fn none() -> Self {
        Self { folded: None, known: None }
    }

    fn of(
        value: f64,
        folded: bool,
    ) -> Self {
        if folded { Self { folded: Some(value), known: None } } else { Self { folded: None, known: Some(value) } }
    }
}

/// `-x`, `NOT x` and `+x` of a constant, which keep it folded or not.
pub(super) fn unary(
    op: Unary,
    operand: &Typed,
) -> Value {
    let apply = |c: f64| match op {
        Unary::Negative => -c,
        Unary::Not => !(c.round() as i64) as f64,
        _ => c,
    };
    match (operand.constant, operand.unfolded) {
        (Some(c), _) => Value::of(apply(c), true),
        (None, Some(c)) => Value::of(apply(c), false),
        _ => Value::none(),
    }
}

/// Whether a logical operator's operand fits the LONG it is converted to.
fn fits_long(value: f64) -> bool {
    (-2147483648.0..=2147483647.0).contains(&value.round())
}

/// `l op r` on constants: the value, and whether BC folds it. `None` when an
/// operand is no constant or the logical operator's operands do not fit a LONG
/// (the caller reports the overflow).
pub(super) fn binary(
    op: Binary,
    l: &Typed,
    r: &Typed,
) -> Result<Value, ()> {
    use Binary::*;
    let (Some(a), Some(b)) = (l.known(), r.known()) else { return Ok(Value::none()) };
    let folds = l.constant.is_some() && r.constant.is_some() && (l.ty != Ty::Int || r.ty != Ty::Int);
    let value = match op {
        Add => a + b,
        Subtract => a - b,
        Multiply => a * b,
        Divide => a / b,
        IntegerDivide => {
            let (a, b) = (a.round_ties_even(), b.round_ties_even());
            if b == 0.0 {
                return Ok(Value::none());
            }
            (a / b).trunc()
        }
        Modulo => {
            let (a, b) = (a.round_ties_even(), b.round_ties_even());
            if b == 0.0 {
                return Ok(Value::none());
            }
            a % b
        }
        And | Or | Xor | Eqv | Imp => {
            if folds && !(fits_long(a) && fits_long(b)) {
                return Err(());
            }
            let (x, y) = (a.round() as i64, b.round() as i64);
            (match op {
                And => x & y,
                Or => x | y,
                Xor => x ^ y,
                Eqv => !(x ^ y),
                _ => !x | y,
            }) as f64
        }
        Power => return Ok(Value::of(a.powf(b), false)),
        _ => return Ok(Value::none()),
    };
    Ok(Value::of(value, folds))
}

/// CINT, CLNG, CSNG or CDBL of a folded constant: its value in the new type.
pub(super) fn conversion(
    function: &str,
    argument: &Typed,
) -> Option<f64> {
    let value = argument.constant?;
    match function {
        "CINT" | "CLNG" => Some(value.round()),
        "CSNG" => Some(f64::from(value as f32)),
        "CDBL" => Some(value),
        _ => None,
    }
}
