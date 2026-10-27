//! Spill slots are shared once, over the allocation kept.

mod common;

const KERNEL: &str = "
long f(int n)
{
    unsigned long long a = 0x123456789ABCDEF1ull, u = 0; long long s = 0; int i;
    for (i = 0; i < n; ++i) {
        unsigned long long d32, d64; long long sa, sd;
        a = a * 6364136223846793005ull + 1442695040888963407ull;
        d32 = (a >> 40) | 1; d64 = (a >> (11 + (i & 15))) | 1;
        u += a / d32 + a % d32;
        u += a / d64 + a % d64;
        sa = (long long)a; sd = (long long)(d64 ^ (a << 20));
        sd = sd ? sd : 3;
        s += sa / (long long)d32 + sa % (long long)d32;
        s += sa / sd + sa % sd;
    }
    return (long)((u ^ (unsigned long long)s) & 0x7FFFFFFF);
}
";

/// Each spill took the first slot whose holders' cells it did not overlap, in
/// the order the allocator spilled, at the cost of every home's interval
/// worked out again at every spill (`intervals homes`, 5% of `d_faces -O1`):
/// this kernel's frame was 48 bytes. The slots coloured once over the kept
/// allocation, the homes taken where each begins, share into 44 with the old
/// allocator; the greedy allocation the pin makes needs 60 (clang -O2
/// -march=i486 on this source: `subl $60, %esp`).
#[test]
fn test_slots_coloured_once_over_the_kept_allocation_make_a_smaller_frame() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-mabi=sysv", "-O2", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let frame: i64 = listing
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("sub esp, "))
        .and_then(|bytes| bytes.parse().ok())
        .unwrap_or_else(|| panic!("no frame in: {listing}"));
    assert!(
        frame <= 60,
        "the frame is {frame} bytes (60 allowed, the pin's; 48 when each spill coloured its own slot): {listing}"
    );
}
