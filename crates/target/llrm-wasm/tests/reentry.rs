//! A landing pad that branches back into the code it guards
//! (`tests/eh/reentry.ll`): the pin stops in `placeTryMarker` ("Explicit
//! branch to an EH pad!"), so there is no golden; the module is linked and run.

use std::path::Path;
use std::process::Command;

fn need(tool: &str) {
    let found = Command::new("which").arg(tool).output().is_ok_and(|out| out.status.success());
    assert!(found, "reentry needs `{tool}` on the PATH");
}

/// `tests/eh/NAME.ll` as a linked module and what `script` prints of it.
fn run(
    name: &str,
    script: &str,
) -> String {
    need("wasm-ld");
    need("node");
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let number = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("llrm-wasm-{name}-{}-{number}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/eh/{name}.ll"));
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-filetype=obj", "-exception-model=wasm", "-mattr=+exception-handling", "-o"])
        .arg(dir.join("f.o"))
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked = Command::new("wasm-ld")
        .args(["--no-entry", "--export-all", "--allow-undefined", "--import-memory"])
        .arg(dir.join("f.o"))
        .arg("-o")
        .arg(dir.join("f.wasm"))
        .output()
        .expect("wasm-ld");
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    let ran = Command::new("node").args(["-e", script]).env("MODULE", dir.join("f.wasm")).output().expect("node");
    std::fs::remove_dir_all(&dir).ok();
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
    String::from_utf8_lossy(&ran.stdout).trim().to_owned()
}

/// `g` throws on the call numbered `fail`; `slot` holds `at`.
fn script(
    fail: u32,
    at: u32,
) -> String {
    format!(
        "const memory = new WebAssembly.Memory({{ initial: 2 }});
         const calls = [];
         const env = {{ memory, g: (n) => {{ calls.push(n); if (calls.length === {fail}) throw new Error('x'); }} }};
         const m = new WebAssembly.Module(require('fs').readFileSync(process.env.MODULE));
         const instance = new WebAssembly.Instance(m, {{ env }});
         new DataView(memory.buffer).setUint32(instance.exports.slot, {at}, true);
         const result = instance.exports.f(0);
         console.log(JSON.stringify({{ result, calls }}));"
    )
}

/// The first call fails and the pad branches to the block holding the second
/// invoke. It was the pin's assertion; now it runs.
#[test]
fn a_pad_that_branches_back_into_its_body_resumes_there() {
    assert_eq!(run("reentry", &script(1, 1)), r#"{"result":7,"calls":[1,2]}"#);
}

/// The second call fails and the pad branches to the block of that same call:
/// the pad is entered again from inside the body it re-enters.
#[test]
fn a_pad_entered_again_from_the_body_it_re_enters_runs_that_body_again() {
    assert_eq!(run("reentry", &script(2, 1)), r#"{"result":7,"calls":[1,2,2]}"#);
}

/// The switch's default successor, the block after both calls.
#[test]
fn the_default_successor_of_the_same_pad_is_entered_too() {
    assert_eq!(run("reentry", &script(1, 0)), r#"{"result":7,"calls":[1]}"#);
}
