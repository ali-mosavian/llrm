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

/// A latch with a second branch after its own (a copied loop test: `jle top;
/// inc ebx; jne again; jmp out`) was priced as falling into the header when the
/// header came next, though its last jump is to `out`: x_binsearch was turned
/// and entered through `jle; jmp`, 1% slower in clocks for no jump saved a
/// pass.
#[test]
fn test_a_latch_that_ends_in_a_jump_elsewhere_is_not_priced_as_falling_through() {
    let source =
        concat!(env!("CARGO_MANIFEST_DIR"), "/crates/target/llrm-x86-m32/vsgcc/kernels/x_binsearch/x_binsearch.c");
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let jumps = text.lines().filter(|line| line.trim().starts_with("jmp ")).count();
    assert!(jumps <= 2, "{jumps} jumps: {text}");
}

/// A turn was kept only for a third fewer jumps, counted alike whatever the
/// jump cost: x_adler's loop, whose `jb` over one arm and `jne` back are the
/// clocks of a taken `jcc` each, stayed as written (1.12 of gcc's clocks; 12%
/// for the turn).
#[test]
fn test_a_turn_that_saves_clocks_is_made_whatever_the_share_of_jumps() {
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/crates/target/llrm-x86-m32/vsgcc/kernels/x_adler/x_adler.c");
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let counters: Vec<&str> = lines.windows(2).filter(|pair| pair[0].starts_with("inc ")).map(|pair| pair[1]).collect();
    assert!(counters.iter().any(|next| next.starts_with("je ")), "the loop's test branches back: {counters:?}\n{text}");
}
