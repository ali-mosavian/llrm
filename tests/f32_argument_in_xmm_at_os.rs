//! An `f32` argument that is only a loaded value is passed in an xmm register.

use std::process::Command;

/// At -Os the dword-bits argument of the x87 stack convention (SD256) was
/// applied to an xmm register too: `movd xmm0, xmm0`, which no encoding has,
/// so `fcmp_all` and `x87_arguments` did not compile on x86_64 at -Os.
const SOURCE: &str = "extern void g(float);\nvolatile float v;\nvoid m(void) { g(v); }\n";

#[test]
fn test_a_loaded_float_argument_goes_in_xmm0_at_os_on_x86_64() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    for output in ["a.o", "a.s"] {
        let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .current_dir(scratch.path())
            .args(["-m64", "-Os", if output == "a.o" { "-c" } else { "-S" }, "-o", output, "a.c"])
            .output()
            .unwrap();
        assert!(built.status.success(), "{output}: {}", String::from_utf8_lossy(&built.stderr));
    }
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(!text.contains("movd xmm0, xmm0"), "{text}");
}
