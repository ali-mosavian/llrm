//! A field of `pipeline::Options` that nothing reads is a switch that does
//! nothing: `peel` (set by `-fpeel-loops`, read only by `wanted("peel")`, which
//! no pass asks), `expensive` (set, never read) and `limits.grows` changed no
//! object on any target.

use std::fs;
use std::path::{Path, PathBuf};

fn sources(
    dir: &Path,
    found: &mut Vec<PathBuf>,
) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" && name != "target" {
                sources(&path, found);
            }
        } else if name.ends_with(".rs") && !name.ends_with("_tests.rs") {
            found.push(path);
        }
    }
}

fn workspace_sources() -> Vec<(PathBuf, String)> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = Vec::new();
    sources(&crates, &mut found);
    found.into_iter().map(|path| (path.clone(), fs::read_to_string(path).unwrap())).collect()
}

/// The text between `start` and the first `end` after it.
fn between<'a>(
    text: &'a str,
    start: &str,
    end: &str,
) -> &'a str {
    let from = text.find(start).unwrap_or_else(|| panic!("{start}")) + start.len();
    &text[from..from + text[from..].find(end).unwrap_or_else(|| panic!("{end}"))]
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// How many times `.field` is read as a whole word.
fn reads(
    text: &str,
    field: &str,
) -> usize {
    let needle = format!(".{field}");
    text.match_indices(&needle)
        .filter(|(at, _)| !text.as_bytes().get(at + needle.len()).is_some_and(|b| is_word(*b)))
        .count()
}

#[test]
fn every_options_field_is_read_by_something() {
    let all = workspace_sources();
    let pipeline = all
        .iter()
        .find(|(path, _)| path.ends_with("llrm-transforms/src/pipeline.rs"))
        .map(|(_, text)| text.as_str())
        .unwrap();
    let fields: Vec<&str> = between(pipeline, "pub struct Options {", "\n}\n")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .filter_map(|line| line.split(':').next())
        .collect();
    assert!(fields.len() > 30, "{fields:?}");
    let unread: Vec<&str> = fields
        .iter()
        .copied()
        .filter(|field| all.iter().map(|(_, text)| reads(text, field)).sum::<usize>() == 0)
        .collect();
    assert!(unread.is_empty(), "Options fields nothing reads: {unread:?}");
}

#[test]
fn every_name_wanted_can_be_asked() {
    let all = workspace_sources();
    let pipeline = all
        .iter()
        .find(|(path, _)| path.ends_with("llrm-transforms/src/pipeline.rs"))
        .map(|(_, text)| text.as_str())
        .unwrap();
    let wanted = between(pipeline, "pub(crate) fn wanted(", "\n    }\n");
    let mut dead = Vec::new();
    for line in wanted.lines().filter(|line| line.contains("=>") && line.trim_start().starts_with('"')) {
        let names: Vec<&str> =
            line.split("=>").next().unwrap().split('|').map(|name| name.trim().trim_matches('"')).collect();
        let asked = names.iter().any(|name| {
            let quoted = format!("\"{name}\"");
            all.iter().map(|(_, text)| text.matches(&quoted).count()).sum::<usize>() > wanted.matches(&quoted).count()
        });
        if !asked {
            dead.push(line.trim());
        }
    }
    assert!(dead.is_empty(), "wanted() arms no pass asks for: {dead:?}");
}
