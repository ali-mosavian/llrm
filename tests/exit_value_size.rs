//! At -Os a loop is not replaced by its closed form when the formula is more
//! code than the loop.

use std::process::Command;

/// A sum the loop optimizer writes in closed form, widened to i64 for the
/// product: 35 B as a loop on main, 107 B as the formula.
const SUM: &str = r#"long b(unsigned short n)
{
    long s = 0;
    unsigned short i;
    for (i = 0; i < n && i < 10; i++) s += i;
    return s;
}
"#;

fn listing(level: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SUM).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-m16", "-march=i386", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// The closed form (a 32-bit `imul` and a `shrd` by one) took 107 B where the
/// loop is under half of that.
#[test]
fn at_os_the_sum_stays_a_loop() {
    let text = listing("-Os");
    assert!(!text.contains("shrd"), "closed form at -Os:\n{text}");
}

#[test]
fn at_o2_the_sum_is_still_the_closed_form() {
    let text = listing("-O2");
    assert!(text.contains("shrd"), "no closed form at -O2:\n{text}");
}
