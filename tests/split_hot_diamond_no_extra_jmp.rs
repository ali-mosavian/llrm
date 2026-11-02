//! A split edge must not turn the hot way of a diamond into a branch not
//! taken plus a jump.

use std::process::Command;

/// bench/grep's scan loop at m16 -Os: the line-end test `cmp cx, 10` is rare,
/// so the hot way is the `jne`. With the successors of a flipped branch
/// reordered, placement laid the hot block out after the branch as
/// `je cold; xor ecx, ecx; jmp join`: one more instruction a trip, 1,012,914
/// in all (3.7% of the clocks).
#[test]
fn the_hot_way_of_a_split_diamond_is_the_taken_branch() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/grep_scan_loop.ll");
    let scratch = tempfile::tempdir().unwrap();
    let out = scratch.path().join("x.s");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-Os", "-march=i486", "--isel=dag", "-S", "-o"])
        .arg(&out)
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(&out).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at =
        lines.iter().position(|line| line.starts_with("cmp ") && line.ends_with(", 10")).expect("the line-end test");
    let branch = lines[at + 1..].iter().find(|line| line.starts_with('j')).expect("its branch");
    assert!(branch.starts_with("jne "), "{branch}\n{text}");
}
