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
