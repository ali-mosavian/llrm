//! The ELF32 relocatable object against the llvm-mos SDK's `llvm-mc`
//! (`~/scratch/fetch-llvm-mos-sdk/x/llvm-mos/bin`) on the same program: the
//! same section bytes, relocations, symbols and section list. The emitter's
//! ca65 text is turned into llvm-mos's syntax for the oracle only.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::elf::{Mode, relocatable};

fn bin() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("HOME")?).join("scratch/fetch-llvm-mos-sdk/x/llvm-mos/bin");
    dir.join("llvm-mc").exists().then_some(dir)
}

/// The text without the platform's `rcN = $nn` equates, which the object
/// leaves to the link (the imaginary registers are `__rcN`).
fn module(
    ir: &str,
    cpu: Cpu,
) -> String {
    let text = assembly(ir, &Request { cpu, ..Request::new(Level::O2) }).expect("compiles");
    text.lines().filter(|line| !line.contains(" = $")).collect::<Vec<_>>().join("\n") + "\n"
}

/// ca65 to llvm-mos: directives, segment names, and `rcN` as `__rcN` with the
/// `.zeropage` the assembler needs to pick the 8-bit form.
fn llvm_mos(text: &str) -> String {
    let mut registers: Vec<String> = Vec::new();
    let mut body = String::new();
    for line in text.lines() {
        let line = line
            .replace(".segment \"CODE\"", ".text")
            .replace(".segment \"DATA\"", ".data")
            .replace(".segment \"BSS\"", ".bss")
            .replace(".segment \"RODATA\"", ".section .rodata,\"a\",@progbits")
            .replace(".export ", ".globl ")
            .replace(".res ", ".zero ");
        let mut out = String::new();
        let mut word = String::new();
        for c in line.chars().chain(std::iter::once('\n')) {
            if c.is_ascii_alphanumeric() || c == '_' {
                word.push(c);
                continue;
            }
            let register =
                word.strip_prefix("rc").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
            if register {
                if !registers.contains(&word) {
                    registers.push(word.clone());
                }
                out.push_str("__");
            }
            out.push_str(&word);
            word.clear();
            out.push(c);
        }
        body.push_str(&out);
    }
    let zero_page: String = registers.iter().map(|name| format!(".zeropage __{name}\n")).collect();
    format!(".text\n{zero_page}{body}")
}

fn symbols(table: &str) -> String {
    let mut found: Vec<String> = table
        .lines()
        .filter_map(|line| {
            let (left, right) = line.split_once('\t')?;
            let flags: Vec<&str> = left.split_whitespace().collect();
            // `llvm-mos` adds the runtime's `__do_copy_data` and
            // `__do_zero_bss` to any file with data or bss; the
            // link supplies them for ours.
            let runtime = right.contains("__do_copy_data") || right.contains("__do_zero_bss");
            if runtime || flags.contains(&"df") || flags.contains(&"d") || flags.get(1) == Some(&"l") {
                return None;
            }
            let bind = flags.get(1).filter(|flag| matches!(**flag, "g" | "w")).copied().unwrap_or("-");
            Some(format!("{bind} {} {}", flags.last()?, right.split_whitespace().last()?))
        })
        .collect();
    found.sort();
    found.join("\n")
}

fn sections(table: &str) -> String {
    let mut found: Vec<String> = table
        .lines()
        .filter_map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            words.first()?.parse::<usize>().ok()?;
            let name = words.get(1)?;
            (name.starts_with('.') && !matches!(*name, ".strtab" | ".symtab" | ".shstrtab" | ".note.GNU-stack"))
                .then(|| format!("{name} {} {}", words[2], words.get(4).copied().unwrap_or("")))
        })
        .collect();
    found.sort();
    found.join("\n")
}

fn dump(
    bin: &Path,
    object: &Path,
) -> String {
    let run = |args: &[&str]| {
        let out = Command::new(bin.join("llvm-objdump")).args(args).arg(object).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    let text = run(&["-s", "-j", ".text"]);
    assert!(text.contains("Contents of section .text"), "the dump holds the code: {text}");
    let data = run(&["-s", "-j", ".data"]);
    let all = [text, data, run(&["-r"]), symbols(&run(&["-t"])), sections(&run(&["-h"]))].join("\n");
    all.lines().filter(|line| !line.contains("file format") && !line.contains(".o:")).collect::<Vec<_>>().join("\n")
}

fn same_as_llvm_mc(ir: &str) {
    same_as_llvm_mc_on(ir, Cpu::Mos6502, "mos6502");
}

fn same_as_llvm_mc_on(
    ir: &str,
    cpu: Cpu,
    mcpu: &str,
) {
    let Some(bin) = bin() else {
        eprintln!("the llvm-mos SDK is not fetched: not run");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let text = module(ir, cpu);
    let ours = scratch.path().join("ours.o");
    std::fs::write(&ours, relocatable("m.s", &text).unwrap_or_else(|error| panic!("{error}\n{text}"))).unwrap();
    let asm = scratch.path().join("m.s");
    std::fs::write(&asm, llvm_mos(&text)).unwrap();
    let theirs = scratch.path().join("theirs.o");
    let made = Command::new(bin.join("llvm-mc"))
        .args(["-triple=mos", &format!("-mcpu={mcpu}"), "-filetype=obj", "-o"])
        .arg(&theirs)
        .arg(&asm)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}\n{text}", String::from_utf8_lossy(&made.stderr));
    let (left, right) = (dump(&bin, &ours), dump(&bin, &theirs));
    if left != right {
        let differing: Vec<String> = left
            .lines()
            .zip(right.lines())
            .filter(|(a, b)| a != b)
            .map(|(a, b)| format!("ours:   {a}\ntheirs: {b}"))
            .collect();
        panic!("{}\n--- ours\n{left}\n--- theirs\n{right}", differing.join("\n"));
    }
}

const LAYOUT: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\n";

/// A loop over a table, a call and a store through a pointer: zero-page
/// operands, `abs,x`, `(zp),y`, `jsr` and a branch within the function.
#[test]
fn a_table_loop_with_a_call_is_as_llvm_mc_has_it() {
    same_as_llvm_mc(&format!(
        "{LAYOUT}@tab = global [4 x i8] [i8 1, i8 2, i8 3, i8 4]\n\
         declare i8 @g(i8)\n\
         define i8 @f(i8 %a, ptr %p) {{\nentry:\n  br label %l\nl:\n  %i = phi i8 [0, %entry], [%i1, %l]\n  \
         %s = phi i8 [%a, %entry], [%s1, %l]\n  %q = getelementptr i8, ptr @tab, i8 %i\n  %v = load i8, ptr %q\n  \
         %s1 = add i8 %s, %v\n  %i1 = add i8 %i, 1\n  %c = icmp ult i8 %i1, 4\n  br i1 %c, label %l, label %x\nx:\n  \
         %r = call i8 @g(i8 %s1)\n  store i8 %r, ptr %p\n  ret i8 %r\n}}"
    ));
}

/// 16-bit arithmetic through a global: word cells and the low/high bytes of
/// an address.
#[test]
fn a_word_counter_is_as_llvm_mc_has_it() {
    same_as_llvm_mc(&format!(
        "{LAYOUT}@cnt = global i16 0\n\
         define i16 @bump(i16 %d) {{\n  %v = load i16, ptr @cnt\n  %n = add i16 %v, %d\n  store i16 %n, ptr @cnt\n  ret i16 %n\n}}"
    ));
}

/// A function that takes the address of a global and one that returns a
/// constant: `#<(sym)` and `#>(sym)`, and a function with no relocations.
#[test]
fn an_address_and_a_constant_are_as_llvm_mc_has_them() {
    same_as_llvm_mc(&format!(
        "{LAYOUT}@buf = global [8 x i8] zeroinitializer\n\
         define ptr @addr() {{\n  ret ptr getelementptr (i8, ptr @buf, i16 3)\n}}\n\
         define i8 @seven() {{\n  ret i8 7\n}}"
    ));
}

/// A table of function addresses: `.word` is `R_MOS_ADDR16` in the data.
#[test]
fn a_table_of_function_pointers_is_as_llvm_mc_has_it() {
    same_as_llvm_mc(&format!(
        "{LAYOUT}define i8 @a(i8 %x) {{\n  %r = add i8 %x, 1\n  ret i8 %r\n}}\n\
         define i8 @b(i8 %x) {{\n  %r = add i8 %x, 2\n  ret i8 %r\n}}\n\
         @t = global [2 x ptr] [ptr @a, ptr @b]\n\
         define i8 @pick(i8 %i, i8 %v) {{\n  %q = getelementptr ptr, ptr @t, i8 %i\n  %f = load ptr, ptr %q\n  \
         %r = call i8 %f(i8 %v)\n  ret i8 %r\n}}"
    ));
}

/// A byte copy between two pointers: two `(zp),y` streams and a loop branch.
#[test]
fn a_byte_copy_loop_is_as_llvm_mc_has_it() {
    same_as_llvm_mc(&format!(
        "{LAYOUT}define void @copy(ptr %d, ptr %s, i8 %n) {{\nentry:\n  %z = icmp eq i8 %n, 0\n  \
         br i1 %z, label %out, label %l\nl:\n  %i = phi i8 [0, %entry], [%i1, %l]\n  \
         %p = getelementptr i8, ptr %s, i8 %i\n  %v = load i8, ptr %p\n  %q = getelementptr i8, ptr %d, i8 %i\n  \
         store i8 %v, ptr %q\n  %i1 = add i8 %i, 1\n  %c = icmp ult i8 %i1, %n\n  br i1 %c, label %l, label %out\n\
         out:\n  ret void\n}}"
    ));
}

/// The 65C02: `bra`, `stz` and the other forms it adds.
#[test]
fn a_65c02_function_is_as_llvm_mc_has_it() {
    same_as_llvm_mc_on(
        &format!(
            "{LAYOUT}@cnt = global i16 0\ndefine void @clear(ptr %p) {{\n  store i8 0, ptr %p\n  store i16 0, ptr @cnt\n  ret void\n}}"
        ),
        Cpu::W65c02,
        "mosw65c02",
    );
}

/// Every row of the opcode table is what `llvm-mc` assembles for it.
#[test]
fn every_opcode_of_the_table_is_what_llvm_mc_assembles() {
    let Some(bin) = bin() else {
        eprintln!("the llvm-mos SDK is not fetched: not run");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    for (mnemonic, mode, code, size, nmos) in llrm_6502::elf::OPCODES {
        let operand = match mode {
            Mode::Imp => String::new(),
            Mode::Acc => " a".into(),
            Mode::Imm => " #1".into(),
            Mode::Zp => " 1".into(),
            Mode::Abs => " 0x1234".into(),
            Mode::ZpX => " 1,x".into(),
            Mode::AbsX => " 0x1234,x".into(),
            Mode::ZpY => " 1,y".into(),
            Mode::AbsY => " 0x1234,y".into(),
            Mode::Ind => " (0x1234)".into(),
            Mode::IndX => " (1,x)".into(),
            Mode::IndY => " (1),y".into(),
            Mode::ZpInd => " (1)".into(),
            Mode::AbsIndX => " (0x1234,x)".into(),
            Mode::Rel => " here".into(),
        };
        let asm = scratch.path().join("o.s");
        let after = if matches!(mode, Mode::Rel) { "nop\nhere:\nnop\n" } else { "" };
        std::fs::write(&asm, format!(".text\n{mnemonic}{operand}\n{after}")).unwrap();
        let object = scratch.path().join("o.o");
        let cpu = if *nmos { "mos6502" } else { "mosw65c02" };
        let made = Command::new(bin.join("llvm-mc"))
            .args(["-triple=mos", &format!("-mcpu={cpu}"), "-filetype=obj", "-o"])
            .arg(&object)
            .arg(&asm)
            .output()
            .unwrap();
        assert!(made.status.success(), "{mnemonic} {mode:?}: {}", String::from_utf8_lossy(&made.stderr));
        let binary = scratch.path().join("o.bin");
        Command::new(bin.join("llvm-objcopy"))
            .args(["-O", "binary", "--only-section=.text"])
            .arg(&object)
            .arg(&binary)
            .status()
            .unwrap();
        let bytes = std::fs::read(&binary).unwrap();
        // A branch is followed by the two `nop`s of its target.
        let length = if matches!(mode, Mode::Rel) { bytes.len() - 2 } else { bytes.len() };
        assert_eq!((bytes[0], length), (*code, usize::from(*size)), "{mnemonic} {mode:?}");
    }
}
