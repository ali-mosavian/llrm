//! The ELF32 relocatable object, against the pin's `llvm-mc -filetype=obj` on
//! the compiler's own asm (`~/scratch/llvm-build-m68k/bin`, LLVM d1106deb71cc
//! with M68k): the same header, `.text` bytes and relocations.

use std::path::PathBuf;
use std::process::Command;

use llrm_m68k::compile::{Item, Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::elf::relocatable;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const IR: &str = "declare i32 @g(i32, i8)\n\
@counter = global i32 0\n\
define i32 @add(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}\n\
define i32 @c(i32 %a, i8 %b) {\n  %r = call i32 @g(i32 %a, i8 %b)\n  ret i32 %r\n}\n\
define i32 @sum(i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [0, %entry], [%i1, %loop]\n  %s = phi i32 [0, %entry], [%s1, %loop]\n  %s1 = add i32 %s, %i\n  %i1 = add i32 %i, 1\n  %c = icmp slt i32 %i1, %n\n  br i1 %c, label %loop, label %exit\nexit:\n  ret i32 %s1\n}\n\
define i32 @bump(i32 %d) {\n  %v = load i32, ptr @counter\n  %n = add i32 %v, %d\n  store i32 %n, ptr @counter\n  ret i32 %n\n}";

fn bin() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("HOME")?).join("scratch/llvm-build-m68k/bin");
    dir.join("llvm-mc").exists().then_some(dir)
}

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{IR}")).expect("parses")
}

/// The symbols as binding, section and name, and with `typed` their type and
/// size too: `llvm-mc` writes no file or section symbol and, without `.type`
/// and `.size` in the asm, no type or size.
fn symbols(
    table: &str,
    typed: bool,
) -> String {
    let mut found: Vec<String> = table
        .lines()
        .filter_map(|line| {
            let (left, right) = line.split_once('\t')?;
            let flags: Vec<&str> = left.split_whitespace().collect();
            if flags.get(1).is_some_and(|flag| matches!(*flag, "l" | "g" | "w")) && flags.contains(&"df")
                || flags.contains(&"d")
            {
                return None;
            }
            let bind = flags.get(1).filter(|flag| matches!(**flag, "l" | "g" | "w")).copied().unwrap_or("-");
            let kind = flags.iter().find(|flag| matches!(**flag, "F" | "O")).copied().unwrap_or("-");
            let (size, name) = (right.split_whitespace().next()?, right.split_whitespace().last()?);
            Some(if typed {
                format!("{bind} {kind} {} {size} {name}", flags.last()?)
            } else {
                format!("{bind} {} {name}", flags.last()?)
            })
        })
        .collect::<Vec<_>>();
    // `llvm-mc` lists them in the order the asm names them.
    found.sort();
    found.join("\n")
}

/// The sections as name, size and type, less the ones that only hold names:
/// `llvm-mc` lays the file out in its own order and writes no
/// `.note.GNU-stack`.
fn sections(table: &str) -> String {
    let mut found: Vec<String> = table
        .lines()
        .filter_map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            let at = words.first()?.parse::<usize>().ok()?;
            let (name, size) = (words.get(1)?, words.get(2)?);
            let kind = words.get(4).copied().unwrap_or("");
            let _ = at;
            (name.starts_with('.') && !matches!(*name, ".strtab" | ".symtab" | ".shstrtab" | ".note.GNU-stack"))
                .then(|| format!("{name} {size} {kind}"))
        })
        .collect();
    found.sort();
    found.join("\n")
}

fn dump(
    bin: &std::path::Path,
    object: &std::path::Path,
) -> String {
    dump_typed(bin, object, false)
}

fn dump_typed(
    bin: &std::path::Path,
    object: &std::path::Path,
    typed: bool,
) -> String {
    let run = |args: &[&str]| {
        let out = Command::new(bin.join("llvm-objdump")).args(args).arg(object).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    // The file name line differs; the rest is the object.
    let all =
        [run(&["-s", "-j", ".text"]), run(&["-r"]), symbols(&run(&["-t"]), typed), sections(&run(&["-h"]))].join("\n");
    assert!(all.contains("Contents of section .text"), "the dump holds the section: {all}");
    all.lines().filter(|line| !line.contains("file format") && !line.contains(".o:")).collect::<Vec<_>>().join("\n")
}

/// One function: our object and `llvm-mc`'s of its own asm give one dump.
fn same_as_llvm_mc(name: &str) {
    let Some(bin) = bin() else {
        eprintln!("the pin's llvm-mc is not built: not run");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let module = module();
    let items: Vec<Item> = items_with(&module, name, Cpu::M68000, Options::PINNED).expect("compiles");
    let ours = scratch.path().join("ours.o");
    std::fs::write(&ours, relocatable(&format!("{name}.s"), &items).expect("an object")).unwrap();

    let asm = scratch.path().join("f.s");
    let body = text_with(&module, name, Cpu::M68000, Options::PINNED).unwrap();
    std::fs::write(&asm, format!("\t.text\n\t.globl\t{name}\n{body}")).unwrap();
    let theirs = scratch.path().join("theirs.o");
    let made = Command::new(bin.join("llvm-mc"))
        .args(["-triple=m68k", "-mcpu=M68000", "-filetype=obj", "-o"])
        .arg(&theirs)
        .arg(&asm)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    assert_eq!(dump(&bin, &ours), dump(&bin, &theirs), "{name}");
}

/// A call to an undefined function is `R_68K_32` at the operand.
#[test]
fn a_call_is_an_absolute_relocation_as_llvm_mc_has_it() {
    same_as_llvm_mc("c");
}

/// A branch to a label of the function is resolved and leaves no relocation.
#[test]
fn a_loop_branch_leaves_no_relocation_as_llvm_mc_has_it() {
    same_as_llvm_mc("sum");
}

#[test]
fn a_function_without_symbols_has_none_to_relocate() {
    same_as_llvm_mc("add");
}

/// A global's address is an absolute 32-bit field.
#[test]
fn a_global_is_an_absolute_relocation_as_llvm_mc_has_it() {
    same_as_llvm_mc("bump");
}

/// Data, bss, a weak and an undefined symbol, built by hand: the object holds
/// what `llc` would give a module with globals, which the m68k compile path
/// does not emit yet.
#[test]
fn data_bss_weak_and_undefined_symbols_are_as_llvm_mc_writes_them() {
    use llrm_object::{Binding, Definition, Fix, FixupKind, Object, Reloc, Role, Section, Symbol, Target};
    let Some(bin) = bin() else {
        eprintln!("the pin's llvm-mc is not built: not run");
        return;
    };
    let symbol =
        |name: &str, binding, definition, size| Symbol { name: name.into(), binding, definition, size, group: None };
    let section = |name: &str, role, image: Vec<u8>, relocs| Section {
        name: name.into(),
        role,
        near: true,
        align: 4,
        spans: vec![[0, image.len()]],
        image,
        relocs,
    };
    let object = Object {
        name: "d.s".into(),
        bits: 32,
        sections: vec![
            section(
                ".text",
                Role::Text,
                vec![0x4E, 0xB9, 0, 0, 0, 0, 0x4E, 0x75],
                vec![Reloc::new(2, Fix::plain(FixupKind::DATA_4), Target::Symbol(3), 0)],
            ),
            section(
                ".data",
                Role::Data,
                vec![0; 8],
                vec![
                    Reloc::new(0, Fix::plain(FixupKind::DATA_4), Target::Symbol(3), 0),
                    Reloc::new(4, Fix::plain(FixupKind::DATA_4), Target::Symbol(4), 12),
                ],
            ),
            section(".bss", Role::Bss, vec![0; 16], Vec::new()),
        ],
        symbols: vec![
            symbol("f", Binding::Public, Definition::Defined { section: 0, offset: 0 }, 8),
            symbol("d", Binding::Public, Definition::Defined { section: 1, offset: 0 }, 8),
            symbol("b", Binding::Public, Definition::Defined { section: 2, offset: 0 }, 16),
            symbol("g", Binding::Public, Definition::Undefined, 0),
            symbol("w", Binding::Weak, Definition::Undefined, 0),
        ],
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        frames: Vec::new(),
    };
    let scratch = tempfile::tempdir().unwrap();
    let ours = scratch.path().join("ours.o");
    std::fs::write(&ours, llrm_m68k::elf::write(&object).expect("an object")).unwrap();
    let asm = scratch.path().join("d.s");
    std::fs::write(
        &asm,
        "\t.text\n\t.globl\tf\n\t.type\tf,@function\nf:\n\tjsr\tg\n\trts\n\t.size\tf, .-f\n\
         \t.data\n\t.globl\td\n\t.type\td,@object\n\t.p2align\t2\nd:\n\t.long\tg\n\t.long\tw+12\n\t.size\td, 8\n\
         \t.weak\tw\n\
         \t.bss\n\t.globl\tb\n\t.type\tb,@object\n\t.p2align\t2\nb:\n\t.zero\t16\n\t.size\tb, 16\n",
    )
    .unwrap();
    let theirs = scratch.path().join("theirs.o");
    let made = Command::new(bin.join("llvm-mc"))
        .args(["-triple=m68k", "-mcpu=M68000", "-filetype=obj", "-o"])
        .arg(&theirs)
        .arg(&asm)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    assert_eq!(dump_typed(&bin, &ours, true), dump_typed(&bin, &theirs, true));
}
