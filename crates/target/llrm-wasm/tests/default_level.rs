use std::path::Path;
use std::process::Command;

fn assembly(level: Option<&str>) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/fill_signext.ll");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"));
    command.args(["-S", "-o", "-"]).args(level);
    let out = command.arg(ir).output().expect("llrm-wasm-llc");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `llrm-wasm-llc` without `-O` ran at -O0 while the pin's `llc` runs -O2: a
/// comparison "ours vs pin llc on the same IR" with no flag compared 209268 B
/// with 172620 B and chased a phantom gap.
#[test]
fn the_default_level_is_the_pins_o2() {
    let (default, o2, o0) = (assembly(None), assembly(Some("-O2")), assembly(Some("-O0")));
    assert_ne!(o2, o0, "the premise: this input differs between -O0 and -O2");
    assert_eq!(default, o2, "the default is not -O2");
}
