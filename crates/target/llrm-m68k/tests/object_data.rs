//! `llrm-m68k-llc` without `-S` printed its usage and exited 2, so `llrm-cc -c`
//! on m68k never produced an object and the objsweep legs had nothing to
//! build. It now writes an ELF32 big-endian relocatable with `.text`, and the
//! data sections of the globals (`.data`, `.rodata`, `.bss`) and their
//! relocations. The shapes fixture links and runs from that object.

use std::path::{Path, PathBuf};
use std::process::Command;

mod support;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt").join(name)
}

fn object_of(
    cpu: &str,
    ir: &str,
) -> (support::Scratch, std::process::Output) {
    let path = support::unique_path("objdata-o");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .args([&format!("-mcpu={cpu}"), "-O2", "-o"])
        .arg(&path)
        .arg(fixture(ir))
        .output()
        .unwrap();
    (path, output)
}

fn binutils(tool: &str) -> Option<Command> {
    let root = std::env::var("GCC_M68K").ok().filter(|root| !root.is_empty());
    let Some(root) = root else {
        assert!(std::env::var("M68K_ISA_SKIP_ORACLE").is_ok_and(|flag| flag == "1"), "GCC_M68K is not set");
        return None;
    };
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{tool}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

#[test]
fn no_s_flag_writes_an_elf_object() {
    let (path, output) = object_of("68000", "shapes.ll");
    assert!(output.status.success(), "llc failed: {}", String::from_utf8_lossy(&output.stderr));
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(&bytes[..4], b"\x7fELF", "not an ELF file");
    assert_eq!(bytes[4], 1, "not 32-bit");
    assert_eq!(bytes[5], 2, "not big-endian");
    assert_eq!(u16::from_be_bytes([bytes[18], bytes[19]]), 4, "not EM_68K");
}

#[test]
fn the_object_has_the_data_sections_and_defines_the_globals() {
    let Some(mut objdump) = binutils("objdump") else { return };
    let (path, output) = object_of("68000", "shapes.ll");
    assert!(output.status.success(), "llc failed: {}", String::from_utf8_lossy(&output.stderr));
    let table = objdump.arg("-t").arg(&path).output().unwrap();
    let text = String::from_utf8_lossy(&table.stdout).into_owned();
    std::fs::remove_file(&path).ok();
    for (symbol, section) in
        [("ints", ".data"), ("z1", ".bss"), ("pad", ".data"), ("f.count", ".data"), ("f.names", ".rodata")]
    {
        let line = text.lines().find(|line| line.ends_with(&format!(" {symbol}"))).unwrap_or("");
        assert!(line.contains(section), "{symbol} is not defined in {section}:\n{text}");
    }
}

#[test]
fn the_shapes_run_from_our_object() {
    let (Some(link), Some(qemu)) = (binutils("gcc"), support::qemu()) else { return };
    for cpu in ["68000", "68020"] {
        let (object, output) = object_of(cpu, "shapes.ll");
        assert!(output.status.success(), "llc failed: {}", String::from_utf8_lossy(&output.stderr));
        let exe = support::unique_path("objdata-exe");
        let status = Command::new(link.get_program())
            .envs(link.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))))
            .args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
            .arg(fixture("shapes.c"))
            .arg(&object)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap();
        assert!(status.success(), "{cpu}: link failed");
        let code = support::run(&qemu, &[], &exe);
        assert_eq!(code, Some(0), "{cpu}: exit code names the first failing shape (11 bss .. 16 statics)");
    }
}

fn section_alignments(bytes: &[u8]) -> Vec<(String, u32)> {
    let word = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    let half = |at: usize| usize::from(u16::from_be_bytes(bytes[at..at + 2].try_into().unwrap()));
    let (table, size, count, names) = (word(0x20) as usize, half(0x2E), half(0x30), half(0x32));
    let names_at = word(table + names * size + 16) as usize;
    (0..count)
        .map(|index| {
            let header = table + index * size;
            let at = names_at + word(header) as usize;
            let end = bytes[at..].iter().position(|byte| *byte == 0).unwrap();
            (String::from_utf8_lossy(&bytes[at..at + end]).into_owned(), word(header + 32))
        })
        .collect()
}

/// `.rodata` and `.bss` were given `sh_addralign` 4 whatever they held; the
/// pin gives them their objects' alignment, 2 for 16-bit data.
#[test]
fn a_section_of_16_bit_data_is_aligned_to_2() {
    let (path, output) = object_of("68000", "halves.ll");
    assert!(output.status.success(), "llc failed: {}", String::from_utf8_lossy(&output.stderr));
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    let alignments = section_alignments(&bytes);
    for name in [".rodata", ".bss"] {
        let found = alignments.iter().find(|(one, _)| one == name).map(|(_, align)| *align);
        assert_eq!(found, Some(2), "{name} sh_addralign in {alignments:?}");
    }
}

/// The file symbol was the scratch IR's path, so two compiles of one source
/// from different directories differed by their path bytes.
#[test]
fn the_object_does_not_depend_on_where_the_ir_file_is() {
    let source = std::fs::read_to_string(fixture("halves.ll")).unwrap();
    let mut objects = Vec::new();
    for tag in ["first-dir", "second-directory-longer"] {
        let dir = support::unique_path(tag);
        std::fs::create_dir_all(&dir).unwrap();
        let ir = dir.join("d.ll");
        std::fs::write(&ir, &source).unwrap();
        let out = dir.join("d.o");
        let status =
            Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc")).args(["-O2", "-o"]).arg(&out).arg(&ir).status().unwrap();
        assert!(status.success());
        objects.push(std::fs::read(&out).unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }
    assert!(objects[0] == objects[1], "two compiles of one source differ");
}
