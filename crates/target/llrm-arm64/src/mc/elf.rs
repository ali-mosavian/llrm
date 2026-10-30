//! An [`Object`] as an ELF64 relocatable file for arm64, RELA relocations, as
//! GNU ld and lld take them. The container is `llrm-elf`'s; the relocation
//! types are the AArch64 ELF psABI's, as LLVM's `AArch64ELFObjectWriter`
//! chooses them
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/MCTargetDesc/AArch64ELFObjectWriter.cpp>).

use llrm_elf::{Machine, Relocation};
use llrm_object::{Fix, FixupKind, Object, Reloc, Unsupported};

use super::fixup;

const EM_AARCH64: u16 = 183;
const R_AARCH64_ABS64: u32 = 257;
const R_AARCH64_ABS32: u32 = 258;
const R_AARCH64_PREL32: u32 = 261;
const R_AARCH64_ADR_PREL_PG_HI21: u32 = 275;
const R_AARCH64_ADD_ABS_LO12_NC: u32 = 277;
const R_AARCH64_LDST8_ABS_LO12_NC: u32 = 278;
const R_AARCH64_CALL26: u32 = 283;
const R_AARCH64_LDST16_ABS_LO12_NC: u32 = 284;
const R_AARCH64_LDST32_ABS_LO12_NC: u32 = 285;
const R_AARCH64_LDST64_ABS_LO12_NC: u32 = 286;
const R_AARCH64_LDST128_ABS_LO12_NC: u32 = 299;
const R_AARCH64_ADR_GOT_PAGE: u32 = 311;
const R_AARCH64_LD64_GOT_LO12_NC: u32 = 312;

struct Arm64;

impl Machine for Arm64 {
    const BITS: u8 = 64;
    const WIDE: bool = true;
    const NUMBER: u16 = EM_AARCH64;
    const ADDEND: bool = true;

    fn backend() -> &'static dyn llrm_object::assembler::AsmBackend {
        &super::backend::BACKEND
    }

    fn relocation(reloc: &Reloc) -> Result<Relocation, Unsupported> {
        // Every instruction relocation computes its value from the symbol and
        // the addend alone (`from` is 0): the page and branch kinds subtract
        // the place themselves.
        let got = reloc.spec == Fix::S_GOT;
        let (kind, width) = match (reloc.kind, reloc.pc_rel, got) {
            (FixupKind::DATA_8, false, _) => (R_AARCH64_ABS64, 8),
            (FixupKind::DATA_4, false, _) => (R_AARCH64_ABS32, 4),
            // An `.eh_frame` entry's pointer to its function.
            (FixupKind::DATA_4, true, _) => (R_AARCH64_PREL32, 4),
            (fixup::BRANCH26, _, _) => (R_AARCH64_CALL26, 4),
            (fixup::PAGE21, _, false) => (R_AARCH64_ADR_PREL_PG_HI21, 4),
            (fixup::ADD_IMM12, _, _) => (R_AARCH64_ADD_ABS_LO12_NC, 4),
            (fixup::LDST_IMM12_SCALE1, _, _) => (R_AARCH64_LDST8_ABS_LO12_NC, 4),
            (fixup::LDST_IMM12_SCALE2, _, _) => (R_AARCH64_LDST16_ABS_LO12_NC, 4),
            (fixup::LDST_IMM12_SCALE4, _, _) => (R_AARCH64_LDST32_ABS_LO12_NC, 4),
            (fixup::LDST_IMM12_SCALE8, _, false) => (R_AARCH64_LDST64_ABS_LO12_NC, 4),
            (fixup::LDST_IMM12_SCALE16, _, _) => (R_AARCH64_LDST128_ABS_LO12_NC, 4),
            (fixup::PAGE21, _, true) => (R_AARCH64_ADR_GOT_PAGE, 4),
            (fixup::LDST_IMM12_SCALE8, _, true) => (R_AARCH64_LD64_GOT_LO12_NC, 4),
            other => return Err(Unsupported(format!("ELF arm64 has no relocation for {other:?}"))),
        };
        Ok(Relocation { kind, width, from: 0 })
    }
}

/// `object` as an ELF64 relocatable file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_elf::write::<Arm64>(object)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use llrm_object::{Binding, Definition, Fix, Reloc, Role, Section, Symbol, Target};

    use super::*;

    /// The pinned build with AArch64 (`LLVM_BUILD`), if present. The version it
    /// prints is the premise: another LLVM is not the oracle.
    fn pinned(tool: &str) -> Option<String> {
        let build = std::env::var("LLVM_BUILD")
            .unwrap_or_else(|_| format!("{}/scratch/llvm-build-aarch64", std::env::var("HOME").unwrap_or_default()));
        let path = format!("{build}/bin/{tool}");
        if !Path::new(&path).exists() {
            return None;
        }
        let version = Command::new(format!("{build}/bin/llvm-mc")).arg("--version").output().ok()?;
        assert!(String::from_utf8_lossy(&version.stdout).contains("24.0.0git"), "not the pinned LLVM");
        Some(path)
    }

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
    ) -> Symbol {
        Symbol {
            name: name.into(),
            binding,
            definition: Definition::Defined { section, offset: 0 },
            size: 0,
            group: None,
        }
    }

    fn reloc(
        at: usize,
        fix: Fix,
        target: usize,
        addend: i64,
    ) -> Reloc {
        Reloc::new(at, fix, Target::Symbol(target), addend)
    }

    /// `_start` writes `ptr`'s string (`msg+1`, "ello\n") and exits 0.
    fn hello() -> Object {
        let words = [
            0x9000_0003u32, // adrp x3, ptr
            0xF940_0061,    // ldr x1, [x3, :lo12:ptr]
            0xD280_0020,    // mov x0, #1
            0xD280_00A2,    // mov x2, #5
            0xD280_0808,    // mov x8, #64
            0xD400_0001,    // svc #0
            0xD280_0000,    // mov x0, #0
            0xD280_0BA8,    // mov x8, #93
            0xD400_0001,    // svc #0
        ];
        let text: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        let relocs = vec![reloc(0, fixup::PAGE21_FIX, 2, 0), reloc(4, fixup::page_offset12(Some(3)), 2, 0)];
        Object {
            name: "hello.s".into(),
            bits: 64,
            sections: vec![
                section(".text", Role::Text, text, relocs),
                section(".rodata", Role::ROData, b"hello\n".to_vec(), vec![]),
                section(".data", Role::Data, vec![0; 8], vec![reloc(0, Fix::abs(8), 1, 1)]),
            ],
            symbols: vec![
                symbol("_start", Binding::Public, 0),
                symbol("msg", Binding::Local, 1),
                symbol("ptr", Binding::Public, 2),
            ],
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        }
    }

    /// `.eh_frame` points at the code with 4-byte place-relative fields, which
    /// the writer had no relocation for: every function with unwind rows
    /// refused to write ("ELF arm64 has no relocation for ...").
    #[test]
    fn a_four_byte_place_relative_field_is_prel32() {
        let reloc = Reloc::new(0, Fix::pc_rel(4, 0), Target::Section(0), 0);
        let relocation = Arm64::relocation(&reloc).expect("a relocation");
        assert_eq!((relocation.kind, relocation.width, relocation.from), (261, 4, 0));
    }

    /// The pinned `llvm-readobj` reads the object, and its relocations are
    /// the ones the pinned `llvm-mc` writes for the same instructions: types
    /// and symbols (a local symbol is named by its section, as there).
    #[test]
    fn its_relocations_are_the_ones_llvm_mc_writes() {
        let (Some(mc), Some(readobj)) = (pinned("llvm-mc"), pinned("llvm-readobj")) else {
            eprintln!("skipped: no pinned LLVM with AArch64");
            return;
        };
        let scratch = tempfile::tempdir().unwrap();
        let (mine, theirs, source) =
            (scratch.path().join("mine.o"), scratch.path().join("theirs.o"), scratch.path().join("ref.s"));
        std::fs::write(&mine, write(&hello()).unwrap()).unwrap();
        std::fs::write(
            &source,
            "
        .text
        .globl _start
_start:
        adrp x3, ptr
        ldr x1, [x3, :lo12:ptr]
        mov x0, #1
        mov x2, #5
        mov x8, #64
        svc #0
        mov x0, #0
        mov x8, #93
        svc #0
        .section .rodata
msg:    .ascii \"hello\\n\"
        .data
        .globl ptr
ptr:    .xword msg+1
",
        )
        .unwrap();
        let made = Command::new(mc)
            .args(["-triple", "aarch64-linux-gnu", "-filetype=obj"])
            .arg(&source)
            .arg("-o")
            .arg(&theirs)
            .output()
            .unwrap();
        assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
        let relocations = |object: &Path| {
            let said = Command::new(&readobj).arg("-r").arg(object).output().unwrap();
            assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
            String::from_utf8_lossy(&said.stdout)
                .lines()
                .filter(|line| line.contains("R_AARCH64") || line.contains("Section"))
                // The section's number is the layout's choice, not the
                // relocation's.
                .map(|line| line.split_whitespace().filter(|word| !word.starts_with('(')).collect::<Vec<_>>().join(" "))
                .collect::<Vec<_>>()
        };
        let (mine, theirs) = (relocations(&mine), relocations(&theirs));
        // Premise: the reference does carry the three relocations.
        assert_eq!(theirs.iter().filter(|line| line.contains("R_AARCH64")).count(), 3, "{theirs:?}");
        assert_eq!(mine, theirs);
    }

    /// The object, linked by the pinned lld, runs under qemu-aarch64 and writes
    /// what the data's pointer plus its addend names.
    #[test]
    fn lld_links_it_and_qemu_runs_it() {
        let Some(lld) = pinned("ld.lld") else {
            eprintln!("skipped: no pinned LLVM with AArch64");
            return;
        };
        if Command::new("qemu-aarch64").arg("--version").output().is_err() {
            eprintln!("skipped: no qemu-aarch64");
            return;
        }
        let scratch = tempfile::tempdir().unwrap();
        let (object, program) = (scratch.path().join("hello.o"), scratch.path().join("hello"));
        std::fs::write(&object, write(&hello()).unwrap()).unwrap();
        let linked = Command::new(lld).args(["-static", "-o"]).arg(&program).arg(&object).output().unwrap();
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        let ran = Command::new("qemu-aarch64").arg(&program).output().unwrap();
        assert_eq!((ran.status.code(), ran.stdout.as_slice()), (Some(0), &b"ello\n"[..]));
    }
}
