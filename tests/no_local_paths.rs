//! Docs name no path on a developer's machine, and nothing tracked, or
//! committed on this branch, names the tools that wrote it. A `/tmp/` or
//! `~/work` path is a file nobody else has; a link or a relative path is
//! one anybody can follow.

use std::fs;
use std::path::Path;
use std::process::Command;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Paths of one machine. `~/.cache/...` is not here: it is where a tool
/// keeps its files, which a doc may state.
const LOCAL: &[&str] = &[
    "/home/",
    "/Users/",
    "/tmp/",
    "/var/folders/",
    "/private/var/",
    "~/scratch",
    "~/work",
    concat!("~/.", "cla", "ude"),
];

/// Split, so this file does not name them.
const NAMES: [[&str; 2]; 2] = [["cla", "ude"], ["anthr", "opic"]];

fn local_path(text: &str) -> Option<&'static str> {
    LOCAL.iter().copied().find(|path| text.contains(path))
}

fn assistant(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    NAMES.iter().map(|parts| parts.concat()).find(|name| lower.contains(name))
}

fn git(
    dir: &Path,
    args: &[&str],
) -> String {
    let out = Command::new("git").args(args).current_dir(dir).output().expect("git runs");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `file:line` of each line of a tracked markdown file that names a path of
/// one machine.
fn local_paths(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for file in git(dir, &["ls-files", "-z", "*.md"]).split('\0').filter(|name| !name.is_empty()) {
        let Ok(text) = fs::read_to_string(dir.join(file)) else { continue };
        for (at, line) in text.lines().enumerate() {
            if let Some(path) = local_path(line) {
                found.push(format!("{file}:{} {path}", at + 1));
            }
        }
    }
    found
}

/// The tracked files whose text names the assistant.
fn assistant_files(dir: &Path) -> Vec<String> {
    git(dir, &["ls-files", "-z"])
        .split('\0')
        .filter(|name| !name.is_empty())
        .filter(|file| fs::read_to_string(dir.join(file)).is_ok_and(|text| assistant(&text).is_some()))
        .map(str::to_owned)
        .collect()
}

/// The commits of `range` whose message names the assistant, by subject.
fn assistant_commits(
    dir: &Path,
    range: &str,
) -> Vec<String> {
    git(dir, &["log", "--format=%H%x1f%s%x1f%B%x1e", range])
        .split('\x1e')
        .filter_map(|entry| {
            let mut fields = entry.trim().splitn(3, '\x1f');
            let (hash, subject, body) = (fields.next()?, fields.next()?, fields.next()?);
            assistant(body).map(|name| format!("{} {subject} ({name})", &hash[..hash.len().min(9)]))
        })
        .collect()
}

#[test]
fn test_no_tracked_markdown_names_a_path_of_one_machine() {
    let found = local_paths(Path::new(ROOT));
    assert!(found.is_empty(), "link the source, or describe it without the path:\n{}", found.join("\n"));
}

#[test]
fn test_nothing_tracked_names_the_assistant() {
    let found = assistant_files(Path::new(ROOT));
    assert!(found.is_empty(), "tracked files name the assistant: {found:?}");
}

/// A branch is its commits since `origin/main`; main's own history is not
/// rewritten.
#[test]
fn test_no_commit_of_this_branch_names_the_assistant() {
    let dir = Path::new(ROOT);
    let known = Command::new("git").args(["rev-parse", "--verify", "-q", "origin/main"]).current_dir(dir).output();
    if !known.is_ok_and(|out| out.status.success()) {
        eprintln!("no origin/main here: the branch's commit messages are not checked");
        return;
    }
    let found = assistant_commits(dir, "origin/main..HEAD");
    assert!(found.is_empty(), "commit messages name the assistant: {found:?}");
}

fn repo(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    git(dir.path(), &["init", "-q"]);
    for (name, text) in files {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }
    git(dir.path(), &["add", "-A"]);
    dir
}

fn commit(
    dir: &Path,
    message: &str,
) {
    git(
        dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            message,
        ],
    );
}

/// 379 references to `/tmp/`, `/Users/` and `~/scratch` were in tracked
/// markdown.
#[test]
fn test_a_local_path_in_tracked_markdown_is_found() {
    let dir = repo(&[
        ("a.md", "Evidence: `/tmp/qbopt-x`.\nfine: https://github.com/llvm/llvm-project\n"),
        ("docs/b.md", "in `~/work/other/gcc`\nand ~/.cache/llrm, crates/opt/x.rs\n"),
        ("c.txt", "/tmp/not-markdown\n"),
    ]);
    assert_eq!(local_paths(dir.path()), ["a.md:1 /tmp/", "docs/b.md:1 ~/work"]);
}

#[test]
fn test_a_clean_tree_has_no_local_path() {
    let dir = repo(&[("a.md", "see [gcc](https://github.com/gcc-mirror/gcc) and ../crates/opt/x.rs\n")]);
    assert!(local_paths(dir.path()).is_empty());
}

/// The assistant's name sat in tracked docs and fixtures as a worktree path.
#[test]
fn test_the_assistant_named_in_a_tracked_file_is_found() {
    let name = NAMES[0].concat();
    let dir = repo(&[
        ("docs/a.md", &format!("kept in ~/.{name}/plans\n")),
        ("tests/x.cgs", &format!("DBSrcFile \"/w/.{}/x.c\"\n", name.to_uppercase())),
        ("clean.md", "nothing\n"),
    ]);
    assert_eq!(assistant_files(dir.path()), ["docs/a.md", "tests/x.cgs"]);
}

/// A trailer in a commit message named the assistant; history is not
/// rewritten, so only the branch's own range is read.
#[test]
fn test_the_assistant_named_in_a_commit_message_is_found_in_its_range() {
    let name = NAMES[1].concat();
    let dir = repo(&[("a.md", "x\n")]);
    commit(dir.path(), "feat: first");
    git(dir.path(), &["branch", "base"]);
    commit(dir.path(), &format!("docs: second\n\nGenerated with a tool from {}.", name.to_uppercase()));
    commit(dir.path(), &format!("docs: third\n\nCo-Authored-By: {name} <n@x>"));
    commit(dir.path(), "docs: fourth");
    let found = assistant_commits(dir.path(), "base..HEAD");
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].contains("docs: third") && found[1].contains("docs: second"), "{found:?}");
    assert!(assistant_commits(dir.path(), "HEAD..HEAD").is_empty());
}
