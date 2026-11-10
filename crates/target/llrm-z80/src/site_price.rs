//! The price of one site's two forms at -O1: a long shift by a constant inline or as the routine's call, and a
//! long multiply by `2^k +- 1` as a shift and an add or as `__mulsi3`. Each site takes the form with the lower
//! `T + 3 B` (the rate `compile::cheaper` states for the levels that trade), whatever else the function holds.
//!
//! The inline costs are what the lowering emits, tabulated by `tests/site_price.rs`, which fails when the
//! lowering changes them. The call costs come from the routine models in `runtime_size`.

use llrm_codegen::backend::selectiondag::isd::ISD;

use crate::runtime_size::{mulsi3_clocks, shift_routine_clocks};

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

/// `ld a, #n; call __ashlsi3` and the routine: 5 B.
fn call_shift(
    op: ISD,
    amount: u32,
) -> (u32, u64) {
    (5, 7 + u64::from(shift_routine_clocks(op, amount)))
}

fn inline_shift(
    op: ISD,
    amount: u32,
) -> Option<(u32, u64)> {
    let row = which(op)?;
    INLINE_SHIFT[row].get(amount.checked_sub(1)? as usize).copied()
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
    inline_shift(op, amount).is_some_and(|inline| priced(call_shift(op, amount)) < priced(inline))
}

/// The cheaper form of a long shift left by `amount`.
fn best_shl(amount: u32) -> u64 {
    let call = priced(call_shift(ISD::Shl, amount));
    inline_shift(ISD::Shl, amount).map_or(call, |inline| priced(inline).min(call))
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
