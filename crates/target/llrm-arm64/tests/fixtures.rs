use std::path::Path;

/// `tests/darwin/division.ll` divided by a literal zero (`(signed char)-256` is 0): undefined, so the
/// pin folded the whole function to `poison` and the parity table read 8840 bytes against 272 as a gap.
#[test]
fn no_darwin_fixture_divides_by_a_literal_zero() {
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/darwin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "ll") {
            continue;
        }
        for (number, line) in std::fs::read_to_string(&path).unwrap().lines().enumerate() {
            let words: Vec<&str> = line.split_whitespace().collect();
            let divides = words.iter().any(|w| matches!(*w, "sdiv" | "udiv" | "srem" | "urem"));
            if divides && words.last() == Some(&"0") {
                bad.push(format!("{}:{}: {line}", path.display(), number + 1));
            }
        }
    }
    assert!(bad.is_empty(), "a division by a literal 0:\n{}", bad.join("\n"));
}
