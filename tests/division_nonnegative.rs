//! A division by a constant of a value that is never negative.

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_cc().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

const LOOP: &str = "int f(int n) { int i, s = 0; for (i = 0; i < n; ++i) s += (i * 97) % 211; return s; }\n";

/// `s += (i * 97) % 211` over `i = 0..n` was `cdq; idiv`, 43 clocks: signed, as
/// `i * 97` was not known to be positive. gcc multiplies by the reciprocal.
/// (bench x_hash: 1.38x of gcc's clocks.)
#[test]
fn a_remainder_of_a_counter_s_product_is_a_multiply_not_a_division() {
    let text = listing(LOOP, &["-m32", "-mabi=sysv", "-O2", "-march=i486"]);
    assert!(!text.contains("idiv") && !text.contains("div "), "{text}");
}

/// The instructions of the loop in `text`: from the target of the last jump
/// back to its label, to that jump.
fn loop_body(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let (at, target) = lines
        .iter()
        .enumerate()
        .rev()
        .find_map(|(at, line)| {
            let label = line.strip_prefix('j')?.split_whitespace().nth(1)?;
            lines[..at].contains(&format!("{label}:").as_str()).then_some((at, label))
        })
        .expect("a loop");
    let start = lines.iter().position(|line| *line == format!("{target}:")).unwrap();
    lines[start..=at].to_vec()
}

/// Tuned for size the loop divides once a trip: the exit value `s` had after
/// the last trip was expanded in the loop too, a second `div` (35 instructions
/// against 23, a spill). Whether the division is `idiv` or `div` is not the
/// contract: the pin multiplies by the reciprocal at -Os (`isIntDivCheap`), and
/// divides only at -Oz.
#[test]
fn tuned_for_size_the_loop_divides_once_a_trip() {
    let text = listing(LOOP, &["-m32", "-mabi=sysv", "-Os", "-march=i486"]);
    let divisions =
        loop_body(&text).iter().filter(|line| line.starts_with("div ") || line.starts_with("idiv ")).count();
    assert_eq!(divisions, 1, "{text}");
}
