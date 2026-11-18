//! The wide accumulator commute asked, for each candidate, how many
//! instructions read a register by scanning the whole function: quadratic in
//! the function, 16% of the 36 G instructions that `div_constants_long` takes
//! to compile. The reads are counted once.

use llrm_6502::driver::{Level, Request, assembly};
use llrm_spike_mf::commute::commute_visits;

mod common;
use common::pipeline::datalayout;

/// A function that adds `n` remainders of its argument: each add is a
/// candidate of the wide commute.
fn remainders(n: usize) -> String {
    let mut ir = String::from("define i32 @f(i32 %x) {\n  %a0 = add i32 %x, 1\n");
    for i in 0..n {
        let divisor = 3 + 2 * (i % 17);
        ir += &format!("  %r{i} = srem i32 %x, {divisor}\n  %a{} = add i32 %a{i}, %r{i}\n", i + 1);
    }
    ir + &format!("  ret i32 %a{n}\n}}\n")
}

/// Instructions visited counting reads, as the function doubles.
fn visits(n: usize) -> u64 {
    let ir = format!("target datalayout = \"{}\"\n{}", datalayout(), remainders(n));
    let before = commute_visits();
    assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    commute_visits() - before
}

#[test]
fn counting_the_reads_of_a_function_costs_its_instructions_not_their_square() {
    let (small, large) = (visits(8), visits(16));
    // twice the instructions, twice the visits (the scan took four times)
    assert!(large * 10 < small * 25, "{small} visits for 8 remainders, {large} for 16");
}
