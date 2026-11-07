//! A constant argument was pushed as `move.l #22, -(%sp)`, 6 B and 20 clk on
//! the 68000; `pea 22:w` is 4 B and 16 clk and pushes the same long (the
//! absolute short address is sign-extended). structcopy -Os: 46 B against
//! gcc's 42.

use std::process::Command;

use llrm_m68k::callpush::{push_of, push_of_sized};
use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Operand, print};
use llrm_m68k::timing::cost_on;

mod support;

const STRUCTCOPY: &str = include_str!("fixtures/structcopy_raw.ll");

fn binutils(tool: &str) -> Option<Command> {
    let root = support::oracle_root()?;
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{tool}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

fn assembled(text: &str) -> Option<Vec<u8>> {
    let object = support::unique_path("pea-o");
    let source = support::unique_path("pea-s");
    std::fs::write(&source, format!("{text}\n")).unwrap();
    let status = binutils("as")?.args(["-m68000", "-o"]).arg(&object).arg(&source).status().unwrap();
    assert!(status.success(), "{text}");
    let flat = support::unique_path("pea-bin");
    let status = binutils("objcopy")?.args(["-O", "binary", "-j", ".text"]).arg(&object).arg(&flat).status().unwrap();
    assert!(status.success());
    let bytes = std::fs::read(&flat).unwrap();
    for path in [object, source, flat] {
        std::fs::remove_file(path).ok();
    }
    Some(bytes)
}

#[test]
fn a_small_constant_pushes_as_pea_and_assembles_as_binutils_does() {
    for value in [0, 22, 32767, -1, -32768] {
        let inst = push_of(&Operand::Imm(value));
        let text = print(&inst).unwrap();
        assert!(text.starts_with("pea\t"), "{value}: {text}");
        let encoded = encode(&inst).unwrap().bytes;
        assert_eq!(encoded.len(), 4, "{value}");
        let Some(want) = assembled(&text) else { return };
        assert_eq!(encoded, want, "{value}: {text}");
    }
}

/// A function built for size pushes zero as `clr.l -(%sp)`: 2 B where `pea 0:w`
/// is 4; for speed it stays the `pea`, which is 6 clk faster on the 68000.
#[test]
fn zero_pushes_as_clr_for_size_and_as_pea_for_speed() {
    let zero = Operand::Imm(0);
    let inst = push_of_sized(&zero, true);
    let text = print(&inst).unwrap();
    assert_eq!(text, "clr.l\t-(%sp)");
    let encoded = encode(&inst).unwrap().bytes;
    assert_eq!(encoded.len(), 2);
    if let Some(want) = assembled(&text) {
        assert_eq!(encoded, want, "{text}");
    }
    assert!(print(&push_of_sized(&zero, false)).unwrap().starts_with("pea\t"));
}

#[test]
fn a_constant_beyond_a_word_keeps_the_move() {
    for value in [32768, -32769, 1 << 20] {
        let text = print(&push_of(&Operand::Imm(value))).unwrap();
        assert!(text.starts_with("move.l\t#"), "{value}: {text}");
    }
}

#[test]
fn pea_is_never_slower_than_the_move_it_replaces_on_any_cpu() {
    use llrm_m68k::isa::Cpu::*;
    for cpu in [M68000, M68010, M68020, M68030, M68040, M68060] {
        let pea = cost_on(cpu, "pea\t22:w", 4, false).unwrap().clocks;
        let mov = cost_on(cpu, "move.l\t#22, -(%sp)", 6, false).unwrap().clocks;
        assert!(pea <= mov, "{cpu:?}: pea {pea} clk, move {mov} clk");
    }
}

#[test]
fn memcpys_length_is_pushed_with_pea() {
    let out = assembly(STRUCTCOPY, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    assert!(out.contains("pea\t22:w") && !out.contains("#22"), "{out}");
}

/// `g(7, -3, 70000)` is `7 + 3 + 1`: the pushed -3 must arrive as -3, not
/// 65533, and 70000 (past a word) by the move.
#[test]
fn pushed_constants_arrive_with_their_signs_under_qemu() {
    use llrm_m68k::compile::{Options, items_with};
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::object::{executable, start, write_executable};
    let Some(qemu) = support::qemu() else { return };
    let text = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define internal i32 @g(i32 %a, i32 %b, i32 %c) #0 {
  %n = sub i32 %a, %b
  %h = lshr i32 %c, 16
  %r = add i32 %n, %h
  ret i32 %r
}
define i32 @f() {
  %r = call i32 @g(i32 7, i32 -3, i32 70000)
  ret i32 %r
}
attributes #0 = { noinline }";
    let module = llrm_mir::parse::module(text).expect("parses");
    let mut all = start("f", &[]);
    for name in ["f", "g"] {
        all.extend(
            items_with(&module, name, Cpu::M68000, Options { size: true, ..Options::default() }).expect("compiles"),
        );
    }
    let path = support::unique_path("pea");
    llrm_m68k::object::write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(11));
}
