//! A `double` result has no registers: it comes back through a hidden pointer,
//! as an `i64` does.

use std::process::Command;

/// `ret double` was refused ("a result the convention has no registers for"):
/// the demotion to a hidden pointer skipped every float, though soft float
/// makes it the integer of its width. float_return, double_across_a_call_result
/// and long_double_arguments in tests/run/c were refused at `-O2`.
#[test]
fn a_double_result_is_returned_through_a_hidden_pointer() {
    let dir = std::env::temp_dir().join("llrm-double-return-z80");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\ntarget triple = \"z80-unknown-none\"\n\
         define double @g(double %a) {\n  ret double %a\n}\n",
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-o", dir.join("g.s").to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    let stderr = String::from_utf8_lossy(&done.stderr);
    assert_eq!(done.status.code(), Some(0), "{stderr}");
}
