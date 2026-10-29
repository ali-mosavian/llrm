//! Encodings and the static executable, against the pin's `llvm-mc`
//! (`~/scratch/llvm-build-m68k/bin`, LLVM d1106deb71cc with M68k): the golden
//! bytes are its `-filetype=obj` `.text` of the compiler's own asm, and the
//! executables run under `qemu-m68k`.
//!
//! `qemu-m68k` exits 1 with no message on a file without the execute bit; the
//! run tests failed that way (status 1, not a wrong result) until the file
//! was written executable.

use std::path::PathBuf;
use std::process::Command;

use llrm_m68k::compile::{Item, items};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::encode::encode;
use llrm_m68k::object::{assemble, executable, start, write_executable};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const IR: &str = "declare i32 @g(i32, i8)\n\
define i32 @add(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}\n\
define i32 @c(i32 %a, i8 %b) {\n  %r = call i32 @g(i32 %a, i8 %b)\n  ret i32 %r\n}\n\
define i32 @sum(i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [0, %entry], [%i1, %loop]\n  %s = phi i32 [0, %entry], [%s1, %loop]\n  %s1 = add i32 %s, %i\n  %i1 = add i32 %i, 1\n  %c = icmp slt i32 %i1, %n\n  br i1 %c, label %loop, label %exit\nexit:\n  ret i32 %s1\n}\n\
define i32 @callee(i32 %a, i8 %b) {\n  %z = zext i8 %b to i32\n  %r = add i32 %a, %z\n  ret i32 %r\n}\n\
define i32 @caller(i32 %a, i8 %b) {\n  %r = call i32 @callee(i32 %a, i8 %b)\n  ret i32 %r\n}";

fn function(name: &str) -> Vec<Item> {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{IR}")).expect("parses");
    items(&module, name, Cpu::M68000).expect("compiles")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn an_add_encodes_as_llvm_mc_does() {
    let (code, _) = assemble(&function("add"), 0).unwrap();
    assert_eq!(hex(&code), "202f0004d0af00084e75");
}

/// `jsr g` is an absolute fixup: `llvm-mc` leaves its four bytes 0 too.
#[test]
fn a_call_encodes_as_llvm_mc_does() {
    let code: Vec<u8> = function("c")
        .iter()
        .filter_map(|item| match item {
            Item::Inst(inst) => Some(encode(inst).unwrap().bytes),
            Item::Label(_) => None,
        })
        .flatten()
        .collect();
    assert_eq!(hex(&code), "9ffc0000000c7000102f00172f400004202f00102e804eb900000000dffc0000000c4e75");
}

/// The loop's `blt` is pc-relative, to a label two instructions in.
#[test]
fn a_loop_branch_is_resolved_as_llvm_mc_does() {
    let (code, _) = assemble(&function("sum"), 0).unwrap();
    assert_eq!(
        hex(&code),
        "9ffc0000000448ef000400007200242f00082001d081d2bc00000001b2826df44cef00040000dffc000000044e75"
    );
}

fn qemu() -> Option<PathBuf> {
    ["/usr/bin/qemu-m68k", "/usr/local/bin/qemu-m68k"].iter().map(PathBuf::from).find(|path| path.exists())
}

/// Links `_start` calling `callee(args)` with the functions and runs it.
fn status(
    callee: &str,
    args: &[i64],
    others: &[&str],
) -> Option<i32> {
    let qemu = qemu().or_else(|| {
        eprintln!("qemu-m68k is not installed: not run");
        None
    })?;
    let mut all = start(callee, args);
    for name in others.iter().chain([&callee]) {
        all.extend(function(name));
    }
    let path = std::env::temp_dir().join(format!("llrm-m68k-{}-{callee}", std::process::id()));
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = Command::new(qemu).arg(&path).status().unwrap().code();
    std::fs::remove_file(&path).ok();
    code
}

#[test]
fn an_add_exits_with_its_sum() {
    if let Some(code) = status("add", &[40, 2], &[]) {
        assert_eq!(code, 42);
    }
}

#[test]
fn a_loop_exits_with_its_sum() {
    if let Some(code) = status("sum", &[10], &[]) {
        assert_eq!(code, 45);
    }
}

/// A byte argument sits at the high end of its slot; a call passes it on.
#[test]
fn a_call_with_a_byte_argument_exits_with_the_sum() {
    if let Some(code) = status("caller", &[40, 2], &["callee"]) {
        assert_eq!(code, 42);
    }
}
