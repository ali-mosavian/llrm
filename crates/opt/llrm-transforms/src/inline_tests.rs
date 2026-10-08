//! Adapted from llrm-core's `optimize/inline_tests.rs`, the port of
//! `tests/test_inline.py`, each body now MIR text.
//!
//! Skipped: `test_inline_refuses_a_live_unmodelled_call_result`, since a call
//! has one result, what `ret` returns. The old file skipped
//! `test_small_private_pure_helpers_inline_in_mir` and
//! `test_tiny_private_leaf_inlines_at_two_call_sites` for want of the cfront
//! optimizer; both now read as MIR below.

use std::collections::BTreeSet;

use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::interpret::{self, Val};
use llrm_mir::module::{Linkage, Module};

use super::*;
use crate::testing::{parsed, printed};

fn id(module: &Module, name: &str) -> GlobalId {
    module.named(name).unwrap_or_else(|| panic!("no @{name}"))
}

fn private(module: &Module) -> BTreeSet<GlobalId> {
    module.functions().filter(|(_, global, _)| matches!(global.linkage, Linkage::Internal | Linkage::Private)).map(|(id, _, _)| id).collect()
}

fn costs(call: i64) -> OperationCosts {
    OperationCosts { call, ..OperationCosts::default() }
}

/// Every call in `caller` that `candidates` admits, inlined.
fn inline_into(module: &mut Module, caller: &str, call: i64) -> bool {
    inline_with(module, caller, call, Threshold::default())
}

fn inline_with(module: &mut Module, caller: &str, call: i64, threshold: Threshold) -> bool {
    let layout = DataLayout::default();
    let available = candidates(module, &layout, &call_counts(module), &private(module), &costs(call), call, threshold);
    let by = Caller { layout: &layout, recursive: recursive(module).contains(&id(module, caller)), base: 0 };
    let mut changed = false;
    while expand_once(module, caller, &by, &available) {
        changed = true;
    }
    changed
}

/// One call of `caller` inlined, where `available` admits it, with the declarations it needed placed.
fn expand_once(module: &mut Module, caller: &str, by: &Caller, available: &IndexMap<GlobalId, Candidate>) -> bool {
    let mut declared = llrm_mir::passes::Declared::of(module);
    let (context, function) = module.function_mut(caller).unwrap();
    let done = expanded(context, function, by, available, None, &mut declared).unwrap();
    declared.place(module).unwrap();
    done
}

fn run(module: &Module, name: &str, arguments: &[u128]) -> Val {
    let arguments = arguments.iter().map(|&bits| Val::Int { bits, width: 16 }).collect();
    interpret::run(module, name, arguments, 10_000).expect("runs")
}

const LEAF: &str = "define internal i16 @leaf() {
b1:
  ret i16 37
}
";

#[test]
fn test_inline_splices_return_before_the_original_successor_phi() {
    let mut module = parsed(&format!(
        "{LEAF}
define i16 @main() {{
b1:
  %result = call i16 @leaf()
  br label %b4

b4:
  %joined = phi i16 [ %result, %b1 ]
  ret i16 %joined
}}
"
    ));
    assert!(inline_into(&mut module, "main", 1));
    assert_eq!(
        printed(&module),
        "define internal i16 @leaf() {
b1:
  ret i16 37
}

define i16 @main() {
b1:
  br label %0

0:
  br label %1

1:
  br label %b4

b4:
  %joined = phi i16 [ 37, %1 ]
  ret i16 %joined
}
"
    );
}

/// Value ids number each function's arena from zero: the caller's actual
/// and the callee's unrelated definition share an id.
#[test]
fn test_inline_binds_an_actual_whose_id_is_a_callee_definitions() {
    let mut module = parsed(
        "define internal i16 @leaf(i16 %formal) {
b1:
  %unrelated = add i16 99, 0
  ret i16 %formal
}

define i16 @main(i16 %p) {
b1:
  %actual = add i16 %p, 7
  %result = call i16 @leaf(i16 %actual)
  ret i16 %result
}
",
    );
    assert!(inline_into(&mut module, "main", 1));
    assert!(printed(&module).contains("  ret i16 %actual\n"), "{}", printed(&module));
    assert_eq!(run(&module, "main", &[5]), Val::Int { bits: 12, width: 16 });
}

#[test]
fn test_inline_policy_refuses_repeated_work_without_a_call_cost() {
    let module = parsed(&format!(
        "{}
define i16 @main() {{
b1:
  %one = call i16 @leaf(i16 1)
  %two = call i16 @leaf(i16 2)
  %sum = add i16 %one, %two
  ret i16 %sum
}}
",
        "define internal i16 @leaf(i16 %x) {
b1:
  %y = add i16 %x, 37
  ret i16 %y
}
"
    ));
    let (private, layout) = (private(&module), DataLayout::default());
    assert_eq!(candidates(&module, &layout, &call_counts(&module), &private, &costs(0), 0, Threshold::default()), IndexMap::default());
    // Priced above the one instruction it duplicates, it is admitted.
    assert_eq!(candidates(&module, &layout, &call_counts(&module), &private, &costs(2), 2, Threshold::default()).len(), 1);
}

const HELPERS: &str = "define internal i16 @scale(i16 %x) {
b1:
  %y = shl i16 %x, 2
  %z = add i16 %y, 1
  ret i16 %z
}

define internal i16 @clamp(i16 %x) {
b1:
  %big = icmp sgt i16 %x, 100
  br i1 %big, label %b2, label %b3

b2:
  ret i16 100

b3:
  ret i16 %x
}

define i16 @main(i16 %a, i16 %b) {
b1:
  %s = call i16 @scale(i16 %a)
  %c = call i16 @clamp(i16 %s)
  %t = call i16 @scale(i16 %b)
  %sum = add i16 %c, %t
  ret i16 %sum
}
";

#[test]
fn test_small_private_pure_helpers_inline_and_keep_their_results() {
    let original = parsed(HELPERS);
    let mut module = parsed(HELPERS);
    assert!(inline_into(&mut module, "main", 4));
    let text = printed(&module);
    assert!(!text.contains("call "), "{text}");
    // Two returns meet in a phi at the continuation.
    assert!(text.contains("phi i16 [ 100, "), "{text}");
    for (a, b) in [(0, 0), (3, 4), (30, 7), (65535, 1)] {
        assert_eq!(run(&module, "main", &[a, b]), run(&original, "main", &[a, b]), "{a} {b}");
    }
}

#[test]
fn test_tiny_private_leaf_inlines_at_two_call_sites_only_when_the_call_costs_more() {
    let mut cheap = parsed(HELPERS);
    assert!(inline_into(&mut cheap, "main", 1));
    // @clamp, called once, goes; @scale, called twice for two instructions each, stays.
    assert_eq!(printed(&cheap).matches("call i16 @scale").count(), 2);
    let mut dear = parsed(HELPERS);
    assert!(inline_into(&mut dear, "main", 3));
    assert!(!printed(&dear).contains("call "));
}

#[test]
fn test_a_helper_called_twice_inlines_by_cost_not_by_shape() {
    let text = HELPERS.replace("%t = call i16 @scale(i16 %b)", "%t = call i16 @clamp(i16 %b)");
    let mut dear = parsed(&text);
    inline_into(&mut dear, "main", 1);
    assert_eq!(printed(&dear).matches("call i16 @clamp").count(), 2);
    let mut cheap = parsed(&text);
    assert!(inline_into(&mut cheap, "main", 40));
    assert_eq!(printed(&cheap).matches("call i16 @clamp").count(), 0);
}

/// A callee reading through its arguments, as BASIC's `Min%(x, y)`.
const MIN: &str = "define i16 @min(ptr %x, ptr %y) {
b1:
  %a = load i16, ptr %x
  %b = load i16, ptr %y
  %less = icmp slt i16 %a, %b
  br i1 %less, label %b2, label %b3

b2:
  ret i16 %a

b3:
  ret i16 %b
}

define i16 @main(i16 %p, i16 %q) {
b1:
  %x = alloca i16
  %y = alloca i16
  store i16 %p, ptr %x
  store i16 %q, ptr %y
  %r = call i16 @min(ptr %x, ptr %y)
  ret i16 %r
}
";

#[test]
fn test_a_public_callee_reading_through_its_arguments_inlines_and_stays_defined() {
    let original = parsed(MIN);
    let mut module = parsed(MIN);
    assert!(inline_into(&mut module, "main", 8), "a by-reference reader was a call in every build");
    assert!(!printed(&module).contains("call "), "{}", printed(&module));
    assert!(module.named("min").is_some());
    for (p, q) in [(1, 2), (2, 1), (5, 5), (0xffff, 1)] {
        assert_eq!(run(&module, "main", &[p, q]), run(&original, "main", &[p, q]), "{p} {q}");
    }
}

#[test]
fn test_a_callee_writing_through_an_argument_inlines_with_its_store() {
    let text = "define internal void @put(ptr %p, i16 %v) {
b1:
  %w = add i16 %v, 1
  store i16 %w, ptr %p
  ret void
}

define i16 @main(i16 %a) {
b1:
  %cell = alloca i16
  store i16 0, ptr %cell
  call void @put(ptr %cell, i16 %a)
  %r = load i16, ptr %cell
  ret i16 %r
}
";
    let original = parsed(text);
    let mut module = parsed(text);
    assert!(inline_into(&mut module, "main", 4));
    assert!(!printed(&module).contains("call "), "{}", printed(&module));
    for a in [0, 7, 0xffff] {
        assert_eq!(run(&module, "main", &[a]), run(&original, "main", &[a]), "{a}");
    }
}

#[test]
fn test_a_callee_frame_cell_inlined_into_a_loop_lands_in_the_callers_entry() {
    let text = "define internal i16 @bump(i16 %x) {
b1:
  %cell = alloca i16
  store i16 %x, ptr %cell
  %v = load i16, ptr %cell
  %w = add i16 %v, 3
  ret i16 %w
}

define i16 @main(i16 %n) {
b1:
  br label %b2

b2:
  %i = phi i16 [ 0, %b1 ], [ %next, %b2 ]
  %next = call i16 @bump(i16 %i)
  %go = icmp ult i16 %next, %n
  br i1 %go, label %b2, label %b3

b3:
  ret i16 %next
}
";
    let original = parsed(text);
    let mut module = parsed(text);
    assert!(inline_into(&mut module, "main", 4));
    let text = printed(&module);
    let main = text.split("define i16 @main").nth(1).unwrap();
    assert!(main.split("b2:").next().unwrap().contains("alloca"), "a frame cell in the loop grows the stack each trip: {text}");
    assert_eq!(main.matches("alloca").count(), 1);
    for n in [0, 5, 300] {
        assert_eq!(run(&module, "main", &[n]), run(&original, "main", &[n]), "{n}");
    }
}

#[test]
fn test_callees_a_clone_would_change_or_no_fact_answers_stay() {
    for (why, callee) in [
        ("recursive", "define internal i16 @leaf(i16 %x) {\nb1:\n  %y = call i16 @leaf(i16 %x)\n  ret i16 %y\n}\n"),
        ("setjmp-like", "declare i16 @setjmp(ptr) returns_twice memory(none)\n\ndefine internal i16 @leaf(i16 %x) {\nb1:\n  %c = alloca i16\n  %y = call i16 @setjmp(ptr %c)\n  ret i16 %y\n}\n"),
        ("noinline", "define internal i16 @leaf(i16 %x) noinline {\nb1:\n  ret i16 %x\n}\n"),
    ] {
        let mut module = parsed(&format!("{callee}\ndefine i16 @main(i16 %p) {{\nb1:\n  %r = call i16 @leaf(i16 %p)\n  ret i16 %r\n}}\n"));
        assert!(!inline_into(&mut module, "main", 40), "{why}");
    }
}

#[test]
fn test_a_call_whose_type_differs_from_its_callee_stays() {
    let mut module = parsed(
        "define internal i16 @leaf(i16 %x) {
b1:
  ret i16 %x
}

define i16 @main() {
b1:
  %r = call i16 @leaf(i16 1, i16 2)
  ret i16 %r
}
",
    );
    assert!(!inline_into(&mut module, "main", 40));
}

#[test]
fn test_inlining_inside_a_loop_retargets_the_self_edge() {
    let text = "define internal i16 @step(i16 %x) {
b1:
  %y = add i16 %x, 3
  ret i16 %y
}

define i16 @main(i16 %n) {
b1:
  br label %b2

b2:
  %i = phi i16 [ 0, %b1 ], [ %next, %b2 ]
  %next = call i16 @step(i16 %i)
  %go = icmp ult i16 %next, %n
  br i1 %go, label %b2, label %b3

b3:
  ret i16 %next
}
";
    let original = parsed(text);
    let mut module = parsed(text);
    assert!(inline_into(&mut module, "main", 1));
    assert!(!printed(&module).contains("call "));
    for n in [0, 1, 10, 300] {
        assert_eq!(run(&module, "main", &[n]), run(&original, "main", &[n]), "{n}");
    }
}

#[test]
fn test_constant_sites_admit_only_a_call_with_a_known_actual() {
    let module = parsed(
        "define internal i16 @leaf(i16 %x) {
b1:
  %y = add i16 %x, 37
  ret i16 %y
}

define i16 @main(i16 %p) {
b1:
  %one = call i16 @leaf(i16 1)
  %two = call i16 @leaf(i16 %p)
  %sum = add i16 %one, %two
  ret i16 %sum
}
",
    );
    let main = module.global(id(&module, "main")).function().unwrap();
    let constants = llrm_analysis::interprocedural::current_call_constants(&module.context, main);
    let sites = constant_sites(&module, &DataLayout::default(), &recursive(&module), main, &constants, &costs(2), 2, Threshold::default());
    let calls = main.walk().map(|(_, inst)| inst).filter(|&inst| callee(&module.context, main, inst).is_some()).collect::<Vec<_>>();
    assert_eq!(sites.keys().copied().collect::<Vec<_>>(), vec![calls[0]]);
}

/// QCport at -O2 grew by 8 KB where a callee of a few operations, which
/// one known actual folds nothing of, was copied to every such site because
/// its operation count was under the call's clocks, whatever the operations cost.
#[test]
fn test_a_constant_site_copies_only_the_work_its_actuals_leave_over_a_call() {
    let module = parsed(
        "define i16 @scaled(i16 %k, i16 %y) {
b1:
  %m = mul i16 %y, %y
  %r = add i16 %m, %k
  ret i16 %r
}

define i16 @squared(i16 %k) {
b1:
  %m = mul i16 %k, %k
  ret i16 %m
}

define i16 @main(i16 %p) {
b1:
  %one = call i16 @scaled(i16 1, i16 %p)
  %two = call i16 @squared(i16 3)
  %sum = add i16 %one, %two
  ret i16 %sum
}
",
    );
    let main = module.global(id(&module, "main")).function().unwrap();
    let constants = llrm_analysis::interprocedural::current_call_constants(&module.context, main);
    // Two operations, under a call of 10 clocks by count; the multiply alone costs 30.
    let priced = OperationCosts { call: 10, multiply: 30, ..OperationCosts::default() };
    let sites = constant_sites(&module, &DataLayout::default(), &recursive(&module), main, &constants, &priced, priced.call, Threshold::default());
    let calls = main.walk().map(|(_, inst)| inst).filter(|&inst| callee(&module.context, main, inst).is_some()).collect::<Vec<_>>();
    // `squared(3)` folds away entirely; `scaled(1, p)` keeps its multiply.
    assert_eq!(sites.keys().copied().collect::<Vec<_>>(), vec![calls[1]]);
    // Pushing and reading two arguments is overhead the copy saves as well.
    let passed = OperationCosts { argument: 20, ..priced };
    let sites = constant_sites(&module, &DataLayout::default(), &recursive(&module), main, &constants, &passed, passed.call, Threshold::default());
    assert_eq!(sites.keys().copied().collect::<Vec<_>>(), vec![calls[1]]);
}

/// QCport's screen.c grew 1.5 KB: peeling made 16 calls of `font_bit(.., gx, gy)` with known
/// coordinates, none of which folds anything, and each was copied as a constant site.
#[test]
fn test_a_constant_site_whose_known_actual_folds_nothing_is_not_copied() {
    let module = parsed(
        "define internal i16 @shifted(i16 %k, i16 %y) {
b1:
  %r = add i16 %y, %k
  ret i16 %r
}

define i16 @main(i16 %p) {
b1:
  %one = call i16 @shifted(i16 1, i16 %p)
  ret i16 %one
}
",
    );
    let main = module.global(id(&module, "main")).function().unwrap();
    let constants = llrm_analysis::interprocedural::current_call_constants(&module.context, main);
    let priced = OperationCosts { call: 10, argument: 20, ..OperationCosts::default() };
    let sites = constant_sites(&module, &DataLayout::default(), &recursive(&module), main, &constants, &priced, priced.call, Threshold::default());
    assert!(sites.is_empty(), "{} sites", sites.len());
}

#[test]
fn test_call_counts_count_direct_calls_to_defined_functions() {
    let module = parsed(&format!("declare i16 @external()\n\n{HELPERS}\ndefine i16 @more() {{\nb1:\n  %x = call i16 @external()\n  %y = call i16 @scale(i16 %x)\n  ret i16 %y\n}}\n"));
    let counts = call_counts(&module);
    assert_eq!(counts.get(&id(&module, "scale")), Some(&3));
    assert_eq!(counts.get(&id(&module, "clamp")), Some(&1));
    assert_eq!(counts.get(&id(&module, "external")), None);
}

/// A callee whose frame is `size` bytes, called from `main`, which the loop
/// calls `sites` times.
fn framed(size: u32, sites: usize) -> String {
    let calls: String = (0..sites).map(|at| format!("  %r{at} = call i16 @peek(i16 {at})\n")).collect();
    let sum: String = (1..sites).map(|at| format!("  %s{at} = add i16 {}, %r{at}\n", if at == 1 { "%r0".to_owned() } else { format!("%s{}", at - 1) })).collect();
    format!(
        "define i16 @peek(i16 %k) {{\nb1:\n  %buf = alloca [{size} x i8]\n  %p = getelementptr i8, ptr %buf, i16 %k\n  store i8 1, ptr %p\n  %v = load i8, ptr %p\n  %w = zext i8 %v to i16\n  ret i16 %w\n}}\n\ndefine i16 @main() {{\nb1:\n{calls}{sum}  ret i16 %s{}\n}}\n",
        sites - 1
    )
}

#[test]
fn test_copies_whose_frames_add_up_past_the_limit_stay() {
    // Three 300-byte frames became `sub sp, 900` in a caller that had none.
    let mut module = parsed(&framed(300, 3));
    assert!(!inline_into(&mut module, "main", 40));
    // Small ones inline.
    let mut module = parsed(&framed(40, 3));
    assert!(inline_into(&mut module, "main", 40));
}

#[test]
fn test_a_frame_is_not_copied_into_a_recursive_function() {
    // Each level of a recursive walk reserved the callee's 300 bytes.
    let text = framed(8, 1).replace("define i16 @main() {\nb1:\n  %r0 = call i16 @peek(i16 0)\n  ret i16 %s0\n}", "define i16 @main(i16 %n) {\nb1:\n  %r0 = call i16 @peek(i16 %n)\n  %m = call i16 @main(i16 %n)\n  %t = add i16 %r0, %m\n  ret i16 %t\n}");
    let mut module = parsed(&text);
    assert!(!inline_into(&mut module, "main", 40), "{}", printed(&module));
}

#[test]
fn test_a_counted_alloca_stays_a_call() {
    // The count is a parameter: hoisted to the caller's entry, it sat above
    // the value it is counted by ("does not dominate").
    let text = "define internal i16 @dyn(i16 %n) {
b1:
  %buf = alloca i8, i16 %n
  store i8 1, ptr %buf
  %v = load i8, ptr %buf
  %w = zext i8 %v to i16
  ret i16 %w
}

define i16 @main(i16 %k) {
b1:
  %m = add i16 %k, 1
  %r = call i16 @dyn(i16 %m)
  ret i16 %r
}
";
    let mut module = parsed(text);
    assert!(!inline_into(&mut module, "main", 40));
}

/// A callee of `ops` additions, carrying `attr`, called at `sites` sites of @main.
fn chain(ops: usize, attr: &str, sites: usize) -> String {
    let body: String = (0..ops).map(|at| format!("  %t{} = add i16 {}, {}\n", at + 1, if at == 0 { "%x".to_owned() } else { format!("%t{at}") }, at + 1)).collect();
    let calls: String = (0..sites).map(|at| format!("  %r{at} = call i16 @big(i16 {at})\n")).collect();
    let sum: String = (1..sites).map(|at| format!("  %s{at} = add i16 {}, %r{at}\n", if at == 1 { "%r0".to_owned() } else { format!("%s{}", at - 1) })).collect();
    format!("define i16 @big(i16 %x) {attr} {{\nb1:\n{body}  ret i16 %t{ops}\n}}\n\ndefine i16 @main() {{\nb1:\n{calls}{sum}  ret i16 %{}\n}}\n", if sites == 1 { "r0".to_owned() } else { format!("s{}", sites - 1) })
}

#[test]
fn test_a_callee_the_language_says_always_inline_is_inlined_at_any_size() {
    // Forty operations at two sites: over the budget and dearer than the calls.
    let mut plain = parsed(&chain(40, "", 2));
    assert!(!inline_into(&mut plain, "main", 8));
    let mut always = parsed(&chain(40, "alwaysinline", 2));
    assert!(inline_into(&mut always, "main", 8));
    assert!(!printed(&always).contains("call i16 @big"), "{}", printed(&always));
    assert!(always.named("big").is_some(), "a public callee stays defined");
}

#[test]
fn test_an_inline_hint_raises_the_budget_by_llvms_ratio_and_not_for_size() {
    // Eight operations, two private sites (the last call of one, tuned for size, inlines at any
    // size): the budget at this call price is 6, a hint's 8.
    let text = |attr: &str| chain(8, attr, 2).replace("define i16 @big", "define internal i16 @big");
    let admits = |attr: &str, threshold: Threshold| {
        let module = parsed(&text(attr));
        candidates(&module, &DataLayout::default(), &call_counts(&module), &private(&module), &costs(8), 8, threshold).len()
    };
    let (speed, size) = (Threshold::default(), Threshold::default().for_size());
    assert_eq!((admits("", speed), admits("inlinehint", speed), admits("inlinehint", size)), (0, 1, 0));
}

#[test]
fn test_a_callee_the_language_says_never_inline_stays_even_if_always_is_stated_too() {
    let mut module = parsed(&chain(2, "noinline alwaysinline", 1));
    assert!(!inline_into(&mut module, "main", 8));
}

/// QCport's glyph loops called `font_bit` once per pixel: refused beside
/// the call's overhead, as a call out of a loop saves it on every trip, the
/// frame was slower than the inlined build's.
#[test]
fn test_a_constant_site_in_a_loop_is_weighed_by_the_hot_site_threshold() {
    let module = parsed(
        "define i16 @sq(i16 %k, i16 %y) {
b1:
  %m = mul i16 %y, %y
  ret i16 %m
}

define i16 @main(i16 %n) {
b1:
  %once = call i16 @sq(i16 1, i16 %n)
  br label %b2

b2:
  %i = phi i16 [ 0, %b1 ], [ %next, %b2 ]
  %acc = phi i16 [ %once, %b1 ], [ %sum, %b2 ]
  %r = call i16 @sq(i16 2, i16 %i)
  %sum = add i16 %acc, %r
  %next = add i16 %i, 1
  %go = icmp ult i16 %next, %n
  br i1 %go, label %b2, label %b3

b3:
  ret i16 %sum
}
",
    );
    let main = module.global(id(&module, "main")).function().unwrap();
    let constants = llrm_analysis::interprocedural::current_call_constants(&module.context, main);
    // A multiply is 20 clocks, a call 10: kept above the overhead, below its hot weight (23).
    let priced = OperationCosts { call: 10, multiply: 20, store: 0, load: 0, return_: 0, ..OperationCosts::default() };
    let sites = constant_sites(&module, &DataLayout::default(), &recursive(&module), main, &constants, &priced, priced.call, Threshold::default());
    let calls = main.walk().map(|(_, inst)| inst).filter(|&inst| callee(&module.context, main, inst).is_some()).collect::<Vec<_>>();
    assert_eq!(sites.keys().copied().collect::<Vec<_>>(), vec![calls[1]]);
    // Where size outranks speed a loop buys nothing.
    assert!(constant_sites(&module, &DataLayout::default(), &recursive(&module), main, &constants, &priced, priced.call, Threshold::default().for_size()).is_empty());
}

/// The last call of a private function inlines at any size, tuned for size or for speed: no copy is made, and
/// the call, its arguments and the return go (QCport -Os, -548 bytes; queens -O2, -30% clocks). One called
/// twice stays a call at any level, and where nothing inlines (`Threshold::none()`) so does the last call.
#[test]
fn test_the_only_call_of_a_large_private_function_inlines_at_every_level() {
    let body: String = (0..40).map(|at| format!("  %t{at} = add i16 {}, {at}\n", if at == 0 { "%x".to_owned() } else { format!("%t{}", at - 1) })).collect();
    let text = |calls: usize| {
        let calls: String = (0..calls).map(|at| format!("  %r{at} = call i16 @big(i16 %x)\n")).collect();
        format!("define internal i16 @big(i16 %x) {{\nb0:\n{body}  ret i16 %t39\n}}\n\ndefine i16 @main(i16 %x) {{\nb0:\n{calls}  ret i16 %r0\n}}\n")
    };
    for threshold in [Threshold::default().for_size(), Threshold::default()] {
        let mut one = parsed(&text(1));
        assert!(inline_with(&mut one, "main", 8, threshold), "{}", printed(&one));
        assert!(!printed(&one).contains("call "));
        let mut two = parsed(&text(2));
        assert!(!inline_with(&mut two, "main", 8, threshold));
    }
    let mut none = parsed(&text(1));
    assert!(!inline_with(&mut none, "main", 8, Threshold::none()));
}

/// Tuned for size a pure body every actual of which is known folds whole, though `folded` follows
/// no loop: `parity_loop(7, 5)` was refused beside a call of 10 bytes with 28 bytes "kept" and
/// stayed a call (loop.nib +114 bytes). Tuned for speed it is weighed as before.
#[test]
fn test_a_pure_body_on_known_actuals_folds_whole_tuned_for_size() {
    let module = parsed(
        "define i16 @sum(i16 %n, i16 %k) memory(none) willreturn {
b1:
  br label %b2
b2:
  %i = phi i16 [ 0, %b1 ], [ %next, %b2 ]
  %t = phi i16 [ 0, %b1 ], [ %add, %b2 ]
  %m = mul i16 %i, %k
  %add = add i16 %t, %m
  %next = add i16 %i, 1
  %c = icmp slt i16 %next, %n
  br i1 %c, label %b2, label %b3
b3:
  ret i16 %add
}

define i16 @main() {
b1:
  %r = call i16 @sum(i16 7, i16 5)
  ret i16 %r
}
",
    );
    let main = module.global(id(&module, "main")).function().unwrap();
    let constants = llrm_analysis::interprocedural::current_call_constants(&module.context, main);
    let priced = OperationCosts { call: 5, add: 2, multiply: 6, branch: 2, return_: 1, argument: 2, ..OperationCosts::default() };
    let at = |threshold: Threshold| constant_sites(&module, &DataLayout::default(), &recursive(&module), main, &constants, &priced, 18, threshold).len();
    assert_eq!((at(Threshold::default()), at(Threshold::default().for_size())), (0, 1));
}

const RELEASING: &str = "define available_externally i16 @first(ptr releases %s) {
b1:
  %b = load i8, ptr %s
  %w = zext i8 %b to i16
  ret i16 %w
}

declare ptr @fresh()

define i16 @temporary() {
b1:
  %t = call ptr @fresh()
  %v = call i16 @first(ptr %t)
  ret i16 %v
}

define i16 @owned() {
b1:
  %s = alloca [2 x i8]
  store i8 7, ptr %s
  %v = call i16 @first(ptr %s)
  ret i16 %v
}
";

/// A routine that frees its string argument where the runtime allocated it was copied in at a
/// call of a runtime temporary: the copy never freed it, and the string space leaked once a call.
#[test]
fn a_routine_that_frees_a_temporary_is_not_inlined_at_a_call_of_one() {
    let mut module = parsed(RELEASING);
    let layout = DataLayout::default();
    let available = candidates(&module, &layout, &call_counts(&module), &private(&module), &costs(5), 5, Threshold::default());
    assert!(available.contains_key(&id(&module, "first")), "the premise: the body is a candidate");
    for (caller, inlined) in [("temporary", false), ("owned", true)] {
        let by = Caller { layout: &layout, recursive: false, base: 0 };
        assert_eq!(expand_once(&mut module, caller, &by, &available), inlined, "{caller}\n{}", printed(&module));
    }
}

/// A caller of 2800 operations with forty callees of 100, each called once: the last call of each inlines, at any size,
/// until the caller is over gcc's `large-function-insns` and has doubled (`large-function-growth` 100%), and no
/// further. Called-once inlining had no limit and QCport's d_alias took 29% more compile time (102.5e9 -> 132.4e9
/// instructions), every inline sending the larger body back through the pipeline.
#[test]
fn test_the_last_calls_into_a_large_caller_stop_where_it_has_doubled() {
    let chain = |count: usize, from: &str| -> String { (0..count).map(|at| format!("  %t{at} = add i16 {}, {at}\n", if at == 0 { from.to_owned() } else { format!("%t{}", at - 1) })).collect() };
    let callees: String = (0..40).map(|at| format!("define internal i16 @c{at}(i16 %x) {{\nb0:\n{}  ret i16 %t99\n}}\n\n", chain(100, "%x"))).collect();
    let calls: String = (0..40).map(|at| format!("  %r{at} = call i16 @c{at}(i16 %y)\n")).collect();
    let text = format!("{callees}define i16 @main(i16 %x) {{\nb0:\n{}  %y = add i16 %t2799, 1\n{calls}  ret i16 %r0\n}}\n", chain(2800, "%x"));
    let mut module = parsed(&text);
    let layout = DataLayout::default();
    let original = operations(module.global(id(&module, "main")).function().unwrap());
    let available = candidates(&module, &layout, &call_counts(&module), &private(&module), &costs(8), 8, Threshold::default());
    let by = Caller { layout: &layout, recursive: false, base: original };
    while expand_once(&mut module, "main", &by, &available) {}
    let after = operations(module.global(id(&module, "main")).function().unwrap());
    let calls_left = module.global(id(&module, "main")).function().unwrap().walk().filter(|&(_, inst)| matches!(module.global(id(&module, "main")).function().unwrap().instruction(inst).opcode, llrm_mir::opcode::Opcode::Call(_))).count();
    assert!(calls_left > 0, "all forty were inlined: {after} operations");
    assert!(after <= original * 2 + 100, "{after} operations after, from {original}");
}

/// The knee: a once-called body of 200 operations is not moved into its caller, one of 100 is. part_frame went from 435
/// to 1647 instructions by absorbing such callees and took 42 times the backend time.
#[test]
fn test_the_only_call_of_a_body_past_the_allocation_knee_stays_a_call() {
    let chain = |count: usize| -> String { (0..count).map(|at| format!("  %t{at} = add i16 {}, {at}\n", if at == 0 { "%x".to_owned() } else { format!("%t{}", at - 1) })).collect() };
    let text = |size: usize| format!("define internal i16 @big(i16 %x) {{\nb0:\n{}  ret i16 %t{}\n}}\n\ndefine i16 @main(i16 %x) {{\nb0:\n  %r = call i16 @big(i16 %x)\n  ret i16 %r\n}}\n", chain(size), size - 1);
    let mut past = parsed(&text(200));
    assert!(!inline_with(&mut past, "main", 8, Threshold::default()));
    let mut within = parsed(&text(100));
    assert!(inline_with(&mut within, "main", 8, Threshold::default()));
}

/// A body moved into a caller that is past the knee already stays a call: sb_build's once-called body of 40 operations went
/// into a caller of 150 (465 LIR instructions) and left 667, backend 0.36 s -> 2.4 s (compile-time, #791). The same body
/// goes into a caller within the knee.
#[test]
fn test_a_body_moved_into_a_caller_past_the_allocation_knee_stays_a_call() {
    let chain = |count: usize| -> String { (0..count).map(|at| format!("  %t{at} = add i16 {}, {at}\n", if at == 0 { "%x".to_owned() } else { format!("%t{}", at - 1) })).collect() };
    let text = |own: usize| format!("define internal i16 @once(i16 %x) {{\nb0:\n{}  ret i16 %t39\n}}\n\ndefine i16 @main(i16 %x) {{\nb0:\n{}  %r = call i16 @once(i16 %t{})\n  ret i16 %r\n}}\n", chain(40), chain(own), own - 1);
    let mut past = parsed(&text(150));
    assert!(!inline_with(&mut past, "main", 8, Threshold::default()));
    let mut within = parsed(&text(50));
    assert!(inline_with(&mut within, "main", 8, Threshold::default()));
}

/// The knee is measured in LIR instructions and counted in MIR operations: a body of the knee's operations comes to the
/// knee's instructions at the stated rate. Counted as the same number it let sb_build's merge through (667 instructions).
#[test]
fn test_the_knee_in_operations_is_the_knee_in_instructions_over_their_rate() {
    assert!((ALLOCATION_KNEE * INSTRUCTIONS_PER_OPERATION / 100 - KNEE_INSTRUCTIONS).abs() <= 2);
    assert!(ALLOCATION_KNEE < KNEE_INSTRUCTIONS);
}

/// A callee that only reads memory takes a `byval` argument as the caller's own pointer (LLVM's HandleByValArgument): the call is gone.
/// One that may write it gets a copy of the object: the pointer would be the caller's object, not the callee's own.
#[test]
fn test_a_byval_argument_inlines_into_a_callee_that_only_reads_and_not_into_one_that_writes() {
    let text = |attrs: &str, body: &str| {
        format!(
            "define internal i16 @peek(ptr byval([4 x i8]) %p) {attrs} {{
b1:
{body}
}}

define i16 @main() {{
b1:
  %cell = alloca [4 x i8]
  store i16 3, ptr %cell
  %r = call i16 @peek(ptr byval([4 x i8]) %cell)
  ret i16 %r
}}
"
        )
    };
    let reads = text("memory(read)", "  %v = load i16, ptr %p\n  ret i16 %v");
    let mut module = parsed(&reads);
    assert!(inline_into(&mut module, "main", 8));
    assert!(!printed(&module).contains("call "), "{}", printed(&module));
    // One that may write it is inlined on a copy: an alloca the object is memcpy'd into, which the body writes instead.
    let writes = text("", "  store i16 9, ptr %p\n  %v = load i16, ptr %p\n  ret i16 %v");
    let mut module = parsed(&writes);
    assert!(inline_into(&mut module, "main", 8));
    let printed = printed(&module);
    assert!(!printed.contains("call i16 @peek("), "{printed}");
    assert!(printed.contains("llvm.memcpy"), "{printed}");
    assert_eq!(printed.matches("alloca").count(), 2, "the object and its copy: {printed}");
}

/// The copy a `byval` argument costs is what an inline saves, as LLVM prices it: a reader of a large struct, called twice, is
/// worth a copy of its body where the same body over a small struct is not.
#[test]
fn test_a_byval_copy_is_part_of_what_an_inline_saves() {
    let text = |bytes: u32| {
        format!(
            "define internal i16 @peek(ptr byval([{bytes} x i8]) %p) memory(read) {{
b1:
  %v = load i16, ptr %p
  %w = add i16 %v, 1
  %x = add i16 %w, %v
  %y = add i16 %x, %w
  ret i16 %y
}}

define i16 @main() {{
b1:
  %cell = alloca [{bytes} x i8]
  store i16 3, ptr %cell
  %a = call i16 @peek(ptr byval([{bytes} x i8]) %cell)
  %b = call i16 @peek(ptr byval([{bytes} x i8]) %cell)
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        )
    };
    let inlined = |bytes: u32| {
        let mut module = parsed(&text(bytes));
        let layout = DataLayout::default();
        let priced = OperationCosts { call: 2, argument: 1, add: 3, load: 3, ..OperationCosts::default() };
        let available = candidates(&module, &layout, &call_counts(&module), &private(&module), &priced, 2, Threshold::default());
        let by = Caller { layout: &layout, recursive: false, base: 0 };
        while expand_once(&mut module, "main", &by, &available) {}
        !printed(&module).contains("call ")
    };
    assert!(!inlined(4), "a 4-byte copy paid for the body");
    assert!(inlined(2000), "a 2,000-byte copy was not counted");
}
