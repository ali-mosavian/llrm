//! An inline the estimate keeps is put through the pipeline caller by caller.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// qb-runtime's `i8out.c` (source with an out-of-bounds store through a macro,
/// which the compiler still has to finish): `_copy_bytes` is kept by the
/// estimate at three sites in two callers. Spliced into both before the first
/// was optimised, the GlobalsAA the first one's pipeline asked for ran for
/// minutes (a 1 s compile at #1221 never finished at #1248): `mir
/// interprocedural` spliced all of a callee's callers and then put them through
/// the pipeline. 2.7 G instructions now; the bound is three times that.
#[test]
fn test_a_kept_inline_is_optimised_in_one_caller_before_the_next_is_spliced_into() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/c/i8out");
    let directory = tempfile::tempdir().expect("a directory");
    let mut child = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .args(["-m16", "-O2", "-I", root.to_str().unwrap(), "-S", "-o", directory.path().join("x.s").to_str().unwrap()])
        .arg(root.join("i8out.c"))
        .env("LLRM_DEBUG", "time")
        .env("LLRM_TIME_TOP", "1")
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("runs llrm-c");
    let started = Instant::now();
    let mut stderr = child.stderr.take().expect("piped");
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut stderr, &mut text).ok();
        text
    });
    let status = loop {
        if let Some(status) = child.try_wait().expect("waits") {
            break status;
        }
        if started.elapsed() > Duration::from_secs(120) {
            child.kill().ok();
            panic!("the compile of i8out.c did not finish in 120 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let report = reader.join().expect("reads");
    assert!(status.success(), "{report}");
    let total = report
        .lines()
        .find_map(|line| line.strip_prefix("[instr] total "))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|number| number.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("no instruction total in {report}"));
    assert!(total < 8_000.0, "{total} Minstr compiling i8out.c");
}
