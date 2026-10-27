//! `tools/cc_check.sh` on a DOS row without the pinned fork clang failed 36
//! programs with `_atan` undefined (0/36 on 2026-10-27); it now stops at once
//! and says why.

use std::process::Command;

fn run(fork: Option<&str>) -> (i32, String) {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut command = Command::new("sh");
    command.arg(format!("{root}/tools/cc_check.sh")).env("LLRM_CC_TARGET", "x86_32-wat").env_remove("LLRM_CLANG_M16");
    if let Some(fork) = fork {
        command.env("LLRM_CLANG_M16", fork);
    }
    let done = command.output().unwrap();
    (done.status.code().unwrap_or(-1), String::from_utf8_lossy(&done.stderr).into_owned())
}

#[test]
fn an_unset_fork_clang_is_one_clear_line_not_link_errors() {
    let (code, error) = run(None);
    assert_eq!(code, 2, "{error}");
    assert!(error.contains("needs LLRM_CLANG_M16") && error.lines().count() == 1, "{error}");
}

#[test]
fn a_clang_that_is_not_the_pinned_fork_is_refused() {
    let (code, error) = run(Some("/bin/true"));
    assert_eq!(code, 2, "{error}");
    assert!(error.contains("toolchain/clang/pin"), "{error}");
}
