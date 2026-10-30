//! An [`Object`] as a COFF object for x86-64 (`IMAGE_FILE_MACHINE_AMD64`). The
//! container is `llrm-coff`'s.

use llrm_coff::{Machine, Relocation};
use llrm_object::{Fix, FixupKind, Object, Reloc, Unsupported};

use super::fixup;

const ADDR64: u16 = 1;
const ADDR32: u16 = 2;
const REL32: u16 = 4;
const ADDR32NB: u16 = 3;
const SECTION: u16 = 0xA;
const SECREL: u16 = 0xB;

struct X8664;

impl Machine for X8664 {
    const BITS: u8 = 64;
    const NUMBER: u16 = 0x8664;

    fn backend() -> &'static dyn llrm_object::assembler::AsmBackend {
        &super::backend::FIELDS
    }

    fn relocation(reloc: &Reloc) -> Result<Relocation, Unsupported> {
        let (kind, baked) = match (reloc.kind, reloc.pc_rel) {
            (FixupKind::DATA_8, false) => (ADDR64, 0),
            // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/MCTargetDesc/X86WinCOFFObjectWriter.cpp#L83
            (FixupKind::DATA_4, false) if reloc.spec == Fix::S_IMGREL => (ADDR32NB, 0),
            (FixupKind::DATA_4, false) => (ADDR32, 0),
            // LLVM writes REL32 only, never REL32_n: the distance past the
            // field's end goes into the field (`FixedValue += 4` in
            // WinCOFFObjectWriter), so equal totals are one object.
            (FixupKind::DATA_4, true) => (REL32, 4),
            (fixup::BRANCH_4, true) => (REL32, 4),
            (FixupKind::SECREL_2, _) => (SECTION, 0),
            (FixupKind::SECREL_4, _) => (SECREL, 0),
            (fixup::SEGMENT_BASE, _) => {
                return Err(Unsupported("a segment selector has no COFF relocation".into()));
            }
            (fixup::FAR_POINTER, _) => {
                return Err(Unsupported("a 16:16 far pointer has no COFF relocation".into()));
            }
            other => return Err(Unsupported(format!("COFF x86-64 has no relocation for {other:?}"))),
        };
        Ok(Relocation { kind, baked })
    }
}

/// `object` as a COFF object file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_coff::write::<X8664>(object)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use llrm_debug::DebugObjectTarget;
    use llrm_object::{Binding, Definition, Fix, Reloc, Role, Section, Symbol, Target};

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
        Object {
            name: "a.c".into(),
            bits: 64,
            sections,
            symbols,
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        }
    }

    fn defined(
        name: &str,
        offset: usize,
    ) -> Symbol {
        Symbol {
            name: name.into(),
            binding: Binding::Public,
            definition: Definition::Defined { section: 0, offset },
            size: 0,
            group: None,
        }
    }

    /// (type, field) of the one relocation of `.text`, for a pc-relative field
    /// `from` bytes short of the place and an `addend` past the symbol.
    fn rel_to(
        from: usize,
        addend: i64,
    ) -> (u16, i32) {
        let reloc = Reloc::new(2, Fix::pc_rel(4, from), Target::Symbol(0), addend);
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

    fn rel(from: usize) -> (u16, i32) {
        rel_to(from, 0)
    }

    /// LLVM writes `REL32` for every pc-relative field and folds the distance
    /// past the field's end into the field; llrm picked `REL32_1..5` by that
    /// distance alone.
    #[test]
    fn a_field_before_an_immediate_is_rel32_with_the_distance_in_the_field() {
        assert_eq!(rel(4), (4, 0));
        assert_eq!(rel(5), (4, -1));
        assert_eq!(rel(8), (4, -4));
        assert_eq!(rel(9), (4, -5));
        assert_eq!(rel(10), (4, -6));
    }

    /// `cmpb $1, x(%rip)` (the field is 5 short of the place) and `lea
    /// x-1(%rip)` (4 short, addend -1) have the same total, -5, and one object
    /// in LLVM: `REL32` and a field of -1. llrm wrote `REL32_1` and `REL32`.
    #[test]
    fn two_instructions_with_the_same_total_addend_write_one_relocation() {
        assert_eq!(rel_to(5, 0), rel_to(4, -1));
        assert_eq!(rel_to(9, 0), rel_to(4, -5));
        assert_eq!(rel_to(6, 1), rel_to(4, -1));
    }

    /// An image-relative field (the `.pdata` RVAs) was written as ADDR32, an
    /// absolute address the linker never rebases to the image; it is
    /// ADDR32NB (type 3).
    #[test]
    fn an_image_relative_field_is_addr32nb() {
        let reloc = Reloc::new(0, Fix::image_relative(4), Target::Symbol(0), 0);
        let bytes = write(&object(vec![text(vec![0; 4], vec![reloc])], vec![defined("v", 0)])).unwrap();
        let relocs = u32::from_le_bytes(bytes[20 + 24..20 + 28].try_into().unwrap()) as usize;
        assert_eq!(u16::from_le_bytes([bytes[relocs + 8], bytes[relocs + 9]]), 3);
    }

    /// lld-link links an x86-64 object: `start` calls `f` and the call reaches
    /// it.
    #[test]
    fn lld_link_links_it_and_the_call_reaches_f() {
        let (Ok(link), Ok(dump)) = (which("lld-link"), which("llvm-objdump")) else {
            eprintln!("skipped: needs lld-link and llvm-objdump");
            return;
        };
        let call = Reloc::new(1, fixup::branch(4), Target::Symbol(1), 0);
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
        use llrm_debug::{Function, Info, Kind as K, Location, Range, Reach, Scalar, Type, Variable};
        let Ok(dump) = which("llvm-readobj") else { return eprintln!("skipped: no llvm-readobj") };
        let target = llrm_debug::DebugTarget::of(64).with_registers(
            "rbp",
            "",
            vec![llrm_debug::Register { name: "rbp".into(), bits: 64, dwarf: Some(6), codeview: Some(334) }],
        );
        let info = Info {
            target: Some(target),
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
        let made = object(vec![text(vec![0x90; 4], vec![])], vec![defined("f", 0)]);
        let debug = Some(info);
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("f.obj");
        std::fs::write(&path, crate::mc::X86Objects.write_described("coff", &made, debug.as_ref()).unwrap()).unwrap();
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
