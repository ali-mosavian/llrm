//! A file created or removed in a target's directory (a Python tool leaves `__pycache__` there) reran llrm-core's
//! build script, the instruction-selector generator, on the next build: the gate's `cargo test` rewrote the binaries'
//! generated inputs after `cargo build --bins`, and every tool then judged the binaries stale. The script now watches
//! the files it reads, not the directory that holds them.

use std::fs;
use std::path::Path;
use std::process::Command;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn build() -> String {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let done = Command::new(cargo).args(["build", "--release", "--bins", "-v"]).current_dir(ROOT).output().expect("cargo runs");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8_lossy(&done.stderr).into_owned()
}

#[test]
fn a_file_made_under_a_target_directory_does_not_rerun_the_selector_generator() {
    build();
    let scratch = Path::new(ROOT).join("crates/target/llrm-x86-m32/.rerun-probe");
    fs::write(&scratch, "").unwrap();
    let made = build();
    fs::remove_file(&scratch).unwrap();
    let removed = build();
    for log in [made, removed] {
        let dirty: Vec<&str> = log.lines().filter(|line| line.contains("Dirty llrm-core")).collect();
        assert!(dirty.is_empty(), "llrm-core's build script reran: {dirty:?}");
    }
}
