//! The x86-64 TTI reports partial and runtime unrolling (its model has a loop
//! buffer), so the unroll pass copies a loop of unknown trip count there; the
//! i486 reports none and leaves it. Before `loop_micro_op_buffer` the x86-64
//! row was a no-op like the i486's: 0 diffs in the A/B of the runtime unroll.

use llrm_transforms::loopunrollpass::{Options, unroll_function};
use llrm_x86::tti::X86;

const UNKNOWN_TRIPS: &str = "define i64 @f(i64 %x, i64 %n) {
b0:
  %enter = icmp sgt i64 %n, 0
  br i1 %enter, label %b1, label %b2

b1:
  %i = phi i64 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi i64 [ %x, %b0 ], [ %s, %b1 ]
  %m = mul i64 %i, %x
  %s = add i64 %acc, %m
  %next = add i64 %i, 1
  %go = icmp slt i64 %next, %n
  br i1 %go, label %b1, label %b2

b2:
  %r = phi i64 [ %x, %b0 ], [ %s, %b1 ]
  ret i64 %r
}
";

fn unrolled_for(cpu: &str) -> (bool, String) {
    let mut module = llrm_mir::parse::module(UNKNOWN_TRIPS).expect("parses");
    let layout =
        llrm_mir::datalayout::DataLayout::parse("e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128").expect("layout");
    let (context, function) = module.function_mut("f").expect("@f");
    llrm_transforms::loopsimplify::simplified(function);
    llrm_transforms::lcssa::closed(context, &layout, function).expect("closes");
    let tti = X86::of_cpu(cpu).expect("a cpu");
    let options = Options {
        opt_level: 2,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: true,
        inliner_follows: true,
        speed_rate: false,
    };
    let changed = unroll_function(&mut module, "f", &tti, options).expect("unrolls");
    let text = llrm_mir::print::module(&module);
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{text}");
    (changed, text)
}

#[test]
fn test_a_loop_of_unknown_trips_runtime_unrolls_on_x86_64_and_not_on_the_i486() {
    let (changed, text) = unrolled_for("x86-64");
    assert!(changed, "x86-64 left the loop alone:\n{text}");
    assert!(text.matches("mul i64").count() > 1, "no copies of the body:\n{text}");
    let (changed, text) = unrolled_for("i486");
    assert!(!changed, "i486 unrolled:\n{text}");
}

/// An init loop fills a local array that a loop of run-time trips reads at
/// run-time indices. The array stays in memory, but the store-credit rule is
/// the arm64 pricing's (`unroll_prices_nest_loops_without_outer_counter`); on
/// the i486 it made x87 nbody -O2 617 -> 797 B and fpbench 1130 -> 1602 B in
/// the nine-target A/B of #8018. The i486 keeps the credit: the fill unrolls.
const FILL_THEN_READ: &str = "define i32 @f(i32 %n) {
b0:
  %a = alloca [8 x i32], align 4
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b1 ]
  %m = mul i32 %i, 3
  %p = getelementptr [8 x i32], ptr %a, i32 0, i32 %i
  store i32 %m, ptr %p, align 4
  %next = add i32 %i, 1
  %go = icmp ult i32 %next, 8
  br i1 %go, label %b1, label %b2

b2:
  %j = phi i32 [ 0, %b1 ], [ %jn, %b2 ]
  %acc = phi i32 [ 0, %b1 ], [ %s, %b2 ]
  %q = getelementptr [8 x i32], ptr %a, i32 0, i32 %j
  %v = load i32, ptr %q, align 4
  %s = add i32 %acc, %v
  %jn = add i32 %j, 1
  %more = icmp ult i32 %jn, %n
  br i1 %more, label %b2, label %b3

b3:
  ret i32 %s
}
";

#[test]
fn test_the_i486_keeps_the_store_credit_for_an_array_read_at_run_time_indices() {
    let mut module = llrm_mir::parse::module(FILL_THEN_READ).expect("parses");
    let layout =
        llrm_mir::datalayout::DataLayout::parse("e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128").expect("layout");
    let (context, function) = module.function_mut("f").expect("@f");
    llrm_transforms::loopsimplify::simplified(function);
    llrm_transforms::lcssa::closed(context, &layout, function).expect("closes");
    let tuning = llrm_mir::target::Tuning { unroll_milliclocks_per_byte: 250_000, ..llrm_mir::target::Tuning::PIN };
    let tti = X86::of_cpu("i486").expect("a cpu").with_tuning(tuning);
    let options = Options {
        opt_level: 2,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: true,
        inliner_follows: true,
        speed_rate: false,
    };
    unroll_function(&mut module, "f", &tti, options).expect("unrolls");
    let text = llrm_mir::print::module(&module);
    assert!(text.matches("store i32").count() > 1, "the fill loop stayed rolled:\n{text}");
}
