//! A BYREF argument that is a plain variable is passed by its address, at every
//! target. At m32 the front end took a 4-byte pointer for a far one and bridged
//! each such argument through a near slot, copied in before the call and out
//! after it: `Hanoi` at -O2 ran 729,041 instructions to m16's 368,621.

use std::process::Command;

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("h.bas"),
        "DEFINT A-Z\nDECLARE FUNCTION Hanoi% (n AS INTEGER, a AS INTEGER, b AS INTEGER, c AS INTEGER)\n\
         PRINT Hanoi%(3, 1, 3, 2)\n\
         FUNCTION Hanoi% (n AS INTEGER, a AS INTEGER, b AS INTEGER, c AS INTEGER)\n\
         IF n = 0 THEN\nHanoi% = 0\nELSE\nHanoi% = Hanoi%(n - 1, a, c, b) + 1 + Hanoi%(n - 1, c, b, a)\nEND IF\nEND FUNCTION\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["h.bas", "--dialect", "qb45", "-fqb-runtime=llrm", "-O2", "-S", "-o", "h.s"])
        .args(flags)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("h.s")).unwrap()
}

/// The text of `HANOI`.
fn hanoi(listing: &str) -> &str {
    let from = listing.find("HANOI proc").expect("HANOI");
    &listing[from..from + listing[from..].find("HANOI endp").expect("its end")]
}

/// Stores of a word through a pointer the body loaded: the copy-out of an
/// argument into its variable.
fn writebacks(body: &str) -> usize {
    body.lines().filter(|line| line.trim_start().starts_with("mov word ptr [e") && line.contains("x], ")).count()
}

#[test]
fn test_at_m32_a_variable_passed_byref_is_not_copied_in_and_out() {
    let body = listing(&["-m32"]);
    assert_eq!(writebacks(hanoi(&body)), 0, "{}", hanoi(&body));
}
