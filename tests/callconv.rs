//! The calling-convention matrices run under dosrun: llrm's callers with
//! BCC's or BC's callees and the other way round, every value that arrives
//! and comes back checked by a harness the reference compiler built. Opt-in,
//! as they need the DOS toolchains; about ten seconds for all of it:
//!
//!   cargo test --release --test callconv -- --ignored
//!
//! Each program prints only its failures. `runs.txt` beside each matrix
//! lists the ones still expected, `<program> <case>`, or `<program> crashed`.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// What each program in `work` reported: its failing cases, or that it
/// printed nothing, which a crash or a hang leaves.
fn failures(work: &Path, programs: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for program in programs {
        let Ok(text) = std::fs::read(work.join(format!("{program}.OUT"))) else {
            out.insert(format!("{program} crashed"));
            continue;
        };
        let text = String::from_utf8_lossy(&text).replace('\r', "");
        if !text.lines().any(|line| line.ends_with("failures") || line.starts_with("failures")) {
            out.insert(format!("{program} crashed"));
        }
        for line in text.lines().filter_map(|line| line.strip_prefix("FAIL ")) {
            let case = line.split(" got").next().unwrap_or(line).split(" lost").next().unwrap_or(line);
            out.insert(format!("{program} {case}"));
        }
    }
    out
}

fn expected(path: &str) -> BTreeSet<String> {
    std::fs::read_to_string(root().join(path)).unwrap().lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).map(str::to_owned).collect()
}

fn run(script: &str, arguments: &[&str]) {
    let status = Command::new(root().join(script)).args(arguments).env("LLRM_BIN", Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap()).status().unwrap();
    assert!(status.success(), "{script} {arguments:?}");
}

fn assert_runs(got: BTreeSet<String>, want: BTreeSet<String>, file: &str) {
    let new: Vec<&String> = got.difference(&want).collect();
    let fixed: Vec<&String> = want.difference(&got).collect();
    assert!(new.is_empty() && fixed.is_empty(), "{file}: fail, not expected: {new:#?}\nexpected, now pass: {fixed:#?}");
}

#[test]
#[ignore = "needs BCC 3.1's libraries and dosrun"]
fn test_c_conventions_run_against_bcc() {
    let work = tempfile::tempdir().unwrap();
    run("tools/callconv/c.sh", &[work.path().to_str().unwrap()]);
    let programs = ["CF", "CN", "PF", "PN"].iter().flat_map(|one| ["BB", "LB", "BL", "LL"].map(|pair| format!("{one}{pair}")));
    assert_runs(failures(work.path(), programs), expected("tests/fixtures/callconv/c/runs.txt"), "c/runs.txt");
}

#[test]
#[ignore = "needs VBDOS, PDS 7.1 and QB 4.5 and dosrun"]
fn test_basic_conventions_run_against_bc() {
    let mut got = BTreeSet::new();
    for dialect in ["vbdos", "pds71", "qb45"] {
        let work = tempfile::tempdir().unwrap();
        run("tools/callconv/bas.sh", &[dialect, work.path().join("w").to_str().unwrap()]);
        let reported = failures(&work.path().join("w"), ["BB", "LB", "BL", "LL"].map(str::to_owned));
        got.extend(reported.into_iter().map(|one| format!("{dialect} {one}")));
    }
    assert_runs(got, expected("tests/fixtures/callconv/bas/runs.txt"), "bas/runs.txt");
}
