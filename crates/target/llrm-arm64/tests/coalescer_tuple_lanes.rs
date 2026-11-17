//! A vector copied into member `i` of a tuple joins the tuple's `i`th register
//! (`getSubReg` through the sub-register index of `REG_SEQUENCE`'s copies), not
//! a byte of one root: a tuple's root is itself. Before, `hosts_at` asked for a
//! register of the same root and no member copy could ever be joined.

use llrm_arm64::REGISTER_INFO;
use llrm_codegen::coalescer::CoalescerEnv;
use llrm_codegen::coalescer::env::TargetEnv;
use llrm_lir::registers::{RegId, Regs};

fn env(palettes: &[(u32, Vec<RegId>)]) -> TargetEnv {
    TargetEnv {
        palettes: palettes
            .iter()
            .map(|(value, registers)| (*value, std::rc::Rc::new(registers.iter().copied().collect())))
            .collect::<llrm_support::hash::IndexMap<_, _>>()
            .into(),
        pins: Default::default(),
        size: false,
        keep_pinned: false,
        joins_global_copies: false,
        joins_pinned_copies: true,
        keeps_split_params: false,
        joins_sub_registers: true,
        joins_lanes: true,
        joins_loop_carried_first: false,
        aliases: Default::default(),
        should_join: None,
        freq: Vec::new(),
        regs: Some(Regs(&REGISTER_INFO)),
        kind_of: None,
        kinds: Default::default(),
        check_kinds: false,
        free_when_unnarrowed: false,
        remat: None,
        remat_wins: None,
    }
}

#[test]
fn a_member_joins_the_tuple_register_that_has_it_at_that_index() {
    let info = Regs(&REGISTER_INFO);
    let pair = info.by_name("v16_v17").expect("tuple");
    let (v16, v17) = (info.by_name("v16").unwrap(), info.by_name("v17").unwrap());
    // value 1 is the tuple, 2 and 3 its members.
    let env = env(&[(1, vec![pair]), (2, vec![v16]), (3, vec![v17])]);
    assert!(env.can_share_lane(3, 1, 16), "v17 is the 16 bytes at 16 of v16_v17");
    assert!(!env.can_share_lane(2, 1, 16), "v16 is at 0, not 16");
    assert!(!env.can_share_lane(3, 1, 32), "a pair has no member at 32");
}
