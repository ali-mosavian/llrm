//! An [`Object`] as an ELF64 relocatable file for x86-64, RELA relocations (the
//! addend is in the entry), as GNU ld and lld take them. The container is
//! `llrm-elf`'s.

use llrm_elf::{Machine, Relocation};
use llrm_object::{FixupKind, Object, Reloc, Unsupported};

use super::fixup;

const R_X86_64_64: u32 = 1;
const R_X86_64_PC32: u32 = 2;
const R_X86_64_PLT32: u32 = 4;
const R_X86_64_32: u32 = 10;
const R_X86_64_16: u32 = 12;
const R_X86_64_PC16: u32 = 13;
const R_X86_64_8: u32 = 14;
const R_X86_64_PC8: u32 = 15;
const R_X86_64_PC64: u32 = 24;
const R_X86_64_REX_GOTPCRELX: u32 = 42;

struct X8664;

impl Machine for X8664 {
    const BITS: u8 = 64;
    const WIDE: bool = true;
    const NUMBER: u16 = 62;
    const ADDEND: bool = true;
    /// `SHT_X86_64_UNWIND`.
    const UNWIND_TYPE: u32 = 0x7000_0001;

    fn backend() -> &'static dyn llrm_object::assembler::AsmBackend {
        &super::backend::FIELDS
    }

    fn relocation(reloc: &Reloc) -> Result<Relocation, Unsupported> {
        let (kind, width) = match (reloc.kind, reloc.pc_rel) {
            (FixupKind::DATA_8, false) => (R_X86_64_64, 8),
            // Zero-extended: a signed 32-bit absolute address has its own type,
            // which no encoder asks for yet.
            (FixupKind::DATA_4, false) => (R_X86_64_32, 4),
            (FixupKind::DATA_2, false) => (R_X86_64_16, 2),
            (FixupKind::DATA_1, false) => (R_X86_64_8, 1),
            (FixupKind::DATA_8, true) => (R_X86_64_PC64, 8),
            (FixupKind::DATA_4, true) => (R_X86_64_PC32, 4),
            (fixup::BRANCH_4, true) => (R_X86_64_PLT32, 4),
            (fixup::GOT_PCREL_4, _) => (R_X86_64_REX_GOTPCRELX, 4),
            (FixupKind::DATA_2, true) => (R_X86_64_PC16, 2),
            (FixupKind::DATA_1, true) => (R_X86_64_PC8, 1),
            (fixup::SEGMENT_BASE, _) => return Err(Unsupported("a segment selector has no ELF relocation".into())),
            (fixup::FAR_POINTER, _) => return Err(Unsupported("a 16:16 far pointer has no ELF relocation".into())),
            other => return Err(Unsupported(format!("ELF x86-64 has no relocation for {other:?}"))),
        };
        Ok(Relocation { kind, width })
    }
}

/// `object` as an ELF64 relocatable file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_elf::write::<X8664>(object)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use llrm_debug::DebugObjectTarget;
    use llrm_object::{Binding, Definition, Fix, Reloc, Role, Section, Symbol, Target};

    use super::*;

    fn section(
        name: &str,
        role: Role,
        image: Vec<u8>,
        relocs: Vec<Reloc>,
    ) -> Section {
        let spans = vec![[0, image.len()]];
        Section { name: name.into(), role, near: true, align: 8, image, spans, relocs }
    }

    fn symbol(
        name: &str,
        binding: Binding,
        section: usize,
        offset: usize,
    ) -> Symbol {
        Symbol { name: name.into(), binding, definition: Definition::Defined { section, offset }, size: 0, group: None }
    }

    fn word(
        bytes: &[u8],
        at: usize,
    ) -> u64 {
        u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
    }

    /// `_start` loads a pointer from `.data` rip-relative (R_X86_64_PC32
    /// against a local symbol) and writes what it points at (R_X86_64_64
    /// with an addend), then exits: GNU ld links it, and it runs.
    fn hello() -> Object {
        let load = Reloc::new(3, Fix::pc_rel(4, 4), Target::Symbol(2), 0);
        let text = [
            vec![0x48, 0x8B, 0x35, 0, 0, 0, 0], // mov ptr(%rip), %rsi
            vec![0xBF, 1, 0, 0, 0],             // mov $1, %edi
            vec![0xB8, 1, 0, 0, 0],             // mov $1, %eax
            vec![0xBA, 5, 0, 0, 0],             // mov $5, %edx
            vec![0x0F, 0x05],                   // syscall
            vec![0x31, 0xFF],                   // xor %edi, %edi
            vec![0xB8, 0x3C, 0, 0, 0],          // mov $60, %eax
            vec![0x0F, 0x05],                   // syscall
        ]
        .concat();
        let pointer = Reloc::new(0, Fix::abs(8), Target::Symbol(1), 1);
        Object {
            name: "hello.s".into(),
            bits: 64,
            sections: vec![
                section(".text", Role::Text, text, vec![load]),
                section(".rodata", Role::ROData, b"hello\n".to_vec(), vec![]),
                section(".data", Role::Data, vec![0; 8], vec![pointer]),
            ],
            symbols: vec![
                symbol("_start", Binding::Public, 0, 0),
                symbol("msg", Binding::Public, 1, 0),
                symbol("ptr", Binding::Local, 2, 0),
            ],
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        }
    }

    /// RELA keeps the addend in the entry and leaves the field zero: the entry
    /// is 24 bytes, its addend minus 4 for a pc-relative field, and
    /// `.rela.text` links the symbol table.
    #[test]
    fn a_rela_entry_carries_the_addend_and_the_field_stays_zero() {
        let bytes = write(&hello()).unwrap();
        assert_eq!((bytes[4], u16::from_le_bytes([bytes[18], bytes[19]])), (2, 62));
        let table = word(&bytes, 40) as usize;
        let count = u16::from_le_bytes([bytes[60], bytes[61]]) as usize;
        let header = |index: usize| table + 64 * index;
        let named = |name: &str| {
            let strings = word(&bytes, header(u16::from_le_bytes([bytes[62], bytes[63]]) as usize) + 24) as usize;
            (0..count).find(|&index| {
                let at =
                    strings + u32::from_le_bytes(bytes[header(index)..header(index) + 4].try_into().unwrap()) as usize;
                bytes[at..].starts_with(name.as_bytes()) && bytes[at + name.len()] == 0
            })
        };
        let rela = header(named(".rela.text").expect(".rela.text"));
        assert_eq!((word(&bytes, rela + 32), word(&bytes, rela + 56)), (24, 24));
        let at = word(&bytes, rela + 24) as usize;
        assert_eq!((word(&bytes, at), word(&bytes, at + 8) & 0xFFFF_FFFF, word(&bytes, at + 16) as i64), (3, 2, -4));
        let text = word(&bytes, header(named(".text").unwrap()) + 24) as usize;
        assert_eq!(&bytes[text + 3..text + 7], [0, 0, 0, 0]);
        let data = rela_of(&bytes, ".rela.data", &named, &header);
        assert_eq!((word(&bytes, data + 8) & 0xFFFF_FFFF, word(&bytes, data + 16)), (1, 1));
    }

    fn rela_of(
        bytes: &[u8],
        name: &str,
        named: &dyn Fn(&str) -> Option<usize>,
        header: &dyn Fn(usize) -> usize,
    ) -> usize {
        word(bytes, header(named(name).unwrap()) + 24) as usize
    }

    /// The object, linked by GNU ld, runs and writes what the data's pointer
    /// plus its addend names.
    #[test]
    fn ld_links_it_and_it_runs() {
        if cfg!(not(all(target_os = "linux", target_arch = "x86_64")))
            || Command::new("ld").arg("--version").output().is_err()
        {
            eprintln!("skipped: needs x86-64 Linux and GNU ld");
            return;
        }
        let scratch = tempfile::tempdir().unwrap();
        let (object, program) = (scratch.path().join("hello.o"), scratch.path().join("hello"));
        std::fs::write(&object, write(&hello()).unwrap()).unwrap();
        let linked = Command::new("ld").args(["-static", "-o"]).arg(&program).arg(&object).output().unwrap();
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        for (tool, arguments) in [("readelf", &["-a", "--wide"][..]), ("objdump", &["-dr"][..])] {
            let said = Command::new(tool).args(arguments).arg(&object).output().unwrap();
            let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
            assert!(said.status.success() && !text.contains("Warning") && !text.contains("Error"), "{tool}:\n{text}");
        }
        let ran = Command::new(&program).output().unwrap();
        assert_eq!((ran.status.code(), ran.stdout.as_slice()), (Some(0), &b"ello\n"[..]));
    }

    /// `main` calls `puts` through the PLT (`Kind::Branch`, R_X86_64_PLT32)
    /// with a `.rodata` string by `lea` (R_X86_64_PC32): the system compiler
    /// driver links it against glibc as a PIE, and it prints the string.
    #[test]
    fn gcc_links_a_plt_call_and_a_pc_relative_string_against_glibc() {
        if cfg!(not(all(target_os = "linux", target_arch = "x86_64")))
            || Command::new("gcc").arg("--version").output().is_err()
        {
            eprintln!("skipped: needs x86-64 Linux and gcc");
            return;
        }
        let text = [
            vec![0x48, 0x83, 0xEC, 0x08],       // sub $8, %rsp
            vec![0x48, 0x8D, 0x3D, 0, 0, 0, 0], // lea str(%rip), %rdi
            vec![0xE8, 0, 0, 0, 0],             // call puts@plt
            vec![0x31, 0xC0],                   // xor %eax, %eax
            vec![0x48, 0x83, 0xC4, 0x08],       // add $8, %rsp
            vec![0xC3],                         // ret
        ]
        .concat();
        let relocs = vec![
            Reloc::new(7, Fix::pc_rel(4, 4), Target::Symbol(1), 0),
            Reloc::new(12, fixup::branch(4), Target::Symbol(2), 0),
        ];
        let object = Object {
            name: "puts.s".into(),
            bits: 64,
            sections: vec![
                section(".text", Role::Text, text, relocs),
                section(".rodata", Role::ROData, b"glibc ok\0".to_vec(), vec![]),
            ],
            symbols: vec![
                symbol("main", Binding::Public, 0, 0),
                symbol("str", Binding::Local, 1, 0),
                Symbol {
                    name: "puts".into(),
                    binding: Binding::Public,
                    definition: Definition::Undefined,
                    size: 0,
                    group: None,
                },
            ],
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        };
        let scratch = tempfile::tempdir().unwrap();
        let (file, program) = (scratch.path().join("puts.o"), scratch.path().join("puts"));
        std::fs::write(&file, write(&object).unwrap()).unwrap();
        let linked = Command::new("gcc").arg(&file).arg("-o").arg(&program).output().unwrap();
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        let ran = Command::new(&program).output().unwrap();
        assert_eq!((ran.status.code(), ran.stdout.as_slice()), (Some(0), &b"glibc ok\n"[..]));
    }

    /// An OMF-only reference is refused whatever the machine.
    #[test]
    fn a_segment_selector_is_refused() {
        let mut made = hello();
        made.sections[0].relocs[0].kind = fixup::SEGMENT_BASE;
        assert!(write(&made).is_err());
    }

    /// An object that asks for CodeView or Turbo Debugger information is
    /// refused by name: the information of another format is never written
    /// in its place.
    #[test]
    fn a_debug_format_this_object_cannot_carry_is_refused() {
        for (format, name) in
            [(llrm_debug::Format::CodeView, "CodeView"), (llrm_debug::Format::TurboDebugger, "Turbo Debugger")]
        {
            let made = hello();
            let debug = Some(llrm_debug::Info {
                options: llrm_debug::DebugOptions { format, ..Default::default() },
                ..Default::default()
            });
            let why = crate::mc::X86Objects.write_described("elf", &made, debug.as_ref()).unwrap_err().0;
            assert!(why.contains("cannot carry") && why.contains(name), "{why}");
        }
    }
}
