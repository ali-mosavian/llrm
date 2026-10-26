use llrm_lir::registers::RegId;

use super::HintWalk;
use crate::support::hash::{HashSet, IndexMap};

fn register(number: u32) -> RegId {
    crate::backend::allocate::_whole(RegId::values().nth(number as usize + 1).expect("a register"))
}

fn graph(edges: &[(u32, &[u32])]) -> IndexMap<u32, Vec<u32>> {
    edges.iter().map(|(one, list)| (*one, list.to_vec())).collect()
}

fn pins(list: &[(u32, u32)]) -> IndexMap<u32, RegId> {
    list.iter().map(|(one, number)| (*one, register(*number))).collect()
}

/// A value wants the register of the nearest pin, the smaller value among
/// equals.
#[test]
fn the_nearest_pin_wins_and_a_tie_goes_to_the_smaller_value() {
    let hints = graph(&[(10, &[1, 2]), (11, &[2]), (1, &[3]), (2, &[3]), (3, &[4])]);
    let pinned = pins(&[(10, 0), (11, 1)]);
    let walk = HintWalk::fresh(&hints, &pinned, 128);
    let wants = |one: u32| walk.wanted().get(&one).copied();
    assert_eq!(wants(1), Some(register(0)));
    assert_eq!(wants(2), Some(register(0)), "10 and 11 are one copy from 2: the smaller pin");
    assert_eq!(wants(3), Some(register(0)), "3 is two copies from 10 by 1 and by 2: through the smaller, 1");
    assert_eq!(wants(4), Some(register(0)));
}

/// The walk stops `depth` copies from a pin.
#[test]
fn the_walk_stops_at_its_depth() {
    let hints = graph(&[(10, &[1]), (1, &[2]), (2, &[3])]);
    let walk = HintWalk::fresh(&hints, &pins(&[(10, 0)]), 2);
    assert_eq!(walk.wanted().keys().copied().collect::<std::collections::BTreeSet<_>>(), [1, 2].into());
}

/// A rewrite names a value and changes its hints: what came through it is
/// found again, and the rest is as it was.
#[test]
fn a_rewrite_that_names_a_value_changes_what_comes_through_it() {
    let hints = graph(&[(10, &[1]), (1, &[2, 3]), (2, &[4])]);
    let pinned = pins(&[(10, 0)]);
    let mut walk = HintWalk::fresh(&hints, &pinned, 128);
    assert_eq!(walk.wanted().len(), 4);
    let after = graph(&[(10, &[1]), (1, &[2, 3])]);
    walk.update(&after, &pinned, Some(&[2].into_iter().collect::<HashSet<u32>>()));
    assert_eq!(walk.wanted().keys().copied().collect::<std::collections::BTreeSet<_>>(), [1, 2, 3].into());
}

/// Edits of every kind to a random graph, each followed by an update that names
/// the values it changed: the walk kept up to date is the walk made afresh.
#[test]
fn a_walk_kept_up_to_date_is_the_walk_made_afresh() {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move |most: u32| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % u64::from(most)) as u32
    };
    for round in 0..1000 {
        let values = 6 + next(30);
        let mut hints: IndexMap<u32, Vec<u32>> = IndexMap::default();
        let mut pinned: IndexMap<u32, RegId> = IndexMap::default();
        for _ in 0..(1 + next(3)) {
            pinned.insert(next(values), register(next(4)));
        }
        let relist = |hints: &mut IndexMap<u32, Vec<u32>>, one: u32, next: &mut dyn FnMut(u32) -> u32| {
            let list: Vec<u32> = (0..next(4)).map(|_| next(values)).filter(|other| *other != one).collect();
            if list.is_empty() {
                hints.swap_remove(&one);
            } else {
                hints.insert(one, list);
            }
        };
        for one in 0..values {
            relist(&mut hints, one, &mut next);
        }
        let depth = [2, 4, 128][next(3) as usize];
        let mut walk = HintWalk::fresh(&hints, &pinned, depth);
        for step in 0..30 {
            let mut touched: HashSet<u32> = HashSet::default();
            for _ in 0..(1 + next(3)) {
                let one = next(values + 2);
                touched.insert(one);
                relist(&mut hints, one, &mut next);
            }
            if next(6) == 0 {
                let one = next(values);
                if !pinned.contains_key(&one) {
                    pinned.insert(one, register(next(4)));
                    touched.insert(one);
                }
            }
            walk.update(&hints, &pinned, Some(&touched));
            if let Err(why) = walk.invariants() {
                panic!(
                    "round {round} step {step}: {why}; touched {touched:?} hints {hints:?} pinned {:?}",
                    pinned.iter().collect::<Vec<_>>()
                );
            }
            let fresh = HintWalk::fresh(&hints, &pinned, depth);
            assert_eq!(
                walk.wanted(),
                fresh.wanted(),
                "round {round} step {step}: depth {depth}, pinned {:?} touched {touched:?} hints {hints:?}",
                pinned.iter().collect::<Vec<_>>()
            );
        }
    }
}

/// A chain of copies behind a pin, a rewrite naming one value at a time and
/// giving it a copy of its own: walked afresh after each, N rewrites came to N
/// squared looks; the work is in proportion to what the rewrite changes.
#[test]
fn the_work_of_a_chain_of_rewrites_grows_with_the_chain_not_with_its_square() {
    let work = |n: u32| {
        let chain = |extra: Option<u32>| -> IndexMap<u32, Vec<u32>> {
            (0..n)
                .map(|one| {
                    let mut list = vec![one + 1];
                    if extra == Some(one) {
                        list.push(n + 1 + one);
                    }
                    (one, list)
                })
                .collect()
        };
        let pinned = pins(&[(0, 0)]);
        let mut walk = HintWalk::fresh(&chain(None), &pinned, 1 << 20);
        let start = walk.work();
        for at in 1..n - 1 {
            walk.update(&chain(Some(at)), &pinned, Some(&[at].into_iter().collect()));
            walk.update(&chain(None), &pinned, Some(&[at].into_iter().collect()));
        }
        walk.work() - start
    };
    let (small, big) = (work(64), work(128));
    assert!(big < small * 3, "128 values took {big} looks against {small} for 64: a square would be four times");
}
