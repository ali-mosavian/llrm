//! Narrowing and the combiner: the compile ends.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `or i8 (trunc a), (trunc b)` never finished (1.8 GB and growing in
/// shift_until_idioms -O2): the combiner hoisted the truncate below the `or`
/// and `ShrinkDemandedOp`, which z80's `free_narrowing` lets narrow an `i16`
/// `or` to `i8`, put it back, for ever. A thread and a deadline turn the
/// loop into a failure.
#[test]
fn an_or_of_two_truncates_is_compiled_at_o2() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define i8 @f(i16 %a, i16 %b) nounwind {
  %x = trunc i16 %a to i8
  %y = trunc i16 %b to i8
  %r = or i8 %x, %y
  ret i8 %r
}
"#;
    let (done, result) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(text(ir, "f", 0, &Request::new(Level::O2)).map(|_| ()));
    });
    let outcome = result.recv_timeout(std::time::Duration::from_secs(20)).expect("the combiner does not end");
    outcome.expect("compiles");
}
