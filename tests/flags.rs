//! Every compiler binary takes gcc's `-m16`/`-m32`/`-m64`, `-march=` and `-mtune=` from the one
//! parser, and none takes the spellings they replaced (`--target`, `--cpu`).
//!
//! The binaries are enumerated from `src/bin`: one nobody classified fails, so a new frontend cannot
//! ship with a parser of its own.

use std::path::Path;
use std::process::{Command, Output};

const C: &str = "int main(void) { return 0; }\n";
const NIB: &str = "fn main() -> i16:\n    return 0\n";
const BASIC: &str = "PRINT 1\n";

/// How a binary meets `-m32`: `Builds` compiles for it; `Refuses` takes the flag and says it builds
/// for another target only (BASIC and BC are code16's).
enum Meets {
    Builds,
    Refuses,
}

struct Tool {
    name: &'static str,
    path: &'static str,
    input: (&'static str, &'static str),
    /// Arguments that make a valid run beside the flag and the input.
    more: &'static [&'static str],
    meets: Meets,
}

fn tools() -> Vec<Tool> {
    vec![
        Tool { name: "llrm-c", path: env!("CARGO_BIN_EXE_llrm-c"), input: ("a.c", C), more: &["-S", "-o", "a.s"], meets: Meets::Builds },
        Tool { name: "llrm-nib", path: env!("CARGO_BIN_EXE_llrm-nib"), input: ("a.nib", NIB), more: &["-S", "-o", "a.s"], meets: Meets::Builds },
        Tool { name: "llrm-qb", path: env!("CARGO_BIN_EXE_llrm-qb"), input: ("a.bas", BASIC), more: &["--dialect", "qb45", "--runtime", "qb45", "-S", "-o", "a.s"], meets: Meets::Refuses },
        Tool { name: "llrm-omf", path: env!("CARGO_BIN_EXE_llrm-omf"), input: ("a.obj", "x"), more: &["--rich", "-o", "out.obj"], meets: Meets::Refuses },
        Tool { name: "llrm-run", path: env!("CARGO_BIN_EXE_llrm-run"), input: ("a.nib", NIB), more: &[], meets: Meets::Builds },
        Tool { name: "nibfront", path: env!("CARGO_BIN_EXE_nibfront"), input: ("a.nib", NIB), more: &[], meets: Meets::Builds },
    ]
}

/// Binaries that name no target, and why.
const EXEMPT: [(&str, &str); 2] = [("llrm-mir", "reads MIR, which states its own datalayout"), ("nib-lsp", "takes the target in `initialize` (`mode`), tested with the server")];

fn run(tool: &Tool, directory: &Path, flags: &[&str]) -> Output {
    std::fs::write(directory.join(tool.input.0), tool.input.1).unwrap();
    Command::new(tool.path).current_dir(directory).args(flags).arg(tool.input.0).args(tool.more).output().unwrap()
}

fn text(done: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&done.stdout), String::from_utf8_lossy(&done.stderr))
}

/// A frontend with a parser of its own would not be in this list.
#[test]
fn every_binary_is_classified() {
    let known: Vec<&str> = tools().iter().map(|one| one.name).chain(EXEMPT.iter().map(|(name, _)| *name)).collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut binaries: Vec<String> = Vec::new();
    for directory in ["src/bin", "crates/frontends/qbfront/src/bin"] {
        for entry in std::fs::read_dir(root.join(directory)).unwrap().flatten() {
            binaries.push(entry.path().file_stem().unwrap().to_string_lossy().into_owned());
        }
    }
    // `nibfront` and `qbparse-stages` only read; the first takes `-m` for what it declares.
    let unclassified: Vec<&String> = binaries.iter().filter(|name| !known.contains(&name.as_str()) && name.as_str() != "qbparse-stages").collect();
    assert!(unclassified.is_empty(), "name how these take the target flags, or why not: {unclassified:?}");
}

#[test]
fn every_compiler_takes_m32_from_the_one_parser() {
    for tool in tools() {
        let directory = tempfile::tempdir().unwrap();
        let done = run(&tool, directory.path(), &["-m32"]);
        match tool.meets {
            Meets::Builds => assert!(done.status.success(), "{} -m32: {}", tool.name, text(&done)),
            Meets::Refuses => {
                let said = text(&done);
                assert!(!done.status.success() && said.contains("-m16 only, not -m32"), "{} -m32: {said}", tool.name);
            }
        }
    }
}

/// `--target NAME` and `--cpu CPU` were spellings of their own; they are refused, not aliased.
#[test]
fn the_old_spellings_are_refused_everywhere() {
    for tool in tools() {
        for flags in [&["--target", "x86-code32"][..], &["--cpu", "486"][..]] {
            let directory = tempfile::tempdir().unwrap();
            let done = run(&tool, directory.path(), flags);
            assert!(!done.status.success(), "{} {flags:?} was accepted: {}", tool.name, text(&done));
        }
    }
}

/// A mode no target declares is a clear refusal, not a fall back to the default.
#[test]
fn m64_is_refused_where_no_target_declares_it() {
    for tool in tools().iter().filter(|one| !matches!(one.name, "llrm-omf" | "llrm-qb")) {
        let directory = tempfile::tempdir().unwrap();
        let done = run(tool, directory.path(), &["-m64"]);
        assert!(!done.status.success() && text(&done).contains("no target for -m64"), "{} -m64: {}", tool.name, text(&done));
    }
}
