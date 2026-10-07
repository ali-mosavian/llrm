//! llrm-core's `indvars_tests.rs` (tests/test_rewind.py), each body now
//! MIR text the interpreter runs before and after. The old rewind test's
//! trip-count assertions, which read the side table `loop_trip_counts`,
//! are `a_rewound_loop_keeps_its_count_for_rotate`: induction re-proves the
//! count from the IR. Choosing and counting a loop's counters is `lsr`'s,
//! tested there.

use std::rc::Rc;

use llrm_analysis::cfg;
use llrm_analysis::testing::layout;
use llrm_analysis::graph::loops;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, Module, Operand};
use llrm_mir::passes::{Analyses, Outer};

use super::rewound;
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
/// its count after. `lsr`'s recurrences start at a value, so it runs
/// first; the one it rewinds is not its loop's control.
#[test]
fn rewinding_the_corpus_loses_no_trip_count() {
    let later = OperationCosts { add: 1, r#move: 1, load: 1, store: 1, memory_update: 1, ..OperationCosts::default() };
    let counts = |context: &Context, layout: &DataLayout, function: &Function, outer: &Outer| {
        let unit = llrm_analysis::testing::with_registers(llrm_analysis::memory::Unit::within(context, layout, function, outer));
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
        managed(&mut module, crate::lsr::Lsr::default());
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

// ------------------------------------------------------------------ widened

/// `for (unsigned short i = 0; i < n; ++i) a[i] = 0` on a 32-bit address:
/// the counter's `zext` to the index width is a conversion a trip and its
/// counted-loop proof was 16 bits against the pointer's 32, so `fill` and
/// `lsr` found no matching counter (m32 sieve, 19277 executed).
fn clearing(layout: &str) -> String {
    format!(
        "target datalayout = \"{layout}\"

define i32 @f(i16 %n) {{
b0:
  %buf = alloca [64 x i8]
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp ult i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %w = zext i16 %i to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %w
  store i8 1, ptr %p
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i32 0
}}
"
    )
}

fn widening(text: &str) -> (bool, String) {
    let (changed, module) = through(text, &[&[0], &[1], &[7], &[64]], |context, layout, function, analyses| super::widened(context, layout, function, analyses.outer()));
    (changed, printed(&module))
}

#[test]
fn a_counter_extended_to_the_index_width_is_that_wide() {
    let (changed, after) = widening(&clearing("e-p:32:32-n8:16:32"));
    assert!(changed, "{after}");
    assert!(after.contains("icmp ult i32 %widen.iv"), "{after}");
    assert!(!after.contains("zext i16 %i to i32"), "{after}");
}

#[test]
fn a_counter_extended_past_the_index_width_stays() {
    // 16-bit addresses (a segment's offset): `zext` to 32 is no index.
    let text = clearing("e-p:16:16-n8:16:32").replace("i32 %w", "i16 %w16").replace("zext i16 %i to i32", "zext i16 %i to i32\n  %w16 = trunc i32 %w to i16");
    let (changed, after) = widening(&text);
    assert!(!changed, "{after}");
}

/// bench/nbody: `for (unsigned short j = i + 1; j < 4; ++j) x[j]`: a start the loop does not define.
/// The narrow counter was `inc bx; movzx esi,bx; cmp bx,4` each trip (38 instructions an
/// iteration against gcc's 27); its zero extension is the counter of the index's width from the
/// extended start.
#[test]
fn a_counter_from_a_start_the_loop_does_not_define_is_widened() {
    let text = clearing("e-p:32:32-n8:16:32").replace("define i32 @f(i16 %n) {\nb0:", "define i32 @f(i16 %n) {\nb0:\n  %s = add i16 %n, 1").replace("[ 0, %b0 ]", "[ %s, %b0 ]").replace("icmp ult i16 %i, %n", "icmp ult i16 %i, 4");
    let (changed, after) = widening(&text);
    assert!(changed, "{after}");
    assert!(after.contains("icmp ult i32 %widen.iv") && after.contains("zext i16 %s to i32"), "{after}");
    assert!(!after.contains("zext i16 %i to i32"), "{after}");
}

/// Each loop asked what is known of the body without memory and derived it again: `widened` over a body of
/// loops derived it once for each (#560). It derives it once for the body.
#[test]
fn what_is_known_of_a_body_is_derived_once_for_all_its_loops() {
    let text = "define i32 @f(i32 %n) {
b0:
  br label %h1

h1:
  %i1 = phi i32 [ 0, %b0 ], [ %x1, %l1 ]
  %g1 = icmp ult i32 %i1, %n
  br i1 %g1, label %l1, label %p2

l1:
  %x1 = add i32 %i1, 1
  br label %h1

p2:
  br label %h2

h2:
  %i2 = phi i32 [ 0, %p2 ], [ %x2, %l2 ]
  %g2 = icmp ult i32 %i2, %n
  br i1 %g2, label %l2, label %p3

l2:
  %x2 = add i32 %i2, 1
  br label %h2

p3:
  br label %h3

h3:
  %i3 = phi i32 [ 0, %p3 ], [ %x3, %l3 ]
  %g3 = icmp ult i32 %i3, %n
  br i1 %g3, label %l3, label %done

l3:
  %x3 = add i32 %i3, 1
  br label %h3

done:
  ret i32 0
}
";
    let mut module = parsed(text);
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let before = llrm_analysis::consts::register_derivations();
    assert!(!super::widened(context, &layout, function, &outer), "nothing to widen");
    assert!(llrm_analysis::consts::register_derivations() - before <= 1, "{} derivations for three loops", llrm_analysis::consts::register_derivations() - before);
}
