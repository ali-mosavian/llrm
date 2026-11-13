//! m32 -O1 (INL-O1): a growing inline stays only where the call's clocks pay
//! for its bytes at the O1 rate. The pin's threshold copied every soft-float
//! helper into each site: sfm16_dbg 24,284 B against 23,372 B priced. Needs the
//! Open Watcom tree (tools/ow-fetch.sh) and the m16 clang (`LLRM_CLANG_M16`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compile(
    name: &str,
    source: &str,
    flag: &str,
) -> (String, u64) {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inlo1-{name}-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), source).unwrap();
    let run = |mode: &str, out: &str| {
        let result = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
            )
            .args([flag, "-w", mode, "-march=i486", "-mabi=sysv", "-o"])
            .arg(dir.join(out))
            .arg(dir.join("d.c"))
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    };
    run("-S", "d.s");
    run("-c", "d.o");
    (std::fs::read_to_string(dir.join("d.s")).unwrap(), std::fs::metadata(dir.join("d.o")).unwrap().len())
}

#[test]
fn test_m32_o1_straight_line_helpers_are_priced_not_all_copied() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source = std::fs::read_to_string(root.join("bench/sfm16_dbg/sfm16_dbg.c")).unwrap();
    let (_, size) = compile("soft", &source, "-O1");
    assert!(size < 23_800, "object is {size} B; copying every helper into each site gave 24,284, pricing them 23,372");
}

/// The inliner's growth is in size units, one per instruction, and a callee's
/// bytes are 3.5 to 5 times that: compared with a per-byte rate it let
/// `sf_cmp_bits` (about 70 B) into each of eight comparison wrappers
/// (sfm16_float 23,432 B against gcc's smaller object and 22,748 B with the
/// unit scaled).
#[test]
fn test_m32_o1_growth_is_priced_in_bytes_not_size_units() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source = std::fs::read_to_string(root.join("bench/sfm16_float/sfm16_float.c")).unwrap();
    let (_, size) = compile("units", &source, "-O1");
    assert!(size < 23_000, "object is {size} B; the unscaled growth gave 23,432");
}

/// The bottom-up walk priced `sf32_pack` in `mul_core` at the site's own
/// frequency (1.0), though the soft-float multiply runs in a loop of the
/// kernel 30 times per call: it stayed a call and x_sf_mul ran 88263 clk
/// against 82587 with it inlined (gcc/clang 88107). The check weighs a site by
/// how often its function runs, up the call graph.
#[test]
fn test_m32_o1_inline_weighs_how_often_the_caller_runs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source =
        std::fs::read_to_string(root.join("crates/target/llrm-x86-m32/vsgcc/kernels/x_sf_mul/x_sf_mul.c")).unwrap();
    let (listing, _) = compile("entry", &source, "-O1");
    let calls = listing.lines().filter(|line| line.contains("call") && line.contains("sf32_pack")).count();
    assert!(calls <= 2, "{calls} calls of sf32_pack: the hot caller's copies were not made (3 without the weight)");
}

/// m32 -O2 (INL-O2 RATE): a growing inline stays where the call sequence's
/// clocks, at the site's run count, pay for the bytes at the 486's rate. The
/// call was priced as one instruction (4 clocks of the 15 it costs, with its
/// pushes, `ret` and `add esp`) and a helper called from a hot loop up the
/// graph kept its call: x_sf_add -O2 object: 22,860 B before, 22,572 B priced.
#[test]
fn test_m32_o2_inline_prices_the_call_sequence_and_the_caller_s_run_count() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source =
        std::fs::read_to_string(root.join("crates/target/llrm-x86-m32/vsgcc/kernels/x_sf_add/x_sf_add.c")).unwrap();
    let (_, size) = compile("o2rate", &source, "-O2");
    assert!(size < 22_700, "object is {size} B; the call priced as one instruction gave 22,860");
}

/// INL-O1 SIZE: the inliner counts a callee in bytes of encoded code. A refill
/// reader of many absolute-address accesses and a variable-length copy is about
/// 190 B; counted as an instruction each (24 units, 84 B at the old 3.5 scale)
/// it was copied into every site of a loop (x_jpeg_single -O1: `next` eight
/// times, +1533 B and +0.5 M clk over keeping it a call). A one-instruction
/// callee still inlines.
#[test]
fn test_m32_o1_inline_counts_a_callee_in_encoded_bytes() {
    let source = r#"
#include <string.h>
static unsigned char inbuf[512], store[4000];
static int inpos, inlen, bad, at, total, refills, last, sum1, sum2;
static int next(void)
{
    if (inpos == inlen) {
        int n = 4000 - at;
        if (n > 511) n = 511;
        memcpy(inbuf, store + at, n + 1);
        at += n + 1;
        inlen = n + 1;
        inpos = 0;
        total += inlen;
        refills++;
        last = inbuf[0];
        sum1 += inbuf[1];
        sum2 ^= inbuf[2];
        if (inlen <= 0) { inlen = 0; bad = 1; return 0; }
    }
    return inbuf[inpos++];
}
static int add1(int x) { return x + 1; }
int caller(int count)
{
    int sum = 0;
    for (int i = 0; i < count; i++)
        sum += next() + next() + next() + next() + add1(i);
    return sum;
}
"#;
    let (listing, _) = compile("bytes", source, "-O1");
    let calls_of = |name: &str| listing.lines().filter(|line| line.contains("call") && line.contains(name)).count();
    assert!(calls_of("next") >= 4, "`next` is copied into its sites: {} calls left of 4", calls_of("next"));
    assert_eq!(calls_of("add1"), 0, "a one-instruction callee stays a call");
}

/// The analyzer prices phis, jumps and returns at nothing, as the pin does,
/// so a loop whose ten float values rotate through phis looked like 31 B and
/// was inlined (x87rotate -O2: 344 B -> 623 B, the callee being external and
/// kept). The copies are real bytes: it stays a call.
#[test]
fn test_m32_inline_counts_the_phi_copies_of_a_loop_callee() {
    let source = r#"
extern void report(long value);
long rot(short n, float seed)
{
    float a0 = seed, a1 = seed + 1, a2 = seed + 2, a3 = seed + 3, a4 = seed + 4;
    float a5 = seed + 5, a6 = seed + 6, a7 = seed + 7, a8 = seed + 8, a9 = seed + 9;
    float t;
    short i;
    for (i = 0; i < n; ++i) {
        t = a0;
        a0 = a1; a1 = a2; a2 = a3; a3 = a4; a4 = a5;
        a5 = a6; a6 = a7; a7 = a8; a8 = a9; a9 = t;
        a0 = a0 + a9 * 0.5f;
    }
    return (long)(a0 + 2 * a1 + 3 * a2 + 4 * a3 + 5 * a4 + 6 * a5 + 7 * a6 + 8 * a7 + 9 * a8 + 10 * a9);
}
static volatile short trips = 7;
int main(void) { report(rot(trips, 1.0f)); return 0; }
"#;
    let (listing, _) = compile("phis", source, "-O2");
    let calls = listing.lines().filter(|line| line.contains("call") && line.contains("rot")).count();
    assert_eq!(calls, 1, "a loop callee of ten rotating values stays a call:\n{listing}");
}

/// A callee priced at more bytes than its call is refused at -O1, but a site
/// whose constant arguments fold the whole loop away is smaller inlined
/// (split_loops -O1: 206 B kept as calls, 271 B with the refusal). The
/// refused site is tried and kept where the caller comes out smaller.
#[test]
fn test_m32_o1_inline_keeps_a_refused_site_that_folds_away() {
    let source = r#"
extern void report(long value);
static long below(short n, short k)
{
    long sum = 0;
    short i;
    for (i = 0; i < n; i++)
        sum += i < k ? i * 3 : i + 7;
    return sum;
}
static long from(short n, short k)
{
    long sum = 0;
    short i;
    for (i = 0; i < n; i++)
        sum += i >= k ? i * 5 : 1;
    return sum;
}
int main(void)
{
    report(below(10, 4));
    report(below(10, 0));
    report(below(10, 20));
    report(below(10, -3));
    report(from(10, 4));
    report(from(10, 0));
    report(from(10, 20));
    report(from(0, 3));
    return 0;
}
"#;
    let (listing, _) = compile("folds", source, "-O1");
    let calls = listing
        .lines()
        .filter(|line| line.contains("call") && (line.contains("below") || line.contains("from")))
        .count();
    assert_eq!(calls, 0, "constant arguments fold the loops: the sites are inlined, not called:\n{listing}");
}

/// A callee refused before the folds that empty it is priced again after
/// them (`second_inline`): `bar` stores to a global nothing reads once `main`
/// has folded the sum, so it ends as a lone `ret` and `foo` drops the call
/// (byvalue_struct_address -O1: 18 B with the call dropped, 41 B without).
#[test]
fn test_m32_o1_inline_drops_the_call_to_a_callee_the_folds_emptied() {
    let source = r#"
extern void report(long value);
struct baz { int a, b, c, d, e; };
static int sum;
void bar(struct baz *x, int f, int g, int h, int i, int j)
{
    sum = x->a * 1 + x->b * 10 + x->c * 100 + x->d * 1000 + x->e * 10000 + f + g + h + i + j;
}
void foo(char *z, struct baz x, char *y)
{
    bar(&x, 6, 7, 8, 9, 10);
}
int main(void)
{
    struct baz x = { 1, 2, 3, 4, 5 };
    foo((char *)0, x, (char *)0);
    report(sum);
    return 0;
}
"#;
    let (listing, _) = compile("emptied", source, "-O1");
    let calls = listing.lines().filter(|line| line.contains("call") && line.contains("bar")).count();
    assert_eq!(calls, 0, "`bar` is empty after the folds: its call is gone:\n{listing}");
}

/// A size caller keeps the unit model its -Os bound was fitted in: priced in
/// bytes, `to_double` (16 B against a 12 B call) stayed a call although the
/// pin's cost was -30, `classify` stayed over the -Os threshold, and the
/// program was 317 B instead of 60 (uint64_to_float -Os).
#[test]
fn test_m32_os_inline_keeps_the_units_its_bound_was_fitted_in() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source = std::fs::read_to_string(root.join("tests/run/c/uint64_to_float.c")).unwrap();
    let (listing, _) = compile("yields", &source, "-Os");
    let calls = listing.lines().filter(|line| line.contains("call") && !line.contains("report")).count();
    assert_eq!(calls, 0, "the conversions fold at their constant calls and `classify` follows:\n{listing}");
}
