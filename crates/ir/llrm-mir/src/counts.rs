//! Always-on event counts, for tests of how often the compiler does a thing.
//!
//! # How it works
//! Each count is a plain integer in a thread-local cell, bumped where the event
//! happens: no atomic, no lock, no clock. `LLRM_DEBUG=counts` prints them when
//! `llrm-c` ends; a test reads them from the process it ran or from that
//! output. The analysis manager's hits and misses are kept by name.
//!
//! A count is exact and the same on every host, so a budget can be a ratio of
//! two sizes of one program (`tools/test_budgets.py`): a pass that is asked
//! once for the function must not be asked once for each loop.
//!
//! # Example
//! `LLRM_DEBUG=counts llrm-c -O2 f.c` ends with lines such as
//! `[counts] function_clones 3` and `[counts] analysis dominators 41 hits 7
//! misses`.
//!
//! # Used by
//! The pass manager (`passes`), `Function`'s copies, the machine phases and the
//! driver, which prints them.

use std::cell::{Cell, RefCell};

/// What is counted besides the analyses.
#[derive(Clone, Copy)]
pub enum Event {
    /// Every copy of a `Function`, a `fork` included.
    FunctionClones,
    /// The copies that continue the same lineage (`Function::fork`).
    FunctionForks,
    /// A function made through the machine phases, one candidate each.
    Machinings,
    /// A selection of a function's instructions.
    Selections,
    /// A register allocation of a function.
    Allocations,
    /// A dominator tree built.
    Dominators,
    /// A loop forest built (MIR).
    Forests,
    /// The natural loops of a LIR body found.
    LirLoops,
    /// A CFG `Shape` derived for a body no manager holds.
    Shapes,
    /// The known constants of a function worked out.
    Knowns,
    /// The allocas with an exposed address found.
    ExposedFrames,
    /// The live rows of a body solved.
    LiveRows,
}

const NAMES: [&str; 12] = [
    "function_clones",
    "function_forks",
    "machinings",
    "selections",
    "allocations",
    "dominators_built",
    "forests_built",
    "lir_loops_found",
    "shapes_derived",
    "knowns_worked_out",
    "exposed_frames_found",
    "live_rows_solved",
];

thread_local! {
    static EVENTS: [Cell<u64>; 12] = const { [const { Cell::new(0) }; 12] };
    /// (name, hits, misses) by the address of the name, which is a literal.
    static ANALYSES: RefCell<Vec<(&'static str, u64, u64)>> = const { RefCell::new(Vec::new()) };
}

pub fn bump(event: Event) {
    EVENTS.with(|events| {
        let one = &events[event as usize];
        one.set(one.get() + 1);
    });
}

pub fn get(event: Event) -> u64 {
    EVENTS.with(|events| events[event as usize].get())
}

/// A lookup of the analysis `name`: a hit when it was kept, else a miss.
pub fn analysis(
    name: &'static str,
    hit: bool,
) {
    ANALYSES.with(|rows| {
        let mut rows = rows.borrow_mut();
        let at = match rows.iter().position(|row| std::ptr::eq(row.0, name)) {
            Some(at) => at,
            None => {
                rows.push((name, 0, 0));
                rows.len() - 1
            }
        };
        if hit {
            rows[at].1 += 1;
        } else {
            rows[at].2 += 1;
        }
    });
}

/// (hits, misses) of the analysis `name` so far.
pub fn analysed(name: &str) -> (u64, u64) {
    ANALYSES.with(|rows| {
        rows.borrow().iter().filter(|row| row.0 == name).fold((0, 0), |sum, row| (sum.0 + row.1, sum.1 + row.2))
    })
}

/// The counts as `LLRM_DEBUG=counts` prints them.
pub fn lines() -> Vec<String> {
    let mut out: Vec<String> = NAMES
        .iter()
        .enumerate()
        .map(|(at, name)| format!("[counts] {name} {}", EVENTS.with(|events| events[at].get())))
        .collect();
    ANALYSES.with(|rows| {
        let mut by_name: std::collections::BTreeMap<&str, (u64, u64)> = std::collections::BTreeMap::new();
        for (name, hits, misses) in rows.borrow().iter() {
            let row = by_name.entry(name).or_default();
            *row = (row.0 + hits, row.1 + misses);
        }
        out.extend(
            by_name
                .iter()
                .map(|(name, (hits, misses))| format!("[counts] analysis {name} {hits} hits {misses} misses")),
        );
    });
    out
}

#[cfg(test)]
mod tests {
    use super::{Event, analysed, analysis, bump, get, lines};

    #[test]
    fn test_counts_add_up_by_event_and_by_analysis_name() {
        let before = get(Event::Allocations);
        bump(Event::Allocations);
        bump(Event::Allocations);
        assert_eq!(get(Event::Allocations) - before, 2);
        analysis("probe", true);
        analysis("probe", false);
        analysis("probe", false);
        assert_eq!(analysed("probe"), (1, 2));
        assert!(lines().contains(&"[counts] analysis probe 1 hits 2 misses".to_owned()));
    }
}
