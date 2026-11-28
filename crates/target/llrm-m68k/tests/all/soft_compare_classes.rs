//! `fcmp_pair_shared` (68020, -O2): the six predicates of one float pair called
//! six routines (`__ltsf2 __eqsf2 __gtsf2 __nesf2 __lesf2 __gesf2`), 216
//! compare calls executed against gcc's 139. The compare routines are two
//! cores (`__lesf2`: eq, ne, lt, le; `__gesf2`: gt, ge), so two calls answer
//! all six.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define dso_local i32 @six_f(float noundef %a, float noundef %b) {
  %1 = fcmp olt float %a, %b
  %2 = fcmp oeq float %a, %b
  %3 = fcmp ogt float %a, %b
  %4 = fcmp une float %a, %b
  %5 = fcmp ole float %a, %b
  %6 = fcmp oge float %a, %b
  %z1 = zext i1 %1 to i32
  %z2 = select i1 %2, i32 2, i32 0
  %z3 = select i1 %3, i32 4, i32 0
  %z4 = select i1 %4, i32 8, i32 0
  %z5 = select i1 %5, i32 16, i32 0
  %z6 = select i1 %6, i32 32, i32 0
  %s1 = add i32 %z1, %z2
  %s2 = add i32 %s1, %z3
  %s3 = add i32 %s2, %z4
  %s4 = add i32 %s3, %z5
  %s5 = add i32 %s4, %z6
  ret i32 %s5
}
"#;

#[test]
fn the_six_predicates_of_one_pair_are_two_compare_calls() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(IR, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        let mut calls: Vec<&str> =
            asm.lines().filter_map(|line| line.trim().strip_prefix("jsr")).map(str::trim).collect();
        calls.sort();
        assert_eq!(calls, ["__gesf2", "__lesf2"], "{cpu}:\n{asm}");
    }
}
