//! Narrowing and the combiner: the compile ends.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const LAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8";

/// The compile of `body` at -O2 ends within a deadline: a thread, so a loop is
/// a failure, not a machine out of memory.
fn ends(body: &str) {
    let ir = format!("target datalayout = \"{LAYOUT}\"\n{body}");
    let (done, result) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(text(&ir, "f", 0, &Request::new(Level::O2)).map(|_| ()));
    });
    let outcome = result.recv_timeout(std::time::Duration::from_secs(20)).expect("the combiner does not end");
    outcome.expect("compiles");
}

/// `or i8 (trunc a), (trunc b)` never finished (1.8 GB and growing in
/// shift_until_idioms -O2): the combiner hoisted the truncate below the `or`
/// and `ShrinkDemandedOp`, which z80's `free_narrowing` lets narrow an `i16`
/// `or` to `i8`, put it back, for ever. A thread and a deadline turn the
/// loop into a failure.
#[test]
fn an_or_of_two_truncates_is_compiled_at_o2() {
    ends(
        "define i8 @f(i16 %a, i16 %b) nounwind {\n  %x = trunc i16 %a to i8\n  %y = trunc i16 %b to i8\n  %r = or i8 %x, %y\n  ret i8 %r\n}\n",
    );
}

/// shift_until_idioms -O2: a loop that shifts an `i32` by its counter until
/// it is zero (the same cycle, found through the shift's compare).
#[test]
fn an_i32_shift_by_a_counter_loop_is_compiled_at_o2() {
    ends(
        "define i32 @f(i32 %v) {\nentry:\n  br label %l\nl:\n  %iv = phi i32 [ 0, %entry ], [ %n, %l ]\n  %s = shl i32 %v, %iv\n  %c = icmp ne i32 %s, 0\n  %n = add nsw i32 %iv, 1\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %iv\n}\n",
    );
}
