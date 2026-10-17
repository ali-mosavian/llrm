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
use std::sync::atomic::{AtomicBool, Ordering};

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

const SLOTS: usize = 64;

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

/// A pass's runs and the instructions of the functions it ran over.
struct PassRow {
    name: &'static str,
    runs: u64,
    instructions: u64,
}

thread_local! {
    /// The function or module pass running, which an analysis miss is charged
    /// to.
    static ASKER: Cell<&'static str> = const { Cell::new("") };
    static PASSES: RefCell<Vec<PassRow>> = const { RefCell::new(Vec::new()) };
    /// (pass, analysis, misses): the analyses a pass made the manager compute.
    static ASKS: RefCell<Vec<(&'static str, &'static str, u64)>> = const { RefCell::new(Vec::new()) };
    static EVENTS: [Cell<u64>; 12] = const { [const { Cell::new(0) }; 12] };
    /// (name, hits, misses) by the address of the name, which is a literal.
    static FOUND: [Cell<usize>; SLOTS] = const { [const { Cell::new(0) }; SLOTS] };
    static ANALYSES: RefCell<Vec<(&'static str, u64, u64)>> = const { RefCell::new(Vec::new()) };
}

/// Puts the pass `name` running over a function of `instructions` instructions:
/// its run is counted, and the analyses the manager computes until the guard
/// drops are charged to it.
pub fn pass_run(
    name: &'static str,
    instructions: usize,
) -> Running {
    if !PER_PASS.load(Ordering::Relaxed) {
        return Running(None);
    }
    PASSES.with(|rows| {
        let mut rows = rows.borrow_mut();
        let at = match rows.iter().position(|row| std::ptr::eq(row.name, name)) {
            Some(at) => at,
            None => {
                rows.push(PassRow { name, runs: 0, instructions: 0 });
                rows.len() - 1
            }
        };
        rows[at].runs += 1;
        rows[at].instructions += instructions as u64;
    });
    Running(Some(ASKER.with(|asker| asker.replace(name))))
}

/// `pass_run` over `function`, by the instructions it has laid out now.
pub fn pass_run_over(
    name: &'static str,
    function: &crate::module::Function,
) -> Running {
    if !PER_PASS.load(Ordering::Relaxed) {
        return Running(None);
    }
    if withholding() {
        let key = (name, function.mark().lineage());
        FIRST.with(|first| {
            let mut seen = first.1.borrow_mut();
            first.0.set(!seen.contains(&key));
            seen.push(key);
        });
    }
    pass_run(name, function.layout().iter().map(|&block| function.block(block).instructions().len()).sum())
}

/// Charges the analyses computed to the pass that was running before.
pub struct Running(Option<&'static str>);

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(before) = self.0 {
            ASKER.with(|asker| asker.set(before));
        }
    }
}

/// Counts runs, sizes and who asked for what by pass (`LLRM_DEBUG=counts`'s
/// `pass` and `ask` lines): off, a pass costs nothing.
pub fn per_pass() {
    PER_PASS.store(true, Ordering::Relaxed);
}

static PER_PASS: AtomicBool = AtomicBool::new(false);

/// What `LLRM_WITHHOLD` names: `all`, or passes separated by commas, each with
/// the facts to withhold from it after a colon, joined by plus (`fold:
/// registers+through-memory,decide`). `LLRM_WITHHOLD_FIRST` withholds on a
/// pass's first run only. A test instrument.
struct Withhold {
    all: bool,
    passes: Vec<(String, Vec<String>)>,
    first: bool,
}

fn spec() -> &'static Withhold {
    static SPEC: std::sync::OnceLock<Withhold> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| {
        let text = std::env::var("LLRM_WITHHOLD").unwrap_or_default();
        let mut found = Withhold {
            all: text == "all",
            passes: Vec::new(),
            first: std::env::var_os("LLRM_WITHHOLD_FIRST").is_some(),
        };
        if !found.all {
            for part in text.split(',').filter(|part| !part.is_empty()) {
                let (pass, facts) = part.split_once(':').unwrap_or((part, ""));
                found
                    .passes
                    .push(
                        (
                            pass.to_owned(),
                            facts.split('+').filter(|one| !one.is_empty()).map(str::to_owned).collect(),
                        ),
                    );
            }
        }
        found
    })
}

/// Whether the withholding instrument is on (`LLRM_WITHHOLD` set).
pub fn withholding() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LLRM_WITHHOLD").is_some())
}

thread_local! {
    /// (pass, fact, withheld): the results handed out in place of the real
    /// ones. (whether the run in progress is its pass's first over its
    /// function, the (pass, function) pairs run).
    static FIRST: (Cell<bool>, RefCell<Vec<(&'static str, u64)>>) = const { (Cell::new(false), RefCell::new(Vec::new())) };
    static WITHHELD: RefCell<Vec<(&'static str, &'static str, u64)>> = const { RefCell::new(Vec::new()) };
    /// (pass, stage, edits): the changes logged by each run of each pass.
    static EDITS: RefCell<Vec<(&'static str, String, u64)>> = const { RefCell::new(Vec::new()) };
}

/// Whether `fact` is to be withheld from the pass running now.
pub fn named(fact: &'static str) -> bool {
    let pass = ASKER.with(Cell::get);
    if pass.is_empty() {
        return false;
    }
    let spec = spec();
    let named = spec.all
        || spec
            .passes
            .iter()
            .any(|(one, facts)| one == pass && (facts.is_empty() || facts.iter().any(|one| one == fact)));
    // The pass's first run over this function (`LLRM_WITHHOLD_FIRST`).
    named && (!spec.first || FIRST.with(|first| first.0.get()))
}

/// `fact` was withheld from the pass running now.
pub fn withheld(fact: &'static str) {
    let pass = ASKER.with(Cell::get);
    WITHHELD.with(|rows| {
        let mut rows = rows.borrow_mut();
        match rows.iter_mut().find(|row| row.0 == pass && row.1 == fact) {
            Some(row) => row.2 += 1,
            None => rows.push((pass, fact, 1)),
        }
    });
}

/// The changes a run of `pass` in `stage` logged.
pub fn edited(
    pass: &'static str,
    stage: &str,
    edits: usize,
) {
    if !PER_PASS.load(Ordering::Relaxed) {
        return;
    }
    EDITS.with(|rows| {
        let mut rows = rows.borrow_mut();
        match rows.iter_mut().find(|row| row.0 == pass && row.1 == stage) {
            Some(row) => row.2 += edits as u64,
            None => rows.push((pass, stage.to_owned(), edits as u64)),
        }
    });
}

fn asked(analysis: &'static str) {
    let pass = ASKER.with(Cell::get);
    ASKS.with(|rows| {
        let mut rows = rows.borrow_mut();
        match rows.iter_mut().find(|row| std::ptr::eq(row.0, pass) && std::ptr::eq(row.1, analysis)) {
            Some(row) => row.2 += 1,
            None => rows.push((pass, analysis, 1)),
        }
    });
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
    // The row last found for a literal's address, by a few bits of it: a scan
    // of every name per lookup was most of the cost.
    let slot = (name.as_ptr() as usize >> 3) & (SLOTS - 1);
    ANALYSES.with(|rows| {
        let mut rows = rows.borrow_mut();
        let guess = FOUND.with(|found| found[slot].get());
        let at = match rows.get(guess) {
            Some(row) if std::ptr::eq(row.0, name) => guess,
            _ => {
                let at = match rows.iter().position(|row| std::ptr::eq(row.0, name)) {
                    Some(at) => at,
                    None => {
                        rows.push((name, 0, 0));
                        rows.len() - 1
                    }
                };
                FOUND.with(|found| found[slot].set(at));
                at
            }
        };
        if hit {
            rows[at].1 += 1;
        } else {
            rows[at].2 += 1;
        }
    });
    if !hit && PER_PASS.load(Ordering::Relaxed) {
        asked(name);
    }
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
    let (runs, computed) = (
        PASSES.with(|rows| rows.borrow().iter().map(|row| row.runs).sum::<u64>()),
        ANALYSES.with(|rows| rows.borrow().iter().map(|row| row.2).sum::<u64>()),
    );
    out.push(format!("[counts] pass_runs {runs}"));
    out.push(format!("[counts] analysis_computations {computed}"));
    PASSES.with(|rows| {
        let mut rows: Vec<_> = rows.borrow().iter().map(|row| (row.name, row.runs, row.instructions)).collect();
        rows.sort();
        out.extend(
            rows.iter().map(|(name, runs, instructions)| {
                format!("[counts] pass {name} {runs} runs {instructions} instructions")
            }),
        );
    });
    WITHHELD.with(|rows| {
        let mut rows: Vec<_> = rows.borrow().iter().copied().collect();
        rows.sort();
        out.extend(rows.iter().map(|(pass, fact, n)| format!("[counts] withheld {pass} {fact} {n}")));
    });
    EDITS.with(|rows| {
        let mut rows: Vec<_> = rows.borrow().iter().cloned().collect();
        rows.sort();
        out.extend(
            rows.iter().map(|(pass, stage, n)| format!("[counts] edits {pass} {} {n}", stage.replace(' ', "_"))),
        );
    });
    ASKS.with(|rows| {
        let mut rows: Vec<_> = rows.borrow().iter().copied().collect();
        rows.sort();
        out.extend(rows.iter().map(|(pass, analysis, misses)| {
            format!("[counts] ask {} {analysis} {misses} misses", if pass.is_empty() { "-" } else { pass })
        }));
    });
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
    use super::{Event, analysed, analysis, bump, get, lines, pass_run, per_pass};

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

    /// A miss was charged to no pass, so the table that says which pass makes
    /// the manager compute what could not be told from the pass-less ones
    /// (the pipeline's own asks, the drivers').
    #[test]
    fn test_a_miss_is_charged_to_the_pass_running_and_a_run_to_its_size() {
        per_pass();
        {
            let _outer = pass_run("probe-outer", 10);
            {
                let _inner = pass_run("probe-inner", 4);
                analysis("probe-analysis", false);
            }
            analysis("probe-analysis", false);
            analysis("probe-analysis", true);
        }
        analysis("probe-analysis", false);
        let text = lines();
        assert!(text.contains(&"[counts] pass probe-inner 1 runs 4 instructions".to_owned()));
        assert!(text.contains(&"[counts] pass probe-outer 1 runs 10 instructions".to_owned()));
        assert!(text.contains(&"[counts] ask probe-inner probe-analysis 1 misses".to_owned()));
        assert!(text.contains(&"[counts] ask probe-outer probe-analysis 1 misses".to_owned()));
        assert!(text.contains(&"[counts] ask - probe-analysis 1 misses".to_owned()), "outside a pass: {text:?}");
    }
}
