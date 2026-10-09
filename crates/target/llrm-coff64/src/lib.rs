//! An [`Object`] as a COFF object for x86-64 (`IMAGE_FILE_MACHINE_AMD64`). The
//! container is `llrm-coff`'s.

use llrm_coff::{Machine, Relocation};
use llrm_object::{Arch, Kind, Object, Unsupported};

const ADDR64: u16 = 1;
const ADDR32: u16 = 2;
const REL32: u16 = 4;
const SECTION: u16 = 0xA;
const SECREL: u16 = 0xB;

struct X8664;

impl Machine for X8664 {
    const ARCH: Arch = Arch::X8664;
    const NUMBER: u16 = 0x8664;

    fn relocation(kind: Kind) -> Result<Relocation, Unsupported> {
        let (kind, width, from, baked) = match kind {
            Kind::Abs { width: 8 } => (ADDR64, 8, 0, 0),
            Kind::Abs { width: 4 } => (ADDR32, 4, 0, 0),
            // REL32_n (types 5 to 9) is relative to the place n bytes past the
            // field's end.
            Kind::PcRel { width: 4, from } if (4..=9).contains(&from) => (REL32 + (from - 4) as u16, 4, from, from),
            Kind::PcRel { width: 4, from } => (REL32, 4, from, 4),
            Kind::Branch { width: 4 } => (REL32, 4, 4, 4),
            Kind::SectionIndex => (SECTION, 2, 0, 0),
            Kind::SectionOffset { width: 4 } => (SECREL, 4, 0, 0),
            Kind::SegmentBase => return Err(Unsupported("a segment selector has no COFF relocation".into())),
            Kind::FarPointer => return Err(Unsupported("a 16:16 far pointer has no COFF relocation".into())),
            other => return Err(Unsupported(format!("COFF x86-64 has no relocation for {other:?}"))),
        };
        Ok(Relocation { kind, width, from, baked })
    }
}

/// `object` as a COFF object file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_coff::write::<X8664>(object)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use llrm_object::{Binding, Definition, Reloc, Role, Section, Symbol, Target};

    use super::*;

    fn text(
        image: Vec<u8>,
        relocs: Vec<Reloc>,
    ) -> Section {
        let spans = vec![[0, image.len()]];
        Section { name: "A_TEXT".into(), role: Role::Text, near: true, align: 16, image, spans, relocs }
    }

    fn object(
        sections: Vec<Section>,
        symbols: Vec<Symbol>,
    ) -> Object {
        Object { name: "a.c".into(), arch: Arch::X8664, sections, symbols, omf_groups: Vec::new(), debug: None }
    }

    fn defined(
        name: &str,
        offset: usize,
    ) -> Symbol {
        Symbol {
            name: name.into(),
            binding: Binding::Public,
            definition: Definition::Defined { section: 0, offset },
            group: None,
        }
    }

    /// (type, field) of the one relocation of `.text`, for a pc-relative field
    /// `from` bytes short of the place.
    fn rel(from: usize) -> (u16, i32) {
        let reloc = Reloc { at: 2, kind: Kind::PcRel { width: 4, from }, target: Target::Symbol(0), addend: 0 };
        let bytes =
            write(&object(vec![text(vec![0x83, 0x3D, 0, 0, 0, 0, 1], vec![reloc])], vec![defined("v", 0)])).unwrap();
        let (raw, relocs) = (
            u32::from_le_bytes(bytes[20 + 20..20 + 24].try_into().unwrap()) as usize,
            u32::from_le_bytes(bytes[20 + 24..20 + 28].try_into().unwrap()) as usize,
        );
        (
            u16::from_le_bytes([bytes[relocs + 8], bytes[relocs + 9]]),
            i32::from_le_bytes(bytes[raw + 2..raw + 6].try_into().unwrap()),
        )
    }

    /// REL32_n is relative to n bytes past the field's end, so an instruction
    /// with an immediate after the field takes the type and leaves the
    /// field zero; plain REL32 there left the reference n bytes off.
    #[test]
    fn a_field_before_an_immediate_is_rel32_n_and_a_field_past_the_end_compensates() {
        assert_eq!(rel(4), (4, 0));
        assert_eq!(rel(5), (5, 0));
        assert_eq!(rel(8), (8, 0));
        assert_eq!(rel(9), (9, 0));
        assert_eq!(rel(10), (4, -6));
    }

    /// lld-link links an x86-64 object: `start` calls `f` and the call reaches
    /// it.
    #[test]
    fn lld_link_links_it_and_the_call_reaches_f() {
        let (Ok(link), Ok(dump)) = (which("lld-link"), which("llvm-objdump")) else {
            eprintln!("skipped: needs lld-link and llvm-objdump");
            return;
        };
        let call = Reloc { at: 1, kind: Kind::Branch { width: 4 }, target: Target::Symbol(1), addend: 0 };
        let made = object(
            vec![text(vec![0xE8, 0, 0, 0, 0, 0xC3, 0x90, 0x90, 0xC3], vec![call])],
            vec![defined("start", 0), defined("f", 8)],
        );
        let scratch = tempfile::tempdir().unwrap();
        let (obj, exe) = (scratch.path().join("a.obj"), scratch.path().join("a.exe"));
        std::fs::write(&obj, write(&made).unwrap()).unwrap();
        let linked = Command::new(link)
            .args([
                "/machine:x64",
                "/subsystem:console",
                "/entry:start",
                "/nodefaultlib",
                "/fixed",
                "/base:0x140000000",
            ])
            .arg(format!("/out:{}", exe.display()))
            .arg(&obj)
            .output()
            .unwrap();
        assert!(
            linked.status.success(),
            "{}{}",
            String::from_utf8_lossy(&linked.stdout),
            String::from_utf8_lossy(&linked.stderr)
        );
        let said = Command::new(dump).args(["-d", "--no-show-raw-insn"]).arg(&exe).output().unwrap();
        assert!(
            String::from_utf8_lossy(&said.stdout).contains("0x140001008"),
            "{}",
            String::from_utf8_lossy(&said.stdout)
        );
    }

    /// An x86-64 object's C13 names the AMD64 machine and an 8-byte pointer,
    /// and RBP (334) is its frame: llvm-readobj reads them back.
    #[test]
    fn debug_information_names_the_amd64_machine_and_eight_byte_pointers() {
        use llrm_object::debug::{Function, Info, Kind as K, Location, Range, Reach, Scalar, Type, Variable};
        let Ok(dump) = which("llvm-readobj") else { return eprintln!("skipped: no llvm-readobj") };
        let info = Info {
            frame_register: "rbp".into(),
            registers: vec![llrm_object::debug::Register {
                name: "rbp".into(),
                bits: 64,
                dwarf: Some(6),
                codeview: Some(334),
            }],
            code: vec![Range { section: 0, offset: 0, length: 4 }],
            types: vec![
                Type::Scalar(Scalar::Int { bytes: 4, signed: true }),
                Type::Pointer { target: 0, bytes: 8, reach: Reach::Near },
                Type::Procedure { result: None, parameters: vec![1], convention: None },
            ],
            functions: vec![Function {
                name: "f".into(),
                symbol: 0,
                r#type: 2,
                ranges: vec![Range { section: 0, offset: 0, length: 4 }],
                body: None,
                far: false,
                module: false,
                variables: vec![Variable {
                    name: "p".into(),
                    r#type: 1,
                    kind: K::Parameter,
                    location: Location::Frame { disp: 16 },
                }],
                blocks: Vec::new(),
                frame: Vec::new(),
            }],
            ..Info::default()
        };
        let mut made = object(vec![text(vec![0x90; 4], vec![])], vec![defined("f", 0)]);
        made.debug = Some(info);
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("f.obj");
        std::fs::write(&path, write(&made).unwrap()).unwrap();
        let said = Command::new(dump).arg("--codeview").arg(&path).output().unwrap();
        let text = String::from_utf8_lossy(&said.stdout);
        for wanted in [
            "Machine: X64 (0xD0)",
            "PtrType: Near64 (0xC)",
            "SizeOf: 8",
            "BaseRegister: RBP (0x14E)",
            "BasePointerOffset: 16",
            "ReturnType: void (0x3)",
        ] {
            assert!(
                said.status.success() && text.contains(wanted),
                "no {wanted}:\n{text}{}",
                String::from_utf8_lossy(&said.stderr)
            );
        }
    }

    fn which(tool: &str) -> Result<std::path::PathBuf, ()> {
        let mut dirs =
            std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect::<Vec<_>>()).unwrap_or_default();
        dirs.push("/usr/lib/llvm-20/bin".into());
        dirs.iter().map(|dir| dir.join(tool)).find(|path| path.exists()).ok_or(())
    }
}
