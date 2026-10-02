//! tests/differential: each program run through its own compiler and a reference one.

use std::path::Path;
use std::process::Command;

fn run(script: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap();
    let done = Command::new("python3")
        .arg(root.join("tools/dosbatch").join(script))
        .env("LLRM_BIN", bin)
        .current_dir(root)
        .output()
        .expect("python3 runs the runner");
    let report = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success(), "{report}{}", String::from_utf8_lossy(&done.stderr));
    println!("{report}");
}

#[test]
fn test_llrm_qb_prints_what_bc_does_under_each_programs_switches() {
    run("run_differential.py");
}
