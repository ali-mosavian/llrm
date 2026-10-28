//! x86-64's `.eh_frame` facts (`X86MCTargetDesc.cpp`'s initial frame state, the
//! System V DWARF register numbers).

use llrm_dwarf::eh_frame::{EhTarget, Pointer};
use llrm_object::assembler::AsmBackend;
use llrm_object::debug::FrameRow;
use llrm_object::{Fix, Object, Reloc, Role, Section, Target, Unsupported};

use super::backend::X86Backend;

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

/// At a function's entry the frame address is 8 past the stack pointer and
/// the return address is just below it.
pub fn eh_target() -> EhTarget<'static> {
    EhTarget {
        pointer: Pointer::Pcrel4,
        section: ".eh_frame",
        address: 8,
        return_column: 16,
        entry: FrameRow { offset: 0, cfa_register: "rsp".into(), cfa_offset: 8, saved: Vec::new() },
        register: &register,
    }
}

/// Mach-O's `__eh_frame`: the same CIE, but 8-byte FDE pointers the container
/// resolves.
pub fn macho_eh_target() -> EhTarget<'static> {
    EhTarget { pointer: Pointer::Pcrel8, section: "__eh_frame", ..eh_target() }
}

/// `object` with Mach-O's unwind sections for its declared frames:
/// `__compact_unwind`, an entry per function the compact form can say or leave
/// to DWARF, then `__eh_frame`.
pub fn macho_expanded(object: &Object) -> Result<Object, Unsupported> {
    let backend = X86Backend { sixteen: false, wide: true, longest: 10 };
    let mut image = Vec::new();
    let mut relocs = Vec::new();
    for frame in &object.frames {
        let encoding = backend.generate_compact_unwind_encoding(&frame.cfi);
        if encoding == 0 {
            continue;
        }
        relocs.push(Reloc::new(image.len(), Fix::abs(8), Target::Section(frame.section), frame.offset as i64));
        image.extend([0u8; 8]);
        image.extend((frame.length as u32).to_le_bytes());
        image.extend(encoding.to_le_bytes());
        // No personality and no language-specific data area.
        image.extend([0u8; 16]);
    }
    let mut sections = object.sections.clone();
    if !image.is_empty() {
        sections.push(Section {
            name: "__compact_unwind".to_owned(),
            role: Role::Unwind,
            near: true,
            align: 8,
            spans: vec![[0, image.len()]],
            image,
            relocs,
        });
    }
    llrm_dwarf::eh_frame::expanded(&Object { sections, ..object.clone() }, &macho_eh_target())
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
                cfi: Vec::new(),
                seh: Vec::new(),
            },
            Frame { section: 0, offset: 0x38, length: 4, rows: Vec::new(), cfi: Vec::new(), seh: Vec::new() },
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
        let frames = vec![Frame { section: 0, offset: 0, length: 0x33, rows, cfi: Vec::new(), seh: Vec::new() }];
        let written = llrm_dwarf::eh_frame::expanded(&object(frames), &eh_target()).unwrap();
        assert_eq!(written.sections.last().unwrap().image, PIN);
    }

    /// Pin clang's `x86_64-apple-macos11 -O1` of `f` in 62 bytes that keeps
    /// rbx in a frame of rbp: `__compact_unwind` (one entry, encoding
    /// 0x01010001) and `__eh_frame` (CIE with 8-byte pc-relative FDE pointers;
    /// the FDE's is -0x80, the text at 0 and the section at 0x60).
    #[test]
    fn the_mach_o_unwind_sections_are_the_pins() {
        use llrm_object::assembler::Cfi;
        const COMPACT: [u8; 32] = [
            0, 0, 0, 0, 0, 0, 0, 0, 0x3E, 0, 0, 0, 0x01, 0x00, 0x01, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0,
        ];
        const EH: [u8; 64] = [
            0x14, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x7A, 0x52, 0, 0x01, 0x78, 0x10, 0x01, 0x10, 0x0C, 0x07, 0x08, 0x90, 0x01,
            0, 0, 0x24, 0, 0, 0, 0x1C, 0, 0, 0, 0x80, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x3E, 0, 0, 0, 0, 0, 0,
            0, 0x00, 0x41, 0x0E, 0x10, 0x86, 0x02, 0x43, 0x0D, 0x06, 0x45, 0x83, 0x03, 0, 0, 0, 0,
        ];
        let frame = Frame {
            section: 0,
            offset: 0,
            length: 0x3E,
            rows: vec![
                row(1, 16, &[("rbp", -16)]),
                FrameRow { cfa_register: "rbp".into(), ..row(4, 16, &[("rbp", -16)]) },
                FrameRow { cfa_register: "rbp".into(), ..row(9, 16, &[("rbp", -16), ("rbx", -24)]) },
            ],
            cfi: vec![
                Cfi::DefCfaOffset { offset: 16 },
                Cfi::Offset { register: 6, offset: -16 },
                Cfi::DefCfaRegister { register: 6 },
                Cfi::Offset { register: 3, offset: -24 },
            ],
        };
        let mut made = object(vec![frame]);
        made.sections[0].image = vec![0x90; 0x3E];
        made.sections[0].spans = vec![[0, 0x3E]];
        let written = macho_expanded(&made).unwrap();
        let [_, compact, eh] = &written.sections[..] else { panic!("{} sections", written.sections.len()) };
        assert_eq!((compact.name.as_str(), compact.image.as_slice()), ("__compact_unwind", &COMPACT[..]));
        assert_eq!(compact.relocs, vec![Reloc::new(0, Fix::abs(8), Target::Section(0), 0)]);
        assert_eq!((eh.name.as_str(), eh.image.len()), ("__eh_frame", 64));
        // The FDE's pointer is a pc-relative reference to the text, which the
        // Mach-O writer resolves; everything else is the pin's already.
        let file = X86Objects.write_object("macho", &made).unwrap();
        assert!(file.windows(COMPACT.len()).any(|bytes| bytes == COMPACT));
        assert!(file.windows(EH.len()).any(|bytes| bytes == EH));
    }

    /// A frame whose CFA moves to a register the compact form has no slot for
    /// is `0x04000000` (DWARF) with the FDE in `__eh_frame`, as the pin writes
    /// it; the linker, not the object, fills in the FDE's offset.
    #[test]
    fn a_frame_the_compact_form_cannot_say_keeps_its_fde() {
        use llrm_object::assembler::Cfi;
        let compact = "00000000000000000b0000000000000400000000000000000000000000000000";
        let eh = "1400000000000000017a520001781001100c070890010000240000001c000000b0ffffffffffffff0b0000000000000000420e108c02430d0c450c0708000000";
        let hex = |text: &str| -> Vec<u8> {
            (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
        };
        let frame = Frame {
            section: 0,
            offset: 0,
            length: 0x0b,
            rows: vec![
                row(2, 16, &[("r12", -16)]),
                FrameRow { cfa_register: "r12".into(), ..row(5, 16, &[("r12", -16)]) },
                row(10, 8, &[("r12", -16)]),
            ],
            cfi: vec![
                Cfi::DefCfaOffset { offset: 16 },
                Cfi::Offset { register: 12, offset: -16 },
                Cfi::DefCfaRegister { register: 12 },
                Cfi::DefCfa { register: 7, offset: 8 },
            ],
        };
        let written = macho_expanded(&object(vec![frame.clone()])).unwrap();
        let [_, unwind, _] = &written.sections[..] else { panic!("{} sections", written.sections.len()) };
        assert_eq!(unwind.image, hex(compact));
        // Everything but the FDE's pointer, which the Mach-O writer resolves
        // from the sections' addresses.
        let file = X86Objects.write_object("macho", &object(vec![frame])).unwrap();
        let eh = hex(eh);
        assert!(file.windows(64).any(|bytes| bytes[..32] == eh[..32] && bytes[40..] == eh[40..]));
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
