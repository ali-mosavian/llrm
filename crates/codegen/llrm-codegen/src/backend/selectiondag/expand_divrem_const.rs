//! `TargetLowering::expandDIVREMByConstant`
//! ([L8638](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L8638)):
//! an unsigned divide or remainder of a value twice a legal width by a
//! constant, worked out in halves instead of a library call. Two ways, in this
//! order: the remainder by summing chunks of the dividend
//! (`expandUDIVREMByConstantViaUREMDecomposition`, L8260; the quotient from it
//! by the divisor's inverse), and the multiplication by the magic number
//! (`expandUDIVREMByConstantViaUMulHiMagic`, L8541) over `expandMUL_LOHI`
//! (L8046).
//!
//! `ExpandIntRes_UDIV` and `ExpandIntRes_UREM` try it before the call, and not
//! when the function is tuned for size ("prefer the smaller libcall"). Signed
//! division is a call, as in the pin.
//!
//! DEVIATE SD106: `expandMUL_LOHI` is translated for the multiplications this
//! asks of it (`UMUL_LOHI` and `MUL` of two half pairs); the shortcut for
//! sign-extended inputs of `MUL` is not (`ComputeMaxSignificantBits`), and the
//! `ADDC/ADDE` form is never taken (`i64` has no such node). A UREM whose
//! library call is missing is not decomposed (`CanDecomposeUREMWithoutMulHi`:
//! the target has the call). Exit: none. Owner: w9.

use super::actions::TargetLowering;
use super::dag::{SDValue, SelectionDAG};
use super::div_const::unsigned_magic;
use super::isd::ISD;
use super::known_bits::known_bits;
use super::mvt::EVT;

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `UDIV`/`UREM` of `node` by a constant, as halves: the low and high half of
/// the quotient or remainder; none where the pin falls back to the call.
pub fn expand_divrem_by_constant(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    node: SDValue,
    half: EVT,
    ll: SDValue,
    lh: SDValue,
) -> Option<(SDValue, SDValue)> {
    if !matches!(dag.opcode(node), ISD::UDiv | ISD::URem) {
        return None;
    }
    let divisor = dag.constant(dag.operand(node, 1))?;
    // The halves' multiply must exist.
    if !tli.is_operation_legal_or_custom(ISD::MulHU, half) && !tli.is_operation_legal_or_custom(ISD::UMulLoHi, half) {
        return None;
    }
    // `shouldOptForSize`: the smaller call. `int_div_cheap` is set from
    // `cpu.size || cpu.min_size`, the same function attributes.
    if dag.config().int_div_cheap {
        return None;
    }
    if divisor <= 1 {
        return None;
    }
    via_urem_decomposition(dag, tli, node, divisor, half, ll, lh)
        .or_else(|| via_umulhi_magic(dag, tli, node, divisor, half, ll, lh))
}

/// Shift a pair right by `amount` bits (`ShiftRight` of the decomposition).
fn shift_right(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    half: EVT,
    lo: SDValue,
    hi: SDValue,
    amount: u32,
) -> (SDValue, SDValue) {
    let bits = half.size_in_bits();
    let zero = dag.get_constant(0, half);
    if amount == 0 {
        return (lo, hi);
    }
    if amount < bits {
        let by = dag.get_constant(u128::from(amount), EVT::I8);
        let low = if tli.is_operation_legal(ISD::FShr, half) {
            dag.get_node(ISD::FShr, half, &[hi, lo, by])
        } else {
            let down = dag.get_node(ISD::Srl, half, &[lo, by]);
            let back = dag.get_constant(u128::from(bits - amount), EVT::I8);
            let up = dag.get_node(ISD::Shl, half, &[hi, back]);
            dag.get_node(ISD::Or, half, &[down, up])
        };
        (low, dag.get_node(ISD::Srl, half, &[hi, by]))
    } else if amount == bits {
        (hi, zero)
    } else {
        let by = dag.get_constant(u128::from(amount - bits), EVT::I8);
        (dag.get_node(ISD::Srl, half, &[hi, by]), zero)
    }
}

/// `GetFSHR` of the decomposition: `fshr(hi, lo, amount)` for `0 < amount <
/// bits`.
fn fshr(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    half: EVT,
    lo: SDValue,
    hi: SDValue,
    amount: u32,
) -> SDValue {
    let bits = half.size_in_bits();
    let by = dag.get_constant(u128::from(amount), EVT::I8);
    if tli.is_operation_legal(ISD::FShr, half) {
        return dag.get_node(ISD::FShr, half, &[hi, lo, by]);
    }
    let down = dag.get_node(ISD::Srl, half, &[lo, by]);
    let back = dag.get_constant(u128::from(bits - amount), EVT::I8);
    let up = dag.get_node(ISD::Shl, half, &[hi, back]);
    dag.get_node(ISD::Or, half, &[down, up])
}

/// The multiplicative inverse of the odd `d` modulo `2^bits`.
fn inverse(
    d: u128,
    bits: u32,
) -> u128 {
    let m = mask(bits);
    let mut x = d & m;
    for _ in 0..7 {
        x = x.wrapping_mul(2u128.wrapping_sub(d.wrapping_mul(x))) & m;
    }
    x
}

/// `expandUDIVREMByConstantViaUREMDecomposition`.
fn via_urem_decomposition(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    node: SDValue,
    mut divisor: u128,
    half: EVT,
    mut ll: SDValue,
    mut lh: SDValue,
) -> Option<(SDValue, SDValue)> {
    let rem_only = dag.opcode(node) == ISD::URem;
    let vt = dag.value_type(node);
    let bits = vt.size_in_bits();
    let hbits = half.size_in_bits();

    // An even divisor is shifted until it is odd.
    let mut trailing = 0;
    if divisor & 1 == 0 {
        trailing = divisor.trailing_zeros();
        divisor >>= trailing;
    }
    // After that it must be less than `1 << hbits`.
    if divisor >= 1u128 << hbits {
        return None;
    }
    // The largest chunk width `w` with `(1 << w) % divisor` 1 or -1.
    let (mut best, mut alt) = (0, 0);
    let mut width = hbits;
    while width > hbits / 2 {
        if width != hbits - 1 {
            let remainder = (1u128 << width) % divisor;
            if remainder == 1 {
                best = width;
                break;
            }
            if width != hbits && remainder == divisor - 1 {
                alt = width;
            }
        }
        width -= 1;
    }
    let mut alternate = false;
    if best == 0 {
        if alt == 0 {
            return None;
        }
        alternate = true;
        best = alt;
    }

    let zero = dag.get_constant(0, half);
    // The bits the shift would drop, which the remainder needs back.
    let (mut partial_lo, mut partial_hi) = (zero, zero);
    if trailing != 0 && rem_only {
        if trailing < hbits {
            let low_mask = dag.get_constant(mask(trailing), half);
            partial_lo = dag.get_node(ISD::And, half, &[ll, low_mask]);
        } else if trailing == hbits {
            partial_lo = ll;
        } else {
            partial_lo = ll;
            let high_mask = dag.get_constant(mask(trailing - hbits), half);
            partial_hi = dag.get_node(ISD::And, half, &[lh, high_mask]);
        }
    }

    let bool_vt = tli.types.set_cc_result_type(half);
    let sum;
    if best == hbits {
        debug_assert!(!alternate);
        let (low, high) = shift_right(dag, tli, half, ll, lh, trailing);
        ll = low;
        lh = high;
        if tli.is_operation_legal_or_custom(ISD::UAddOCarry, half) {
            let first = dag.get_node_multi(ISD::UAddO, &[half, bool_vt], &[ll, lh]);
            let carried = dag.get_node_multi(
                ISD::UAddOCarry,
                &[half, bool_vt],
                &[SDValue { node: first.node, res: 0 }, zero, SDValue { node: first.node, res: 1 }],
            );
            sum = SDValue { node: carried.node, res: 0 };
        } else {
            let plain = dag.get_node(ISD::Add, half, &[ll, lh]);
            let wrapped = dag.get_setcc(bool_vt, plain, ll, super::isd::CondCode::SetULt);
            let carry = dag.get_zext_or_trunc(wrapped, half);
            sum = dag.get_node(ISD::Add, half, &[plain, carry]);
        }
    } else {
        // Chunks of `best` bits added together.
        let chunk_mask = dag.get_constant(mask(best), half);
        let mut total: Option<SDValue> = None;
        let mut at = 0;
        while at < bits - trailing {
            let shift = at + trailing;
            let mut chunk = if shift == 0 {
                ll
            } else if shift >= hbits {
                let by = dag.get_constant(u128::from(shift - hbits), EVT::I8);
                dag.get_node(ISD::Srl, half, &[lh, by])
            } else {
                fshr(dag, tli, half, ll, lh, shift)
            };
            // The last chunk needs no mask.
            if at + best < bits - trailing {
                chunk = dag.get_node(ISD::And, half, &[chunk, chunk_mask]);
            }
            total = Some(match total {
                None => chunk,
                Some(sum) => {
                    // An alternate subtracts the odd chunks.
                    let number = at / best;
                    let op = if alternate && number % 2 != 0 { ISD::Sub } else { ISD::Add };
                    dag.get_node(op, half, &[sum, chunk])
                }
            });
            at += best;
        }
        let mut total = total?;
        // The sum of an alternate may be negative: add a multiple of the
        // divisor, `2^best + 1`, once or twice.
        if alternate {
            let chunks = (bits - trailing).div_ceil(best);
            debug_assert!(chunks <= 4);
            let mut adjust = (1u128 << best) | 1;
            if chunks == 4 {
                adjust <<= 1;
            }
            let adjust = dag.get_constant(adjust & mask(hbits), half);
            total = dag.get_node(ISD::Add, half, &[total, adjust]);
        }
        sum = total;
    }

    // The remainder of the sum by the divisor, in the half type.
    let truncated = dag.get_constant(divisor & mask(hbits), half);
    let mut rem_lo = dag.get_node(ISD::URem, half, &[sum, truncated]);
    let mut rem_hi = zero;

    let result = if !rem_only {
        // Where the dividend was not shifted yet, now.
        if best != hbits {
            let (low, high) = shift_right(dag, tli, half, ll, lh, trailing);
            ll = low;
            lh = high;
        }
        // The dividend less the remainder is a multiple: times the inverse.
        let dividend = dag.get_node(ISD::BuildPair, vt, &[ll, lh]);
        let rem = dag.get_node(ISD::BuildPair, vt, &[rem_lo, rem_hi]);
        let exact = dag.get_node(ISD::Sub, vt, &[dividend, rem]);
        let factor = dag.get_constant(inverse(divisor, bits), vt);
        let quotient = dag.get_node(ISD::Mul, vt, &[exact, factor]);
        split(dag, quotient, half)
    } else {
        // The remainder of the shifted dividend, shifted back with the bits it
        // dropped.
        if trailing != 0 {
            if trailing < hbits {
                let back = dag.get_constant(u128::from(hbits - trailing), EVT::I8);
                rem_hi = dag.get_node(ISD::Srl, half, &[rem_lo, back]);
                let up = dag.get_constant(u128::from(trailing), EVT::I8);
                let shifted = dag.get_node(ISD::Shl, half, &[rem_lo, up]);
                rem_lo = dag.get_node(ISD::Or, half, &[shifted, partial_lo]);
            } else if trailing == hbits {
                rem_hi = rem_lo;
                rem_lo = partial_lo;
            } else {
                let up = dag.get_constant(u128::from(trailing - hbits), EVT::I8);
                let shifted = dag.get_node(ISD::Shl, half, &[rem_lo, up]);
                rem_hi = dag.get_node(ISD::Or, half, &[shifted, partial_hi]);
                rem_lo = partial_lo;
            }
        }
        (rem_lo, rem_hi)
    };
    Some(result)
}

/// `SplitScalar`: the low and high half of `value`.
fn split(
    dag: &mut SelectionDAG,
    value: SDValue,
    half: EVT,
) -> (SDValue, SDValue) {
    let vt = dag.value_type(value);
    let low = dag.get_node(ISD::Truncate, half, &[value]);
    let by = dag.get_constant(u128::from(half.size_in_bits()), EVT::I8);
    let high = dag.get_node(ISD::Srl, vt, &[value, by]);
    (low, dag.get_node(ISD::Truncate, half, &[high]))
}

/// `expandMUL_LOHI` for `UMUL_LOHI` (four results: the product's quarters,
/// lowest first) or `MUL` (two), of `(ll, lh)` by `(rl, rh)`.
fn mul_lohi(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    half: EVT,
    full_product: bool,
    (ll, lh): (SDValue, SDValue),
    (rl, rh): (SDValue, SDValue),
) -> Option<Vec<SDValue>> {
    let has_lohi = tli.is_operation_legal_or_custom(ISD::UMulLoHi, half);
    let has_mulhu = tli.is_operation_legal_or_custom(ISD::MulHU, half);
    if !has_lohi && !has_mulhu {
        return None;
    }
    let make = |dag: &mut SelectionDAG, l: SDValue, r: SDValue| -> (SDValue, SDValue) {
        if has_lohi {
            let both = dag.get_node_multi(ISD::UMulLoHi, &[half, half], &[l, r]);
            (SDValue { node: both.node, res: 0 }, SDValue { node: both.node, res: 1 })
        } else {
            (dag.get_node(ISD::Mul, half, &[l, r]), dag.get_node(ISD::MulHU, half, &[l, r]))
        }
    };
    let hbits = half.size_in_bits();
    let high_zero = |dag: &SelectionDAG, value: SDValue| known_bits(dag, value).zero & mask(hbits) == mask(hbits);
    let zero = dag.get_constant(0, half);
    // Both inputs zero-extended: one product.
    if high_zero(dag, lh) && high_zero(dag, rh) {
        let (low, high) = make(dag, ll, rl);
        return Some(if full_product { vec![low, high, zero, zero] } else { vec![low, high] });
    }
    let (low, mut high) = make(dag, ll, rl);
    if !full_product {
        let across = dag.get_node(ISD::Mul, half, &[ll, rh]);
        let down = dag.get_node(ISD::Mul, half, &[lh, rl]);
        high = dag.get_node(ISD::Add, half, &[high, across]);
        high = dag.get_node(ISD::Add, half, &[high, down]);
        return Some(vec![low, high]);
    }
    let bool_vt = tli.types.set_cc_result_type(half);
    // `a + b` and its carry out, as a half.
    let add = |dag: &mut SelectionDAG, a: SDValue, b: SDValue| -> (SDValue, SDValue) {
        let sum = dag.get_node_multi(ISD::UAddO, &[half, bool_vt], &[a, b]);
        let carry = dag.get_zext_or_trunc(SDValue { node: sum.node, res: 1 }, half);
        (SDValue { node: sum.node, res: 0 }, carry)
    };
    let (lo1, hi1) = make(dag, ll, rh);
    let (lo2, hi2) = make(dag, lh, rl);
    let (lo3, hi3) = make(dag, lh, rh);
    let (mid, c0) = add(dag, high, lo1);
    let (mid, c1) = add(dag, mid, lo2);
    let (upper, c2) = add(dag, hi1, hi2);
    let (upper, c3) = add(dag, upper, lo3);
    let (upper, c4) = add(dag, upper, c0);
    let (upper, c5) = add(dag, upper, c1);
    let carries = dag.get_node(ISD::Add, half, &[c2, c3]);
    let carries = dag.get_node(ISD::Add, half, &[carries, c4]);
    let carries = dag.get_node(ISD::Add, half, &[carries, c5]);
    let top = dag.get_node(ISD::Add, half, &[hi3, carries]);
    Some(vec![low, mid, upper, top])
}

/// `expandUDIVREMByConstantViaUMulHiMagic`.
fn via_umulhi_magic(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    node: SDValue,
    divisor: u128,
    half: EVT,
    ll: SDValue,
    lh: SDValue,
) -> Option<(SDValue, SDValue)> {
    let opcode = dag.opcode(node);
    let n0 = dag.operand(node, 0);
    let vt = dag.value_type(n0);
    let bits = vt.size_in_bits();
    let hbits = half.size_in_bits();
    let bool_vt = tli.types.set_cc_result_type(half);

    // Known leading zeros of the dividend may shrink the multiplier.
    let known = known_bits(dag, n0).zero & mask(bits);
    let known_leading = (0..bits).rev().take_while(|bit| known >> bit & 1 == 1).count() as u32;
    let divisor_leading = bits - (128 - divisor.leading_zeros());
    let magic = unsigned_magic(divisor, bits, known_leading.min(divisor_leading), true, false);

    // A pair shifted right (`MakeSRLLong`).
    let srl_long = |dag: &mut SelectionDAG, lo: SDValue, hi: SDValue, shift: u32| -> (SDValue, SDValue) {
        if shift < hbits {
            let by = dag.get_constant(u128::from(shift), EVT::I8);
            (dag.get_node(ISD::FShr, half, &[hi, lo, by]), dag.get_node(ISD::Srl, half, &[hi, by]))
        } else {
            let zero = dag.get_constant(0, half);
            if shift == hbits {
                (hi, zero)
            } else {
                let by = dag.get_constant(u128::from(shift - hbits), EVT::I8);
                (dag.get_node(ISD::Srl, half, &[hi, by]), zero)
            }
        }
    };
    // A pair added or subtracted (`MakeAddSubLong`).
    let add_sub_long = |dag: &mut SelectionDAG, sub: bool, a: (SDValue, SDValue), b: (SDValue, SDValue)| {
        let low = dag.get_node_multi(if sub { ISD::USubO } else { ISD::UAddO }, &[half, bool_vt], &[a.0, b.0]);
        let carried = dag.get_node_multi(
            if sub { ISD::USubOCarry } else { ISD::UAddOCarry },
            &[half, bool_vt],
            &[a.1, b.1, SDValue { node: low.node, res: 1 }],
        );
        (SDValue { node: low.node, res: 0 }, SDValue { node: carried.node, res: 0 })
    };
    let constant_halves = |dag: &mut SelectionDAG, value: u128| {
        (dag.get_constant(value & mask(hbits), half), dag.get_constant((value >> hbits) & mask(hbits), half))
    };

    let (mut ql, mut qh) = (ll, lh);
    if magic.pre_shift != 0 {
        (ql, qh) = srl_long(dag, ql, qh, magic.pre_shift);
    }
    let multiplier = constant_halves(dag, magic.magic);
    let product = mul_lohi(dag, tli, half, true, (ql, qh), multiplier)?;
    (ql, qh) = (product[2], product[3]);
    if magic.is_add {
        let np = add_sub_long(dag, true, (ll, lh), (ql, qh));
        let np = srl_long(dag, np.0, np.1, 1);
        (ql, qh) = add_sub_long(dag, false, np, (ql, qh));
    }
    if magic.post_shift != 0 {
        (ql, qh) = srl_long(dag, ql, qh, magic.post_shift);
    }
    if opcode != ISD::URem {
        return Some((ql, qh));
    }
    // The remainder: the dividend less the quotient times the divisor.
    let factor = constant_halves(dag, divisor);
    let times = mul_lohi(dag, tli, half, false, (ql, qh), factor)?;
    Some(add_sub_long(dag, true, (ll, lh), (times[0], times[1])))
}
