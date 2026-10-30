//! Every full ```mir block of a pass, analysis or backend header parses and
//! verifies. `define ... @NAME(ARGS) ... {`, where the header elides the
//! attributes, is read as `define void @NAME(ARGS) {`. A block that elides text
//! with `...` or has no `define`, or calls or cites metadata that the header
//! does not declare, is an excerpt: it is skipped, counted and printed.

use std::fs;
use std::process::Command;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

const DIRS: [&str; 3] =
    ["crates/opt/llrm-analysis/src", "crates/opt/llrm-transforms/src", "crates/codegen/llrm-codegen/src/backend"];

struct Block {
    file: String,
    line: usize,
    text: String,
}

fn tracked(dir: &str) -> Vec<String> {
    let out = Command::new("git").args(["ls-files", "-z", &format!("{dir}/*.rs")]).current_dir(ROOT).output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().split('\0').filter(|name| !name.is_empty()).map(String::from).collect()
}

fn blocks(file: &str) -> Vec<Block> {
    let source = fs::read_to_string(format!("{ROOT}/{file}")).unwrap();
    let mut found = Vec::new();
    let mut open: Option<Block> = None;
    let mut started = false;
    for (number, raw) in source.lines().enumerate() {
        let Some(rest) = raw.trim_start().strip_prefix("//!") else {
            if started || !raw.trim().is_empty() {
                break;
            }
            continue;
        };
        started = true;
        let line = rest.strip_prefix(' ').unwrap_or(rest);
        match open.take() {
            None if line.trim_end() == "```mir" => {
                open = Some(Block { file: file.into(), line: number + 1, text: String::new() })
            }
            None => {}
            Some(block) if line.trim_start().starts_with("```") => found.push(block),
            Some(mut block) => {
                block.text.push_str(line);
                block.text.push('\n');
                open = Some(block);
            }
        }
    }
    found
}

fn return_type(text: &str) -> String {
    let ret = text.lines().find_map(|line| line.trim_start().strip_prefix("ret "));
    ret.and_then(|rest| rest.split_whitespace().next()).unwrap_or("void").to_string()
}

fn normalized(text: &str) -> String {
    let ty = return_type(text);
    let lines: Vec<String> = text
        .lines()
        .map(|line| match (line.starts_with("define "), line.find('@'), line.rfind(')')) {
            (true, Some(at), Some(end)) if line.contains("...") && line.ends_with('{') => {
                format!("define {ty} {} {{", &line[at..=end])
            }
            _ => line.to_string(),
        })
        .collect();
    lines.join("\n")
}

fn symbols(text: &str) -> Vec<(String, bool)> {
    let mut found = Vec::new();
    let bytes = text.as_bytes();
    let mut at = 0;
    while let Some(start) = text[at..].find('@').map(|n| n + at) {
        let quoted = bytes.get(start + 1) == Some(&b'"');
        let end = if quoted {
            text[start + 2..].find('"').map_or(text.len(), |n| start + 3 + n)
        } else {
            text[start + 1..]
                .find(|c: char| !(c.is_alphanumeric() || "._$".contains(c)))
                .map_or(text.len(), |n| start + 1 + n)
        };
        found.push((text[start..end].to_string(), text[end..].starts_with('(')));
        at = end.max(start + 1);
    }
    found
}

fn excerpt(text: &str) -> Option<String> {
    let text = normalized(text);
    if !text.lines().any(|line| line.starts_with("define ")) {
        return Some("no define".into());
    }
    if text.contains("...") {
        return Some("elided with ...".into());
    }
    let defined = |name: &str| {
        text.lines().any(|line| {
            (line.starts_with("define ") || line.starts_with("declare ")) && line.contains(&format!("{name}("))
                || line.starts_with(&format!("{name} ="))
        })
    };
    let missing = symbols(&text).into_iter().find(|(name, called)| *called && !defined(name));
    if let Some((name, _)) = missing {
        return Some(format!("calls {name}, declared elsewhere"));
    }
    let metadata = text
        .split(|c: char| !(c.is_alphanumeric() || c == '!'))
        .find(
            |word| word.len() > 1
                && word.starts_with('!')
                && word[1..].bytes().all(|b| b.is_ascii_digit())
                && !text.contains(&format!("{word} = ")),
        );
    metadata.map(|word| format!("uses {word}, defined elsewhere"))
}

fn with_stubs(text: &str) -> String {
    let text = normalized(text);
    let mut stubs = String::new();
    for (name, called) in symbols(&text) {
        let known = text.contains(&format!("{name} = ")) || stubs.contains(&format!("{name} = "));
        if !called && !known {
            stubs.push_str(&format!("{name} = global i8 0\n"));
        }
    }
    format!("{stubs}{text}")
}

#[test]
fn test_every_full_mir_block_of_a_header_parses_and_verifies() {
    let mut checked = 0;
    let mut skipped = Vec::new();
    let mut failed = Vec::new();
    for file in DIRS.iter().flat_map(|dir| tracked(dir)) {
        for block in blocks(&file) {
            if let Some(why) = excerpt(&block.text) {
                skipped.push(format!("{}:{}: {why}", block.file, block.line));
                continue;
            }
            checked += 1;
            match llrm_mir::parse::module(&with_stubs(&block.text)) {
                Err(error) => failed.push(format!("{}:{}: {error}", block.file, block.line)),
                Ok(module) => {
                    for problem in llrm_mir::verify::verify(&module) {
                        failed.push(format!("{}:{}: verify: {problem}", block.file, block.line));
                    }
                }
            }
        }
    }
    println!("{checked} blocks checked, {} skipped:\n{}", skipped.len(), skipped.join("\n"));
    assert!(checked > 0, "no ```mir block found: the scan reads nothing");
    assert!(failed.is_empty(), "{} header blocks do not parse or verify:\n{}", failed.len(), failed.join("\n"));
}

#[test]
fn test_a_block_without_a_define_or_with_an_elision_is_an_excerpt() {
    assert_eq!(excerpt("b1:\n  ret void\n").as_deref(), Some("no define"));
    assert_eq!(excerpt("define void @f() {\n  ; ...\n}\n").as_deref(), Some("elided with ..."));
    assert!(excerpt("define void @f() {\nb0:\n  call void @g()\n  ret void\n}\n").is_some());
    assert!(excerpt("define void @f() {\nb0:\n  ret void, !dbg !9\n}\n").is_some());
    assert_eq!(excerpt("define void @f() {\nb0:\n  ret void\n}\n"), None);
    assert_eq!(excerpt("define ... @f(i16 %x) ... {\nb0:\n  ret void\n}\n"), None);
}

#[test]
fn test_elided_attributes_of_a_define_are_dropped() {
    assert_eq!(normalized("define ... @f(i16 %x) ... {\nb0:"), "define void @f(i16 %x) {\nb0:");
    assert_eq!(normalized("define ... @f() ... {\n  ret i16 0"), "define i16 @f() {\n  ret i16 0");
}
