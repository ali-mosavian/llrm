//! The unroll cost analysis simulated every trip of a loop whose body has no
//! side effect (nothing counts until the live-outs are walked) up to its
//! budget before giving up: a 20000-trip sum took 2.4 G compile instructions at
//! -O2 against gcc's 53 M (0.8 G with the budget at 65536). Every trip walks
//! the loop header, so trips x header instructions over the budget is refused
//! with no simulation. This is the only test in its binary: `max_simulated`
//! is a process-wide maximum, so a second test would race on it.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn sum_of(trips: u32) -> String {
    format!(
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
  %z = icmp eq i32 %i1, {trips}
  br i1 %z, label %done, label %loop
done:
  ret i32 %s1
}}"
    )
}

#[test]
fn a_loop_whose_trips_cannot_fit_the_budget_is_refused_without_simulating_them() {
    assembly(&sum_of(8), cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    let small = llrm_transforms::loopunrollpass::max_simulated();
    assert!(small > 0, "premise: the analysis did not run on the short loop");
    assembly(&sum_of(20000), cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    let most = llrm_transforms::loopunrollpass::max_simulated();
    assert!(most <= 10 * small, "{most} instructions simulated for a 20000-trip loop, {small} for 8 trips");
}
