//! `--isel=` of `llrm-arm64-llc` chooses the instruction selector; no
//! environment variable does.

use std::process::Command;

/// A remainder by a constant: the two selectors lower it differently.
const IR: &str = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i32 @f(i32 %a) {\n %r = urem i32 %a, 7\n ret i32 %r\n}\n";

fn run(
    flags: &[&str],
    env: &[(&str, &str)],
) -> std::process::Output {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, IR).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2"])
        .args(flags)
        .arg("-o")
        .arg(&output)
        .arg(&input)
        .envs(env.iter().copied())
        .output()
        .unwrap();
    if said.status.success() {
        return std::process::Output { stdout: std::fs::read(&output).unwrap(), ..said };
    }
    said
}

/// The selector was chosen by `LLRM_ISEL_DAG` / `LLRM_ISEL_SHADOW` that the
/// option set in the environment: a stray variable changed any compile.
#[test]
fn the_environment_does_not_choose_the_selector() {
    let hand = run(&["--isel=hand"], &[]);
    assert!(hand.status.success(), "{}", String::from_utf8_lossy(&hand.stderr));
    // Premise: the two selectors differ on this function.
    let dag = run(&["--isel=dag"], &[]);
    assert!(dag.status.success(), "{}", String::from_utf8_lossy(&dag.stderr));
    assert_ne!(hand.stdout, dag.stdout);
    for variable in ["LLRM_ISEL_DAG", "LLRM_ISEL_SHADOW"] {
        assert_eq!(run(&[], &[(variable, "1")]).stdout, hand.stdout, "{variable}");
    }
}

#[test]
fn an_unknown_selector_is_refused() {
    let said = run(&["--isel=bogus"], &[]);
    assert_eq!(said.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&said.stderr).contains("choose hand, dag or shadow"));
}
