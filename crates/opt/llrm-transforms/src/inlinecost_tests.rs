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

/// m16 priced every instruction with the generic rules (`Machine::tti` was
/// none): a narrow `trunc` cost 5 more per use than under the i386 X86 model
/// LLVM prices `-code16` with, so a callee the walk inlines under it was
/// refused. A scale-4 address is no 16-bit form, so it stays as dear as the
/// generic rule has it.
#[test]
fn test_m16_prices_a_narrow_trunc_with_the_x86_model_and_a_scaled_address_dear() {
    use llrm_mir::target::Machine;
    let m16 = llrm_x86_m16::Dos::default().tti();
    let gep = format!(
        "{GLOBAL}define internal i32 @callee(ptr %p, i32 %i) {{\nb0:\n  %q = getelementptr i32, ptr %p, i32 %i\n  %v = load i32, ptr %q\n  ret i32 %v\n}}\n\ndefine i32 @f(ptr %a, i32 %j) {{\nb0:\n  %r = call i32 @callee(ptr %a, i32 %j)\n  %s = call i32 @callee(ptr %a, i32 %r)\n  ret i32 %s\n}}\n"
    );
    let generic = priced_with(&gep, 0, false, CallerSize::Speed, None).0;
    let target = priced_with(&gep, 0, false, CallerSize::Speed, m16.as_deref()).0;
    assert_eq!(generic, target, "a scale-4 address is not free in real mode");
    let trunc = format!(
        "{GLOBAL}define internal i1 @callee(i32 %x) {{\nb0:\n  %t = trunc i32 %x to i1\n  ret i1 %t\n}}\n\ndefine i1 @f(i32 %y) {{\nb0:\n  %r = call i1 @callee(i32 %y)\n  %s = call i1 @callee(i32 %y)\n  ret i1 %r\n}}\n"
    );
    let generic = priced_with(&trunc, 0, false, CallerSize::Speed, None).0;
    let target = priced_with(&trunc, 0, false, CallerSize::Speed, m16.as_deref()).0;
    assert_eq!(generic - target, 5, "generic {generic}, m16 {target}");
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

/// A site eight `strcmp(..) == 0` tests deep is 0.625^8 = 2.3% of the entry
/// in LLVM's `BlockFrequencyInfo` over `BranchProbabilityInfo` (the zero
/// heuristic of a three-way compare): above the 2% cold cut, so `opt` holds
/// `leaf` to 337 (225 and the single-block bonus). `profit`'s frequencies put
/// it under the cut, cold, threshold 45 (`_cvar_find` in QCport's `_cmd_run`).
#[test]
fn test_a_site_llvms_frequencies_keep_warm_is_not_held_to_the_cold_threshold() {
    let mut text = String::from(
        "declare i16 @strcmp(ptr, ptr) threeway\n\n\
         define internal i16 @leaf(i16 %x) {\nentry:\n  %r = add i16 %x, 1\n  ret i16 %r\n}\n\n\
         define i16 @main(ptr %p, ptr %q, i16 %x) {\nentry:\n  br label %t0\n\n",
    );
    for i in 0..8 {
        text += &format!(
            "t{i}:\n  %s{i} = call i16 @strcmp(ptr %p, ptr %q)\n  %c{i} = icmp eq i16 %s{i}, 0\n  br i1 %c{i}, label %out, label %t{}\n\n",
            i + 1
        );
    }
    text += "t8:\n  %v = call i16 @leaf(i16 %x)\n  ret i16 %v\n\nout:\n  ret i16 0\n}\n";
    let module = parsed(&text);
    let main = module.globals.iter().position(|one| one.name.as_deref() == Some("main")).unwrap();
    let mut decided = Vec::new();
    crate::inlinecost::sites(
        &module,
        &DataLayout::default(),
        llrm_mir::context::GlobalId(main as u32),
        &crate::inline::call_counts(&module),
        &Default::default(),
        &Default::default(),
        InlineParams::from_level(2),
        CallerSize::Speed,
        false,
        None,
        Some(&mut decided),
    );
    let [(_, _, InlineCost::Cost { threshold, .. })] = decided[..] else { panic!("one priced site: {decided:?}") };
    assert_eq!(threshold, 337);
}

/// `decide`, the walk's question of one site, is the verdict `sites` gives it,
/// and no verdict for a call of a routine with no body.
#[test]
fn test_one_site_is_decided_as_the_site_list_decides_it() {
    let text = "declare i16 @ext(i16)\n\n\
        define internal i16 @leaf(i16 %x) {\nentry:\n  %r = add i16 %x, 1\n  ret i16 %r\n}\n\n\
        define i16 @main(i16 %x) {\nentry:\n  %a = call i16 @leaf(i16 %x)\n  %b = call i16 @ext(i16 %a)\n  ret i16 %b\n}\n";
    let module = parsed(text);
    let layout = DataLayout::default();
    let main = llrm_mir::context::GlobalId(
        module.globals.iter().position(|one| one.name.as_deref() == Some("main")).unwrap() as u32,
    );
    let counts = crate::inline::call_counts(&module);
    let none = Default::default();
    let context = crate::inlinecost::DecideContext {
        counts: &counts,
        addressed: &none,
        recursive: &none,
        params: InlineParams::from_level(2),
        size: CallerSize::Speed,
        always_only: false,
        tti: None,
    };
    let llrm_mir::module::GlobalKind::Function(caller) = &module.global(main).kind else { panic!("main") };
    let frequencies = crate::inlinecost::caller_frequencies(&module, &layout, caller);
    let calls: Vec<_> = caller
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(
            caller.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Call(_)
        ))
        .collect();
    let one =
        crate::inlinecost::decide(&module, &layout, main, calls[0], &context, &frequencies).expect("leaf has a body");
    let mut listed = Vec::new();
    crate::inlinecost::sites(
        &module,
        &layout,
        main,
        &counts,
        &none,
        &none,
        InlineParams::from_level(2),
        CallerSize::Speed,
        false,
        None,
        Some(&mut listed),
    );
    assert_eq!((one.inline, &one.cost), (true, &listed[0].2));
    assert_eq!(crate::inlinecost::decide(&module, &layout, main, calls[1], &context, &frequencies), None);
}

/// `inlinehint` raises a callee's threshold to the hint threshold, not for a
/// caller built for minimum size (opt: 487 = 325 * 1.5 for a single block at
/// speed, against 337 = 225 * 1.5 without the hint).
#[test]
fn test_an_inline_hint_raises_the_budget_by_llvms_ratio_and_not_for_size() {
    let text = |attribute: &str| {
        format!(
            "{GLOBAL}define internal i32 @callee(i32 %x) {attribute} {{\nb0:\n  %a = load i32, ptr @g\n  %b = add i32 %a, %x\n  store i32 %b, ptr @g\n  ret i32 %b\n}}\n\ndefine i32 @f(i32 %y) {{\nb0:\n  %r = call i32 @callee(i32 %y)\n  %s = call i32 @callee(i32 %r)\n  ret i32 %s\n}}\n"
        )
    };
    // Premise: the same callee unhinted, at speed and for minimum size.
    assert_eq!(priced(&text(""), 0, false, CallerSize::Speed).1, 337);
    assert_eq!(priced(&text(""), 0, false, CallerSize::MinSize).1, 5);
    assert_eq!(priced(&text("inlinehint"), 0, false, CallerSize::Speed).1, 487);
    assert_eq!(priced(&text("inlinehint"), 0, false, CallerSize::MinSize).1, 5);
}
