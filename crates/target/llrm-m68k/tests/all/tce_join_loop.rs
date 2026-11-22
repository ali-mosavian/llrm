//! TCE-ACC-PHI stays on for m68k, whose bar has gcc's `pass_tail_recursion`:
//! a recursive call whose result joins a phi, an `add` and the `ret` is a
//! loop, not a `jsr` of itself. (wasm turned it off, `recursive_join` there.)

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DEEP: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @deep(i32 %n, i32 %a, i32 %b, i32 %c, i32 %d) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %base, label %rec
base:
  %ab = add i32 %a, %b
  %abc = add i32 %ab, %c
  br label %join
rec:
  %n1 = sub i32 %n, 1
  %an = add i32 %a, %n
  %r = call i32 @deep(i32 %n1, i32 %b, i32 %c, i32 %d, i32 %an)
  br label %join
join:
  %t = phi i32 [ %abc, %base ], [ %r, %rec ]
  %s = phi i32 [ %d, %base ], [ %n, %rec ]
  %res = add i32 %t, %s
  ret i32 %res
}
"#;

#[test]
fn a_recursive_call_through_a_join_is_a_loop_on_m68k() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let out = assembly(DEEP, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(!out.contains("jsr\tdeep") && !out.contains("bsr\tdeep"), "{cpu} {level:?}: still a call:\n{out}");
        }
    }
}
