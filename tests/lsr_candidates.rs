//! lsr's candidates for the uses of a loop.

mod common;

/// x_life's generation step: the 3 x 3 neighbour sum is two loops of three
/// trips.
const KERNEL: &str = "
static unsigned char g[2][24][24];
long bench_x_life(int n)
{
    int x, y, gen, cur = 0, dx, dy; long alive = 0;
    for (y = 0; y < n; ++y) for (x = 0; x < n; ++x) g[0][y][x] = (unsigned char)(((x * 7 + y * 13) % 5) == 0);
    for (gen = 0; gen < 10; ++gen) {
        for (y = 1; y < n - 1; ++y) for (x = 1; x < n - 1; ++x) {
            int c = 0;
            for (dy = -1; dy <= 1; ++dy) for (dx = -1; dx <= 1; ++dx) c += g[cur][y + dy][x + dx];
            c -= g[cur][y][x];
            g[1 - cur][y][x] = (unsigned char)(c == 3 || (c == 2 && g[cur][y][x]));
        }
        cur = 1 - cur;
    }
    for (y = 0; y < n; ++y) for (x = 0; x < n; ++x) alive += g[cur][y][x];
    return alive;
}
";

/// Every address use added a candidate for its own constant start (one per copy
/// of an unrolled body: a 65-use loop had 133 candidates and lsr spent 370
/// Minstr of 951), though the address takes the constant as its displacement.
/// The extra candidates also crowded out the exit-tested one: the `dy` loop
/// counted from -1 and compared with 2 (`inc ecx; cmp ecx, 2; jne`) where it
/// can count from -3 to zero (`inc ecx; jne`), a compare less a trip.
#[test]
fn test_an_address_use_adds_no_candidate_for_its_constant_and_the_exit_counts_to_zero() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(directory)
        // Without the unroller: since the pin's LoopFullUnroll cost
        // (7761d2b672) the `dy` loop may be copied out at -O2, and what this
        // test pins is lsr's choice for the loop that stays.
        .args(["-m32", "-O2", "-march=i486", "-fno-unroll-loops", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert!(listing.contains(", -3\n"), "{listing}");
}
