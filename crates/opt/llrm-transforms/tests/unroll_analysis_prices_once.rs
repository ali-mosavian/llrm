//! The unroll cost analysis priced every instruction of the loop again in
//! every simulated trip (`instruction_cost`: 2 M of particle's 89 M compile
//! instructions at -O2 68020, the same answer each time).

use llrm_transforms::loopunrollpass::{Options, rolled_priced, unroll_function};
use llrm_x86::tti::X86;

fn body(
    trips: u32,
    adds: u32,
) -> String {
    let mut text = format!(
        "define i32 @f(i32 %x) {{\nb0:\n  br label %b1\n\nb1:\n  %i = phi i32 [ 0, %b0 ], [ %next, %b1 ]\n  %acc = phi i32 [ %x, %b0 ], [ %s{adds}, %b1 ]\n  %m = mul i32 %i, %x\n  %s0 = add i32 %acc, %m\n"
    );
    for at in 1..=adds {
        text += &format!("  %s{at} = xor i32 %s{}, %i\n", at - 1);
    }
    text += &format!(
        "  %next = add i32 %i, 1\n  %go = icmp ult i32 %next, {trips}\n  br i1 %go, label %b1, label %b2\n\nb2:\n  ret i32 %s{adds}\n}}\n"
    );
    text
}

#[test]
fn analysing_a_loop_prices_each_instruction_once_not_once_per_trip() {
    let mut module = llrm_mir::parse::module(&body(8, 40)).expect("parses");
    let layout =
        llrm_mir::datalayout::DataLayout::parse("e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128").expect("layout");
    let (context, function) = module.function_mut("f").expect("@f");
    llrm_transforms::loopsimplify::simplified(function);
    llrm_transforms::lcssa::closed(context, &layout, function).expect("closes");
    let tti = X86::of_cpu("x86-64").expect("a cpu");
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
    let before = rolled_priced();
    unroll_function(&mut module, "f", &tti, options).expect("unrolls");
    let priced = rolled_priced() - before;
    assert!(priced > 0, "premise: the analysis did not run");
    assert!(priced <= 50, "{priced} prices for a loop of 45 instructions and 8 trips");
}
