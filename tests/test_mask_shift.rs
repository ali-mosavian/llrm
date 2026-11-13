//! A zero test of the low or high bits of a 32-bit value is a shift (or `add
//! r, r`) whose flags are the answer, not a `test` with a 32-bit immediate.

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_c().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// After #7837 `(x << 6) == 0` is `(x & 0x3ffffff) == 0` at the selector and
/// came out as `test eax, 67108863` (6 bytes) where `shl eax, 6` is 3; the
/// f32/f64 soft-float runtime grew 3 bytes per site (f32 trunc, cmp).
#[test]
fn a_zero_test_of_a_masked_i32_is_a_shift() {
    let source = "int low(unsigned a, unsigned b) { return ((a | b) & 0x3ffffff) == 0; }\n\
                  int top(unsigned a, unsigned b) { return ((a | b) & 0x7fffffff) == 0; }\n";
    for flags in [&["--target=x86_32-linux", "-march=i486", "-O2"][..], &["--target=x86_64-linux", "-O2"][..]] {
        let text = listing(source, flags);
        assert!(!text.contains("67108863") && !text.contains("2147483647"), "{flags:?}: {text}");
        assert!(text.contains("shl e") && text.contains(", 6\n"), "{flags:?}: {text}");
        assert!(
            text.lines().any(|l| l.trim().starts_with("add e")
                && l.trim()
                    .split_once(" ")
                    .map(|(_, r)| r.split(", ").collect::<Vec<_>>())
                    .is_some_and(|v| v[0] == v[1])),
            "{text}"
        );
    }
}
