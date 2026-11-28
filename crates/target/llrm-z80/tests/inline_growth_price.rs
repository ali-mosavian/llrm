//! A callee whose copies add bytes the clocks they save do not repay stays a
//! call at -O2. The speed trial priced the copies by the forms' own bytes (0.54
//! of what is written, so `sum` read 22 B against 146) and the
//! no-growth variant did not bind it, so the module with the calls was never
//! tried: memmove_overlap -O2 549 B / 45498 T against 387 B / 31789 T at -O1;
//! shift_int64_masked_amount -O2 893 B / 4276255 T against 812 B / 3559721 T.

use llrm_z80::compile::{effective, module_priced};
use llrm_z80::driver::{Level, Request};

fn priced(
    ir: &str,
    level: Level,
) -> (u32, u64) {
    let request = effective(ir, &Request::new(level)).expect("chooses");
    module_priced(ir, &request).expect("prices")
}

fn o2_is_not_dominated_by_o1(ir: &str) {
    let (o1, o2) = (priced(ir, Level::O1), priced(ir, Level::O2));
    assert!(o2.0 <= o1.0 && o2.1 <= o1.1, "-O2 {o2:?} against -O1 {o1:?}");
}

#[test]
fn memmove_overlap_o2_keeps_the_calls_of_fill_and_sum() {
    o2_is_not_dominated_by_o1(include_str!("data/memmove_overlap_o2.ll"));
}
