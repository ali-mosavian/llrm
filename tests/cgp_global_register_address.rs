//! A global indexed by two registers is one memory operand, not a `lea` made
//! outside the loop.

mod common;

/// `tile` is external: a `static` array nothing writes is constant zero, and
/// the pin's `GlobalOpt` folds the whole walk to `return 0` (so do we).
const KERNEL: &str = "
short tile[64][64];

long walk(short dy)
{
    long total = 0;
    short x, y;
    for (y = 0; y < 25; ++y)
        for (x = 0; x < 40; ++x)
            total += tile[(y + dy) & 63][(x + 7) & 63];
    return total;
}
";

/// The row address `_tile + row` was made before the inner loop (`lea bx,
/// _tile[bx]`, 4 bytes at m16, and a register held across the loop) where the
/// pin folds the symbol into the load (`_tile+94[bx+si]`): `cgpaddr` had no
/// base-register-and-global form (CGP1). tile.c m16 -Os, +10 bytes.
fn listing(target: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(target)
        .args(["--isel=dag", "-Os", "-S", "-o", "-", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn test_a_global_indexed_by_two_registers_has_no_lea_m32() {
    let asm = listing(&["-m32", "-mabi=sysv", "-march=i486"]);
    assert!(!asm.contains("lea") && asm.contains("_tile+"), "{asm}");
}

#[test]
fn test_a_global_indexed_by_two_registers_has_no_lea_m16() {
    let asm = listing(&["-m16", "-march=i386"]);
    assert!(!asm.contains("lea") && asm.contains("_tile+"), "{asm}");
}
