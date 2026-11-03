//! arm64's register file has lanes of its own. The shared `Lanes` were the
//! x86 file's table only: naming an arm64 register in one panicked ("AL bytes
//! 0..=0 are no lanes"), so no pass over lanes could run on arm64.

use llrm_arm64::{REGISTER_INFO, reg};
use llrm_codegen::backend::lanes::{self, Lanes};
use llrm_lir::registers::Regs;

#[test]
fn an_arm64_register_and_its_views_are_one_lane() {
    let _file = lanes::enter(Regs(&REGISTER_INFO));
    assert_eq!(Lanes::span(reg::X5, 0, 1), Lanes::span(reg::W5, 0, 1));
    assert_eq!(Lanes::span(reg::V3, 0, 1), Lanes::span(reg::D3, 0, 1));
    assert!(Lanes::span(reg::X5, 0, 1).is_disjoint(&Lanes::span(reg::X6, 0, 1)));
    assert!(Lanes::span(reg::X0, 0, 1).is_disjoint(&Lanes::span(reg::V0, 0, 1)));
    assert!(Lanes::span(reg::X30, 0, 1).is_disjoint(&Lanes::flags(0xffff_ffff)));
    assert_eq!(Lanes::span(reg::X5, 0, 1).iter().count(), 1);
}

#[test]
fn without_the_file_an_arm64_register_is_no_lane_and_after_it_the_x86_lanes_are_back() {
    assert!(std::panic::catch_unwind(|| Lanes::span(reg::V3, 0, 1)).is_err(), "premise: the x86 table has no v3");
    {
        let _file = lanes::enter(Regs(&REGISTER_INFO));
        assert_eq!(Lanes::count(), 32 + 31 + 2 + 32, "32 flags, x0-x30, sp, xzr, v0-v31");
    }
    assert_eq!(Lanes::count(), 119, "x86's table again");
}
