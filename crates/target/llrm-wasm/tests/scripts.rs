//! The pin tools the scripts need come from the environment: a script run
//! without them stops by name and does not look in a private directory.

use std::path::Path;
use std::process::Command;

fn run_bare(script: &str) -> (bool, String) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(script);
    let out = Command::new("bash")
        .arg(path)
        .env_remove("LLC")
        .env_remove("LLRM_PIN_LLC")
        .env_remove("LLRM_PIN_SRC")
        .env_remove("LLRM_PIN_BUILD")
        .output()
        .expect("bash");
    (out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned())
}

/// `compare.sh` and `regen.sh` defaulted to `$HOME/scratch/...`: on any other
/// machine they ran against a missing path, or another user's build.
#[test]
fn the_scripts_name_the_pin_tool_they_need() {
    for (script, variable) in [("compare.sh", "LLRM_PIN_LLC"), ("regen.sh", "LLRM_PIN_SRC")] {
        let (ok, why) = run_bare(script);
        assert!(!ok && why.contains(variable), "{script}: {why}");
    }
}

/// `-o -` wrote a file named `-` into the working directory (and was committed
/// by mistake) instead of the text to standard output.
#[test]
fn llc_writes_standard_output_for_a_dash() {
    let dir = std::env::temp_dir().join(format!("llrm-wasm-dash-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/arith.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-o", "-"])
        .arg(ir)
        .current_dir(&dir)
        .output()
        .expect("llrm-wasm-llc");
    let stray = dir.join("-").exists();
    std::fs::remove_dir_all(&dir).ok();
    assert!(out.status.success() && !stray, "stray file: {stray}");
    assert!(String::from_utf8_lossy(&out.stdout).contains(".functype"));
}
