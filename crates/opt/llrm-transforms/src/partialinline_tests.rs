use super::PartiallyInlineLibCalls;
use crate::testing::{Tuned, managed_on, parsed};
use llrm_analysis::testing::DOS;

fn run(
    text: &str,
    machine: Tuned,
) -> String {
    let mut module = parsed(&format!("{DOS}{text}"));
    managed_on(&mut module, PartiallyInlineLibCalls, machine)
}

fn fast() -> Tuned {
    Tuned { fast_sqrt: true, ..Default::default() }
}

const SQRT: &str = "declare double @sqrt(double)
define double @f(double %x) {
b0:
  %r = call double @sqrt(double %x)
  ret double %r
}
";

/// A `sqrt` that may set `errno` stayed a call on every path; the pin keeps
/// the library call for a NaN or a negative operand only
/// (`call.sqrt`), and the result joins in a phi.
#[test]
fn a_sqrt_that_sets_errno_is_the_instruction_with_a_library_call_on_the_error_path() {
    let text = run(SQRT, fast());
    assert!(text.contains("call.sqrt:"), "{text}");
    assert!(text.contains("fcmp ord double"), "{text}");
    assert!(text.contains("phi double"), "{text}");
    assert_eq!(text.matches("call double @sqrt").count(), 2, "{text}");
    assert_eq!(text.matches("memory(none)").count(), 1, "the first call is the instruction\n{text}");
    let fcmp = text.find("fcmp ord").expect("compare");
    let slow = text.find("call.sqrt:").expect("block");
    assert!(fcmp < slow, "{text}");
}

/// Where `fcmp ord` is dear (X86), the test is `x >= 0.0`.
#[test]
fn a_target_where_ord_is_dear_compares_the_operand_with_zero() {
    let text = run(SQRT, Tuned { ord_dear: true, ..fast() });
    assert!(text.contains("fcmp oge double %x"), "{text}");
    assert!(!text.contains("fcmp ord"), "{text}");
}

/// No sqrt instruction: the call is all there is.
#[test]
fn a_target_without_a_sqrt_instruction_keeps_the_call() {
    let text = run(SQRT, Tuned::default());
    assert!(!text.contains("call.sqrt"), "{text}");
    assert_eq!(text.matches("call double @sqrt").count(), 1, "{text}");
}

/// What the back end already emits as an instruction, and what the pin does
/// not touch.
#[test]
fn a_call_the_pass_must_not_touch_is_left_alone() {
    for (what, text) in [
        ("readonly call", SQRT.replace("%x)\n  ret", "%x) memory(read)\n  ret")),
        ("memory(none) call", SQRT.replace("%x)\n  ret", "%x) memory(none)\n  ret")),
        ("nobuiltin call", SQRT.replace("%x)\n  ret", "%x) nobuiltin\n  ret")),
        ("musttail call", SQRT.replace("= call", "= musttail call")),
        (
            "local function",
            "define internal double @sqrt(double %y) {\nb0:\n  ret double %y\n}\ndefine double @f(double %x) {\nb0:\n  %r = call double @sqrt(double %x)\n  ret double %r\n}\n"
                .to_owned(),
        ),
    ] {
        let after = run(&text, fast());
        assert!(!after.contains("call.sqrt"), "{what}\n{after}");
    }
}

/// `sqrtf` is the same function in single precision.
#[test]
fn sqrtf_is_split_the_same_way() {
    let text = run(
        "declare float @sqrtf(float)
define float @f(float %x) {
b0:
  %r = call float @sqrtf(float %x)
  ret float %r
}
",
        fast(),
    );
    assert!(text.contains("call.sqrt:") && text.contains("fcmp ord float"), "{text}");
}

/// The pin visits the block the split leaves behind: a second `sqrt` after the
/// first is split too, and the first's join block is where the second lives.
#[test]
fn a_second_sqrt_after_the_first_is_split_in_the_join_block() {
    let text = run(
        "declare double @sqrt(double)
define double @f(double %x, double %y) {
b0:
  %a = call double @sqrt(double %x)
  %b = call double @sqrt(double %y)
  %s = fadd double %a, %b
  ret double %s
}
",
        fast(),
    );
    assert_eq!(text.matches("call.sqrt").count() >= 2, true, "{text}");
    assert_eq!(text.matches("phi double").count(), 2, "{text}");
}

/// DEVIATE PIL-OS: the pin splits at -Os too, which grows the code. Where the
/// target's `Tuning` says so, a function with `optsize` (or `minsize`) keeps
/// its call; elsewhere it is the pin's, split.
#[test]
fn an_optsize_function_keeps_its_sqrt_call_where_the_tuning_says_so() {
    for attr in ["optsize", "minsize"] {
        let text = SQRT.replace("double %x) {", &format!("double %x) {attr} {{"));
        let kept = run(&text, Tuned { skip_partial_at_size: true, ..fast() });
        assert!(!kept.contains("call.sqrt"), "{attr}\n{kept}");
        assert_eq!(kept.matches("call double @sqrt").count(), 1, "{attr}\n{kept}");
        let pins = run(&text, fast());
        assert!(pins.contains("call.sqrt:"), "the pin's default splits at size\n{attr}\n{pins}");
    }
}

/// The tuning is for functions tuned for size alone: a plain function is still split.
#[test]
fn the_size_tuning_leaves_a_speed_function_split() {
    let text = run(SQRT, Tuned { skip_partial_at_size: true, ..fast() });
    assert!(text.contains("call.sqrt:"), "{text}");
}
