//! The DAG path is the only selector: what it refuses is a compile error that
//! names the function and the node, not a silent second selector's code.

use std::process::Command;

const HEAD: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\ntarget triple = \"aarch64-unknown-linux-gnu\"\n";

/// A call in `coldcc`: the pin has no calling-convention table for it
/// (`CCAssignFnForCall`), so it is refused. With a hand selector behind the
/// DAG it compiled to a `bl` placed by other rules.
#[test]
fn a_call_in_a_convention_the_pin_has_no_table_for_is_a_compile_error_naming_the_node() {
    let scratch = tempfile::tempdir().unwrap();
    let (source, out) = (scratch.path().join("c.ll"), scratch.path().join("c.o"));
    std::fs::write(
        &source,
        format!("{HEAD}declare coldcc i64 @cold(i64)\ndefine i64 @f(i64 %a) {{\n  %r = call coldcc i64 @cold(i64 %a)\n  ret i64 %r\n}}\n"),
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-O2", "-o"])
        .arg(&out)
        .arg(&source)
        .output()
        .expect("the driver runs");
    let stderr = String::from_utf8_lossy(&done.stderr);
    assert_eq!(done.status.code(), Some(1), "{stderr}");
    assert!(!out.exists(), "wrote an object");
    assert!(stderr.contains("@f"), "names the function: {stderr}");
    assert!(stderr.contains("calling convention"), "names the node: {stderr}");
    assert!(!stderr.contains("isel-shadow"), "no second selector: {stderr}");
}
