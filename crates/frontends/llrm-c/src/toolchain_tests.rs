//! The toolchain's shared cache (toolchain/cache.sh): a tree under
//! ~/.cache/llrm is whole or absent, whatever builds ran at once, and wccq says
//! what it was made from.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("LLRM_ROOT")).to_path_buf()
}

/// `body` under `sh` with `cached` defined, in `directory`.
fn sh(directory: &Path, body: &str) -> std::process::Child {
    let script = format!(". {}/toolchain/cache.sh\n{body}", root().display());
    Command::new("sh").arg("-c").arg(script).current_dir(directory).spawn().expect("sh starts")
}

/// Four builds of one tree at once: one produces, and every one that returns
/// finds the tree complete. A shared tree made in place had each build see
/// another's half-made files, or a tree for another commit, and relink wccq
/// against it.
#[test]
fn test_concurrent_builds_of_one_tree_make_it_once_and_whole() {
    let directory = tempfile::tempdir().unwrap();
    let body = r#"
        producer() { sleep 1; echo half > "$1/a"; sleep 1; echo whole > "$1/b"; echo made >> made; }
        cached tree producer
        [ -f tree/b ] && [ -f tree/.complete ]
    "#;
    let builds: Vec<_> = (0..4).map(|_| sh(directory.path(), body)).collect();
    for mut build in builds {
        assert!(build.wait().unwrap().success(), "a build returned before the tree was whole");
    }
    assert_eq!(std::fs::read_to_string(directory.path().join("made")).unwrap().lines().count(), 1);
    assert!(!directory.path().join("tree.lock").exists());
}

/// Of eight callers claiming one lock at once, exactly one wins, round after round. The lock
/// was `mkdir`, which on a host with uutils coreutils 0.2.2 let two both succeed: a tree
/// was then produced twice, one run in six.
#[test]
fn test_of_many_concurrent_claims_one_wins() {
    let directory = tempfile::tempdir().unwrap();
    for round in 0..30 {
        let body = format!("claimed lock{round} && echo won >> won{round}; true");
        let claims: Vec<_> = (0..8).map(|_| sh(directory.path(), &body)).collect();
        for mut claim in claims {
            assert!(claim.wait().unwrap().success());
        }
        let won = std::fs::read_to_string(directory.path().join(format!("won{round}"))).unwrap();
        assert_eq!(won.lines().count(), 1, "round {round}: {won}");
    }
}

/// A producer that fails leaves nothing a later build would take for a tree.
#[test]
fn test_a_failed_build_leaves_no_tree() {
    let directory = tempfile::tempdir().unwrap();
    let body = r#"
        broken() { echo half > "$1/a"; return 1; }
        ! cached tree broken
        [ ! -e tree ] && [ ! -e tree.lock ]
        fixed() { echo whole > "$1/b"; }
        cached tree fixed && [ -f tree/b ]
    "#;
    assert!(sh(directory.path(), body).wait().unwrap().success());
}

/// A build killed mid-way leaves its lock; the next takes it over.
#[test]
fn test_a_dead_builds_lock_is_taken_over() {
    let directory = tempfile::tempdir().unwrap();
    let dead = Command::new("true").spawn().and_then(|mut one| one.wait().map(|_| one.id())).unwrap();
    std::fs::write(directory.path().join("tree.lock"), dead.to_string()).unwrap();
    let body = r#"
        producer() { echo whole > "$1/b"; }
        cached tree producer && [ -f tree/b ]
    "#;
    assert!(sh(directory.path(), body).wait().unwrap().success());
}

/// wccq is stamped with the Open Watcom commit and a hash of what is linked
/// into it; a wccq left from other patches or another pin does not match the
/// checkout.
#[test]
fn test_wccq_says_what_it_was_made_from() {
    let Some(wccq) = option_env!("LLRM_WCCQ") else { return };
    let stamp = std::fs::read_to_string(Path::new(wccq).with_file_name("stamp")).expect("wccq has a stamp");
    let hash = Command::new("sh").arg(root().join("toolchain/owshim/hash.sh")).output().unwrap();
    assert_eq!(stamp.trim(), String::from_utf8(hash.stdout).unwrap().trim());
}
