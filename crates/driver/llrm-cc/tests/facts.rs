//! What clang states reaches the facts the passes read (`llrm-mir`'s
//! `facts_coverage` names these tests). Needs the pinned clang
//! (`LLRM_PIN_CLANG`); without it they skip, and fail where
//! `LLRM_REQUIRE_PIN_CLANG` is set (tools/debug-gate.env, and CI's setup action
//! when it requires clang), so a gate cannot pass them by skipping.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::{Path, PathBuf};
use std::process::Command;

const SOURCE: &str = "int add(int *restrict a, int *restrict b, int *restrict c) { return *a + *b + *c; }\n\
                      int halve(int x) { return x / 2; }\n";

/// The `define` lines of the first and the last stage dump of `SOURCE`
/// compiled at -O2, or `None` without the clang.
fn defines(name: &str) -> Option<(Vec<String>, Vec<String>)> {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() {
        assert!(
            std::env::var_os("LLRM_REQUIRE_PIN_CLANG").is_none(),
            "LLRM_REQUIRE_PIN_CLANG is set, and LLRM_PIN_CLANG is not"
        );
        eprintln!("SKIPPED: LLRM_PIN_CLANG names no pinned clang");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    // On disk: `--dump` refuses a tmpfs.
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.c"), SOURCE).unwrap();
    let dump = dir.join("dump");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-m32", "-O2", "-c", "--dump"])
        .arg(&dump)
        .arg("-o")
        .arg(dir.join("f.o"))
        .arg(dir.join("f.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let mut stages: Vec<PathBuf> = std::fs::read_dir(&dump)
        .unwrap()
        .flatten()
        .map(|one| one.path())
        .filter(|one| one.extension().is_some_and(|ext| ext == "ll"))
        .collect();
    stages.sort();
    let lines = |path: &PathBuf| {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter(|line| line.starts_with("define"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let found = (lines(stages.first().expect("a stage")), lines(stages.last().expect("a stage")));
    let _ = std::fs::remove_dir_all(dir);
    Some(found)
}

fn of<'a>(
    defines: &'a [String],
    function: &str,
) -> &'a str {
    defines
        .iter()
        .find(|line| line.contains(&format!("@{function}(")))
        .unwrap_or_else(|| panic!("{function}: {defines:?}"))
}

/// C99 6.7.3.1: the three restrict parameters of `add` reach the backend
/// `noalias`, as clang states them.
#[test]
fn test_restrict_parameters_are_noalias() {
    let Some((first, _)) = defines("facts-noalias") else { return };
    let add = of(&first, "add");
    assert_eq!(add.matches("noalias").count(), 3, "{add}");
}

/// CGCall.cpp `DetermineNoUndef`: a scalar parameter is `noundef`. None was
/// stated once, so SimplifyCFG could not tell `a || b` from one that reads an
/// undefined `b` (a branch more on m16).
#[test]
fn test_scalar_parameters_are_noundef() {
    let Some((first, _)) = defines("facts-noundef") else { return };
    assert!(of(&first, "halve").contains("i32 noundef"), "{}", of(&first, "halve"));
    assert_eq!(of(&first, "add").matches("noundef").count(), 3, "{}", of(&first, "add"));
}

/// A C function that calls nothing came out of the compile with no word that
/// nothing re-enters it: `norecurse` was inferred where no compiler ran.
#[test]
fn test_a_leaf_function_comes_out_of_the_compile_norecurse() {
    let Some((_, last)) = defines("facts-norecurse") else { return };
    assert!(last.iter().all(|line| line.contains("norecurse")), "{last:?}");
}
