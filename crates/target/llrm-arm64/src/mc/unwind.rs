//! arm64's `.eh_frame` facts (`AArch64MCTargetDesc.cpp`'s initial frame state,
//! the AArch64 DWARF register numbers: `x0`..`x30` are 0..30, `sp` 31 and
//! `v0`..`v31` 64..95).

use llrm_dwarf::eh_frame::{EhTarget, Pointer};
use llrm_object::debug::FrameRow;

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

/// The 32-bit word of Mach-O's `__compact_unwind` for a function with `rows`
/// and its FDE at `fde` of `__eh_frame` (`AArch64AsmBackend::
/// generateCompactUnwindEncoding`): a stack of locals and nothing saved, or the
/// frame record alone below the frame pointer. The saved pairs of the compact
/// frame are stored high register first below the record, and this backend
/// stores the lower one first, so a function that saves more is left to its
/// DWARF frame, as is one whose frame is not made at its start.
pub fn compact_encoding(
    rows: &[FrameRow],
    fde: usize,
) -> u32 {
    let dwarf = MODE_DWARF | (fde as u32 & 0x00FF_FFFF);
    let Some(first) = rows.first() else { return MODE_FRAMELESS };
    if first.offset != 4 {
        return dwarf;
    }
    let (fp, lr) = (("x29".to_owned(), -16), ("x30".to_owned(), -8));
    match rows.iter().find(|row| row.cfa_register == "x29") {
        Some(row)
            if row.cfa_offset == 16 && row.saved.len() == 2 && row.saved.contains(&fp) && row.saved.contains(&lr) =>
        {
            MODE_FRAME
        }
        Some(_) => dwarf,
        None if rows.iter().any(|row| !row.saved.is_empty()) => dwarf,
        None if first.cfa_register == "sp" && first.cfa_offset % 16 == 0 && first.cfa_offset / 16 < 0xFFF => {
            MODE_FRAMELESS | (first.cfa_offset as u32 / 16) << 12
        }
        None => dwarf,
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
            row(4, "sp", 16, &[("x29", -16), ("x30", -8)]),
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

    #[test]
    fn saved_pairs_are_left_to_the_dwarf_frame_at_its_entry() {
        let rows = [row(4, "sp", 32, &[("x19", -32)]), row(8, "x29", 16, &[("x19", -32), ("x29", -16), ("x30", -8)])];
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
