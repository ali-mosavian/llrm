//! At `-Os` a constant a loop only reads once is made at its use, not held in
//! a callee-saved register for the whole loop.

use std::process::Command;

fn listing(level: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), SOURCE).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(directory)
        .args(["-m64", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// Three divisors the loop reads once each (the pin hoists them: they are
/// rematerializable) and five values it carries.
const SOURCE: &str = "static long mix(long a, long b, long c, long d, long e, int n) {
    for (; n; n--) {
        long t = b + 1; b = c + a % 5; c = d ^ t; d = e + c % 3; e = a + d % 7; a = t;
    }
    return a + b + c + d + e;
}
long f(int n) { return mix(1, 2, 3, 4, 5, n); }
";

/// recmany -Os held 5, 3 and 7 in `rbx`, `rbp` and `r14`: three pushes and
/// pops (8 B with REX) against `mov r, 5` at the use, 296 B against main's 273.
#[test]
fn a_constant_read_once_in_a_loop_takes_no_callee_saved_register_at_os() {
    let text = listing("-Os");
    let pushes: Vec<&str> = text.lines().map(str::trim).filter(|line| line.starts_with("push ")).collect();
    assert!(pushes.len() <= 1, "{pushes:?}\n{text}");
}
