//! A trial that only flips a switch of the backend (the carry-save drop, the
//! long step) writes the kept program again: the mid-end does not run twice. A
//! module with such a site ran its whole flow, mid-end and trials included, a
//! second time (2026-11-22: 2 mid-end runs for one module, the trials' compile
//! time x2).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly, mid_ends, selected};

/// (mid-end runs, functions selected) of one compile.
fn cost(ir: &str) -> (usize, usize) {
    let (before, chosen) = (mid_ends(), selected());
    assembly(ir, &Request::new(Level::O2)).unwrap();
    (mid_ends() - before, selected() - chosen)
}

#[test]
fn a_long_step_trial_runs_the_mid_end_once() {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i32 @f(i32 %a) {{\n%r = add i32 %a, 1\nret i32 %r\n}}\n",
        datalayout()
    );
    let (mid, selections) = cost(&ir);
    assert_eq!(mid, 1);
    assert!(selections >= 2, "the trial is not run: {selections} selections");
}

#[test]
fn a_save_drop_trial_runs_the_mid_end_once() {
    let (mid, selections) = cost(include_str!("data/shared_sum_chain.ll"));
    assert_eq!(mid, 1);
    assert!(selections >= 2, "the trial is not run: {selections} selections");
}
