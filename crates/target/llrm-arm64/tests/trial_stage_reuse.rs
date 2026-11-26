//! Each -Os trial built the whole module again, the mid-end with it, however
//! late in the build the setting it changes was first read: a trial of the
//! backend alone or of GlobalMerge repeated a mid-end whose result was the one
//! in hand.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const HEADER: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
                      target triple = \"aarch64-unknown-linux-gnu\"\n";

/// Mid-end runs and priced builds of one -Os compile of `text`.
fn counts(text: &str) -> (usize, usize) {
    let (mid, priced) = (driver::mid_ends(), llrm_arm64::trial::priced());
    driver::assembly(text, Platform::Linux, Level::Os).expect("lowers");
    (driver::mid_ends() - mid, llrm_arm64::trial::priced() - priced)
}

#[test]
fn a_trial_of_the_backend_alone_runs_no_second_mid_end() {
    let divide = format!("{HEADER}define i32 @f(i32 %a) optsize {{\n %r = sdiv i32 %a, 10\n ret i32 %r\n}}\n");
    let (mid, priced) = counts(&divide);
    assert!(priced >= 2, "premise: the divide was built both ways ({priced} priced)");
    assert_eq!(mid, 1, "the mid-end ran {mid} times");
}

#[test]
fn a_trial_of_global_merge_runs_no_second_mid_end() {
    let (mid, priced) = counts(include_str!("data/particle_os.ll"));
    assert!(priced >= 2, "premise: the merge was tried ({priced} priced)");
    assert_eq!(mid, 1, "the mid-end ran {mid} times");
}
