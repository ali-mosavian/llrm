//! A loop with a call keeps one induction variable, not one for each scaled use
//! of it. switch_peeled_table -Os: `m * 16` was an induction variable of its
//! own, stepped by `ld hl, #16; add hl, de` and stored to the frame every trip
//! (the call to `pick` leaves nothing in a register): 97,875 T, against 88,621
//! T with the use taken from `m` by four shifts.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = include_str!("data/switch_peeled_table_os.ll");

#[test]
fn a_scaled_use_across_a_call_is_taken_from_the_counter_not_kept_as_a_second_one() {
    let main = text(IR, "main", 1, &Request::new(Level::Os)).expect("compiles");
    assert!(!main.contains("#16"), "`m * 16` is stepped by 16 as an induction variable of its own:\n{main}");
}
