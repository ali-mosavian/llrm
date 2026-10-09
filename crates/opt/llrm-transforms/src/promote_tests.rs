//! Adapted from llrm-core's `optimize/promote_tests.rs`, the port of
//! `tests/test_promote.py`, each body now MIR text.
//!
//! Ported as behaviour where the old test read internals: the singleton
//! index (`_bounded_ref`), the exact frame pointer, the dynamic allocation
//! and the sign extension (`_affine_values`). The call and overlap tests
//! reach the cell through an argument or an escape, since an unknown callee
//! cannot reach an unescaped frame object.
//!
//! Skipped: `test_sroa_refines_a_conservative_aggregate_range_from_the_exact_pointer`
//! (no provenance is attached, `annotated` derives it); the complete half of
//! `test_split_initializer_requires_every_byte` and
//! `test_packed_capture_keeps_wide_and_narrow_definitions_and_rejects_unknown_overlap`
//! (`_initializers`: a partial overlap is blocked); the scalar-update half of
//! `test_same_object_leaf_is_promoted_across_equivalent_pointer_values`
//! (`_separated`); `test_an_integer_cell_a_conversion_reads_is_promoted`
//! (no conversion reads memory). Stay behind, reading BC objects or
//! emitted code: `test_promotion_preserves_existing_cse_value_edges`,
//! `test_spill_accumulator_is_a_loop_carried_value`,
//! `test_addrm_long_accumulator_survives_split_initialization`,
//! `test_guarded_indexed_accumulators_do_not_reload_in_loop`,
//! `test_promotion_is_only_sound_because_the_runtime_was_measured` and
//! `test_production_press_keeps_the_loop_counter_in_a_value`.
//! `test_hotlop_keeps_initialization_for_memory_arithmetic` and
//! `test_promoted_global_remains_visible_outside_the_body` are the corpus
//! test.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_analysis::manager::Summaries;
use llrm_analysis::testing::{DOS, corpus};
use llrm_mir::module::{Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Outer, PassManager};

use super::{Promote, Sroa, promoted};
use crate::interprocedural::function_mut;
use crate::testing::{ACROSS_READONLY_CALL, bodies, managed, parsed, printed, results};

fn module(body: &str) -> Module {
    parsed(&format!("{DOS}{body}"))
}

/// Every body of `module` promoted, or with `aggregate_only` its
/// aggregates' leaves.
fn promote_all(
    module: &mut Module,
    aggregate_only: bool,
) -> Result<(), String> {
    let (layout, outer) = (llrm_analysis::testing::layout(module), Outer::of(module, None));
    for id in bodies(module) {
        let (context, function) = function_mut(module, id);
        promoted(context, &layout, function, &outer, aggregate_only)?;
    }
    Ok(())
}

/// `body` promoted, or with `aggregate_only` its aggregates' leaves; it
/// must verify.
fn run(
    body: &str,
    aggregate_only: bool,
) -> Module {
    let mut module = module(body);
    promote_all(&mut module, aggregate_only).unwrap();
    printed(&module);
    module
}

/// `body` promoted, printed without its datalayout line.
fn promote(body: &str) -> String {
    printed(&run(body, false)).split_once("\n\n").expect("a datalayout line").1.to_owned()
}

/// How many instructions of `module` `is` picks.
fn count(
    module: &Module,
    is: impl Fn(&Opcode) -> bool,
) -> usize {
    module
        .functions()
        .flat_map(|(_, _, function)| {
            function.walk().map(|(_, inst)| function.instruction(inst).opcode.clone()).collect::<Vec<_>>()
        })
        .filter(|one| is(one))
        .count()
}

fn loads(module: &Module) -> usize {
    count(module, |one| matches!(one, Opcode::Load { .. }))
}

fn stores(module: &Module) -> usize {
    count(module, |one| matches!(one, Opcode::Store { .. }))
}

/// `body`, left as it was.
fn untouched(body: &str) {
    assert_eq!(printed(&run(body, false)), printed(&module(body)), "{body}");
}

/// `body`'s loads promoted away, its stores kept, and what it returns for
/// each of `inputs` as before.
fn forwarded(
    body: &str,
    inputs: &[&[i128]],
) -> Module {
    let after = run(body, false);
    let before = module(body);
    assert_eq!(loads(&after), 0, "{}", printed(&after));
    assert_eq!(stores(&after), stores(&before), "{}", printed(&after));
    if !inputs.is_empty() {
        assert_eq!(results(&after, inputs), results(&before, inputs));
    }
    after
}

const INPUTS: &[&[i128]] = &[&[0], &[1], &[5], &[-3]];

/// A global sum and counter, as QB keeps its variables.
const LOOP: &str = "@sum = internal global i16 0
@count = internal global i16 0

define i16 @f(i16 %n) {
b0:
  store i16 0, ptr @sum
  store i16 0, ptr @count
  br label %b1

b1:
  %i = load i16, ptr @count
  %more = icmp slt i16 %i, %n
  br i1 %more, label %b2, label %b3

b2:
  %s = load i16, ptr @sum
  %t = add i16 %s, %i
  store i16 %t, ptr @sum
  %j = add i16 %i, 1
  store i16 %j, ptr @count
  br label %b1

b3:
  %r = load i16, ptr @sum
  ret i16 %r
}
";

/// press reloaded its loop counter each iteration despite having just
/// stored it: the loop now carries it, and every store stays.
#[test]
fn test_a_stored_value_reaches_a_loop_as_a_phi() {
    forwarded(LOOP, INPUTS);
    assert_eq!(
        promote(LOOP),
        "@sum = internal global i16 0
@count = internal global i16 0

define i16 @f(i16 %n) {
b0:
  store i16 0, ptr @sum
  store i16 0, ptr @count
  br label %b1

b1:
  %0 = phi i16 [ 0, %b0 ], [ %j, %b2 ]
  %1 = phi i16 [ 0, %b0 ], [ %t, %b2 ]
  %more = icmp slt i16 %0, %n
  br i1 %more, label %b2, label %b3

b2:
  %t = add i16 %1, %0
  store i16 %t, ptr @sum
  %j = add i16 %0, 1
  store i16 %j, ptr @count
  br label %b1

b3:
  ret i16 %1
}
"
    );
}

const DIAMOND: &str = "define i16 @f(i16 %c) {
b0:
  %a = alloca i16
  %z = icmp eq i16 %c, 0
  br i1 %z, label %b1, label %b2

b1:
  store i16 7, ptr %a
  br label %b3

b2:
  %d = mul i16 %c, 3
  store i16 %d, ptr %a
  br label %b3

b3:
  %x = load i16, ptr %a
  %y = add i16 %x, 1
  ret i16 %y
}
";

/// Two stores meet in a phi.
#[test]
fn test_stores_on_both_arms_join_in_a_phi() {
    forwarded(DIAMOND, INPUTS);
    assert!(
        promote(DIAMOND).contains("b3:\n  %0 = phi i16 [ 7, %b1 ], [ %d, %b2 ]\n  %y = add i16 %0, 1\n"),
        "{}",
        promote(DIAMOND)
    );
}

/// A path the cell was not stored on leaves the load reading memory.
#[test]
fn test_a_store_on_one_arm_is_not_available_after_the_join() {
    untouched(&DIAMOND.replace("  %d = mul i16 %c, 3\n  store i16 %d, ptr %a\n", ""));
}

/// A load arriving before the first store must not become an undefined
/// value.
#[test]
fn test_a_read_before_assignment_keeps_its_memory_value() {
    untouched(
        "@g = internal global i16 0

define i16 @f(i16 %c) {
b0:
  %x = load i16, ptr @g
  store i16 %c, ptr @g
  ret i16 %x
}
",
    );
}

/// A write the call may make, through the cell's own pointer or one that
/// escaped, keeps the load; another object's does not, nor a call that
/// only reads.
#[test]
fn test_frame_promotion_respects_unknown_and_overlapping_writes() {
    let escaped = "store ptr %a, ptr @slot\n  ";
    for (escape, call, reused) in [
        ("", "@g(ptr %b)", true),
        ("", "@peek(ptr %a)", true),
        ("", "@g(ptr %a)", false),
        (escaped, "@g(ptr null)", false),
    ] {
        let text = format!(
            "@slot = internal global ptr null

declare void @g(ptr)

declare void @peek(ptr) memory(read)

define i16 @f(i16 %c) {{
b0:
  %a = alloca i16
  %b = alloca i16
  {escape}store i16 %c, ptr %a
  call void {call}
  %x = load i16, ptr %a
  ret i16 %x
}}
"
        );
        let after = run(&text, false);
        assert_eq!(loads(&after) == 0, reused, "{call}: {}", printed(&after));
    }
}

/// A later call cannot invalidate an earlier read; an intervening call
/// must. A volatile store elsewhere writes only its own bytes.
#[test]
fn test_only_an_intervening_call_invalidates_a_stored_value() {
    for (effect, clobbers) in [("call void @h()", true), ("store volatile i16 1, ptr @y", false)] {
        for (position, between) in [(0, false), (1, true), (2, false)] {
            let reused = !(clobbers && between);
            let mut sequence = vec!["store i16 %c, ptr @x", "%v = load i16, ptr @x"];
            sequence.insert(position, effect);
            let text = format!(
                "@x = internal global i16 0
@y = internal global i16 0

declare void @h()

define i16 @f(i16 %c) {{
b0:
  {}
  ret i16 %v
}}
",
                sequence.join("\n  ")
            );
            let after = run(&text, false);
            assert_eq!(loads(&after) == 0, reused, "{effect} {position}: {}", printed(&after));
        }
    }
}

/// Volatile accesses stay as they are.
#[test]
fn test_a_volatile_access_is_never_promoted() {
    for (store, load) in [("store volatile", "load"), ("store", "load volatile")] {
        untouched(&format!(
            "define i16 @f(i16 %c) {{
b0:
  %a = alloca [8 x i8]
  {store} i16 %c, ptr %a
  %x = {load} i16, ptr %a
  ret i16 %x
}}
"
        ));
    }
}

/// A partial overlap keeps both ranges in memory: a narrower store into a
/// wider cell, the halves of one, or the wider cell read in part.
#[test]
fn test_a_partial_overlap_is_not_promoted() {
    let whole = "store i32 %v, ptr %a\n  %x = load i32, ptr %a\n  %y = trunc i32 %x to i16";
    assert_eq!(loads(&run(&PARTIAL.replace("{accesses}", whole), false)), 0);
    for accesses in [
        "store i32 %v, ptr %a\n  store i16 7, ptr %h\n  %x = load i32, ptr %a\n  %y = trunc i32 %x to i16",
        "store i16 0, ptr %a\n  store i16 7, ptr %h\n  %x = load i32, ptr %a\n  %y = trunc i32 %x to i16",
        "store i32 %v, ptr %a\n  %w = load i32, ptr %a\n  %y = load i16, ptr %h",
    ] {
        untouched(&PARTIAL.replace("{accesses}", accesses));
    }
}

const PARTIAL: &str = "define i16 @f(i32 %v) {
b0:
  %a = alloca i32
  %h = getelementptr i8, ptr %a, i16 2
  {accesses}
  ret i16 %y
}
";

/// A store through a pointer that may alias the cell keeps its load; one
/// to another global does not.
#[test]
fn test_a_store_through_a_may_alias_pointer_keeps_the_load() {
    let text = |other: &str| {
        format!(
            "@g = internal global i16 0
@h = internal global i16 0

define i16 @f(ptr %p, i16 %c) {{
b0:
  store i16 %c, ptr @g
  store i16 9, ptr {other}
  %x = load i16, ptr @g
  ret i16 %x
}}
"
        )
    };
    untouched(&text("%p"));
    assert_eq!(loads(&run(&text("@h"), false)), 0);
}

/// Two parameters may point at one cell, unless both are `noalias`.
#[test]
fn test_a_load_through_a_may_alias_pointer_keeps_its_value_in_memory() {
    let text = |attribute: &str| {
        format!(
            "define i16 @f(ptr {attribute}%p, ptr {attribute}%q) {{
b0:
  store i16 1, ptr %p
  store i16 2, ptr %q
  %x = load i16, ptr %p
  ret i16 %x
}}
"
        )
    };
    untouched(&text(""));
    assert!(promote(&text("noalias ")).contains("  ret i16 1\n"), "{}", promote(&text("noalias ")));
}

/// A cell read as another type than it was stored keeps its load.
#[test]
fn test_a_cell_loaded_as_another_type_keeps_its_load() {
    let text = |ty: &str| {
        format!(
            "define {ty} @f(i16 %c) {{
b0:
  %a = alloca i16
  store i16 %c, ptr %a
  %x = load {ty}, ptr %a
  ret {ty} %x
}}
"
        )
    };
    untouched(&text("ptr"));
    forwarded(&text("i16"), INPUTS);
}

/// Accesses of one leaf through distinct but equal pointers share its
/// value: a frame aggregate's and a global's, promoted and scalarized. Two
/// `!tbaa` types on its bytes are a union's members and keep them in
/// memory, as does a store to part of the leaf.
#[test]
fn test_same_object_leaf_is_promoted_across_equivalent_pointer_values() {
    let tbaa = "\n!0 = !{!\"root\"}\n!1 = !{!\"int4\", !0, i64 0}\n!2 = !{!1, !1, i64 0}\n!3 = !{!\"float4\", !0, i64 0}\n!4 = !{!3, !3, i64 0}\n";
    for object in ["%object = alloca [12 x i8]", "%object = getelementptr i8, ptr @aggregate, i16 0"] {
        let body = |accesses: &str| {
            format!(
                "@aggregate = internal global [12 x i8] zeroinitializer

define i32 @f(i32 %v) {{
b0:
  {object}
  %first = getelementptr i8, ptr %object, i16 4
  %second = getelementptr i8, ptr %object, i16 4
  {accesses}
  ret i32 %x
}}
{tbaa}"
            )
        };
        let plain = body("store i32 %v, ptr %first, !tbaa !2\n  %x = load i32, ptr %second, !tbaa !2");
        let inputs: &[&[i128]] = &[&[0], &[70_000], &[-1]];
        forwarded(&plain, inputs);
        assert_eq!(loads(&run(&plain, true)), 0, "{object}: an aggregate's leaf is scalarized");
        untouched(&body("store i32 %v, ptr %first, !tbaa !2\n  %x = load i32, ptr %second, !tbaa !4"));
        let upper = "%upper = getelementptr i8, ptr %object, i16 6\n  ";
        untouched(&body(&format!(
            "store i32 %v, ptr %first\n  {upper}store i16 0, ptr %upper\n  %x = load i32, ptr %second"
        )));
    }
}

/// An index a constant proves singleton names one exact leaf.
#[test]
fn test_sroa_uses_a_singleton_index_range_as_an_exact_leaf() {
    let text = "define i16 @f(i16 %c) {
b0:
  %a = alloca [6 x i16]
  %k = add i16 1, 1
  %p = getelementptr [6 x i16], ptr %a, i16 0, i16 %k
  store i16 %c, ptr %p
  %q = getelementptr [6 x i16], ptr %a, i16 0, i16 2
  %x = load i16, ptr %q
  ret i16 %x
}
";
    assert_eq!(loads(&run(text, true)), 0, "{}", printed(&run(text, true)));
}

/// Pointer arithmetic that wraps back to the frame object's start is its
/// start.
#[test]
fn test_sroa_uses_exact_frame_pointer_provenance_as_a_leaf() {
    let text = "define i32 @f(i32 %v) {
b0:
  %a = alloca [12 x i8]
  %past = getelementptr i8, ptr %a, i16 16
  %back = getelementptr i8, ptr %past, i16 -16
  store i32 %v, ptr %back
  %x = load i32, ptr %a
  ret i32 %x
}
";
    assert_eq!(loads(&run(text, true)), 0, "{}", printed(&run(text, true)));
    forwarded(text, &[&[0], &[70_000]]);
}

/// Two spellings of one address in a heap block, off the one pointer
/// loaded from its descriptor, are one cell.
#[test]
fn test_sroa_matches_equivalent_affine_addresses_inside_one_dynamic_allocation() {
    let text = "@descriptor = internal global ptr null

define i16 @f(i16 %c) {
b0:
  %block = load ptr, ptr @descriptor
  store i16 %c, ptr %block, !tbaa !2
  %over = getelementptr i8, ptr %block, i16 2
  %back = getelementptr i8, ptr %over, i16 -2
  %x = load i16, ptr %back, !tbaa !2
  ret i16 %x
}

!0 = !{!\"root\"}
!1 = !{!\"allocation\", !0, i64 0}
!2 = !{!1, !1, i64 0}
";
    let after = run(text, false);
    assert_eq!(loads(&after), 1, "only the descriptor is read: {}", printed(&after));
    assert!(printed(&after).contains("ret i16 %c"), "{}", printed(&after));
}

/// Sign and zero extension of one index are different addresses.
#[test]
fn test_sroa_does_not_treat_sign_extension_as_address_preserving() {
    let text = "define i16 @f(ptr %p, i8 %i, i16 %c) {
b0:
  %s = sext i8 %i to i16
  %z = zext i8 %i to i16
  %one = getelementptr i16, ptr %p, i16 %s
  %other = getelementptr i16, ptr %p, i16 %z
  store i16 %c, ptr %one
  %x = load i16, ptr %other
  ret i16 %x
}
";
    untouched(text);
    assert_eq!(loads(&run(&text.replace("i16 %z\n", "i16 %s\n"), false)), 0, "one index value is one address");
}

#[test]
fn test_sroa_never_promotes_a_volatile_aggregate_leaf() {
    let text = "define i32 @f(i32 %v) {
b0:
  %a = alloca [8 x i8]
  store volatile i32 %v, ptr %a
  %x = load volatile i32, ptr %a
  ret i32 %x
}
";
    assert_eq!(printed(&run(text, true)), printed(&module(text)));
}

/// A partial store after an unknown effect restores nothing from before it.
#[test]
fn test_partial_store_does_not_restore_constants_from_before_unknown_effect() {
    for effect in ["call void @g(ptr %a)", "store volatile i16 0, ptr %other"] {
        untouched(&format!(
            "declare void @g(ptr)

define i32 @f(i32 %v) {{
b0:
  %a = alloca i32
  %other = alloca i16
  store i32 287454020, ptr %a
  {effect}
  store i16 7, ptr %a
  %x = load i32, ptr %a
  ret i32 %x
}}
"
        ));
    }
}

/// A float cell holds its type's value: FRACLINE's loop was loads and
/// stores of `z`.
#[test]
fn test_a_float_store_reaches_later_loads_as_a_value() {
    forwarded(
        "define i16 @f(i16 %c) {
b0:
  %a = alloca double
  %d = sitofp i16 %c to double
  store double %d, ptr %a
  %x = load double, ptr %a
  %y = load double, ptr %a
  %z = fadd double %x, %y
  %r = fptosi double %z to i16
  ret i16 %r
}
",
        INPUTS,
    );
}

/// The loads in `module`'s loops that read a global directly: a QB
/// variable reloaded each iteration.
fn globals_loaded_in_loops(module: &Module) -> usize {
    let mut found = 0;
    for (_, _, function) in module.functions().filter(|(_, _, one)| !one.is_declaration()) {
        let inside = loops::loops(&cfg::graph(function), function.entry().map(cfg::id))
            .into_iter()
            .flat_map(|one| one.body)
            .collect::<BTreeSet<_>>();
        found += function
            .walk()
            .filter(|(block, _)| inside.contains(&cfg::id(*block)))
            .map(|(_, inst)| function.instruction(inst))
            .filter(|one| matches!(one.opcode, Opcode::Load { .. }) && matches!(one.operands[0], Operand::Constant(_)))
            .count();
    }
    found
}

/// Promotion keeps every corpus module verifying and every store in it.
/// press reloaded its counter, SPILL its accumulator across the inner
/// loop, ADDRM its LONG, hotlop its multiplier: none is reloaded now.
#[test]
fn test_every_corpus_module_verifies_and_keeps_its_stores() {
    let loops_of = ["qb-press", "qb-spill", "qb-addrm", "qb-hotlop"];
    let mut checked = 0;
    for (name, mut module) in corpus() {
        let before = stores(&module);
        let reloaded = globals_loaded_in_loops(&module);
        for aggregate_only in [true, false] {
            promote_all(&mut module, aggregate_only).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{name}");
            assert_eq!(stores(&module), before, "{name}");
        }
        if loops_of.iter().any(|one| name.ends_with(one)) {
            assert!(reloaded > 0, "{name} reloads nothing to begin with");
            assert_eq!(globals_loaded_in_loops(&module), 0, "{name}: {}", llrm_mir::print::module(&module));
            checked += 1;
        }
    }
    assert_eq!(checked, 2 * loops_of.len());
}

/// Under the pass manager Promote and Sroa still read the module's
/// globals: without them @g is no object and @peek a writer, and the load
/// stays.
#[test]
fn a_global_cell_is_promoted_across_a_readonly_call() {
    let after = managed(&mut module(ACROSS_READONLY_CALL), Promote);
    assert!(after.contains("  ret i16 7\n"), "{after}");
    let aggregate = ACROSS_READONLY_CALL
        .replace("@g = global i16 0", "@g = global [2 x i16] zeroinitializer")
        .replace("  store i16 7, ptr @g", "  %e = getelementptr [2 x i16], ptr @g, i16 0, i16 1\n  store i16 7, ptr %e")
        .replace("load i16, ptr @g", "load i16, ptr %e");
    let after = managed(&mut module(&aggregate), Sroa);
    assert!(after.contains("  ret i16 7\n"), "{after}");
}

/// Promote took every call to write anything, so a store before a call
/// to a procedure that writes something else was loaded again after it. A
/// weak body may be replaced, so what it does says nothing.
#[test]
fn a_call_to_a_procedure_that_does_not_write_a_cell_keeps_it_promoted() {
    let text = |linkage: &str| {
        format!(
            "@x = global i16 0
@y = global i16 0

define {linkage}void @p() {{
b0:
  store i16 1, ptr @y
  ret void
}}

define i16 @f(i16 %c) {{
b0:
  store i16 %c, ptr @x
  call void @p()
  %v = load i16, ptr @x
  ret i16 %v
}}
"
        )
    };
    let promoted = |linkage: &str| {
        let mut module = module(&text(linkage));
        let mut manager = PassManager::default();
        manager.require::<Summaries>();
        manager.add(Promote);
        manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
        printed(&module);
        module
    };
    let internal = promoted("internal ");
    assert_eq!(loads(&internal), 0, "{}", printed(&internal));
    assert_eq!(results(&internal, INPUTS), results(&module(&text("internal ")), INPUTS));
    assert_eq!(loads(&promoted("weak ")), 1);
}

/// matmul8's loop counters stayed in memory: a store through a pointer
/// loaded from far memory was taken to reach an unescaped local.
#[test]
fn a_store_through_a_loaded_far_pointer_leaves_a_local_counter_promoted() {
    let module = run(
        "define void @f(ptr addrspace(1) noalias %p, i16 %n) {
b0:
  %c = alloca i16
  store i16 0, ptr %c
  br label %b1

b1:
  %i = load i16, ptr %c
  %t = icmp ult i16 %i, %n
  br i1 %t, label %b2, label %b3

b2:
  %q = load ptr addrspace(1), ptr addrspace(1) %p
  store i16 %i, ptr addrspace(1) %q
  %j = load i16, ptr %c
  %k = add i16 %j, 1
  store i16 %k, ptr %c
  br label %b1

b3:
  ret void
}
",
        false,
    );
    assert_eq!(loads(&module), 1, "{}", printed(&module));
}

/// A cell keeps its `!tbaa` type when a write is asked about it: segld's
/// global stayed in memory past a far store typed `allocation`.
#[test]
fn a_write_of_another_tbaa_type_leaves_a_cell_available() {
    let body = "@t = internal global i16 0

define i16 @f(ptr addrspace(1) %far, i16 %v) {
b0:
  store i16 %v, ptr @t, !tbaa !2
  store i16 1, ptr addrspace(1) %far, !tbaa !4
  %x = load i16, ptr @t, !tbaa !2
  ret i16 %x
}

!0 = !{!\"llrm hir\"}
!1 = !{!\"place\", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!\"allocation\", !0, i64 0}
!4 = !{!3, !3, i64 0}
";
    forwarded(body, &[]);
}

/// A callee that stores through a pointer built from an integer, as QB writes
/// an array's data through its descriptor's segment word, wrote "unknown
/// memory", and no global kept a forwarded value across a call to it (GETPAL
/// and the palette variables: deedlines reloaded them). Its store's `!tbaa`
/// type is `allocation`, a variable's `place`: apart, so `@g` keeps its value;
/// a store of the variable's own type still kills it.
#[test]
fn an_unplaced_store_of_another_type_leaves_a_stored_global() {
    let across = |store_tag: &str| {
        let text = format!(
            "@g = internal global i16 0

define internal void @w(i16 %s) {{
b0:
  %p = inttoptr i16 %s to ptr addrspace(2)
  %q = addrspacecast ptr addrspace(2) %p to ptr addrspace(1)
  store i16 1, ptr addrspace(1) %q, !tbaa {store_tag}
  ret void
}}

define i16 @f(i16 %s) {{
b0:
  store i16 7, ptr @g, !tbaa !2
  call void @w(i16 %s)
  %r = load i16, ptr @g, !tbaa !2
  ret i16 %r
}}

!0 = !{{!\"root\"}}
!1 = !{{!\"place\", !0, i64 0}}
!2 = !{{!1, !1, i64 0}}
!3 = !{{!\"allocation\", !0, i64 0}}
!4 = !{{!3, !3, i64 0}}
"
        );
        crate::testing::summarized(&module(&text), Promote, true, &[])
    };
    assert!(across("!4").contains("ret i16 7"), "{}", across("!4"));
    assert!(across("!2").contains("load i16"), "{}", across("!2"));
}

/// A declared variable the promotion makes values of is told to the debugger as it goes: the value each store gives it,
/// from the instruction after the store, and the phi that merges the paths, from the top of the block. Without them the
/// variable's stores (which dead-store elimination removes) were its only record, and `-g` kept them by making them
/// volatile.
#[test]
fn a_promoted_variable_is_named_by_the_value_of_each_store_and_the_phi_that_joins_them() {
    let mut module = module(
        "define i16 @f(i1 %c) {\nentry:\n  %x = alloca i16\n  #dbg_declare(ptr %x, !0)\n  store i16 1, ptr %x\n  br i1 %c, label %a, label %j\na:\n  store i16 2, ptr %x\n  br label %j\nj:\n  %v = load i16, ptr %x\n  ret i16 %v\n}\n\n!0 = !{!\"f\", !\"x\", !1, i64 0}\n!1 = !{!\"int\"}\n",
    );
    promote_all(&mut module, false).expect("promotes");
    let text = printed(&module);
    let values: Vec<&str> = text.lines().map(str::trim).filter(|line| line.starts_with("#dbg_value")).collect();
    assert_eq!(values.len(), 3, "{text}");
    assert!(values.contains(&"#dbg_value(i16 1, !0)") && values.contains(&"#dbg_value(i16 2, !0)"), "{text}");
    // The third names the phi, which the return reads.
    let phi = text
        .lines()
        .map(str::trim)
        .find_map(|line| line.split_once(" = phi i16 ").map(|(result, _)| result.to_owned()))
        .expect("a phi");
    assert!(values.contains(&format!("#dbg_value(i16 {phi}, !0)").as_str()), "{phi} in {text}");
}
