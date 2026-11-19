//! Two sibling calls to different callees were tail-merged into one
//! (`isIdenticalTo` compared callees only for `Call`, not for the sibling-call
//! return): `if (n == 1) f(); else if (n == 2) h();` called `h` for n == 1.

use std::process::Command;

const IR: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
target triple = \"x86_64-unknown-linux-gnu\"\n\
declare void @f()\n\
declare void @h()\n\
define void @g(i32 %n) {\n\
  %a = icmp eq i32 %n, 1\n\
  br i1 %a, label %cf, label %chk\n\
chk:\n\
  %b = icmp eq i32 %n, 2\n\
  br i1 %b, label %ch, label %done\n\
cf:\n\
  tail call void @f()\n\
  ret void\n\
ch:\n\
  tail call void @h()\n\
  ret void\n\
done:\n\
  ret void\n\
}\n";

#[test]
fn test_two_sibling_calls_to_different_callees_are_not_merged() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("g.ll");
    std::fs::write(&source, IR).unwrap();
    let output =
        Command::new(env!("CARGO_BIN_EXE_llrm-llc")).args(["-O2", "-S", "-o", "-"]).arg(&source).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).unwrap();
    let jumps_to = |name: &str| {
        text.lines().any(|line| line.trim().split_once(' ').is_some_and(|(op, to)| op.starts_with('j') && to == name))
    };
    assert!(jumps_to("f") && jumps_to("h"), "{text}");
}
