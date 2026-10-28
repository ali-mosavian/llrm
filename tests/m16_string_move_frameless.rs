//! A function whose only inline code is a string move keeps no frame register.

mod common;

/// scroll.c under the DAG selector had `push bp; mov bp, sp` where the old one
/// had none: the backward move is inline bytes, and any inline code was taken
/// to address the frame (+4 B a call, scroll -4).
#[test]
fn test_a_backward_string_move_needs_no_frame() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "static short s[2000];\nvoid f(void)\n{\n    int i;\n    for (i = 1919; i >= 0; --i) s[i + 80] = s[i];\n}\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["a.c", "-m16", "-march=i386", "-Os", "--isel=dag", "-S", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert!(asm.contains("db 01eh,007h,0fdh"), "not the inline move:\n{asm}");
    assert!(!asm.contains("push bp"), "{asm}");
}
