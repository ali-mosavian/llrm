//! The byte prices the -Os trial counts a function by.

use llrm_target::opcosts::Description;

/// The trial priced a QB `unpack` copy -22 B where the objects grew +5: a load,
/// a store and an address were each 6 B against 3.8, 3.4 and 2.0 fitted to 1228
/// assembled functions, and the `ret` that ends a frame 1 against 9.4.
#[test]
fn the_trial_prices_load_store_address_and_return_as_the_objects_measure() {
    let costs = Description::parse(include_str!("../src/opcosts.txt")).expect("opcosts.txt parses").size_costs();
    assert_eq!((costs.load, costs.store, costs.address, costs.return_), (4, 4, 2, 9));
}
