//! At -Os a loop of several stores stays one where a fill for each store is
//! more bytes than the loop.

use std::process::Command;

fn fill_listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "short x[64], y[64];\nvoid init(void)\n{\n    int i;\n    for (i = 0; i < 64; ++i) {\n        x[i] = 0;\n        y[i] = -1;\n    }\n}\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(flags)
        .args(["-m16", "-march=i386", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// lru.c m16 -Os: a 3-trip loop of four stores became five `rep stosb`, 32
/// bytes over the loop.
#[test]
fn at_os_two_stores_a_trip_are_not_two_rep_stos() {
    let text = fill_listing(&["-Os"]);
    assert!(!text.contains("rep stos"), "{text}");
}

#[test]
fn at_o2_the_same_loop_is_fills() {
    let text = fill_listing(&["-O2"]);
    assert!(text.contains("rep stos"), "{text}");
}
