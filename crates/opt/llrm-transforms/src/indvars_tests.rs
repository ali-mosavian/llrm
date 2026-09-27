//! llrm-core's `indvars_tests.rs` (tests/test_indvars.py and
//! tests/test_rewind.py), each body now MIR text the interpreter runs
//! before and after.
//!
//! Skipped: `test_symbolic_control_proves_a_zero_terminal_recurrence`, an
//! induction test, which has its own there; the BC fixtures through the
//! old emission (`test_harr_reuses_an_existing_recurrence_for_termination`,
//! `test_harr_initializes_the_reused_counter_before_its_exit_bound`,
//! `test_counting_one_loop_to_zero_leaves_a_loop_sharing_its_start_alone`).
//! The old rewind test's trip-count assertions, which read the side table
//! `loop_trip_counts`, are `a_rewound_loop_keeps_its_count_for_rotate`:
//! induction re-proves the count from the IR.

use std::rc::Rc;

use llrm_analysis::cfg;
use llrm_analysis::testing::layout;
use llrm_analysis::graph::loops;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, Module, Operand};
use llrm_mir::opcode::{IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Outer};

use super::{CountToZero, rewound, simplified, zeroed};
use crate::profit::OperationCosts;
use crate::rotate::Rotate;
use crate::testing::{f, managed, parsed, printed, results};

/// @f of `text` through `change`: whether it changed, the module, and that
/// it computes what it did on `inputs`.
fn through(text: &str, inputs: &[&[i128]], change: impl FnOnce(&mut Context, &DataLayout, &mut Function, &Analyses) -> bool) -> (bool, Module) {
    let mut module = parsed(text);
    let before = (printed(&module), results(&module, inputs));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let analyses = Analyses::new(Rc::new(outer));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = change(context, &layout, function, &analyses);
    let after = printed(&module);
    assert_eq!(results(&module, inputs), before.1, "{after}");
    assert_eq!(changed, after != before.0, "{after}");
    (changed, module)
}

/// The `icmp`s of @f whose first operand is `%name`d.
fn compared(module: &mut Module, name: &str) -> Vec<(IntPredicate, Vec<Option<String>>)> {
    let function = f(module);
    function
        .walk()
        .filter_map(|(_, inst)| match function.instruction(inst).opcode {
            Opcode::ICmp(predicate) => Some((predicate, function.instruction(inst).operands.iter().map(|one| match one {
                Operand::Value(value) => function.value(*value).name.clone(),
                _ => None,
            }).collect::<Vec<_>>())),
            _ => None,
        })
        .filter(|(_, operands)| operands[0].as_deref() == Some(name))
        .collect()
}

fn loops_of(module: &mut Module) -> Vec<loops::Loop> {
    let function = f(module);
    loops::loops(&cfg::graph(function), function.entry().map(cfg::id))
}

// ---------------------------------------------------------------- simplified

/// Nested loops of six trips, each with a counter and a scaled offset, the
/// inner body reached only where the counters differ.
fn nested(outer_stride: i64, inner_stride: i64, test: &str) -> String {
    format!(
        "@sum = global i16 0

define i16 @f(i16 %x) {{
b0:
  store i16 %x, ptr @sum
  br label %b1

b1:
  %o = phi i16 [ 0, %b0 ], [ %onext, %b6 ]
  %oo = phi i16 [ 0, %b0 ], [ %oonext, %b6 ]
  %ogo = icmp slt i16 %o, 6
  br i1 %ogo, label %b2, label %b9

b2:
  br label %b3

b3:
  %in = phi i16 [ 0, %b2 ], [ %innext, %b5 ]
  %io = phi i16 [ 0, %b2 ], [ %ionext, %b5 ]
  %igo = icmp slt i16 %in, 6
  br i1 %igo, label %b4, label %b6

b4:
  %same = icmp {test} i16 %in, %o
  br i1 %same, label %b5, label %b7

b7:
  %s = load i16, ptr @sum
  %t = add i16 %s, %io
  store i16 %t, ptr @sum
  br label %b5

b5:
  %innext = add i16 %in, 1
  %ionext = add i16 %io, {inner_stride}
  br label %b3

b6:
  %onext = add i16 %o, 1
  %oonext = add i16 %oo, {outer_stride}
  br label %b1

b9:
  %r = load i16, ptr @sum
  ret i16 %r
}}
"
    )
}

/// C nbody carried `other` beside `other*4` only for `other != body`.
///
/// The byte offsets are injective over their 0..5 domains, so they answer
/// both the equality and the loops' ends; the scalar counter added an
/// increment, a compare and spills to every interaction. A different map,
/// an ordered compare or a short period keep the counters.
#[test]
fn test_scaled_recurrences_replace_nested_counter_equality() {
    for (outer, inner, test, replaced) in [(4, 4, "eq", true), (5, 4, "eq", false), (4, 4, "slt", false), (32768, 32768, "eq", false)] {
        let (changed, mut module) = through(&nested(outer, inner, test), &[&[0], &[3]], |context, layout, function, analyses| simplified(context, layout, function, analyses).unwrap());
        assert_eq!(changed, replaced, "{outer} {inner} {test}");
        let (control, equal) = if replaced { ("io", "io") } else { ("in", "in") };
        assert!(compared(&mut module, control).iter().any(|(_, operands)| operands[1].is_none()), "{outer} {test}");
        let equality = compared(&mut module, equal).into_iter().find(|(_, operands)| operands[1].is_some()).expect("the equality");
        assert_eq!(equality.1[1].as_deref(), Some(if replaced { "oo" } else { "o" }), "{outer} {test}");
    }
}

/// Mandelbrot kept 16-bit `px` beside the 32-bit `cx` that already advances
/// every trip. The wider recurrence may end the loop where its period says
/// the final value comes no earlier; the loop stays counted, so Rotate
/// still enters it at its body. An 8-bit coordinate stepping by 64 repeats
/// in four trips and may not.
#[test]
fn test_cross_width_recurrence_replaces_counter_only_for_its_full_period() {
    for (width, stride, reused) in [(32, 24, true), (8, 64, false)] {
        let text = format!(
            "@seen = global i{width} 0

define i16 @f(i{width} %x) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b2 ]
  %c = phi i{width} [ %x, %b0 ], [ %cnext, %b2 ]
  %go = icmp slt i16 %i, 4
  br i1 %go, label %b2, label %b9

b2:
  store i{width} %c, ptr @seen
  %inext = add i16 %i, 1
  %cnext = add i{width} %c, {stride}
  br label %b1

b9:
  %r = add i16 %i, 1
  ret i16 %r
}}
"
        );
        let (changed, mut module) = through(&text, &[&[0], &[-7], &[100]], |context, layout, function, analyses| simplified(context, layout, function, analyses).unwrap());
        assert_eq!(changed, reused, "{width}");
        assert_eq!(compared(&mut module, "c").len(), usize::from(reused), "{width}");
        if reused {
            let rotated = printed(&module);
            let (entered, mut module) = through(&rotated, &[&[0], &[-7]], |context, layout, function, analyses| crate::rotate::entered(context, layout, function, analyses).unwrap());
            assert!(entered);
            assert_eq!(loops_of(&mut module)[0].body.len(), 1);
        }
    }
}

// ------------------------------------------------------------------ rewound

/// An outer loop of two trips around an inner one of four, from a start
/// computed before both.
fn rewinding(start: &str) -> String {
    format!(
        "define i32 @f(i32 %x) {{
b0:
  %start = {start}
  %bound = add i32 %start, 8
  br label %b1

b1:
  %o = phi i16 [ 0, %b0 ], [ %onext, %b5 ]
  %acc = phi i32 [ 0, %b0 ], [ %total, %b5 ]
  br label %b2

b2:
  br label %b3

b3:
  %cur = phi i32 [ %start, %b2 ], [ %fol, %b4 ]
  %sum = phi i32 [ %acc, %b2 ], [ %more, %b4 ]
  %done = icmp eq i32 %cur, %bound
  br i1 %done, label %b5, label %b4

b4:
  %more = add i32 %sum, %cur
  %fol = add i32 %cur, 2
  br label %b3

b5:
  %total = add i32 %sum, %cur
  %onext = add i16 %o, 1
  %again = icmp slt i16 %onext, 2
  br i1 %again, label %b1, label %b6

b6:
  ret i32 %total
}}
"
    )
}

/// Mandel reloaded and stored `xStart` at the start of every row.
///
/// The inner coordinate has advanced by exactly 4 * 2 on its only exit;
/// where a memory update is no dearer than a reload and a store, carrying
/// it round the outer loop, less 8, frees the saved start. A 386 keeps the
/// copy; so does a constant start, which is rebuilt for nothing (P6 grew
/// from 199 to 219 bytes when that was rewound).
#[test]
fn test_exact_nested_recurrence_rewinds_before_reloading_its_start() {
    let later = OperationCosts { add: 1, r#move: 1, load: 1, store: 1, memory_update: 1, ..OperationCosts::default() };
    let i386 = OperationCosts { add: 2, r#move: 2, load: 4, store: 2, memory_update: 8, ..OperationCosts::default() };
    let inputs: &[&[i128]] = &[&[0], &[-5], &[1000]];
    let (changed, mut module) = through(&rewinding("add i32 %x, 5"), inputs, |context, layout, function, analyses| rewound(context, layout, function, analyses, 1, &later));
    assert!(changed);
    let function = f(&mut module);
    let inner = function.layout().iter().copied().find(|&one| function.block(one).name.as_deref() == Some("b3")).unwrap();
    let phi = function.block(inner).instructions()[0];
    assert!(!function.instruction(phi).operands.iter().any(|one| matches!(one, Operand::Value(value) if function.value(*value).name.as_deref() == Some("start"))));
    for (text, costs, registers) in [(rewinding("add i32 %x, 5"), &i386, 1), (rewinding("add i32 %x, 5"), &later, 0), (rewinding("add i32 5, 0"), &later, 1)] {
        assert!(!through(&text, inputs, |context, layout, function, analyses| rewound(context, layout, function, analyses, registers, costs)).0);
    }
}

// ------------------------------------------------------------------- zeroed

/// A dynamic counted loop with a second recurrence from `start`, read
/// plus 100 and folded into `%acc`.
fn symbolic(start: i64) -> String {
    format!(
        "define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b2 ]
  %c = phi i16 [ {start}, %b0 ], [ %cnext, %b2 ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %b2 ]
  %go = icmp ult i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %off = add i16 %c, 100
  %twice = shl i16 %acc, 1
  %sum = xor i16 %twice, %off
  %inext = add i16 %i, 1
  %cnext = add i16 %c, 1
  br label %b1

b3:
  ret i16 %acc
}}
"
    )
}

const TRIPS: &[&[i128]] = &[&[0], &[1], &[7], &[300]];

/// A recurrence seeded at 5 is rebased by its final value, so its last
/// update is zero: one loop block testing it for zero, a guard skipping the
/// loop for no trips, and the counter gone.
#[test]
fn test_symbolic_control_rebases_a_nonzero_start_recurrence() {
    for start in [5, 0] {
        let mut module = parsed(&symbolic(start));
        let before = results(&module, TRIPS);
        let after = managed(&mut module, CountToZero);
        assert_eq!(results(&module, TRIPS), before, "{after}");
        let found = loops_of(&mut module);
        let [loop_] = &found[..] else { panic!("{after}") };
        assert_eq!(loop_.body.len(), 1, "{after}");
        assert!(!after.contains("%i ="), "the counter is gone:\n{after}");
        let function = f(&mut module);
        let entry = function.entry().unwrap();
        assert_eq!(function.instruction(function.terminator(entry).unwrap()).operands.len(), 3, "a guard:\n{after}");
        assert!(after.contains("icmp ne i16 %cnext, 0"), "{after}");
    }
}

/// A recurrence indexing memory is rebased by moving the base before the
/// loop; a mask whose span the bias is not a multiple of refuses.
#[test]
fn a_recurrence_indexing_memory_counts_to_zero() {
    let text = "@table = global [16 x i16] zeroinitializer

define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b2 ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %b2 ]
  %go = icmp ult i16 %i, 9
  br i1 %go, label %b2, label %b3

b2:
  %at = getelementptr inbounds [16 x i16], ptr @table, i16 0, i16 %i
  store i16 %i, ptr %at
  %got = load i16, ptr %at
  %sum = add i16 %acc, %got
  %inext = add i16 %i, 1
  br label %b1

b3:
  ret i16 %acc
}
";
    let (changed, _) = through(text, &[&[0]], |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap());
    assert!(!changed, "the counter is stored: an observed value");
    let indexed = text.replace("  store i16 %i, ptr %at\n", "");
    let (changed, module) = through(&indexed, &[&[0]], |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap());
    assert!(changed, "{}", printed(&module));
    let masked = indexed.replace("%at = getelementptr inbounds [16 x i16], ptr @table, i16 0, i16 %i", "%low = and i16 %i, 3\n  %at = getelementptr inbounds [16 x i16], ptr @table, i16 0, i16 %low");
    assert!(!through(&masked, &[&[0]], |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap()).0);
}

/// A recurrence less an invariant absorbs the bias as a sum does. An
/// index into an array based below zero reads `x - -640`, and refusing it
/// left such a loop counting up to a compare.
#[test]
fn a_recurrence_less_an_invariant_counts_to_zero() {
    let text = "@table = global [16 x i16] zeroinitializer

define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b2 ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %b2 ]
  %go = icmp ult i16 %i, 9
  br i1 %go, label %b2, label %b3

b2:
  %k = sub i16 %i, -2
  %at = getelementptr inbounds [16 x i16], ptr @table, i16 0, i16 %k
  %got = load i16, ptr %at
  %sum = add i16 %acc, %got
  %inext = add i16 %i, 1
  br label %b1

b3:
  ret i16 %acc
}
";
    let (changed, module) = through(text, &[&[0]], |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap());
    assert!(changed, "{}", printed(&module));
}

/// Unknown trips counted down behind a guard, for 0, 1 and many trips.
#[test]
fn a_dead_counter_of_unknown_trips_counts_down() {
    let (changed, _) = through(&symbolic(0).replace("%off = add i16 %c, 100", "%off = add i16 %n, 100"), TRIPS, |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap());
    assert!(changed);
}

/// A recurrence read after a guarded loop would need its final value where
/// the guard skipped the loop: the dead counter takes control instead.
#[test]
fn a_recurrence_read_after_a_guarded_loop_keeps_its_values() {
    let text = symbolic(5).replace("  ret i16 %acc", "  %after = add i16 %acc, %c\n  ret i16 %after").replace("%off = add i16 %c, 100", "%off = add i16 %n, 100");
    let (changed, module) = through(&text, TRIPS, |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap());
    let after = printed(&module);
    assert!(changed && after.contains("icmp ne i16 %inext, 0") && after.contains("%cnext = add i16 %c1, 1"), "{after}");
}

/// Nothing counts twice: a loop counted to zero is left alone.
#[test]
fn a_loop_counted_to_zero_is_settled() {
    let mut module = parsed(&symbolic(5));
    managed(&mut module, CountToZero);
    let once = printed(&module);
    assert_eq!(managed(&mut module, CountToZero), once);
    assert_eq!(managed(&mut module, Rotate), once);
}

/// A loop rotation cannot take -- its body two blocks -- counts to zero
/// at its header, and the exit reads the counter's final value.
#[test]
fn a_loop_rotation_cannot_take_tests_zero_at_its_header() {
    let text = "define i16 @f(i16 %x) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b3 ]
  %acc = phi i16 [ %x, %b0 ], [ %sum, %b3 ]
  %go = icmp slt i16 %i, 10
  br i1 %go, label %b2, label %b4

b2:
  %k = add i16 %i, 7
  br label %b3

b3:
  %sum = xor i16 %acc, %k
  %inext = add i16 %i, 1
  br label %b1

b4:
  %r = add i16 %acc, %i
  ret i16 %r
}
";
    let (changed, module) = through(text, &[&[0], &[5], &[-3]], |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap());
    let after = printed(&module);
    assert!(changed && after.contains("icmp ne i16 %i, 0") && after.contains("add i16 %acc, 10"), "{after}");
}

/// Rewound, the inner loop starts at a phi carried round the outer loop,
/// but it still runs four trips each row, so Rotate enters it. Induction
/// once lost the count there, and the loop stayed unrotated.
#[test]
fn a_rewound_loop_keeps_its_count_for_rotate() {
    let later = OperationCosts { add: 1, r#move: 1, load: 1, store: 1, memory_update: 1, ..OperationCosts::default() };
    let inputs: &[&[i128]] = &[&[0], &[-5], &[1000]];
    let (changed, mut module) = through(&rewinding("add i32 %x, 5"), inputs, |context, layout, function, analyses| rewound(context, layout, function, analyses, 1, &later));
    assert!(changed);
    let (before, results_before) = (printed(&module), results(&module, inputs));
    let after = managed(&mut module, Rotate);
    assert_ne!(after, before);
    assert!(after.contains("  %done = icmp eq i32 %fol, %bound\n  br i1 %done, label %b5, label %b4\n"), "{after}");
    assert_eq!(results(&module, inputs), results_before, "{after}");
}

/// Over the corpus, every loop induction counted before `rewound` keeps
/// its count after. Only Strength's recurrences start at a value, so it
/// runs first; the one it rewinds is not its loop's control.
#[test]
fn rewinding_the_corpus_loses_no_trip_count() {
    let later = OperationCosts { add: 1, r#move: 1, load: 1, store: 1, memory_update: 1, ..OperationCosts::default() };
    let counts = |context: &Context, layout: &DataLayout, function: &Function, outer: &Outer| {
        let unit = llrm_analysis::memory::Unit::within(context, layout, function, outer);
        let facts = llrm_analysis::consts::known(&unit, None, None, None);
        loops::loops(&cfg::graph(function), function.entry().map(cfg::id))
            .into_iter()
            .filter_map(|loop_| Some((loop_.header, llrm_analysis::induction::trip_count(&unit, &loop_, &facts)?)))
            .collect::<Vec<_>>()
    };
    let mut fired = 0;
    for (name, mut module) in llrm_analysis::testing::corpus() {
        // Emitted counters live in allocas until promoted.
        managed(&mut module, crate::promote::Promote);
        managed(&mut module, crate::strength::Strength::default());
        let (layout, outer) = (layout(&module), Outer::of(&module, None));
        let analyses = Analyses::new(Rc::new(Outer::of(&module, None)));
        let names = crate::testing::bodies(&module).into_iter().filter_map(|id| module.global(id).name.clone()).collect::<Vec<_>>();
        for callee in names {
            let (context, function) = module.function_mut(&callee).expect("a body");
            let before = counts(context, &layout, function, &outer);
            if !rewound(context, &layout, function, &analyses.fresh(), 1, &later) {
                continue;
            }
            fired += 1;
            let after = counts(context, &layout, function, &outer);
            for counted in &before {
                assert!(after.contains(counted), "{name}: {counted:?} lost; after {after:?}");
            }
        }
    }
    assert!(fired > 0, "the corpus rewinds somewhere");
}


/// A counter the loop stores beside a pointer it steps: the loop ends on
/// its own compare until the pointer's byte offset may count to zero.
fn pointer_beside_a_stored_counter() -> String {
    format!(
        "{}@a = internal global [8200 x i8] zeroinitializer

define i16 @f(i16 %x) {{
b0:
  br label %b1

b1:
  %v = phi i16 [ 1, %b0 ], [ %vnext, %b2 ]
  %p = phi ptr [ @a, %b0 ], [ %pnext, %b2 ]
  %go = icmp ne i16 %v, 81
  br i1 %go, label %b2, label %b3

b2:
  store i16 %v, ptr %p
  %pnext = getelementptr i8, ptr %p, i16 100
  %vnext = add i16 %v, 1
  br label %b1

b3:
  %at = getelementptr i8, ptr @a, i16 500
  %got = load i16, ptr %at
  ret i16 %got
}}
",
        llrm_analysis::testing::DOS
    )
}

/// The pointer counts its offset to zero, as the old route's byte index
/// did; it kept a second compare against the counter's bound.
#[test]
fn a_pointer_beside_a_stored_counter_counts_its_offset_to_zero() {
    let (changed, module) = through(&pointer_beside_a_stored_counter(), &[&[0]], |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap());
    let after = printed(&module);
    assert!(changed && !after.contains("%p ="), "{after}");
    assert!(after.contains("phi i16 [ -8000, %b0 ]") && after.contains("icmp ne i16 %v, 81") == false, "{after}");
}

/// Strength leaves a loop counted to zero alone: it carried the offset's
/// address as a pointer again and ended the loop on the counter, and the
/// pipeline cycled until it gave up.
#[test]
fn strength_leaves_a_pointer_counted_to_zero() {
    let mut module = parsed(&pointer_beside_a_stored_counter());
    let before = results(&module, &[&[0]]);
    llrm_mir::program::Program::lend(&mut module, Rc::new(llrm_x86_code16::Dos::default()), |program| crate::pipeline::applied(program, &crate::pipeline::Applied::default())).and_then(|done| done).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, &[&[0]]), before, "{after}");
    assert!(after.contains(", 0\n") && !after.contains("%p ="), "{after}");
}

/// The counter read after the loop stays its own: the pointer still counts
/// to zero, as the old route's did. The port refused, and suite/stride
/// compared the counter every trip.
#[test]
fn a_counter_read_after_the_loop_leaves_the_pointer_counting_to_zero() {
    let text = pointer_beside_a_stored_counter().replace("  ret i16 %got", "  %r = add i16 %got, %v\n  ret i16 %r");
    let (changed, module) = through(&text, &[&[0]], |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap());
    let after = printed(&module);
    assert!(changed && !after.contains("icmp ne i16 %v, 81"), "{after}");
}

/// A pointer takes control only where the counter then dies: kept alive by
/// a read in the loop, the counter counts to zero itself. The pointer's
/// offset took control, and arena kept both, a step each.
#[test]
fn a_counter_the_loop_reads_counts_to_zero_itself() {
    let text = pointer_beside_a_stored_counter().replace("  store i16 %v, ptr %p", "  %w = add i16 %v, 7\n  store i16 %w, ptr %p").replace("  ret i16 %got", "  %r = add i16 %got, %v\n  ret i16 %r");
    let (changed, module) = through(&text, &[&[0]], |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap());
    let after = printed(&module);
    assert!(changed && after.contains("%p =") && !after.contains("icmp ne i16 %v, 81"), "{after}");
}

/// Read only after the loop, the counter leaves as its final value and
/// dies: the header test kept it stepping for that read alone.
#[test]
fn a_counter_read_only_after_the_loop_dies() {
    let text = pointer_beside_a_stored_counter().replace("  store i16 %v, ptr %p", "  store i16 7, ptr %p").replace("  ret i16 %got", "  %r = add i16 %got, %v\n  ret i16 %r");
    let (changed, module) = through(&text, &[&[0]], |context, layout, function, analyses| zeroed(context, layout, function, analyses).unwrap());
    let after = printed(&module);
    assert!(changed && after.contains("add i16 %got, 81") && !after.contains("%p ="), "{after}");
}

/// A word counter indexing a dword array, widened: the wide counter is the
/// address's index, its product and test are rebuilt wide, and a value
/// reading the counter takes its truncation. Unwidened, every scaled index
/// needed a register and a shift of its own.
#[test]
fn test_a_widened_counter_indexes_and_tests_wide() {
    let text = "define i32 @f(i32 %n) {
entry:
  %a = alloca [16 x i32]
  br label %body
body:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i32 [ 0, %entry ], [ %t, %body ]
  %m = mul nsw i16 %i, 4
  %p = getelementptr inbounds i8, ptr %a, i16 %m
  %x = zext i16 %i to i32
  %y = add i32 %x, %n
  store i32 %y, ptr %p
  %v = load i32, ptr %p
  %t = add i32 %s, %v
  %j = add i16 %i, 1
  %c = icmp ult i16 %j, 16
  br i1 %c, label %body, label %done
done:
  ret i32 %t
}
";
    let (changed, module) = through(text, &[&[5], &[-3]], |context, layout, function, analyses| {
        let facts = analyses.fresh().get::<llrm_analysis::manager::Registers>(context, layout, function);
        let (loop_, counter, domain) = {
            let view = llrm_analysis::memory::Unit::within(context, layout, function, analyses.outer());
            let (loop_, basics, _) = llrm_analysis::induction::of(&view).into_iter().next().expect("a loop");
            let counter = basics.values().next().expect("a counter").clone();
            let domain = llrm_analysis::induction::domain(&view, &loop_, &counter, &facts).expect("a domain");
            (loop_, counter, domain)
        };
        super::widened(context, function, &loop_, &counter, &domain, 32)
    });
    assert!(changed);
    let text = printed(&module);
    assert!(text.contains("getelementptr inbounds i8, ptr %a, i32"), "{text}");
    assert!(text.contains("icmp ult i32"), "{text}");
    assert!(text.contains("trunc i32"), "{text}");
}
