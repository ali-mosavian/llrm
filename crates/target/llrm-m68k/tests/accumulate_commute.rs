//! `t += (long)a[i] * b[i]` in a loop tied the product, not the accumulator:
//! `muls %d3,%d1; add.l %d0,%d1; move.l %d1,%d0` every trip, a 2-byte `move`
//! more than gcc's `add.l %d1,%d0` (bench/dot -Os 116 B against gcc's 114).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @dot(ptr %a, ptr %b, i16 %n) {
entry:
  %go = icmp sgt i16 %n, 0
  br i1 %go, label %loop, label %done
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %t = phi i32 [ 0, %entry ], [ %sum, %loop ]
  %pa = getelementptr i16, ptr %a, i16 %i
  %pb = getelementptr i16, ptr %b, i16 %i
  %x = load i16, ptr %pa
  %y = load i16, ptr %pb
  %xs = sext i16 %x to i32
  %ys = sext i16 %y to i32
  %prod = mul i32 %xs, %ys
  %sum = add i32 %t, %prod
  %next = add i16 %i, 1
  %more = icmp slt i16 %next, %n
  br i1 %more, label %loop, label %done
done:
  %r = phi i32 [ 0, %entry ], [ %sum, %loop ]
  ret i32 %r
}";

#[test]
fn an_accumulating_add_after_a_multiply_has_no_copy_in_the_loop() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = assembly(IR, cpu_of("68000").unwrap(), level).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        let body = text.split("dot:").nth(1).and_then(|rest| rest.split("rts").next()).expect("dot");
        let looped = body.split("muls").nth(1).expect("a multiply in the loop");
        let after_add = looped.split("add.l").nth(1).expect("the accumulating add");
        let next = after_add.lines().nth(1).unwrap_or("");
        assert!(!next.contains("move.l"), "{level:?}: the sum is copied back\n{body}");
    }
}
