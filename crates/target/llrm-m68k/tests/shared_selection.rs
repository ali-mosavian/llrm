//! Every trial build of a function ran all of it again: `c_ushort_3` at -Os on
//! the 68020 selected it ten times and ran every rewrite before allocation
//! ten times, although a trial changes one option. A stage runs once per
//! distinct set of the inputs it and the stages before it read.

use llrm_m68k::compile::{Options, items_with, runs};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, lowered_for};

const CASE: &str = include_str!("fixtures/div_ushort3_raw.ll");

#[test]
fn trial_builds_of_a_function_share_their_common_stages() {
    let module = lowered_for(CASE, Cpu::M68020, Level::Os).expect("lowers");
    let before = runs();
    let options = Options { size: true, mid_end: false, ..Options::default() };
    items_with(&module, "c_ushort_3", Cpu::M68020, options).expect("compiles");
    let after = runs();
    let (builds, selections) = (after.builds - before.builds, after.selections - before.selections);
    let (early, late) = (after.early - before.early, after.late - before.late);
    assert!(builds >= 8, "{builds} builds: the case no longer has the trials it tests");
    assert!(selections <= 3, "selection ran {selections} times in {builds} builds");
    assert!(early <= 4, "the early rewrites ran {early} times in {builds} builds");
    assert!(late < builds, "the late rewrites ran {late} times in {builds} builds");
}
