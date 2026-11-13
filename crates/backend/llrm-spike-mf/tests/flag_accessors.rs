//! `Inst::sets_flags` is a full kill of every flag lane. Passes read it through
//! the accessor, which carries that contract; a raw field read bypasses it
//! (#9532: `eliminate_tests` kept a zero test across an `add` that its own
//! exception said left the lane alone, and a divide miscompiled).

use std::path::Path;

/// The lines of `source` that read `.sets_flags` or `.reads_flags` as a field:
/// not a call, not an assignment, not a comment.
fn raw_reads(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for name in [".sets_flags", ".reads_flags"] {
            let mut rest = code;
            while let Some(at) = rest.find(name) {
                let after = &rest[at + name.len()..];
                rest = after;
                if after.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let after = after.trim_start();
                let assigned = after.starts_with('=') && !after.starts_with("==");
                if !after.starts_with('(') && !assigned {
                    found.push((number + 1, line.trim().to_string()));
                }
            }
        }
    }
    found
}

fn sources(
    dir: &Path,
    out: &mut Vec<std::path::PathBuf>,
) {
    for entry in std::fs::read_dir(dir).expect("a source directory") {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// A read of the field that the test cannot see is the failure this guards:
/// the scan has to flag a planted one, and pass writes and calls.
#[test]
fn the_scan_flags_a_planted_field_read() {
    assert_eq!(raw_reads("if inst.sets_flags {").len(), 1);
    assert_eq!(raw_reads("a.reads_flags == b.reads_flags").len(), 2);
    assert_eq!(raw_reads("!i.sets_flags && x").len(), 1);
    assert!(raw_reads("inst.sets_flags = true;").is_empty());
    assert!(raw_reads("inst.sets_flags()").is_empty());
    assert!(raw_reads("// inst.sets_flags is a doc").is_empty());
    assert!(raw_reads("inst.sets_flags_after()").is_empty());
}

#[test]
fn no_pass_reads_the_flag_fields_raw() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    sources(&root.join("../../target/llrm-z80/src"), &mut files);
    sources(&root.join("../../target/llrm-6502/src"), &mut files);
    sources(&root.join("../../target/llrm-m68k/src"), &mut files);
    let mut bad = Vec::new();
    for file in files {
        if file.ends_with("llrm-spike-mf/src/mfunc.rs") {
            continue;
        }
        for (line, text) in raw_reads(&std::fs::read_to_string(&file).unwrap()) {
            bad.push(format!("{}:{line}: {text}", file.display()));
        }
    }
    assert!(bad.is_empty(), "read through Inst::sets_flags() / reads_flags():\n{}", bad.join("\n"));
}
