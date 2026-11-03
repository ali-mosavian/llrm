//! `s += a > b` ended `slt; and.l #255; and.l #1; add`: the first mask is
//! dead, the second keeps only bit 0 of what it covers. 6 bytes and 16 clocks
//! a use on the 68000. A mask the next `and` of the register covers goes; the
//! values under qemu, since a mask dropped that is not covered would still
//! assemble.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const COUNT: &str = "
define i32 @f(i32 %acc, i32 %a, i32 %b) {
  %c = icmp sgt i32 %a, %b
  %z = zext i1 %c to i32
  %r = add i32 %acc, %z
  ret i32 %r
}";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

#[test]
fn a_mask_the_next_and_covers_is_dropped() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(COUNT), "f", cpu, Options::default()).expect("compiles");
        assert!(out.contains("and.l\t#1,"), "{cpu:?}: the mask that stays is gone:\n{out}");
        assert!(!out.contains("#255"), "{cpu:?}: a mask the next and covers is left:\n{out}");
    }
}

#[test]
fn the_count_is_still_right() {
    let Some(qemu) = support::qemu() else { return };
    for (a, b, want) in [(5_i64, 3_i64, 11), (1, 3, 10), (3, 3, 10), (-1, -2, 11), (-2, -1, 10)] {
        let mut all = start("f", &[10, a, b]);
        all.extend(items_with(&module(COUNT), "f", Cpu::M68000, Options::default()).unwrap());
        let path = support::unique_path("mask-covered");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68000"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(want), "{a} > {b}");
    }
}
