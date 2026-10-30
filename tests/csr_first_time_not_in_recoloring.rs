//! A callee-saved register's first use is not pre-split while recoloring.

mod common;

const SOURCE: &str = r#"static long power(long b, int e)
{
    if (e == 0) return 1;
    if (e & 1) return b * power(b, e - 1) % 30011;
    return power(b * b % 30011, e >> 1);
}
long bench_recpow(int n)
{
    long total = 0;
    int i;
    for (i = 1; i <= n; ++i) total += power(i % 97 + 2, i + 40) % 1000;
    return total;
}
"#;

/// recpow's `power` at -O1 m32 with the old selector: the first use of EBX
/// pre-split a range inside last-chance recoloring, `tryRecoloringCandidates`
/// took the new vregs for a failed recolor, and the compile stopped with "the
/// greedy allocator could not place every range".
#[test]
fn a_recolored_range_is_not_pre_split_for_its_first_callee_saved_register() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O1", "--isel=old", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
