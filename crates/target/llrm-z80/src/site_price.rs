//! The price of one site's two forms at -O1: a long shift by a constant inline or as the routine's call, and a
//! long multiply by `2^k +- 1` as a shift and an add or as `__mulsi3`. Each site takes the form with the lower
//! `T + 3 B` (the rate `compile::cheaper` states for the levels that trade), whatever else the function holds.
//!
//! The inline costs are what the lowering emits, tabulated by `tests/site_price.rs`, which fails when the
//! lowering changes them. The call costs come from the routine models in `runtime_size`.

use llrm_codegen::backend::selectiondag::isd::ISD;

use crate::runtime_size::mulsi3_clocks;

/// Clocks one byte is worth where a level trades bytes for clocks.
pub const T_PER_BYTE: u64 = 3;

/// `(bytes, clocks)` of `shl`, `lshr`, `ashr` of an `i32` by 1..=31 inline, without the `ret`.
pub static INLINE_SHIFT: [[(u32, u64); 31]; 3] = [
    [
        (9, 50), (26, 117), (32, 148), (38, 179), (42, 202), (45, 222),
        (48, 241), (11, 44), (42, 167), (42, 167), (42, 165), (40, 157),
        (34, 133), (28, 109), (22, 85), (3, 10), (4, 21), (5, 32),
        (6, 43), (7, 54), (8, 65), (9, 76), (10, 87), (6, 21),
        (8, 29), (10, 37), (12, 44), (13, 48), (12, 44), (11, 40),
        (10, 36),
    ],
    [
        (21, 83), (30, 119), (39, 155), (48, 191), (55, 219), (61, 244),
        (67, 268), (10, 39), (23, 140), (26, 145), (29, 148), (30, 145),
        (27, 126), (24, 107), (22, 85), (3, 10), (7, 26), (11, 42),
        (15, 58), (19, 74), (23, 90), (27, 106), (31, 122), (6, 21),
        (8, 29), (10, 37), (12, 44), (13, 48), (12, 44), (11, 40),
        (10, 36),
    ],
    [
        (21, 83), (30, 119), (39, 155), (48, 191), (55, 219), (61, 244),
        (67, 268), (10, 40), (45, 180), (45, 180), (45, 179), (44, 175),
        (41, 163), (38, 151), (21, 83), (5, 20), (10, 40), (14, 56),
        (18, 72), (22, 88), (26, 104), (30, 120), (34, 136), (12, 48),
        (14, 56), (16, 64), (18, 72), (20, 80), (22, 88), (24, 96),
        (7, 28),
    ],
];

/// `(bytes, clocks)` of the shift as the routine's call less the inline shift, in a function at -O1 that holds two
/// long multiplies by constants (so the routine is linked): the argument moves and the clobbered registers count.
pub static CALL_MINUS_INLINE_SHIFT: [[(i64, i64); 31]; 3] = [
    [
        (0, 0), (-22, 19), (-28, 28), (-34, 37), (-38, 54), (-41, 74),
        (-44, 95), (-7, 332), (-38, 249), (-38, 289), (-38, 331), (-36, 379),
        (-30, 443), (-24, 507), (-18, 571), (0, 1000000), (0, 715), (-1, 744),
        (-2, 773), (-3, 802), (-4, 831), (-5, 860), (-6, 889), (-2, 995),
        (-4, 1027), (-6, 1059), (-8, 1092), (-9, 1128), (-8, 1172), (-7, 1216),
        (-6, 1260),
    ],
    [
        (-17, 18), (-26, 27), (-35, 36), (-44, 45), (-51, 62), (-57, 82),
        (-63, 103), (-6, 377), (-19, 321), (-22, 361), (-25, 403), (-26, 451),
        (-23, 515), (-20, 579), (-18, 646), (0, 1000000), (-3, 795), (-7, 824),
        (-11, 853), (-15, 882), (-19, 911), (-23, 940), (-27, 969), (-2, 1115),
        (-4, 1152), (-6, 1189), (-8, 1227), (-9, 1268), (-8, 1317), (-7, 1366),
        (-6, 1415),
    ],
    [
        (-17, 18), (-26, 27), (-35, 36), (-44, 45), (-51, 62), (-57, 82),
        (-63, 103), (-6, 376), (-41, 281), (-41, 326), (-41, 372), (-40, 421),
        (-37, 478), (-34, 535), (-17, 648), (0, 1000000), (-6, 781), (-10, 810),
        (-14, 839), (-18, 868), (-22, 897), (-26, 926), (-30, 955), (-8, 1088),
        (-10, 1125), (-12, 1162), (-14, 1199), (-16, 1236), (-18, 1273), (-20, 1310),
        (-3, 1423),
    ],
];

/// `(bytes, clocks)` of `x * c` for c = 2^k - 1, 2^k + 1 (k = 2..=8, 2^k - 1 from k = 2) with every shift inline,
/// keyed by `c`.
pub static INLINE_MUL: &[(u32, (u32, u64))] = &[
    (3, (37, 243)),
    (5, (36, 210)),
    (7, (41, 244)),
    (9, (39, 236)),
    (15, (44, 270)),
    (17, (42, 262)),
    (31, (45, 288)),
    (33, (43, 280)),
    (63, (45, 303)),
    (65, (43, 295)),
    (127, (45, 317)),
    (129, (43, 309)),
    (255, (28, 163)),
    (257, (26, 155)),
];

fn priced((bytes, clocks): (u32, u64)) -> u64 {
    clocks + T_PER_BYTE * u64::from(bytes)
}

fn which(op: ISD) -> Option<usize> {
    match op {
        ISD::Shl => Some(0),
        ISD::Srl => Some(1),
        ISD::Sra => Some(2),
        _ => None,
    }
}

fn inline_shift(
    op: ISD,
    amount: u32,
) -> Option<(u32, u64)> {
    let row = which(op)?;
    INLINE_SHIFT[row].get(amount.checked_sub(1)? as usize).copied()
}

/// The call's `T + 3 B` less the inline form's, with the argument moves and the registers the call clobbers.
fn call_minus_inline(
    op: ISD,
    amount: u32,
) -> Option<i64> {
    let row = which(op)?;
    let (bytes, clocks) = CALL_MINUS_INLINE_SHIFT[row].get(amount.checked_sub(1)? as usize).copied()?;
    Some(clocks + T_PER_BYTE as i64 * bytes)
}

/// `Z80_SITE_FORM=call:N` or `inline:N` forces the shifts by `N` to one form, so the tables can be measured.
fn forced(amount: u32) -> Option<bool> {
    let form = std::env::var("Z80_SITE_FORM").ok()?;
    let (form, at) = form.split_once(':')?;
    (at.parse() == Ok(amount)).then_some(form == "call")
}

/// Whether a long shift by the constant `amount` is the routine's call here: its `T + 3 B` is the lower.
pub fn shift_calls(
    op: ISD,
    amount: u128,
    bits: u128,
) -> bool {
    let Ok(amount) = u32::try_from(amount) else { return false };
    if bits != 32 {
        return false;
    }
    forced(amount).unwrap_or_else(|| call_minus_inline(op, amount).is_some_and(|extra| extra < 0))
}

/// The cheaper form of a long shift left by `amount`.
fn best_shl(amount: u32) -> u64 {
    let inline = inline_shift(ISD::Shl, amount).map_or(0, priced);
    inline.saturating_add_signed(call_minus_inline(ISD::Shl, amount).unwrap_or(0).min(0))
}

/// `ld bc, #c; call __mulsi3` and the operand moves around it: 6 B, 10 T, the call and the routine.
fn call_mul(c: u32) -> (u32, u64) {
    (6, 10 + u64::from(mulsi3_clocks(c)))
}

/// Whether `x * c` stays as a shift and an add rather than the `__mulsi3` call: the shift is priced at its own
/// cheaper form, the add as the table measured it.
pub fn mul_inline(c: u128) -> bool {
    let Ok(c) = u32::try_from(c) else { return false };
    let k = [c.wrapping_sub(1), c + 1].into_iter().filter(|n| n.is_power_of_two()).map(u32::trailing_zeros).min();
    let Some(k) = k else { return false };
    let Some(&(_, inline)) = INLINE_MUL.iter().find(|(one, _)| *one == c) else { return false };
    let Some(shift) = inline_shift(ISD::Shl, k) else { return false };
    // the add and the copies the decomposition leaves once its shift is taken out
    let rest = priced(inline).saturating_sub(priced(shift));
    rest + best_shl(k) <= priced(call_mul(c))
}
