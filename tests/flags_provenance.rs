//! A zero test of a value an instruction has just computed is that instruction's flags.

use std::process::Command;

const KERNEL: &str = "
extern int g;
extern int h(int);
int f(int x)
{
    int d = x - g;
    if (d == 0) return h(1);
    return h(2) + d;
}
";

/// `sub ebx, [g]; or ebx, ebx; jne` tested what the `sub` had set: the pass that folds the test into its setter
/// moved the setter and so wanted its operands in registers, though nothing stood between the two (queens' `safe`:
/// `sub esi, [esp+32]; or esi, esi; je`; gcc's `add; je`, LLVM's `optimizeCompareInstr`).
#[test]
fn test_a_zero_test_right_after_the_instruction_that_computed_it_is_not_made() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(directory).args(["-m32", "-mabi=sysv", "-O2", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let tested = text.lines().map(str::trim).any(|line| line.strip_prefix("or ").is_some_and(|rest| rest.split_once(", ").is_some_and(|(left, right)| left == right)));
    assert!(!tested, "{text}");
}
