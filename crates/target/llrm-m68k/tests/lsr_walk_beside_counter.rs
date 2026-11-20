//! `sum += cells[i] * (i + 1)`: the counter `i` stays live for the multiply, yet LSR's end-biased form (`lea` the end,
//! `neg.l` the length, `(0,%a0,%d2)`) replaced the pointer walk: it saved a compare the live counter still pays, and
//! lost the `(%a0)+` that is free on the 68000 (loop_versioning -O3, +3.4% clocks against gcc).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@cells = global [256 x i16] zeroinitializer, align 2
define i32 @total(i16 %n) {
  %c0 = icmp sgt i16 %n, 0
  br i1 %c0, label %pre, label %done
pre:
  %trip = zext nneg i16 %n to i32
  br label %l
l:
  %i = phi i32 [ 0, %pre ], [ %j, %l ]
  %s = phi i32 [ 0, %pre ], [ %t, %l ]
  %p = getelementptr inbounds nuw [2 x i8], ptr @cells, i32 %i
  %v = load i16, ptr %p, align 2
  %ve = sext i16 %v to i32
  %j = add nuw nsw i32 %i, 1
  %m = mul nsw i32 %j, %ve
  %t = add nsw i32 %m, %s
  %c = icmp eq i32 %j, %trip
  br i1 %c, label %exit, label %l
exit:
  br label %done
done:
  %r = phi i32 [ 0, %0 ], [ %t, %exit ]
  ret i32 %r
}";

#[test]
fn a_live_counter_keeps_the_pointer_walk_not_the_end_biased_index() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::O3] {
            let out = assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(out.contains(")+"), "{cpu} {level:?}:\n{out}");
            assert!(!out.contains("neg.l"), "{cpu} {level:?}:\n{out}");
        }
    }
}
