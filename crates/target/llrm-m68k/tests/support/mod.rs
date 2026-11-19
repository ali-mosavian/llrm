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

/// What `$GCC_M68K` (the unpacked m68k binutils) asks of a test.
#[derive(Debug, PartialEq)]
pub enum Oracle {
    Root(String),
    /// The oracle was skipped on purpose; the text is the line to print.
    Skipped(String),
}

/// `$GCC_M68K` against `$M68K_ISA_SKIP_ORACLE`: the root, the explicit skip, or the reason the test must fail.
pub fn oracle_with(
    root: Option<&str>,
    skip: Option<&str>,
) -> Result<Oracle, String> {
    if let Some(root) = root.filter(|root| !root.is_empty()) {
        let assembler = format!("{root}/usr/bin/m68k-linux-gnu-as");
        if !Path::new(&assembler).exists() {
            return Err(format!(
                "GCC_M68K={root} has no {assembler}: point it at the unpacked tree's root (the directory holding usr/)"
            ));
        }
        return Ok(Oracle::Root(root.to_owned()));
    }
    if skip == Some("1") {
        return Ok(Oracle::Skipped("ORACLE SKIPPED: GCC_M68K is not set and M68K_ISA_SKIP_ORACLE=1".to_owned()));
    }
    Err("GCC_M68K is not set: the unpacked m68k binutils tree (gcc-m68k); M68K_ISA_SKIP_ORACLE=1 skips this oracle".to_owned())
}

/// The binutils root for an oracle test. Unset, the test fails naming what to set; `M68K_ISA_SKIP_ORACLE=1`
/// skips it with a visible `ORACLE SKIPPED` line and gives `None`.
pub fn oracle_root() -> Option<String> {
    let root = std::env::var("GCC_M68K").ok();
    let skip = std::env::var("M68K_ISA_SKIP_ORACLE").ok();
    match oracle_with(root.as_deref(), skip.as_deref()) {
        Ok(Oracle::Root(root)) => Some(root),
        Ok(Oracle::Skipped(line)) => {
            eprintln!("{line}");
            None
        }
        Err(why) => panic!("{why}"),
    }
}

/// A path no other call in this process has been given. Naming it by the IR's
/// length gave two tests whose programs had the same length one file, and each
/// ran the other's. The file at it is deleted when the value is dropped, a
/// failed test included.
pub fn unique_path(kind: &str) -> Scratch {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    Scratch(std::env::temp_dir().join(format!(
        "llrm-m68k-{kind}-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    )))
}

/// A scratch path whose file goes with it.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn to_path_buf(&self) -> PathBuf {
        self.0.clone()
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<std::ffi::OsStr> for Scratch {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.0.as_os_str()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.0.is_dir() {
            std::fs::remove_dir_all(&self.0).ok();
        } else {
            std::fs::remove_file(&self.0).ok();
        }
    }
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
