//! A tuple register (`QQ`..`QQQQ`) holds the register units of every member:
//! the allocator's interference follows the units, so two tuples that share a
//! vector register, or a tuple and one of its members, meet. A tuple read as
//! one root and one lane mask (the scalar path) saw none of this.

use llrm_arm64::REGISTER_INFO;
use llrm_codegen::greedy::matrix::clobbers;
use llrm_codegen::greedy::virtregmap::RegisterInfo;
use llrm_lir::registers::{RegSet, Regs};

fn units(name: &str) -> Vec<u32> {
    let tri = RegisterInfo::new(Regs(&REGISTER_INFO));
    let reg = REGISTER_INFO.by_name(name).unwrap_or_else(|| panic!("{name}"));
    let mut found: Vec<u32> = tri.regunits(reg).collect();
    found.sort_unstable();
    found
}

fn meet(
    a: &str,
    b: &str,
) -> bool {
    let (a, b) = (units(a), units(b));
    a.iter().any(|unit| b.contains(unit))
}

#[test]
fn a_tuple_has_the_units_of_each_member() {
    let mut both: Vec<u32> = units("v0").into_iter().chain(units("v1")).collect();
    both.sort_unstable();
    assert_eq!(units("v0_v1"), both);
    assert_eq!(units("v0_v1_v2").len(), 3 * units("v0").len());
}

#[test]
fn tuples_meet_where_they_share_a_register_and_only_there() {
    assert!(meet("v0_v1", "v1_v2"));
    assert!(meet("v0_v1", "v1"));
    assert!(!meet("v0_v1", "v2_v3"));
    assert!(!meet("v0_v1", "v2"));
}

/// The last tuple wraps: `v31_v0` holds `v0`.
#[test]
fn the_wrapping_tuple_holds_the_first_register() {
    assert!(meet("v31_v0", "v0"));
    assert!(meet("v31_v0", "v31"));
    assert!(!meet("v31_v0", "v1"));
}

/// A call that destroys `v1` destroys every tuple holding it, and none that
/// does not: the mask was matched against the tuple's own root, a register
/// no call names, so a live-across-call range was handed `v0_v1`.
#[test]
fn a_call_that_destroys_a_member_destroys_the_tuple() {
    let regs = Regs(&REGISTER_INFO);
    let reg = |name: &str| REGISTER_INFO.by_name(name).unwrap_or_else(|| panic!("{name}"));
    let mask: RegSet = [regs.root(reg("v1"))].into_iter().collect();
    assert!(clobbers(&regs, &mask, reg("v0_v1")));
    assert!(clobbers(&regs, &mask, reg("v1_v2_v3")));
    assert!(clobbers(&regs, &mask, reg("v1")));
    assert!(!clobbers(&regs, &mask, reg("v2_v3")));
    assert!(!clobbers(&regs, &mask, reg("v0")));
}
