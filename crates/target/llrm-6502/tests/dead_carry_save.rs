//! A dead carry save is dropped after allocation, not before: dropped before, the
//! free `a` made the allocator pick differently at each site of a repeated sum
//! chain and the outliner stopped sharing them (wide_shift_by_one -O2 843 -> 916
//! B, `jsr __outl_N` five times to none).

mod common;

use llrm_6502::driver::{Level, Request, assembly};

#[test]
fn a_repeated_sum_chain_is_still_one_outlined_routine() {
    let text = assembly(include_str!("data/shared_sum_chain.ll"), &Request::new(Level::O2)).unwrap();
    assert!(text.matches("jsr __outl_").count() >= 3, "{text}");
}
