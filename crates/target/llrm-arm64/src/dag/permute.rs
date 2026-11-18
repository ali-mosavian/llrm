//! `AArch64PerfectShuffle.h`'s `isZIPMask`, `isUZPMask`, `isTRNMask` and their
//! `_v_undef_` forms, and the part of `LowerVECTOR_SHUFFLE` that turns a mask
//! they match into `ZIP1`/`ZIP2`, `UZP1`/`UZP2` or `TRN1`/`TRN2`.
//!
//! DEVIATE S3-1 (owner llrm-w11-coalescer-join, exit 2026-11-24): the pin
//! tries `EXT`, the 4-lane perfect-shuffle table, `INS`, `tryFormConcat`,
//! `tryWidenMaskForShuffle`, `BSP` and `TBL` after these; here a mask no
//! permute matches is declined.

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_dagisel::generated::aarch64::ids::isd as aarch64_isd;

/// Which permute a mask is, as the pin's `WhichResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Zip,
    Uzp,
    Trn,
}

/// A mask that is one permute: the second half (`2`) or first (`1`), and
/// whether the operands are swapped (`OperandOrder`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Matched {
    pub kind: Kind,
    pub second: bool,
    pub swapped: bool,
    /// The `_v_undef_` form: both operands are the first.
    pub single: bool,
}

/// `isZIPMask`/`isTRNMask`: the one of four (result, operand order) readings
/// every defined lane agrees with; `value(i, odd, second, swapped)` is what
/// lane `i` (or `i + 1`, `odd`) holds in a reading.
fn two_input(
    mask: &[i32],
    lanes: usize,
    value: impl Fn(usize, bool, bool, bool) -> usize,
) -> Option<(bool, bool)> {
    if lanes % 2 != 0 {
        return None;
    }
    // [result][order]
    let mut live = [[true; 2]; 2];
    for i in (0..lanes).step_by(2) {
        for (odd, &lane) in [mask[i], mask[i + 1]].iter().enumerate() {
            if lane < 0 {
                continue;
            }
            for result in 0..2 {
                for order in 0..2 {
                    if lane as usize != value(i, odd == 1, result == 1, order == 1) {
                        live[result][order] = false;
                    }
                }
            }
        }
    }
    let mut hits = (0..2).flat_map(|result| (0..2).map(move |order| (result, order))).filter(|&(r, o)| live[r][o]);
    let only = hits.next()?;
    hits.next().is_none().then_some((only.0 == 1, only.1 == 1))
}

/// `isZIPMask`.
fn zip(
    mask: &[i32],
    lanes: usize,
) -> Option<(bool, bool)> {
    two_input(mask, lanes, |i, odd, second, swapped| {
        let base = i / 2 + if second { lanes / 2 } else { 0 };
        // The first operand supplies the even lanes unless swapped.
        base + if odd != swapped { lanes } else { 0 }
    })
}

/// `isTRNMask`.
fn trn(
    mask: &[i32],
    lanes: usize,
) -> Option<(bool, bool)> {
    two_input(mask, lanes, |i, odd, second, swapped| {
        let base = i + usize::from(second);
        base + if odd != swapped { lanes } else { 0 }
    })
}

/// `isUZPMask`: the half the first defined lane says.
fn uzp(mask: &[i32]) -> Option<bool> {
    let first = mask.iter().position(|&lane| lane >= 0)?;
    let second = mask[first] as usize != first * 2;
    mask.iter().enumerate().all(|(i, &lane)| lane < 0 || lane as usize == 2 * i + usize::from(second)).then_some(second)
}

/// `isZIP_v_undef_Mask`.
fn zip_single(
    mask: &[i32],
    lanes: usize,
) -> Option<bool> {
    if lanes % 2 != 0 {
        return None;
    }
    let second = mask[0] != 0;
    let mut at = usize::from(second) * lanes / 2;
    for pair in mask.chunks(2) {
        if pair.iter().any(|&lane| lane >= 0 && lane as usize != at) {
            return None;
        }
        at += 1;
    }
    Some(second)
}

/// `isUZP_v_undef_Mask`.
fn uzp_single(
    mask: &[i32],
    lanes: usize,
) -> Option<bool> {
    let half = lanes / 2;
    let second = mask[0] != 0;
    for part in 0..2 {
        let mut at = usize::from(second);
        for i in 0..half {
            let lane = mask[i + part * half];
            if lane >= 0 && lane as usize != at {
                return None;
            }
            at += 2;
        }
    }
    Some(second)
}

/// `isTRN_v_undef_Mask`.
fn trn_single(
    mask: &[i32],
    lanes: usize,
) -> Option<bool> {
    if lanes % 2 != 0 {
        return None;
    }
    let second = mask[0] != 0;
    for (pair, i) in mask.chunks(2).zip((0..).step_by(2)) {
        if pair.iter().any(|&lane| lane >= 0 && lane as usize != i + usize::from(second)) {
            return None;
        }
    }
    Some(second)
}

/// The permute `mask` over `lanes` lanes is, in the pin's order: ZIP, UZP,
/// TRN of two inputs, then of one.
pub fn matched(
    mask: &[i32],
    lanes: usize,
) -> Option<Matched> {
    if let Some((second, swapped)) = zip(mask, lanes) {
        return Some(Matched { kind: Kind::Zip, second, swapped, single: false });
    }
    if let Some(second) = uzp(mask) {
        return Some(Matched { kind: Kind::Uzp, second, swapped: false, single: false });
    }
    if let Some((second, swapped)) = trn(mask, lanes) {
        return Some(Matched { kind: Kind::Trn, second, swapped, single: false });
    }
    if let Some(second) = zip_single(mask, lanes) {
        return Some(Matched { kind: Kind::Zip, second, swapped: false, single: true });
    }
    if let Some(second) = uzp_single(mask, lanes) {
        return Some(Matched { kind: Kind::Uzp, second, swapped: false, single: true });
    }
    trn_single(mask, lanes).map(|second| Matched { kind: Kind::Trn, second, swapped: false, single: true })
}

/// `LowerVECTOR_SHUFFLE`'s `ZIP`/`UZP`/`TRN` nodes for a 64- or 128-bit
/// vector whose operands are its own type.
pub fn lower(
    dag: &mut SelectionDAG,
    op: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(op);
    let (v1, v2) = (dag.operand(op, 0), dag.operand(op, 1));
    if !matches!(vt.size_in_bits(), 64 | 128) || vt.lanes() < 2 || dag.value_type(v1) != vt {
        return None;
    }
    let mask = dag.shuffle_mask(op).to_vec();
    let found = matched(&mask, vt.lanes() as usize)?;
    // The pin's one-input forms are for a second operand that is undef.
    let second_is_undef = matches!(dag.opcode(v2), ISD::Undef | ISD::Poison);
    if found.single && !second_is_undef {
        return None;
    }
    let (a, b) = match (found.single, found.swapped) {
        (true, _) => (v1, v1),
        (false, false) => (v1, v2),
        (false, true) => (v2, v1),
    };
    let opcode = match (found.kind, found.second) {
        (Kind::Zip, false) => aarch64_isd::ZIP1,
        (Kind::Zip, true) => aarch64_isd::ZIP2,
        (Kind::Uzp, false) => aarch64_isd::UZP1,
        (Kind::Uzp, true) => aarch64_isd::UZP2,
        (Kind::Trn, false) => aarch64_isd::TRN1,
        (Kind::Trn, true) => aarch64_isd::TRN2,
    };
    Some(dag.get_node(ISD::Target(opcode), vt, &[a, b]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(mask: &[i32]) -> Option<(Kind, bool, bool, bool)> {
        matched(mask, mask.len()).map(|m| (m.kind, m.second, m.swapped, m.single))
    }

    /// The pin's own examples (`isZIPMask`, `isUZPMask`, `isTRNMask` comments).
    #[test]
    fn test_the_pins_documented_masks_are_matched() {
        assert_eq!(kind(&[0, 8, 1, 9, 2, 10, 3, 11]), Some((Kind::Zip, false, false, false)));
        assert_eq!(kind(&[4, 12, 5, 13, 6, 14, 7, 15]), Some((Kind::Zip, true, false, false)));
        assert_eq!(kind(&[8, 0, 9, 1, 10, 2, 11, 3]), Some((Kind::Zip, false, true, false)));
        assert_eq!(kind(&[0, 2, 4, 6, 8, 10, 12, 14]), Some((Kind::Uzp, false, false, false)));
        assert_eq!(kind(&[1, 3, 5, 7, 9, 11, 13, 15]), Some((Kind::Uzp, true, false, false)));
        assert_eq!(kind(&[0, 8, 2, 10, 4, 12, 6, 14]), Some((Kind::Trn, false, false, false)));
        assert_eq!(kind(&[9, 1, 11, 3, 13, 5, 15, 7]), Some((Kind::Trn, true, true, false)));
    }

    /// Only the `_v_undef_` forms read `<0, 0, 1, 1>`: two inputs never do.
    #[test]
    fn test_the_one_input_forms_are_matched() {
        assert_eq!(kind(&[0, 0, 1, 1]), Some((Kind::Zip, false, false, true)));
        assert_eq!(kind(&[0, 2, 0, 2]), Some((Kind::Uzp, false, false, true)));
        assert_eq!(kind(&[0, 0, 2, 2]), Some((Kind::Trn, false, false, true)));
    }

    #[test]
    fn test_an_ordinary_mask_is_no_permute() {
        assert_eq!(kind(&[0, 1, 2, 3, 4, 5, 6, 7]), None);
        assert_eq!(kind(&[3, 2, 1, 0]), None);
    }
}
