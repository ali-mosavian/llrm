//! `--isel=dag`: the first function through the pin's AArch64 table to assembly.

use std::process::Command;

const IR: &str = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i32 @f(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}\n\
define i32 @g(ptr %p, i32 %b) {\n  %r = atomicrmw add ptr %p, i32 %b seq_cst\n  ret i32 %r\n}\n";

fn llc(flag: &str) -> (bool, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, IR).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "--no-opt", flag, "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    (
        said.status.success(),
        std::fs::read_to_string(&output).unwrap_or_default(),
        String::from_utf8_lossy(&said.stderr).into_owned(),
    )
}

/// `f` comes out of the DAG path as the pin's `add w0, w0, w1` / `ret`; `g`
/// (an atomic add) is refused by `dag`, and by `shadow` counted and made
/// by the hand selector.
#[test]
fn f_is_the_pins_add_and_ret_and_g_falls_back_in_shadow() {
    let (ok, _, why) = llc("--isel=dag");
    assert!(!ok && why.contains("@g"), "dag refuses g: {why}");

    let (ok, asm, why) = llc("--isel=shadow");
    assert!(ok, "{why}");
    assert!(why.contains("isel-shadow: @g"), "the fallback is counted: {why}");
    assert!(!why.contains("isel-shadow: @f"), "f needs no fallback: {why}");
    let f = &asm[asm.find("_f:").unwrap()..asm.find("_g:").unwrap()];
    let lines: Vec<&str> = f.lines().skip(1).map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('.')).collect();
    assert_eq!(lines, ["add w0, w0, w1, lsl #0", "ret x30"], "{f}");
    // The printer spells an `ADDWrs` with a zero shift this way; it encodes as `add w0, w0, w1`.
    let (_, hand, _) = llc("--isel=hand");
    assert_eq!(&hand[hand.find("_f:").unwrap()..hand.find("_g:").unwrap()], f, "the hand selector's f is the same");
}

/// The pin selects `sub` as `SUBSWrr` with a dead `$nzcv` and `optimizeCompareInstr` turns it into
/// `SUBWrr`. The DAG path printed `subs w0, w0, w1` for a plain `sub`: a flag write nothing reads.
#[test]
fn a_sub_whose_flags_nothing_reads_is_not_flag_setting() {
    let ir = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i32 @s(i32 %a, i32 %b) {\n  %r = sub i32 %a, %b\n  ret i32 %r\n}\n\
define i32 @t(i32 %a) {\n  %r = add i32 %a, -1\n  ret i32 %r\n}\n\
define i64 @u(i64 %a, i64 %b) {\n  %r = sub i64 %a, %b\n  ret i64 %r\n}\n";
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, ir).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "--no-opt", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(!asm.contains("subs"), "a flag write nothing reads:\n{asm}");
    for want in ["sub w0, w0, w1", "sub w0, w0, #1", "sub x0, x0, x1"] {
        assert!(asm.contains(want), "{want}:\n{asm}");
    }
}

/// The pin's MachineSink moves the zero a phi takes from the entry onto that edge. The DAG path
/// built it in the entry block, so it ran on both paths: `orr x0, xzr, xzr` before the branch.
#[test]
fn a_constant_only_one_edge_reads_is_made_on_that_edge() {
    let ir = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i32 @f(i32 %a, i32 %b) {\n  %c = icmp sgt i32 %a, 0\n  br i1 %c, label %t, label %e\n\
t:\n  %m = mul i32 %b, %a\n  br label %e\n\
e:\n  %r = phi i32 [ 0, %0 ], [ %m, %t ]\n  ret i32 %r\n}\n";
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, ir).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "--no-opt", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    let entry = asm.split(".Lf.1:").next().unwrap();
    assert!(!entry.contains("xzr, xzr"), "the zero is made before the branch that skips its use:\n{asm}");
}

/// The pin's MachineCSE merges the dominated `SUBSWri a, 1` with the compare's, so the compare
/// defines the value (`subs w8, w0, #1`). The DAG path kept `subs wzr, w0, #1` in the entry block
/// and a second `sub w9, w0, #1` after the branch: 2 instructions for the pin's 1.
#[test]
fn a_subtract_after_a_compare_of_the_same_operands_shares_it() {
    let ir = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i32 @f(i32 %a, i32 %b) {\n  %c = icmp sgt i32 %a, 0\n  br i1 %c, label %t, label %e\n\
t:\n  %d = add nsw i32 %a, -1\n  %m = mul i32 %b, %d\n  ret i32 %m\n\
e:\n  ret i32 0\n}\n";
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, ir).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "--no-opt", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(!asm.contains("subs wzr"), "the compare should define the difference:\n{asm}");
    assert_eq!(asm.matches("sub").count(), 1, "one subtract, not a compare and a subtract:\n{asm}");
}
