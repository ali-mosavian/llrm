//! A function that calls itself and copies in a loop.

use std::process::Command;

/// A self-recursive function whose copy loop becomes a memcpy call: the call's callee is a declaration the pass
/// pipeline made after its table of globals was taken, and `tailrec` indexed that table by it: `index out of bounds:
/// the len is 6 but the index is 6` (a merge sort, at -O2).
#[test]
fn a_recursive_function_with_a_loop_that_becomes_a_copy_compiles() {
    let source = "static int a[64], b[64];\nvoid m(int lo, int hi)\n{\n    int k, mid;\n    if (hi - lo < 2) return;\n    mid = (lo + hi) / 2;\n    m(lo, mid);\n    m(mid, hi);\n    for (k = lo; k < hi; ++k) a[k] = b[k];\n}\nint main(void) { m(0, 64); return a[3]; }\n";
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-m32", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
