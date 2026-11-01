//! `addq` and `subq`: an add or subtract of 1 to 8 was the pin's `add.l #n`,
//! two bytes longer in a register and 4 in memory; the encodings against
//! binutils and the selection by immediate range.

use std::path::PathBuf;
use std::process::Command;

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::dagisel::quick;
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
    let object = support::unique_path("quick-o");
    let source = support::unique_path("quick-s");
    std::fs::write(&source, format!("{text}\n")).unwrap();
    let status = binutils("as")?.args(["-m68020", "-o"]).arg(&object).arg(&source).status().unwrap();
    assert!(status.success(), "{text}");
    let flat = support::unique_path("quick-bin");
    let status = binutils("objcopy")?.args(["-O", "binary", "-j", ".text"]).arg(&object).arg(&flat).status().unwrap();
    assert!(status.success());
    let bytes = std::fs::read(&flat).unwrap();
    for path in [object, source, flat] {
        std::fs::remove_file(path).ok();
    }
    Some(bytes)
}

/// The operands a quick instruction of `class` takes, with the immediate.
fn operands(
    class: &str,
    value: i64,
) -> Vec<Operand> {
    let (data, address, index) = (Operand::Reg(reg::D3), Operand::Reg(reg::A2), Operand::Reg(reg::D1));
    let imm = Operand::Imm(value);
    match class {
        "di" => vec![data.clone(), data, imm],
        "ai" => vec![address.clone(), address, imm],
        "ji" => vec![address, imm],
        "pi" => vec![Operand::Imm(12), address, imm],
        "fi" => vec![Operand::Imm(12), address, index, imm],
        _ => unreachable!("{class}"),
    }
}

/// Every quick instruction's bytes are what the assembler makes of its text,
/// for each immediate: the encoding rows are built from the pin's, and a
/// wrong field would run as another instruction.
#[test]
fn every_quick_form_encodes_as_binutils_does() {
    let mut checked = 0;
    for name in quick::names() {
        for value in 1..=8 {
            let inst = Inst { opcode: name, operands: operands(&name[name.len() - 2..], value) };
            let text = print(&inst).unwrap();
            let Some(want) = assembled(&text) else { return };
            assert_eq!(encode(&inst).unwrap().bytes, want, "{name} #{value}: {text}");
            checked += 1;
        }
    }
    assert_eq!(checked, 8 * quick::names().count());
}

fn text(
    ir: &str,
    cpu: Cpu,
) -> String {
    text_with(&module(ir), "f", cpu, Options::default()).expect("compiles")
}

/// The long add of 1 to 8 to a register, in memory and to an address, and the
/// subtract; 0, 9, and -9 stay the pin's.
#[test]
fn immediates_one_to_eight_are_quick_and_the_rest_are_not() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for (op, value, want) in [
            ("add", 1, "addq.l\t#1, %d0"),
            ("add", 8, "addq.l\t#8, %d0"),
            ("sub", 5, "subq.l\t#5, %d0"),
            ("add", -3, "subq.l\t#3, %d0"),
            ("sub", -8, "addq.l\t#8, %d0"),
            ("add", 9, "add.l\t#9, %d0"),
            ("add", -9, "add.l\t#-9, %d0"),
            ("add", 100, "add.l\t#100, %d0"),
        ] {
            let ir = format!("define i32 @f(i32 %x) {{\n  %r = {op} i32 %x, {value}\n  ret i32 %r\n}}");
            let out = text(&ir, cpu);
            assert!(out.contains(want), "{cpu:?} {op} {value}: want {want}:\n{out}");
        }
    }
}

#[test]
fn memory_and_narrow_forms_are_quick_too() {
    for (width, op, value, want) in [
        ("i32", "add", 1, "addq.l\t#1, (%a0)"),
        ("i16", "sub", 2, "subq.w\t#2, (%a0)"),
        ("i8", "add", -8, "subq.b\t#8, (%a0)"),
        ("i8", "add", 7, "addq.b\t#7, (%a0)"),
    ] {
        let ir = format!(
            "define void @f(ptr %p) {{\n  %v = load {width}, ptr %p\n  %w = {op} {width} %v, {value}\n  store {width} %w, ptr %p\n  ret void\n}}"
        );
        let out = text(&ir, Cpu::M68000);
        assert!(out.contains(want), "{width} {op} {value}: want {want}:\n{out}");
    }
}

/// `x + -5` read for its carry or overflow: the add of a negative immediate
/// may become a `subq` only where nothing reads the carry. No selection reads
/// the flags of an add of an immediate today (the pin routes these through a
/// register), so this pins the values; it fails if one starts to.
#[test]
fn the_carry_and_overflow_of_an_add_of_a_negative_immediate_are_read_right() {
    let unsigned = String::from("declare { i8, i1 } @llvm.uadd.with.overflow.i8(i8, i8)
define i8 @f(i8 %x) {
  %p = call { i8, i1 } @llvm.uadd.with.overflow.i8(i8 %x, i8 -5)
  %c = extractvalue { i8, i1 } %p, 1
  %v = extractvalue { i8, i1 } %p, 0
  %r = select i1 %c, i8 %v, i8 7
  ret i8 %r
}");
    let signed = unsigned.replace("uadd", "sadd");
    // the sum where it carries (x >= 5) or overflows (x - 5 < -128), else 7
    for (ir, x, want) in [(&unsigned, 10, 5), (&unsigned, 3, 7), (&unsigned, 5, 0), (&signed, 10, 7), (&signed, -124, 127), (&signed, -123, 7)] {
        if let Some(code) = status(ir, &[x]) {
            assert_eq!(code, want, "f({x})");
        }
    }
}

/// Exit status of `f(args)`; `None` without an optional qemu.
fn status(
    ir: &str,
    args: &[i64],
) -> Option<i32> {
    let qemu: PathBuf = support::qemu()?;
    let mut all = start("f", args);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, Options::default()).expect("compiles"));
    let path = support::unique_path("quick");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert!(code.is_some(), "f({args:?}) ended on a signal");
    code
}

/// The values: each immediate from -9 to 10 added to a register and in memory
/// gives the sum, through the quick form or not.
#[test]
fn quick_and_plain_forms_compute_the_same_sums() {
    for value in -9..=10i64 {
        let reg = format!("define i32 @f(i32 %x) {{\n  %r = add i32 %x, {value}\n  ret i32 %r\n}}");
        let mem = format!(
            "define i32 @f(i32 %x) {{\n  %s = alloca i32\n  store i32 %x, ptr %s\n  %v = load i32, ptr %s\n  %w = add i32 %v, {value}\n  store i32 %w, ptr %s\n  %r = load i32, ptr %s\n  ret i32 %r\n}}"
        );
        for ir in [reg, mem] {
            if let Some(code) = status(&ir, &[100]) {
                assert_eq!(code, (100 + value) as i32 & 0xFF, "100 + {value}");
            }
        }
    }
}
