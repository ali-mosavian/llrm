//! Every link in a tracked markdown file resolves, and every file under
//! `docs/` is reachable from the root readme, so a doc cannot be orphaned
//! by a move or a deletion.

use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const START: &str = "readme.md";

fn tracked_markdown(dir: &Path) -> Vec<String> {
    let out = Command::new("git").args(["ls-files", "-z", "*.md"]).current_dir(dir).output().expect("git runs");
    assert!(out.status.success(), "git ls-files failed");
    String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|name| !name.is_empty() && dir.join(name).is_file())
        .map(str::to_owned)
        .collect()
}

/// The text without fenced blocks and inline code, where `](x)` is not a link.
fn prose(text: &str) -> String {
    let mut out = String::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        for (at, part) in line.split('`').enumerate() {
            if at % 2 == 0 {
                out.push_str(part);
            }
        }
        out.push('\n');
    }
    out
}

/// The relative file targets of the inline links of `text`, without fragments.
fn targets(text: &str) -> Vec<String> {
    let text = prose(text);
    let mut found = Vec::new();
    let mut rest = text.as_str();
    while let Some(at) = rest.find("](") {
        rest = &rest[at + 2..];
        let end = rest.find(')').unwrap_or(rest.len());
        let target = rest[..end].split('#').next().unwrap_or("");
        let external = target.split(':').next().is_some_and(|scheme| target.contains(':') && !scheme.contains('/'));
        if !target.is_empty() && !external && !target.starts_with('/') && !target.contains(' ') {
            found.push(target.to_owned());
        }
    }
    found
}

/// `base/target` with `.` and `..` folded; `None` if it leaves the root.
fn join(
    base: &str,
    target: &str,
) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in Path::new(base).parent().unwrap_or(Path::new("")).join(target).components() {
        match part {
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::Normal(name) => out.push(name),
            _ => {}
        }
    }
    Some(out)
}

/// `file:line target` of each link whose target is neither a file nor a
/// directory.
fn broken(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for file in tracked_markdown(dir) {
        let text = fs::read_to_string(dir.join(&file)).unwrap_or_default();
        for target in targets(&text) {
            match join(&file, &target) {
                Some(path) if dir.join(&path).exists() => {}
                _ => found.push(format!("{file} {target}")),
            }
        }
    }
    found
}

/// `file path` of each `docs/…md` named in the text, as a link or not, that
/// is not a file. A URL naming a doc of another commit is not a path here.
fn named_missing(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for file in tracked_markdown(dir) {
        let text = fs::read_to_string(dir.join(&file)).unwrap_or_default();
        for line in text.lines() {
            for (at, _) in line.match_indices("docs/") {
                if line[..at].ends_with(|c: char| c.is_alphanumeric() || "/._-".contains(c)) {
                    continue;
                }
                let tail = &line[at..];
                let end = tail.find(|c: char| !(c.is_alphanumeric() || "/._-".contains(c))).unwrap_or(tail.len());
                let path = tail[..end].trim_end_matches('.');
                if path.ends_with(".md") && !dir.join(path).is_file() {
                    found.push(format!("{file} {path}"));
                }
            }
        }
    }
    found
}

/// What a link to `path` reaches: the file, or a directory's readme.
fn reached(
    dir: &Path,
    path: &Path,
) -> Option<String> {
    let full = dir.join(path);
    let name = if full.is_dir() { path.join("readme.md") } else { path.to_path_buf() };
    (dir.join(&name).is_file() && name.extension().is_some_and(|ext| ext == "md"))
        .then(|| name.to_string_lossy().into_owned())
}

/// The files under `docs/` no chain of links from the root readme reaches.
fn orphans(dir: &Path) -> Vec<String> {
    let all = tracked_markdown(dir);
    let mut seen = BTreeSet::from([START.to_owned()]);
    let mut queue = VecDeque::from([START.to_owned()]);
    while let Some(file) = queue.pop_front() {
        let text = fs::read_to_string(dir.join(&file)).unwrap_or_default();
        for target in targets(&text) {
            if let Some(next) = join(&file, &target).and_then(|path| reached(dir, &path)) {
                if seen.insert(next.clone()) {
                    queue.push_back(next);
                }
            }
        }
    }
    all.into_iter().filter(|file| file.starts_with("docs/") && !seen.contains(file)).collect()
}

/// The catalogs of the optimizer and the backend: every source file of a
/// directory is named in its doc.
const CATALOGS: [(&str, &str); 2] = [
    ("crates/opt/llrm-transforms/src", "docs/optimizations/passes.md"),
    ("crates/backend/llrm-core/src/backend", "docs/optimizations/backend.md"),
];

/// `source file` of each non-test source file of `src` its `catalog` never
/// names.
fn uncatalogued(
    dir: &Path,
    src: &str,
    catalog: &str,
) -> Vec<String> {
    let text = fs::read_to_string(dir.join(catalog)).unwrap_or_default();
    let mut found: Vec<String> = fs::read_dir(dir.join(src))
        .map(|entries| entries.filter_map(Result::ok).map(|entry| entry.path()).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .filter_map(|path| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
        .filter(|stem| !stem.ends_with("_tests") && !["lib", "mod", "testing"].contains(&stem.as_str()))
        .filter(|stem| !text.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|word| word == stem))
        .map(|stem| format!("{src}/{stem}.rs ({catalog})"))
        .collect();
    found.sort();
    found
}

#[test]
fn test_every_pass_analysis_and_backend_file_is_in_its_catalog() {
    let dir = Path::new(ROOT);
    let found: Vec<String> = CATALOGS.iter().flat_map(|(src, doc)| uncatalogued(dir, src, doc)).collect();
    assert!(found.is_empty(), "add these to their catalog:\n{}", found.join("\n"));
}

#[test]
fn test_every_markdown_link_resolves() {
    let found = broken(Path::new(ROOT));
    assert!(found.is_empty(), "links to nothing (fix or remove):\n{}", found.join("\n"));
}

#[test]
fn test_every_doc_path_named_in_markdown_exists() {
    let found = named_missing(Path::new(ROOT));
    assert!(found.is_empty(), "docs named that are not there (update or remove):\n{}", found.join("\n"));
}

#[test]
fn test_every_doc_is_reachable_from_the_root_readme() {
    let found = orphans(Path::new(ROOT));
    assert!(found.is_empty(), "link these from an index, or delete them:\n{}", found.join("\n"));
}

fn repo(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let git = |args: &[&str]| {
        assert!(Command::new("git").args(args).current_dir(dir.path()).status().expect("git runs").success());
    };
    git(&["init", "-q"]);
    for (name, text) in files {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }
    git(&["add", "-A"]);
    dir
}

/// 66 docs were deleted and 30 moved with nothing checking the links to
/// them: a link to a deleted file read as a live reference.
#[test]
fn test_a_link_to_a_moved_or_deleted_file_is_found() {
    let dir = repo(&[
        ("readme.md", "[a](docs/a.md) [gone](docs/gone.md) [web](https://x.y/z.md) [dir](docs)\n"),
        ("docs/a.md", "[up](../readme.md#top) [no](../old/b.md)\n```\n[fenced](nothing.md)\n```\n`[c](nothing.md)`\n"),
        ("docs/readme.md", "x\n"),
    ]);
    assert_eq!(broken(dir.path()), ["docs/a.md ../old/b.md", "readme.md docs/gone.md"]);
}

/// 20 files under `docs/` were linked from nowhere after the regrouping.
#[test]
fn test_a_doc_no_index_links_is_an_orphan() {
    let dir = repo(&[
        ("readme.md", "[docs](docs)\n"),
        ("docs/readme.md", "[a](a.md)\n"),
        ("docs/a.md", "[b](sub/b.md)\n"),
        ("docs/sub/b.md", "end\n"),
        ("docs/lost.md", "[b](sub/b.md)\n"),
        ("other/readme.md", "not under docs\n"),
    ]);
    assert_eq!(orphans(dir.path()), ["docs/lost.md"]);
}

/// `docs/testing.md` and `docs/history/takeover-progress.md` stayed named in
/// prose after they moved or were deleted.
#[test]
fn test_a_doc_path_named_in_prose_that_is_gone_is_found() {
    let dir = repo(&[
        ("a.md", "see `docs/gone.md`, docs/here.md. and https://x/blob/c/docs/other.md and (docs/gone2.md)\n"),
        ("docs/here.md", "x\n"),
    ]);
    assert_eq!(named_missing(dir.path()), ["a.md docs/gone.md", "a.md docs/gone2.md"]);
}

/// A pass added to the pipeline with no word in `passes.md` left the
/// catalog incomplete without a failure.
#[test]
fn test_a_source_file_a_catalog_does_not_name_is_found() {
    let dir = repo(&[
        ("src/alpha.rs", ""),
        ("src/beta.rs", ""),
        ("src/beta_tests.rs", ""),
        ("src/lib.rs", ""),
        ("src/gamma_delta.rs", ""),
        ("cat.md", "`alpha` and gamma only; alpha_beta is not it\n"),
    ]);
    assert_eq!(uncatalogued(dir.path(), "src", "cat.md"), ["src/beta.rs (cat.md)", "src/gamma_delta.rs (cat.md)"]);
}
