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
    let layout = DataLayout::default();
    let available = candidates(module, &layout, &call_counts(module), &private(module), &costs(call), Threshold::default(), true);
    let by = Caller { layout: &layout, recursive: recursive(module).contains(&id(module, caller)) };
    let (context, function) = module.function_mut(caller).unwrap();
    let mut changed = false;
    while expanded(context, function, &by, &available, None).unwrap() {
        changed = true;
    }
    changed
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
    assert_eq!(candidates(&module, &layout, &call_counts(&module), &private, &costs(0), Threshold::default(), true), IndexMap::default());
    // Priced above the one instruction it duplicates, it is admitted.
    assert_eq!(candidates(&module, &layout, &call_counts(&module), &private, &costs(2), Threshold::default(), true).len(), 1);
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
    let sites = constant_sites(&module, &DataLayout::default(), &recursive(&module), main, &constants, &costs(2), Threshold::default());
    let calls = main.walk().map(|(_, inst)| inst).filter(|&inst| callee(&module.context, main, inst).is_some()).collect::<Vec<_>>();
    assert_eq!(sites.keys().copied().collect::<Vec<_>>(), vec![calls[0]]);
    // Not priced above the work it clones: none.
    assert!(constant_sites(&module, &DataLayout::default(), &recursive(&module), main, &constants, &costs(1), Threshold::default()).is_empty());
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
    // Eight operations, one private site: the budget at this call price is 6, a hint's 8.
    let text = |attr: &str| chain(8, attr, 1).replace("define i16 @big", "define internal i16 @big");
    let admits = |attr: &str, hints: bool| {
        let module = parsed(&text(attr));
        candidates(&module, &DataLayout::default(), &call_counts(&module), &private(&module), &costs(8), Threshold::default(), hints).len()
    };
    assert_eq!((admits("", true), admits("inlinehint", true), admits("inlinehint", false)), (0, 1, 0));
}

#[test]
fn test_a_callee_the_language_says_never_inline_stays_even_if_always_is_stated_too() {
    let mut module = parsed(&chain(2, "noinline alwaysinline", 1));
    assert!(!inline_into(&mut module, "main", 8));
}
