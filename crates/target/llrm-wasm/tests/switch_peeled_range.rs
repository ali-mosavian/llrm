use std::path::Path;
use std::process::Command;

/// `pick(marker, fall)` of `tests/ir/switch_peeled_range.ll` linked and run.
/// The spike coalescer's flow graph lacked the edge from the range-test block
/// to the jump-table block (a conditional branch with its else-successor not
/// next in layout), so the argument `fall` shared the phi's register and every
/// input returned it: switch_peeled_table (tests/run/c) failed at -Os.
fn picked(level: &str) -> Vec<String> {
    let dir = std::env::temp_dir().join(format!("llrm-wasm-switch-peeled-{level}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/switch_peeled_range.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args([level, "-filetype=obj", "-o"])
        .arg(dir.join("f.o"))
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked = Command::new("wasm-ld")
        .args(["--no-entry", "--export-all", "--allow-undefined"])
        .arg(dir.join("f.o"))
        .arg("-o")
        .arg(dir.join("f.wasm"))
        .output()
        .expect("wasm-ld");
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    let script = "const m = new WebAssembly.Module(require('fs').readFileSync(process.env.MODULE));
         const e = new WebAssembly.Instance(m, {}).exports;
         console.log([0, 191, 192, 193, 194, 195, 196, 207, 210, 218, 219, 220].map((x) => e.pick(x, 9)).join(' '));";
    let ran = Command::new("node").args(["-e", script]).env("MODULE", dir.join("f.wasm")).output().expect("node");
    std::fs::remove_dir_all(&dir).ok();
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
    String::from_utf8_lossy(&ran.stdout).trim().split(' ').map(str::to_owned).collect()
}

#[test]
fn a_switch_with_a_peeled_range_and_a_table_returns_each_case_s_value() {
    // 0 191 192 193 194 195 196 207 210 218 219 220
    let want = ["0", "0", "3", "0", "4", "9", "2", "9", "9", "9", "1", "0"];
    for level in ["-O2", "-Os"] {
        assert_eq!(picked(level), want, "{level}");
    }
}
