//! A read that leaves a value fewer than four registers, in a block less deep
//! in loops than the value's other references, reads a copy of its own
//! (`InstrEmitter.cpp` `MinRCSize`, `constrain::distinct_classes`).

use std::process::Command;

fn kernel(
    source: &std::path::Path,
    function: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .arg(source)
        .args(["--dialect", "qb45", "--runtime", "qb45", "-m16", "-S", "-O2", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let start = text.find(&format!("\n{function} proc")).expect(function);
    text[start..].split(&format!("{function} endp")).next().unwrap().to_owned()
}

/// The loop counter was also the operand of one `lea` for the loop bound,
/// before the loop. That read confined the counter to bx/si/di for its whole
/// life: it took `si`, and the three array bases competed for `di`, reloaded
/// from the frame on every trip (3 reloads a trip, `dec si`).
fn counter_of(body: &str) -> String {
    body.lines().find(|line| line.trim_start().starts_with("dec ")).expect("a counted loop").trim().to_owned()
}

#[test]
fn test_sum_three_counts_in_a_register_the_addresses_do_not_need() {
    let body = kernel(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/parity/sum_three/sum_three.bas"),
        "SUMTHREE",
    );
    let counter = counter_of(&body);
    assert!(counter == "dec cx" || counter == "dec dx", "{counter}\n{body}");
}

#[test]
fn test_a_counter_summed_over_two_arrays_counts_outside_the_address_registers() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("two.bas");
    std::fs::write(
        &source,
        "DEFINT A-Z\nDECLARE FUNCTION Dot% (a() AS INTEGER, b() AS INTEGER)\nDIM SHARED x(0 TO 3) AS INTEGER\nDIM SHARED y(0 TO 3) AS INTEGER\nPRINT Dot%(x(), y())\nEND\n\nFUNCTION Dot% (a() AS INTEGER, b() AS INTEGER) STATIC\n    DIM i AS INTEGER, t AS INTEGER\n    t = 0\n    FOR i = LBOUND(a) TO UBOUND(a)\n        t = t + a(i) - b(i)\n    NEXT i\n    Dot% = t\nEND FUNCTION\n",
    )
    .unwrap();
    let body = kernel(&source, "DOT");
    let counter = counter_of(&body);
    assert!(counter != "dec si" && counter != "dec di", "{counter}\n{body}");
}
