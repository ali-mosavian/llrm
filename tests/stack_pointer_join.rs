//! A saved copy of the stack pointer nothing moves is the stack pointer.

use std::process::Command;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// A constant-size array in a scope is bracketed by `stacksave` and
/// `stackrestore` with only loops between. It kept `mov rax, rsp` and
/// `mov rsp, rax`, and the write to `rsp` kept a frame register
/// (vla_callee_constant x86_64 -O2: 887 B against 880 once the array was a
/// static frame cell).
#[test]
fn a_restore_of_an_unmoved_stack_pointer_needs_no_frame_register() {
    let text = listing(
        "extern void report(long value);\nvoid f(int k)\n{\n    int n = 50, s = 0;\n    {\n        volatile int a[n];\n        int i;\n        for (i = 0; i < n; i++)\n            a[i] = i * k;\n        for (i = 0; i < n; i++)\n            s += a[i];\n    }\n    report(s);\n    report(s + 1);\n}\n",
    );
    assert!(!text.contains("mov rsp,") && !text.contains("rbp"), "{text}");
}

/// A call between the save and the restore may move the stack pointer (the
/// pin's `ADJCALLSTACK` defs): the restore stays.
#[test]
fn a_restore_after_a_call_is_kept() {
    let text = listing(
        "extern void report(long value);\nvoid f(int k)\n{\n    int n = 50, s = 0;\n    {\n        volatile int a[n];\n        int i;\n        for (i = 0; i < n; i++)\n            a[i] = i * k;\n        report(a[3]);\n        for (i = 0; i < n; i++)\n            s += a[i];\n    }\n    report(s);\n    report(s + 1);\n}\n",
    );
    assert!(text.contains("mov rsp,"), "{text}");
}
