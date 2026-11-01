//! The imaginary registers are named by `target::imaginary` alone: 221 literals
//! of `rc2`... across the emitters meant a rename (`__rcN`, another base)
//! touched every one and missed some.

use std::path::Path;

#[test]
fn only_the_target_description_spells_an_imaginary_register() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut spelled = Vec::new();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs") || path.file_name().is_some_and(|n| n == "target.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for (at, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            let mut rest = code;
            while let Some(found) = rest.find("rc") {
                let before = rest[..found].chars().next_back();
                let digit = rest[found + 2..].chars().next().is_some_and(|c| c.is_ascii_digit());
                if digit && !before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    spelled.push(format!("{}:{}: {}", path.display(), at + 1, line.trim()));
                }
                rest = &rest[found + 2..];
            }
        }
    }
    assert!(spelled.is_empty(), "spelled outside target.rs:\n{}", spelled.join("\n"));
}
