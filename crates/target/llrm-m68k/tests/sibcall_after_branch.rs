//! A tail call that is not in the entry block stayed a `jsr` and a `rts`
//! (tail_calls `odd`/`even`: +880 clocks each against gcc's `jra`): the
//! sibling call looks for the caller's incoming argument slots in the DAG, and
//! only the entry block's DAG held them.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @even(i32, i32)
define i32 @odd(i32 %n, i32 %acc) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %done, label %step
step:
  %n1 = add i32 %n, -1
  %a1 = add i32 %acc, 3
  %r = tail call i32 @even(i32 %n1, i32 %a1)
  ret i32 %r
done:
  ret i32 %acc
}";

#[test]
fn a_tail_call_after_a_branch_reuses_the_callers_argument_slots() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let out = assembly(IR, cpu_of("68000").unwrap(), level).expect("compiles");
        assert!(!out.contains("jsr\teven"), "{level:?}: the call stays\n{out}");
        assert!(out.contains("even"), "{level:?}\n{out}");
    }
}
