//! switch_jump_table -Os (68000/68020, +64..+70 B against gcc; -O1 +64/+66
//! too): `for (op = 248; op < 258; op++) report(byte(op, 100 + op))` kept a
//! loop and a six-entry jump table (302 B against gcc's 232): the -Os and -O1
//! trials held the loop to its copies' size (22 units a trip, 220), where each
//! copy keeps one arm of the switch (the analysis finds 30). Unrolled, the
//! table and its dispatch go.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
declare void @report(i32)
define i32 @main() optsize {
  br label %1

1:
  %2 = phi i8 [ %25, %20 ], [ -8, %0 ]
  %3 = phi i32 [ %24, %20 ], [ 348, %0 ]
  %4 = phi i32 [ %23, %20 ], [ -348, %0 ]
  %5 = phi i32 [ 248, %0 ], [ %22, %20 ]
  %6 = add nuw nsw i32 %5, 100
  switch i8 %2, label %19 [
    i8 -6, label %7
    i8 -5, label %9
    i8 -4, label %11
    i8 -3, label %13
    i8 -2, label %15
    i8 -1, label %17
  ]

7:
  %8 = add nuw nsw i32 %5, 101
  br label %20

9:
  %10 = mul nuw nsw i32 %6, 3
  br label %20

11:
  %12 = add nuw nsw i32 %5, 96
  br label %20

13:
  %14 = xor i32 %3, 9
  br label %20

15:
  %16 = or i32 %3, 16
  br label %20

17:
  %18 = lshr i32 %3, 1
  br label %20

19:
  br label %20

20:
  %21 = phi i32 [ %4, %19 ], [ %8, %7 ], [ %10, %9 ], [ %12, %11 ], [ %14, %13 ], [ %16, %15 ], [ %18, %17 ]
  tail call void @report(i32 %21)
  %22 = add nuw nsw i32 %5, 1
  %23 = add nsw i32 %4, -1
  %24 = add nuw nsw i32 %3, 1
  %25 = add nsw i8 %2, 1
  %exitcond = icmp eq i32 %23, -358
  br i1 %exitcond, label %26, label %1

26:
  ret i32 0
}
"#;

#[test]
fn a_loop_whose_copies_fold_to_a_few_calls_is_unrolled_at_size() {
    for cpu in ["68000", "68020"] {
        for level in [Level::Os] {
            let asm = assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(!asm.contains(".LJTI"), "{cpu} {level:?}: the jump table stayed:\n{asm}");
        }
    }
}
