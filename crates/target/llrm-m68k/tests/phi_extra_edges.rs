//! `if (x > 0 && y < 10) r = x + y; else r = 7`: the condition `and` is two
//! branches, the second in a block of its own that also leaves for the join.
//! The join's phi took an input from the first block only, so the second
//! block's edge reached it with `d0` unset: the result was whatever the
//! register held (seeds 92, 93 and 179 of the random-program test).

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const IR: &str = "
define i32 @f(i32 %x, i32 %y) {
entry:
  %a = icmp sgt i32 %x, 0
  %b = icmp slt i32 %y, 10
  %c = and i1 %a, %b
  br i1 %c, label %then, label %end
then:
  %t = add i32 %x, %y
  br label %end
end:
  %r = phi i32 [ 7, %entry ], [ %t, %then ]
  ret i32 %r
}";

#[test]
fn every_edge_into_a_join_carries_the_phi_value() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{IR}")).expect("parses");
    let options = Options { mid_end: false, ..Options::default() };
    for (x, y, want) in [(1_i64, 1_i64, 2), (-1, 1, 7), (1, 20, 7), (0, 0, 7), (5, 9, 14)] {
        let mut all = start("f", &[x, y]);
        all.extend(items_with(&module, "f", Cpu::M68000, options).unwrap());
        let path = support::unique_path("phi-extra");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68000"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(want), "f({x}, {y})");
    }
}
