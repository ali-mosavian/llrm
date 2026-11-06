//! A block of 300 bytes ends in a `q` access at offset 284, which `stur` and
//! `ldur` (9 bits) and `str q` (a multiple of 16) cannot encode: the object
//! failed ("an unscaled offset 284 does not fit 9 bits") for every
//! `__builtin_memset(d, 0, 300)` and tests/run/c/memory_expansion.

use llrm_arm64::driver::{self, IselMode, Level};
use llrm_arm64::machine::Platform;

#[test]
fn a_block_access_past_the_unscaled_range_is_encoded() {
    driver::set_isel_mode(IselMode::Dag);
    for level in [Level::O1, Level::Os] {
        driver::object(include_str!("data/memset_offset.ll"), Platform::Linux, "t", level)
            .unwrap_or_else(|why| panic!("{level:?}: {why}"));
    }
}
