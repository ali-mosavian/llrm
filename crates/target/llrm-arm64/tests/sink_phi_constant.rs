//! A phi's constant is made on the edge that carries it, not in the entry.

use std::process::Command;

/// `power` of recpow at -O2: `movz w9, #1` ran in the entry, took x9, and the return paid
/// `orr x0, xzr, x9` (+4.2% executed against the pin). The constant now sits on its own edge,
/// in x0, as the pin has it.
#[test]
fn a_constant_returned_through_a_phi_is_made_on_its_own_edge() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let (source, out) = (dir.join("f.ll"), dir.join("f.s"));
    std::fs::write(
        &source,
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
target triple = \"aarch64-unknown-linux-gnu\"\ndeclare i64 @g(i64, i32)\n\
define i64 @f(i64 %b, i32 %e) {\n  %c = icmp eq i32 %e, 0\n  br i1 %c, label %j, label %n\n\
n:\n  %r0 = call i64 @g(i64 %b, i32 %e)\n  %r = mul i64 %r0, %b\n  br label %j\n\
j:\n  %p = phi i64 [ 1, %0 ], [ %r, %n ]\n  ret i64 %p\n}\n",
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(&out).unwrap();
    let entry = asm.split(".Lf.1:").next().unwrap();
    assert!(!entry.contains("movz"), "the constant is made before the branch that skips its use:\n{asm}");
    assert!(asm.contains("movz w0, #1"), "the constant is made in the return register:\n{asm}");
}
