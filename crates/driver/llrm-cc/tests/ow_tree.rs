//! The Open Watcom tree `{ow}` names, with no environment variable set.

use std::path::{Path, PathBuf};
use std::process::Command;

fn commit() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../toolchain/owshim/ow-commit"))
        .unwrap()
        .trim()
        .to_owned()
}

/// `llrm-cc -c` for `x86_16-wat` with `home` as HOME and no `LLRM_OW`; its
/// stderr.
fn stderr_with(home: &Path) -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("ow-tree-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), "int f(int a) { return a + 1; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env_remove("LLRM_OW")
        .env_remove("XDG_CACHE_HOME")
        .env("HOME", home)
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .args(["-c", "-o"])
        .arg(dir.join("d.o"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    String::from_utf8_lossy(&run.stderr).into_owned()
}

/// A tree the user must fetch was a variable they had to know: 94 m16 cells of
/// an A/B read "refused" with "needs the environment variable LLRM_OW". Now the
/// first line names the command that makes the tree.
#[test]
fn a_missing_open_watcom_tree_names_the_command_that_makes_it() {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ow-home-missing-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let first = stderr_with(&home).lines().next().unwrap_or_default().to_owned();
    assert!(first.contains("tools/ow-fetch.sh") && first.contains(&format!("ow-src-{}", commit())), "{first}");
}

#[test]
fn the_tree_under_cache_is_found_with_no_environment_variable() {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ow-home-present-{}", std::process::id()));
    let tree = home.join(format!(".cache/llrm/ow-src-{}", commit()));
    std::fs::create_dir_all(&tree).unwrap();
    std::fs::write(tree.join(".complete"), "").unwrap();
    let text = stderr_with(&home);
    assert!(!text.contains("ow-fetch.sh") && !text.contains("LLRM_OW"), "{text}");
}

/// toolchain/cache.sh fills the directory in place and writes `.complete` last.
/// A directory without it (an interrupted or running fetch) resolved `{ow}` and
/// the compile then failed on a missing library instead of naming ow-fetch.sh.
#[test]
fn a_tree_without_its_complete_marker_is_missing() {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ow-home-partial-{}", std::process::id()));
    std::fs::create_dir_all(home.join(format!(".cache/llrm/ow-src-{}", commit()))).unwrap();
    let first = stderr_with(&home).lines().next().unwrap_or_default().to_owned();
    assert!(first.contains("tools/ow-fetch.sh"), "{first}");
}
