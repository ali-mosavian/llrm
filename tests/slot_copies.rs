//! A spill copied from a frame cell reads that cell, and the frame has no cell
//! more for it (`slotcopies`).

mod common;

/// x87 `lrint` stores through a stack temporary; the result lives across a
/// call.
const SOURCE: &str = "extern void p(long);
extern void q(long);
void g(double x) { long v = (long)x; p(v); q(v); }
";

fn listing() -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let done = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["-m16", "-march=i386", "-Os", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// `sub sp, N` of the listing.
fn frame_bytes(asm: &str) -> u32 {
    asm.lines().find_map(|line| line.trim().strip_prefix("sub sp, ")?.parse().ok()).unwrap_or(0)
}

/// Stores of `eax` to a frame cell.
fn stores_of_eax(asm: &str) -> usize {
    asm.lines().filter(|line| line.trim().starts_with("mov dword ptr [bp-") && line.trim().ends_with(", eax")).count()
}

#[test]
fn a_value_loaded_from_a_cell_is_not_copied_to_a_spill_cell() {
    let asm = listing();
    assert!(asm.contains("fistp dword ptr [bp-4]"), "premise: the conversion goes through a frame cell: {asm}");
    assert_eq!(stores_of_eax(&asm), 0, "{asm}");
    assert_eq!(frame_bytes(&asm), 8, "{asm}");
}

#[test]
fn the_check_counts_the_store_it_names() {
    assert_eq!(stores_of_eax("    mov dword ptr [bp-8], eax\n"), 1);
    assert_eq!(frame_bytes("    sub sp, 12\n"), 12);
}
