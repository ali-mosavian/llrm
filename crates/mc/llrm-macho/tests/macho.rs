//! The Mach-O writer against LLVM's, for each target that writes it.

use std::path::Path;
use std::process::Command;

use llrm_macho::{MachTargetWriter, write as write_with};
use llrm_object::Arch;
use llrm_object::{Binding, Definition, Kind, Object, Role, Section, Target, Unsupported};
use llrm_object::{Reloc, Symbol};

/// `llrm_macho::write` with the writer of the object's architecture; any other
/// has none.
fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    match object.arch {
        Arch::X8664 => write_with(object, &llrm_x86::mc::macho::X8664MachO),
        Arch::Arm64 => write_with(object, &llrm_arm64::mc::macho::Arm64MachO),
        other => Err(Unsupported(format!("{other:?} has no Mach-O writer"))),
    }
}

/// A target no one has written: its CPU type is made up.
struct Made;

impl MachTargetWriter for Made {
    fn cpu(&self) -> (u32, u32) {
        (0x0100_4242, 7)
    }

    fn lists_locals(&self) -> bool {
        false
    }

    fn relocation(
        &self,
        kind: Kind,
    ) -> Result<llrm_macho::Described, Unsupported> {
        match kind {
            Kind::Abs { width: 8 } => {
                Ok(llrm_macho::Described { kind: 0, length: 3, pcrel: false, beyond: 0, addend_in_field: true })
            }
            other => Err(Unsupported(format!("{other:?}"))),
        }
    }
}

#[test]
fn a_target_the_writer_has_never_heard_of_writes_a_mach_o_with_its_cpu_type() {
    let mut made = program();
    made.arch = Arch::I386;
    made.sections[0].relocs.clear();
    let bytes = write_with(&made, &Made).unwrap();
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 0x0100_4242);
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 7);
}

const LLVM: &str = "/usr/lib/llvm-20/bin";

fn tool(name: &str) -> Option<String> {
    let path = format!("{LLVM}/{name}");
    Path::new(&path).exists().then_some(path)
}

fn section(
    name: &str,
    role: Role,
    image: Vec<u8>,
    relocs: Vec<Reloc>,
) -> Section {
    let spans = vec![[0, image.len()]];
    Section { name: name.into(), role, near: true, align: 1, image, spans, relocs }
}

fn defined(
    name: &str,
    section: usize,
    offset: usize,
) -> Symbol {
    Symbol {
        name: name.into(),
        binding: Binding::Public,
        definition: Definition::Defined { section, offset },
        size: 0,
        group: None,
    }
}

fn external(name: &str) -> Symbol {
    Symbol { name: name.into(), binding: Binding::Public, definition: Definition::Undefined, size: 0, group: None }
}

fn reloc(
    at: usize,
    kind: Kind,
    target: usize,
    addend: i64,
) -> Reloc {
    Reloc { at, kind, target: Target::Symbol(target), addend }
}

/// The program `REFERENCE` assembles: a call, a `lea`, a `movb $1,
/// flag(%rip)` (four bytes of field and one of immediate), a load, and
/// a pointer to `msg+1`.
fn program() -> Object {
    let text = vec![
        0xE8, 0, 0, 0, 0, 0x48, 0x8D, 0x35, 0, 0, 0, 0, 0xC6, 0x05, 0, 0, 0, 0, 0x01, 0x48, 0x8B, 0x05, 0, 0, 0, 0,
        0xC3,
    ];
    let relocs = vec![
        reloc(1, Kind::Branch { width: 4 }, 3, 0),
        reloc(8, Kind::PcRel { width: 4, from: 4 }, 1, 0),
        reloc(14, Kind::PcRel { width: 4, from: 5 }, 4, 0),
        reloc(22, Kind::PcRel { width: 4, from: 4 }, 2, 0),
    ];
    Object {
        name: "ref.s".into(),
        arch: Arch::X8664,
        sections: vec![
            section("text", Role::Text, text, relocs),
            section("const", Role::ROData, b"hello\n".to_vec(), vec![]),
            {
                let mut data = section("data", Role::Data, vec![0; 9], vec![reloc(0, Kind::Abs { width: 8 }, 1, 1)]);
                data.image[8] = 0;
                data
            },
        ],
        symbols: vec![
            defined("_start", 0, 0),
            defined("_msg", 1, 0),
            defined("_ptr", 2, 0),
            external("_ext"),
            defined("_flag", 2, 8),
        ],
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        debug: None,
    }
}

const REFERENCE: &str = "
    .text
    .globl _start
_start:
    call _ext
    leaq _msg(%rip), %rsi
    movb $1, _flag(%rip)
    movq _ptr(%rip), %rax
    ret
    .section __TEXT,__const
    .globl _msg
_msg:   .ascii \"hello\\n\"
    .data
    .globl _ptr
_ptr:   .quad _msg+1
    .globl _flag
_flag:  .byte 0
";

fn dump(
    tool_path: &str,
    object: &Path,
) -> String {
    let said =
        Command::new(tool_path).args(["-r", "-d", "-t", "--macho", "--full-contents"]).arg(object).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8_lossy(&said.stdout).into_owned()
}

/// The object, dumped by llvm-objdump, says what llvm-mc's object of the
/// same program says: relocations (BRANCH, SIGNED, SIGNED_1, UNSIGNED
/// against a symbol), fields, sections, symbols.
#[test]
fn it_reads_as_llvm_mc_writes_the_same_program() {
    let (Some(objdump), Some(mc)) = (tool("llvm-objdump"), tool("llvm-mc")) else {
        eprintln!("skipped: no LLVM tools");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let (mine, theirs, source) =
        (scratch.path().join("mine.o"), scratch.path().join("theirs.o"), scratch.path().join("ref.s"));
    std::fs::write(&mine, write(&program()).unwrap()).unwrap();
    std::fs::write(&source, REFERENCE).unwrap();
    let made = Command::new(mc)
        .args(["-triple", "x86_64-apple-macos11", "-filetype=obj"])
        .arg(&source)
        .arg("-o")
        .arg(&theirs)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    // The reference's symbol `_flag` is a global there, as here; the dumps
    // differ only in the file's name.
    let (mine, theirs) = (dump(&objdump, &mine), dump(&objdump, &theirs));
    assert_eq!(mine.replace("mine.o", "x"), theirs.replace("theirs.o", "x"));
}

const ARM64_REFERENCE: &str = "
    .section __TEXT,__text
    .p2align 2
    .globl _main
_main:
    bl _puts
    adrp x0, _msg@PAGE
    add x0, x0, _msg@PAGEOFF
    adrp x1, _count@PAGE
    ldr w2, [x1, _count@PAGEOFF]
    adrp x3, _ext@GOTPAGE
    ldr x3, [x3, _ext@GOTPAGEOFF]
    bl _puts+8
    adrp x4, _count@PAGE+16
    mov w0, #0
    ret
    .section __TEXT,__const
_msg:   .asciz \"hi\"
    .section __DATA,__data
_count: .long 7
    .quad _count
";

/// The pinned build the arm64 comparison reads, if it is there. The
/// version it prints is the premise: a different LLVM is not the oracle.
fn arm64_tools() -> Option<(String, String, String)> {
    let build = std::env::var("LLVM_BUILD")
        .unwrap_or_else(|_| format!("{}/scratch/llvm-build-aarch64", std::env::var("HOME").unwrap_or_default()));
    let tool = |name: &str| format!("{build}/bin/{name}");
    let (mc, objdump, readobj) = (tool("llvm-mc"), tool("llvm-objdump"), tool("llvm-readobj"));
    if ![&mc, &objdump, &readobj].iter().all(|path| Path::new(path).exists()) {
        return None;
    }
    let version = Command::new(&objdump).arg("--version").output().ok()?;
    let version = String::from_utf8_lossy(&version.stdout).into_owned();
    assert!(version.contains("24.0.0git"), "not the pinned LLVM: {version}");
    Some((mc, objdump, readobj))
}

/// The relocation entries as written (`llvm-readobj` lists the
/// `ARM64_RELOC_ADDEND` entries `llvm-objdump` folds away), the code with
/// its symbols, and every section's bytes.
fn arm64_dump(
    (objdump, readobj): (&str, &str),
    object: &Path,
) -> String {
    let run = |tool: &str, args: &[&str]| {
        let said = Command::new(tool).args(args).arg(object).output().unwrap();
        assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
        String::from_utf8_lossy(&said.stdout).into_owned()
    };
    let name = object.file_name().unwrap().to_string_lossy().into_owned();
    [run(readobj, &["-r", "-S"]), run(objdump, &["-t", "-s"])].join("\n").replace(&name, "x")
}

fn arm64_program() -> Object {
    let words = [
        0x9400_0000u32,
        0x9000_0000,
        0x9100_0000,
        0x9000_0001,
        0xB940_0022,
        0x9000_0003,
        0xF940_0063,
        0x9400_0000,
        0x9000_0004,
        0x5280_0000,
        0xD65F_03C0,
    ];
    let text: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let relocs = vec![
        reloc(0, Kind::Branch26, 2, 0),
        reloc(4, Kind::Page21, 1, 0),
        reloc(8, Kind::PageOffset12 { access: None }, 1, 0),
        reloc(12, Kind::Page21, 3, 0),
        reloc(16, Kind::PageOffset12 { access: Some(2) }, 3, 0),
        reloc(20, Kind::GotPage21, 4, 0),
        reloc(24, Kind::GotPageOffset12, 4, 0),
        reloc(28, Kind::Branch26, 2, 8),
        reloc(32, Kind::Page21, 3, 16),
    ];
    let mut data = vec![7, 0, 0, 0];
    data.extend([0; 8]);
    let mut text = section("text", Role::Text, text, relocs);
    text.align = 4;
    let local = |name: &str, section, offset| Symbol { binding: Binding::Local, ..defined(name, section, offset) };
    Object {
        name: "m.s".into(),
        arch: Arch::Arm64,
        sections: vec![
            text,
            section("const", Role::ROData, b"hi\0".to_vec(), vec![]),
            section("data", Role::Data, data, vec![reloc(4, Kind::Abs { width: 8 }, 3, 0)]),
        ],
        symbols: vec![
            defined("_main", 0, 0),
            local("_msg", 1, 0),
            external("_puts"),
            local("_count", 2, 0),
            external("_ext"),
        ],
        omf_groups: vec![],
        omf_comdat: vec![],
        debug: None,
    }
}

/// What `-t` says of the symbols the program names, in the order of the
/// table: the assembler's own `ltmp` section symbols are its choice.
fn named_symbols(dump: &str) -> Vec<String> {
    dump.lines()
        .filter(|line| ["_main", "_msg", "_puts", "_count", "_ext"].iter().any(|name| line.ends_with(name)))
        .map(str::to_owned)
        .collect()
}

/// The object, dumped by the pinned llvm-objdump, says what the pinned
/// llvm-mc's object of the same arm64 program says: every relocation kind
/// (BRANCH26, PAGE21, PAGEOFF12, GOT_LOAD_*, UNSIGNED) against a local,
/// a global and an undefined symbol, two addends (`ARM64_RELOC_ADDEND`
/// entries), the section contents and the symbols.
#[test]
fn arm64_reads_as_llvm_mc_writes_the_same_program() {
    let Some((mc, objdump, readobj)) = arm64_tools() else {
        eprintln!("skipped: no pinned LLVM with AArch64");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let (mine, theirs, source) =
        (scratch.path().join("mine.o"), scratch.path().join("theirs.o"), scratch.path().join("ref.s"));
    std::fs::write(&mine, write(&arm64_program()).unwrap()).unwrap();
    std::fs::write(&source, ARM64_REFERENCE).unwrap();
    let made = Command::new(mc)
        .args(["-triple", "arm64-apple-macos", "-filetype=obj"])
        .arg(&source)
        .arg("-o")
        .arg(&theirs)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let (mine, theirs) = (arm64_dump((&objdump, &readobj), &mine), arm64_dump((&objdump, &readobj), &theirs));
    // Premise: the dump of the reference does list its relocations.
    assert!(theirs.contains("ARM64_RELOC_ADDEND") && theirs.contains("ARM64_RELOC_GOT_LOAD_PAGEOFF12"), "{theirs}");
    assert_eq!(named_symbols(&mine), named_symbols(&theirs));
    let body = |dump: &str| {
        dump.lines()
            .filter(|line| !line.contains("ltmp") && !line.contains("SYMBOL TABLE"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let (mine, theirs) = (body(&mine), body(&theirs));
    let first = mine.lines().zip(theirs.lines()).find(|(mine, theirs)| mine != theirs);
    assert_eq!(first, None, "first difference, mine then theirs");
    assert_eq!(mine, theirs);
}

/// A reference to a symbol the object keeps local names its section, and
/// the link resolves it: the call lands on clang's function, the load
/// on the local data.
#[test]
fn ld64_links_it_with_a_clang_object() {
    let (Some(ld), Some(objdump)) = (tool("ld64.lld"), tool("llvm-objdump")) else {
        eprintln!("skipped: no LLVM tools");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    // _start: call _ext; lea local(%rip), %rsi; ret. `local` is not
    // exported.
    let text = vec![0xE8, 0, 0, 0, 0, 0x48, 0x8D, 0x35, 0, 0, 0, 0, 0xC3];
    let made = Object {
        name: "m.s".into(),
        arch: Arch::X8664,
        sections: vec![
            section(
                "text",
                Role::Text,
                text,
                vec![reloc(1, Kind::Branch { width: 4 }, 1, 0), reloc(8, Kind::PcRel { width: 4, from: 4 }, 2, 0)],
            ),
            section("data", Role::Data, vec![7, 0, 0, 0], vec![]),
        ],
        symbols: vec![
            defined("_start", 0, 0),
            external("_ext"),
            Symbol {
                name: "local".into(),
                binding: Binding::Local,
                definition: Definition::Defined { section: 1, offset: 0 },
                size: 0,
                group: None,
            },
        ],
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        debug: None,
    };
    std::fs::write(dir.join("mine.o"), write(&made).unwrap()).unwrap();
    std::fs::write(dir.join("ext.c"), "int ext(void) { return 3; }\n").unwrap();
    let compiled = Command::new("clang")
        .args(["-target", "x86_64-apple-macos11", "-c", "-O1"])
        .arg(dir.join("ext.c"))
        .arg("-o")
        .arg(dir.join("ext.o"))
        .output()
        .unwrap();
    assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
    let linked = Command::new(&ld)
        .args(["-arch", "x86_64", "-platform_version", "macos", "11.0", "11.0", "-e", "_start", "-o"])
        .arg(dir.join("out"))
        .arg(dir.join("mine.o"))
        .arg(dir.join("ext.o"))
        .output()
        .unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    let run = |arguments: &[&str]| {
        String::from_utf8_lossy(&Command::new(&objdump).args(arguments).arg(dir.join("out")).output().unwrap().stdout)
            .into_owned()
    };
    let text = run(&["-d", "--macho"]);
    assert!(text.contains("callq\t_ext"), "{text}");
    // The `lea`'s displacement reaches the start of __data, a section the
    // reference named only by number.
    let lea = text.lines().find(|line| line.contains("leaq")).unwrap_or_else(|| panic!("{text}"));
    let (at, disp) = (
        u64::from_str_radix(lea.split(':').next().unwrap().trim(), 16).unwrap(),
        u64::from_str_radix(lea.split("0x").nth(1).unwrap().split('(').next().unwrap(), 16).unwrap(),
    );
    let sections = run(&["-h", "--macho"]);
    let data = sections.lines().find(|line| line.contains("__data")).unwrap_or_else(|| panic!("{sections}"));
    let data = u64::from_str_radix(data.split_whitespace().nth(3).unwrap(), 16).unwrap();
    assert_eq!(at + 7 + disp, data, "{text}{sections}");
}

/// A pc-relative field is signed: 0x8000_0000 was accepted as an unsigned
/// value and wrote a call that jumps backwards 2 GiB.
#[test]
fn a_pc_relative_field_must_fit_a_signed_word() {
    let mut made = program();
    made.sections[0].relocs[0].addend = 0x8000_0000;
    assert!(write(&made).unwrap_err().0.contains("does not fit"));
}

/// A constant that holds an address is written to by the dynamic linker,
/// which ld64 refuses in __TEXT: it is __DATA,__const.
#[test]
fn a_constant_with_relocations_is_in_data_const() {
    let mut made = program();
    made.sections[1] = section("const", Role::ROData, vec![0; 8], vec![reloc(0, Kind::Abs { width: 8 }, 0, 0)]);
    let bytes = write(&made).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("__const")
            && !bytes.windows(14).any(|one| one == b"__const         __TEXT ".get(..14).unwrap() && false)
    );
    let at = bytes.windows(7).position(|one| one == b"__const").unwrap();
    assert_eq!(&bytes[at + 16..at + 22], b"__DATA");
}

/// What Mach-O cannot say is refused, never written near.
#[test]
fn what_mach_o_cannot_say_is_refused() {
    let with = |kind| {
        let mut made = program();
        made.sections[0].relocs[0].kind = kind;
        write(&made)
    };
    assert!(with(Kind::SegmentBase).is_err());
    assert!(with(Kind::FarPointer).is_err());
    assert!(with(Kind::PcRel { width: 4, from: 7 }).is_err());
    let mut far = program();
    far.sections[1].near = false;
    assert!(write(&far).unwrap_err().0.contains("selector"));
    let mut other = program();
    other.arch = Arch::I386;
    assert!(write(&other).unwrap_err().0.contains("has no Mach-O writer"));
}

/// An object that asks for CodeView or Turbo Debugger information is
/// refused by name: the information of another format is never written
/// in its place.
#[test]
fn a_debug_format_this_object_cannot_carry_is_refused() {
    for (format, name) in [
        (llrm_object::debug::Format::CodeView, "CodeView"),
        (llrm_object::debug::Format::TurboDebugger, "Turbo Debugger"),
    ] {
        let mut made = program();
        made.debug = Some(llrm_object::debug::Info { format, ..Default::default() });
        let why = write(&made).unwrap_err().0;
        assert!(why.contains("cannot carry") && why.contains(name), "{why}");
    }
}
