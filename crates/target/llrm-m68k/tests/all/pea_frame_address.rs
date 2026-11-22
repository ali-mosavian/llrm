//! `h(a, b, c)` on three local arrays pushed `lea (36,%sp), %a0; move.l %a0,
//! -(%sp)` for each address (6 B); `pea (36,%sp)` is 4 B and needs no register.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

use crate::support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define internal i32 @h(ptr %a, ptr %b, ptr %c) #0 {
  %x = load i32, ptr %a
  %y = load i32, ptr %b
  %z = load i32, ptr %c
  %s = shl i32 %x, 8
  %t = shl i32 %y, 4
  %u = add i32 %s, %t
  %r = add i32 %u, %z
  ret i32 %r
}
define i32 @f(i32 %n) {
  %a = alloca [8 x i32]
  %b = alloca [8 x i32]
  %c = alloca [8 x i32]
  store i32 1, ptr %a
  store i32 2, ptr %b
  store i32 3, ptr %c
  %r = call i32 @h(ptr %a, ptr %b, ptr %c)
  %t = add i32 %r, %n
  ret i32 %t
}
attributes #0 = { noinline }
";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(IR).expect("parses")
}

fn options() -> Options {
    Options { size: true, call_targets: false, ..Options::default() }
}

#[test]
fn a_local_address_argument_is_one_pea() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = text_with(&module(), "f", cpu, options()).expect("compiles");
        let peas = text.lines().filter(|line| line.contains("pea\t(") && line.contains("%sp")).count();
        assert!(peas >= 3, "{cpu:?}: {peas} pea (d,%sp) of 3:\n{text}");
        assert!(!text.contains("%a0, -(%sp)") && !text.contains("%a1, -(%sp)"), "{cpu:?}:\n{text}");
    }
}

/// 1*256 + 2*16 + 3 = 291, plus n = 4.
#[test]
fn the_pushed_addresses_are_the_arrays() {
    let Some(qemu) = support::qemu() else { return };
    let mut all = start("f", &[4]);
    for name in ["f", "h"] {
        all.extend(items_with(&module(), name, Cpu::M68000, options()).expect("compiles"));
    }
    let path = support::unique_path("pea_frame_address");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some((291 + 4) & 0xff), "the exit status is the low byte of the sum");
}
