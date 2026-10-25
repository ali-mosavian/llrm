use super::*;

/// A row `c >= sum(coefficient * variable)` from `[c, c1, c2, ...]`, the
/// variable of each coefficient its place. The constant entry stays when it
/// is 0, as a query's does (`negate` needs it); `add_row` drops it.
fn row(coefficients: &[i64]) -> Row {
    coefficients
        .iter()
        .enumerate()
        .map(|(id, &c)| Entry::new(c, id as u16))
        .filter(|e| e.id == 0 || e.coefficient != 0)
        .collect()
}

fn system(
    rows: &[&[i64]],
    variables: usize,
) -> ConstraintSystem {
    let mut system = ConstraintSystem::default();
    for one in rows {
        system.add_row(&row(one), variables);
    }
    system
}

/// The header's example: `0 >= x - 2y` and `3 >= 2x + 3y` eliminate `y` to
/// `6 >= 7x`, and a system with a solution has one.
#[test]
fn test_eliminating_the_last_variable_combines_rows_of_opposite_sign() {
    let mut s = system(&[&[0, 1, -2], &[3, 2, 3]], 2);
    assert!(s.eliminate_using_fm());
    assert_eq!(s.constraints, vec![vec![Entry::new(6, 0), Entry::new(7, 1)]]);
    assert_eq!(s.num_variables, 1);
    assert!(system(&[&[0, 1, -2], &[3, 2, 3]], 2).may_have_solution());
}

/// `x >= 3` (as `-3 >= -x`) and `x <= 1` (`1 >= x`) have no solution; `x >= 1`
/// and `x <= 3` have.
#[test]
fn test_a_system_with_contradicting_bounds_has_no_solution() {
    assert!(!system(&[&[-3, -1], &[1, 1]], 1).may_have_solution());
    assert!(system(&[&[-1, -1], &[3, 1]], 1).may_have_solution());
}

/// A row with no variable says nothing and is not kept.
#[test]
fn test_a_constant_only_row_is_not_added() {
    let mut s = ConstraintSystem::<u32>::default();
    assert!(!s.add_row(&row(&[5]), 0));
    assert!(!s.add_row(&[], 0));
    assert_eq!(s.size(), 0);
    assert!(s.add_row(&row(&[0, 1]), 1));
    assert_eq!(s.last_constraint(), &vec![Entry::new(1, 1)], "the 0 constant is dropped");
}

/// With `x >= 1` known, `x >= 0` is implied and `x >= 2` is not.
#[test]
fn test_a_condition_is_implied_by_a_stronger_bound() {
    let s = system(&[&[-1, -1]], 1);
    assert!(s.is_condition_implied(row(&[0, -1])));
    assert!(!s.is_condition_implied(row(&[-2, -1])));
    // Constant-only rows are decided by the sign.
    assert!(s.is_condition_implied(row(&[0])));
    assert!(!s.is_condition_implied(row(&[-1])));
}

/// `x <= y` and `y <= z` (as `0 >= x - y`, `0 >= y - z`) imply `x <= z`: the
/// elimination goes through `y`.
#[test]
fn test_a_chain_of_bounds_implies_the_ends() {
    let s = system(&[&[0, 1, -1, 0], &[0, 0, 1, -1]], 3);
    assert!(s.is_condition_implied(row(&[0, 1, 0, -1])));
    assert!(!s.is_condition_implied(row(&[0, -1, 0, 1])));
}

/// A coefficient that overflows drops the combination; the answer is still
/// "may have a solution".
#[test]
fn test_an_overflowing_combination_is_skipped() {
    let big = i64::MAX / 2 + 1;
    let mut s = system(&[&[0, big, -1], &[0, -big, big]], 2);
    assert!(s.may_have_solution());
}

/// The helpers of the header: negate adds one and flips, strict less-than
/// subtracts one, and an overflow gives the empty row.
#[test]
fn test_negate_and_strict_less_than() {
    assert_eq!(ConstraintSystem::<u32>::negate(row(&[3, 1, -2])), row(&[-4, -1, 2]));
    assert_eq!(ConstraintSystem::<u32>::negate_or_equal(row(&[3, 1])), row(&[-3, -1]));
    assert_eq!(ConstraintSystem::<u32>::to_strict_less_than(row(&[3, 1])), row(&[2, 1]));
    assert!(ConstraintSystem::<u32>::negate(row(&[i64::MAX, 1])).is_empty());
    assert!(ConstraintSystem::<u32>::negate_or_equal(row(&[i64::MIN, 1])).is_empty());
    assert!(ConstraintSystem::<u32>::to_strict_less_than(row(&[i64::MIN, 1])).is_empty());
}

/// Only the rows connected to the query's variables are kept, the variables
/// renumbered densely; the answer is the whole system's.
#[test]
fn test_the_sub_system_keeps_the_rows_connected_to_the_query() {
    // Rows over x1 (id 1), x2, x3, x4: x1 <= x2, and x3 <= x4 apart from them.
    let s = system(&[&[0, 1, -1, 0, 0], &[0, 0, 0, 1, -1]], 4);
    let query = row(&[0, 1, -1]);
    let (sub, mapped) = s.sub_system(&query);
    assert_eq!(sub.size(), 1);
    assert_eq!(sub.num_variables, 2);
    assert_eq!(mapped, row(&[0, 1, -1]));
    assert!(s.is_condition_implied_in_sub_system(&query));
    assert_eq!(s.is_condition_implied_in_sub_system(&query), s.is_condition_implied(query.clone()));
    // The other pair is renumbered 1, 2 too.
    let (other, remapped) = s.sub_system(&row(&[0, 0, 0, 1, -1]));
    assert_eq!((other.size(), remapped), (1, row(&[0, 1, -1])));
}
