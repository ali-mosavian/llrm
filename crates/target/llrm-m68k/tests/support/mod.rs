//! What the tests that run a program under `qemu-m68k` share.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Long enough for a loaded machine: a run that ends in a timeout is a failure
/// of the test, never a result.
pub const SECONDS: u32 = 60;

/// The `qemu-m68k` to run programs with. Without one the test fails, since a
/// run it skipped passed with nothing checked; `M68K_QEMU_OPTIONAL=1` marks the
/// run optional and gives `None`.
pub fn qemu() -> Option<PathBuf> {
    let found = ["/usr/bin/qemu-m68k", "/usr/local/bin/qemu-m68k"].iter().map(PathBuf::from).find(|path| path.exists());
    if found.is_none() {
        let optional = std::env::var("M68K_QEMU_OPTIONAL").is_ok_and(|flag| flag == "1");
        assert!(optional, "qemu-m68k is not installed (M68K_QEMU_OPTIONAL=1 skips the runs)");
    }
    found
}

/// A path no other call in this process has been given. Naming it by the IR's
/// length gave two tests whose programs had the same length one file, and each
/// ran the other's.
pub fn unique_path(kind: &str) -> PathBuf {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "llrm-m68k-{kind}-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Exit status of `qemu args... exe`, `None` if a signal ended it. `timeout`'s
/// own status, 124, is a panic: it read as the program's answer ("got 124") on
/// a loaded machine.
pub fn run(
    qemu: &Path,
    args: &[&str],
    exe: &Path,
) -> Option<i32> {
    run_for(SECONDS, qemu, args, exe)
}

/// [`run`] with `seconds` before the run is cut off.
pub fn run_for(
    seconds: u32,
    qemu: &Path,
    args: &[&str],
    exe: &Path,
) -> Option<i32> {
    let code = Command::new("timeout").arg(seconds.to_string()).arg(qemu).args(args).arg(exe).status().unwrap().code();
    assert_ne!(code, Some(124), "{} did not finish in {seconds} s", exe.display());
    code
}
