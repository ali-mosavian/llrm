//! The Python unit tests beside the sweep scripts (`tests/programs/test_*.py`) run under `cargo test`:
//! nothing ran them before, so the sweep's row-count guard could regress unseen.

use std::process::Command;

#[test]
fn the_programs_python_unit_tests_pass() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let done = match Command::new("python3").args(["-m", "unittest", "discover", "-s", &format!("{dir}/tests/programs")]).output() {
        Ok(done) => done,
        Err(_) => return, // no python3 on this machine
    };
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}
