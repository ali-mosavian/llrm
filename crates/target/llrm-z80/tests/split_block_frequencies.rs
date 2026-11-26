//! Tail merging splits loop blocks into new ones. At -Os the fold had no
//! frequency map, and the lines of one allocation were weighed by the map of
//! the last one made: the inner loop was priced as run once an entry of the
//! outer, and the choice between LSR's orders rejected a much smaller and
//! faster module.

use llrm_z80::compile::{effective, module_priced};
use llrm_z80::driver::{Level, Request};

#[test]
fn a_loop_nest_merged_at_size_is_priced_by_its_trips_and_takes_the_smaller_form() {
    let ir = include_str!("data/nested_vector_add.ll");
    let base = Request::new(Level::Os);
    let held = module_priced(ir, &base).expect("prices");
    // 100 trips of 101 trips of three loads, adds and stores
    assert!(held.1 > 100 * 101 * 100, "the inner loop is priced at {} T", held.1);
    let chosen = effective(ir, &base).expect("chooses");
    let kept = module_priced(ir, &chosen).expect("prices");
    assert!(kept.0 * 2 < held.0 && kept.1 < held.1, "kept {kept:?} beside {held:?}");
}
