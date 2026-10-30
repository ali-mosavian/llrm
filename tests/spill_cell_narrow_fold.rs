//! A fold into a word spill cell stores a word.

use std::process::Command;

/// jpeg_fixed's `convert` at `-O2 -march=i486`: a value read only as a word
/// has a word cell, and folding its 4-byte copy stored a dword there, over the
/// next cell (the program panicked at run time).
#[test]
fn test_a_dword_copy_folded_into_a_word_cell_does_not_overlap_the_next_cell() {
    let source = format!("{}/bench/jpeg_fixed/jpeg_fixed.nib", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .current_dir(scratch.path())
        .args(["-O2", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let mut procedure = "";
    let mut cells: Vec<(i64, i64)> = Vec::new();
    let check = |procedure: &str, cells: &mut Vec<(i64, i64)>| {
        cells.sort();
        cells.dedup();
        for pair in cells.windows(2) {
            let ((low, width), (next, _)) = (pair[0], pair[1]);
            assert!(next >= low + width || next == low, "{procedure}: [bp{low}] ({width} bytes) overlaps [bp{next}]");
        }
        cells.clear();
    };
    for line in text.lines() {
        if let Some((name, _)) = line.split_once(" proc ") {
            check(procedure, &mut cells);
            procedure = name;
        }
        for (keyword, width) in [("byte", 1), ("word", 2), ("dword", 4), ("qword", 8)] {
            if let Some(rest) = line.split(&format!(" {keyword} ptr [bp-")).nth(1) {
                if let Some(offset) = rest.split(']').next().and_then(|n| n.parse::<i64>().ok()) {
                    cells.push((-offset, width));
                }
            }
        }
    }
    check(procedure, &mut cells);
}
