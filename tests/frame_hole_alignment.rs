//! The frame laid again for a spill hole (`relayout`) is the frame the backend
//! makes with that hole (`LLRM_CHECK_FRAME=1`).

mod common;

/// Nine shared spill slots end on a word (30 bytes): the hole was not a
/// multiple of the 4-byte frame alignment, so the frame moved by it put the
/// long array at 2 mod 4 and differed from the backend run again by 2 bytes
/// ("reserve 286 against 288", bench/matmul at -O2).
const SOURCE: &str = "long f(short *a, short *b)
{
    long c[64];
    unsigned short i, k;
    for (i = 0; i < 8; ++i) {
        long total = 0;
        for (k = 0; k < 8; ++k) total += (long)a[k] * b[k * 8 + i];
        c[i] = total;
    }
    return c[5];
}
";

#[test]
fn a_frame_laid_again_equals_the_backend_run_again() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let done = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["-m32", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"])
        .env("LLRM_CHECK_FRAME", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&done.stderr);
    assert!(done.status.success(), "{stderr}");
}
