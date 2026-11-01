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
/// i486 in 16-bit code with the target's -Os growth guard (the m16
/// description's `inline_size_guard`).
fn x86_real() -> llrm_x86::tti::X86 {
    llrm_x86::tti::X86::of_cpu("i486")
        .unwrap()
        .in_real_mode(
            llrm_x86::tti::RealMode {
                scale_costs: vec![(1, 0)],
                registers: 6,
                scaled_index_insns: 0,
                huge_space: None,
                inline_size_guard: true,
                float_code_size_percent: 100,
                unroll_size_few_trips: (0, 0),
                recursive_inlining: None,
            },
        )
}

fn priced_with(
    text: &str,
    at: usize,
    sole: bool,
    size: CallerSize,
    tti: Option<&dyn llrm_mir::tti::Tti>,
) -> (i64, i64) {
    priced_at(text, at, sole, size, tti, None)
}

fn priced_at(
    text: &str,
    at: usize,
    sole: bool,
    size: CallerSize,
    tti: Option<&dyn llrm_mir::tti::Tti>,
    frequency: Option<f64>,
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
        relative_frequency: frequency,
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

/// An external callee that stays after the inline is not copied into an
/// `optsize` caller in 16-bit code (D11, the target's `inline_size_guard`):
/// crc, tile and recchop grew 52, 42 and 15 bytes at -Os (cost 45, 30 and 40
/// against the pin's threshold of 50). In 32-bit code the pin's decision
/// stands: the `called_once` kernel's `twice` (cost 45) is inlined.
#[test]
fn an_optsize_caller_does_not_copy_a_callee_that_stays_in_16_bit_code() {
    let text = format!(
        "{GLOBAL}define i32 @callee(i32 %x) {{
b0:
  %a = load i32, ptr @g
  %b = add i32 %a, %x
  %c = add i32 %b, %a
  %d = mul i32 %c, %x
  %e = add i32 %d, %b
  %f = mul i32 %e, %a
  %g = add i32 %f, %c
  %h = mul i32 %g, %d
  %i = add i32 %h, %a
  %j = mul i32 %i, %x
  %k = add i32 %j, %c
  %l = mul i32 %k, %b
  %m = add i32 %l, %e
  store i32 %m, ptr @g
  ret i32 %m
}}

define i32 @f(i32 %y) {{
b0:
  %r = call i32 @callee(i32 %y)
  ret i32 %r
}}
"
    );
    let (x86, flat) = (x86_real(), llrm_x86::tti::X86::of_cpu("i486").unwrap());
    let (generic, bound) = priced_with(&text, 0, false, CallerSize::OptSize, None);
    let (target, held) = priced_with(&text, 0, false, CallerSize::OptSize, Some(&x86));
    let (wide, kept) = priced_with(&text, 0, false, CallerSize::OptSize, Some(&flat));
    assert!(generic < bound, "premise: the pin inlines it, cost {generic} threshold {bound}");
    assert!(target >= held, "16-bit code inlined it, cost {target} threshold {held}");
    assert!(wide < kept, "32-bit code kept it, cost {wide} threshold {kept}");
}

/// A callee that stays is not copied into a speed caller where the target's
/// `inline_size_guard_at_speed` says so: qlight c at -O2 inlined a
/// 30-instruction `ls_scale_byte` four times, 211 -> 376 bytes. Off, the pin's
/// decision stands.
#[test]
fn a_speed_caller_does_not_copy_a_callee_that_stays_where_the_target_guards_size() {
    let text = format!(
        "{GLOBAL}define i32 @callee(i32 %x) {{
b0:
  %a = load i32, ptr @g
  %b = add i32 %a, %x
  %c = add i32 %b, %a
  %d = mul i32 %c, %x
  %e = add i32 %d, %b
  %f = mul i32 %e, %a
  %g = add i32 %f, %c
  %h = mul i32 %g, %d
  %i = add i32 %h, %a
  %j = mul i32 %i, %x
  %k = add i32 %j, %c
  %l = mul i32 %k, %b
  %m = add i32 %l, %e
  store i32 %m, ptr @g
  ret i32 %m
}}

define i32 @f(i32 %y) {{
b0:
  %r = call i32 @callee(i32 %y)
  ret i32 %r
}}
"
    );
    let guarded = x86_real()
        .with_tuning(llrm_mir::target::Tuning { inline_size_guard_at_speed: true, ..llrm_mir::target::Tuning::PIN });
    let (plain, bound) = priced_with(&text, 0, false, CallerSize::Speed, Some(&x86_real()));
    let (cost, held) = priced_with(&text, 0, false, CallerSize::Speed, Some(&guarded));
    assert!(plain < bound, "premise: without the guard the pin inlines it, cost {plain} threshold {bound}");
    assert!(cost >= held, "the guard kept it, cost {cost} threshold {held}");
}

/// A callee at `-Os` on x86 (D11): the inline must not make the code bigger.
/// `idioms.bas` copied a 14-instruction loop (cost 15 against the target's old
/// bound of 25) into ten call sites, +690 bytes. The callee that stays is
/// refused when its instructions that do not fold cost more code than the call
/// and its argument pushes; the last call of a local callee, which goes with
/// it, and a callee smaller than its call are inlined.
#[test]
fn an_optsize_caller_inlines_only_what_does_not_grow_the_code_on_x86() {
    let text = |linkage: &str, body: &str| {
        format!(
            "@arr = global [64 x i32] zeroinitializer

define {linkage}i32 @callee(i32 %x) {{
{body}
}}

define i32 @f(i32 %y) {{
b0:
  %r = call i32 @callee(i32 %y)
  ret i32 %r
}}
"
        )
    };
    let looped = "b0:
  br label %h
h:
  %i = phi i32 [ 0, %b0 ], [ %n, %h ]
  %s = phi i32 [ %x, %b0 ], [ %t, %h ]
  %p = getelementptr i32, ptr @arr, i32 %i
  %v = load i32, ptr %p
  %t = add i32 %s, %v
  %n = add i32 %i, 1
  %c = icmp ne i32 %n, 64
  br i1 %c, label %h, label %e
e:
  ret i32 %t";
    let small = "b0:
  %a = add i32 %x, 1
  ret i32 %a";
    let x86 = x86_real();
    let held = |text: &str, sole: bool| {
        let (cost, threshold) = priced_with(text, 0, sole, CallerSize::OptSize, Some(&x86));
        cost < threshold
    };
    let (generic, bound) = priced_with(&text("", looped), 0, false, CallerSize::OptSize, None);
    assert!(generic < bound, "premise: the pin inlines the loop, cost {generic} threshold {bound}");
    assert!(!held(&text("", looped), false), "an external loop callee was copied into the caller");
    assert!(!held(&text("internal ", looped), false), "a local callee with other calls was copied");
    assert!(held(&text("internal ", looped), true), "the last call of a local callee, which goes, stays");
    assert!(held(&text("", small), false), "a callee smaller than its call stays");
    // A call and the push of its argument are bigger than the instructions
    // they count as: `_savegame_read_block` (28 calls) grew its module by 408
    // bytes while the call counted as three plain instructions.
    let three = "b0:
  %a = add i32 %x, 1
  %b = xor i32 %a, 85
  %c = mul i32 %b, 3
  ret i32 %c";
    assert!(held(&text("", three), false), "a callee of the size of a far call and its push was kept out");
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

/// A `weak` callee is never inlined: the QB runtime's `__attribute__((weak))
/// void music_tick(void) {}` was inlined into the tick thread as nothing, so
/// the real `music_tick` of another object never ran and a program that waited
/// on music hung. `weak_odr` and `linkonce_odr` bodies are equivalent by
/// promise and are inlined.
#[test]
fn an_interposable_callee_is_not_inlined() {
    let text = |linkage: &str| {
        format!(
            "{GLOBAL}define {linkage} void @callee() {{\nb0:\n  store i32 1, ptr @g\n  ret void\n}}\n\ndefine void @f() {{\nb0:\n  call void @callee()\n  ret void\n}}\n"
        )
    };
    for (linkage, inlined) in [("weak", false), ("linkonce", false), ("weak_odr", true), ("linkonce_odr", true)] {
        let module = crate::testing::parsed(&text(linkage));
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
        assert_eq!(verdict.is_ok(), inlined, "{linkage}: {verdict:?}");
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
    let priced = InlineCost::Cost { cost: 40, threshold: 225, static_bonus: 0, blocked: false };
    assert_eq!(
        crate::inlinecost::remark("_main@0", "_f@3", &priced, true),
        "'_f@3' inlined into '_main@0' with (cost=40, threshold=225)"
    );
    let dear = InlineCost::Cost { cost: 300, threshold: 225, static_bonus: 0, blocked: false };
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

/// An `optsize` caller still raises an `inlinehint` callee to the hint
/// threshold (`OptSizeHintThreshold` = `HintThreshold`, InlineCost.cpp:3394;
/// opt: threshold 487 hinted, 75 not). `Threshold::for_size` once stated 1.
#[test]
fn test_an_inline_hint_still_raises_the_budget_at_an_optsize_caller() {
    let text = |attrs: &str| {
        format!(
            "{GLOBAL}define internal i32 @callee(i32 %x) {attrs} {{\nb0:\n  %a = load i32, ptr @g\n  ret i32 %a\n}}\n\ndefine i32 @f(i32 %y) {{\nb0:\n  %r = call i32 @callee(i32 %y)\n  ret i32 %r\n}}\n"
        )
    };
    assert_eq!(priced(&text(""), 0, false, CallerSize::OptSize).1, 75);
    assert_eq!(priced(&text("inlinehint"), 0, false, CallerSize::OptSize).1, 487);
}

/// intrinsics_errors_qb45.bas m32 -O1: the module handler registers the error
/// handler of the frame that runs it (`onerror(true)`); inlined into a
/// procedure's pad, the registration belonged to a frame that returns, and the
/// next error found no handler ("Illegal function call"; erlnum.bas hung).
#[test]
fn test_a_callee_that_makes_a_frame_bound_call_is_never_inlined() {
    let module = crate::testing::parsed(
        "declare void @llrm.qb.onerror(i1) nounwind frame_bound

define internal i16 @callee(i16 %c) {
b0:
  call void @llrm.qb.onerror(i1 true)
  ret i16 1
}

define i16 @f(i16 %a) {
b0:
  %r = call i16 @callee(i16 %a)
  ret i16 %r
}
",
    );
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
        sole_call_to_local: true,
        caller_recursive: false,
        caller_size: CallerSize::Speed,
        relative_frequency: None,
        tti: None,
    };
    let verdict = inline_cost(&site, InlineParams::from_level(2));
    assert!(matches!(verdict, InlineCost::Never(_)), "{verdict:?}");
}

/// stft_single.bas -Os: a call in front of `unreachable` is held to threshold
/// 0, and the walk stopped at cost 0; `cost < max(1, threshold)` is
/// `finalizeAnalysis`'s, the verdict is `Cost < Threshold` (InlineCost.h:138),
/// so the 350-instruction benchmark body was copied into `__main` and kept.
#[test]
fn test_a_verdict_of_cost_zero_at_threshold_zero_does_not_pay() {
    assert!(!InlineCost::Cost { cost: 0, threshold: 0, static_bonus: 0, blocked: false }.is_ok());
    assert!(InlineCost::Cost { cost: -1, threshold: 0, static_bonus: 0, blocked: false }.is_ok());
    assert!(!InlineCost::Cost { cost: 75, threshold: 75, static_bonus: 0, blocked: false }.is_ok());
}

/// `-fno-inline-functions-called-once` set `Threshold::last` and nothing read
/// it after `decide` replaced the clocks inliner (5bff5e074): the sole call of
/// a local callee still took the 15000 bonus and nib's `bench_scroll` was
/// inlined into `main`.
#[test]
fn the_last_call_bonus_is_off_where_called_once_inlining_is() {
    let text = "define internal i32 @callee(i32 %x) {
b0:
  %a = add i32 %x, 1
  ret i32 %a
}

define i32 @f(i32 %y) {
b0:
  %r = call i32 @callee(i32 %y)
  ret i32 %r
}
";
    let price = |last_call_to_static: bool| {
        let module: Module = parsed(text);
        let layout = DataLayout::default();
        let (caller_id, _, caller) =
            module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap();
        let (callee_id, _, callee) =
            module.functions().find(|(_, global, _)| global.name.as_deref() == Some("callee")).unwrap();
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
            sole_call_to_local: true,
            caller_recursive: false,
            caller_size: CallerSize::Speed,
            relative_frequency: None,
            tti: None,
        };
        let params =
            InlineParams { compute_full_inline_cost: true, last_call_to_static, ..InlineParams::from_level(2) };
        match inline_cost(&site, params) {
            InlineCost::Cost { cost, .. } => cost,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(price(false) - price(true), 15000);
}

/// The extension of a simple load is free on x86 (an extending load:
/// `BasicTTIImplBase::getCastInstrCost`, BasicTTIImpl.h:1298-1311), where the
/// cast alone costs an instruction: scroll's `sext (load i16)` made the callee
/// 125 against the pin's 120 (`opt`: cost=-15025 for this callee at i386).
#[test]
fn test_a_sext_of_a_load_is_free_on_x86() {
    let text = "@h = global i16 0

define internal i32 @callee() {
b0:
  %a = load i16, ptr @h
  %b = sext i16 %a to i32
  ret i32 %b
}

define i32 @f() {
b0:
  %r = call i32 @callee()
  ret i32 %r
}
";
    let flat = llrm_x86::tti::X86::of_cpu("i486").unwrap();
    let (cost, _) = priced_with(text, 0, true, CallerSize::Speed, Some(&flat));
    assert_eq!(cost, -15025, "the pin prices this callee at -15025");
}

/// jpeg_fixed -O1/-O2 inlined its large helpers at hot sites and came out 1.8x
/// gcc's bytes. At speed a callee that stays grows the code unless the clocks
/// saved at the site's frequency (capped) pay for the bytes.
#[test]
fn a_speed_caller_inlines_growth_only_where_the_saved_clocks_pay_for_it() {
    let text = "@arr = global [64 x i32] zeroinitializer

define i32 @callee(i32 %x) {
b0:
  br label %h
h:
  %i = phi i32 [ 0, %b0 ], [ %n, %h ]
  %s = phi i32 [ %x, %b0 ], [ %t, %h ]
  %p = getelementptr i32, ptr @arr, i32 %i
  %v = load i32, ptr %p
  %v1 = mul i32 %v, %s
  %v2 = add i32 %v1, %i
  %v3 = mul i32 %v2, %v
  %v4 = xor i32 %v3, %s
  %v5 = mul i32 %v4, 7
  %t = add i32 %s, %v5
  %n = add i32 %i, 1
  %c = icmp ne i32 %n, 64
  br i1 %c, label %h, label %e
e:
  ret i32 %t
}

define i32 @f(i32 %y) {
b0:
  %r = call i32 @callee(i32 %y)
  ret i32 %r
}
";
    let priced = |cap: i64, frequency: f64| {
        let tti = llrm_x86::tti::X86::of_cpu("i486")
            .unwrap()
            .with_tuning(
                llrm_mir::target::Tuning {
                    inline_size_guard_at_speed: true,
                    inline_growth_price: (8000, 8, cap),
                    ..llrm_mir::target::Tuning::PIN
                },
            );
        let (cost, threshold) = priced_at(text, 0, false, CallerSize::Speed, Some(&tti), Some(frequency));
        cost < threshold
    };
    assert!(!priced(4, 1.0), "a cold site paid for the growth");
    assert!(!priced(4, 1000.0), "the cap did not hold the frequency down");
    assert!(priced(0, 1000.0), "a hot site, uncapped, did not pay for it");
}
