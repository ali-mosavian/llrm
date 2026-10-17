//! `inlinecost` against `opt -passes=inline -pass-remarks=inline` (LLVM
//! 24.0.0git d1106deb71cc, `-inline-threshold=225`, except the optsize case) on
//! the same text: the cost and threshold of each call site, before any
//! inlining.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;

use crate::inlinecost::{CallerSize, InlineCost, InlineParams, Site, inline_cost};
use crate::testing::parsed;

const GLOBAL: &str = "@g = global i32 0\n\n";

/// The cost and threshold of the `at`th call of @callee in @f, `sole`: the
/// callee's only call (the `LastCallToStatic` bonus is in the cost, as in
/// `opt`'s remark).
fn priced(
    text: &str,
    at: usize,
    sole: bool,
    size: CallerSize,
) -> (i64, i64) {
    let module: Module = parsed(text);
    let layout = DataLayout::default();
    let (caller_id, _, caller) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap();
    let (callee_id, _, callee) =
        module.functions().find(|(_, global, _)| global.name.as_deref() == Some("callee")).unwrap();
    let call = caller
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| llrm_mir::memory::callee(&module.context, caller, inst) == Some(callee_id))
        .nth(at)
        .unwrap();
    let site = Site {
        module: &module,
        layout: &layout,
        caller,
        caller_id,
        call,
        callee,
        callee_id,
        sole_call_to_local: sole,
        caller_recursive: false,
        caller_size: size,
        relative_frequency: None,
    };
    let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
    match inline_cost(&site, params) {
        InlineCost::Cost { cost, threshold, .. } => (cost, threshold),
        other => panic!("{other:?}"),
    }
}

/// A callee of straight code: the single-block bonus raises the threshold to
/// 337, and the loads, adds and stores come to -20 (`opt`: cost=-20,
/// threshold=337).
#[test]
fn test_a_single_block_callee_costs_what_opt_says() {
    let text = format!(
        "{GLOBAL}define internal i32 @callee(i32 %x) {{
b0:
  %a = load i32, ptr @g
  %b = add i32 %a, %x
  store i32 %b, ptr @g
  ret i32 %b
}}

define i32 @f(i32 %y) {{
b0:
  %r = call i32 @callee(i32 %y)
  %s = call i32 @callee(i32 %r)
  ret i32 %s
}}
"
    );
    assert_eq!(priced(&text, 0, false, CallerSize::Speed), (-20, 337));
    assert_eq!(priced(&text, 1, true, CallerSize::Speed), (-15020, 337));
}

/// A constant argument decides the callee's branch: the dead arm is not
/// priced (`opt`: cost=-35 threshold=337; the other site -5, 225 with two
/// blocks).
#[test]
fn test_a_constant_argument_prices_only_the_live_arm() {
    let text = format!(
        "{GLOBAL}define internal i32 @callee(i32 %x) {{
b0:
  %c = icmp eq i32 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  ret i32 7

b2:
  %a = load i32, ptr @g
  %b = mul i32 %a, %x
  %d = add i32 %b, 1
  ret i32 %d
}}

define i32 @f(i32 %y) {{
b0:
  %r = call i32 @callee(i32 0)
  %s = call i32 @callee(i32 %y)
  ret i32 %r
}}
"
    );
    assert_eq!(priced(&text, 0, false, CallerSize::Speed), (-35, 337));
    assert_eq!(priced(&text, 1, false, CallerSize::Speed), (-5, 225));
}

/// A float argument folds `fcmp` and the arm it decides (`opt`: -35, 337 for
/// the constant; before float folding the dead arm was priced).
#[test]
fn test_a_float_constant_folds_the_branch_it_decides() {
    let text = "define internal float @callee(float %x) {
b0:
  %a = fmul float %x, 2.000000e+00
  %c = fcmp ogt float %a, 1.000000e+00
  br i1 %c, label %t, label %e

t:
  %s = fadd float %a, 1.000000e+00
  ret float %s

e:
  ret float %a
}

define float @f(float %y) {
b0:
  %r = call float @callee(float 1.000000e+00)
  %s = call float @callee(float %y)
  %t = fadd float %r, %s
  ret float %t
}
";
    assert_eq!(priced(text, 0, false, CallerSize::Speed), (-35, 337));
}

/// A caller marked `optsize` holds the call to 50 (`OptSizeThreshold`), and
/// a one-block callee's bonus makes it 75 (`opt` without `-inline-threshold`:
/// cost=-15030, threshold=75).
#[test]
fn test_an_optsize_caller_is_held_to_the_optsize_threshold() {
    let text = format!(
        "{GLOBAL}define internal i32 @callee(i32 %x) {{
b0:
  %a = load i32, ptr @g
  ret i32 %a
}}

define i32 @f(i32 %y) {{
b0:
  %r = call i32 @callee(i32 %y)
  ret i32 %r
}}
"
    );
    assert_eq!(priced(&text, 0, false, CallerSize::OptSize).1, 75);
}
