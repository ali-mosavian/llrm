//! switch_peeled_table -Os (205 B / 88421 T against 182 B / 29429 T at -O1,
//! SDCC 225 B / 80104 T): `pick` (cost 125, the -Os threshold 50) stayed a call
//! in a loop of 256 trips, and a site over the threshold was never tried. A
//! call in a loop of 16 trips or more is tried inlined at a size build and kept
//! where the module shrinks or the copy saves
//! clocks (`Tti::inline_hot_trial_at_size`): 189 B / 29261 T.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_call_in_a_hot_loop_over_the_os_threshold_is_tried_inlined() {
    let ir = include_str!("data/switch_peeled_hot_os.ll");
    for level in [Level::Os, Level::Oz] {
        let asm = text(ir, "main", 0, &Request::new(level)).expect("compiles");
        assert!(!asm.contains("call _pick"), "{level:?}: pick stayed a call:\n{asm}");
    }
}
