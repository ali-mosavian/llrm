//! `InstWorklist` against LLVM's `InstructionWorklist` at the pin: the order
//! of `add`/`pop_deferred` and `push`/`remove_one`, dedup, and removal.

use llrm_mir::module::InstId;

use crate::instworklist::InstWorklist;

fn id(n: u32) -> InstId {
    InstId(n)
}

fn drained_deferred(list: &mut InstWorklist) -> Vec<u32> {
    std::iter::from_fn(|| list.pop_deferred()).map(|one| one.0).collect()
}

fn drained(list: &mut InstWorklist) -> Vec<u32> {
    std::iter::from_fn(|| list.remove_one()).map(|one| one.0).collect()
}

/// The deferred list pops the last one added first.
#[test]
fn deferred_instructions_pop_last_added_first() {
    let mut list = InstWorklist::default();
    for n in [1, 2, 3] {
        list.add(id(n));
    }
    assert_eq!(drained_deferred(&mut list), [3, 2, 1]);
}

/// Adding twice keeps the first position, as `SmallSetVector::insert` does.
#[test]
fn an_instruction_added_twice_stays_where_it_was_first_added() {
    let mut list = InstWorklist::default();
    for n in [1, 2, 1] {
        list.add(id(n));
    }
    assert_eq!(drained_deferred(&mut list), [2, 1]);
}

/// The stack pops the last pushed first, and holds an instruction once.
#[test]
fn the_stack_pops_last_pushed_first_and_dedups() {
    let mut list = InstWorklist::default();
    for n in [1, 2, 1, 3] {
        list.push(id(n));
    }
    assert_eq!(drained(&mut list), [3, 2, 1]);
}

/// A driver moves each deferred instruction to the stack, then pops the stack:
/// they come out in the order they were added.
#[test]
fn deferred_moved_to_the_stack_come_out_in_the_order_added() {
    let mut list = InstWorklist::default();
    for n in [5, 6, 7] {
        list.add(id(n));
    }
    while let Some(one) = list.pop_deferred() {
        list.push(one);
    }
    assert_eq!(drained(&mut list), [5, 6, 7]);
}

/// `remove` takes an instruction off both lists and leaves the order of the
/// rest.
#[test]
fn remove_takes_an_instruction_off_both_lists() {
    let mut list = InstWorklist::default();
    for n in [1, 2, 3] {
        list.add(id(n));
        list.push(id(n));
    }
    list.remove(id(2));
    assert_eq!(drained_deferred(&mut list), [3, 1]);
    assert_eq!(drained(&mut list), [3, 1]);
}

/// Removed from the middle, then added again: it goes to the end, not back to
/// its old slot.
#[test]
fn an_instruction_added_after_removal_goes_to_the_end() {
    let mut list = InstWorklist::default();
    for n in [1, 2, 3] {
        list.push(id(n));
    }
    list.remove(id(1));
    list.push(id(1));
    assert_eq!(drained(&mut list), [1, 3, 2]);
}

/// A removed entry at the end is stepped over: the pop returns the one before
/// it, and `is_empty` is true once only removed entries remain.
#[test]
fn a_removed_entry_is_stepped_over() {
    let mut list = InstWorklist::default();
    for n in [1, 2] {
        list.push(id(n));
    }
    list.remove(id(2));
    assert!(!list.is_empty());
    assert_eq!(list.remove_one(), Some(id(1)));
    assert!(list.is_empty());
    assert_eq!(list.remove_one(), None);
    list.zap();
}

#[test]
fn removing_an_instruction_not_on_a_list_changes_nothing() {
    let mut list = InstWorklist::default();
    list.push(id(1));
    list.remove(id(9));
    assert_eq!(drained(&mut list), [1]);
}

/// Either list non-empty makes the worklist non-empty.
#[test]
fn the_worklist_is_empty_only_when_both_lists_are() {
    let mut list = InstWorklist::default();
    assert!(list.is_empty());
    list.add(id(1));
    assert!(!list.is_empty());
    list.pop_deferred();
    list.push(id(1));
    assert!(!list.is_empty());
}

/// Half of 2000 instructions removed, the rest come out in order.
#[test]
fn many_removals_keep_the_order_of_the_rest() {
    let mut list = InstWorklist::default();
    list.reserve(2000);
    (0..2000).for_each(|n| list.push(id(n)));
    (0..1000).for_each(|n| list.remove(id(n * 2)));
    let odd: Vec<u32> = (0..1000).rev().map(|n| n * 2 + 1).collect();
    assert_eq!(drained(&mut list), odd);
}
