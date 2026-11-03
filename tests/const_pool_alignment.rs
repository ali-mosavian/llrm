//! Every constant-pool entry sits on its own alignment.

mod common;

/// The 10-byte extended constant `0.01L` was followed by dword constants at
/// offset 18, 2 mod 4: three of them in x_ld_poly's loop, 769 misaligned reads
/// (2,307 of 36,092 clocks at 3 each) that gcc's 12-byte slot does not have.
const SOURCE: &str = "long double f(long double x) { return (x * 0.01L + 1.5L) * x + 2.25L; }\n";

/// (offset, byte count) of each `$K` datum in the first `.data` section of the
/// listing.
fn pool(text: &str) -> Vec<(usize, usize)> {
    let mut at: usize = 0;
    let mut found = Vec::new();
    for line in text.lines().skip_while(|line| *line != ".data").skip(1).take_while(|line| *line != ".data?") {
        let line = line.trim();
        if let Some(to) = line.strip_prefix("align ") {
            let to: usize = to.parse().unwrap();
            at = at.next_multiple_of(to);
        } else if let Some(bytes) = line.strip_prefix("db ") {
            let count = bytes.split(',').count();
            found.push((at, count));
            at += count;
        }
    }
    found
}

#[test]
fn test_a_dword_constant_after_an_extended_one_is_on_a_dword_boundary() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args([
            "--target=x86_32-wat",
            "-m32",
            "-mabi=sysv",
            "-mlong-double-80",
            "-march=i486",
            "-O2",
            "-S",
            "-o",
            "a.s",
            "a.c",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let entries = pool(&text);
    assert!(entries.iter().any(|&(_, count)| count == 10) && entries.iter().any(|&(_, count)| count == 4), "{text}");
    for (at, count) in entries {
        assert_eq!(at % count.min(4), 0, "a {count}-byte constant at offset {at}: {text}");
    }
}

/// Four-byte constants that already sit on a dword boundary were each given an
/// `align 4`, which raised every such program's data segment from word to dword
/// and grew nine linked m16 programs by 2 bytes at -Os (float_bits, floats,
/// fpbench: 1,852 -> 1,854).
#[test]
fn test_dword_constants_on_their_boundary_ask_for_no_alignment() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), "float f(float x) { return x * 0.3f + 1.5f; }\n").unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_32-wat", "-m32", "-mabi=sysv", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.matches("db ").count() >= 2, "{text}");
    assert!(!text.lines().skip_while(|line| *line != ".data").take_while(|line| *line != ".data?").any(|line| line.trim() == "align 4"), "{text}");
}
