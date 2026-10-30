//! Each body is MIR text run by llrm-mir's interpreter before and after, so a
//! wrong conversion shows as a wrong answer; what is asserted of the text is
//! whether the float operations are gone.

use llrm_analysis::testing::DOS;

use super::Float2Int;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn converted(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, Float2Int);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

fn floats(text: &str) -> usize {
    ["fadd", "fsub", "fmul", "fneg", "fcmp", "sitofp", "uitofp", "fptosi", "fptoui"]
        .iter()
        .map(|op| text.matches(&format!("= {op} ")).count())
        .sum()
}

const ADD_ONE: &str = "define i32 @f(i8 %a) {
b0:
  %x = sitofp i8 %a to float
  %y = fadd float %x, 1.0
  %z = fptosi float %y to i32
  ret i32 %z
}
";

const INPUTS: &[&[i128]] = &[&[0], &[1], &[-1], &[127], &[-128], &[100]];

/// `(int)((float)a + 1.0f)` for `a` of `i8` is an `add` on the smallest legal
/// integer that holds `[-127, 129)`, and no float.
#[test]
fn an_add_of_a_converted_integer_is_done_on_integers() {
    let after = converted(ADD_ONE, INPUTS);
    assert_eq!(floats(&after), 0, "{after}");
    assert!(after.contains("add i16"), "{after}");
}

/// The comparison of two converted integers is an `icmp` of the predicate's
/// integer counterpart.
#[test]
fn a_compare_of_converted_integers_is_an_icmp() {
    let text = "define i1 @f(i8 %a, i8 %b) {
b0:
  %x = sitofp i8 %a to float
  %y = sitofp i8 %b to float
  %c = fcmp olt float %x, %y
  ret i1 %c
}
";
    let after = converted(text, &[&[1, 2], &[2, 1], &[-5, -5], &[-128, 127]]);
    assert_eq!(floats(&after), 0, "{after}");
    assert!(after.contains("icmp slt i16"), "{after}");
}

/// `fneg` is `0 - x`.
#[test]
fn a_negation_is_a_subtraction_from_zero() {
    let text = ADD_ONE.replace("%y = fadd float %x, 1.0", "%y = fneg float %x");
    let after = converted(&text, INPUTS);
    assert_eq!(floats(&after), 0, "{after}");
    assert!(after.contains("sub i16 0,"), "{after}");
}

/// A value of the range the mantissa cannot hold stays a float: `i32` into a
/// `float` of 24 bits.
#[test]
fn a_range_wider_than_the_mantissa_stays_float() {
    let text = "define i32 @f(i32 %a) {
b0:
  %x = sitofp i32 %a to float
  %y = fadd float %x, 1.0
  %z = fptosi float %y to i32
  ret i32 %z
}
";
    let after = converted(text, &[&[0], &[5], &[-5]]);
    assert_eq!(floats(&after), 3, "{after}");
}

/// A use of the float result that is not part of the graph keeps the graph.
#[test]
fn a_float_used_elsewhere_keeps_the_graph() {
    let text = "@g = global float 0.0

define i32 @f(i8 %a) {
b0:
  %x = sitofp i8 %a to float
  %y = fadd float %x, 1.0
  store float %y, ptr @g
  %z = fptosi float %y to i32
  ret i32 %z
}
";
    let after = converted(text, INPUTS);
    assert!(after.contains("fadd float"), "{after}");
}

/// A constant with a fraction is not an integer.
#[test]
fn a_fractional_constant_keeps_the_graph() {
    let text = ADD_ONE.replace("1.0", "0.5");
    let after = converted(&text, INPUTS);
    assert!(after.contains("fadd float"), "{after}");
}

/// Negative zero is not an integer, unless the operation ignores the sign of
/// zero.
#[test]
fn negative_zero_is_an_integer_only_under_nsz() {
    let text = ADD_ONE.replace("1.0", "-0.0");
    assert!(converted(&text, INPUTS).contains("fadd float"));
    let nsz = ADD_ONE.replace("fadd float", "fadd nsz float").replace("1.0", "-0.0");
    let after = converted(&nsz, INPUTS);
    assert_eq!(floats(&after), 0, "{after}");
}

/// `(int)(float)a` is a `sext`, and no float.
#[test]
fn a_root_reading_one_converted_integer_is_an_extension() {
    let text = "define i32 @f(i8 %a) {
b0:
  %x = sitofp i8 %a to float
  %z = fptosi float %x to i32
  ret i32 %z
}
";
    let after = converted(text, INPUTS);
    assert_eq!(floats(&after), 0, "{after}");
    assert!(after.contains("sext i8"), "{after}");
}

/// A chain of `n` additions, rooted at an `fptosi`.
fn chain(n: usize) -> String {
    let mut text = String::from("define i32 @f(i8 %a) {\nb0:\n  %x0 = sitofp i8 %a to double\n");
    for k in 0..n {
        text += &format!("  %x{} = fadd double %x{k}, 1.0\n", k + 1);
    }
    text + &format!("  %z = fptosi double %x{n} to i32\n  ret i32 %z\n}}\n")
}

/// The pass is linear in the chain: four times the length costs about four
/// times, not sixteen (a slope test at 1x and 4x, the least of three runs).
#[test]
fn the_time_grows_with_the_chain_not_with_its_square() {
    let least = |n: usize| {
        let text = chain(n);
        (0..3)
            .map(|_| {
                let mut module = parsed(&format!("{DOS}{text}"));
                llrm_support::debug::work_of(|| managed(&mut module, Float2Int)).1 as f64
            })
            .fold(f64::MAX, f64::min)
    };
    let (small, large) = (least(500), least(2000));
    assert!(large < small * 6.0, "500 adds {small:.0} work, 2000 adds {large:.0}: a ratio of {:.1}", large / small);
}

/// A chain of 2000 long converts (the pin's recursion would be deep): the
/// answer is the sum.
#[test]
fn a_long_chain_converts_without_recursion() {
    let after = converted(&chain(2000), &[&[0], &[7]]);
    assert_eq!(floats(&after), 0, "{}", &after[..200.min(after.len())]);
}
