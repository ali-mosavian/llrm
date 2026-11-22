//! `LowerShift` for the 128-bit integer vectors on SSE2 ([L31392][pin]), with
//! `LowerShiftByScalarImmediate` and `LowerShiftByScalarVariable`. A shift the
//! target has no instruction for is built from the ones it has, instead of one
//! scalar shift a lane.
//!
//! DEVIATE X86V5 (owner llrm-target-x86-m64-b, exit 2026-11-27): there is no
//! legal float vector, so a variable `v4i32`/`v8i16` left shift is not the
//! `cvttps2dq` scale multiply (L31337) but the per-lane and the `pcmpgt`
//! ladder; a lane shuffle is `punpck*`/`pshufd` (the vector shuffle is not
//! selected), not `shufps`; the `v16i8` splat of a variable amount multiplies
//! by `0x0101` where the pin shuffles; a signed byte shift by one is not
//! `pavgb`; the SWAR widening of constant pairs and the `psraq` emulation by
//! a constant take the generic paths.
//!
//! [pin]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L31392

use llrm_codegen::backend::selectiondag::dag::{SDValue, SelectionDAG};
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::known_bits::{known_bits, masked_value_is_zero};
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};

use super::x86_lowering::{reinterpreted, scalar_in_lane0, v128, x86isd};

fn lane_bits(vt: EVT) -> u128 {
    u128::from(vt.element().size_in_bits())
}

/// `getTargetVShiftUniformOpcode(Opc, false)`.
fn by_immediate(opcode: ISD) -> ISD {
    match opcode {
        ISD::Shl => x86isd::VSHLI,
        ISD::Srl => x86isd::VSRLI,
        _ => x86isd::VSRAI,
    }
}

/// `getTargetVShiftUniformOpcode(Opc, true)`: the amount is the low quadword
/// of a register.
fn by_register(opcode: ISD) -> ISD {
    match opcode {
        ISD::Shl => x86isd::VSHL,
        ISD::Srl => x86isd::VSRL,
        _ => x86isd::VSRA,
    }
}

fn byte(
    dag: &mut SelectionDAG,
    value: u128,
) -> SDValue {
    dag.get_constant_with(value, EVT::I8, true, false)
}

/// `getTargetVShiftByConstNode`: an amount a lane is shifted out by is zero (a
/// logical shift) or the sign (an arithmetic one).
fn shift_by_constant(
    dag: &mut SelectionDAG,
    opcode: ISD,
    vt: EVT,
    value: SDValue,
    amount: u128,
) -> SDValue {
    let bits = lane_bits(vt);
    let amount = match amount < bits {
        true => amount,
        false if opcode == ISD::Sra => bits - 1,
        false => return dag.get_constant(0, vt),
    };
    let by = byte(dag, amount);
    dag.get_node(by_immediate(opcode), vt, &[value, by])
}

/// `getConstVector`: a lane no value is given is undefined.
fn constant_vector(
    dag: &mut SelectionDAG,
    vt: EVT,
    lanes: &[Option<u128>],
) -> SDValue {
    let lane_vt = vt.element();
    let ops: Vec<SDValue> = lanes
        .iter()
        .map(|lane| match lane {
            Some(value) => dag.get_constant(*value, lane_vt),
            None => dag.get_undef(lane_vt),
        })
        .collect();
    dag.get_build_vector(vt, &ops)
}

/// `isBuildVectorOfConstantSDNodes`, undefined lanes `None`.
fn constant_lanes(
    dag: &SelectionDAG,
    amount: SDValue,
) -> Option<Vec<Option<u128>>> {
    if dag.opcode(amount) != ISD::BuildVector {
        return None;
    }
    dag.node(amount)
        .ops
        .iter()
        .map(|lane| if dag.is_undef(*lane) { Some(None) } else { dag.constant(*lane).map(Some) })
        .collect()
}

/// The scalar every lane of a build vector is: a splat of a variable.
fn splat_scalar(
    dag: &SelectionDAG,
    amount: SDValue,
) -> Option<SDValue> {
    if dag.opcode(amount) != ISD::BuildVector {
        return None;
    }
    let first = *dag.node(amount).ops.first()?;
    let variable = !dag.is_undef(first) && dag.constant(first).is_none();
    (variable && dag.node(amount).ops.iter().all(|lane| *lane == first)).then_some(first)
}

/// `getTargetVShiftNode`'s amount: the scalar zero-extended into the low
/// quadword of a vector of `vt`.
fn amount_register(
    dag: &mut SelectionDAG,
    vt: EVT,
    scalar: SDValue,
) -> SDValue {
    if vt.element() == EVT::I64 {
        let wide = dag.get_zext_or_trunc(scalar, EVT::I64);
        return scalar_in_lane0(dag, vt, EVT::I64, wide, true);
    }
    let dword = dag.get_zext_or_trunc(scalar, EVT::I32);
    let register = scalar_in_lane0(dag, v128(32), EVT::I32, dword, true);
    reinterpreted(dag, vt, register)
}

fn shift_by_register(
    dag: &mut SelectionDAG,
    opcode: ISD,
    vt: EVT,
    value: SDValue,
    amount: SDValue,
) -> SDValue {
    dag.get_node(by_register(opcode), vt, &[value, amount])
}

/// `mask ? a : b` for a mask of all ones or zeros a lane: `pand`, `pandn`,
/// `por` (the expansion of `VSELECT` without SSE4.1, with the complement made
/// by the instruction).
fn select_by_mask(
    dag: &mut SelectionDAG,
    vt: EVT,
    mask: SDValue,
    a: SDValue,
    b: SDValue,
) -> SDValue {
    let taken = dag.get_node(ISD::And, vt, &[a, mask]);
    let kept = dag.get_node(x86isd::ANDNP, vt, &[mask, b]);
    dag.get_node(ISD::Or, vt, &[taken, kept])
}

/// Lanes of `a` where `from_a` holds, of `b` elsewhere.
fn blend(
    dag: &mut SelectionDAG,
    vt: EVT,
    from_a: &[Option<bool>],
    a: SDValue,
    b: SDValue,
) -> SDValue {
    // The low half of one and the high half of the other: the high
    // quadword of the second moved down, then `punpcklqdq`.
    let half = from_a.len() / 2;
    let (low, high) = (&from_a[..half], &from_a[half..]);
    let all = |lanes: &[Option<bool>], on: bool| lanes.iter().all(|lane| lane.is_none_or(|lane| lane == on));
    if (all(low, true) && all(high, false)) || (all(low, false) && all(high, true)) {
        let (first, second) = if all(low, true) { (a, b) } else { (b, a) };
        let doublewords = reinterpreted(dag, v128(32), second);
        let order = byte(dag, 0xEE);
        let down = dag.get_node(x86isd::PSHUFD, v128(32), &[doublewords, order]);
        let down = reinterpreted(dag, vt, down);
        let pair = v128(64);
        let (first, down) = (reinterpreted(dag, pair, first), reinterpreted(dag, pair, down));
        let joined = dag.get_node(x86isd::UNPCKL, pair, &[first, down]);
        return reinterpreted(dag, vt, joined);
    }
    let lanes: Vec<Option<u128>> = from_a.iter().map(|lane| lane.map(|a| if a { u128::MAX } else { 0 })).collect();
    let mask = constant_vector(dag, vt, &lanes);
    select_by_mask(dag, vt, mask, a, b)
}

/// What a shift's amount is, read off the node before its operands are
/// legalized: a constant `BUILD_VECTOR` is a pool load and a splat of a
/// variable a `movd` and `pshufd` once they are, and the shifts a lowering
/// makes of its own take the amount from here, not from a node.
enum Amount {
    Constant(Vec<Option<u128>>),
    Splat(SDValue),
    Vector(SDValue),
}

impl Amount {
    fn of(
        dag: &SelectionDAG,
        amount: SDValue,
    ) -> Amount {
        if let Some(lanes) = constant_lanes(dag, amount) {
            return Amount::Constant(lanes);
        }
        match splat_scalar(dag, amount) {
            Some(scalar) => Amount::Splat(scalar),
            None => Amount::Vector(amount),
        }
    }

    /// The one constant every lane is.
    fn splat_constant(&self) -> Option<u128> {
        let Amount::Constant(lanes) = self else { return None };
        // `X86::isConstantSplat`: an undef lane agrees with any amount.
        let first = lanes.iter().flatten().next().copied()?;
        lanes.iter().all(|lane| lane.is_none_or(|lane| lane == first)).then_some(first)
    }

    fn constants(&self) -> Option<&[Option<u128>]> {
        match self {
            Amount::Constant(lanes) => Some(lanes),
            _ => None,
        }
    }

    /// The amount as a vector of `vt`.
    fn vector(
        &self,
        dag: &mut SelectionDAG,
        vt: EVT,
    ) -> SDValue {
        match self {
            Amount::Constant(lanes) => constant_vector(dag, vt, lanes),
            Amount::Splat(scalar) => dag.get_splat_build_vector(vt, *scalar),
            Amount::Vector(vector) => *vector,
        }
    }
}

/// `LowerShift` of a 128-bit integer vector. `None`: the shift is expanded a
/// lane at a time.
pub fn lower_vector_shift(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let amount = Amount::of(dag, dag.operand(node, 1));
    let users: Vec<u32> = dag.users(node).collect();
    let feeds_and = users.len() == 1 && dag.opcode(SDValue { node: users[0], res: 0 }) == ISD::And;
    lower_shift(dag, dag.opcode(node), vt, dag.operand(node, 0), &amount, feeds_and)
}

fn lower_shift(
    dag: &mut SelectionDAG,
    opcode: ISD,
    vt: EVT,
    r: SDValue,
    amount: &Amount,
    feeds_and: bool,
) -> Option<SDValue> {
    let EVT::Simple(mvt) = vt else { return None };
    if !matches!(mvt, MVT::V16I8 | MVT::V8I16 | MVT::V4I32 | MVT::V2I64) {
        return None;
    }
    let constants = amount.constants();
    if let Some(by) = amount.splat_constant()
        && let Some(result) = shift_by_scalar_immediate(dag, opcode, vt, r, by, feeds_and)
    {
        return Some(result);
    }
    if let Amount::Splat(scalar) = amount
        && let Some(result) = shift_by_scalar_variable(dag, opcode, vt, r, *scalar)
    {
        return Some(result);
    }
    let words = v128(16);
    // `ashr(R, Amt) === sub(xor(lshr(R, Amt), M), M)`, `M = lshr(SIGN_MASK,
    // Amt)`.
    if mvt == MVT::V2I64 && opcode == ISD::Sra {
        let sign = dag.get_constant(1 << 63, vt);
        let mask = lower_shift(dag, ISD::Srl, vt, sign, amount, false)?;
        let shifted = lower_shift(dag, ISD::Srl, vt, r, amount, false)?;
        let flipped = dag.get_node(ISD::Xor, vt, &[shifted, mask]);
        return Some(dag.get_node(ISD::Sub, vt, &[flipped, mask]));
    }
    // 2i64 logical shifts do the shift per lane and put the halves together.
    if mvt == MVT::V2I64 {
        let (low, high) = match constants {
            Some(lanes) => {
                let low = shift_by_constant(dag, opcode, vt, r, lanes[0].unwrap_or(0));
                (low, shift_by_constant(dag, opcode, vt, r, lanes[1].unwrap_or(0)))
            }
            None => {
                let amt = amount.vector(dag, vt);
                let upper = reinterpreted(dag, v128(32), amt);
                let order = byte(dag, 0xEE);
                let upper = dag.get_node(x86isd::PSHUFD, v128(32), &[upper, order]);
                let upper = reinterpreted(dag, vt, upper);
                (shift_by_register(dag, opcode, vt, r, amt), shift_by_register(dag, opcode, vt, r, upper))
            }
        };
        // {low.lo, high.hi}: the high quadword of `high` moved down, then
        // `punpcklqdq`.
        let doublewords = reinterpreted(dag, v128(32), high);
        let order = byte(dag, 0xEE);
        let down = dag.get_node(x86isd::PSHUFD, v128(32), &[doublewords, order]);
        let down = reinterpreted(dag, vt, down);
        return Some(dag.get_node(x86isd::UNPCKL, vt, &[low, down]));
    }
    if let Some(lanes) = constants {
        // Two amounts: two shifts by a constant and a blend.
        let mut unique: Vec<u128> = Vec::new();
        for by in lanes.iter().flatten().filter(|by| **by < lane_bits(vt)) {
            if !unique.contains(by) {
                unique.push(*by);
            }
        }
        if unique.len() == 2 && matches!(mvt, MVT::V8I16 | MVT::V4I32) {
            let from_a: Vec<Option<bool>> =
                lanes.iter().map(|lane| lane.filter(|by| *by < lane_bits(vt)).map(|by| by == unique[0])).collect();
            let widens = from_a
                .chunks(2)
                .all(
                    |pair| match (pair[0], pair[1]) {
                        (Some(x), Some(y)) => x == y,
                        _ => true,
                    },
                );
            if mvt == MVT::V4I32 || opcode != ISD::Shl || widens {
                let by_a = shift_by_constant(dag, opcode, vt, r, unique[0]);
                let by_b = shift_by_constant(dag, opcode, vt, r, unique[1]);
                return Some(blend(dag, vt, &from_a, by_a, by_b));
            }
        }
    }
    // A left shift by a constant is a multiply (`convertShiftLeftToScale`).
    if opcode == ISD::Shl
        && let Some(lanes) = constants
        && matches!(mvt, MVT::V8I16 | MVT::V4I32 | MVT::V16I8)
    {
        let scale: Vec<Option<u128>> =
            lanes.iter().map(|lane| lane.filter(|by| *by < lane_bits(vt)).map(|by| 1u128 << by)).collect();
        let scale = constant_vector(dag, vt, &scale);
        return Some(dag.get_node(ISD::Mul, vt, &[r, scale]));
    }
    // A constant `v8i16` right shift is the high half of a multiply.
    if mvt == MVT::V8I16
        && let Some(lanes) = constants
        && opcode != ISD::Shl
    {
        let scale: Vec<Option<u128>> =
            lanes.iter().map(|lane| lane.filter(|by| (1..16).contains(by)).map(|by| 1u128 << (16 - by))).collect();
        if opcode == ISD::Srl {
            let scale = constant_vector(dag, vt, &scale);
            let product = dag.get_node(ISD::MulHU, vt, &[r, scale]);
            let zero: Vec<Option<bool>> = lanes.iter().map(|lane| lane.map(|by| by == 0)).collect();
            return Some(if zero.contains(&Some(true)) { blend(dag, vt, &zero, r, product) } else { product });
        }
        // `isKnownNeverZero(Amt)`: no lane of the amount is zero.
        if lanes.iter().all(|lane| lane.is_some_and(|by| by != 0)) {
            let scale = constant_vector(dag, vt, &scale);
            let product = dag.get_node(ISD::MulHS, vt, &[r, scale]);
            let one: Vec<Option<bool>> = lanes.iter().map(|lane| lane.map(|by| by == 1)).collect();
            if !one.contains(&Some(true)) {
                return Some(product);
            }
            let by_one = shift_by_constant(dag, ISD::Sra, vt, r, 1);
            return Some(blend(dag, vt, &one, by_one, product));
        }
    }
    if mvt == MVT::V4I32 {
        return Some(shift_dword_lanes(dag, opcode, r, amount));
    }
    // A constant `v16i8` right shift widens to words and multiplies.
    if mvt == MVT::V16I8
        && opcode != ISD::Shl
        && let Some(lanes) = constants
    {
        let scale: Vec<Option<u128>> =
            lanes.iter().map(|lane| lane.filter(|by| *by < 8).map(|by| 1u128 << (8 - by))).collect();
        let half = |dag: &mut SelectionDAG, unpack: ISD, scale: &[Option<u128>]| {
            let unpacked = dag.get_node(unpack, vt, &[r, r]);
            let unpacked = reinterpreted(dag, words, unpacked);
            let widened = shift_by_constant(dag, opcode, words, unpacked, 8);
            let scale = constant_vector(dag, words, scale);
            let product = dag.get_node(ISD::Mul, words, &[widened, scale]);
            shift_by_constant(dag, ISD::Srl, words, product, 8)
        };
        let low = half(dag, x86isd::UNPCKL, &scale[..8]);
        let high = half(dag, x86isd::UNPCKH, &scale[8..]);
        return Some(dag.get_node(x86isd::PACKUS, vt, &[low, high]));
    }
    let amt = amount.vector(dag, vt);
    match mvt {
        MVT::V16I8 => Some(shift_bytes_by_ladder(dag, opcode, r, amt)),
        MVT::V8I16 => Some(shift_words_by_ladder(dag, opcode, r, amt)),
        _ => None,
    }
}

/// `LowerShiftByScalarImmediate`: a constant splat amount.
fn shift_by_scalar_immediate(
    dag: &mut SelectionDAG,
    opcode: ISD,
    vt: EVT,
    r: SDValue,
    amount: u128,
    feeds_and: bool,
) -> Option<SDValue> {
    let bits = lane_bits(vt);
    if bits >= 16 {
        if bits == 64 && opcode == ISD::Sra {
            // `psraq` is AVX-512: the logical shift and the sign put back.
            let mask = dag.get_constant((1u128 << 63) >> amount.min(63), vt);
            let logical = shift_by_constant(dag, ISD::Srl, vt, r, amount);
            let flipped = dag.get_node(ISD::Xor, vt, &[logical, mask]);
            return Some(dag.get_node(ISD::Sub, vt, &[flipped, mask]));
        }
        return Some(shift_by_constant(dag, opcode, vt, r, amount));
    }
    if amount >= bits {
        return Some(dag.get_undef(vt));
    }
    let words = v128(16);
    match opcode {
        // `(shl R, 1)` is even: `R` is frozen so the sum of an undefined `R`
        // is too.
        ISD::Shl if amount == 1 || (amount == 2 && !masked_value_is_zero(dag, r, 0xC0) && !feeds_and) => {
            let mut result = dag.get_node(ISD::Freeze, vt, &[r]);
            for _ in 0..amount {
                result = dag.get_node(ISD::Add, vt, &[result, result]);
            }
            Some(result)
        }
        // `ashr(R, 7) === cmp_slt(R, 0)`.
        ISD::Sra if amount == 7 => {
            let zero = dag.get_constant(0, vt);
            Some(dag.get_node(x86isd::PCMPGT, vt, &[zero, r]))
        }
        ISD::Shl | ISD::Srl => {
            let wide = reinterpreted(dag, words, r);
            let shifted = shift_by_constant(dag, opcode, words, wide, amount);
            let shifted = reinterpreted(dag, vt, shifted);
            let mask = if opcode == ISD::Shl { (0xFFu128 << amount) & 0xFF } else { 0xFFu128 >> amount };
            let mask = dag.get_constant(mask, vt);
            Some(dag.get_node(ISD::And, vt, &[shifted, mask]))
        }
        // `ashr(R, Amt) === sub(xor(lshr(R, Amt), Mask), Mask)`.
        _ => {
            let logical = shift_by_scalar_immediate(dag, ISD::Srl, vt, r, amount, false)?;
            let mask = dag.get_constant(128 >> amount, vt);
            let flipped = dag.get_node(ISD::Xor, vt, &[logical, mask]);
            Some(dag.get_node(ISD::Sub, vt, &[flipped, mask]))
        }
    }
}

/// `LowerShiftByScalarVariable`: one amount for every lane, in a register.
fn shift_by_scalar_variable(
    dag: &mut SelectionDAG,
    opcode: ISD,
    vt: EVT,
    r: SDValue,
    scalar: SDValue,
) -> Option<SDValue> {
    if vt.element() != EVT::I8 {
        if vt.element() == EVT::I64 && opcode == ISD::Sra {
            return None;
        }
        let amount = amount_register(dag, vt, scalar);
        return Some(shift_by_register(dag, opcode, vt, r, amount));
    }
    // vXi8 shifts: as `v8i16` and a mask.
    let words = v128(16);
    let amount = amount_register(dag, words, scalar);
    let logical = if opcode == ISD::Shl { ISD::Shl } else { ISD::Srl };
    let ones = dag.get_constant(u128::MAX, words);
    let mut mask = shift_by_register(dag, logical, words, ones, amount);
    if opcode == ISD::Shl {
        let low = dag.get_constant(0xFF, words);
        mask = dag.get_node(ISD::And, words, &[mask, low]);
    } else {
        mask = shift_by_constant(dag, ISD::Srl, words, mask, 8);
    }
    // The mask byte in both halves of the word.
    let both = dag.get_constant(0x0101, words);
    let mask = dag.get_node(ISD::Mul, words, &[mask, both]);
    let mask = reinterpreted(dag, vt, mask);
    let wide = reinterpreted(dag, words, r);
    let shifted = shift_by_register(dag, logical, words, wide, amount);
    let shifted = reinterpreted(dag, vt, shifted);
    let mut result = dag.get_node(ISD::And, vt, &[shifted, mask]);
    if opcode == ISD::Sra {
        // `SignMask = lshr(0x8080, Amt)`.
        let signs = dag.get_constant(0x8080, words);
        let signs = shift_by_register(dag, ISD::Srl, words, signs, amount);
        let signs = reinterpreted(dag, vt, signs);
        result = dag.get_node(ISD::Xor, vt, &[result, signs]);
        result = dag.get_node(ISD::Sub, vt, &[result, signs]);
    }
    Some(result)
}

/// A `v4i32` shifted by an amount a lane: each lane is shifted by the
/// amount of its own in a whole register, and the lanes that count are put
/// together.
fn shift_dword_lanes(
    dag: &mut SelectionDAG,
    opcode: ISD,
    r: SDValue,
    amount: &Amount,
) -> SDValue {
    let vt = v128(32);
    let amt = amount.vector(dag, vt);
    let shifted: Vec<SDValue> = (0..4)
        .map(|lane| match amount.constants() {
            Some(lanes) => shift_by_constant(dag, opcode, vt, r, lanes[lane].unwrap_or(0)),
            None => {
                // The amount in the low quadword: the lane moved down, and
                // its top 16 bits duplicated, which is the same for an
                // amount out of range (and cheaper than a zero).
                let source = if lane >= 2 {
                    let doublewords = reinterpreted(dag, v128(32), amt);
                    let order = byte(dag, 0x0E);
                    dag.get_node(x86isd::PSHUFD, v128(32), &[doublewords, order])
                } else {
                    amt
                };
                let source = reinterpreted(dag, v128(16), source);
                let order = byte(dag, if lane % 2 == 0 { 0x54 } else { 0xFE });
                let low = dag.get_node(x86isd::PSHUFLW, v128(16), &[source, order]);
                let low = reinterpreted(dag, vt, low);
                shift_by_register(dag, opcode, vt, r, low)
            }
        })
        .collect();
    // {r0[0], r1[1], r2[2], r3[3]}.
    let low = dag.get_node(x86isd::UNPCKL, vt, &[shifted[0], shifted[1]]);
    let high = dag.get_node(x86isd::UNPCKH, vt, &[shifted[2], shifted[3]]);
    let order = byte(dag, 0xCC);
    let (low, high) =
        (dag.get_node(x86isd::PSHUFD, vt, &[low, order]), dag.get_node(x86isd::PSHUFD, vt, &[high, order]));
    let pair = v128(64);
    let (low, high) = (reinterpreted(dag, pair, low), reinterpreted(dag, pair, high));
    let joined = dag.get_node(x86isd::UNPCKL, pair, &[low, high]);
    reinterpreted(dag, vt, joined)
}

/// The sign bit of `select` picks `taken` (`pcmpgt` against zero, then the
/// mask select).
fn sign_bit_select(
    dag: &mut SelectionDAG,
    vt: EVT,
    select: SDValue,
    taken: SDValue,
    kept: SDValue,
) -> SDValue {
    let zero = dag.get_constant(0, vt);
    let mask = dag.get_node(x86isd::PCMPGT, vt, &[zero, select]);
    select_by_mask(dag, vt, mask, taken, kept)
}

/// The variable `v16i8` shift: the amount's bits, one at a time from the top,
/// pick the value shifted by 4, 2 and 1 (`pcmpgtb` on the amount shifted so
/// its bit is the sign).
fn shift_bytes_by_ladder(
    dag: &mut SelectionDAG,
    opcode: ISD,
    mut r: SDValue,
    amt: SDValue,
) -> SDValue {
    let vt = v128(8);
    let words = v128(16);
    let known_zero = known_bits(dag, amt).zero & 7;
    if known_zero == 7 {
        return r;
    }
    let leading_zeros = if known_zero & 4 == 0 {
        0
    } else if known_zero & 2 == 0 {
        1
    } else {
        2
    };
    // `a = a << 5`, as words: only the low 3 bits of each byte are read.
    let amount = reinterpreted(dag, words, amt);
    let amount = shift_by_constant(dag, ISD::Shl, words, amount, 5 + leading_zeros);
    let mut amount = reinterpreted(dag, vt, amount);
    let steps: Vec<u128> = [4u128, 2, 1]
        .into_iter()
        .filter(|step| *step == 1 || (*step == 4 && leading_zeros < 1) || (*step == 2 && leading_zeros < 2))
        .collect();
    if opcode != ISD::Sra {
        for step in steps {
            let moved = shift_bytes_by_constant(dag, opcode, r, step);
            r = sign_bit_select(dag, vt, amount, moved, r);
            if step != 1 {
                amount = dag.get_node(ISD::Add, vt, &[amount, amount]);
            }
        }
        return r;
    }
    // Each byte in the high half of a word, where the sign extends; the low
    // half is whatever the unpack of the register with itself leaves.
    let unpack = |dag: &mut SelectionDAG, value: SDValue| {
        let low = dag.get_node(x86isd::UNPCKL, vt, &[value, value]);
        let high = dag.get_node(x86isd::UNPCKH, vt, &[value, value]);
        [reinterpreted(dag, words, low), reinterpreted(dag, words, high)]
    };
    let [mut amount_low, mut amount_high] = unpack(dag, amount);
    let [mut low, mut high] = unpack(dag, r);
    for step in steps {
        let moved_low = shift_by_constant(dag, ISD::Sra, words, low, step);
        let moved_high = shift_by_constant(dag, ISD::Sra, words, high, step);
        low = sign_bit_select(dag, words, amount_low, moved_low, low);
        high = sign_bit_select(dag, words, amount_high, moved_high, high);
        if step != 1 {
            amount_low = dag.get_node(ISD::Add, words, &[amount_low, amount_low]);
            amount_high = dag.get_node(ISD::Add, words, &[amount_high, amount_high]);
        }
    }
    // Back to the low byte, leaving a zero high byte to pack.
    let low = shift_by_constant(dag, ISD::Srl, words, low, 8);
    let high = shift_by_constant(dag, ISD::Srl, words, high, 8);
    dag.get_node(x86isd::PACKUS, vt, &[low, high])
}

/// A `v16i8` shifted by a constant: the amount is below 8.
fn shift_bytes_by_constant(
    dag: &mut SelectionDAG,
    opcode: ISD,
    r: SDValue,
    amount: u128,
) -> SDValue {
    shift_by_scalar_immediate(dag, opcode, v128(8), r, amount, false).expect("a byte shift by a constant")
}

/// The variable `v8i16` shift: as the bytes' ladder, by 8, 4, 2 and 1, the
/// sign of the lane splat by `psraw`.
fn shift_words_by_ladder(
    dag: &mut SelectionDAG,
    opcode: ISD,
    mut r: SDValue,
    amt: SDValue,
) -> SDValue {
    let vt = v128(16);
    // `a = a << 12`.
    let mut amount = shift_by_constant(dag, ISD::Shl, vt, amt, 12);
    for step in [8u128, 4, 2, 1] {
        let moved = shift_by_constant(dag, opcode, vt, r, step);
        let mask = shift_by_constant(dag, ISD::Sra, vt, amount, 15);
        r = select_by_mask(dag, vt, mask, moved, r);
        if step != 1 {
            amount = dag.get_node(ISD::Add, vt, &[amount, amount]);
        }
    }
    r
}
