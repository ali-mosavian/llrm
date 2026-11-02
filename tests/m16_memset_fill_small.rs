//! A small constant memset of a local at real mode: tuned for size the bytes
//! decide (the fill is taken when it is smaller), at speed the bytes it saves
//! at the target's rate must pay for the clocks it adds.

mod common;

fn listing(flags: &[&str], length: u32) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    let source = format!("void use(char *);\nvoid f(void) {{ char b[{length}] = {{0}}; use(b); }}\n");
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_c().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// 12 bytes at -Os were three dword stores (18 bytes with the `66` prefixes
/// and the setup), kept by a fitted 10_000 milliclocks/B rate against the
/// 16-byte `rep stosb` fill.
#[test]
fn test_a_12_byte_clear_at_size_is_the_smaller_fill() {
    let text = listing(&["-Os"], 12);
    assert!(text.contains("rep stosb"), "{text}");
}

/// At speed the fill's extra clocks outweigh its 2 bytes at 500 milliclocks/B.
#[test]
fn test_a_12_byte_clear_at_speed_stays_stores() {
    let text = listing(&["-O2"], 12);
    assert!(!text.contains("rep stosb"), "{text}");
}
