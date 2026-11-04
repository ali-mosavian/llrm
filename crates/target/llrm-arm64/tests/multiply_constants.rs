//! A multiply by a constant is the shifts and adds the pin emits
//! (`performMulCombine`), not a `movz` and a `madd`.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/multiply_constants.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// `x * 6`, `x * 10`, `x * 12` and `x * -3` were a `movz` and a `madd` (a
/// multiplier's latency, and one more instruction for the negatives): the pin
/// has `add` and `lsl`, or `sub` of a shifted `x`.
#[test]
fn a_constant_of_shifts_and_adds_has_no_multiply() {
    for name in ["xm6", "xm10", "xm12", "xm24", "xn3", "xn7", "xn6", "xn12", "wm6", "wn3"] {
        let text = body(name);
        assert!(!text.contains("madd") && !text.contains("movz") && !text.contains("movn"), "{name}: {text}");
    }
    assert_eq!(
        body("xn3").lines().filter(|line| line.starts_with('\t') || line.starts_with(' ')).count(),
        2,
        "sub, ret"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn the_products_are_the_values_c_gives() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let llc = std::path::Path::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/multiply_constants.ll");
    let object = dir.join("f.o");
    let said = std::process::Command::new(llc).args(["-O2", "-o"]).arg(&object).arg(ir).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let driver = dir.join("main.c");
    std::fs::write(&driver, include_str!("data/multiply_constants.c")).unwrap();
    let program = dir.join("prog");
    let built = std::process::Command::new("cc").arg(&driver).arg(&object).arg("-o").arg(&program).output().unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let ran = std::process::Command::new(&program).output().unwrap();
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stdout));
}
