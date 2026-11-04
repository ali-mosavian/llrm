//! A float loop's carried values are read back from the cells the program
//! stores them to.
//!
//! Compiled with `-fno-peel-loops -fno-unroll-loops`, the shape these were
//! written for: peeling the `j` loop by the extent of `x[4]` (#1673) copies the
//! pair loops out as well (38 stores to 21 cells instead of 12; nbody -O3
//! 383,565 → 354,259 clocks), and the full unroll (slot 31, 7761d2b672) copies
//! the whole 4x4 nest: neither is what they count. The unrolled shape has a
//! test of its own below.

mod common;

/// nbody's pair loop: `x[i]` and `y[i]` are invariant in the inner loop, five
/// more floats live through the nest. The spill victim was the read furthest
/// ahead in block order, which ignores the back edge: an invariant read
/// again on the next trip looked never read again, was spilled at its
/// definition, and every outer trip paid an `fld; fstp` per invariant (nbody
/// 61477 against 60677 instructions once the hoist worked).
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

/// The running values `x[0..3]`, `y[0..3]` are phis of the step loop, the
/// program stores each to its array cell on every trip, and nine floats crowd
/// the x87 stack under the inner loop. Each was spilled to a cell of its own
/// besides, 14 stores more than the 12 the program makes (nbody 61880 against
/// 59081 instructions). -O3: gcc's -O2 copies a loop out only where the code
/// does not grow, so the nest stays rolled there (6 stores).
// DEBT x87-zero-memset: the pin merges the zero `double` stores into a memset
// (four zero dword stores), 10 fp-cell stores here; ours is `fldz`
// and `fst`, 18. The fix (MemCpyOpt
// `isBytewiseValue` of floats, #6007) grows nbody objects (nib -O2 +103 B,
// bq32 -O2 +80, bq32 -Os +46) and is held until traced.
#[test]
#[ignore = "DEBT x87-zero-memset"]
fn test_a_stored_loop_carried_float_is_spilled_to_the_cell_the_program_stores_it_in() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-O3", "-fno-peel-loops", "-fno-unroll-loops", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let stores = listing
        .lines()
        .filter(|line| {
            line.trim_start().starts_with("fstp qword ptr [") || line.trim_start().starts_with("fst qword ptr [")
        })
        .count();
    // The program makes 12; the code makes 6 (clang -O3 -fno-unroll-loops: 10).
    assert!(stores <= 12, "{stores} stores: {listing}");
}

/// -Os hoisted `x[i]` and `y[i]` out of the inner loop of 1.5 trips: two `fld`
/// before it and two `fstp st(0)` after, 4 B and 800 instructions more than not
/// hoisting (x86-m32 priced -Os in clocks, and nothing priced the release).
/// Since d921656cda (the loop header is copied at -Os as the pin does) the
/// hoist is the pin's own: clang -Os -mno-sse has 2 such pops here, ours 3
/// (X87-GAP-3, a stackifier detail; the bound tightens to 2 with its fix).
#[test]
fn test_os_does_not_hold_floats_across_a_loop_for_the_release_they_cost() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-Os", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let pops = listing.matches("fstp st(0)").count();
    assert!(pops <= 3, "{pops} pops: {listing}");
}

/// A phi's input read back from its cell leaves its constant unread: `fld1;
/// fstp st(0)`, eleven pairs in nbody's prologue, 40 B of code and 22
/// instructions that do nothing.
// DEBT x87-zero-memset: the pin merges the zero `double` stores into a memset
// (four zero dword stores), 10 fp-cell stores here; ours is `fldz`
// and `fst`, 18. The fix (MemCpyOpt
// `isBytewiseValue` of floats, #6007) grows nbody objects (nib -O2 +103 B,
// bq32 -O2 +80, bq32 -Os +46) and is held until traced.
#[test]
#[ignore = "DEBT x87-zero-memset"]
fn test_a_constant_nothing_reads_is_not_loaded_and_popped() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-O3", "-fno-peel-loops", "-fno-unroll-loops", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let stores =
        lines.iter().filter(|line| line.starts_with("fstp qword ptr [") || line.starts_with("fst qword ptr [")).count();
    assert!(stores <= 12, "premise: the nest is rolled, as in the test above ({stores} stores)\n{listing}");
    let pairs = lines
        .windows(2)
        .filter(|pair| {
            (pair[0] == "fld1" || pair[0] == "fldz" || pair[0].starts_with("fld dword ptr $K"))
                && pair[1] == "fstp st(0)"
        })
        .count();
    assert_eq!(pairs, 0, "{listing}");
}

/// The unrolled nest (-O3, no `-fno-unroll-loops`): the full unroll of slot 31
/// copies the pair loops out and stores 56 qwords, where clang -O3 -mno-sse
/// -march=i486 stores 48 (X87-GAP-1, w-1's code-growth row). The bound is
/// what the code does; it tightens to 48 with that row.
#[test]
fn test_the_unrolled_nest_stores_no_more_than_it_does_today() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-O3", "-fno-peel-loops", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let stores = listing
        .lines()
        .filter(|line| {
            line.trim_start().starts_with("fstp qword ptr [") || line.trim_start().starts_with("fst qword ptr [")
        })
        .count();
    assert!(stores <= 56, "{stores} stores: {listing}");
}
