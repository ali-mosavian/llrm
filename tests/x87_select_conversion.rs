//! A conversion of a clamp is made on the path that takes neither limit.

mod common;

/// `(long)(x < -3 ? -3 : x > 4 ? 4 : x)`: one `fistp` after the clamp selects
/// made it 2020 conversions in x_ldcmp -O2 where gcc made 1180 (249830 clocks
/// against 204830).
const SOURCE: &str = "int f(double x) { return (int)(x < -3 ? -3 : (x > 4 ? 4 : x)); }\n";

fn assembled(source: &str) -> String {
    assembled_for(source, &["--target=x86_32-wat", "-m32", "-mabi=sysv", "-march=i486"])
}

fn assembled_for(
    source: &str,
    target: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(target)
        .args(["-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

#[test]
fn test_a_clamp_converted_to_an_integer_converts_on_the_middle_path_only() {
    let text = assembled(SOURCE);
    assert!(text.contains("mov ecx, -3") || text.contains(", -3"), "{text}");
    let at = text.find("fistp").expect("the middle path converts");
    let before = &text[..at];
    assert!(before.contains("j"), "a branch precedes the conversion: {text}");
    assert_eq!(text.matches("fistp").count(), 1, "{text}");
    // Neither limit's path loads the control word.
    assert!(!text[..text.find("-3").unwrap()].contains("fldcw"), "{text}");
}

/// Control: a select of two variables has no constant to convert at compile
/// time, and keeps its one conversion after the select.
#[test]
fn test_a_select_of_two_variables_keeps_one_conversion_after_it() {
    let text = assembled("int f(double x, double y, int c) { return (int)(c ? x : y); }\n");
    assert_eq!(text.matches("fistp").count(), 1, "{text}");
}

/// Control: where one `cvttsd2si` converts (x86-64 SSE) the select stays and
/// the limits are not converted on their own paths.
#[test]
fn test_a_cheap_conversion_on_x86_64_keeps_the_clamp_a_select() {
    let text = assembled_for(SOURCE, &["--target=x86_64-linux"]);
    assert_eq!(text.matches("cvttsd2si").count(), 1, "{text}");
    assert!(!text.contains(", -3\n"), "{text}");
}

/// `a < b ? -1 : (a > b)`: the second compare (`fcom; fnstsw; sahf`, 9 clocks
/// at 486) was made before the branch on the first, on every call
/// (x_ldcmp -O2: 213070 clocks against gcc's 204830).
#[test]
fn test_the_second_compare_of_a_three_way_compare_follows_the_branch_on_the_first() {
    let text = assembled("int f(double a, double b) { return a < b ? -1 : (a > b); }\n");
    let first = text.find("fnstsw").expect("a compare");
    let second = text[first + 1..].find("fnstsw").map(|at| at + first + 1).expect("a second compare");
    let between = &text[first..second];
    assert!(between.contains("\n    j"), "a branch separates the compares: {text}");
}
