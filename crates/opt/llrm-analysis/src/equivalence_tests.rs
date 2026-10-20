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
