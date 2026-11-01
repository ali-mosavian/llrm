//! A load through `(An)+` used once by an arithmetic instruction was a `move`
//! and a register (`move.l (%a1)+, %d2; add.l %d2, %d0`, 2 instructions, 4
//! bytes, a register to save); it is `add.l (%a1)+, %d0`. The encodings
//! against binutils, the selection, and the values under qemu.

use std::process::Command;

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::dagisel::indexed;
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand, print};
use llrm_m68k::object::{executable, start, write_executable};
use llrm_m68k::reg;

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
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

/// The bytes binutils assembles `text` to.
fn assembled(text: &str) -> Option<Vec<u8>> {
    let object = support::unique_path("foldload-o");
    let source = support::unique_path("foldload-s");
    std::fs::write(&source, format!("{text}\n")).unwrap();
    let status = binutils("as")?.args(["-m68020", "-o"]).arg(&object).arg(&source).status().unwrap();
    assert!(status.success(), "{text}");
    let flat = support::unique_path("foldload-bin");
    let status = binutils("objcopy")?.args(["-O", "binary", "-j", ".text"]).arg(&object).arg(&flat).status().unwrap();
    assert!(status.success());
    let bytes = std::fs::read(&flat).unwrap();
    for path in [object, source, flat] {
        std::fs::remove_file(path).ok();
    }
    Some(bytes)
}

/// Every instruction that takes its source through `(An)+` is what the
/// assembler makes of its text: the rows are the pin's `(An)` ones with one
/// field changed, and a wrong field would run as another instruction.
#[test]
fn every_post_increment_source_encodes_as_binutils_does() {
    let mut checked = 0;
    for name in indexed::source_rows() {
        let one =
            Inst { opcode: name, operands: vec![Operand::Reg(reg::D3), Operand::Reg(reg::D3), Operand::Reg(reg::A2)] };
        // a compare has no tied source
        let inst = if name.starts_with("CMP") {
            Inst { opcode: name, operands: vec![Operand::Reg(reg::D3), Operand::Reg(reg::A2)] }
        } else {
            one
        };
        let text = print(&inst).unwrap();
        assert!(text.contains("(%a2)+"), "{name}: {text}");
        let Some(want) = assembled(&text) else { return };
        assert_eq!(encode(&inst).unwrap().bytes, want, "{name}: {text}");
        checked += 1;
    }
    assert!(checked >= 15, "only {checked} rows");
}

/// Folds `n` longs read through a post-incremented pointer into `acc` by `op`.
fn fold(
    op: &str,
    init: i64,
) -> String {
    format!(
        "define i32 @f(ptr %p, i32 %n) {{
entry:
  br label %l
l:
  %q = phi ptr [ %p, %entry ], [ %q1, %l ]
  %i = phi i32 [ 0, %entry ], [ %i1, %l ]
  %s = phi i32 [ {init}, %entry ], [ %s1, %l ]
  %v = load i32, ptr %q
  %s1 = {op} i32 %s, %v
  %q1 = getelementptr i32, ptr %q, i32 1
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %l, label %x
x:
  ret i32 %s1
}}"
    )
}

fn text(
    ir: &str,
    cpu: Cpu,
) -> String {
    text_with(&module(ir), "f", cpu, Options::default()).expect("compiles")
}

/// The load is part of the arithmetic instruction, and there is no register
/// for it.
#[test]
fn a_load_used_once_by_arithmetic_is_its_memory_source() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for (op, mnemonic) in [("add", "add.l"), ("sub", "sub.l"), ("and", "and.l"), ("or", "or.l")] {
            let out = text(&fold(op, 0), cpu);
            assert!(out.contains(&format!("{mnemonic}\t(%a")) && out.contains(")+, %d"), "{cpu:?} {op}:\n{out}");
            assert!(!out.contains("move.l\t(%a1)+") && !out.contains("move.l\t(%a0)+"), "{cpu:?} {op}:\n{out}");
        }
    }
}

/// Exit status of `f(n)`; `None` without an optional qemu.
fn status(
    ir: &str,
    n: i64,
) -> Option<i32> {
    let qemu = support::qemu()?;
    let mut all = start("f", &[n]);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, Options::default()).expect("compiles"));
    let path = support::unique_path("foldload");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert!(code.is_some(), "f({n}) ended on a signal");
    code
}

/// Fills a stack buffer with `i * 37 + 5` at long `i`, then folds `n` of them.
fn filled(
    op: &str,
    init: i64,
) -> String {
    format!(
        "define i32 @f(i32 %n) {{
entry:
  %buf = alloca [256 x i8]
  br label %fill
fill:
  %o = phi i32 [ 0, %entry ], [ %o1, %fill ]
  %p = getelementptr i32, ptr %buf, i32 %o
  %m = mul i32 %o, 37
  %v = add i32 %m, 5
  store i32 %v, ptr %p
  %o1 = add i32 %o, 1
  %d = icmp slt i32 %o1, 64
  br i1 %d, label %fill, label %go
go:
  br label %l
l:
  %q = phi ptr [ %buf, %go ], [ %q1, %l ]
  %i = phi i32 [ 0, %go ], [ %i1, %l ]
  %s = phi i32 [ {init}, %go ], [ %s1, %l ]
  %w = load i32, ptr %q
  %s1 = {op} i32 %s, %w
  %q1 = getelementptr i32, ptr %q, i32 1
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %l, label %x
x:
  %r = and i32 %s1, 255
  ret i32 %r
}}"
    )
}

/// The folded instruction reads the same longs and leaves the pointer past
/// them: the result is what a plain fold gives.
#[test]
fn the_folded_instruction_computes_what_the_two_did() {
    let longs: Vec<u32> = (0..64u32).map(|i| i * 37 + 5).collect();
    let n = 40;
    for (op, init, want) in [
        ("add", 3i64, longs[..n].iter().fold(3u32, |a, v| a.wrapping_add(*v))),
        ("sub", 1000, longs[..n].iter().fold(1000u32, |a, v| a.wrapping_sub(*v))),
        ("and", -1, longs[..n].iter().fold(u32::MAX, |a, v| a & v)),
        ("or", 0, longs[..n].iter().fold(0u32, |a, v| a | v)),
    ] {
        let Some(got) = status(&filled(op, init), n as i64) else { return };
        assert_eq!(got, (want & 255) as i32, "{op}");
    }
}

/// Counts how many of `n` longs of a filled buffer satisfy `v <pred> 1000`
/// (`v_first`) or `1000 <pred> v`.
fn counted(
    pred: &str,
    v_first: bool,
) -> String {
    let (a, b) = if v_first { ("%w", "%t") } else { ("%t", "%w") };
    format!(
        "define i32 @f(i32 %n) {{
entry:
  %buf = alloca [256 x i8]
  br label %fill
fill:
  %o = phi i32 [ 0, %entry ], [ %o1, %fill ]
  %p = getelementptr i32, ptr %buf, i32 %o
  %m = mul i32 %o, 37
  %v = add i32 %m, 5
  store i32 %v, ptr %p
  %o1 = add i32 %o, 1
  %d = icmp slt i32 %o1, 64
  br i1 %d, label %fill, label %go
go:
  %t = add i32 %n, 960
  br label %l
l:
  %q = phi ptr [ %buf, %go ], [ %q1, %l ]
  %i = phi i32 [ 0, %go ], [ %i1, %l ]
  %s = phi i32 [ 0, %go ], [ %s1, %l ]
  %w = load i32, ptr %q
  %k = icmp {pred} i32 {a}, {b}
  %z = zext i1 %k to i32
  %s1 = add i32 %s, %z
  %q1 = getelementptr i32, ptr %q, i32 1
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, 64
  br i1 %c, label %l, label %x
x:
  ret i32 %s1
}}"
    )
}

/// A compare of the loaded value is the memory source whichever side the
/// value is on, and the condition that reads the codes follows it: swapping
/// the operands of a compare without swapping `gt` for `lt` counts the wrong
/// elements.
#[test]
fn a_compare_of_a_loaded_value_counts_the_same_either_way_round() {
    let longs: Vec<i64> = (0..64i64).map(|i| i * 37 + 5).collect();
    let threshold = 1000;
    for (pred, test) in [
        ("sgt", (|a: i64, b: i64| a > b) as fn(i64, i64) -> bool),
        ("sge", |a, b| a >= b),
        ("slt", |a, b| a < b),
        ("sle", |a, b| a <= b),
        ("ugt", |a, b| a > b),
        ("ult", |a, b| a < b),
        ("uge", |a, b| a >= b),
        ("ule", |a, b| a <= b),
        ("eq", |a, b| a == b),
        ("ne", |a, b| a != b),
    ] {
        for v_first in [true, false] {
            let ir = counted(pred, v_first);
            for cpu in [Cpu::M68000, Cpu::M68020] {
                let out = text(&ir, cpu);
                assert!(out.contains("cmp.l\t(%a") && out.contains(")+, %d"), "{pred} {v_first} {cpu:?}:\n{out}");
            }
            let want =
                longs.iter().filter(|v| if v_first { test(**v, threshold) } else { test(threshold, **v) }).count();
            let Some(got) = status(&ir, 40) else { return };
            assert_eq!(got, want as i32, "{pred} value first {v_first}");
        }
    }
}
