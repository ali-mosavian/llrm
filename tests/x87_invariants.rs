//! A float loop's carried values are read back from the cells the program stores them to.

use std::process::Command;

/// nbody's pair loop: `x[i]` and `y[i]` are invariant in the inner loop, five more floats live through the nest.
/// The spill victim was the read furthest ahead in block order, which ignores the back edge: an invariant read
/// again on the next trip looked never read again, was spilled at its definition, and every outer trip paid an
/// `fld; fstp` per invariant (nbody 61477 against 60677 instructions once the hoist worked).
const KERNEL: &str = "
extern void report(long value);
long bench(unsigned short steps)
{
    double x[4], y[4], vx[4], vy[4];
    unsigned short step, i, j;
    x[0] = -1.0; x[1] = 1.0; x[2] = 0.0; x[3] = 0.0;
    y[0] = 0.0; y[1] = 0.0; y[2] = -1.0; y[3] = 1.0;
    vx[0] = 0.0; vx[1] = 0.0; vx[2] = 0.0125; vx[3] = -0.0125;
    vy[0] = -0.0125; vy[1] = 0.0125; vy[2] = 0.0; vy[3] = 0.0;
    for (step = 0; step < steps; ++step) {
        for (i = 0; i < 4; ++i)
            for (j = (unsigned short)(i + 1); j < 4; ++j) {
                double dx = x[j] - x[i], dy = y[j] - y[i];
                double scale = 0.00001 / (dx * dx + dy * dy + 0.125);
                vx[i] += dx * scale; vy[i] += dy * scale;
                vx[j] -= dx * scale; vy[j] -= dy * scale;
            }
        for (i = 0; i < 4; ++i) { x[i] += vx[i]; y[i] += vy[i]; }
    }
    return (long)((x[0] + y[1] + x[2] + y[3]) * 1000000.0);
}
";

/// The running values `x[0..3]`, `y[0..3]` are phis of the step loop, the program stores each to its
/// array cell on every trip, and nine floats crowd the x87 stack under the inner loop. Each was spilled to a cell
/// of its own besides, 14 stores more than the 12 the program makes (nbody 61880 against 59081 instructions).
#[test]
fn test_a_stored_loop_carried_float_is_spilled_to_the_cell_the_program_stores_it_in() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(directory).args(["-m32", "-O2", "-march=i486", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let stores = listing.lines().filter(|line| line.trim_start().starts_with("fstp qword ptr [") || line.trim_start().starts_with("fst qword ptr [")).count();
    assert_eq!(stores, 12, "{listing}");
}

/// -Os hoisted `x[i]` and `y[i]` out of the inner loop of 1.5 trips: two `fld` before it and two `fstp st(0)` after,
/// 4 B and 800 instructions more than not hoisting (x86-m32 priced -Os in clocks, and nothing priced the release).
#[test]
fn test_os_does_not_hold_floats_across_a_loop_for_the_release_they_cost() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(directory).args(["-m32", "-Os", "-march=i486", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert_eq!(listing.matches("fstp st(0)").count(), 0, "{listing}");
}

/// A phi's input read back from its cell leaves its constant unread: `fld1; fstp st(0)`, eleven pairs in nbody's
/// prologue, 40 B of code and 22 instructions that do nothing.
#[test]
fn test_a_constant_nothing_reads_is_not_loaded_and_popped() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(directory).args(["-m32", "-O2", "-march=i486", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let pairs = lines.windows(2).filter(|pair| (pair[0] == "fld1" || pair[0] == "fldz" || pair[0].starts_with("fld dword ptr $K")) && pair[1] == "fstp st(0)").count();
    assert_eq!(pairs, 0, "{listing}");
}
