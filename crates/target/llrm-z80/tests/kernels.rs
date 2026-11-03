//! The corpus kernels compiled by this backend, run on `ucsim_z80` and checked
//! against the value worked out by hand (`tests/kernels/measure.py`). A kernel
//! whose compiled code returns another value, or that the backend refuses,
//! fails here with its row; the table also lists bytes and T-states next to the
//! best SDCC variant (`tests/kernels/sdcc.md`).

use std::process::Command;

fn have(tool: &str) -> bool {
    Command::new("which").arg(tool).output().is_ok_and(|o| o.status.success())
}

/// Rows that compile and run correctly now; a row is added when it does.
const WORKING: &[&str] = &[
    "zero", "add8", "add16", "max8", "max16", "sum_arr", "find", "fill", "popcount", "mul8", "fib", "sum4", "slen",
    "mcopy", "crc16", "mul16", "digits", "reverse", "bsearch8", "sum16", "schr", "gcd", "scmp",
];

#[test]
fn working_kernels_return_what_c_returns() {
    let missing: Vec<_> = ["sdasz80", "sdldz80", "ucsim_z80"].into_iter().filter(|tool| !have(tool)).collect();
    if !missing.is_empty() {
        let optional = std::env::var("Z80_ORACLE_OPTIONAL").is_ok_and(|flag| flag == "1");
        assert!(optional, "{missing:?} not found: the kernels cannot be run (Z80_ORACLE_OPTIONAL=1 skips them)");
        return;
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let z80c = std::env::var("CARGO_BIN_EXE_z80c").unwrap_or_default();
    let z80c = if z80c.is_empty() {
        // an example is built beside the test binary
        let exe = std::env::current_exe().expect("the test's path");
        exe.parent().and_then(|deps| deps.parent()).expect("target dir").join("examples/z80c").display().to_string()
    } else {
        z80c
    };
    assert!(std::path::Path::new(&z80c).exists(), "{z80c} is not built: cargo build -p llrm-z80 --example z80c");
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/kernels/measure.py"), "--z80c", &z80c])
        .args(["--ir-dir", &format!("{dir}/tests/kernels/ir"), "--baseline", &format!("{dir}/tests/kernels/sdcc.md")])
        .args(["--kernels", &WORKING.join(",")])
        .output()
        .expect("python3 runs");
    let table = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success(), "{table}\n{}", String::from_utf8_lossy(&done.stderr));
    for row in WORKING {
        let line = table.lines().find(|line| line.starts_with(&format!("| {row} |"))).unwrap_or("");
        assert!(
            !line.contains("refused") && !line.contains("WRONG") && !line.contains("link:") && !line.is_empty(),
            "{row}: {line}\n{table}"
        );
    }
}
