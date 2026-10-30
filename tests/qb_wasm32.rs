//! A QB program compiled by llrm-qb for wasm32, linked with the C runtime built by clang and llrm-wasm-llc
//! (runtime/wasm/build.sh) and run in node on the headless device (runtime/wasm/headless.mjs). A machine without
//! clang for wasm32, wasm-ld, node, or a built llrm-wasm-llc fails here by name; it does not skip.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// `cargo build -p llrm-wasm --bin llrm-wasm-llc` puts it next to llrm-qb.
fn llc() -> PathBuf {
    let found = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).with_file_name("llrm-wasm-llc");
    assert!(found.exists(), "qb_wasm32 needs {}: cargo build -p llrm-wasm --bin llrm-wasm-llc", found.display());
    found
}

fn need(tool: &str) {
    let found = Command::new("which").arg(tool).output().is_ok_and(|out| out.status.success());
    assert!(found, "qb_wasm32 needs `{tool}` on the PATH");
}

/// `source` built into a module; the directory it is in.
fn built(
    name: &str,
    source: &str,
) -> PathBuf {
    for tool in ["clang", "wasm-ld", "node"] {
        need(tool);
    }
    let dir = std::env::temp_dir().join(format!("{name}_wasm32_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bas = dir.join(format!("{name}.bas"));
    std::fs::write(&bas, source).unwrap();
    let made = Command::new(root().join("runtime/wasm/build.sh"))
        .arg(&bas)
        .arg(dir.join(format!("{name}.wasm")))
        .arg(dir.join("obj"))
        .env("LLRM_QB", env!("CARGO_BIN_EXE_llrm-qb"))
        .env("LLRM_WASM_LLC", llc())
        .output()
        .expect("runtime/wasm/build.sh runs");
    assert!(made.status.success(), "build.sh: {}", String::from_utf8_lossy(&made.stderr));
    dir
}

/// The text screen the program left, and its exit code.
fn run(
    dir: &Path,
    name: &str,
    flags: &[&str],
) -> (String, Option<i32>) {
    let ran = Command::new("node")
        .arg(root().join("runtime/wasm/headless.mjs"))
        .arg(dir.join(format!("{name}.wasm")))
        .args(["--limit-ms", "30000", "--out"])
        .arg(dir.join("frames"))
        .args(flags)
        .output()
        .expect("node runs");
    (String::from_utf8_lossy(&ran.stdout).into_owned(), ran.status.code())
}

/// `PRINT "hello"`: the first QB program to run on wasm32. Before `--wasm32` the compiler had no route to the wasm
/// backend, and its irreducible loops, variadic runtime entries and `unreachable` after a runtime error did not
/// compile or ran off the end of a function.
#[test]
fn print_hello_runs_on_wasm32() {
    let dir = built("hello", "PRINT \"hello\"\nEND\n");
    let (text, code) = run(&dir, "hello", &[]);
    assert_eq!(text.trim_end(), "hello");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// DIM and COLOR call the runtime's block entries (B$DDIM, B$COLR), C varargs on a host that take the block's words in
/// the order BASIC pushes them. Built as for DOS they were called in the Microsoft order and the first DIM died with
/// "Subscript out of range".
#[test]
fn block_entries_are_called_as_a_host_calls_them() {
    let dir = built("dims", "DIM a(1 TO 2)\na(2) = 5\nCOLOR 4\nPRINT a(2)\nEND\n");
    let (text, code) = run(&dir, "dims", &[]);
    assert_eq!(text.trim_end(), " 5");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// PLAY waits for the clock tick to take its notes off the queue. `music_tick` was a weak no-op in tick.c, bound
/// locally by the wasm object writer, so play.c's never ran: the queue never drained and PLAY spun until the time
/// limit (Gorillas hung on its first throw).
#[test]
fn play_finishes_on_wasm32() {
    let dir = built("play", "PLAY \"T255 L64 CDEFG\"\nPRINT \"played\"\nEND\n");
    let (text, code) = run(&dir, "play", &[]);
    assert_eq!(text.trim_end(), "played");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}
