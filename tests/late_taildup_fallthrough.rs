//! Post-RA tail duplication keeps the last instruction of a predecessor that
//! falls into the tail.

mod common;

/// `__clzdi2` came out of m64 -O2 without `add eax, 32`: the predecessor fell
/// into the shared return, and its last instruction was dropped as if it were
/// the jump to the tail. A wrong result, and 3 bytes "smaller".
#[test]
fn a_predecessor_that_falls_into_the_tail_keeps_its_last_instruction() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "int g(unsigned x);\nint f(unsigned long x) { unsigned hi = x >> 32; return hi ? g(hi) : 32 + g((unsigned)x); }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["--target=x86_64-linux", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert!(text.contains("add eax, 32"), "{text}");
}
