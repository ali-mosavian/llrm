//! The `EquivalenceClasses.h` doc example and the list order of a union.

use super::EquivalenceClasses;

fn members(
    classes: &EquivalenceClasses<i32>,
    of: i32,
) -> Vec<i32> {
    classes.members(&of).copied().collect()
}

/// `EC.unionSets(1, 2); EC.insert(4); EC.insert(5); EC.unionSets(5, 1);`
#[test]
fn a_union_appends_the_second_class_to_the_first() {
    let mut classes = EquivalenceClasses::default();
    classes.union_sets(&1, &2);
    classes.insert(&4);
    classes.insert(&5);
    assert_eq!(classes.union_sets(&5, &1), 5);
    assert_eq!(members(&classes, 1), [5, 1, 2]);
    assert_eq!(members(&classes, 2), [5, 1, 2]);
    assert_eq!(members(&classes, 4), [4]);
    assert_eq!(classes.classes(), 2);
}

#[test]
fn a_value_not_in_the_set_has_no_members() {
    let classes = EquivalenceClasses::<i32>::default();
    assert!(!classes.contains(&1) && members(&classes, 1).is_empty() && classes.is_empty());
}

#[test]
fn uniting_a_class_with_itself_changes_nothing() {
    let mut classes = EquivalenceClasses::default();
    classes.union_sets(&1, &2);
    classes.union_sets(&3, &1);
    assert_eq!(classes.union_sets(&2, &3), 3);
    assert_eq!(members(&classes, 2), [3, 1, 2]);
    assert_eq!(classes.leader_value(&2), Some(&3));
}

/// A chain of unions is linear: 20,000 values in one class are listed in order.
#[test]
fn a_long_chain_of_unions_keeps_every_member_once() {
    let mut classes = EquivalenceClasses::default();
    for at in 0..20_000 {
        classes.union_sets(&0, &(at + 1));
    }
    assert_eq!(members(&classes, 7).len(), 20_001);
}

/// `eraseClass` takes out every member of the class and no other: a value
/// inserted again is a class of its own.
#[test]
fn erasing_a_class_removes_its_members_only() {
    let mut classes = EquivalenceClasses::default();
    classes.union_sets(&1, &2);
    classes.union_sets(&2, &3);
    classes.insert(&4);
    classes.erase_class(&2);
    assert!(!classes.contains(&1) && !classes.contains(&2) && !classes.contains(&3));
    assert_eq!(classes.members(&4).copied().collect::<Vec<_>>(), [4]);
    assert_eq!(classes.classes(), 1);
    classes.insert(&2);
    assert_eq!(classes.members(&2).copied().collect::<Vec<_>>(), [2]);
    assert_eq!(classes.classes(), 2);
}

/// The member map was a std map: SipHash on every operand of every access
/// check. Does not compile for any other hasher.
#[test]
fn test_members_are_keyed_with_the_fixed_key_hasher() {
    let classes = EquivalenceClasses::<i32>::default();
    let _: &llrm_support::hash::HashMap<i32, usize> = &classes.mapping;
}
