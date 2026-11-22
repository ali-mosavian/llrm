//! A tree of float compares joined by `and`/`or` was an `fcmp`, a `cset` per
//! compare and an `ands` (`emitConjunction` refused a floating-point leaf).
//! The pin chains `fccmp`: QuickrBasic operands.c -Os, +12 B in each
//! float-to-integer conversion with a range check.

fn assembly() -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/fp_conditional_compare.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

/// The lines of `name`, up to the next function label.
fn function(
    asm: &str,
    name: &str,
) -> String {
    let label = format!("{name}:");
    let is_function = |line: &&str| line.ends_with(':') && !line.starts_with('.');
    let lines: Vec<&str> = asm.lines().map(str::trim).skip_while(|line| *line != label).collect();
    assert!(!lines.is_empty(), "no {name}:\n{asm}");
    lines[1..].iter().take_while(|line| !is_function(line)).map(|line| format!("{line}\n")).collect()
}

#[test]
fn a_range_check_of_a_float_is_an_fcmp_and_an_fccmp() {
    let asm = assembly();
    let body = function(&asm, "in_range");
    assert_eq!(body.matches("fccmp").count(), 1, "{body}");
    assert!(!body.contains("csinc"), "{body}");
}

#[test]
fn a_condition_of_two_flag_tests_is_two_chained_fccmps() {
    let asm = assembly();
    let body = function(&asm, "different");
    assert_eq!(body.matches("fccmp").count(), 2, "{body}");
    assert!(!body.contains("csinc"), "{body}");
}

#[test]
fn an_or_of_float_compares_is_a_negated_fccmp_chain() {
    let asm = assembly();
    let body = function(&asm, "either");
    assert_eq!(body.matches("fccmp").count(), 1, "{body}");
    assert!(!body.contains("csinc"), "{body}");
}
