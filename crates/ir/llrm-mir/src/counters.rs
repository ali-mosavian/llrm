//! Counts of work done, always on: an integer increment at each site. The
//! compile-time budgets are counts, not times, so a test can hold a stage to
//! them exactly; `LLRM_DEBUG=counts` prints them when the compile ends (the
//! binary calls `report`), and
//! `tools/budgets.py` reads that over the fixtures.
//!
//! Counters are per thread, as the compiler's work is: a test reads its own.

use std::cell::Cell;

/// What is counted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Count {
    /// Function passes run on a function.
    PassRuns,
    /// Instructions the functions had (the arena, erased included) where a pass
    /// ran, summed: the size of the work a full run reads.
    PassRunInstructions,
    /// An analysis worked out from nothing.
    AnalysisComputed,
    /// An analysis brought up to date from the change log.
    AnalysisUpdated,
    /// An analysis kept past changes that did not reach it.
    AnalysisKept,
    /// An analysis asked and held.
    AnalysisHits,
    /// A module analysis worked out.
    ModuleComputed,
    /// A function put in place by its edited copy.
    FunctionForks,
    /// A register allocation of a function.
    Allocations,
    /// The LIR phases built for a function.
    LirPipelineRuns,
    /// A set of values spilled.
    Spills,
    /// A value split.
    Splits,
}

const NAMES: [(&str, Count); 12] = [
    ("pass_runs", Count::PassRuns),
    ("pass_run_instructions", Count::PassRunInstructions),
    ("analysis_computed", Count::AnalysisComputed),
    ("analysis_updated", Count::AnalysisUpdated),
    ("analysis_kept", Count::AnalysisKept),
    ("analysis_hits", Count::AnalysisHits),
    ("module_computed", Count::ModuleComputed),
    ("function_forks", Count::FunctionForks),
    ("allocations", Count::Allocations),
    ("lir_pipeline_runs", Count::LirPipelineRuns),
    ("spills", Count::Spills),
    ("splits", Count::Splits),
];

thread_local! {
    static COUNTS: [Cell<u64>; 12] = const { [const { Cell::new(0) }; 12] };
}

/// Adds `by` to `count`.
pub fn bump(
    count: Count,
    by: u64,
) {
    COUNTS.with(|counts| {
        let one = &counts[count as usize];
        one.set(one.get() + by);
    });
}

/// How many of `count` this thread has seen.
pub fn get(count: Count) -> u64 {
    COUNTS.with(|counts| counts[count as usize].get())
}

/// Every count, by name.
pub fn all() -> Vec<(&'static str, u64)> {
    NAMES.iter().map(|&(name, count)| (name, get(count))).collect()
}

/// Starts this thread's counts again from nothing, for a test.
pub fn reset() {
    COUNTS.with(|counts| counts.iter().for_each(|one| one.set(0)));
}

/// The `counts` channel's report: one line, `name=value` for each count.
pub fn report() {
    let on = std::env::var("LLRM_DEBUG")
        .is_ok_and(|value| value.split(',').any(|one| matches!(one.trim(), "counts" | "all")));
    if !on {
        return;
    }
    let line: Vec<String> = all().into_iter().map(|(name, value)| format!("{name}={value}")).collect();
    eprintln!("[counts] {}", line.join(" "));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_is_its_own_and_reset_clears_it() {
        reset();
        bump(Count::Splits, 3);
        bump(Count::Spills, 1);
        assert_eq!((get(Count::Splits), get(Count::Spills), get(Count::Allocations)), (3, 1, 0));
        assert!(all().contains(&("splits", 3)));
        reset();
        assert_eq!(get(Count::Splits), 0);
    }

    /// The names table and the enum must agree, or a count is read under
    /// another's name.
    #[test]
    fn every_count_is_named_at_its_own_place() {
        for (at, &(_, count)) in NAMES.iter().enumerate() {
            assert_eq!(count as usize, at);
        }
    }
}
