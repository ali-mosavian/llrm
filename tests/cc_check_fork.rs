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

/// The 36 programs of tests/llc through llrm-cc on the flat Watcom row, run
/// under dosrun, against the host clang: 0/36 went unseen for a day when the
/// fork clang was missing. Needs LLRM_CLANG_M16, LLRM_OW and the pinned clang;
/// the DOS gate runs it with `cargo test --test cc_check_fork -- --ignored`.
#[test]
#[ignore = "needs LLRM_CLANG_M16, LLRM_OW, LLRM_PIN_CLANG and dosrun"]
fn the_llc_programs_all_build_and_run_at_x86_32_wat() {
    let root = env!("CARGO_MANIFEST_DIR");
    let done = Command::new("sh")
        .arg(format!("{root}/tools/cc_check.sh"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_CC_REF_FLAGS", "-m32")
        .env("LLRM_CC_RUN", format!("python3 -I {root}/tools/dosbatch/run_exe.py x86-m32"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&done.stdout);
    let ok = text.lines().filter(|line| line.starts_with("ok ")).count();
    let total = std::fs::read_dir(format!("{root}/tests/llc"))
        .unwrap()
        .filter(|one| one.as_ref().unwrap().path().extension().is_some_and(|extension| extension == "c"))
        .count();
    assert!(done.status.success() && ok == total, "{ok}/{total} ok\n{text}{}", String::from_utf8_lossy(&done.stderr));
}
