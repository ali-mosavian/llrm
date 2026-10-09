//! A loop whose body is a diamond is laid out so its latch falls into the
//! header.

use std::process::Command;

const KERNEL: &str = "
long steps(unsigned x)
{
    long n = 0;
    while (x != 1) { x = (x & 1) ? 3 * x + 1 : x >> 1; ++n; }
    return n;
}
";

/// The pass took two jumps round the loop, one over or into an arm and a
/// conditional one back to the header: collatz's x_collatz ran 1.19 of gcc's
/// clocks for the same instructions. gcc's `rotate_loop` leaves one, and the
/// test of the count leaves the loop instead of repeating it.
#[test]
fn test_the_latch_of_a_loop_with_a_diamond_body_falls_into_the_header() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let latches: Vec<&str> = lines
        .windows(4)
        .filter(|four| four[0].starts_with("inc") && four[1].ends_with(':') && four[2].starts_with("cmp"))
        .map(|four| four[3])
        .collect();
    assert!(!latches.is_empty(), "no latch found: {text}");
    assert!(
        latches.iter().all(|next| next.starts_with("je ")),
        "the test of x != 1 branches back: {latches:?}\n{text}"
    );
}
