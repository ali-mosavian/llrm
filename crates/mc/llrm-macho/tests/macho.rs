//! The Mach-O writer against LLVM's, for each target that writes it.

use std::path::Path;
use std::process::Command;

use llrm_debug::DebugObjectTarget;
use llrm_macho::{
    LC_BUILD_VERSION, LC_SYMTAB, MachTargetWriter, N_EXT, N_WEAK_DEF, PLATFORM_MACOS, write as write_with,
};
use llrm_object::ObjectTarget;
use llrm_object::{Binding, Definition, Fix, Object, Role, Section, Target, Unsupported};
use llrm_object::{Reloc, Symbol};

/// The x86-64 Mach-O writer; any other width has none.
fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_x86::mc::X86Objects.write_object("macho", object)
}

fn write_arm64(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_arm64::mc::Arm64Objects.write_object("macho", object)
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
        reloc: &Reloc,
    ) -> Result<llrm_macho::Described, Unsupported> {
        match (reloc.fix(), reloc.pc_rel) {
            (fix, _) if fix == Fix::abs(8) => {
                Ok(llrm_macho::Described { kind: 0, length: 3, pcrel: false, beyond: 0, addend_in_field: true })
            }
            (other, _) => Err(Unsupported(format!("{other:?}"))),
        }
    }
}

#[test]
fn a_target_the_writer_has_never_heard_of_writes_a_mach_o_with_its_cpu_type() {
    let mut made = program();
    made.bits = 32;
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
    fix: Fix,
    target: usize,
    addend: i64,
) -> Reloc {
    Reloc::new(at, fix, Target::Symbol(target), addend)
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
        reloc(1, llrm_x86::mc::fixup::branch(4), 3, 0),
        reloc(8, Fix::pc_rel(4, 4), 1, 0),
        reloc(14, Fix::pc_rel(4, 5), 4, 0),
        reloc(22, Fix::pc_rel(4, 4), 2, 0),
    ];
    Object {
        name: "ref.s".into(),
        bits: 64,
        sections: vec![
            section("text", Role::Text, text, relocs),
            section("const", Role::ROData, b"hello\n".to_vec(), vec![]),
            {
                let mut data = section("data", Role::Data, vec![0; 9], vec![reloc(0, Fix::abs(8), 1, 1)]);
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
        frames: Vec::new(),
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
        reloc(0, llrm_arm64::mc::fixup::BRANCH26_FIX, 2, 0),
        reloc(4, llrm_arm64::mc::fixup::PAGE21_FIX, 1, 0),
        reloc(8, llrm_arm64::mc::fixup::page_offset12(None), 1, 0),
        reloc(12, llrm_arm64::mc::fixup::PAGE21_FIX, 3, 0),
        reloc(16, llrm_arm64::mc::fixup::page_offset12(Some(2)), 3, 0),
        reloc(20, llrm_arm64::mc::fixup::GOT_PAGE21, 4, 0),
        reloc(24, llrm_arm64::mc::fixup::GOT_PAGE_OFFSET12, 4, 0),
        reloc(28, llrm_arm64::mc::fixup::BRANCH26_FIX, 2, 8),
        reloc(32, llrm_arm64::mc::fixup::PAGE21_FIX, 3, 16),
    ];
    let mut data = vec![7, 0, 0, 0];
    data.extend([0; 8]);
    let mut text = section("text", Role::Text, text, relocs);
    text.align = 4;
    let local = |name: &str, section, offset| Symbol { binding: Binding::Local, ..defined(name, section, offset) };
    Object {
        name: "m.s".into(),
        bits: 64,
        sections: vec![
            text,
            section("const", Role::ROData, b"hi\0".to_vec(), vec![]),
            section("data", Role::Data, data, vec![reloc(4, Fix::abs(8), 3, 0)]),
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
        frames: Vec::new(),
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
    std::fs::write(&mine, write_arm64(&arm64_program()).unwrap()).unwrap();
    std::fs::write(&source, ARM64_REFERENCE).unwrap();
    let made = Command::new(mc)
        // macOS 11, the platform the writer names: without a version LLVM
        // writes no LC_BUILD_VERSION.
        .args(["-triple", "arm64-apple-macos11", "-filetype=obj"])
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
        bits: 64,
        sections: vec![
            section(
                "text",
                Role::Text,
                text,
                vec![reloc(1, llrm_x86::mc::fixup::branch(4), 1, 0), reloc(8, Fix::pc_rel(4, 4), 2, 0)],
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
        frames: Vec::new(),
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
    made.sections[1] = section("const", Role::ROData, vec![0; 8], vec![reloc(0, Fix::abs(8), 0, 0)]);
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
    let with = |fix| {
        let mut made = program();
        let at = made.sections[0].relocs[0].at;
        made.sections[0].relocs[0] = Reloc::new(at, fix, Target::Symbol(3), 0);
        write(&made)
    };
    assert!(with(Fix::plain(llrm_x86::mc::fixup::SEGMENT_BASE)).is_err());
    assert!(with(Fix::plain(llrm_x86::mc::fixup::FAR_POINTER)).is_err());
    // A pc-relative field at any distance is a SIGNED relocation, never
    // refused (the pin writes `movb $1, x+3(%rip)` and `cmpl $1, x+2(%rip)` as
    // type 1); it was refused until the writer picked SIGNED_n by the total.
    assert!(with(Fix::pc_rel(4, 7)).is_ok());
    let mut far = program();
    far.sections[1].near = false;
    assert!(write(&far).unwrap_err().0.contains("selector"));
    let mut other = program();
    other.bits = 32;
    assert!(write(&other).unwrap_err().0.contains("no macho writer for a 32-bit"));
}

/// An object that asks for CodeView or Turbo Debugger information is
/// refused by name: the information of another format is never written
/// in its place.
#[test]
fn a_debug_format_this_object_cannot_carry_is_refused() {
    for (format, name) in
        [(llrm_debug::Format::CodeView, "CodeView"), (llrm_debug::Format::TurboDebugger, "Turbo Debugger")]
    {
        let made = program();
        let debug = llrm_debug::Info {
            options: llrm_debug::DebugOptions { format, ..Default::default() },
            ..Default::default()
        };
        let why = llrm_x86::mc::X86Objects.write_described("macho", &made, Some(&debug)).unwrap_err().0;
        assert!(why.contains("cannot carry") && why.contains(name), "{why}");
    }
}

/// Every command of a Mach-O object: `(cmd, bytes)`.
fn commands_of(object: &[u8]) -> Vec<(u32, Vec<u8>)> {
    let word = |at: usize| u32::from_le_bytes(object[at..at + 4].try_into().unwrap());
    let (count, mut at) = (word(16), 32);
    (0..count)
        .map(|_| {
            let (cmd, size) = (word(at), word(at + 4) as usize);
            let one = (cmd, object[at..at + size].to_vec());
            at += size;
            one
        })
        .collect()
}

/// An arm64 object had no `LC_BUILD_VERSION`: `ld` warned "no platform load
/// command found" for every object and assumed macOS.
#[test]
fn an_arm64_object_names_its_platform() {
    let mut object = program();
    object.sections = vec![section("text", Role::Text, vec![0xC0, 0x03, 0x5F, 0xD6], vec![])];
    object.symbols = vec![defined("_f", 0, 0)];
    let written = write_arm64(&object).unwrap();
    let build: Vec<_> = commands_of(&written).into_iter().filter(|(cmd, _)| *cmd == LC_BUILD_VERSION).collect();
    assert_eq!(build.len(), 1);
    let words: Vec<u32> = build[0].1.chunks(4).map(|w| u32::from_le_bytes(w.try_into().unwrap())).collect();
    assert_eq!(words, [LC_BUILD_VERSION, 24, PLATFORM_MACOS, 11 << 16, 0, 0]);
    // The header's byte count covers it: the symbol table is where the commands
    // end.
    let total: usize = commands_of(&written).iter().map(|(_, bytes)| bytes.len()).sum();
    assert_eq!(total, u32::from_le_bytes(written[20..24].try_into().unwrap()) as usize);
}

/// An x86-64 object's bytes are unchanged: it carries no build version.
#[test]
fn an_x86_64_object_has_none() {
    let written = write(&program()).unwrap();
    assert!(commands_of(&written).iter().all(|(cmd, _)| *cmd != LC_BUILD_VERSION));
}

/// A weak definition (`__attribute__((weak))`) was written as a strong one: two
/// objects that defined it were "duplicate symbol" to `ld`. It carries
/// `N_WEAK_DEF`.
#[test]
fn a_weak_symbol_is_a_weak_definition() {
    let mut object = program();
    object.sections = vec![section("text", Role::Text, vec![0xC0, 0x03, 0x5F, 0xD6], vec![])];
    object.symbols = vec![defined("_strong", 0, 0), Symbol { binding: Binding::Weak, ..defined("_hook", 0, 0) }];
    let written = write_arm64(&object).unwrap();
    let word = |at: usize| u32::from_le_bytes(written[at..at + 4].try_into().unwrap()) as usize;
    let symtab = commands_of(&written).into_iter().find(|(cmd, _)| *cmd == LC_SYMTAB).unwrap().1;
    let field = |at: usize| u32::from_le_bytes(symtab[at..at + 4].try_into().unwrap()) as usize;
    let (symbols_at, count, strings_at) = (field(8), field(12), field(16));
    let mut found = Vec::new();
    for index in 0..count {
        let entry = symbols_at + index * 16;
        let name_at = strings_at + word(entry);
        let name =
            written[name_at..].iter().take_while(|byte| **byte != 0).map(|byte| *byte as char).collect::<String>();
        let desc = u16::from_le_bytes([written[entry + 6], written[entry + 7]]);
        found.push((name, written[entry + 4] & N_EXT != 0, desc & N_WEAK_DEF != 0));
    }
    found.sort();
    assert_eq!(found, [("_hook".to_owned(), true, true), ("_strong".to_owned(), true, false)]);
}
