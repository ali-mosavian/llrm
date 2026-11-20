//! switch -O2 (1298 B against 703 at -Os, SDCC 1103): `main` inlined
//! `bench_switch(2000)` and the external body stayed, +402 B for the same
//! clocks. The known-loop-bound hint kept the inline for a copy that folds, but
//! a bound over what a full unroll takes folds nothing.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define i32 @spin(i16 %n) nounwind {
entry:
  br label %body
body:
  %i = phi i16 [ 0, %entry ], [ %i1, %body ]
  %s = phi i32 [ 1, %entry ], [ %t, %body ]
  %acc = phi i32 [ 0, %entry ], [ %acc1, %body ]
  %m = mul i32 %s, 75
  %a = add i32 %m, 74
  %t = urem i32 %a, 65537
  %d = urem i32 %t, 10
  %q = mul i32 %d, %d
  %q2 = mul i32 %q, 37
  %acc1 = add i32 %acc, %q2
  %i1 = add i16 %i, 1
  %c = icmp ult i16 %i1, %n
  br i1 %c, label %body, label %exit
exit:
  ret i32 %acc1
}
define i32 @caller() nounwind {
  %r = call i32 @spin(i16 2000)
  ret i32 %r
}
"#;

#[test]
fn a_bound_too_big_to_unroll_does_not_keep_a_growing_inline_at_o2() {
    let asm = text(IR, "caller", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(asm.contains("_spin"), "caller copied the body of spin:\n{asm}");
}
