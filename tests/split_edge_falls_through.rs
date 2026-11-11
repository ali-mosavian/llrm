//! A split critical edge is laid out right after its predecessor.

use std::process::Command;

/// The reduced m16 -Os `safe` loop kept `jcc split; jmp next` where the pin's
/// `SplitCriticalEdge` + `updateTerminator` leaves `j!cc next`. The four extra
/// instruction slots lowered the spill weights 4-11% and the allocator evicted
/// the loop pointer for the invariant: 3 frame operations a trip, the pin's 2.
#[test]
fn a_split_edge_costs_the_loop_no_extra_frame_operation() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/safe_loop_reduced.ll");
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
    // The loop is what the last backward jump closes.
    let (back, top) = lines
        .iter()
        .enumerate()
        .filter_map(|(at, line)| {
            let target = line.strip_prefix('j')?.split_once(' ')?.1;
            let top = lines.iter().position(|label| label.strip_suffix(':') == Some(target))?;
            (top < at).then_some((at, top))
        })
        .last()
        .expect("a loop");
    let frame = lines[top..=back].iter().filter(|line| line.contains("[bp-")).count();
    assert_eq!(frame, 2, "{text}");
}
