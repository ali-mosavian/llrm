//! The SelectionDAG DEVIATE registry (`docs/working/selectiondag-plan.md`,
//! section 6) is the one list of `SD` numbers. Two groups took the same
//! number three times (SD10, SD14, SD21); each number now has one owner row,
//! one declaration, and the owner's block: SD44..SD99 are closed, then
//! sdag-a takes SD100.., sdag-b SD200.., sdag-c SD300.., sdag-d SD400...

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use regex::Regex;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const PLAN: &str = "docs/working/selectiondag-plan.md";

/// A file's text and its path, for messages.
struct Source {
    path: String,
    text: String,
}

fn tracked(pattern: &str) -> Vec<Source> {
    let out = Command::new("git").args(["ls-files", "-z", pattern]).current_dir(ROOT).output().expect("git runs");
    assert!(out.status.success(), "git ls-files failed");
    String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|path| !path.is_empty() && Path::new(ROOT).join(path).is_file())
        .map(|path| Source {
            path: path.to_owned(),
            text: fs::read_to_string(Path::new(ROOT).join(path)).expect("tracked file reads"),
        })
        .collect()
}

/// A number outside its owner's block: SD44..SD99 are closed, SD100.. go
/// to sdag-a, SD200.. to sdag-b, SD300.. to sdag-c, SD400.. to sdag-d.
fn block_problem(
    id: u32,
    owner: &str,
) -> Option<String> {
    if (44..100).contains(&id) {
        return Some(format!("SD{id} is in the closed range SD44..SD99"));
    }
    if id >= 100 {
        let letter = ["a", "b", "c", "d"].get((id / 100) as usize - 1)?;
        if owner != format!("sdag-{letter}") {
            return Some(format!("SD{id} is in sdag-{letter}'s block but its row says {owner}"));
        }
    }
    None
}

/// Every problem with the registry in `plan` given the declarations in
/// `code` (the `//!` headers of Rust files) and `docs` (other working docs).
fn problems(
    plan: &Source,
    code: &[Source],
    docs: &[Source],
) -> Vec<String> {
    let row = Regex::new(r"^\|\s*SD(\d+)\s*\|\s*([\w-]+)\s*\|").unwrap();
    let bold = Regex::new(r"^\s*[-*]\s+\*\*SD(\d+)\*\*").unwrap();
    let deviate = Regex::new(r"^//!\s*(?:[-*]\s+\*\*SD(\d+)\*\*|DEVIATE\s+SD(\d+):)").unwrap();
    let mut out = Vec::new();

    let mut rows: BTreeMap<u32, usize> = BTreeMap::new();
    for line in plan.text.lines() {
        if let Some(found) = row.captures(line) {
            let id: u32 = found[1].parse().unwrap();
            *rows.entry(id).or_default() += 1;
            if let Some(why) = block_problem(id, &found[2]) {
                out.push(why);
            }
        }
    }
    for (id, count) in &rows {
        if *count > 1 {
            out.push(format!("SD{id} has {count} rows in the registry of {}", plan.path));
        }
    }

    // Declarations: other docs' bullets and code headers (the plan's own
    // bullets describe, and the registry rows list).
    let mut declared: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (source, pattern) in docs.iter().map(|doc| (doc, &bold)).chain(code.iter().map(|file| (file, &deviate))) {
        for (at, line) in source.text.lines().enumerate() {
            if let Some(found) = pattern.captures(line) {
                let id: u32 = found.iter().skip(1).flatten().next().unwrap().as_str().parse().unwrap();
                declared.entry(id).or_default().push(format!("{}:{}", source.path, at + 1));
            }
        }
    }
    for (id, places) in &declared {
        if places.len() > 1 {
            out.push(format!("SD{id} is declared {} times: {}", places.len(), places.join(", ")));
        }
        if !rows.contains_key(id) {
            out.push(format!("SD{id} is declared at {} but has no row in the registry of {}", places[0], plan.path));
        }
    }

    if !plan.text.lines().any(|line| line.starts_with("Next free:") && line.contains("blocks:")) {
        out.push(format!("{} has no `Next free: ... blocks: ...` line", plan.path));
    }
    out
}

fn source(
    path: &str,
    text: &str,
) -> Source {
    Source { path: path.to_owned(), text: text.to_owned() }
}

fn real_tree() -> (Source, Vec<Source>, Vec<Source>) {
    let plan = source(PLAN, &fs::read_to_string(Path::new(ROOT).join(PLAN)).expect("the plan exists"));
    let code = tracked("*.rs");
    let docs = tracked("docs/working/*.md").into_iter().filter(|doc| doc.path != PLAN).collect();
    (plan, code, docs)
}

#[test]
fn the_registry_has_no_clash_and_every_declared_number_is_in_it() {
    let (plan, code, docs) = real_tree();
    let found = problems(&plan, &code, &docs);
    assert!(found.is_empty(), "SD registry:\n{}", found.join("\n"));
}

const PLAN_TEXT: &str = "| SD1 | a | x | y |\n| SD2 | b | x | y |\n\nNext free: SD44..SD99 closed; blocks: a SD100, b SD200, c SD300, d SD400.\n";

/// A number taken twice was found only by a human reading two PRs. A seeded
/// duplicate (a second header declaring SD2) must be named with both places.
#[test]
fn a_number_declared_twice_is_reported() {
    let plan = source("plan.md", PLAN_TEXT);
    let code = [source("a.rs", "//! DEVIATE SD2: one\n"), source("b.rs", "//! - **SD2** two\n")];
    let found = problems(&plan, &code, &[]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("SD2 is declared 2 times") && found[0].contains("a.rs:1") && found[0].contains("b.rs:1"),
        "{found:?}"
    );
}

#[test]
fn a_number_with_no_row_is_reported() {
    let plan = source("plan.md", PLAN_TEXT);
    let found = problems(&plan, &[source("c.rs", "//! DEVIATE SD3: new\n")], &[]);
    assert!(found.iter().any(|problem| problem.contains("SD3") && problem.contains("no row")), "{found:?}");
}

/// Numbers clashed three times; a row in the closed range or in another
/// owner's block is reported.
#[test]
fn a_number_outside_its_owners_block_is_reported() {
    let plan = source(
        "plan.md",
        "| SD50 | sdag-a | x | y |\n| SD150 | sdag-b | x | y |\n| SD120 | sdag-a | x | y |\n\nNext free: SD44..SD99 closed; blocks: a SD100, b SD200, c SD300, d SD400.\n",
    );
    let found = problems(&plan, &[], &[]);
    assert!(found.iter().any(|problem| problem.contains("SD50 is in the closed range")), "{found:?}");
    assert!(found.iter().any(|problem| problem.contains("SD150") && problem.contains("sdag-a's block")), "{found:?}");
    assert!(!found.iter().any(|problem| problem.contains("SD120")), "{found:?}");
}

#[test]
fn a_missing_next_free_line_is_reported() {
    let found = problems(&source("plan.md", "| SD1 | a | x | y |\n"), &[], &[]);
    assert!(found.iter().any(|problem| problem.contains("no `Next free")), "{found:?}");
}

#[test]
fn a_duplicate_registry_row_is_reported() {
    let plan = source(
        "plan.md",
        "| SD1 | a | x | y |\n| SD1 | b | x | y |\n\nNext free: SD44..SD99 closed; blocks: a SD100, b SD200, c SD300, d SD400.\n",
    );
    let found = problems(&plan, &[], &[]);
    assert!(found.iter().any(|problem| problem.contains("SD1 has 2 rows")), "{found:?}");
}

/// References such as `(DEVIATE SD11)` or `(SD30)` in a body are not
/// declarations.
#[test]
fn references_are_not_declarations() {
    let plan = source("plan.md", PLAN_TEXT);
    let code = [source(
        "a.rs",
        "//! DEVIATE SD1: real\n//! see (DEVIATE SD1) and SD2 in the text\nfn f() { /* DEVIATE SD1: not a header */ }\n",
    )];
    assert!(problems(&plan, &code, &[]).is_empty());
}
