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
        managed(&mut module, crate::lsr::Lsr);
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
