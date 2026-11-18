use std::path::Path;
use std::process::Command;

fn assembly(function: &str) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/dso_local_implicit.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let body = text.split(&format!("\n{function}:")).nth(1).expect("function");
    body.split(".end_function").next().unwrap().to_string()
}

/// An `external` global of default visibility was taken as `dso_local`, so
/// `@e + i + 8` folded to `e+8 + i`; the pin leaves it preemptible
/// (`shouldAssumeDSOLocal`) and keeps `e` and the offset apart (+4 B per
/// access past the first on a loop of four).
#[test]
fn an_external_default_visibility_global_keeps_its_offset_out_of_the_symbol() {
    let asm = assembly("f");
    assert!(!asm.contains("preemptible+8"), "{asm}");
}

/// A hidden one is `isImplicitDSOLocal`: the offset folds into the symbol.
#[test]
fn a_hidden_global_folds_its_offset_into_the_symbol() {
    let asm = assembly("g");
    assert!(asm.contains("hidden+8"), "{asm}");
}
