//! `main` calls `bench` twice and `bench` returns `sext (load i8)`: without
//! CodeGenPrepare's extension promotion each call kept its sign extension on
//! the wrong side of the loop, one instruction more (stack_arrays_copy -O2:
//! 2 B at 68000 and 68020).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const BENCH: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @report(i32)
define i32 @main() {
  %1 = call i32 @bench(i32 63)
  %2 = shl nsw i32 %1, 2
  %3 = call i32 @bench(i32 5)
  %4 = add nsw i32 %2, %3
  call void @report(i32 %4)
  ret i32 0
}
define internal i32 @bench(i32 %n) {
  %a = alloca [70 x i8]
  %b = alloca [70 x i8]
  br label %fill
fill:
  %i = phi i32 [ 0, %0 ], [ %i1, %fill ]
  %t = trunc i32 %i to i8
  %pa = getelementptr inbounds [70 x i8], ptr %a, i32 0, i32 %i
  store i8 %t, ptr %pa
  %i1 = add nsw i32 %i, 1
  %more = icmp slt i32 %i1, 70
  br i1 %more, label %fill, label %head
head:
  %j = phi i32 [ 0, %fill ], [ %j1, %body ]
  %m = and i32 %n, 63
  %c = icmp slt i32 %j, %m
  br i1 %c, label %body, label %done
body:
  %qa = getelementptr inbounds [70 x i8], ptr %a, i32 0, i32 %j
  %v = load i8, ptr %qa
  %qb = getelementptr inbounds [70 x i8], ptr %b, i32 0, i32 %j
  store i8 %v, ptr %qb
  %j1 = add nsw i32 %j, 1
  br label %head
done:
  %p3 = getelementptr inbounds [70 x i8], ptr %b, i32 0, i32 3
  %l = load i8, ptr %p3
  %s = sext i8 %l to i32
  ret i32 %s
}";

/// The count is the one the promotion gives (one more without it). `shl`, not `mul 100`: #9197 prices a constant
/// multiply by its cheapest chain, 6 instructions at the 68000 where the count once held 4 (33 -> 35).
#[test]
fn a_sign_extension_is_made_where_the_promoted_load_is_used() {
    for (cpu, sunk) in [("68000", 30), ("68020", 28)] {
        let out = assembly(BENCH, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        let instructions = out.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
        assert!(instructions <= sunk, "{cpu}: {instructions} instructions (promoted: {sunk}):\n{out}");
    }
}
