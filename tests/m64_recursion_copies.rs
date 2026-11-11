//! A two-call recursion is copied into itself two deep at m64 -O2, not eight.

mod common;

/// Copies eight deep left 41 `call`s in a lattice-path count (rectwo -O2:
/// 719563 instructions, 2120 B); two deep leaves 8 (161513, 392 B).
#[test]
fn at_o2_a_two_call_recursion_is_copied_two_deep() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "static long paths(int n, int m) { if (n == 0 || m == 0) return 1; return paths(n - 1, m) + paths(n, m - 1); }\n\
         long f(int n) { return paths(n, n); }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m64", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let calls = text.matches("call").count();
    assert!(calls <= 16, "{calls} calls\n{text}");
}
