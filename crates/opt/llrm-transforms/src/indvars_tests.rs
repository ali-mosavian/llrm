//! llrm-core's `indvars_tests.rs` (tests/test_rewind.py), each body now
//! MIR text the interpreter runs before and after. The old rewind test's
//! trip-count assertions, which read the side table `loop_trip_counts`,
//! are `a_rewound_loop_keeps_its_count_for_rotate`: induction re-proves the
//! count from the IR. Choosing and counting a loop's counters is `lsr`'s,
//! tested there.

use std::rc::Rc;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_analysis::testing::layout;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, Module, Operand};
use llrm_mir::passes::{Analyses, Outer};

use super::{IndVars, rewound};
use crate::profit::OperationCosts;
use crate::rotate::Rotate;
use crate::testing::{f, managed, parsed, printed, results};

/// @f of `text` through `change`: whether it changed, the module, and that
/// it computes what it did on `inputs`.
fn through(
    text: &str,
    inputs: &[&[i128]],
    change: impl FnOnce(&mut Context, &DataLayout, &mut Function, &Analyses) -> bool,
) -> (bool, Module) {
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
    let (changed, mut module) = through(&rewinding("add i32 %x, 5"), inputs, |context, layout, function, analyses| {
        rewound(context, layout, function, analyses, 1, &later)
    });
    assert!(changed);
    let function = f(&mut module);
    let inner =
        function.layout().iter().copied().find(|&one| function.block(one).name.as_deref() == Some("b3")).unwrap();
    let phi = function.block(inner).instructions()[0];
    assert!(
        !function
            .instruction(phi)
            .operands
            .iter()
            .any(|one| matches!(one, Operand::Value(value) if function.value(*value).name.as_deref() == Some("start")))
    );
    for (text, costs, registers) in [
        (rewinding("add i32 %x, 5"), &i386, 1),
        (rewinding("add i32 %x, 5"), &later, 0),
        (rewinding("add i32 5, 0"), &later, 1),
    ] {
        assert!(
            !through(&text, inputs, |context, layout, function, analyses| rewound(
                context, layout, function, analyses, registers, costs
            ))
            .0
        );
    }
}

/// Rewound, the inner loop starts at a phi carried round the outer loop,
/// but it still runs four trips each row, so Rotate enters it. Induction
/// once lost the count there, and the loop stayed unrotated.
#[test]
fn a_rewound_loop_keeps_its_count_for_rotate() {
    let later = OperationCosts { add: 1, r#move: 1, load: 1, store: 1, memory_update: 1, ..OperationCosts::default() };
    let inputs: &[&[i128]] = &[&[0], &[-5], &[1000]];
    let (changed, mut module) = through(&rewinding("add i32 %x, 5"), inputs, |context, layout, function, analyses| {
        rewound(context, layout, function, analyses, 1, &later)
    });
    assert!(changed);
    let (before, results_before) = (printed(&module), results(&module, inputs));
    let after = managed(&mut module, Rotate { proven: true, copy: false });
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
        let unit = llrm_analysis::testing::with_registers(llrm_analysis::memory::Unit::within(
            context, layout, function, outer,
        ));
        let facts = llrm_analysis::consts::known(&unit);
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
        let names = crate::testing::bodies(&module)
            .into_iter()
            .filter_map(|id| module.global(id).name.clone())
            .collect::<Vec<_>>();
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
/// counted-loop proof was 16 bits against the pointer's 32, so `loopidioms` and
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
    let (changed, module) = through(text, &[&[0], &[1], &[7], &[64]], |context, layout, function, analyses| {
        super::widened(context, layout, function, analyses.outer())
    });
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
    let text = clearing("e-p:16:16-n8:16:32")
        .replace("i32 %w", "i16 %w16")
        .replace("zext i16 %i to i32", "zext i16 %i to i32\n  %w16 = trunc i32 %w to i16");
    let (changed, after) = widening(&text);
    assert!(!changed, "{after}");
}

/// `clearing` for `short i`: signed, tested by `slt`, read through `sext`.
fn clearing_signed(layout: &str) -> String {
    clearing(layout).replace("icmp ult i16", "icmp slt i16").replace("zext i16 %i to i32", "sext i16 %i to i32")
}

/// d_faces: `short k; for (k = 0; k < n; k++) a[k]` is `inc cx; movsx esi,cx;
/// cmp cx,bx; jl` a trip (12,700 of the 18,142 `movsx` the file executes
/// against gcc's 5,459). The counter is below `n` when it is stepped, so
/// the step never passes the width's largest value, and its sign extension is
/// the counter of the index's width (LLVM's `WidenIV` with `sext` and `nsw`;
/// gcc's ivopts on the loop bound).
#[test]
fn a_signed_counter_extended_to_the_index_width_is_that_wide() {
    let (changed, after) = widening(&clearing_signed("e-p:32:32-n8:16:32"));
    assert!(changed, "{after}");
    assert!(after.contains("icmp slt i32 %widen.iv"), "{after}");
    assert!(after.contains("sext i16 %n to i32"), "{after}");
    assert!(!after.contains("sext i16 %i to i32"), "{after}");
}

/// bench/lru's `for (short i = 0; i < 3; i++)` stores to five arrays: `unroll`
/// takes it whole where its size fits the rolled loop's, which it did to the
/// operation and no longer did with the counter widened (one operation
/// fewer rolled): a signed counter of a loop of a few trips is widened only
/// once `unroll` has had it.
#[test]
fn a_signed_counter_of_a_loop_that_unroll_may_take_is_widened_only_late() {
    let text = clearing_signed("e-p:32:32-n8:16:32").replace("icmp slt i16 %i, %n", "icmp slt i16 %i, 3");
    let early = through(&text, &[&[0], &[1]], |context, layout, function, analyses| {
        super::widened_with(
            context,
            layout,
            function,
            analyses.outer(),
            &mut llrm_analysis::memory::Standing::underived(),
            Some(16),
        )
    });
    assert!(!early.0, "{}", printed(&early.1));
    let (changed, after) = widening(&text);
    assert!(changed && after.contains("icmp slt i32 %widen.iv"), "{after}");
    // A loop of many trips is widened at once.
    let long = clearing_signed("e-p:32:32-n8:16:32").replace("icmp slt i16 %i, %n", "icmp slt i16 %i, 40");
    let first = through(&long, &[&[0], &[1]], |context, layout, function, analyses| {
        super::widened_with(
            context,
            layout,
            function,
            analyses.outer(),
            &mut llrm_analysis::memory::Standing::underived(),
            Some(16),
        )
    });
    assert!(first.0, "{}", printed(&first.1));
}

/// A start below zero is a negative number of the wide width, not the narrow
/// pattern.
#[test]
fn a_signed_counter_from_a_negative_start_is_widened_from_the_negative_number() {
    // The index is the counter plus 8, so that the interpreter's buffer holds
    // it.
    let text = clearing_signed("e-p:32:32-n8:16:32")
        .replace("[ 0, %b0 ]", "[ -5, %b0 ]")
        .replace(
            "%p = getelementptr inbounds i8, ptr %buf, i32 %w",
            "%v = add i32 %w, 8\n  %p = getelementptr inbounds i8, ptr %buf, i32 %v",
        );
    let (changed, after) = widening(&text);
    assert!(changed, "{after}");
    assert!(after.contains("phi i32 [ -5, %b0 ]"), "{after}");
}

/// An unsigned counter read through `sext` is not a signed one: its sign
/// extension differs past 32767.
#[test]
fn an_unsigned_counter_read_through_sext_is_not_widened() {
    let text = clearing("e-p:32:32-n8:16:32").replace("zext i16 %i to i32", "sext i16 %i to i32");
    let (changed, after) = widening(&text);
    assert!(!changed, "{after}");
}

/// bench/nbody: `for (unsigned short j = i + 1; j < 4; ++j) x[j]`: a start the
/// loop does not define. The narrow counter was `inc bx; movzx esi,bx; cmp
/// bx,4` each trip (38 instructions an iteration against gcc's 27); its zero
/// extension is the counter of the index's width from the extended start.
#[test]
fn a_counter_from_a_start_the_loop_does_not_define_is_widened() {
    let text = clearing("e-p:32:32-n8:16:32")
        .replace("define i32 @f(i16 %n) {\nb0:", "define i32 @f(i16 %n) {\nb0:\n  %s = add i16 %n, 1")
        .replace("[ 0, %b0 ]", "[ %s, %b0 ]")
        .replace("icmp ult i16 %i, %n", "icmp ult i16 %i, 4");
    let (changed, after) = widening(&text);
    assert!(changed, "{after}");
    assert!(after.contains("icmp ult i32 %widen.iv") && after.contains("zext i16 %s to i32"), "{after}");
    assert!(!after.contains("zext i16 %i to i32"), "{after}");
}

/// Each loop asked what is known of the body without memory and derived it
/// again: `widened` over a body of loops derived it once for each (#560). It
/// derives it once for the body.
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
    assert!(
        llrm_analysis::consts::register_derivations() - before <= 1,
        "{} derivations for three loops",
        llrm_analysis::consts::register_derivations() - before
    );
}

/// A pass that asks the manager what the counted loops bound, and changes
/// nothing.
struct AsksBounds;

impl llrm_mir::passes::FunctionPass for AsksBounds {
    fn name(&self) -> &'static str {
        "asks-bounds"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        analyses.get::<llrm_analysis::manager::Bounded>(unit.context, unit.layout, unit.function);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// A `sext` of a counter made indvars solve what the loops bound again, by
/// hand, beside the manager's it could have asked: fpbench -O1 spent 40 Minstr
/// in indvars, 29 of them in that solve.
#[test]
fn test_indvars_reads_the_managers_bounds_for_a_widened_counter() {
    let mut module = parsed(
        "define i32 @f(i32 %n) {
b0:
  br label %h

h:
  %i = phi i16 [ 0, %b0 ], [ %in, %l ]
  %s = phi i32 [ 0, %b0 ], [ %sn, %l ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %l, label %end

l:
  %w = sext i16 %i to i32
  %sn = add i32 %s, %w
  %in = add nsw i16 %i, 1
  br label %h

end:
  ret i32 %s
}
",
    );
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(AsksBounds);
    manager.add(super::IndVars { exit_values: false, unrolls: None });
    let before = llrm_analysis::ranges::loops_solved();
    manager.run_module(&mut module, Rc::new(crate::testing::Tuned::default())).unwrap();
    // The pass that asks, and the one indvars's evaluation of the loop leaves
    // the body needing; by hand it was a third.
    assert_eq!(
        llrm_analysis::ranges::loops_solved() - before,
        2,
        "the loop's bounds were worked out more than the manager's twice"
    );
}

/// Each loop indvars widened made it derive what is known of the body again,
/// from nothing, to look at the next: 13 times at d_faces -O2,
/// 25 M. The manager brings what it holds up to date from the changes logged:
/// the manager's own first derivation is the only one from nothing (this body
/// took two).
#[test]
fn test_indvars_brings_the_registers_up_to_date_and_does_not_derive_them_again_for_each_loop_it_widens() {
    let loops = 4;
    let mut text = String::from("define i32 @f(i32 %n) {\nb0:\n  br label %h0\n\n");
    for k in 0..loops {
        let (from, next) = (
            if k == 0 { "b0".to_owned() } else { format!("e{}", k - 1) },
            if k + 1 == loops { "end".to_owned() } else { format!("h{}", k + 1) },
        );
        text += &format!(
            "h{k}:\n  %i{k} = phi i16 [ 0, %{from} ], [ %in{k}, %l{k} ]\n  %s{k} = phi i32 [ 0, %{from} ], [ %sn{k}, %l{k} ]\n  %c{k} = icmp slt i16 %i{k}, 10\n  br i1 %c{k}, label %l{k}, label %e{k}\n\nl{k}:\n  %w{k} = sext i16 %i{k} to i32\n  %sn{k} = add i32 %s{k}, %w{k}\n  %in{k} = add nsw i16 %i{k}, 1\n  br label %h{k}\n\ne{k}:\n  br label %{next}\n\n"
        );
    }
    text += "end:\n  ret i32 0\n}\n";
    let mut module = parsed(&text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(super::IndVars { exit_values: false, unrolls: None });
    let before = llrm_analysis::consts::scratch_derivations();
    manager.run_module(&mut module, Rc::new(crate::testing::Tuned::default())).unwrap();
    let derived = llrm_analysis::consts::scratch_derivations() - before;
    assert_eq!(
        derived, 1,
        "what is known of the body was derived from nothing {derived} times for {loops} loops (the manager's, once)"
    );
}

/// The same loop entered behind a copy of its test (`-ftree-ch`): the counter
/// is tested stepped, after its trip, and the phi lists its latch first. It
/// stayed 16 bits (a `movzx` and a 5-instruction trip where the pre-tested loop
/// had a pointer and a negative count: m32 sieve -O1 +11.7% clocks with the
/// header copy).
#[test]
fn a_counter_tested_after_its_trip_behind_a_copy_of_its_test_is_widened() {
    let text = "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f(i16 %n) {
b0:
  %buf = alloca [64 x i8]
  %some = icmp ne i16 %n, 0
  br i1 %some, label %pre, label %b3

pre:
  br label %b1

b1:
  %i = phi i16 [ %next, %b1 ], [ 0, %pre ]
  %w = zext i16 %i to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %w
  store i8 1, ptr %p
  %next = add i16 %i, 1
  %go = icmp ult i16 %next, %n
  br i1 %go, label %b1, label %b3

b3:
  ret i32 0
}
";
    let (changed, after) = widening(text);
    assert!(changed, "{after}");
    assert!(after.contains("icmp ult i32 %widen.iv.next"), "{after}");
    assert!(!after.contains("zext i16 %i to i32"), "{after}");
}

fn with_exit_values(text: &str) -> String {
    let mut module = crate::testing::parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(super::IndVars { exit_values: true, unrolls: None });
    manager.run_module(&mut module, Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    crate::testing::printed(&module)
}

/// The counter's step leaves a loop that also breaks, on `i + 1 < 32` false:
/// the exit reads 32. It read the stepped counter, which kept the old one
/// alive past its step (mandel: `lea; or; je; mov` where `inc; je` does), and
/// made 32 in the latch each trip for the exit's phi.
#[test]
fn test_an_exit_phi_of_a_stepped_counter_is_the_number_its_count_gives() {
    let text = with_exit_values(
        "define i32 @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp sgt i32 %i, %n
  br i1 %c, label %b3, label %b2

b2:
  %next = add i32 %i, 1
  %again = icmp slt i32 %next, 32
  br i1 %again, label %b1, label %b4

b3:
  ret i32 -1

b4:
  %last = phi i32 [ %next, %b2 ]
  ret i32 %last
}
",
    );
    assert!(text.contains("ret i32 32") && !text.contains("%last"), "{text}");
}

/// An exit whose count is not a number is written from the count, as
/// `rewriteLoopExitValues` writes it: the loop runs `smax(n, 1)` times, two
/// instructions, under the budget.
#[test]
fn test_an_exit_phi_of_a_loop_with_an_unknown_count_is_written_from_the_count() {
    let text = with_exit_values(
        "define i32 @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  br label %b2

b2:
  %next = add i32 %i, 1
  %again = icmp slt i32 %next, %n
  br i1 %again, label %b1, label %b4

b4:
  %last = phi i32 [ %next, %b2 ]
  ret i32 %last
}
",
    );
    assert!(text.contains("llvm.smax.i32(i32 %n, i32 1)") && text.contains("ret i32 %0"), "{text}");
}

/// bench/sieve: `if (i <= 31)` on the narrow counter of a widened loop read
/// `trunc i32 %widen.iv to i16` and compared that, which `split` could not see
/// as a test of the counter: the compare is of the wide counter with the number
/// extended, as the counter never passes the narrow width.
#[test]
fn an_unsigned_compare_of_a_widened_counter_reads_the_wide_one() {
    let text = clearing("e-p:32:32-n8:16:32").replace(
        "  store i8 1, ptr %p",
        "  %low = icmp ule i16 %i, 31\n  %m = zext i1 %low to i8\n  store i8 %m, ptr %p",
    );
    let (changed, after) = widening(&text);
    assert!(changed, "{after}");
    assert!(after.contains("icmp ule i32 %widen.iv, 31"), "{after}");
    assert!(!after.contains("trunc"), "{after}");
}

/// A signed compare reads the narrow value's sign, which the wide one does not
/// have, so it stays a compare of the low part.
#[test]
fn a_signed_compare_of_a_widened_counter_reads_its_low_part() {
    let text = clearing("e-p:32:32-n8:16:32").replace(
        "  store i8 1, ptr %p",
        "  %low = icmp slt i16 %i, 31\n  %m = zext i1 %low to i8\n  store i8 %m, ptr %p",
    );
    let (changed, after) = widening(&text);
    assert!(changed, "{after}");
    assert!(after.contains("trunc i32 %widen.iv to i16"), "{after}");
}

// ---------------------------------------------------------------- congruent

/// `a[i] = i` and `b[j] = j` in one loop, `j` starting at `start` and
/// stepping by `step` where `i` starts at 0 and steps by 1.
fn two_counters(
    start: i16,
    step: i16,
) -> String {
    format!(
        "define i16 @f(i16 %n) {{
b0:
  %a = alloca [64 x i16]
  %b = alloca [64 x i16]
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %j = phi i16 [ {start}, %b0 ], [ %j.next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %io = mul i16 %i, 2
  %ia = getelementptr i8, ptr %a, i16 %io
  store i16 %i, ptr %ia
  %jo = mul i16 %j, 2
  %jb = getelementptr i8, ptr %b, i16 %jo
  store i16 %j, ptr %jb
  %i.next = add i16 %i, 1
  %j.next = add i16 %j, {step}
  br label %b1

b3:
  %x = load i16, ptr %a
  %y = load i16, ptr %b
  %z = add i16 %x, %y
  ret i16 %z
}}
"
    )
}

fn header_phis(module: &mut Module) -> usize {
    let function = f(module);
    let [one] = &loops::loops(&cfg::graph(function), function.entry().map(cfg::id))[..] else { panic!("one loop") };
    function
        .block(cfg::block(one.header))
        .instructions()
        .iter()
        .filter(|&&inst| function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Phi)
        .count()
}

/// Two counters that start and step alike are one: the loop kept both and
/// addressed `b` by a counter that always equalled `i`.
#[test]
fn counters_with_one_start_and_one_step_are_one_counter() {
    let mut module = parsed(&two_counters(0, 1));
    managed(&mut module, IndVars { exit_values: false, unrolls: None });
    assert_eq!(header_phis(&mut module), 1, "{}", printed(&module));
    let mut other = parsed(&two_counters(0, 1));
    assert_eq!(results(&module, &[&[0], &[3], &[9]]), results(&other, &[&[0], &[3], &[9]]));
    assert_eq!(header_phis(&mut other), 2);
}

/// A counter that starts or steps otherwise is another counter.
#[test]
fn counters_that_start_or_step_otherwise_stay_two() {
    for (start, step) in [(1, 1), (0, 2)] {
        let mut module = parsed(&two_counters(start, step));
        managed(&mut module, IndVars { exit_values: false, unrolls: None });
        assert_eq!(header_phis(&mut module), 2, "{start} {step}: {}", printed(&module));
    }
}

/// A loop `indvars` has nothing to do for.
const IDLE_LOOP: &str = "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %s = phi i16 [ 0, %b0 ], [ %t, %b1 ]
  %t = add i16 %s, %i
  %next = add i16 %i, 1
  %c = icmp slt i16 %next, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %t
}
";

/// Asks the manager for what `indvars` starts from, and nothing else.
struct AsksWhatIndVarsDoes;

impl llrm_mir::passes::FunctionPass for AsksWhatIndVarsDoes {
    fn name(&self) -> &'static str {
        "asks-what-indvars-does"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
        analyses.get::<cfg::Shape>(unit.context, unit.layout, unit.function);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// Each step of a run found the loops of the body for itself (`Shape::of`: the
/// dominators and the loop forest again): the exit values, the exits, the
/// widening, the counters merged: 25 of 119 Minstr of `indvars` at d_faces
/// -O2. A run that changes no block finds them no more often than the manager
/// does for the facts the run starts from.
#[test]
fn a_run_that_changes_no_block_finds_the_loops_once() {
    let derived = |pass: Box<dyn FnOnce(&mut Module)>| {
        let mut module = parsed(IDLE_LOOP);
        let before = cfg::shapes_derived();
        pass(&mut module);
        cfg::shapes_derived() - before
    };
    let manager = derived(Box::new(|module| {
        managed(module, AsksWhatIndVarsDoes);
    }));
    let indvars = derived(Box::new(|module| {
        managed(module, IndVars { exit_values: true, unrolls: None });
    }));
    assert_eq!(indvars, manager);
}

/// Each widening dropped what was known of the body and the next loop derived
/// it afresh: d_faces -O2 widened 12 counters and derived a 1,350-instruction
/// body 12 times (21.8 M, a third of `indvars`). What stands for the values a
/// widening did not reach is kept, and only the reached are settled again.
#[test]
fn test_a_widening_settles_again_only_the_values_it_reached() {
    let loops = 6;
    let mut text = String::from(
        "target datalayout = \"e-p:32:32-n8:16:32\"\n\ndefine i32 @f(i16 %n) {\nb0:\n  %buf = alloca [64 x i8]\n  br label %h0\n\n",
    );
    for at in 0..loops {
        let next = if at + 1 == loops { "done".to_string() } else { format!("h{}", at + 1) };
        text += &format!(
            "h{at}:\n  %i{at} = phi i16 [ 0, %{pre} ], [ %n{at}, %l{at} ]\n  %go{at} = icmp ult i16 %i{at}, %n\n  br i1 %go{at}, label %l{at}, label %{next}\n\nl{at}:\n  %w{at} = zext i16 %i{at} to i32\n  %p{at} = getelementptr inbounds i8, ptr %buf, i32 %w{at}\n  store i8 1, ptr %p{at}\n  %n{at} = add i16 %i{at}, 1\n  br label %h{at}\n\n",
            pre = if at == 0 { "b0".to_string() } else { format!("h{}", at - 1) },
        );
    }
    text += "done:\n  ret i32 0\n}\n";
    let mut module = parsed(&text);
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let size = function.walk().count();
    let before = llrm_analysis::consts::derived_work();
    assert!(super::widened(context, &layout, function, &outer), "the counters are widened");
    let work = llrm_analysis::consts::derived_work() - before;
    assert!(work <= 3 * size, "{work} values settled for {loops} widenings of a body of {size} instructions");
}

/// A rotated loop leaves from its latch. The step that took the exit values
/// before asked only the header for its exit (`_exit`), so it left a rotated
/// loop's sum computed by the loop; `rewriteLoopExitValues` asks every exit.
#[test]
fn test_a_rotated_loops_exit_value_is_rewritten_from_its_latch() {
    let text = with_exit_values(
        "define i16 @f(i16 %n) {
entry:
  %any = icmp ne i16 %n, 0
  br i1 %any, label %pre, label %done
pre:
  br label %loop
loop:
  %i = phi i16 [ 0, %pre ], [ %inc, %loop ]
  %s = phi i16 [ 0, %pre ], [ %s2, %loop ]
  %s2 = add i16 %s, 2
  %inc = add i16 %i, 1
  %c = icmp ne i16 %inc, %n
  br i1 %c, label %loop, label %exit
exit:
  %out = phi i16 [ %s2, %loop ]
  br label %done
done:
  %r = phi i16 [ 0, %entry ], [ %out, %exit ]
  ret i16 %r
}
",
    );
    assert!(text.contains("shl i16 %n, 1") || text.contains("mul i16 %n, 2"), "{text}");
    assert!(!text.contains("%s2 = add"), "the loop's sum is dead: {text}");
}
