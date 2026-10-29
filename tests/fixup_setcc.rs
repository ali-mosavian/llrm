//! A comparison used as a value is `xor r,r; cmp; setcc r8`, as `X86FixupSetCC`
//! makes it, not `cmp; setcc r8; movzx r,r8`.

mod common;

fn asm(
    source: &str,
    level: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args([level, "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// `a < b` as an int was `cmp; setl al; movzx eax,al`: a partial-register write
/// and an instruction more than the pin's `xor eax,eax; cmp; setl al`.
#[test]
fn test_a_compare_as_a_value_zeroes_its_register_before_the_compare() {
    let text = asm("int f(int a, int b) { return a < b; }\n", "-O2");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let cmp = lines.iter().position(|one| one.starts_with("cmp ")).expect("a cmp");
    assert!(lines[..cmp].contains(&"xor eax, eax"), "{text}");
    assert!(lines.contains(&"setl al"), "{text}");
    assert!(!lines.iter().any(|one| one.starts_with("movzx")), "{text}");
}

/// -O0 runs none of the machine phases that only make code better.
#[test]
fn test_o0_keeps_the_setcc_and_the_movzx() {
    let text = asm("int f(int a, int b) { return a < b; }\n", "-O0");
    assert!(text.lines().any(|one| one.trim().starts_with("movzx")), "{text}");
}
