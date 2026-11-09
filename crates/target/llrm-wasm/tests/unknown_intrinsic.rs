use std::path::Path;
use std::process::Command;

/// `llvm.frameaddress.p0` has no lowering here. It was emitted as `call llvm.frameaddress.p0` with a stray `i32.const 0`
/// argument: an import named after the intrinsic that no host provides (the pin lowers it to the stack pointer).
#[test]
fn an_intrinsic_with_no_lowering_is_refused_by_name() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unknown_intrinsic.ll");
    for level in ["-O0", "-O2"] {
        let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc")).args([level, "-S", "-o", "-"]).arg(&ir).output().expect("llrm-wasm-llc");
        let (out, err) = (String::from_utf8_lossy(&made.stdout), String::from_utf8_lossy(&made.stderr));
        assert!(!made.status.success(), "{level}: compiled to\n{out}");
        assert!(err.contains("llvm.frameaddress.p0"), "{level}: {err}");
        assert!(!out.contains("call\tllvm."), "{level}: {out}");
    }
}
