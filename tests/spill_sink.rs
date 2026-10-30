//! Where the spill stores of the allocator land.

mod common;

use std::process::Command;

/// The copy that defined a spilled value, folded into a store, was not known as
/// a spill store, so `postrasink` left it in the branching block: lru's
/// `mov ax, [bp-4]; mov [bp-2], ax` ran on the path that never reads the cell
/// (+2 B at -Os).
#[test]
fn test_a_folded_copy_that_stores_a_spill_moves_to_the_block_that_reads_it() {
    let source = format!("{}/bench/lru/lru.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-Os", "-m16", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let stored =
        lines.windows(2).any(|pair| pair[0] == "mov ax, word ptr [bp-4]" && pair[1] == "mov word ptr [bp-2], ax");
    assert!(!stored, "{text}");
}

/// A spill store sunk into a block entered from elsewhere got a block of its
/// own and a jump to it: bintree's walk at -O3 held 147 instructions where 144
/// do (+7 B). Such a store stays where it was.
#[test]
fn test_a_spill_store_is_not_bridged_into_a_block_entered_from_elsewhere() {
    let source = format!("{}/bench/bintree/bintree.nib", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .current_dir(scratch.path())
        .args(["-O3", "-m16", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let count = text.lines().filter(|line| line.starts_with("    ")).count();
    assert!(count <= 144, "{count} instructions\n{text}");
}
