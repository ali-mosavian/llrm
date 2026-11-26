//! Asking whether a selected node is a float row allocated for every
//! candidate name it tried (`format!` per vector form, 57 per node): 148 M of
//! a 3.7 G compile went to building strings that were compared and dropped.

use llrm_arm64::dag::emit::names_a_float_row;
use llrm_support::alloc_count::{Counting, counted};

#[global_allocator]
static COUNTING: Counting = Counting;

#[test]
fn a_row_that_is_no_float_row_is_refused_without_allocating() {
    assert!(names_a_float_row("FADDDrr"), "the premise: a float row is read");
    assert!(names_a_float_row("DUPi32"), "the premise: so is a vector one");
    let (all_refused, made) = counted(|| {
        (0..50).all(|_| {
            !std::hint::black_box(names_a_float_row("ADDXrr")) && !std::hint::black_box(names_a_float_row("LDRXui"))
        })
    });
    assert!(all_refused);
    assert_eq!(made, 0, "{made} allocations to refuse 100 integer rows");
}
