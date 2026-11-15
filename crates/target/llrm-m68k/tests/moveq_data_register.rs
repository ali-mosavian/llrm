//! bench/switch -Os (68000): the arms of `route` returned a constant, each
//! `move.w #n, %a1` (4 B), `bench_switch` 308 B against gcc's 262. The
//! allocator tries `a1`, `a0` before `d0`, `d1` for a value either kind of
//! register can hold (M68K-FLEXORDER), and a `moveq` constant read only by
//! data operands was such a value. The pin tries the data registers first,
//! where `MOVI32ri` is a `moveq`. Now `moveq #n, %dN` (2 B), 292 B.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @route(i32 %op, i32 %sum) optsize {
entry:
  switch i32 %op, label %other [
    i32 0, label %a
    i32 37, label %b
    i32 148, label %c
    i32 333, label %d
    i32 592, label %e
    i32 925, label %f
  ]
a: br label %join
b: br label %join
c: br label %join
d: br label %join
e: br label %join
f: br label %join
other: br label %join
join:
  %k = phi i32 [ 11, %a ], [ 7, %b ], [ 23, %c ], [ 5, %d ], [ 19, %e ], [ 2, %f ], [ 1, %other ]
  %r = add i32 %k, %sum
  ret i32 %r
}
"#;

#[test]
fn a_constant_moveq_makes_is_held_in_a_data_register() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(IR, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(asm.contains("moveq\t#23,"), "{cpu}: the arms make their constants:\n{asm}");
        let wide = asm.lines().filter(|line| line.trim_start().starts_with("move.w\t#") && line.contains("%a")).count();
        assert_eq!(wide, 0, "{cpu}: a constant moveq makes goes to an address register:\n{asm}");
    }
}
