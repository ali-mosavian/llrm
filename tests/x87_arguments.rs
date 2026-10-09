//! A float or double argument goes to the stack as the space made for it and a
//! store into it, not through a frame cell pushed in dwords.

use std::process::Command;

const KERNEL: &str = "
extern double f(double a, float b, int n);
double g(double x, float y) { return f(x * 0.5 + 1.0, y + 1.0f, 3); }
";

/// Each double was `fstp qword ptr [esp+16]` into a cell and two `push dword
/// ptr [esp+20]` (three instructions and a cell); gcc makes the space and
/// stores into it.
#[test]
fn test_a_float_argument_is_stored_into_the_space_made_for_it() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.contains("fstp qword ptr [esp]") && text.contains("fstp dword ptr [esp]"), "{text}");
    assert!(!text.contains("push dword ptr [esp+"), "an argument was pushed from a cell: {text}");
}
