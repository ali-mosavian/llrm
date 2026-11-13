//! The arm64 unroll price at a low byte rate.

use llrm_arm64::machine::Platform;
use llrm_arm64::tti::AArch64;
use llrm_transforms::loopunrollpass::{Options, unroll_function};

/// shellsort -O3 at 20 clocks a byte: the 6-trip gap loop was credited a move
/// for each read of `gap` in its inner loop, run 1,000 times each (12,491
/// clocks "saved" against 416 B). Unrolled it ran 7,090 instructions for
/// 560 B, rolled 6,177 for 164 B. Where an immediate saves nothing the loop
/// stays rolled: one loop of the nest is left with a counter phi of 32.
#[test]
fn the_gap_loop_of_a_shellsort_stays_rolled_at_a_low_rate() {
    let mut program =
        llrm_arm64::driver::program(include_str!("data/shellsort_o1.ll"), Platform::Linux).expect("parses");
    let layout = program.layout.clone();
    let module = &mut program.modules[0];
    let (context, function) = module.function_mut("bench_shellsort").expect("function");
    llrm_transforms::loopsimplify::simplified(function);
    llrm_transforms::lcssa::closed(context, &layout, function).expect("closes");
    let mut tti = AArch64::of_cpu("generic").expect("a cpu");
    tti.unroll_rate = 20_000;
    let options = Options {
        opt_level: 3,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: false,
        inliner_follows: true,
    };
    unroll_function(module, "bench_shellsort", &tti, options).expect("unrolls");
    let text = llrm_mir::print::module(module);
    let blocks = text.lines().filter(|line| line.ends_with(':') || line.contains(": ; preds")).count();
    assert!(blocks < 20, "{blocks} blocks:\n{text}");
}

/// fpbench -O2: a 6-trip loop of floats that saves 0.7 clock a byte (17 clocks
/// for 24 B here; %39 there, 174 for 217) was kept rolled at 1 clock a byte,
/// where the pin unrolls it (size 9 against 150): 13,415 instructions run
/// against 9,539 with it unrolled. At the default rate (0.5) it is one block.
#[test]
fn a_loop_that_saves_most_of_a_clock_a_byte_is_unrolled() {
    let mut program =
        llrm_arm64::driver::program(include_str!("data/arm64_unroll_float_pair.ll"), Platform::Linux).expect("parses");
    let layout = program.layout.clone();
    let module = &mut program.modules[0];
    let (context, function) = module.function_mut("shape").expect("function");
    llrm_transforms::loopsimplify::simplified(function);
    llrm_transforms::lcssa::closed(context, &layout, function).expect("closes");
    let tti = AArch64::of_cpu("generic").expect("a cpu");
    let options = Options {
        opt_level: 2,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: false,
        inliner_follows: true,
    };
    unroll_function(module, "shape", &tti, options).expect("unrolls");
    let text = llrm_mir::print::module(module);
    let blocks = text.lines().filter(|line| line.ends_with(':') || line.contains(": ; preds")).count();
    assert!(blocks <= 2, "{blocks} blocks:\n{text}");
}
