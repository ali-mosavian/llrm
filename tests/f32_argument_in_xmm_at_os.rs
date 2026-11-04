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

fn at_os(
    target: &str,
    output: &str,
) -> Vec<u8> {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .env("LLRM_CC_TARGET", target)
        .args(["-Os", "-S", "-o", output, "a.c"])
        .output()
        .unwrap();
    assert!(built.status.success(), "{target}: {}", String::from_utf8_lossy(&built.stderr));
    std::fs::read(scratch.path().join(output)).unwrap()
}

/// arm64 -Os passed the float in `w0` (`fmov w0, s16`): the callee reads `s0`.
#[test]
fn test_a_loaded_float_argument_goes_in_s0_at_os_on_arm64() {
    let text = String::from_utf8(at_os("arm64-linux", "a.s")).unwrap();
    assert!(!text.contains("fmov w0"), "{text}");
}

/// wasm32 -Os called `g` with an `i32.reinterpret_f32` (0xbc) of the load
/// where `g` takes an `f32`: the module did not validate.
#[test]
fn test_a_loaded_float_argument_is_not_reinterpreted_at_os_on_wasm() {
    let module = at_os("wasm32", "a.wasm");
    assert!(!module.windows(2).any(|pair| pair == [0x00, 0xbc]), "{module:02x?}");
}
