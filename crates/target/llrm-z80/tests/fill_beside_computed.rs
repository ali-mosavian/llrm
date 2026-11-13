//! A constant store beside a computed one in a long loop is one `ldir` fill.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// With the rate stated for every speed decision, loop idiom left `dst[i] = 0xEE` in the loop beside
/// `src[i] = i * 7` (a store and the spilled pointer a trip, no `ldir`): memory_expansion `reset`, 1200 trips, 13M T.
#[test]
fn a_constant_store_beside_a_computed_one_over_many_trips_is_an_ldir_fill() {
    for level in [Level::O2, Level::O3] {
        let asm = text(include_str!("kernels/ir/fill_beside_computed.ll"), "g", 0, &Request::new(level)).expect("compiles");
        assert!(asm.contains("ldir"), "{level:?}: no fill:\n{asm}");
    }
}
