//! Which allocation `LLRM_GREEDY_DECISIONS` traces.
//!
//! A caller that allocates one function several times (a target choosing the
//! cheapest of its candidates) gets a trace of all of them, interleaved. Each
//! allocation is numbered in the order they start, the `== name` line carries
//! the number, and `LLRM_GREEDY_DECISIONS_CAND=n` traces allocation `n` only.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

static STARTED: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(true) };
}

/// Whether decision lines are printed now.
pub fn on() -> bool {
    llrm_support::env_flag!("LLRM_GREEDY_DECISIONS") && ACTIVE.get()
}

/// How many allocations have started: the number the next one gets.
pub fn started() -> usize {
    STARTED.load(Ordering::Relaxed)
}

/// Whether allocation `number` is traced when `wanted` is the one asked for.
fn selects(
    wanted: Option<usize>,
    number: usize,
) -> bool {
    wanted.is_none_or(|wanted| wanted == number)
}

/// An allocation of `name` starts: numbers it and selects whether it is traced.
pub fn begin(name: &str) -> usize {
    let number = STARTED.fetch_add(1, Ordering::Relaxed);
    let wanted =
        llrm_support::env_var("LLRM_GREEDY_DECISIONS_CAND").and_then(|one| one.to_str()?.parse::<usize>().ok());
    ACTIVE.set(selects(wanted, number));
    if on() {
        eprintln!("== {name} #{number}");
    }
    number
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eight interleaved allocations of one function could not be told apart:
    /// each is numbered, and the filter keeps one.
    #[test]
    fn allocations_are_numbered_and_the_filter_keeps_one() {
        let first = begin("f");
        let second = begin("f");
        assert_eq!(second, first + 1);
        assert_eq!(started(), second + 1);
        assert!(selects(None, 3));
        assert!(selects(Some(3), 3));
        assert!(!selects(Some(3), 4));
    }
}
