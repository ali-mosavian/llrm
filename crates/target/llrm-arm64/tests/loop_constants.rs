//! A loop whose switch arms each make a constant: the DAG path hoisted all of
//! them into callee-saved registers (five save/restore pairs, 984 B at -O2
//! against the hand selector's 788). The pin keeps an arm's constant in the
//! arm.

use llrm_arm64::driver::{self, IselMode, Level};
use llrm_arm64::machine::Platform;

const SWITCH_LOOP: &str = include_str!("data/switch_loop_o2.ll");

#[test]
fn a_loop_of_switch_arms_saves_no_callee_saved_register_for_their_constants() {
    driver::set_isel_mode(IselMode::Dag);
    for level in [Level::O2, Level::Os] {
        let text = driver::assembly(SWITCH_LOOP, Platform::Linux, level).expect("lowers");
        assert!(text.contains("cbnz") || text.contains("subs"), "premise: the loop is there:\n{text}");
        let saved: Vec<&str> =
            text.lines().filter(|line| line.contains("stp x19") || line.contains("str x19")).collect();
        assert!(saved.is_empty(), "{level:?}: callee-saved registers saved:\n{text}");
    }
}
