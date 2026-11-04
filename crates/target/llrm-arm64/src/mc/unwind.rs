//! arm64's `.eh_frame` facts (`AArch64MCTargetDesc.cpp`'s initial frame state,
//! the AArch64 DWARF register numbers: `x0`..`x30` are 0..30, `sp` 31 and
//! `v0`..`v31` 64..95).

use llrm_debug::FrameRow;
use llrm_dwarf::eh_frame::{EhTarget, Pointer};

/// The DWARF number of the frame pointer, `x29`.
pub const FP: u16 = 29;
/// The DWARF number of the link register, `x30`.
pub const LR: u16 = 30;
/// The DWARF number of the stack pointer.
pub const SP: u16 = 31;
const V0: u16 = 64;

/// A register's name by its DWARF number.
pub fn register_label(number: u16) -> String {
    match number {
        0..=30 => format!("x{number}"),
        SP => "sp".to_owned(),
        _ => format!("d{}", number - V0),
    }
}

/// The DWARF number of a register by name.
pub fn register(name: &str) -> Option<u64> {
    match name {
        "sp" => Some(u64::from(SP)),
        _ => {
            let (class, index) = name.split_at(1);
            let index: u16 = index.parse().ok()?;
            match class {
                "x" if index <= 30 => Some(u64::from(index)),
                "d" if index <= 31 => Some(u64::from(V0 + index)),
                _ => None,
            }
        }
    }
}

const MODE_FRAMELESS: u32 = 0x0200_0000;
const MODE_DWARF: u32 = 0x0300_0000;
const MODE_FRAME: u32 = 0x0400_0000;

/// The flag of a saved pair in the compact frame's word, by its low register
/// (`x19` for `x19`/`x20`, `d8` for `d8`/`d9`), and the flags that may not be
/// set yet: the pairs go in register number order, `x` before `d`.
fn pair_flag(
    low: &str,
    high: &str,
    set: u32,
) -> Option<u32> {
    const PAIRS: [(&str, &str, u32, u32); 9] = [
        ("x19", "x20", 0x001, 0xF1E),
        ("x21", "x22", 0x002, 0xF1C),
        ("x23", "x24", 0x004, 0xF18),
        ("x25", "x26", 0x008, 0xF10),
        ("x27", "x28", 0x010, 0xF00),
        ("d8", "d9", 0x100, 0xE00),
        ("d10", "d11", 0x200, 0xC00),
        ("d12", "d13", 0x400, 0x800),
        ("d14", "d15", 0x800, 0),
    ];
    PAIRS.iter().find(|(a, b, _, blocked)| (*a, *b) == (low, high) && set & blocked == 0).map(|pair| pair.2)
}

/// The 32-bit word of Mach-O's `__compact_unwind` for a function with `rows`
/// and its FDE at `fde` of `__eh_frame` (`AArch64AsmBackend::
/// generateCompactUnwindEncoding`). A frame is the frame record, then saved
/// pairs below it, each at the next two words down, and in the flags; frameless
/// is the stack's size in sixteenths. Anything else is left to the DWARF frame.
pub fn compact_encoding(
    rows: &[FrameRow],
    fde: usize,
) -> u32 {
    let dwarf = MODE_DWARF | (fde as u32 & 0x00FF_FFFF);
    let Some(first) = rows.first() else { return MODE_FRAMELESS };
    // The saves, highest address first: the order they are stated in.
    let chain = |row: &FrameRow| {
        let mut saved = row.saved.clone();
        saved.sort_by_key(|(_, at)| std::cmp::Reverse(*at));
        saved
    };
    // The saved pairs of `saved`, each word 8 below the one before from `at`
    // (0 where nothing was stated yet).
    let flags = |saved: &[(String, i64)], mut at: i64| -> Option<u32> {
        let mut set = 0;
        for pair in saved.chunks(2) {
            let [(low, one), (high, two)] = pair else { return None };
            if at != 0 && *one != at - 8 || *two != one - 8 {
                return None;
            }
            at = *two;
            set |= pair_flag(low, high, set)?;
        }
        Some(set)
    };
    let Some(at) = rows.iter().position(|row| row.cfa_register == "x29") else {
        // A stack of locals. Saves in it are not a word the unwinder can
        // read (the pin's own words for them are DWARF's too).
        let size = first.cfa_offset;
        let saves = rows.iter().any(|row| row.cfa_register == "sp" && row.cfa_offset == size && !row.saved.is_empty());
        return if !saves && first.cfa_register == "sp" && size % 16 == 0 && size / 16 < 0x1000 {
            MODE_FRAMELESS | (size as u32 / 16) << 12
        } else {
            dwarf
        };
    };
    // One stack adjustment before the frame register, and the saves stated with
    // it.
    if at > 1 || rows[..at].iter().any(|row| row.cfa_register != "sp" || !row.saved.is_empty()) {
        return dwarf;
    }
    let saved = chain(&rows[at]);
    match saved.as_slice() {
        [(lr, link), (fp, frame), rest @ ..] if (lr.as_str(), fp.as_str()) == ("x30", "x29") && frame + 8 == *link => {
            flags(rest, *frame).map_or(dwarf, |set| MODE_FRAME | set)
        }
        _ => dwarf,
    }
}

/// At a function's entry the frame address is the stack pointer and the return
/// address is in `x30`.
pub fn eh_target() -> EhTarget<'static> {
    EhTarget {
        pointer: Pointer::Pcrel4,
        address: 8,
        return_column: u64::from(LR),
        return_saved: false,
        code_alignment: 4,
        entry: FrameRow { offset: 0, cfa_register: "sp".into(), cfa_offset: 0, saved: Vec::new() },
        register: &register,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        offset: usize,
        register: &str,
        cfa: i64,
        saved: &[(&str, i64)],
    ) -> FrameRow {
        FrameRow {
            offset,
            cfa_register: register.into(),
            cfa_offset: cfa,
            saved: saved.iter().map(|(name, at)| ((*name).to_owned(), *at)).collect(),
        }
    }

    #[test]
    fn a_frame_record_alone_is_the_compact_frame() {
        let rows = [
            row(4, "sp", 16, &[]),
            row(8, "x29", 16, &[("x29", -16), ("x30", -8)]),
            row(20, "sp", 16, &[("x29", -16), ("x30", -8)]),
            row(24, "sp", 0, &[]),
        ];
        assert_eq!(compact_encoding(&rows, 0x40), 0x0400_0000);
    }

    #[test]
    fn locals_alone_are_a_frameless_stack_of_sixteenths() {
        assert_eq!(compact_encoding(&[row(4, "sp", 48, &[]), row(12, "sp", 0, &[])], 0), 0x0200_3000);
        assert_eq!(compact_encoding(&[], 0), 0x0200_0000);
    }

    fn frame(saved: &[(&str, i64)]) -> [FrameRow; 2] {
        [row(4, "sp", 48, &[]), row(12, "x29", 16, saved)]
    }

    /// The saved pairs are in the flags (the pin's `0x04000001` for `x19`,
    /// `x20`): they went to the DWARF frame, an entry per function the
    /// unwinder reads slowly.
    #[test]
    fn saved_pairs_are_flags_of_the_compact_frame() {
        let record = [("x30", -8), ("x29", -16)];
        let with = |more: &[(&str, i64)]| frame(&[&record[..], more].concat());
        assert_eq!(compact_encoding(&with(&[("x19", -24), ("x20", -32)]), 0), 0x0400_0001);
        assert_eq!(compact_encoding(&with(&[("x19", -24), ("x20", -32), ("x21", -40), ("x22", -48)]), 0), 0x0400_0003);
        assert_eq!(compact_encoding(&with(&[("x23", -24), ("x24", -32), ("d8", -40), ("d9", -48)]), 0), 0x0400_0104);
    }

    #[test]
    fn a_save_the_compact_frame_cannot_describe_is_left_to_dwarf() {
        let record = [("x30", -8), ("x29", -16)];
        let with = |more: &[(&str, i64)]| frame(&[&record[..], more].concat());
        // A hole between the record and the pair, a lone register, a pair
        // out of register order.
        assert_eq!(compact_encoding(&with(&[("x19", -32), ("x20", -40)]), 0x1c), 0x0300_001c);
        assert_eq!(compact_encoding(&with(&[("x19", -24)]), 0x1c), 0x0300_001c);
        assert_eq!(compact_encoding(&with(&[("d8", -24), ("d9", -32), ("x19", -40), ("x20", -48)]), 0x1c), 0x0300_001c);
    }

    #[test]
    fn a_frame_whose_record_is_not_the_top_save_is_left_to_dwarf() {
        let rows = frame(&[("x19", -8), ("x30", -16), ("x29", -24)]);
        assert_eq!(compact_encoding(&rows, 0x1c), 0x0300_001c);
    }

    #[test]
    fn registers_saved_without_a_frame_are_left_to_dwarf() {
        let rows = [row(4, "sp", 32, &[]), row(12, "sp", 32, &[("x19", -24), ("x20", -32)])];
        assert_eq!(compact_encoding(&rows, 0x1c), 0x0300_001c);
    }

    #[test]
    fn registers_round_trip_through_their_dwarf_numbers() {
        for number in (0..=SP).chain(V0 + 8..V0 + 16) {
            assert_eq!(register(&register_label(number)), Some(u64::from(number)));
        }
        assert_eq!(register("x31"), None);
    }
}
