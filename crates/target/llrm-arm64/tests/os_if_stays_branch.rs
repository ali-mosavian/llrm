//! A two-entry phi folded into selects costs a `csel` each at -Os, and the
//! branch it replaced cost one instruction: grep -Os kept four `csel` where
//! gcc keeps two branches (576 B against 544 B).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const IR: &str = include_str!("data/os_if_stays_branch.ll");
const ONE_PHI: &str = include_str!("data/os_if_one_phi.ll");

fn selects(
    ir: &str,
    level: Level,
) -> usize {
    let asm = driver::assembly(ir, Platform::Linux, level).expect("lowers");
    asm.lines().filter(|line| line.trim().starts_with("csel") || line.trim().starts_with("csinc")).count()
}

#[test]
fn an_if_with_two_phis_stays_a_branch_at_os() {
    assert_eq!(selects(IR, Level::Os), 0, "the two phis were folded into selects");
}

#[test]
fn the_same_if_is_selects_at_o2() {
    assert!(selects(IR, Level::O2) >= 2, "O2 keeps the pin's fold");
}

#[test]
fn an_if_with_one_phi_is_a_select_at_os() {
    assert_eq!(selects(ONE_PHI, Level::Os), 1, "a lone phi is the select that replaces the branch");
}
