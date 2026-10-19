//! A block holds its instructions as a `Vec<InstId>`; LLVM's is an intrusive
//! list with a cursor. Every reader of the slice (`.instructions()` and the
//! `Vec` calls on the field inside llrm-mir) is a place the list swap must
//! rewrite, so their number may only fall. `tools/block_list/known.txt` holds
//! each file's count. New code uses the cursor API. A file over its count
//! fails with the file:line of its readers.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const KNOWN: &str = "tools/block_list/known.txt";
const FIELD_CALLS: [&str; 22] = [
    "[",
    ".iter",
    ".iter_mut",
    ".len",
    ".insert",
    ".remove",
    ".push",
    ".first",
    ".last",
    ".get",
    ".retain",
    ".drain",
    ".splice",
    ".truncate",
    ".swap",
    ".is_empty",
    ".contains",
    ".clear",
    ".extend",
    ".pop",
    ".position",
    ".as_slice",
];

fn sources(
    dir: &Path,
    out: &mut Vec<String>,
) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path.strip_prefix(ROOT).unwrap().to_string_lossy().into_owned());
        }
    }
}

/// The 1-based lines of `text` that read a block's instruction slice. The
/// field's `Vec` calls count only in llrm-mir, where the field is visible.
fn readers(
    file: &str,
    text: &str,
) -> Vec<usize> {
    let in_mir = file.starts_with("crates/ir/llrm-mir/src/");
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let code = line.split("//").next().unwrap();
            code.contains(".instructions()")
                || (in_mir
                    && code.match_indices(".instructions").any(|(at, name)| {
                        let after = &code[at + name.len()..];
                        FIELD_CALLS
                            .iter()
                            .any(
                                |call| after.strip_prefix(call).is_some_and(|rest| {
                                    !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') || call.ends_with('[')
                                }),
                            )
                    }))
        })
        .map(|(number, _)| number + 1)
        .collect()
}

fn counted() -> BTreeMap<String, Vec<usize>> {
    let mut files = Vec::new();
    for dir in ["crates", "src"] {
        sources(&Path::new(ROOT).join(dir), &mut files);
    }
    files
        .into_iter()
        .filter_map(|file| {
            let lines = readers(&file, &fs::read_to_string(Path::new(ROOT).join(&file)).unwrap());
            (!lines.is_empty()).then_some((file, lines))
        })
        .collect()
}

fn known() -> BTreeMap<String, usize> {
    fs::read_to_string(Path::new(ROOT).join(KNOWN))
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (file, count) = line.rsplit_once(' ').unwrap();
            (file.to_string(), count.parse().unwrap())
        })
        .collect()
}

fn known_text(counts: &BTreeMap<String, Vec<usize>>) -> String {
    let total: usize = counts.values().map(Vec::len).sum();
    let mut out = format!(
        "# Readers of a block's instruction slice per file, total {total}; written by `cargo test --test \
         block_list_ratchet write_known -- --ignored`. The count may only fall: use the cursor API.\n"
    );
    for (file, lines) in counts {
        out += &format!("{file} {}\n", lines.len());
    }
    out
}

#[test]
fn test_a_planted_slice_reader_is_found_and_the_cursor_api_is_not() {
    let text = "let a = block.instructions();\nlet b = block.first();\nlet c = x.instructions.len(); // x\nlet d = done.instructions + 1;\nlet e = b.instructions[0];\nlet f = b.instructions_len();\n";
    assert_eq!(readers("crates/ir/llrm-mir/src/a.rs", text), vec![1, 3, 5]);
    assert_eq!(readers("crates/opt/llrm-transforms/src/a.rs", text), vec![1]);
}

#[test]
fn test_no_file_gains_a_reader_of_the_instruction_slice() {
    let (counts, known) = (counted(), known());
    let mut worse = Vec::new();
    let mut better = Vec::new();
    for (file, lines) in &counts {
        let allowed = known.get(file).copied().unwrap_or(0);
        if lines.len() > allowed {
            worse.push(format!("{file}: {} readers, {allowed} allowed, at lines {lines:?}", lines.len()));
        } else if lines.len() < allowed {
            better.push(format!("{file}: {} readers, {allowed} listed", lines.len()));
        }
    }
    better.extend(known.keys().filter(|file| !counts.contains_key(*file)).map(|file| format!("{file}: none left")));
    assert!(
        worse.is_empty(),
        "a new reader of the instruction slice; use the cursor API (alim/feat/block-list-api):\n{}",
        worse.join("\n")
    );
    assert!(
        better.is_empty(),
        "readers fell; lower {KNOWN} (cargo test --test block_list_ratchet write_known -- --ignored):\n{}",
        better.join("\n")
    );
}

#[test]
#[ignore = "writes tools/block_list/known.txt"]
fn write_known() {
    fs::write(Path::new(ROOT).join(KNOWN), known_text(&counted())).unwrap();
}
