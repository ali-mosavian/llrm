//! A shrink-wrapped frame restore off a return must not pop into a register.

mod common;

/// `f` closed its one-slot frame mid-function with `pop rcx` (the scratch the
/// return path picks), clobbering `rcx` for the code after the close; the pin
/// pops only at returns (`findDeadCallerSavedReg` is 0 elsewhere) and uses
/// `add rsp, 8` otherwise.
const SOURCE: &str = "extern void a(int);\nlong f(int n) { for (int r = 0; r < n; r++) a(r); return 0; }\n";

#[test]
fn test_a_mid_function_frame_close_adds_to_rsp_instead_of_popping_a_scratch() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_64-linux", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.contains("add rsp, 8"), "{text}");
    assert!(!text.contains("pop rcx"), "{text}");
}
