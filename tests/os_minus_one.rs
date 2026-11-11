//! -Os loads -1 with `or r, -1` where the flags are dead.

mod common;

/// `mov eax, -1` is five bytes, `or eax, -1` three: -Os kept the `mov` at
/// every dead-flags site.
const SOURCE: &str = "int g, h;\nint f(int x) { if (x) { g = -1; h = -1; return -1; } return x + 3; }\n";

fn assembled(level: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_32-wat", "-m32", "-mabi=sysv", "-march=i486", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

#[test]
fn test_minus_one_is_or_at_size_and_mov_at_speed() {
    let size = assembled("-Os");
    assert!(size.contains("or eax, -1") && !size.contains("mov eax, -1"), "{size}");
    let speed = assembled("-O2");
    assert!(speed.contains("mov eax, -1") && !speed.contains("or eax, -1"), "{speed}");
}
