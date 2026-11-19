//! A flag setter and the instruction that reads its flags were separated by
//! another instruction that sets them (`add.b #-1,%d2; and.l #-65536,%d1;
//! sne %d5`): the loop's exit read the wrong flags. A difftest seed ran off
//! its counter and hung; this one leaves after one pass instead of six.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// Counts the passes of a loop that leaves when `q >= m << 16` or after six.
const LOOP: &str = "
define i32 @f(i32 %p0, i32 %q, i32 %x, i32 %z) {
entry:
  %zb = trunc i32 %z to i1
  br label %loop
loop:
  %i = phi i8 [ 6, %entry ], [ %i1, %loop ]
  %n = phi i32 [ 0, %entry ], [ %n1, %loop ]
  %v = phi i32 [ %x, %entry ], [ %m, %loop ]
  %c0 = icmp ule i32 %v, %p0
  %o = or i1 %c0, %zb
  %m = select i1 %o, i32 16777215, i32 0
  %sh = shl i32 %m, 16
  %c1 = icmp sge i32 %q, %sh
  %i1 = add i8 %i, -1
  %c2 = icmp eq i8 %i1, 0
  %e = select i1 %c1, i1 true, i1 %c2
  %n1 = add i32 %n, 1
  br i1 %e, label %out, label %loop
out:
  ret i32 %n1
}";

#[test]
fn a_loop_exit_reads_the_flags_of_the_compare_it_tests() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{LOOP}")).expect("parses");
    for options in [Options::PINNED, Options::default()] {
        let mut all = start("f", &[0, -100000, 1, 0]);
        all.extend(items_with(&module, "f", Cpu::M68000, options).expect("compiles"));
        let path = support::unique_path("flags-schedule");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68000"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(6), "{options:?}");
    }
}
