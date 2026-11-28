//! M68K-LSR-TWICE: the m68k mid-end runs LoopStrengthReduce after the shared
//! pipeline's own run, so one loop is reduced twice; the pin runs it once.
//! Removing the second run changes 107 programs (68000 O2 +136 B, Os +96 B) and
//! its LoopSimplify adds the empty edge blocks known_loads_walks keeps. This
//! pins the count: when it reads 1, the row's premise is gone, and the +12
//! blocks / +2 phis it names are the thing to re-measure.

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_transforms::lsr_pass::runs_taken;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@cells = global [256 x i16] zeroinitializer, align 2
define void @clear() {
  br label %l
l:
  %i = phi i32 [ 0, %0 ], [ %j, %l ]
  %p = getelementptr inbounds nuw [2 x i8], ptr @cells, i32 %i
  store i16 7, ptr %p, align 2
  %j = add nuw nsw i32 %i, 1
  %c = icmp eq i32 %j, 256
  br i1 %c, label %x, label %l
x:
  ret void
}";

#[test]
fn lsr_runs_twice_over_a_function_on_m68k_where_the_pin_runs_it_once() {
    let before = runs_taken();
    assembly(IR, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    assert_eq!(runs_taken() - before, 2, "the shared pipeline's run and the m68k mid-end's");
}
