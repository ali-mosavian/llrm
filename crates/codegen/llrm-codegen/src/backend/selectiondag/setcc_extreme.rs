//! `SimplifySetCC` against the smallest or largest value of the compare's
//! signedness
//! ([L5321 to
//! L5410](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L5321)):
//! `x u>= 0` and `x u<= MAX` are true, `x u< 0` and `x u> MAX` false; `x > MIN`
//! and `x < MAX` are `!=`, `x < MIN+1` and (unsigned) `x > MAX-1` are `==`.
//! After type legalization this folds the half of a wide compare that cannot
//! decide it.

use super::dag::{SDValue, SelectionDAG};
use super::isd::CondCode;
use super::mvt::EVT;

/// The simpler `setcc vt n0, c1, cond`, or `None`. `c1` is the constant, or
/// the splat of a vector compare; the folds to `!=` and `==` are not made for
/// a vector once its operations are legalized (`before_legalize_ops`).
pub fn fold_against_an_extreme(
    dag: &mut SelectionDAG,
    vt: EVT,
    n0: SDValue,
    c1: u128,
    cond: CondCode,
    before_legalize_ops: bool,
) -> Option<SDValue> {
    let op_type = dag.value_type(n0);
    let vector = op_type.is_vector();
    let bits = if vector { op_type.element().size_in_bits() } else { op_type.size_in_bits() };
    if bits < 2 {
        return None;
    }
    let mask = super::fold::mask(bits);
    let (unsigned, less, strict) = match cond {
        CondCode::SetUGe => (true, false, false),
        CondCode::SetGe => (false, false, false),
        CondCode::SetULe => (true, true, false),
        CondCode::SetLe => (false, true, false),
        CondCode::SetULt => (true, true, true),
        CondCode::SetLt => (false, true, true),
        CondCode::SetUGt => (true, false, true),
        CondCode::SetGt => (false, false, true),
        _ => return None,
    };
    let (min, max) = if unsigned { (0, mask) } else { (1u128 << (bits - 1), mask >> 1) };
    let c1 = c1 & mask;
    let mut same = |value: u128, cond: CondCode| {
        let rhs = dag.get_constant(value, op_type);
        Some(dag.get_setcc(vt, n0, rhs, cond))
    };
    match (less, strict) {
        // x >= MIN, x <= MAX
        (false, false) if c1 == min => Some(dag.get_bool_constant(true, vt)),
        (true, false) if c1 == max => Some(dag.get_bool_constant(true, vt)),
        // x < MIN is false; x < MAX is x != MAX; x < MIN+1 is x == MIN
        (true, true) if c1 == min => Some(dag.get_bool_constant(false, vt)),
        (true, true) if c1 == max => same(c1, CondCode::SetNe),
        (true, true) if c1 == min + 1 => same(min, CondCode::SetEq),
        // x > MAX is false; x > MIN is x != MIN; unsigned x > MAX-1 is x == MAX
        (false, true) if c1 == max => Some(dag.get_bool_constant(false, vt)),
        (false, true) if c1 == min && (!vector || before_legalize_ops) => same(c1, CondCode::SetNe),
        (false, true) if unsigned && c1 == max - 1 && (!vector || before_legalize_ops) => same(max, CondCode::SetEq),
        _ => None,
    }
}
