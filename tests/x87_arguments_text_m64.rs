//! x87_arguments at -Os m64 grew from 37 to 253 B of `.text` when contraction
//! went on for x86 (d8ea648ff3): the `fmuladd` in the unrollable loops was
//! charged as a call and the loops stayed rolled.

use std::process::Command;

#[test]
fn test_x87_arguments_at_os_m64_is_no_larger_than_main() {
    let scratch = tempfile::tempdir().unwrap();
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/run/c/x87_arguments.c");
    let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-Os", "-c", "-o", "a.o", source])
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let sizes = Command::new("size").current_dir(scratch.path()).args(["-A", "a.o"]).output().unwrap();
    let sizes = String::from_utf8_lossy(&sizes.stdout).into_owned();
    let text: u64 = sizes
        .lines()
        .find_map(|line| line.strip_prefix(".text").map(|rest| rest.split_whitespace().next().unwrap().parse().unwrap()))
        .unwrap_or_else(|| panic!("{sizes}"));
    assert!(text <= 37, ".text is {text} B (main: 37)\n{sizes}");
}
