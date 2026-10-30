//! No tracked tool ends a process by a name or a pattern. A command line is not
//! an identity: `pkill -f merge-batch` also matched every session whose prompt
//! mentioned it. A tool stops what it started, by a recorded pid or pgid.
//! The scanner is `tools/landing/check-kills.sh`; its own test covers each
//! form.

use std::path::Path;
use std::process::Command;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Files that still kill by pattern. Only shrinks: a file that no longer does
/// must come off this list.
const BASELINE: &[&str] = &[];

fn flagged(files: &[String]) -> Vec<String> {
    let out = Command::new("bash")
        .arg(Path::new(ROOT).join("tools/landing/check-kills.sh"))
        .args(files)
        .current_dir(ROOT)
        .output()
        .expect("bash runs");
    let mut found: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_owned).collect();
    found.sort();
    found
}

fn tracked_tools() -> Vec<String> {
    let out = Command::new("git").args(["ls-files", "tools"]).current_dir(ROOT).output().expect("git runs");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|f| !f.ends_with(".lock") && !f.ends_with(".png"))
        // the scanner and its test name every form they flag; they end no
        // process
        .filter(|f| *f != "tools/landing/check-kills.sh" && *f != "tools/landing/test-check-kills.sh")
        .map(str::to_owned)
        .collect()
}

#[test]
fn test_no_tool_kills_by_name_or_pattern() {
    let found: Vec<String> = flagged(&tracked_tools())
        .into_iter()
        .filter(|line| !BASELINE.iter().any(|file| line.starts_with(&format!("{file}:"))))
        .collect();
    assert!(
        found.is_empty(),
        "stop what you started by a recorded pid or pgid, never by name or pattern:\n{}",
        found.join("\n")
    );
}

#[test]
fn test_the_baseline_only_shrinks() {
    for file in BASELINE {
        assert!(
            !flagged(&[(*file).to_owned()]).is_empty(),
            "{file} no longer kills by pattern: remove it from BASELINE"
        );
    }
}
