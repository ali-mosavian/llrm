//! Where the target has the P6 instructions, a select of two x87 values is
//! `fcmovCC` and a compare is `fcomi`; below it, the branch diamond and
//! `fnstsw; sahf`.

mod common;

/// `(unsigned long long)long_double`: a select of 2^63 or 0.0 that is
/// subtracted.
const SOURCE: &str = "unsigned long long f(long double d) { return (unsigned long long)d; }\n";

fn assembled_source(
    source: &str,
    args: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(args)
        .args(["-Os", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

fn assembled(args: &[&str]) -> String {
    assembled_source(SOURCE, args)
}

const COMPARE: &str = "int f(long double a, long double b) { return a < b; }\n";

/// x86-64 has `cmov`: the select of two long double constants was a compare, a
/// jump and a pop on one arm (double_to_uint64 -Os m64: +6 B over main).
#[test]
fn test_a_long_double_select_is_an_fcmov_where_the_target_has_cmov() {
    let text = assembled(&["--target=x86_64-linux"]);
    assert!(text.contains("fcmov"), "{text}");
}

/// The 486 has no `fcmov`: the diamond stays.
#[test]
fn test_a_long_double_select_is_a_branch_where_the_target_has_no_cmov() {
    let text = assembled(&["-m32", "-mabi=sysv", "-march=i486"]);
    assert!(!text.contains("fcmov"), "{text}");
}

/// x86-64 has `fcomi`: the compare went through `fnstsw ax; sahf`, three
/// instructions and the AX register for one.
#[test]
fn test_a_long_double_compare_is_fcomi_where_the_target_has_it() {
    let text = assembled_source(COMPARE, &["--target=x86_64-linux"]);
    assert!(text.contains("fcomi"), "{text}");
    assert!(!text.contains("sahf"), "{text}");
}

/// The 486 has no `fcomi`: the status word reaches the flags through AX.
#[test]
fn test_a_long_double_compare_is_the_status_word_where_the_target_has_no_fcomi() {
    let text = assembled_source(COMPARE, &["-m32", "-mabi=sysv", "-march=i486"]);
    assert!(text.contains("sahf") && !text.contains("fcomi"), "{text}");
}

/// A `long double` maximum was a compare, a jump and two returns: the MIR
/// select was branched before selection, so the DAG never saw `fcmov`.
#[test]
fn test_a_long_double_maximum_is_fcmov_at_m64() {
    let source = "long double f(long double a, long double b) { return a > b ? a : b; }\n";
    let text = assembled_source(source, &["--target=x86_64-linux"]);
    assert!(text.contains("fcmov") && !text.contains(" ja "), "{text}");
}
