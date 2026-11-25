//! The unroll cost analysis simulated every trip of a loop whose body has no
//! side effect (nothing counts until the live-outs are walked), up to 262144
//! instructions per loop: a 20000-trip sum took 2.4 G compile instructions at
//! -O2 against gcc's 53 M.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

#[test]
fn a_loop_of_many_trips_is_not_simulated_past_the_budget() {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"
@g = global [20000 x i8] zeroinitializer
define i32 @f() {{
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %p = getelementptr i8, ptr @g, i32 %i
  %v = load i8, ptr %p
  %w = zext i8 %v to i32
  %s1 = add i32 %s, %w
  %i1 = add i32 %i, 1
  %z = icmp eq i32 %i1, 20000
  br i1 %z, label %done, label %loop
done:
  ret i32 %s1
}}"
    );
    assembly(&ir, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    let most = llrm_transforms::loopunrollpass::max_simulated();
    assert!(most > 0, "premise: the analysis did not run");
    assert!(most <= 70_000, "{most} instructions simulated for one analysis");
}
