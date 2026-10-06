//! An [`Object`] as an ELF64 relocatable file for x86-64, RELA relocations (the addend is in the
//! entry), as GNU ld and lld take them. The container is `llrm-elf`'s.

use llrm_elf::{Machine, Relocation};
use llrm_object::{Arch, Kind, Object, Unsupported};

const R_X86_64_64: u32 = 1;
const R_X86_64_PC32: u32 = 2;
const R_X86_64_32: u32 = 10;
const R_X86_64_16: u32 = 12;
const R_X86_64_PC16: u32 = 13;
const R_X86_64_8: u32 = 14;
const R_X86_64_PC8: u32 = 15;
const R_X86_64_PC64: u32 = 24;

struct X8664;

impl Machine for X8664 {
    const ARCH: Arch = Arch::X8664;
    const WIDE: bool = true;
    const NUMBER: u16 = 62;
    const ADDEND: bool = true;

    fn relocation(kind: Kind) -> Result<Relocation, Unsupported> {
        let (kind, width, from) = match kind {
            Kind::Abs { width: 8 } => (R_X86_64_64, 8, 0),
            // Zero-extended: a signed 32-bit absolute address has its own type, which no encoder asks for yet.
            Kind::Abs { width: 4 } => (R_X86_64_32, 4, 0),
            Kind::Abs { width: 2 } => (R_X86_64_16, 2, 0),
            Kind::Abs { width: 1 } => (R_X86_64_8, 1, 0),
            Kind::PcRel { width: 8, from } => (R_X86_64_PC64, 8, from),
            Kind::PcRel { width: 4, from } => (R_X86_64_PC32, 4, from),
            Kind::PcRel { width: 2, from } => (R_X86_64_PC16, 2, from),
            Kind::PcRel { width: 1, from } => (R_X86_64_PC8, 1, from),
            Kind::SegmentBase => return Err(Unsupported("a segment selector has no ELF relocation".into())),
            Kind::FarPointer => return Err(Unsupported("a 16:16 far pointer has no ELF relocation".into())),
            other => return Err(Unsupported(format!("ELF x86-64 has no relocation for {other:?}"))),
        };
        Ok(Relocation { kind, width, from })
    }
}

/// `object` as an ELF64 relocatable file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_elf::write::<X8664>(object)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use llrm_object::{Binding, Definition, Reloc, Role, Section, Symbol, Target};

    use super::*;

    fn section(name: &str, role: Role, image: Vec<u8>, relocs: Vec<Reloc>) -> Section {
        let spans = vec![[0, image.len()]];
        Section { name: name.into(), role, near: true, align: 8, image, spans, relocs, lines: Vec::new() }
    }

    fn symbol(name: &str, binding: Binding, section: usize, offset: usize) -> Symbol {
        Symbol { name: name.into(), binding, definition: Definition::Defined { section, offset }, group: None }
    }

    fn word(bytes: &[u8], at: usize) -> u64 {
        u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
    }

    /// `_start` loads a pointer from `.data` rip-relative (R_X86_64_PC32 against a local symbol) and
    /// writes what it points at (R_X86_64_64 with an addend), then exits: GNU ld links it, and it runs.
    fn hello() -> Object {
        let load = Reloc { at: 3, kind: Kind::PcRel { width: 4, from: 4 }, target: Target::Symbol(2), addend: 0 };
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
        let pointer = Reloc { at: 0, kind: Kind::Abs { width: 8 }, target: Target::Symbol(1), addend: 1 };
        Object {
            name: "hello.s".into(),
            arch: Arch::X8664,
            sections: vec![section(".text", Role::Text, text, vec![load]), section(".rodata", Role::ROData, b"hello\n".to_vec(), vec![]), section(".data", Role::Data, vec![0; 8], vec![pointer])],
            symbols: vec![symbol("_start", Binding::Public, 0, 0), symbol("msg", Binding::Public, 1, 0), symbol("ptr", Binding::Local, 2, 0)],
            omf_groups: Vec::new(),
            debug: None,
        }
    }

    /// RELA keeps the addend in the entry and leaves the field zero: the entry is 24 bytes, its
    /// addend minus 4 for a pc-relative field, and `.rela.text` links the symbol table.
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
                let at = strings + u32::from_le_bytes(bytes[header(index)..header(index) + 4].try_into().unwrap()) as usize;
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

    fn rela_of(bytes: &[u8], name: &str, named: &dyn Fn(&str) -> Option<usize>, header: &dyn Fn(usize) -> usize) -> usize {
        word(bytes, header(named(name).unwrap()) + 24) as usize
    }

    /// The object, linked by GNU ld, runs and writes what the data's pointer plus its addend names.
    #[test]
    fn ld_links_it_and_it_runs() {
        if cfg!(not(all(target_os = "linux", target_arch = "x86_64"))) || Command::new("ld").arg("--version").output().is_err() {
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

    /// An OMF-only reference is refused whatever the machine.
    #[test]
    fn a_segment_selector_is_refused() {
        let mut made = hello();
        made.sections[0].relocs[0].kind = Kind::SegmentBase;
        assert!(write(&made).is_err());
    }
}
