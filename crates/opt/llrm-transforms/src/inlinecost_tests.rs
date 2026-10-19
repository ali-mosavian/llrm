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
fn priced_with(
    text: &str,
    at: usize,
    sole: bool,
    size: CallerSize,
    tti: Option<&dyn llrm_mir::tti::Tti>,
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
        tti,
    };
    let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
    match inline_cost(&site, params) {
        InlineCost::Cost { cost, threshold, .. } => (cost, threshold),
        other => panic!("{other:?}"),
    }
}

fn priced(
    text: &str,
    at: usize,
    sole: bool,
    size: CallerSize,
) -> (i64, i64) {
    priced_with(text, at, sole, size, None)
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

/// A `noipa` callee is never inlined, whatever its cost (`alwaysinline`
/// included): the same callee without it costs -20 and is inlined (the
/// premise).
#[test]
fn a_noipa_callee_is_never_inlined_by_cost() {
    let text = |attribute: &str| {
        format!(
            "{GLOBAL}define internal i32 @callee(i32 %x) {attribute} {{\nb0:\n  %a = load i32, ptr @g\n  %b = add i32 %a, %x\n  store i32 %b, ptr @g\n  ret i32 %b\n}}\n\ndefine i32 @f(i32 %y) {{\nb0:\n  %r = call i32 @callee(i32 %y)\n  %s = call i32 @callee(i32 %r)\n  ret i32 %s\n}}\n"
        )
    };
    assert_eq!(priced(&text(""), 0, false, CallerSize::Speed), (-20, 337), "premise");
    for attribute in ["noipa", "noipa alwaysinline"] {
        let module = crate::testing::parsed(&text(attribute));
        let layout = llrm_mir::datalayout::DataLayout::default();
        let (caller_id, _, caller) = module.functions().find(|(_, g, _)| g.name.as_deref() == Some("f")).unwrap();
        let (callee_id, _, callee) = module.functions().find(|(_, g, _)| g.name.as_deref() == Some("callee")).unwrap();
        let call = caller
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| llrm_mir::memory::callee(&module.context, caller, inst) == Some(callee_id))
            .unwrap();
        let site = Site {
            module: &module,
            layout: &layout,
            caller,
            caller_id,
            call,
            callee,
            callee_id,
            sole_call_to_local: false,
            caller_recursive: false,
            caller_size: CallerSize::Speed,
            relative_frequency: None,
            tti: None,
        };
        let verdict = inline_cost(&site, InlineParams::from_level(2));
        assert!(matches!(verdict, InlineCost::Never(_)) && !verdict.is_ok(), "{attribute}: {verdict:?}");
    }
}

/// The target's price decides what is free: an address `p + 4 * i` folds into
/// an x86 load (scale 4), the generic rule takes scale 1 only; a `trunc` to
/// `i1` is free for X86's `isTruncateFree` and not for the generic legal
/// integer rule. Each callee costs 5 less per such instruction under the X86
/// model, and the generic number is unchanged.
#[test]
fn the_x86_model_makes_a_scaled_address_and_a_narrow_trunc_free() {
    let x86 = llrm_x86::tti::X86::of_cpu("i486").unwrap();
    let gep = format!(
        "{GLOBAL}define internal i32 @callee(ptr %p, i32 %i) {{\nb0:\n  %q = getelementptr i32, ptr %p, i32 %i\n  %v = load i32, ptr %q\n  ret i32 %v\n}}\n\ndefine i32 @f(ptr %a, i32 %j) {{\nb0:\n  %r = call i32 @callee(ptr %a, i32 %j)\n  %s = call i32 @callee(ptr %a, i32 %r)\n  ret i32 %s\n}}\n"
    );
    let generic = priced_with(&gep, 0, false, CallerSize::Speed, None).0;
    let target = priced_with(&gep, 0, false, CallerSize::Speed, Some(&x86)).0;
    assert_eq!(generic - target, 5, "generic {generic}, x86 {target}");
    let trunc = format!(
        "{GLOBAL}define internal i1 @callee(i32 %x) {{\nb0:\n  %t = trunc i32 %x to i1\n  ret i1 %t\n}}\n\ndefine i1 @f(i32 %y) {{\nb0:\n  %r = call i1 @callee(i32 %y)\n  %s = call i1 @callee(i32 %y)\n  ret i1 %r\n}}\n"
    );
    let generic = priced_with(&trunc, 0, false, CallerSize::Speed, None).0;
    let target = priced_with(&trunc, 0, false, CallerSize::Speed, Some(&x86)).0;
    assert_eq!(generic - target, 5, "generic {generic}, x86 {target}");
}

#[test]
fn test_a_decided_site_is_remarked_as_opt_remarks_it() {
    let priced = InlineCost::Cost { cost: 40, threshold: 225, static_bonus: 0 };
    assert_eq!(
        crate::inlinecost::remark("_main@0", "_f@3", &priced, true),
        "'_f@3' inlined into '_main@0' with (cost=40, threshold=225)"
    );
    let dear = InlineCost::Cost { cost: 300, threshold: 225, static_bonus: 0 };
    assert_eq!(
        crate::inlinecost::remark("g", "f", &dear, false),
        "'f' not inlined into 'g' because too costly to inline (cost=300, threshold=225)"
    );
    assert_eq!(
        crate::inlinecost::remark("g", "f", &priced, false),
        "'f' not inlined into 'g' because the copy was not made (cost=40, threshold=225)"
    );
}
