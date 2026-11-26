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
                address_registers: None,
                inline_size_guard: true,
                float_code_size_percent: 100,
                unroll_size_few_trips: (0, 0),
                recursive_inlining: None,
                priced_self_inlining: None,
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
    let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
    priced_params(text, at, sole, size, tti, params, (if sole { 1 } else { 2 }, false))
}

fn priced_params(
    text: &str,
    at: usize,
    sole: bool,
    size: CallerSize,
    tti: Option<&dyn llrm_mir::tti::Tti>,
    params: InlineParams,
    (calls, dies): (i64, bool),
) -> (i64, i64) {
    priced_hot(text, at, sole, size, tti, params, (calls, dies), None)
}

#[allow(clippy::too_many_arguments)]
fn priced_hot(
    text: &str,
    at: usize,
    sole: bool,
    size: CallerSize,
    tti: Option<&dyn llrm_mir::tti::Tti>,
    params: InlineParams,
    (calls, dies): (i64, bool),
    frequency: Option<f64>,
) -> (i64, i64) {
    match inline_cost_hot(text, at, sole, size, tti, params, (calls, dies), frequency) {
        InlineCost::Cost { cost, threshold, .. } => (cost, threshold),
        other => panic!("{other:?}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn inline_cost_hot(
    text: &str,
    at: usize,
    sole: bool,
    size: CallerSize,
    tti: Option<&dyn llrm_mir::tti::Tti>,
    params: InlineParams,
    (calls, dies): (i64, bool),
    frequency: Option<f64>,
) -> InlineCost {
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
        callee_calls: calls,
        callee_dies: dies,
        caller_recursive: false,
        callee_linear_recursive: false,
        caller_size: size,
        relative_frequency: frequency,
        cold_frequency: None,
        entry_weight: None,
        planned: None,
        tti,
    };
    inline_cost(&site, params)
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

const COMPARE_LOOP: &str = "@dst = internal global [1200 x i8] zeroinitializer
@ref = internal global [1200 x i8] zeroinitializer
@bad = internal global i32 0
declare void @report(i64)
define internal void @callee(i32 %n) nounwind optsize {
entry:
  br label %loop

loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %pa = getelementptr inbounds nuw i8, ptr @dst, i64 %i
  %va = load i8, ptr %pa, align 1
  %pb = getelementptr inbounds nuw i8, ptr @ref, i64 %i
  %vb = load i8, ptr %pb, align 1
  %differs = icmp ne i8 %va, %vb
  %next = add nuw nsw i64 %i, 1
  %end = icmp eq i64 %next, 1200
  %stop = select i1 %differs, i1 true, i1 %end
  br i1 %stop, label %exit, label %loop

exit:
  br i1 %differs, label %report, label %done

report:
  %wide = sext i32 %n to i64
  tail call void @report(i64 noundef %wide) optsize
  store i32 1, ptr @bad, align 4
  br label %done

done:
  ret void
}

define void @f() {
b0:
  call void @callee(i32 noundef 1) optsize
  call void @callee(i32 noundef 2) optsize
  ret void
}
";

/// The 64-bit target the pinned costs below were measured on: a position
/// dependent one. PIE (the default since #7955) prices a global beside an
/// index higher, 55 for COMPARE_LOOP against the pin's 45.
fn x86_64_non_pie() -> llrm_x86::tti::X86 {
    let tti = llrm_x86::tti::X86::of_cpu("x86-64").unwrap();
    tti.position_independent.set(false);
    tti
}

/// D11 at the 64-bit target: an `optsize` caller does not copy a callee that
/// stays when the copy grows the code, whatever the pin's threshold says (cost
/// 45 against 50: the compare loop of memory_expansion's `check`, 186 sites,
/// -Os m64 46045 -> 57303 B). Off: the pin's.
#[test]
fn test_an_optsize_caller_keeps_a_compare_loop_with_many_sites_when_the_target_guards_size() {
    let tti = |guard: bool| {
        x86_64_non_pie()
            .with_tuning(llrm_mir::target::Tuning { inline_size_guard_at_size: guard, ..llrm_mir::target::Tuning::PIN })
    };
    let (cost, threshold) = priced_with(COMPARE_LOOP, 0, false, CallerSize::OptSize, Some(&tti(false)));
    assert!(cost < threshold, "the premise: the pin inlines it ({cost} against {threshold})");
    let (cost, threshold) = priced_with(COMPARE_LOOP, 0, false, CallerSize::OptSize, Some(&tti(true)));
    assert!(cost >= threshold, "{cost} against {threshold}");
}

/// The -Os size guard leaked into -O1 at m64: a callee of two sites whose
/// copy grows the code stayed a call (QB strings.bas -O1: `exec_popi`, 22
/// sites, 354M instructions against clang's 262M). -O1 keeps the pin's
/// threshold unless the target says the guard binds there.
#[test]
fn test_an_o1_speed_caller_inlines_a_growing_callee_though_the_target_guards_size_at_os() {
    let tti = |at_o1: bool| {
        x86_64_non_pie().with_tuning(llrm_mir::target::Tuning {
            inline_size_guard_at_size: true,
            inline_size_guard_at_o1: at_o1,
            ..llrm_mir::target::Tuning::PIN
        })
    };
    let params = InlineParams { compute_full_inline_cost: true, o1_bound: true, ..InlineParams::from_level(2) };
    let price =
        |at_o1: bool| priced_params(COMPARE_LOOP, 0, false, CallerSize::Speed, Some(&tti(at_o1)), params, (2, false));
    let (cost, threshold) = price(true);
    assert!(cost >= threshold, "the premise: the guard holds it when asked ({cost} against {threshold})");
    let (cost, threshold) = price(false);
    assert!(cost < threshold, "-O1 took the guard of -Os ({cost} against {threshold})");
}

/// SZ2 at the 64-bit target: an external callee with ONE site is not copied
/// into an `optsize` caller when the copy grows the code (cost 45 against 50
/// inlined it; crc -Os m64: 139 B against gcc 96, which keeps the call).
#[test]
fn test_an_optsize_caller_keeps_a_callee_with_one_site_when_the_target_guards_single_sites() {
    let tti = |single: bool| {
        x86_64_non_pie().with_tuning(llrm_mir::target::Tuning {
            inline_size_guard_at_size: true,
            inline_size_guard_single_site: single,
            ..llrm_mir::target::Tuning::PIN
        })
    };
    let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
    let price = |single: bool| {
        priced_params(COMPARE_LOOP, 0, false, CallerSize::OptSize, Some(&tti(single)), params, (1, false))
    };
    let (cost, threshold) = price(false);
    assert!(cost < threshold, "the premise: the pin inlines it ({cost} against {threshold})");
    let (cost, threshold) = price(true);
    assert!(cost >= threshold, "{cost} against {threshold}");
}

/// arm64 is a large platform: at speed it prices a callee as the pin does, so
/// the compare loop with two sites is inlined as on x86 (it was kept while
/// arm64 priced a byte in clocks; pattern_fill's `fillw` stayed a call at cost
/// -5).
#[test]
fn a_speed_caller_inlines_a_compare_loop_with_two_sites_as_the_pin_on_arm64() {
    let arm = llrm_arm64::tti::AArch64::of_cpu("generic").unwrap();
    let x86 = llrm_x86::tti::X86::of_cpu("x86-64").unwrap();
    for tti in [&x86 as &dyn llrm_mir::tti::Tti, &arm] {
        let (cost, threshold) = priced_with(COMPARE_LOOP, 0, false, CallerSize::Speed, Some(tti));
        assert!(cost < threshold, "{cost} against {threshold}");
    }
}

/// The speed guard leaves a dying callee whose growth is under one call's
/// size to the pin: its sites fold differently, and refusing one of them kept
/// the callee and a frame (recgcd -O2 on arm64: 136 B -> 188 B, +6.4% instrs).
#[test]
fn the_speed_guard_inlines_a_dying_callee_whose_growth_is_under_a_call() {
    let text = "define internal i32 @callee(i32 %a, i32 %b) nounwind {
entry:
  %r = srem i32 %a, %b
  %s = add i32 %r, %a
  %t = mul i32 %s, %b
  %u = xor i32 %t, %a
  %v = sub i32 %u, %b
  %w = or i32 %v, %r
  ret i32 %w
}
define i32 @f(i32 %n) {
entry:
  %p = call i32 @callee(i32 %n, i32 36)
  %q = call i32 @callee(i32 %n, i32 90)
  %s = add i32 %p, %q
  ret i32 %s
}
";
    let arm = llrm_arm64::tti::AArch64::of_cpu("generic").unwrap();
    let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
    let (cost, threshold) = priced_params(text, 0, false, CallerSize::Speed, Some(&arm), params, (2, true));
    assert!(cost < threshold, "{cost} against {threshold}");
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

/// A callee that dies with its calls is removed when they are all inlined, so
/// two calls of it grow the code only if `(2 - 1) * size > 2 * call`.
/// stack_arrays_copy -Os m64 kept two calls of a static 70-trip loop pair
/// (113 -> 170 B with the guard, 65 B inlined as main and the pin do).
#[test]
fn an_optsize_caller_inlines_a_dying_callee_when_all_its_calls_cost_no_more_than_the_callee_and_one_call() {
    let text = format!(
        "{GLOBAL}define i32 @callee(i32 %x) {{
b0:
  %a = load i32, ptr @g
  %b = add i32 %a, %x
  %c = mul i32 %b, %x
  %d = add i32 %c, %a
  store i32 %d, ptr @g
  ret i32 %d
}}

define i32 @f(i32 %y) {{
b0:
  %r = call i32 @callee(i32 %y)
  ret i32 %r
}}
"
    );
    let guarded = x86_real();
    let params = || InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
    let verdict = |calls, dies| {
        let (cost, threshold) =
            priced_params(&text, 0, false, CallerSize::OptSize, Some(&guarded), params(), (calls, dies));
        cost < threshold
    };
    assert!(!verdict(2, false), "premise: a callee that stays and outgrows its call is refused");
    assert!(verdict(2, true), "two calls of a callee that dies cost less than the callee and one call");
    assert!(!verdict(60, true), "sixty copies of it outgrow the callee and the calls");
}

/// At `-Os` on x86 the call's price left out the copy of a by-value argument
/// (`byvalue_struct_address` m64: `foo(struct baz x)` read 7 units against a
/// call of 6, so `foo` stayed and `main` built the 20-byte copy). The same
/// body is refused behind a pointer argument and inlined behind a by-value one.
#[test]
fn an_optsize_caller_counts_the_copy_of_a_byval_argument_in_the_call() {
    let text = |param: &str, arg: &str| {
        format!(
            "{GLOBAL}%S = type {{ i32, i32, i32, i32, i32 }}

define i32 @callee({param} %p) {{
b0:
  %a = load i32, ptr %p
  %b = load i32, ptr @g
  %c = add i32 %a, %b
  %d = mul i32 %c, %a
  %e = add i32 %d, %b
  store i32 %e, ptr @g
  ret i32 %e
}}

define i32 @f(ptr %q) {{
b0:
  %r = call i32 @callee({arg} %q)
  ret i32 %r
}}
"
        )
    };
    let x86 = x86_real();
    let params = || InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
    let verdict = |text: &str| {
        let (cost, threshold) = priced_params(text, 0, false, CallerSize::OptSize, Some(&x86), params(), (2, false));
        cost < threshold
    };
    assert!(!verdict(&text("ptr", "ptr")), "premise: a callee that stays and outgrows its call is refused");
    assert!(verdict(&text("ptr byval(%S)", "ptr byval(%S)")), "the 20-byte copy is part of the call");
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

/// D11-MEM: a callee of one 48-byte `memcpy` is one instruction in the pin's
/// count, under the call it replaces, so `them_dark` (QB m64 -Os)
/// copied three vector loads and stores for a 10-byte call (37 B against 10).
/// The guard counts the copy as the loads and stores it becomes: refused on
/// x86-64, where the three pieces are six units; a 4-byte copy, one piece,
/// stays inlined.
#[test]
fn an_optsize_caller_counts_a_constant_memcpy_as_its_loads_and_stores_on_x86() {
    let text = |bytes: u32| {
        format!(
            "@src = global [64 x i8] zeroinitializer
@dst = global [64 x i8] zeroinitializer
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)

define void @callee() {{
b0:
  call void @llvm.memcpy.p0.p0.i64(ptr @dst, ptr @src, i64 {bytes}, i1 false)
  ret void
}}

define void @f() {{
b0:
  call void @callee()
  ret void
}}
"
        )
    };
    let x64 = llrm_x86::tti::X86::of_cpu("x86-64")
        .unwrap()
        .with_tuning(llrm_mir::target::Tuning { inline_size_guard_at_size: true, ..llrm_mir::target::Tuning::PIN });
    let held = |text: &str| {
        let (cost, threshold) = priced_with(text, 0, false, CallerSize::OptSize, Some(&x64));
        cost < threshold
    };
    assert!(!held(&text(48)), "a 48-byte copy was inlined for a call that is smaller");
    assert!(held(&text(4)), "a one-piece copy stays inlined");
}

/// D11-MEM: a callee of two fills and a move of an unknown length is three
/// instructions in the pin's count, under the 9 units of its call, so QB's
/// `justify` went into `set` three times (`file.c` m64 -Os, 445 B against
/// gcc's 212). Each stays the call it is on x86-64: refused, while a callee of
/// one fill of the same kind is smaller than its call and stays inlined.
#[test]
fn an_optsize_caller_counts_a_variable_length_fill_as_the_call_it_stays_on_x86() {
    let text = |body: &str| {
        format!(
            "declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)

define internal void @callee(ptr %d, ptr %s, i64 %n) {{
b0:
{body}  ret void
}}

define void @f(ptr %d, ptr %s, i64 %n) {{
b0:
  call void @callee(ptr %d, ptr %s, i64 %n)
  ret void
}}
"
        )
    };
    let fill = "  call void @llvm.memset.p0.i64(ptr %d, i8 32, i64 %n, i1 false)\n";
    let slide = "  call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 %n, i1 false)\n";
    let x64 = llrm_x86::tti::X86::of_cpu("x86-64")
        .unwrap()
        .with_tuning(llrm_mir::target::Tuning { inline_size_guard_at_size: true, ..llrm_mir::target::Tuning::PIN });
    let held = |body: &str| {
        let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
        let (cost, threshold) =
            priced_params(&text(body), 0, false, CallerSize::OptSize, Some(&x64), params, (3, true));
        cost < threshold
    };
    assert!(!held(&format!("{fill}{slide}{fill}")), "two fills and a move were copied three times");
    assert!(held(&fill), "one fill, smaller than its call, stays inlined");
}

/// D11-MEM: the length of a fill or move that the call's constant argument
/// fixes is the constant it is at the site. `bench(63)` of QB's
/// `stack_arrays_copy` passes the length of a copy; with a length of 4 the
/// three intrinsics below are one store piece each and stay inlined, where an
/// unknown length is three calls' worth.
#[test]
fn an_optsize_caller_prices_a_fill_by_the_length_its_call_site_fixes_on_x86() {
    let text = |length: &str| {
        format!(
            "declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)

define internal void @callee(ptr %d, ptr %s, i64 %n) {{
b0:
  call void @llvm.memset.p0.i64(ptr %d, i8 32, i64 %n, i1 false)
  call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 %n, i1 false)
  call void @llvm.memset.p0.i64(ptr %d, i8 32, i64 %n, i1 false)
  ret void
}}

define void @f(ptr %d, ptr %s, i64 %m) {{
b0:
  call void @callee(ptr %d, ptr %s, i64 {length})
  ret void
}}
"
        )
    };
    let x64 = llrm_x86::tti::X86::of_cpu("x86-64")
        .unwrap()
        .with_tuning(llrm_mir::target::Tuning { inline_size_guard_at_size: true, ..llrm_mir::target::Tuning::PIN });
    let held = |length: &str| {
        let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
        let (cost, threshold) =
            priced_params(&text(length), 0, false, CallerSize::OptSize, Some(&x64), params, (3, true));
        cost < threshold
    };
    assert!(!held("%m"), "premise: an unknown length is three calls' worth");
    assert!(held("4"), "a length of 4 at the site is three single stores");
}

/// D11-MEM: a copy or fill between the callee's own stack objects is deleted
/// by SROA and `memcpyopt` once the site's constants reach it, so the guard
/// keeps the pin's one instruction for it: QB's `stack_arrays_copy` (m64 -Os,
/// 111 -> 168 B) kept `bench` out of line for a copy of two local arrays.
#[test]
fn an_optsize_caller_does_not_price_a_copy_between_the_callees_stack_objects_as_a_call_on_x86() {
    let text = |objects: &str| {
        format!(
            "declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)

define internal void @callee(ptr %p, ptr %q, i64 %n) {{
b0:
{objects}  call void @llvm.memset.p0.i64(ptr %d, i8 32, i64 %n, i1 false)
  call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 %n, i1 false)
  call void @llvm.memset.p0.i64(ptr %d, i8 32, i64 %n, i1 false)
  ret void
}}

define void @f(ptr %p, ptr %q, i64 %m) {{
b0:
  call void @callee(ptr %p, ptr %q, i64 %m)
  ret void
}}
"
        )
    };
    let local = "  %d = alloca [70 x i8]\n  %s = alloca [70 x i8]\n";
    let passed = "  %d = getelementptr i8, ptr %p, i64 0\n  %s = getelementptr i8, ptr %q, i64 0\n";
    let x64 = llrm_x86::tti::X86::of_cpu("x86-64")
        .unwrap()
        .with_tuning(llrm_mir::target::Tuning { inline_size_guard_at_size: true, ..llrm_mir::target::Tuning::PIN });
    let held = |objects: &str| {
        let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
        let (cost, threshold) =
            priced_params(&text(objects), 0, false, CallerSize::OptSize, Some(&x64), params, (3, true));
        cost < threshold
    };
    assert!(!held(passed), "premise: the same copies of passed pointers are three calls' worth");
    assert!(held(local), "copies between the callee's own arrays were priced as calls");
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
            callee_calls: 2,
            callee_dies: false,
            caller_recursive: false,
            callee_linear_recursive: false,
            caller_size: CallerSize::Speed,
            relative_frequency: None,
            cold_frequency: None,
            entry_weight: None,
            planned: None,
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
            callee_calls: 2,
            callee_dies: false,
            caller_recursive: false,
            callee_linear_recursive: false,
            caller_size: CallerSize::Speed,
            relative_frequency: None,
            cold_frequency: None,
            entry_weight: None,
            planned: None,
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
    let priced = InlineCost::Cost { cost: 40, threshold: 225, static_bonus: 0, blocked: false, hot: false };
    assert_eq!(
        crate::inlinecost::remark("_main@0", "_f@3", &priced, true),
        "'_f@3' inlined into '_main@0' with (cost=40, threshold=225)"
    );
    let dear = InlineCost::Cost { cost: 300, threshold: 225, static_bonus: 0, blocked: false, hot: false };
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

/// `@main`, whose only call (of an internal leaf) follows two checked loop
/// nests of depth 6: the pin reads it as cold.
fn after_a_checked_loop_nest() -> Module {
    let depth = 6;
    let nests = 2;
    let mut text = String::from(
        "declare void @trap() noreturn\n\n\
         define internal i16 @leaf(i16 %x) {\nentry:\n  %r = add i16 %x, 1\n  ret i16 %r\n}\n\n\
         define i16 @main(i16 %n, i16 %x) {\nentry:\n  br label %a0.h0\n\n",
    );
    for a in 0..nests {
        for k in 0..depth {
            let last = k == depth - 1;
            let pre = match (a, k) {
                (0, 0) => "entry".to_string(),
                (_, 0) => format!("a{}.out", a - 1),
                _ => format!("a{a}.p{k}"),
            };
            let body = if last { format!("a{a}.chk") } else { format!("a{a}.p{}", k + 1) };
            text += &format!(
                "a{a}.h{k}:\n  %a{a}.i{k} = phi i16 [ 0, %{pre} ], [ %a{a}.j{k}, %a{a}.l{k} ]\n  %a{a}.c{k} = icmp slt i16 %a{a}.i{k}, %n\n  br i1 %a{a}.c{k}, label %{body}, label %a{a}.x{k}\n\n"
            );
            if !last {
                text += &format!("a{a}.p{}:\n  br label %a{a}.h{}\n\n", k + 1, k + 1);
            }
            let next = if k == 0 { format!("a{a}.out") } else { format!("a{a}.l{}", k - 1) };
            text += &format!(
                "a{a}.x{k}:\n  br label %{next}\n\na{a}.l{k}:\n  %a{a}.j{k} = add i16 %a{a}.i{k}, 1\n  br label %a{a}.h{k}\n\n"
            );
        }
        let onward = if a + 1 < nests { format!("a{}.h0", a + 1) } else { "call".to_string() };
        let inner = depth - 1;
        text += &format!(
            "a{a}.chk:\n  %a{a}.ok = icmp ult i16 %a{a}.i{inner}, 64\n  br i1 %a{a}.ok, label %a{a}.l{inner}, label %a{a}.fail, !prof !0\n\n\
             a{a}.fail:\n  call void @trap()\n  unreachable\n\na{a}.out:\n  br label %{onward}\n\n"
        );
    }
    text +=
        "call:\n  %v = call i16 @leaf(i16 %x)\n  ret i16 %v\n}\n\n!0 = !{!\"branch_weights\", i32 1048575, i32 1}\n";
    parsed(&text)
}

/// Two successive nests of six loops, each with a bounds check in the
/// innermost (`!prof {1048575, 1}` to a `noreturn` sink): the failure edges
/// leak mass in every pass, so the call after them read 0.0011 of the entry
/// (1.0 with no checks), cold, and a sole call to a local function was held to
/// threshold 45 and not inlined (Nib `jpeg_single` -O1: 342 B).
#[test]
fn test_a_site_after_a_checked_loop_nest_is_not_cold_for_the_checks_sake() {
    let module = after_a_checked_loop_nest();
    let main = module.globals.iter().position(|one| one.name.as_deref() == Some("main")).unwrap();
    let mut decided = Vec::new();
    crate::inlinecost::sites(
        &module,
        &DataLayout::default(),
        llrm_mir::context::GlobalId(main as u32),
        &crate::inline::call_counts(&module),
        &Default::default(),
        &Default::default(),
        &Default::default(),
        InlineParams::from_level(1),
        CallerSize::Speed,
        false,
        None,
        Some(&mut decided),
    );
    let [(_, _, InlineCost::Cost { threshold, .. })] = decided[..] else { panic!("one priced site: {decided:?}") };
    assert_eq!(threshold, 45, "the pin's verdict stays with the flag off");
    let mut lifted = Vec::new();
    crate::inlinecost::sites(
        &module,
        &DataLayout::default(),
        llrm_mir::context::GlobalId(main as u32),
        &crate::inline::call_counts(&module),
        &Default::default(),
        &Default::default(),
        &Default::default(),
        InlineParams { cold_site_ignores_sinks: true, ..InlineParams::from_level(1) },
        CallerSize::Speed,
        false,
        None,
        Some(&mut lifted),
    );
    let [(_, _, InlineCost::Cost { threshold, .. })] = lifted[..] else { panic!("one priced site: {lifted:?}") };
    assert!(threshold > 45, "held to the cold threshold: {threshold}");
}

/// suggest.c -Os (x86-64): a function called once, from a site the pin reads as
/// cold, stayed out of line (threshold 45, no last-call bonus) where gcc's
/// called-once inlining removes it. With the bonus kept at cold sites of an -Os
/// caller the site is taken; at -O2 and off, the pin's verdict.
#[test]
fn test_the_last_call_bonus_stays_at_a_cold_site_of_an_os_caller_where_the_target_says_so() {
    let module = after_a_checked_loop_nest();
    let main = module.globals.iter().position(|one| one.name.as_deref() == Some("main")).unwrap();
    let cost_of = |params: InlineParams, size: CallerSize| {
        let mut decided = Vec::new();
        crate::inlinecost::sites(
            &module,
            &DataLayout::default(),
            llrm_mir::context::GlobalId(main as u32),
            &crate::inline::call_counts(&module),
            &Default::default(),
            &Default::default(),
            &Default::default(),
            params,
            size,
            false,
            None,
            Some(&mut decided),
        );
        let [(_, _, InlineCost::Cost { cost, .. })] = decided[..] else { panic!("one priced site: {decided:?}") };
        cost
    };
    let on = InlineParams { last_call_in_cold_site: true, ..InlineParams::from_level(1) };
    let pin = cost_of(InlineParams::from_level(1), CallerSize::OptSize);
    assert_eq!(cost_of(on, CallerSize::OptSize), pin - 15_000, "the bonus stays for an -Os caller");
    let speed = cost_of(InlineParams::from_level(1), CallerSize::Speed);
    assert_eq!(cost_of(on, CallerSize::Speed), speed, "-O2: the pin's");
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
        linear_recursive: &none,
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
    let one = crate::inlinecost::decide(&module, &layout, main, calls[0], &context, &frequencies, None)
        .expect("leaf has a body");
    let mut listed = Vec::new();
    crate::inlinecost::sites(
        &module,
        &layout,
        main,
        &counts,
        &none,
        &none,
        &none,
        InlineParams::from_level(2),
        CallerSize::Speed,
        false,
        None,
        Some(&mut listed),
    );
    assert_eq!((one.inline, &one.cost), (true, &listed[0].2));
    assert_eq!(crate::inlinecost::decide(&module, &layout, main, calls[1], &context, &frequencies, None), None);
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
        callee_calls: 1,
        callee_dies: false,
        caller_recursive: false,
        callee_linear_recursive: false,
        caller_size: CallerSize::Speed,
        relative_frequency: None,
        cold_frequency: None,
        entry_weight: None,
        planned: None,
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
    assert!(!InlineCost::Cost { cost: 0, threshold: 0, static_bonus: 0, blocked: false, hot: false }.is_ok());
    assert!(InlineCost::Cost { cost: -1, threshold: 0, static_bonus: 0, blocked: false, hot: false }.is_ok());
    assert!(!InlineCost::Cost { cost: 75, threshold: 75, static_bonus: 0, blocked: false, hot: false }.is_ok());
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
            callee_calls: 1,
            callee_dies: false,
            caller_recursive: false,
            callee_linear_recursive: false,
            caller_size: CallerSize::Speed,
            relative_frequency: None,
            cold_frequency: None,
            entry_weight: None,
            planned: None,
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

/// A target that states only its `inline_policy`, and whether its rate binds at
/// -O1.
struct PolicyOnly(llrm_mir::tti::InlinePolicy, Option<bool>, bool);

impl llrm_mir::tti::Tti for PolicyOnly {
    fn inline_policy(&self) -> llrm_mir::tti::InlinePolicy {
        self.0
    }

    fn speed_inline_rate_at_o1(&self) -> bool {
        self.1.unwrap_or_else(|| llrm_mir::tti::Basic.speed_inline_rate_at_o1())
    }

    fn inline_hints_known_loop_bound(&self) -> bool {
        self.2
    }

    fn cmp_sel_cost(
        &self,
        kind: llrm_mir::tti::CmpSel,
        ty: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cmp_sel_cost(kind, ty, cost)
    }

    fn memory_op_cost(
        &self,
        op: llrm_mir::tti::MemoryOp,
        ty: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
        volatile: bool,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.memory_op_cost(op, ty, cost, volatile)
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cf_instr_cost(phi, kind)
    }

    fn int_imm_cost(
        &self,
        value: i128,
        bits: u32,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.int_imm_cost(value, bits)
    }

    fn int_imm_cost_inst(
        &self,
        user: llrm_mir::tti::ImmUser,
        index: u32,
        value: i128,
        bits: u32,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.int_imm_cost_inst(user, index, value, bits)
    }

    fn is_truncate_free(
        &self,
        from: llrm_mir::tti::Ty,
        to: llrm_mir::tti::Ty,
    ) -> bool {
        llrm_mir::tti::Basic.is_truncate_free(from, to)
    }

    fn is_legal_addressing_mode(
        &self,
        global: bool,
        offset: i64,
        base: bool,
        scale: i64,
    ) -> bool {
        llrm_mir::tti::Basic.is_legal_addressing_mode(global, offset, base, scale)
    }

    fn cast_cost(
        &self,
        kind: llrm_mir::tti::CastKind,
        from: llrm_mir::tti::Ty,
        to: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cast_cost(kind, from, to, cost)
    }

    fn arithmetic_cost(
        &self,
        op: llrm_mir::tti::ArithOp,
        ty: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
        lhs: llrm_mir::tti::OperandInfo,
        rhs: llrm_mir::tti::OperandInfo,
    ) -> llrm_mir::tti::Cost {
        use llrm_mir::tti::ArithOp;
        // a shift by a variable amount is a library call (z80 `__ashlhi3`), by
        // a constant a few instructions
        if cost == llrm_mir::tti::CostKind::CodeSize
            && matches!(op, ArithOp::Shl | ArithOp::LShr | ArithOp::AShr)
            && !rhs.constant
        {
            return llrm_mir::tti::Cost::of(12);
        }
        llrm_mir::tti::Basic.arithmetic_cost(op, ty, cost, lhs, rhs)
    }
}

/// The inliner reads the target's policy and no other inline hook: a target
/// that returns a rate from `inline_policy` alone has a speed caller's inline
/// of a compare loop with two sites refused, and `InlinePolicy::PIN` leaves it
/// to the pin.
#[test]
fn the_inliner_reads_the_inline_policy_of_the_target() {
    use llrm_mir::tti::{InlinePolicy, Tti};
    let arm = llrm_arm64::tti::AArch64::of_cpu("generic").unwrap();
    let pin = PolicyOnly(InlinePolicy::PIN, Some(true), false);
    assert_eq!(arm.inline_policy().by_rate, 0, "AArch64 inlines as the pin does");
    let priced = PolicyOnly(InlinePolicy { by_rate: 62_500, ..InlinePolicy::PIN }, Some(true), false);
    let (cost, threshold) = priced_with(COMPARE_LOOP, 0, false, CallerSize::Speed, Some(&pin));
    assert!(cost < threshold, "the premise: the pin inlines it ({cost} against {threshold})");
    let (cost, threshold) = priced_with(COMPARE_LOOP, 0, false, CallerSize::Speed, Some(&priced));
    assert!(cost >= threshold, "{cost} against {threshold}");
}

/// m64 -O2 kept `bench_crc(0)` a call (21 executed instructions became 561):
/// the target's bar of 150 refused a cost of 180 the level's own 225 takes, and
/// a refusal by the bar alone never reached the priced trial. A speed site the
/// cut bar refuses and the own bar takes is `blocked`, for the trial to decide;
/// without the own bar it is a plain refusal.
#[test]
fn a_site_over_the_cut_bar_and_under_the_levels_own_is_blocked_for_the_trial() {
    let own = InlineParams::from_level(2).default_threshold;
    let (cost, _) = priced(COMPARE_LOOP, 0, false, CallerSize::Speed);
    assert!(0 < cost && cost < own, "the fixture's cost {cost} is not between 0 and {own}");
    let blocked = |cut_from, o1_bound, size| {
        let params = InlineParams { default_threshold: cost, cut_from, o1_bound, ..InlineParams::from_level(2) };
        match inline_cost_hot(COMPARE_LOOP, 0, false, size, None, params, (2, false), None) {
            InlineCost::Cost { blocked, .. } => blocked,
            other => panic!("{other:?}"),
        }
    };
    assert!(blocked(Some(own), false, CallerSize::Speed), "the cut bar's refusal is not a trial");
    assert!(!blocked(None, false, CallerSize::Speed), "a bar that was not cut made a trial");
    assert!(!blocked(Some(own), true, CallerSize::Speed), "-O1 takes the pin's verdict, no trial");
    // A trial costs the caller's cleanup list again: a call with an argument
    // that is not a constant has nothing to fold whole (+4.55% compile at -O2
    // when every refused site was tried).
    let unknown = COMPARE_LOOP
        .replace("define void @f() {", "define void @f(i32 %x) {")
        .replace("@callee(i32 noundef 1)", "@callee(i32 noundef %x)");
    let params = InlineParams { default_threshold: cost, cut_from: Some(own), ..InlineParams::from_level(2) };
    match inline_cost_hot(&unknown, 0, false, CallerSize::Speed, None, params, (2, false), None) {
        InlineCost::Cost { blocked, .. } => assert!(!blocked, "a call of an unknown argument made a trial"),
        other => panic!("{other:?}"),
    }
}

/// z80 -O1 inlined with the -O2 byte price (1 B = 3 T): 5 cells bigger or
/// slower than -O1 must be (loop_fusion +43,813 T). A rate that does not bind
/// at -O1 leaves the pin's verdict there and refuses at -O2.
#[test]
fn a_rate_that_binds_from_o2_leaves_o1_to_the_pin() {
    use llrm_mir::tti::InlinePolicy;
    let policy = InlinePolicy { by_rate: 250_000, ..InlinePolicy::PIN };
    let from_o2 = PolicyOnly(policy, Some(false), false);
    let always = PolicyOnly(policy, Some(true), false);
    let verdict = |tti: &dyn llrm_mir::tti::Tti, o1_bound| {
        let params = InlineParams { compute_full_inline_cost: true, o1_bound, ..InlineParams::from_level(2) };
        let (cost, threshold) = priced_params(COMPARE_LOOP, 0, false, CallerSize::Speed, Some(tti), params, (2, false));
        cost < threshold
    };
    assert!(!verdict(&from_o2, false), "-O2 is priced");
    assert!(verdict(&from_o2, true), "-O1 keeps the pin's verdict");
    assert!(!verdict(&always, true), "a rate that binds at -O1 still does");
}

/// m32 -O1 moved (x_jpeg_fixed 5647 -> 6028 B) when a target that stated only a
/// rate inherited the default "the rate binds at -O1": -O1 is strict, so a rate
/// is opt-in there.
#[test]
fn a_target_stating_only_a_rate_leaves_o1_to_the_pin() {
    use llrm_mir::tti::InlinePolicy;
    let rate_only = PolicyOnly(InlinePolicy { by_rate: 250_000, ..InlinePolicy::PIN }, None, false);
    let params = InlineParams { compute_full_inline_cost: true, o1_bound: true, ..InlineParams::from_level(2) };
    let (cost, threshold) =
        priced_params(COMPARE_LOOP, 0, false, CallerSize::Speed, Some(&rate_only), params, (2, false));
    assert!(cost < threshold, "the rate refused a copy at -O1");
}

/// DEVIATE wasm SZ1 reaches -O1: with the rate off a speed caller holds a
/// growing inline the pin's `default<O1>` takes (jpeg_fixed -O1 22099 B against
/// the pin's 26245). Closing #8527 kept it: undoing it moved 33 wasm -O1 rows
/// up by 28 525 B.
#[test]
fn the_size_rule_holds_a_growing_inline_in_a_speed_caller_at_o1() {
    use llrm_mir::tti::{InlinePolicy, SizeRule};
    let policy = InlinePolicy {
        by_size: Some(SizeRule { min_sites: 2, yields_to_negative_cost: false, yields_to_inlinehint: false }),
        by_rate: 0,
    };
    let tti = PolicyOnly(policy, Some(false), false);
    let params = InlineParams { compute_full_inline_cost: true, o1_bound: true, ..InlineParams::from_level(2) };
    let (cost, threshold) = priced_params(COMPARE_LOOP, 0, false, CallerSize::Speed, Some(&tti), params, (2, false));
    assert!(cost >= threshold, "-O1 took a growing inline the size rule holds");
}

const BOUNDED_LOOP: &str = "@dst = internal global [1200 x i8] zeroinitializer
@ref = internal global [1200 x i8] zeroinitializer
@sum = internal global i16 0
define internal void @callee(i16 %n) nounwind {
entry:
  br label %loop

loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %pa = getelementptr inbounds nuw i8, ptr @dst, i16 %i
  %va = load i8, ptr %pa, align 1
  %pb = getelementptr inbounds nuw i8, ptr @ref, i16 %i
  %vb = load i8, ptr %pb, align 1
  %m = mul i8 %va, %vb
  %w = zext i8 %m to i16
  %old = load i16, ptr @sum, align 2
  %new = add i16 %old, %w
  store i16 %new, ptr @sum, align 2
  %next = add nuw nsw i16 %i, 1
  %end = icmp eq i16 %next, %n
  br i1 %end, label %done, label %loop

done:
  ret void
}

define void @f(i16 %k) {
b0:
  call void @callee(i16 noundef 10)
  call void @callee(i16 noundef %k)
  ret void
}
";

/// z80 loop_fusion -O3: `same(10)` was no longer copied into `main` at 3 T a
/// byte, and the loops it holds no longer met their constant trip count (39,020
/// T became 299,430 T). A copy that gives a loop a constant bound is exempt
/// from the rate, where the target says so; an unknown bound is not.
#[test]
fn a_constant_loop_bound_exempts_a_copy_from_the_rate_where_the_target_hints_it() {
    use llrm_mir::tti::InlinePolicy;
    let policy = InlinePolicy { by_rate: 250_000, ..InlinePolicy::PIN };
    let (hints, silent) = (PolicyOnly(policy, Some(true), true), PolicyOnly(policy, Some(true), false));
    let inlined = |tti: &dyn llrm_mir::tti::Tti, at| {
        let (cost, threshold) = priced_with(BOUNDED_LOOP, at, false, CallerSize::Speed, Some(tti));
        cost < threshold
    };
    assert!(!inlined(&silent, 0), "the premise: the rate refuses the copy");
    assert!(inlined(&hints, 0), "a constant bound");
    assert!(!inlined(&hints, 1), "an unknown bound");
}

/// A target that states its rules through the older hooks gets the policy they
/// make; one that states nothing gets the pin's.
#[test]
fn the_default_inline_policy_is_made_from_the_hooks_a_target_states() {
    use llrm_mir::tti::{InlinePolicy, SizeRule, Tti};
    assert_eq!(llrm_mir::tti::Basic.inline_policy(), InlinePolicy::PIN);
    let arm = llrm_arm64::tti::AArch64::of_cpu("generic").unwrap();
    // At -Os an inline must not grow the code (#8228): the size rule is the
    // hooks'.
    let by_size = SizeRule {
        min_sites: arm.opt_size_inline_guard_min_sites(),
        yields_to_negative_cost: arm.opt_size_inline_guard_yields_to_negative_cost(),
        yields_to_inlinehint: arm.opt_size_inline_guard_yields_to_inlinehint(),
    };
    assert!(arm.opt_size_inline_by_code_size());
    assert_eq!(
        arm.inline_policy(),
        InlinePolicy { by_size: Some(by_size), by_rate: arm.speed_inline_milliclocks_per_byte() }
    );
    assert_eq!(arm.inline_policy().by_rate, 0, "arm64 is a large platform: the pin's threshold alone");
    let x86 = |guard: bool| {
        llrm_x86::tti::X86::of_cpu("x86-64")
            .unwrap()
            .with_tuning(llrm_mir::target::Tuning { inline_size_guard_at_size: guard, ..llrm_mir::target::Tuning::PIN })
    };
    assert_eq!(x86(false).inline_policy().by_size, None);
    assert!(x86(true).inline_policy().by_size.is_some());
}

/// `LLRM_DEBUG=inline` was documented and printed nothing (jpeg_single -O1:
/// `emit` cost 90 against threshold 90, not the pin's 225, was invisible). The
/// line it prints for a verdict reads as clang's `-Rpass=inline` does.
#[test]
fn a_verdict_is_described_with_its_cost_and_threshold() {
    use crate::inlinecost::describe;
    let cost = InlineCost::Cost { cost: 90, threshold: 225, static_bonus: 0, blocked: false, hot: false };
    assert_eq!(describe(&cost), "cost=90 threshold=225 inline");
    let over = InlineCost::Cost { cost: 90, threshold: 90, static_bonus: 0, blocked: false, hot: false };
    assert_eq!(describe(&over), "cost=90 threshold=90 no");
    assert_eq!(describe(&InlineCost::Never("recursive".into())), "never (recursive)");
}

const ROTATE: &str = "define internal i16 @callee(i16 %x, i16 %n) nounwind {
b0:
  %a = and i16 %n, 15
  %l = shl i16 %x, %a
  %b = sub i16 0, %n
  %c = and i16 %b, 15
  %r = lshr i16 %x, %c
  %o = or i16 %l, %r
  ret i16 %o
}

define i16 @f(i16 %x, i16 %k) nounwind {
b0:
  %p = call i16 @callee(i16 %x, i16 noundef 5)
  %q = call i16 @callee(i16 %p, i16 %k)
  ret i16 %q
}
";

/// z80 `rotate_small` -O1 576 -> 922 B once `fshl` had a price: the size rule
/// priced `rotl16(x, 5)` with the callee's variable amount (two library calls,
/// 27 B against a 10 B call) and kept it out of line with `rotl16(x, n)`. The
/// copy at a constant amount is three instructions; the rule counts it so
/// (`isGEPFree` prices with the site's constants).
#[test]
fn the_size_rule_prices_a_callee_with_the_constants_of_the_site() {
    use llrm_mir::tti::{InlinePolicy, SizeRule};
    let policy = InlinePolicy {
        by_size: Some(SizeRule { min_sites: 2, yields_to_negative_cost: false, yields_to_inlinehint: false }),
        by_rate: 0,
    };
    let tti = PolicyOnly(policy, None, false);
    let inlined = |at| {
        let (cost, threshold) = priced_with(ROTATE, at, false, CallerSize::OptSize, Some(&tti));
        cost < threshold
    };
    assert!(inlined(0), "a constant amount: three instructions against a call");
    assert!(!inlined(1), "a variable amount keeps the call");
}

/// A float taken to an integer through `llvm.experimental.fptosi.checked` was
/// charged as a call (the `llrm.`-named first version: 25 and its arguments),
/// so `clamp` in bench/jpeg_single.nib stayed a call in the hot loop: m16 -O2
/// +7.5% clocks. The intrinsic is lowered where it is called, as `llvm.lrint`
/// is: it costs what the plain `fptosi` it replaces cost, within an instruction
/// or two, not a `CALL_PENALTY` more.
#[test]
fn test_a_checked_float_to_integer_is_not_charged_as_a_call() {
    let price = |body: &str, declaration: &str| {
        let text = format!(
            "{GLOBAL}{declaration}
define internal i32 @callee(double %x) {{
b0:
{body}
}}

define i32 @f(double %y) {{
b0:
  %r = call i32 @callee(double %y)
  ret i32 %r
}}
"
        );
        priced(&text, 0, false, CallerSize::Speed).0
    };
    let plain = price("  %v = fptosi double %x to i32\n  ret i32 %v", "");
    let checked = price(
        "  %p = call { i32, i1 } @llvm.experimental.fptosi.checked.i32.f64(double %x)\n  %v = extractvalue { i32, i1 } %p, 0\n  ret i32 %v",
        "declare { i32, i1 } @llvm.experimental.fptosi.checked.i32.f64(double)\n",
    );
    assert!(checked - plain < crate::inlinecost::CALL_PENALTY, "checked {checked} against plain {plain}");
}

/// x_jpeg_fixed -O2 (arm64): `emit(out, data, n)` with `out` a constant IPSCCP
/// left unread. `deadargelim` removes it after the walk, so the guard prices
/// the call it will contain: the callee is held exactly as the same callee
/// without the parameter. Priced with it, the call read 3 units, growth fell
/// under the call and the cold site inlined (+72 B, +151 k instructions).
#[test]
fn test_the_speed_guard_prices_a_call_without_the_arguments_nothing_reads() {
    let arm = llrm_arm64::tti::AArch64::of_cpu("generic").unwrap();
    let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
    let held = |text: &str| {
        let (cost, threshold) = priced_params(text, 0, false, CallerSize::Speed, Some(&arm), params.clone(), (2, true));
        cost >= threshold
    };
    // A body of `ops` instructions: the parameter that is unread flips no
    // verdict at any size.
    for ops in 2..16 {
        let mut body = String::from("  %v0 = load i32, ptr @g\n");
        for at in 1..ops {
            body += &format!("  %v{at} = add i32 %v{}, %x\n", at - 1);
        }
        body += &format!("  store i32 %v{}, ptr @g\n  ret void\n}}\n", ops - 1);
        let with = format!(
            "{GLOBAL}define internal void @callee(i32 %unread, i32 %x) {{\nb0:\n{body}
define void @f(i32 %y) {{\nb0:\n  call void @callee(i32 1, i32 %y)\n  call void @callee(i32 1, i32 3)\n  ret void\n}}\n"
        );
        let without = format!(
            "{GLOBAL}define internal void @callee(i32 %x) {{\nb0:\n{body}
define void @f(i32 %y) {{\nb0:\n  call void @callee(i32 %y)\n  call void @callee(i32 3)\n  ret void\n}}\n"
        );
        assert_eq!(held(&with), held(&without), "{ops} instructions");
    }
}

/// `PolicyOnly` with a byte-priced call sequence (8 B and 4 B an argument), as
/// m68k and 6502 state theirs.
struct Sequenced(PolicyOnly);

impl llrm_mir::tti::Tti for Sequenced {
    fn inline_policy(&self) -> llrm_mir::tti::InlinePolicy {
        self.0.inline_policy()
    }

    fn call_sequence_cost(
        &self,
        shape: &llrm_mir::tti::CallShape,
        kind: llrm_mir::tti::CostKind,
    ) -> Option<llrm_mir::tti::Cost> {
        (kind == llrm_mir::tti::CostKind::CodeSize)
            .then(|| llrm_mir::tti::Cost::of(8 + 4 * shape.arguments.len() as i64))
    }

    fn cmp_sel_cost(
        &self,
        kind: llrm_mir::tti::CmpSel,
        ty: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        self.0.cmp_sel_cost(kind, ty, cost)
    }

    fn memory_op_cost(
        &self,
        op: llrm_mir::tti::MemoryOp,
        ty: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
        volatile: bool,
    ) -> llrm_mir::tti::Cost {
        self.0.memory_op_cost(op, ty, cost, volatile)
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        self.0.cf_instr_cost(phi, kind)
    }

    fn int_imm_cost(
        &self,
        value: i128,
        bits: u32,
    ) -> llrm_mir::tti::Cost {
        self.0.int_imm_cost(value, bits)
    }

    fn int_imm_cost_inst(
        &self,
        user: llrm_mir::tti::ImmUser,
        index: u32,
        value: i128,
        bits: u32,
    ) -> llrm_mir::tti::Cost {
        self.0.int_imm_cost_inst(user, index, value, bits)
    }

    fn is_truncate_free(
        &self,
        from: llrm_mir::tti::Ty,
        to: llrm_mir::tti::Ty,
    ) -> bool {
        self.0.is_truncate_free(from, to)
    }

    fn is_legal_addressing_mode(
        &self,
        global: bool,
        offset: i64,
        base: bool,
        scale: i64,
    ) -> bool {
        self.0.is_legal_addressing_mode(global, offset, base, scale)
    }

    fn cast_cost(
        &self,
        kind: llrm_mir::tti::CastKind,
        from: llrm_mir::tti::Ty,
        to: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        self.0.cast_cost(kind, from, to, cost)
    }

    fn arithmetic_cost(
        &self,
        op: llrm_mir::tti::ArithOp,
        ty: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
        lhs: llrm_mir::tti::OperandInfo,
        rhs: llrm_mir::tti::OperandInfo,
    ) -> llrm_mir::tti::Cost {
        self.0.arithmetic_cost(op, ty, cost, lhs, rhs)
    }
}

/// m68k x_idioms 68000 -Os: `sumlong(L, 40)` twice kept its out-of-line copy
/// until `deadargelim` ran after the walk, then its unread `n` was priced as an
/// argument push (call 18 B against 8), the -Os guard saw the inline as no
/// growth and the program grew +10 B. A target with a call sequence must price
/// the call without the unread argument.
#[test]
fn test_the_size_guard_prices_a_call_sequence_without_the_arguments_nothing_reads() {
    use llrm_mir::tti::{InlinePolicy, SizeRule};
    let policy = InlinePolicy {
        by_size: Some(SizeRule { min_sites: 1, yields_to_negative_cost: false, yields_to_inlinehint: false }),
        by_rate: 0,
    };
    let tti = Sequenced(PolicyOnly(policy, None, false));
    let params =
        InlineParams { compute_full_inline_cost: true, opt_size_threshold: 100_000, ..InlineParams::from_level(2) };
    let held = |text: &str| {
        let (cost, threshold) =
            priced_params(text, 0, false, CallerSize::OptSize, Some(&tti), params.clone(), (2, true));
        cost >= threshold
    };
    for ops in 2..48 {
        let mut body = String::from("  %v0 = load i32, ptr @g\n");
        for at in 1..ops {
            body += &format!("  %v{at} = add i32 %v{}, %x\n", at - 1);
        }
        body += &format!("  store i32 %v{}, ptr @g\n  ret void\n}}\n", ops - 1);
        let with = format!(
            "{GLOBAL}define internal void @callee(i32 %unread, i32 %x) {{\nb0:\n{body}
define void @f(i32 %y) {{\nb0:\n  call void @callee(i32 1, i32 %y)\n  call void @callee(i32 1, i32 3)\n  ret void\n}}\n"
        );
        let without = format!(
            "{GLOBAL}define internal void @callee(i32 %x) {{\nb0:\n{body}
define void @f(i32 %y) {{\nb0:\n  call void @callee(i32 %y)\n  call void @callee(i32 3)\n  ret void\n}}\n"
        );
        assert_eq!(held(&with), held(&without), "{ops} instructions");
    }
}

/// jpeg_fixed -O1 (arm64): `tmul`, a 5-instruction static callee with 6 sites,
/// stayed a call in the idct loops (frequency 29,791 times the caller's
/// entry): 6.9M of the kernel's 25.5M instructions against clang's 19.8M.
/// The -O1 guard holds a growing inline; a locally hot site (the pin's
/// `HotCallSiteRelFreq`) whose growth is within the call it removes inlines.
/// A cold site stays held, and so does a target that states no budget.
#[test]
fn a_hot_o1_site_inlines_a_callee_that_grows_the_code_by_less_than_a_call() {
    let text = "define internal i64 @callee(i64 %a, i64 %b) nounwind {
entry:
  %m = mul i64 %a, %b
  %s = ashr i64 %m, 63
  %l = lshr i64 %s, 54
  %t = add i64 %m, %l
  %u = xor i64 %t, %a
  %v = add i64 %u, %b
  %z = lshr i64 %v, 3
  %r = ashr i64 %z, 10
  ret i64 %r
}
define i64 @f(i64 %x, i64 %y) {
entry:
  %p = call i64 @callee(i64 %x, i64 %y)
  %q = call i64 @callee(i64 %p, i64 %y)
  %s = add i64 %p, %q
  ret i64 %s
}
";
    let arm = llrm_arm64::tti::AArch64::of_cpu("generic").unwrap();
    let params = InlineParams { compute_full_inline_cost: true, o1_bound: true, ..InlineParams::from_level(2) };
    let at = |tti: &dyn llrm_mir::tti::Tti, frequency| {
        let (cost, threshold) =
            priced_hot(text, 0, false, CallerSize::Speed, Some(tti), params.clone(), (6, true), Some(frequency));
        cost < threshold
    };
    assert!(at(&arm, 29_791.0), "hot: inlined");
    assert!(!at(&arm, 1.0), "cold: held");
    let x86 = llrm_x86::tti::X86::of_cpu("x86-64").unwrap();
    assert!(at(&x86, 29_791.0), "a target with no guard: the pin's verdict");
}

/// A float operation or cast on a target with no legal `FADD` is a library
/// call: `visitBinaryOperator` and `visitCastInst` add `CALL_PENALTY` where
/// `getFPOpCost` is `TCC_Expensive` (InlineCost.cpp:1959, :2317). Before, the
/// m68k priced a soft-float `fadd` at 5 like an add, so a float-heavy callee
/// was inlined at sites where the pin refuses it.
#[test]
fn test_a_float_operation_on_a_soft_float_target_costs_a_call() {
    let text = format!(
        "{GLOBAL}define internal double @callee(double %x, double %y) {{
b0:
  %a = fadd double %x, %y
  %b = sitofp i32 3 to double
  %c = fmul double %a, %y
  ret double %c
}}

define double @f(double %p, double %q) {{
b0:
  %r = call double @callee(double %p, double %q)
  %s = call double @callee(double %r, double %q)
  ret double %s
}}
"
    );
    let hard = crate::testing::Flagged::default();
    let soft = crate::testing::Flagged { soft_float: true, ..Default::default() };
    let (hard_cost, _) = priced_with(&text, 0, false, CallerSize::Speed, Some(&hard));
    let (soft_cost, _) = priced_with(&text, 0, false, CallerSize::Speed, Some(&soft));
    // the fadd, the fmul (the constant sitofp folds)
    assert_eq!(soft_cost - hard_cost, 2 * crate::inlinecost::CALL_PENALTY, "hard {hard_cost}, soft {soft_cost}");
}

/// An indirect call through an argument the site makes a function is priced
/// as the pin's `onLoweredCall` does: the target pretend-inlined at
/// `IndirectCallThreshold`, what that leaves of 100 comes off the cost. We took
/// the unknown call's 25 (QB `arry_each_slot` into `arry_free`: cost 290
/// against the pin's 170, so the walk over each array slot stayed a call and
/// ownf_free ran +5.6% instructions over clang -O2). `opt`: cost=-190,
/// threshold=225.
#[test]
fn test_an_indirect_call_of_a_known_function_is_priced_as_the_pin_does() {
    let text = format!(
        "{GLOBAL}define internal void @target(ptr %p) {{
b0:
  %v = load i32, ptr %p
  %w = add i32 %v, 1
  store i32 %w, ptr %p
  ret void
}}

define void @callee(ptr %p, ptr %fn, i32 %n) {{
b0:
  br label %loop

loop:
  %i = phi i32 [ 0, %b0 ], [ %j, %loop ]
  call void %fn(ptr %p)
  %j = add i32 %i, 1
  %c = icmp ult i32 %j, %n
  br i1 %c, label %loop, label %done

done:
  ret void
}}

define void @f(ptr %p, i32 %n) {{
b0:
  call void @callee(ptr %p, ptr @target, i32 %n)
  call void @callee(ptr %p, ptr @target, i32 %n)
  ret void
}}
"
    );
    assert_eq!(priced(&text, 0, false, CallerSize::Speed), (-190, 225));
}

/// pfpu.c `control(cw)` (`fesetround(modes[cw >> 10 & 3])`) was kept out of
/// line at -Os on arm64 for six constant-argument sites: the guard counted the
/// table load (3 units against a call's 2), though the load folds once the
/// argument is a constant. The pin inlines it at cost 5 against 75; pfpu.o was
/// +124 B over it.
#[test]
fn an_optsize_caller_inlines_a_callee_whose_table_load_folds_on_arm64() {
    let text = "@modes = internal constant [4 x i32] [i32 0, i32 4194304, i32 8388608, i32 12582912]
declare i32 @fesetround(i32)

define internal void @callee(i16 %cw) {
b0:
  %s = lshr i16 %cw, 10
  %z = zext i16 %s to i64
  %p = getelementptr inbounds i32, ptr @modes, i64 %z
  %m = load i32, ptr %p
  %r = call i32 @fesetround(i32 %m)
  ret void
}

define void @f() {
b0:
  call void @callee(i16 0)
  call void @callee(i16 1024)
  call void @callee(i16 2048)
  ret void
}
";
    let arm = llrm_arm64::tti::AArch64::of_cpu("generic").unwrap();
    let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
    let (cost, threshold) = priced_params(text, 0, false, CallerSize::OptSize, Some(&arm), params, (6, true));
    assert!(cost < threshold, "{cost} against {threshold}");
}

/// KEEP-HINTED-CALLS prices bytes alone, so the trial that puts back the
/// calls an `inlinehint` let in kept them in a computed-goto dispatch loop too:
/// QuickrBasic `exec.c` -Os, `sng_math` 528.8M instructions against 288.7M with
/// the calls inlined. A site hotter than its entry is the speed side's: the
/// hint's yield stands there in the trial, and is not noted for it.
#[test]
fn a_hinted_call_in_a_loop_is_inlined_in_the_keep_calls_trial() {
    use llrm_mir::tti::{InlinePolicy, SizeRule};
    let policy = InlinePolicy {
        by_size: Some(SizeRule { min_sites: 2, yields_to_negative_cost: false, yields_to_inlinehint: true }),
        by_rate: 0,
    };
    let tti = PolicyOnly(policy, None, false);
    let text = ROTATE.replace("@callee(i16 %x, i16 %n) nounwind", "@callee(i16 %x, i16 %n) nounwind inlinehint");
    let inlined = |frequency, trial: bool| {
        let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
        let price =
            || priced_hot(&text, 1, false, CallerSize::OptSize, Some(&tti), params.clone(), (2, false), frequency);
        let (cost, threshold) = if trial { crate::trial::with_kept_hinted_calls(price) } else { price() };
        cost < threshold
    };
    assert!(inlined(None, false), "premise: the hint's yield lets the growing call in");
    assert!(!inlined(None, true), "outside a loop the trial puts the call back");
    assert!(!inlined(Some(0.5), true), "a site colder than its entry is not a loop's");
    assert!(inlined(Some(4.0), true), "in a loop the call is inlined in the trial too");
}

const HOT_CALLEE: &str = "define internal void @callee(ptr %p, i32 %c, i32 %m) {
b0:
  %a = load i32, ptr %p
  %b = and i32 %a, %m
  %d = or i32 %b, %c
  %e = xor i32 %d, %m
  %g0 = add i32 %e, %m
  %g1 = xor i32 %g0, %c
  %g2 = or i32 %g1, %m
  %g3 = and i32 %g2, %c
  %g4 = sub i32 %g3, %m
  %g5 = add i32 %g4, %c
  %g6 = xor i32 %g5, %m
  %g7 = or i32 %g6, %c
  %g8 = and i32 %g7, %m
  %g9 = sub i32 %g8, %c
  %g10 = add i32 %g9, %m
  %g11 = xor i32 %g10, %c
  %g12 = or i32 %g11, %m
  %g13 = and i32 %g12, %c
  store i32 %g13, ptr %p
  ret void
}

define void @f(ptr %p, i32 %c, i32 %m) {
b0:
  call void @callee(ptr %p, i32 %c, i32 %m)
  call void @callee(ptr %p, i32 %m, i32 %c)
  ret void
}
";

/// An x86 -Os target whose size bound yields at a hot site within `percent`.
fn hot_yield_tti(percent: Option<i64>) -> impl llrm_mir::tti::Tti {
    use llrm_mir::target::Tuning;
    llrm_x86::tti::X86::of_cpu("i486")
        .unwrap()
        .with_tuning(
            Tuning {
                inline_size_guard_at_size: true,
                inline_size_hot_growth_percent: percent,
                ..Tuning::PIN
            },
        )
}

/// Whether the first call of `HOT_CALLEE` is inlined at a site `frequency`
/// times its caller's entry.
fn hot_callee_inlined(
    percent: Option<i64>,
    frequency: f64,
) -> bool {
    let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
    let (cost, threshold) = priced_hot(
        HOT_CALLEE,
        0,
        false,
        CallerSize::OptSize,
        Some(&hot_yield_tti(percent)),
        params,
        (2, true),
        Some(frequency),
    );
    cost < threshold
}

/// The -Os size bound held a call in a pixel loop: QuickrBasic `pixmem.c`
/// `blend` (52 units, 12 sites) stayed a call at `pm_box`'s inner loop,
/// frequency 961 times the entry, where the pin's `optsize` threshold takes it
/// (`q45d25` -Os 2,335,641 instructions against clang's 1,443,306). A locally
/// hot site now passes the bound where its growth is within the target's share
/// of the call it removes; a cold site and a site over the share keep it.
#[test]
fn a_hot_site_passes_the_size_bound_within_the_growth_budget() {
    use llrm_mir::target::Tuning;
    let text = HOT_CALLEE;
    let tti = |percent: Option<i64>| {
        llrm_x86::tti::X86::of_cpu("i486")
            .unwrap()
            .with_tuning(
                Tuning {
                    inline_size_guard_at_size: true,
                    inline_size_hot_growth_percent: percent,
                    ..Tuning::PIN
                },
            )
    };
    let inlined = |percent: Option<i64>, frequency: f64| {
        let params = InlineParams { compute_full_inline_cost: true, ..InlineParams::from_level(2) };
        let (cost, threshold) =
            priced_hot(text, 0, false, CallerSize::OptSize, Some(&tti(percent)), params, (2, true), Some(frequency));
        cost < threshold
    };
    assert!(!inlined(None, 961.0), "premise: the bound holds a hot site where the target states no share");
    assert!(inlined(Some(1000), 961.0), "a hot site within the share passes the bound");
    assert!(!inlined(Some(1000), 1.0), "a site below the pin's hot frequency keeps the bound");
    assert!(!inlined(Some(0), 961.0), "growth over the share keeps the bound");
}

/// The hot-site yield trusts the units' growth, which credits the callee's
/// death over all its sites; cold sites that keep it make the inline cost bytes
/// (QuickrBasic `suggest.c` +92 B, `comp.c` +49 B, `x_idioms` +143 B). The
/// trial compile that keeps those calls refuses the yield, and notes each
/// caller where it stood, so the finished object can decide.
#[test]
fn a_hot_yield_is_noted_and_refused_in_the_keep_hot_calls_trial() {
    let _ = crate::trial::take_hot_yields();
    assert!(hot_callee_inlined(Some(1000), 961.0), "premise: the yield inlines the hot site");
    assert_eq!(crate::trial::take_hot_yields(), ["f"], "the caller where the yield stood is noted");
    assert!(!crate::trial::with_kept_hot_calls(|| hot_callee_inlined(Some(1000), 961.0)), "the trial keeps the call");
    assert!(crate::trial::take_hot_yields().is_empty(), "the trial notes nothing");
    assert!(!hot_callee_inlined(Some(1000), 1.0));
    assert!(crate::trial::take_hot_yields().is_empty(), "a cold site is not a yield");
}
