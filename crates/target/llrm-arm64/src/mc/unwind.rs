//! arm64's `.eh_frame` facts (`AArch64MCTargetDesc.cpp`'s initial frame state,
//! the AArch64 DWARF register numbers: `x0`..`x30` are 0..30, `sp` 31 and
//! `v0`..`v31` 64..95).

use llrm_debug::FrameRow;
use llrm_dwarf::eh_frame::{EhTarget, Pointer};
use llrm_object::assembler::Cfi;

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

/// The flag of a saved pair in the compact word, by its registers (DWARF
/// numbers) and the flags that may not be set yet: the pairs go in register
/// number order, `x` before `d`.
fn pair_flag(
    first: u16,
    second: u16,
    set: u32,
) -> Option<u32> {
    const D0: u16 = 64;
    const PAIRS: [(u16, u16, u32, u32); 9] = [
        (19, 20, 0x001, 0xF1E),
        (21, 22, 0x002, 0xF1C),
        (23, 24, 0x004, 0xF18),
        (25, 26, 0x008, 0xF10),
        (27, 28, 0x010, 0xF00),
        (D0 + 8, D0 + 9, 0x100, 0xE00),
        (D0 + 10, D0 + 11, 0x200, 0xC00),
        (D0 + 12, D0 + 13, 0x400, 0x800),
        (D0 + 14, D0 + 15, 0x800, 0),
    ];
    PAIRS.iter().find(|pair| (pair.0, pair.1) == (first, second) && set & pair.3 == 0).map(|pair| pair.2)
}

/// Whether the word leaves the function to its DWARF frame.
pub fn is_dwarf(encoding: u32) -> bool {
    encoding & 0x0F00_0000 == MODE_DWARF
}

/// The 32-bit word of Mach-O's `__compact_unwind` for a function that stated
/// `cfi`, in order (`AArch64AsmBackend::generateCompactUnwindEncoding`, step
/// for step). The DWARF mode names no FDE: the linker finds it (the pin's
/// object states `0x03000000`). A
/// frame is the frame record, then saved pairs below it; frameless is the
/// stack's size in sixteenths. Any directive it does not know, in any place,
/// leaves the function to the DWARF frame: an epilogue's `def_cfa sp` or
/// `def_cfa_offset 0` does.
pub fn compact_encoding(cfi: &[Cfi]) -> u32 {
    let dwarf = MODE_DWARF;
    if cfi.is_empty() {
        return MODE_FRAMELESS;
    }
    let (mut frame, mut stack, mut word, mut at) = (false, 0u64, 0u32, 0i64);
    let mut index = 0;
    while index < cfi.len() {
        match cfi[index] {
            Cfi::DefCfa { register, .. } => {
                // The frame pointer is the only frame register compact unwind
                // has, with the link register and the pointer saved next.
                let [
                    _,
                    Cfi::Offset { register: link, offset: link_at },
                    Cfi::Offset { register: pointer, offset: pointer_at },
                    ..,
                ] = cfi[index..]
                else {
                    return dwarf;
                };
                if register != FP || pointer_at + 8 != link_at || (link, pointer) != (LR, FP) {
                    return dwarf;
                }
                at = pointer_at;
                word |= MODE_FRAME;
                frame = true;
                index += 2;
            }
            Cfi::DefCfaOffset { offset } => {
                if stack != 0 {
                    return dwarf;
                }
                stack = offset.unsigned_abs();
            }
            Cfi::Offset { register: first, offset: first_at } => {
                // Saves go in consecutive pairs, each a word below the last.
                let Some(Cfi::Offset { register: second, offset: second_at }) = cfi.get(index + 1).copied() else {
                    return dwarf;
                };
                if at != 0 && first_at != at - 8 || second_at != first_at - 8 {
                    return dwarf;
                }
                at = second_at;
                let Some(flag) = pair_flag(first, second, word) else { return dwarf };
                word |= flag;
                index += 1;
            }
            Cfi::DefCfaRegister { .. } | Cfi::Restore { .. } => return dwarf,
        }
        index += 1;
    }
    if !frame {
        // Compact unwind states a stack of at most 65520 bytes.
        if stack > 65520 {
            return dwarf;
        }
        word |= MODE_FRAMELESS | (stack as u32 / 16) << 12;
    }
    word
}

/// At a function's entry the frame address is the stack pointer and the return
/// address is in `x30`. The CIE counts code in bytes and saves in slots of the
/// pin's `getCalleeSaveStackSlotSize`: 4 on ELF (the `MCAsmInfo` default),
/// where an FDE names its code by a 4-byte offset, and 8 on Mach-O, by 8.
/// Code alignment 1: `MinInstAlignment`
/// (https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/MC/MCDwarf.cpp#L1958,
/// default
/// `include/llvm/MC/MCAsmInfo.h#L122`). Slot size: `getDataAlignmentFactor`
/// (https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/MC/MCDwarf.cpp#L1340),
/// the `MCAsmInfo.h#L90` default of 4, and 8 for Darwin at
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/MCTargetDesc/AArch64MCAsmInfo.cpp#L156.
pub fn eh_target(darwin: bool) -> EhTarget<'static> {
    EhTarget {
        pointer: if darwin { Pointer::Pcrel8 } else { Pointer::Pcrel4 },
        address: 8,
        slot: if darwin { 8 } else { 4 },
        return_column: u64::from(LR),
        return_saved: false,
        code_alignment: 1,
        entry: FrameRow { offset: 0, cfa_register: "sp".into(), cfa_offset: 0, saved: Vec::new() },
        register: &register,
        def_cfa_on_register_change: true,
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

    /// `mov x29, sp` after `stp x29, x30, [sp, #-16]!` was
    /// `DW_CFA_def_cfa_register x29` (2 bytes); the pin writes
    /// `.cfi_def_cfa w29, 16`, `DW_CFA_def_cfa` (3): 30 of 77 same-size
    /// FDEs of the bench differed by it.
    #[test]
    fn a_new_frame_register_is_written_with_its_offset_as_the_pin_writes_it() {
        use llrm_object::{Frame, Object, Role, Section};
        let rows =
            vec![row(4, "sp", 16, &[("x29", -16), ("x30", -8)]), row(8, "x29", 16, &[("x29", -16), ("x30", -8)])];
        let object = Object {
            name: "f.c".into(),
            bits: 64,
            sections: vec![Section {
                name: ".text".into(),
                role: Role::Text,
                near: true,
                align: 4,
                image: vec![0; 16],
                spans: vec![[0, 16]],
                relocs: Vec::new(),
            }],
            symbols: Vec::new(),
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            comdats: Vec::new(),
            frames: vec![Frame { section: 0, offset: 0, length: 16, rows, cfi: Vec::new(), seh: Vec::new() }],
        };
        let written = llrm_dwarf::eh_frame::expanded(&object, &eh_target(false)).unwrap();
        let image = &written.sections.last().unwrap().image;
        let fde = image.windows(3).any(|bytes| bytes == [0x0C, 29, 16]);
        assert!(fde, "no DW_CFA_def_cfa x29, 16 in {image:x?}");
    }

    fn offset(
        register: u16,
        offset: i64,
    ) -> Cfi {
        Cfi::Offset { register, offset }
    }

    /// The prologue of a frame record, step by step (`uwtable`).
    fn record_steps() -> Vec<Cfi> {
        vec![
            Cfi::DefCfaOffset { offset: 16 },
            Cfi::DefCfa { register: FP, offset: 16 },
            offset(LR, -8),
            offset(FP, -16),
        ]
    }

    /// The same, stated once after the prologue (`uwtable(sync)`).
    fn record() -> Vec<Cfi> {
        record_steps().split_off(1)
    }

    #[test]
    fn a_frame_record_alone_is_the_compact_frame() {
        assert_eq!(compact_encoding(&record()), 0x0400_0000);
    }

    /// The epilogue's `def_cfa sp` and `def_cfa_offset 0` are directives the
    /// pin's encoder does not take: an asynchronous table is DWARF's (the
    /// rows' encoder gave the frame word).
    #[test]
    fn an_asynchronous_frame_is_left_to_dwarf() {
        let mut steps = record_steps();
        assert_eq!(compact_encoding(&steps), 0x0400_0000, "the prologue alone is a frame");
        steps.extend([Cfi::DefCfa { register: SP, offset: 16 }, Cfi::DefCfaOffset { offset: 0 }]);
        assert_eq!(compact_encoding(&steps), 0x0300_0000);
    }

    #[test]
    fn locals_alone_are_a_frameless_stack_of_sixteenths() {
        assert_eq!(compact_encoding(&[Cfi::DefCfaOffset { offset: 48 }]), 0x0200_3000);
        assert_eq!(compact_encoding(&[]), 0x0200_0000);
    }

    /// The saved pairs are in the flags (the pin's `0x04000001` for `x19`,
    /// `x20`): they went to the DWARF frame, an entry per function the
    /// unwinder reads slowly.
    #[test]
    fn saved_pairs_are_flags_of_the_compact_frame() {
        let with = |more: &[(u16, i64)]| {
            let mut cfi = record();
            cfi.extend(more.iter().map(|&(register, at)| offset(register, at)));
            cfi
        };
        assert_eq!(compact_encoding(&with(&[(19, -24), (20, -32)])), 0x0400_0001);
        assert_eq!(compact_encoding(&with(&[(19, -24), (20, -32), (21, -40), (22, -48)])), 0x0400_0003);
        assert_eq!(compact_encoding(&with(&[(23, -24), (24, -32), (72, -40), (73, -48)])), 0x0400_0104);
    }

    #[test]
    fn a_save_the_compact_frame_cannot_describe_is_left_to_dwarf() {
        let with = |more: &[(u16, i64)]| {
            let mut cfi = record();
            cfi.extend(more.iter().map(|&(register, at)| offset(register, at)));
            cfi
        };
        // A hole between the record and the pair, a lone register, a pair
        // out of register order.
        assert_eq!(compact_encoding(&with(&[(19, -32), (20, -40)])), 0x0300_0000);
        assert_eq!(compact_encoding(&with(&[(19, -24)])), 0x0300_0000);
        assert_eq!(compact_encoding(&with(&[(72, -24), (73, -32), (19, -40), (20, -48)])), 0x0300_0000);
    }

    #[test]
    fn a_frame_whose_record_is_not_the_top_save_is_left_to_dwarf() {
        let cfi = [Cfi::DefCfa { register: FP, offset: 16 }, offset(19, -8), offset(LR, -16), offset(FP, -24)];
        assert_eq!(compact_encoding(&cfi), 0x0300_0000);
    }

    /// The pin's encoder sets the pair flags of a stack without a frame record
    /// too (the word stays frameless); the rows' encoder left it to DWARF.
    #[test]
    fn pairs_saved_without_a_frame_are_flags_of_the_frameless_word() {
        let cfi = [Cfi::DefCfaOffset { offset: 32 }, offset(19, -24), offset(20, -32)];
        assert_eq!(compact_encoding(&cfi), 0x0200_2001);
    }

    #[test]
    fn registers_round_trip_through_their_dwarf_numbers() {
        for number in (0..=SP).chain(V0 + 8..V0 + 16) {
            assert_eq!(register(&register_label(number)), Some(u64::from(number)));
        }
        assert_eq!(register("x31"), None);
    }
}
