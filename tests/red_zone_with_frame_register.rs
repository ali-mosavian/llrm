//! A leaf function keeps its frame register and still lives in the red zone.

use std::process::Command;

/// `push rbp; mov rbp, rsp; sub rsp, 16` for a leaf function: the pin
/// (X86FrameLowering::emitPrologue, `has128ByteRedZone`) skips the `sub`
/// whether or not a frame register is kept, 4 bytes per leaf at -O0.
#[test]
fn a_leaf_with_a_frame_register_has_no_sub_rsp() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int leaf(int a, long b)\n{\n    long c = a + b;\n    return (int)c * 3;\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("push rbp") && asm.contains("pop rbp"), "{asm}");
    assert!(!asm.contains("sub rsp") && !asm.contains("leave"), "{asm}");
}

/// The red zone took a function with a variable-sized object (`int a[n]`):
/// `tests/llc/vla.c` printed nothing at -O0 (the pin: `!hasVarSizedObjects()`).
#[test]
fn a_function_with_a_vla_does_not_use_the_red_zone() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "#include <stdio.h>\nstatic int f(int n)\n{\n    int a[n];\n    for (int i = 0; i < n; i++)\n        a[i] = i * i;\n    int s = 0;\n    for (int i = 0; i < n; i++)\n        s += a[i];\n    return s;\n}\nint main(void)\n{\n    printf(\"%d\\n\", f(10));\n    return 0;\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .current_dir(scratch.path())
        .args(["-O0", "a.c", "-o", "a.out"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let run = Command::new(scratch.path().join("a.out")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&run.stdout), "285\n", "status {:?}", run.status);
}
