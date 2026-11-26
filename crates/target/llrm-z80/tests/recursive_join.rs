//! A private recursion whose result joins a phi, one `add` and the `ret` was made an accumulator loop (TCE-ACC-PHI):
//! with wide arguments the loop copies them through the frame every trip. `recursive_stack_args` -Os was 634 B and
//! 8,539 T against 374 B and 6,271 T for the recursion (SDCC 331 B and 7,302 T). The pin keeps the call.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8";

fn ir() -> String {
    format!(
        "target datalayout = \"{DATALAYOUT}\"\n\
         declare void @report(i32)\n\
         define internal i32 @deep(i32 %n, i32 %a, i32 %b, i32 %c, i32 %d) {{\n\
         entry:\n  %z = icmp eq i32 %n, 0\n  br i1 %z, label %base, label %rec\n\
         base:\n  %ab = add i32 %a, %b\n  %abc = add i32 %ab, %c\n  br label %join\n\
         rec:\n  %n1 = sub i32 %n, 1\n  %an = add i32 %a, %n\n\
         \x20 %r = call i32 @deep(i32 %n1, i32 %b, i32 %c, i32 %d, i32 %an)\n  br label %join\n\
         join:\n  %t = phi i32 [ %abc, %base ], [ %r, %rec ]\n  %s = phi i32 [ %d, %base ], [ %n, %rec ]\n\
         \x20 %res = add i32 %t, %s\n  ret i32 %res\n}}\n\
         define i32 @main(i32 %n) {{\n  %v = call i32 @deep(i32 %n, i32 1, i32 2, i32 3, i32 4)\n\
         \x20 call void @report(i32 %v)\n  ret i32 0\n}}\n"
    )
}

#[test]
fn a_recursive_call_through_a_join_stays_a_call_on_z80() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let asm = text(&ir(), "deep", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        assert!(
            asm.lines().any(|line| line.contains("call") && line.contains("deep")),
            "{level:?}: the recursion was made a loop:\n{asm}"
        );
    }
}
