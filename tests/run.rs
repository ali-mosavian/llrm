//! tests/run: every program there compiles with its own compiler, runs in
//! DOSBox and prints what its `.out` says. One launch for all of them.

use std::path::Path;
use std::process::Command;

#[test]
fn test_every_program_under_tests_run_prints_its_out() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap();
    let done = Command::new("python3")
        .arg(root.join("tools/dosbatch/run_tests.py"))
        .env("LLRM_BIN", bin)
        .current_dir(root)
        .output()
        .expect("python3 runs tools/dosbatch/run_tests.py");
    let report = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success(), "{report}{}", String::from_utf8_lossy(&done.stderr));
    println!("{report}");
}
