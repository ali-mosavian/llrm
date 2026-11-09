//! `bfffo` counts leading and trailing zeros from the 68020. The pin selects no
//! bitfield instruction (DEVIATE M68K-BFFFO, `dagisel/bfffo.rs`): `clz32` was
//! the 20-instruction shift-or-popcount expansion, 828 B against gcc's 772 in
//! `bitcount_u32` (executables with every count routine linked into both
//! columns, not whole-program: docs/working/m68k-bitcount-cells.md).

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::object::{executable, start, write_executable};
use llrm_m68k::reg;

mod support;

const HEAD: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn count(
    name: &str,
    bits: u32,
    poison: bool,
    cpu: Cpu,
) -> String {
    let ir = format!(
        "{HEAD}declare i{bits} @llvm.{name}.i{bits}(i{bits}, i1)\n\
         define i{bits} @f(i{bits} %x) {{ %r = call i{bits} @llvm.{name}.i{bits}(i{bits} %x, i1 {poison}) ret i{bits} %r }}"
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    text_with(&module, "f", cpu, Options::default()).expect("compiles")
}

/// Hardware on the 68020 to the 68060; the 68000 and 68010 have none and keep
/// the pin's expansion (the BC1 routines for the zero-poison forms).
#[test]
fn the_68020_and_later_count_with_bfffo_and_the_68000_does_not() {
    for cpu in [Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060] {
        for poison in [false, true] {
            let out = count("ctlz", 32, poison, cpu);
            assert_eq!(out.matches("bfffo").count(), 1, "{cpu:?} ctlz.i32 {poison}:\n{out}");
            assert!(!out.contains("jsr"), "{cpu:?}:\n{out}");
        }
        let out = count("ctlz", 64, false, cpu);
        assert_eq!(out.matches("bfffo").count(), 2, "{cpu:?} ctlz.i64:\n{out}");
        let out = count("cttz", 32, true, cpu);
        assert!(out.contains("bfffo") && out.contains("neg.l"), "{cpu:?} cttz.i32:\n{out}");
    }
    for cpu in [Cpu::M68000, Cpu::M68010] {
        for (name, bits) in [("ctlz", 32), ("ctlz", 64), ("cttz", 32), ("cttz", 64)] {
            assert!(!count(name, bits, true, cpu).contains("bfffo"), "{cpu:?} {name}.i{bits}");
        }
    }
}

/// `bfffo %d0{#0:#0}, %d1` is `edc0 1000` to GNU as.
#[test]
fn bfffo_encodes_as_edc0_with_the_result_in_the_extension_word() {
    let inst = Inst { opcode: "BFFFO32dd", operands: vec![Operand::Reg(reg::D1), Operand::Reg(reg::D0)] };
    assert_eq!(encode(&inst).unwrap().bytes, [0xed, 0xc0, 0x10, 0x00]);
    let inst = Inst { opcode: "BFFFO32dd", operands: vec![Operand::Reg(reg::D7), Operand::Reg(reg::D3)] };
    assert_eq!(encode(&inst).unwrap().bytes, [0xed, 0xc3, 0x70, 0x00]);
}

/// `bfffo (4,%sp){#0:#0}, %d1` is `edef 1000 0004` to GNU as: the displacement
/// follows the bit-field word.
#[test]
fn bfffo_from_a_displacement_encodes_with_the_displacement_after_the_field_word() {
    let inst = Inst {
        opcode: "BFFFO32dp",
        operands: vec![Operand::Reg(reg::D1), Operand::Imm(4), Operand::Reg(reg::SP)],
    };
    assert_eq!(encode(&inst).unwrap().bytes, [0xed, 0xef, 0x10, 0x00, 0x00, 0x04]);
}

/// A stack argument only the count reads was `move.l (4,%sp), %d0; bfffo
/// %d0{#0:#0}, %d0`, 10 B against gcc's 6 (`bfffo (4,%sp){#0:#0}, %d0`).
#[test]
fn a_stack_argument_only_the_count_reads_is_counted_in_place_at_size() {
    let ir = format!(
        "{HEAD}declare i32 @llvm.ctlz.i32(i32, i1)\n\
         define i32 @f(i32 %x) optsize {{ %r = call i32 @llvm.ctlz.i32(i32 %x, i1 false) ret i32 %r }}"
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let out = text_with(&module, "f", Cpu::M68020, Options::default()).expect("compiles");
    assert!(out.contains("bfffo\t(4,%sp){#0:#0}, %d0"), "{out}");
    assert!(!out.contains("move.l\t(4,%sp)"), "{out}");
}

const CLZ: [(i64, i64); 8] =
    [(0, 32), (1, 31), (2, 30), (-2147483648, 0), (-1, 0), (65536, 15), (2147483647, 1), (305419896, 3)];
const CTZ: [(i64, i64); 8] =
    [(0, 32), (1, 0), (2, 1), (-2147483648, 31), (-1, 0), (65536, 16), (2147483647, 0), (305419896, 3)];
const CLZ64: [(i64, i64); 5] = [(0, 64), (1, 63), (4294967296, 31), (-1, 0), (2147483648, 32)];
const CTZ64: [(i64, i64); 5] = [(0, 64), (1, 0), (4294967296, 32), (i64::MIN, 63), (2147483648, 31)];

fn program() -> String {
    let mut text = format!(
        "{HEAD}declare i32 @llvm.ctlz.i32(i32, i1)\ndeclare i32 @llvm.cttz.i32(i32, i1)\n\
         declare i64 @llvm.ctlz.i64(i64, i1)\ndeclare i64 @llvm.cttz.i64(i64, i1)\n"
    );
    for (name, intrinsic, bits, poison) in [
        ("clz", "ctlz", 32, false),
        ("clzs", "ctlz", 32, false),
        ("clzp", "ctlz", 32, true),
        ("ctz", "cttz", 32, false),
        ("ctzp", "cttz", 32, true),
        ("clz64", "ctlz", 64, false),
        ("ctz64", "cttz", 64, false),
    ] {
        // the stack argument is counted in place in a function built for size
        let size = if name == "clzs" { " optsize" } else { "" };
        let wide = if bits == 64 { "\n  %t = trunc i64 %r to i32\n  ret i32 %t" } else { "\n  ret i32 %r" };
        text.push_str(&format!(
            "define i32 @{name}(i{bits} %x){size} {{\n  %r = call i{bits} @llvm.{intrinsic}.i{bits}(i{bits} %x, i1 {poison}){wide}\n}}\n"
        ));
    }
    // each case adds its own number when the answer is wrong; the inputs come
    // through calls the mid-end is off for
    let mut cases: Vec<(&str, &str, i64, i64)> = Vec::new();
    cases.extend(CLZ.iter().map(|&(x, e)| ("clz", "i32", x, e)));
    cases.extend(CLZ.iter().map(|&(x, e)| ("clzs", "i32", x, e)));
    cases.extend(CLZ.iter().filter(|c| c.0 != 0).map(|&(x, e)| ("clzp", "i32", x, e)));
    cases.extend(CTZ.iter().map(|&(x, e)| ("ctz", "i32", x, e)));
    cases.extend(CTZ.iter().filter(|c| c.0 != 0).map(|&(x, e)| ("ctzp", "i32", x, e)));
    cases.extend(CLZ64.iter().map(|&(x, e)| ("clz64", "i64", x, e)));
    cases.extend(CTZ64.iter().map(|&(x, e)| ("ctz64", "i64", x, e)));
    text.push_str("define i32 @f() {\n  %a0 = add i32 0, 0\n");
    for (n, (name, ty, x, expected)) in cases.iter().enumerate() {
        text.push_str(&format!(
            "  %r{n} = call i32 @{name}({ty} {x})\n  %w{n} = icmp ne i32 %r{n}, {expected}\n  %z{n} = zext i1 %w{n} to i32\n  \
             %m{n} = mul i32 %z{n}, {}\n  %a{} = add i32 %a{n}, %m{n}\n",
            n + 1,
            n + 1
        ));
    }
    text.push_str(&format!(
        "  %ok = icmp eq i32 %a{}, 0\n  %s = select i1 %ok, i32 42, i32 1\n  ret i32 %s\n}}\n",
        cases.len()
    ));
    text
}

/// The edge values of the card, executed: 0, 1, 2, `0x80000000`, `-1`, a
/// middle bit; the 64-bit counts across the word boundary.
#[test]
fn the_counts_give_the_right_answers_on_qemu_at_68020_68040_68060() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&program()).expect("parses");
    for (cpu, name) in [(Cpu::M68020, "m68020"), (Cpu::M68040, "m68040"), (Cpu::M68060, "m68060")] {
        let options = Options { mid_end: false, ..Options::default() };
        let mut all = start("f", &[]);
        for function in ["f", "clz", "clzs", "clzp", "ctz", "ctzp", "clz64", "ctz64"] {
            all.extend(items_with(&module, function, cpu, options).unwrap_or_else(|why| panic!("{cpu:?}: {why}")));
        }
        let path = support::unique_path("bfffo");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", name], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(42), "{cpu:?}");
    }
}
