//! A loop that counts `n` down and ends on `n - 1 == 0` ran `addq; cmpi.l
//! #-1; bne` on a second, counted-up register (tail_calls `countdown`:
//! +22 clk a trip on the 68000, +7.0% on the cell against gcc). LSR's own
//! model rates that rewrite worse than the loop as written; the loop keeps
//! the flags of its own `subq`.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @countdown(i32 %n, i32 %acc) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %done, label %loop
loop:
  %a = phi i32 [ %acc, %entry ], [ %a2, %loop ]
  %i = phi i32 [ %n, %entry ], [ %i2, %loop ]
  %i2 = add nsw i32 %i, -1
  %m = mul nsw i32 %a, 3
  %a2 = add nsw i32 %m, %i
  %e = icmp eq i32 %i2, 0
  br i1 %e, label %done, label %loop
done:
  %r = phi i32 [ %acc, %entry ], [ %a2, %loop ]
  ret i32 %r
}";

#[test]
fn a_countdown_loop_ends_on_its_own_decrement() {
    for level in [Level::O1, Level::O2, Level::O3] {
        let out = assembly(IR, cpu_of("68000").unwrap(), level).expect("compiles");
        assert!(!out.contains("cmpi.l\t#-1"), "{level:?}: a second counter and a compare\n{out}");
    }
}
