//! x86-64's `.eh_frame` facts (`X86MCTargetDesc.cpp`'s initial frame state, the
//! System V DWARF register numbers).

use llrm_dwarf::eh_frame::EhTarget;
use llrm_object::debug::FrameRow;

const NAMES: [&str; 17] = [
    "rax", "rdx", "rcx", "rbx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15", "rip",
];

/// The DWARF number of an x86-64 register by name.
pub fn register(name: &str) -> Option<u64> {
    NAMES.iter().position(|one| *one == name).map(|number| number as u64)
}

/// The name of an x86-64 register by its DWARF number.
pub fn register_name(number: u64) -> Option<&'static str> {
    NAMES.get(number as usize).copied()
}

/// The DWARF numbers of the registers a 64-bit function must preserve, which
/// the prologue's pushes are told apart by (rbp among them).
pub const CALLEE_SAVED: [u16; 6] = [3, 6, 12, 13, 14, 15];

/// Whether functions of `bits`-bit code are given call frame information.
pub fn declares(bits: u32) -> bool {
    bits == 64
}

/// A register's name by its DWARF number, `rip` for any that has none.
pub fn register_label(number: u16) -> String {
    register_name(u64::from(number)).unwrap_or("rip").to_owned()
}

/// A function's frame at its entry: the frame address 8 past the stack
/// pointer.
pub fn entry_row() -> FrameRow {
    eh_target().entry
}

/// At a function's entry the frame address is 8 past the stack pointer and
/// the return address is just below it.
pub fn eh_target() -> EhTarget<'static> {
    EhTarget {
        address: 8,
        return_column: 16,
        entry: FrameRow { offset: 0, cfa_register: "rsp".into(), cfa_offset: 8, saved: Vec::new() },
        register: &register,
    }
}

#[cfg(test)]
mod tests {
    use llrm_object::debug::FrameRow;
    use llrm_object::{Binding, Definition, Frame, Object, ObjectTarget, Reloc, Role, Section, Symbol, Target};

    use super::*;
    use crate::mc::X86Objects;

    fn row(
        offset: usize,
        cfa_offset: i64,
        saved: &[(&str, i64)],
    ) -> FrameRow {
        FrameRow {
            offset,
            cfa_register: "rsp".into(),
            cfa_offset,
            saved: saved.iter().map(|(name, at)| ((*name).to_owned(), *at)).collect(),
        }
    }

    fn object(frames: Vec<Frame>) -> Object {
        Object {
            name: "f.c".into(),
            bits: 64,
            sections: vec![Section {
                name: ".text".into(),
                role: Role::Text,
                near: true,
                align: 16,
                image: vec![0x90; 0x3C],
                spans: vec![[0, 0x3C]],
                relocs: Vec::new(),
            }],
            symbols: vec![Symbol {
                name: "f".into(),
                binding: Binding::Public,
                definition: Definition::Defined { section: 0, offset: 0 },
                size: 0x36,
                group: None,
            }],
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            debug: None,
            frames,
        }
    }

    /// The `.eh_frame` pin clang writes for `x86_64-linux-gnu -O1` of a
    /// function that pushes two registers, subtracts from the stack and
    /// pops them (rows at 2, 3, 7, 50, 51, 53), and a leaf of 4 bytes after
    /// it. Read with `objdump -s -j .eh_frame` and `readelf -r`.
    const PIN: [u8; 0x58] = [
        0x14, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x7A, 0x52, 0, 0x01, 0x78, 0x10, 0x01, 0x1B, 0x0C, 0x07, 0x08, 0x90, 0x01, 0,
        0, 0x24, 0, 0, 0, 0x1C, 0, 0, 0, 0, 0, 0, 0, 0x36, 0, 0, 0, 0x00, 0x42, 0x0E, 0x10, 0x41, 0x0E, 0x18, 0x44,
        0x0E, 0x40, 0x83, 0x03, 0x8E, 0x02, 0x6B, 0x0E, 0x18, 0x41, 0x0E, 0x10, 0x42, 0x0E, 0x08, 0x00, 0x14, 0, 0, 0,
        0x44, 0, 0, 0, 0, 0, 0, 0, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    /// The two functions of `PIN`: one that pushes two registers, subtracts
    /// from the stack and pops them, and a 4-byte leaf.
    fn both() -> Vec<Frame> {
        let saved = [("rbx", -24), ("r14", -16)];
        vec![
            Frame {
                section: 0,
                offset: 0,
                length: 0x36,
                rows: vec![
                    row(2, 16, &[]),
                    row(3, 24, &[]),
                    row(7, 64, &saved),
                    row(50, 24, &saved),
                    row(51, 16, &saved),
                    row(53, 8, &saved),
                ],
                seh: Vec::new(),
            },
            Frame { section: 0, offset: 0x38, length: 4, rows: Vec::new(), seh: Vec::new() },
        ]
    }

    /// Functions with declared frames had no `.eh_frame` at all, so a C++ or
    /// `-fexceptions` unwinder found nothing: the section is the pin's, byte
    /// for byte, with a PC32 against `.text` for each function.
    #[test]
    fn the_eh_frame_is_the_pins_for_the_same_rows() {
        let frames = both();
        let written = llrm_dwarf::eh_frame::expanded(&object(frames), &eh_target()).unwrap();
        let section = written.sections.last().unwrap();
        assert_eq!(section.name, ".eh_frame");
        assert_eq!(section.image, PIN);
        assert_eq!(
            section.relocs,
            vec![
                Reloc::new(0x20, llrm_object::Fix::pc_rel(4, 0), Target::Section(0), 0),
                Reloc::new(0x48, llrm_object::Fix::pc_rel(4, 0), Target::Section(0), 0x38),
            ]
        );
        assert!(written.frames.is_empty());
        // The written file carries the section: its type is SHT_X86_64_UNWIND
        // and its bytes are the pin's.
        let file = X86Objects.write_object("elf", &object(both())).unwrap();
        assert!(file.windows(4).any(|word| word == 0x7000_0001u32.to_le_bytes()));
        assert!(file.windows(PIN.len()).any(|bytes| bytes == PIN));
    }

    /// Pin clang's `-O1 -fomit-frame-pointer` of a function that pushes rbx,
    /// subtracts 16 and leaves by two tail calls: the second exit re-declares
    /// `def_cfa_offset 32` where the first left the frame, and no
    /// `remember_state`/`restore_state` is written (`readelf`/`objdump` of
    /// the pin's `.eh_frame`).
    #[test]
    fn two_exits_by_tail_call_need_no_remembered_state() {
        const PIN: [u8; 0x40] = [
            0x14, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x7A, 0x52, 0, 0x01, 0x78, 0x10, 0x01, 0x1B, 0x0C, 0x07, 0x08, 0x90, 0x01,
            0, 0, 0x24, 0, 0, 0, 0x1C, 0, 0, 0, 0, 0, 0, 0, 0x33, 0, 0, 0, 0x00, 0x41, 0x0E, 0x10, 0x44, 0x0E, 0x20,
            0x83, 0x02, 0x56, 0x0E, 0x10, 0x41, 0x0E, 0x08, 0x45, 0x0E, 0x20, 0x4C, 0x0E, 0x10, 0x41, 0x0E, 0x08,
        ];
        let saved = [("rbx", -16)];
        let rows = vec![
            row(1, 16, &[]),
            row(5, 32, &saved),
            row(0x1B, 16, &saved),
            row(0x1C, 8, &saved),
            row(0x21, 32, &saved),
            row(0x2D, 16, &saved),
            row(0x2E, 8, &saved),
        ];
        let frames = vec![Frame { section: 0, offset: 0, length: 0x33, rows, seh: Vec::new() }];
        let written = llrm_dwarf::eh_frame::expanded(&object(frames), &eh_target()).unwrap();
        assert_eq!(written.sections.last().unwrap().image, PIN);
    }

    #[test]
    fn the_registers_have_system_v_numbers() {
        assert_eq!(
            (register("rbx"), register("rbp"), register("rsp"), register("r14"), register("rip")),
            (Some(3), Some(6), Some(7), Some(14), Some(16))
        );
        assert_eq!(register("eax"), None);
    }
}
