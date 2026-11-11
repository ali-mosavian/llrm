use llrm_mir::datalayout::DataLayout;

use super::casts_branched;
use crate::testing::{parsed, printed};

/// The clamp of x_ldcmp: `(long)(x < -3 ? -3 : x > 4 ? 4 : x)`.
const CLAMP: &str = "define i32 @f(double %x) {
b0:
  %lo = fcmp olt double %x, -3.0
  %hi = fcmp ogt double %x, 4.0
  %up = select i1 %hi, double 4.0, double %x
  %s = select i1 %lo, double -3.0, double %up
  %r = fptosi double %s to i32
  ret i32 %r
}
";

fn after(
    text: &str,
    conversion: i64,
    branch: i64,
) -> (bool, String) {
    after_priced(text, conversion, 0, branch)
}

fn after_priced(
    text: &str,
    conversion: i64,
    compare: i64,
    branch: i64,
) -> (bool, String) {
    let mut module = parsed(text);
    let layout = DataLayout::default();
    let llrm_mir::module::Module { context, globals, .. } = &mut module;
    let llrm_mir::module::GlobalKind::Function(function) = &mut globals[0].kind else { panic!("a function") };
    let changed = casts_branched(context, &layout, function, conversion, compare, branch);
    (changed, printed(&module))
}

/// Both constants are converted here and the conversion of `%x` is made once,
/// where neither constant is taken (x_ldcmp -O2: 2020 `fistp` against gcc's
/// 1180).
#[test]
fn test_a_dear_conversion_of_a_clamp_is_made_on_the_path_that_takes_neither_constant() {
    let (changed, text) = after(CLAMP, 43, 3);
    assert!(changed, "{text}");
    assert_eq!(text.matches("fptosi").count(), 1, "{text}");
    assert!(text.contains("phi i32 [ -3, %b0 ]") || text.contains("phi i32 [-3, %b0]"), "{text}");
    assert!(text.contains("phi i32 [ 4,") || text.contains("[ 4, "), "{text}");
}

/// Control: where one instruction converts, the select stays.
#[test]
fn test_a_cheap_conversion_keeps_the_select() {
    let (changed, text) = after(CLAMP, 1, 3);
    assert!(!changed, "{text}");
    assert!(text.contains("select i1 %lo"), "{text}");
}

/// Control: a select of two values has no constant to convert at compile time.
#[test]
fn test_a_select_without_a_constant_arm_keeps_its_conversion_after_it() {
    let (changed, text) = after(
        "define i32 @f(double %x, double %y, i1 %c) {
b0:
  %s = select i1 %c, double %x, double %y
  %r = fptosi double %s to i32
  ret i32 %r
}
",
        43,
        3,
    );
    assert!(!changed, "{text}");
}

/// Control: a select the conversion does not own alone keeps its other reader.
#[test]
fn test_a_select_with_a_second_reader_is_left() {
    let (changed, text) = after(
        "define i32 @f(double %x, i1 %c, ptr %p) {
b0:
  %s = select i1 %c, double 4.0, double %x
  store double %s, ptr %p
  %r = fptosi double %s to i32
  ret i32 %r
}
",
        43,
        3,
    );
    assert!(!changed, "{text}");
}

/// `a < b ? -1 : (a > b)`: the second compare is made on the path that needs
/// it.
const CMP3: &str = "define i32 @f(double %a, double %b) {
b0:
  %lt = fcmp olt double %a, %b
  %gt = fcmp ogt double %a, %b
  %z = zext i1 %gt to i32
  %r = select i1 %lt, i32 -1, i32 %z
  ret i32 %r
}
";

#[test]
fn test_a_dear_compare_behind_a_select_with_a_constant_is_made_on_its_own_path() {
    let (changed, text) = after_priced(CMP3, 0, 9, 3);
    assert!(changed, "{text}");
    let branch = text.find("br i1 %lt").expect("a branch on the first compare");
    assert!(text.find("fcmp ogt").unwrap() > branch, "the second compare follows the branch: {text}");
    assert!(text.contains("phi i32 [ -1, %b0 ]"), "{text}");
    assert!(!text.contains("select"), "{text}");
}

/// Control: a compare that costs no more than the branches around it stays.
#[test]
fn test_a_cheap_compare_keeps_the_select() {
    let (changed, text) = after_priced(CMP3, 0, 2, 3);
    assert!(!changed, "{text}");
    assert!(text.contains("select i1 %lt"), "{text}");
}

/// Control: a compare something else reads is not sunk.
#[test]
fn test_a_compare_with_a_second_reader_is_left() {
    let (changed, text) = after_priced(
        "define i32 @f(double %a, double %b, ptr %p) {
b0:
  %lt = fcmp olt double %a, %b
  %gt = fcmp ogt double %a, %b
  %w = zext i1 %gt to i32
  store i32 %w, ptr %p
  %r = select i1 %lt, i32 -1, i32 %w
  ret i32 %r
}
",
        0,
        9,
        3,
    );
    assert!(!changed, "{text}");
}

/// `x > 65520 ? x - 65521 : x` computed the subtraction before the branch
/// every select makes where there is no conditional move: a copy and a
/// subtract on both paths (x_adler -O2 +7.5k clocks over gcc).
#[test]
fn test_a_select_arm_only_the_select_reads_is_made_on_its_own_path() {
    let mut module = parsed(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp ugt i32 %x, 65520
  %d = add i32 %x, -65521
  %r = select i1 %c, i32 %d, i32 %x
  ret i32 %r
}
",
    );
    let llrm_mir::module::Module { context, globals, .. } = &mut module;
    let llrm_mir::module::GlobalKind::Function(function) = &mut globals[0].kind else { panic!("a function") };
    assert!(super::select_arms_sunk(context, function, &|_, _| false));
    let text = printed(&module);
    assert!(!text.contains("select"), "{text}");
    let (before, after) = text.split_once("add i32").expect("the subtraction");
    assert!(before.contains("br i1 %c"), "the branch comes first: {text}");
    assert!(after.contains("phi i32"), "{text}");
}

/// Control: an arm another instruction reads stays before the branch.
#[test]
fn test_a_select_arm_with_a_second_reader_stays() {
    let mut module = parsed(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp ugt i32 %x, 65520
  %d = add i32 %x, -65521
  %r = select i1 %c, i32 %d, i32 %x
  %t = add i32 %r, %d
  ret i32 %t
}
",
    );
    let llrm_mir::module::Module { context, globals, .. } = &mut module;
    let llrm_mir::module::GlobalKind::Function(function) = &mut globals[0].kind else { panic!("a function") };
    assert!(!super::select_arms_sunk(context, function, &|_, _| false));
}

/// `x & 1 ? 3 * x + 1 : x >> 1`: the multiply moved with the add, or the
/// selector had `lea [x + x*2]; inc` where it folds one `lea [x + x*2 + 1]`
/// (x_collatz -O3 +5.9k clocks).
#[test]
fn test_a_select_arm_moves_with_the_instructions_only_it_reads() {
    let mut module = parsed(
        "define i32 @f(i32 %x) {
b0:
  %b = and i32 %x, 1
  %c = icmp ne i32 %b, 0
  %m = mul i32 %x, 3
  %d = add i32 %m, 1
  %h = lshr i32 %x, 1
  %r = select i1 %c, i32 %d, i32 %h
  ret i32 %r
}
",
    );
    let llrm_mir::module::Module { context, globals, .. } = &mut module;
    let llrm_mir::module::GlobalKind::Function(function) = &mut globals[0].kind else { panic!("a function") };
    assert!(super::select_arms_sunk(context, function, &|_, _| false));
    let text = printed(&module);
    let branch = text.find("br i1 %c").expect("the branch");
    for what in ["mul i32", "add i32 %m", "lshr i32"] {
        assert!(text.find(what).expect(what) > branch, "{what} after the branch: {text}");
    }
    // The true arm's block follows the branch, as the pin's lowered select places it
    // (x_collatz -O3 +1196 clocks with the false arm first).
    assert!(text.find("add i32 %m").unwrap() < text.find("lshr i32").unwrap(), "{text}");
}

/// x_fclip -O2 m32: InstCombine leaves the outer select of a clamp on the
/// converted value, `select hi, 32767, fptosi (select lo, -32768.0, x)`; the
/// conversion ran on the path that takes the upper limit too (753 of 2560
/// samples, a `fist` and two control word loads each).
#[test]
fn test_a_clamp_whose_outer_select_is_on_the_converted_value_converts_where_neither_limit_is_taken() {
    let (changed, text) = after(
        "define i32 @f(double %x) {
b0:
  %hi = fcmp ogt double %x, 32767.0
  %lo = fcmp olt double %x, -32768.0
  %in = select i1 %lo, double -32768.0, double %x
  %c = fptosi double %in to i32
  %r = select i1 %hi, i32 32767, i32 %c
  ret i32 %r
}
",
        43,
        3,
    );
    assert!(changed, "{text}");
    let first = text.find("br i1 %hi").unwrap_or_else(|| panic!("a branch on the upper limit: {text}"));
    let convert = text.find("fptosi").unwrap_or_else(|| panic!("a conversion: {text}"));
    let lower = text.find("br i1 %lo").unwrap_or_else(|| panic!("a branch on the lower limit: {text}"));
    assert!(first < lower && lower < convert, "the conversion follows both tests: {text}");
    assert_eq!(text.matches("fptosi").count(), 1, "{text}");
}
