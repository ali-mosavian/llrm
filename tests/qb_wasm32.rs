//! A QB program compiled by llrm-qb for wasm32, linked with the C runtime built
//! by clang and llrm-wasm-llc (runtime/wasm/build.sh) and run in node on the
//! headless device (runtime/wasm/headless.mjs). A machine without
//! clang for wasm32, wasm-ld, node, or a built llrm-wasm-llc fails here by
//! name; it does not skip.

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

/// What the harness printed and its exit status, standard error included.
fn run_raw(
    dir: &Path,
    name: &str,
    flags: &[&str],
) -> std::process::Output {
    // A program that spins without asking the host anything is never stopped by
    // the harness's own limit.
    Command::new("timeout")
        .args(["-k", "5", "60", "node"])
        .arg(root().join("runtime/wasm/headless.mjs"))
        .arg(dir.join(format!("{name}.wasm")))
        .args(["--limit-ms", "30000", "--out"])
        .arg(dir.join("frames"))
        .args(flags)
        .output()
        .expect("node runs")
}

/// The text screen the program left, and its exit code.
fn run(
    dir: &Path,
    name: &str,
    flags: &[&str],
) -> (String, Option<i32>) {
    let ran = run_raw(dir, name, flags);
    (String::from_utf8_lossy(&ran.stdout).into_owned(), ran.status.code())
}

/// `PRINT "hello"`: the first QB program to run on wasm32. Before `--wasm32`
/// the compiler had no route to the wasm backend, and its irreducible loops,
/// variadic runtime entries and `unreachable` after a runtime error did not
/// compile or ran off the end of a function.
#[test]
fn print_hello_runs_on_wasm32() {
    let dir = built("hello", "PRINT \"hello\"\nEND\n");
    let (text, code) = run(&dir, "hello", &[]);
    assert_eq!(text.trim_end(), "hello");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// DIM and COLOR call the runtime's block entries (B$DDIM, B$COLR), C varargs
/// on a host that take the block's words in the order BASIC pushes them. Built
/// as for DOS they were called in the Microsoft order and the first DIM died
/// with "Subscript out of range".
#[test]
fn block_entries_are_called_as_a_host_calls_them() {
    let dir = built("dims", "DIM a(1 TO 2)\na(2) = 5\nCOLOR 4\nPRINT a(2)\nEND\n");
    let (text, code) = run(&dir, "dims", &[]);
    assert_eq!(text.trim_end(), " 5");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// PLAY waits for the clock tick to take its notes off the queue. `music_tick`
/// was a weak no-op in tick.c, bound locally by the wasm object writer, so
/// play.c's never ran: the queue never drained and PLAY spun until the time
/// limit (Gorillas hung on its first throw).
#[test]
fn play_finishes_on_wasm32() {
    let dir = built("play", "PLAY \"T255 L64 CDEFG\"\nPRINT \"played\"\nEND\n");
    let (text, code) = run(&dir, "play", &[]);
    assert_eq!(text.trim_end(), "played");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// The harness wrote its frame colours and typed keys at `__heap_base - 1024`
/// and `- 64`: the top of the program's stack, under its outermost frames. A
/// frame taken while a program waited for a key corrupted its locals (Gorillas
/// lost the player names on the next turn and later trapped in MAKECITYSCAPE).
#[test]
fn a_frame_and_a_key_leave_the_programs_locals_alone() {
    // A SUB with 120 string temporaries: a frame of about a kilobyte under the
    // program's entry, the range written.
    let temps: String =
        (1..=120).map(|n| format!("  t{n}$ = LEFT$(keep$, {}) + a$ + RIGHT$(keep$, 2)\n", 1 + n % 5)).collect();
    let source = format!(
        "DECLARE SUB Echo (a$)\nEcho \"alpha\"\nEND\nSUB Echo (a$)\n  keep$ = a$ + \"!\"\n  INPUT \"key\"; k$\n{temps}  PRINT keep$; LEFT$(t120$, 3)\nEND SUB\n"
    );
    let dir = built("locals", &source);
    let (text, code) = run(&dir, "locals", &["--dump-at", "300=mid", "--keys", "x\\r@600"]);
    assert!(text.trim_end().ends_with("alpha!aal"), "{text:?}");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// ON ERROR cannot land on wasm32 (no unwinding): an error that reaches a
/// handler stops the program with a named failure, not by running on past it or
/// by a bare trap. Gorillas' SCREEN 9 and PALETTE probes raise nothing, so it
/// never lands.
#[test]
fn an_error_that_reaches_a_handler_stops_with_a_named_failure() {
    let dir =
        built("onerror", "ON ERROR GOTO Handler\nERROR 5\nPRINT \"went on\"\nEND\nHandler:\nPRINT \"handled\"\nEND\n");
    let ran = run_raw(&dir, "onerror", &[]);
    let (out, err) = (String::from_utf8_lossy(&ran.stdout), String::from_utf8_lossy(&ran.stderr));
    assert!(!out.contains("went on") && !out.contains("handled"), "{out:?}");
    assert!(err.contains("ON ERROR landed: longjmp is not supported on wasm32"), "{err}");
    assert_ne!(ran.status.code(), Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// The host sounds what PLAY plays, from the tones the module hands its host
/// (`llrm_host_tone`, a wrap of `dev_tone`). Without the wrap the host heard
/// nothing. The sequence is the x86-32 host build's own for this tune.
#[test]
fn play_hands_its_tones_to_the_host() {
    let dir = built("tune", "PLAY \"T120 L16 CDE\"\nPRINT \"played\"\nEND\n");
    let (text, code) = run(&dir, "tune", &[]);
    assert_eq!(text.trim_end(), "played");
    assert_eq!(code, Some(0));
    let log = std::fs::read_to_string(dir.join("frames/tones.json")).expect("the host heard no tone");
    let hertz: Vec<u32> =
        log.split("\"hertz\":").skip(1).map(|rest| rest.split(['}', ',']).next().unwrap().parse().unwrap()).collect();
    assert_eq!(hertz, [524, 0, 588, 0, 0]);
    std::fs::remove_dir_all(dir).ok();
}

/// `READ` into a `STRING * 5` called B$RDSD, which wrote a descriptor over the
/// fixed buffer (garbage on screen); a fixed string is read by B$RDFS now.
#[test]
fn read_fills_a_fixed_length_string_padded_and_cut() {
    let dir = built(
        "fixed",
        "DIM s AS STRING * 5\nDIM t AS STRING * 2\nREAD s, t\nPRINT \"[\"; s; \"][\"; t; \"]\"\nDATA ab, xyz\nEND\n",
    );
    let (text, code) = run(&dir, "fixed", &[]);
    assert_eq!(text.trim_end(), "[ab   ][xy]");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// EXP, LOG and `^` compile to calls of `exp`, `log` and `log2`/`pow`; the host
/// supplied only sin, cos, atan and lrint, so the module did not instantiate
/// ("function import requires a callable": data_after_checked, log_exp_digits,
/// range_power).
#[test]
fn exp_log_and_power_run_on_wasm32() {
    let dir = built(
        "mathfns",
        "DIM x AS DOUBLE\nx = 3\nPRINT EXP(x - 3); LOG(x - 2); 2# ^ x; x ^ 2#; SQR(x + 1)\nEND\n",
    );
    let (text, code) = run(&dir, "mathfns", &[]);
    assert_eq!(text.trim_end(), " 1  0  8  9  2");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}
