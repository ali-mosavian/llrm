//! The state a target's driver and a pass share for a trial compile: the module is
//! compiled again with one decision of the pass reversed, and the driver keeps
//! whichever result it prices smaller or cheaper. Two trials use it, UNROLL-TRIAL and
//! ROTATION-TRIAL.
//!
//! DEVIATE UNROLL-TRIAL (beyond the pin; owner 6502, exit 2026-12-05): the
//! state the target's driver and the pass share for a trial unroll.
//!
//! The pin decides a full unroll by a threshold and by what the loop's own
//! body folds (`analyzeLoopUnrollCost`, at most ten trips). A loop whose
//! copies fold the loads of a later loop (an array filled by one loop and read
//! by the next) is seen by neither: the pin unrolls it only because its
//! threshold is large in its own units. Here the pass notes that it refused a
//! loop the target's trial size admits; the driver then compiles the program
//! again with every loop under that size unrolled and keeps whichever
//! assembly it prices cheaper.

use std::cell::{Cell, RefCell};

thread_local! {
    /// The unrolled size under which every loop is unrolled, set while the
    /// trial compile runs.
    static TRIAL: Cell<Option<u32>> = const { Cell::new(None) };
    /// The functions with a loop the threshold refused that a trial may take,
    /// and the unrolled size of those loops together.
    static ELIGIBLE: RefCell<(Vec<String>, u64)> = const { RefCell::new((Vec::new(), 0)) };
    /// Set while the trial compile of ROTATION-TRIAL runs: a counted loop whose
    /// copied guard does not fold stays unrotated.
    static UNROTATED: Cell<bool> = const { Cell::new(false) };
    /// The functions with a loop that ROTATION-TRIAL rotated and a trial may
    /// leave unrotated.
    static ROTATED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// The loops ROTATION-TRIAL noted, counted apart from the functions they are in.
    static ROTATED_LOOPS: Cell<usize> = const { Cell::new(0) };
    /// Set while the trial compile of ROTATION-HOT runs: a loop whose copied guard does not fold and whose
    /// trips are not counted is rotated as well.
    static MORE: Cell<bool> = const { Cell::new(false) };
    /// The functions, and loops together, ROTATION-HOT left unrotated.
    static LEFT: RefCell<(Vec<String>, usize)> = const { RefCell::new((Vec::new(), 0)) };
}

thread_local! {
    /// Set while the trial compile of LOOP-COPY-TRIAL runs: a callee that holds a loop is not inlined at
    /// several sites of a loop of its caller.
    static REFUSE_LOOP_COPIES: Cell<bool> = const { Cell::new(false) };
    /// The inlines of that kind the pass made since [`take_loop_copies`].
    static LOOP_COPIES: Cell<usize> = const { Cell::new(0) };
}

/// DEVIATE LOOP-COPY-TRIAL (beyond the pin; owner z80, exit 2026-11-24): whether the trial compile that
/// refuses the copies of a loop-holding callee at several sites inside a loop is running. The pin's
/// InlineCost prices the call away and the callee's body, never the registers the merged loop needs (its
/// `LoopPenalty` binds minsize callers only): on the z80 each copy spilled its pointers and the module came
/// out larger and slower (idioms -O2: 4555 B / 25.58M T against 1993 B / 24.49M T).
pub fn refusing_loop_copies() -> bool {
    REFUSE_LOOP_COPIES.with(Cell::get)
}

/// Runs `body` with those copies refused.
pub fn with_refused_loop_copies<T>(body: impl FnOnce() -> T) -> T {
    let before = REFUSE_LOOP_COPIES.with(|cell| cell.replace(true));
    let value = body();
    REFUSE_LOOP_COPIES.with(|cell| cell.set(before));
    value
}

/// Notes one inline the trial may refuse.
pub(crate) fn note_loop_copy() {
    LOOP_COPIES.with(|count| count.set(count.get() + 1));
}

/// The inlines [`note_loop_copy`] saw since the last call.
pub fn take_loop_copies() -> usize {
    LOOP_COPIES.with(|count| count.replace(0))
}

/// The trial threshold in force, if a trial compile is running.
pub fn active() -> Option<u32> {
    TRIAL.with(Cell::get)
}

/// Runs `body` with the trial threshold `limit` in force.
pub fn with<T>(
    limit: u32,
    body: impl FnOnce() -> T,
) -> T {
    let before = TRIAL.with(|cell| cell.replace(Some(limit)));
    let value = body();
    TRIAL.with(|cell| cell.set(before));
    value
}

/// Notes that `function` has a loop of `size` unrolled that the threshold
/// refused and a trial may take.
pub(crate) fn note(
    function: &str,
    size: u64,
) {
    ELIGIBLE.with(|list| {
        let mut list = list.borrow_mut();
        if !list.0.iter().any(|one| one == function) {
            list.0.push(function.to_owned());
        }
        list.1 += size;
    });
}

/// The functions noted since the last call, and the unrolled size of their
/// loops together.
pub fn take() -> (Vec<String>, u64) {
    ELIGIBLE.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// Puts back what [`take`] returned, for the compile whose candidates they were.
pub fn restore(noted: (Vec<String>, u64)) {
    ELIGIBLE.with(|list| *list.borrow_mut() = noted);
}

/// Whether a trial unroll is kept at -O1, where each axis is strict: it adds no bytes (`added`
/// <= 0) and costs no work (`saved` >= 0, work being what the code is expected to execute). The
/// one rule of every target's -O1 trial (DEVIATE UNROLL-TRIAL, 6502-UNROLL-O1, UNROLL-O1-TRIAL).
pub fn keeps_at_o1(
    saved: i64,
    added: i64,
) -> bool {
    added <= 0 && saved >= 0
}

/// DEVIATE ROTATION-TRIAL (beyond the pin; gcc's `-Os` leaves a counted loop whose
/// guard does not fold unrotated, `ch_base::copy_headers`; owner greedy-os-causes,
/// exit 2026-12-05): whether the trial compile that leaves such loops unrotated is
/// running.
pub fn unrotated() -> bool {
    UNROTATED.with(Cell::get)
}

/// Runs `body` with counted loops of an unfolded guard left unrotated.
pub fn with_unrotated<T>(body: impl FnOnce() -> T) -> T {
    let before = UNROTATED.with(|cell| cell.replace(true));
    let value = body();
    UNROTATED.with(|cell| cell.set(before));
    value
}

/// Notes that `function` has a counted loop of an unfolded guard that was rotated
/// and a trial may leave as it was.
pub(crate) fn note_rotated(function: &str) {
    ROTATED_LOOPS.with(|count| count.set(count.get() + 1));
    ROTATED.with(|list| {
        let mut list = list.borrow_mut();
        if !list.iter().any(|one| one == function) {
            list.push(function.to_owned());
        }
    });
}

/// The loops [`note_rotated`] saw since [`clear_rotated_loops`].
pub fn rotated_loops() -> usize {
    ROTATED_LOOPS.with(Cell::get)
}

/// Forgets the loops counted for an earlier compile.
pub fn clear_rotated_loops() {
    ROTATED_LOOPS.with(|count| count.set(0));
}

/// The functions noted by [`note_rotated`] since the last call.
pub fn take_rotated() -> Vec<String> {
    ROTATED.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// DEVIATE ROTATION-HOT (beyond the pin; owner x86-m32, exit 2026-11-24): whether the trial compile
/// that also rotates the loops of an unfolded guard and uncounted trips is running. The pin rotates them
/// at -Os; RS1 leaves them, and the finished code decides which pays (hot loops: fewer executed
/// instructions for a few bytes).
pub fn rotates_more() -> bool {
    MORE.with(Cell::get)
}

/// Runs `body` with those loops rotated as well.
pub fn with_more_rotation<T>(body: impl FnOnce() -> T) -> T {
    let before = MORE.with(|cell| cell.replace(true));
    let value = body();
    MORE.with(|cell| cell.set(before));
    value
}

/// Notes that `function` has a loop RS1 left unrotated that [`with_more_rotation`] would rotate.
pub(crate) fn note_left(function: &str) {
    LEFT.with(|list| {
        let mut list = list.borrow_mut();
        if !list.0.iter().any(|one| one == function) {
            list.0.push(function.to_owned());
        }
        list.1 += 1;
    });
}

/// The functions and loops noted by [`note_left`] since the last call.
pub fn take_left() -> (Vec<String>, usize) {
    LEFT.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// Bytes a loop that ROTATION-HOT rotates may add (a copied guard: a compare and a jump), where the
/// target (`Tti::rotation_hot_bytes_per_loop`) does not say.
pub const ROTATION_HOT_BYTES_PER_LOOP: i64 = 4;

/// Whether the ROTATION-HOT compile is kept: it executes less, and grows the code by no more than
/// `bytes` for each loop it rotated.
pub fn keeps_rotation_hot(
    saved: i64,
    added: i64,
    loops: usize,
    bytes: i64,
) -> bool {
    saved > 0 && added <= bytes * loops as i64
}

/// A function of more instructions than this is not priced twice: the second compile of a big function
/// costs more than the loops in it can save (known_loads_walks.c -Os: one 6219-instruction `main`, 2.27x the
/// compile time, put back). The functions the trial leaves unrotated hold up to 2038
/// (x_jpeg_fixed -Os, -213 B), so 1024 gave that away.
pub const ROTATION_TRIAL_MAX_INSTRUCTIONS: usize = 4096;

/// Whether the unrotated trial is kept at -Os, where bytes are the one strict axis: it
/// is smaller. A tie keeps the pass's own result.
pub fn keeps_at_size(added: i64) -> bool {
    added < 0
}

/// DEVIATE UNROLL-OS-TRIAL: whether the -Os unroll trial is kept: the object is smaller, or it grows by
/// at most `ROTATION_HOT_BYTES_PER_LOOP` for each function the trial took and the code runs less.
/// `bytes_strict` (`Tuning::unroll_trial_bytes_strict`): bytes are the one axis at -Os, so only the
/// smaller object is kept.
pub fn keeps_unroll_at_size(
    saved: i64,
    added: i64,
    functions: usize,
    bytes_strict: bool,
) -> bool {
    added < 0 || (!bytes_strict && saved > 0 && added <= ROTATION_HOT_BYTES_PER_LOOP * functions as i64)
}

/// The names of the functions whose finished bodies differ between two modules
/// of one source (one that only one of them defines counts).
pub fn differing(
    one: &llrm_mir::module::Module,
    other: &llrm_mir::module::Module,
) -> std::collections::BTreeSet<String> {
    // Printed, not compared: a constant's id means a value only within its own
    // module.
    let bodies = |module: &llrm_mir::module::Module| -> std::collections::BTreeMap<String, Option<String>> {
        module
            .globals
            .iter()
            .filter_map(|global| {
                let function = global.function().filter(|one| !one.is_declaration());
                Some((
                    global.name.as_deref()?.to_owned(),
                    function.map(|one| llrm_mir::print::body(&module.context, one)),
                ))
            })
            .collect()
    };
    let (one, other) = (bodies(one), bodies(other));
    one.keys().chain(other.keys()).filter(|name| one.get(*name) != other.get(*name)).cloned().collect()
}

/// `module` with every function outside `keep` cut to its declaration: the
/// backend then works on `keep` alone, in a module whose other functions stand
/// as they do in both versions.
pub fn only(
    module: &llrm_mir::module::Module,
    keep: &std::collections::BTreeSet<String>,
) -> llrm_mir::module::Module {
    let mut cut = module.clone();
    for global in &mut cut.globals {
        let named = global.name.as_deref().is_some_and(|name| keep.contains(name));
        if let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind
            && !named
            && !function.is_declaration()
        {
            **function = function.declaration();
        }
    }
    cut
}

/// DEVIATE OS-INLINE-TRIAL (beyond the pin; owner 6502-c, exit 2026-12-05): a call site the -Os
/// byte model refused within its own error, to be inlined in a second compile and kept if the
/// finished functions of its chain are smaller. The model is fit to the bytes the backend writes
/// (`Tti::opt_size_inline_trial_by_bytes`) and is within 25% of them for two functions in three, so
/// a copy it refuses by less than that is the backend's to decide. `caller` and `callees` are names:
/// the compile is deterministic up to the site, so the same site is found again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub caller: String,
    pub callees: Vec<String>,
}

thread_local! {
    /// The sites the second compile inlines, while it runs.
    static INLINED: RefCell<Vec<Site>> = const { RefCell::new(Vec::new()) };
    /// The sites a compile refused within the model's error.
    static REFUSED: RefCell<Vec<Site>> = const { RefCell::new(Vec::new()) };
}

/// Runs `body` with `sites` inlined whenever the pass reaches them.
pub fn with_inlined<T>(
    sites: &[Site],
    body: impl FnOnce() -> T,
) -> T {
    let before = INLINED.with(|cell| cell.replace(sites.to_vec()));
    let value = body();
    INLINED.with(|cell| *cell.borrow_mut() = before);
    value
}

/// Whether the trial compile in force inlines this site.
pub(crate) fn inlines(
    caller: &str,
    callees: &[String],
) -> bool {
    INLINED.with(|list| list.borrow().iter().any(|one| one.caller == caller && one.callees == callees))
}

/// Whether a trial compile is running.
pub(crate) fn inlining() -> bool {
    INLINED.with(|list| !list.borrow().is_empty())
}

/// Notes a site refused within the model's error.
pub(crate) fn note_refused(site: Site) {
    REFUSED.with(|list| {
        let mut list = list.borrow_mut();
        if !list.contains(&site) {
            list.push(site);
        }
    });
}

/// The sites noted since the last call.
pub fn take_refused() -> Vec<Site> {
    REFUSED.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// Which of the sites' groups the finished code keeps: sites that share a function (a caller or a
/// callee) are one group, kept whole if the bytes of its functions are fewer in the second compile
/// (`other`) than in the first (`kept`; a function a compile does not define is 0). A group whose
/// caller one compile did not define is not priced: another inline took it, and its bytes went
/// with that. Returns one flag per site.
pub fn accepted(
    sites: &[Site],
    kept: &std::collections::BTreeMap<String, i64>,
    other: &std::collections::BTreeMap<String, i64>,
) -> Vec<bool> {
    // union-find by shared name
    let mut group: Vec<usize> = (0..sites.len()).collect();
    for first in 0..sites.len() {
        for second in first + 1..sites.len() {
            if names(&sites[first]).iter().any(|name| names(&sites[second]).contains(name)) {
                let (a, b) = (root(&group, first), root(&group, second));
                group[a.max(b)] = a.min(b);
            }
        }
    }
    let roots: Vec<usize> = (0..sites.len()).map(|one| root(&group, one)).collect();
    (0..sites.len())
        .map(|site| {
            let members: Vec<&Site> = (0..sites.len()).filter(|one| roots[*one] == roots[site]).map(|one| &sites[one]).collect();
            if members.iter().any(|one| !kept.contains_key(&one.caller) || !other.contains_key(&one.caller)) {
                return false;
            }
            let mut every: Vec<&str> = members.iter().flat_map(|one| names(one)).collect();
            every.sort_unstable();
            every.dedup();
            let sum = |bytes: &std::collections::BTreeMap<String, i64>| -> i64 {
                every.iter().map(|name| bytes.get(*name).copied().unwrap_or(0)).sum()
            };
            sum(other) < sum(kept)
        })
        .collect()
}

/// The caller and the callees of `site`.
fn names(site: &Site) -> Vec<&str> {
    std::iter::once(site.caller.as_str()).chain(site.callees.iter().map(String::as_str)).collect()
}

fn root(
    group: &[usize],
    mut one: usize,
) -> usize {
    while group[one] != one {
        one = group[one];
    }
    one
}

#[cfg(test)]
mod inline_tests {
    use std::collections::BTreeMap;

    use super::*;

    fn bytes(list: &[(&str, i64)]) -> BTreeMap<String, i64> {
        list.iter().map(|(name, n)| ((*name).to_owned(), *n)).collect()
    }

    fn site(
        caller: &str,
        callees: &[&str],
    ) -> Site {
        Site { caller: caller.to_owned(), callees: callees.iter().map(|one| (*one).to_owned()).collect() }
    }

    /// memcpy_libcall -Os: the model read the copies of `same` as +52 B, the backend writes -58 B once
    /// `same` is gone. The chain's functions are priced together; the callee's bytes leave with it.
    #[test]
    fn a_chain_whose_functions_are_fewer_bytes_is_kept_with_its_callee_gone() {
        let sites = [site("main", &["same"])];
        let kept = bytes(&[("main", 300), ("same", 80)]);
        let other = bytes(&[("main", 322)]);
        assert_eq!(accepted(&sites, &kept, &other), [true]);
        assert_eq!(accepted(&sites, &kept, &bytes(&[("main", 380)])), [false], "a tie or more is put back");
    }

    /// Two callers of one callee are one group: priced apart, each would count the callee's bytes as
    /// its own saving.
    #[test]
    fn callers_of_one_callee_are_priced_as_one_group() {
        let sites = [site("a", &["h"]), site("b", &["h"])];
        let kept = bytes(&[("a", 100), ("b", 100), ("h", 50)]);
        let other = bytes(&[("a", 130), ("b", 130)]);
        assert_eq!(accepted(&sites, &kept, &other), [false, false], "a +60 B, h -50 B: larger");
        let cheaper = bytes(&[("a", 120), ("b", 120)]);
        assert_eq!(accepted(&sites, &kept, &cheaper), [true, true]);
    }

    /// A caller another inline absorbed has no bytes of its own in one compile: that is not a saving.
    #[test]
    fn a_caller_a_compile_does_not_define_is_not_priced() {
        let sites = [site("line", &["kernel"])];
        let kept = bytes(&[("line", 100), ("kernel", 90)]);
        assert_eq!(accepted(&sites, &kept, &bytes(&[])), [false]);
    }

    /// Sites of different chains are decided apart.
    #[test]
    fn unrelated_chains_are_decided_apart() {
        let sites = [site("a", &["x"]), site("b", &["y"])];
        let kept = bytes(&[("a", 100), ("x", 20), ("b", 100), ("y", 20)]);
        let other = bytes(&[("a", 110), ("b", 130)]);
        assert_eq!(accepted(&sites, &kept, &other), [true, false]);
    }
}

#[cfg(test)]
mod moved_tests {
    use std::collections::BTreeSet;

    use llrm_mir::module::Module;

    use super::*;

    fn module(first_returns: &str) -> Module {
        llrm_mir::parse::module(&format!(
            "define i32 @f() {{\n  ret i32 {first_returns}\n}}\ndefine i32 @g() {{\n  ret i32 7\n}}\ndeclare i32 @h()\n"
        ))
        .unwrap()
    }

    /// The trial's second backend run once covered the whole module, so a file
    /// with one counted loop and 400 other functions compiled its backend
    /// twice (memory_expansion.c -Os: 2.09x). It covers the functions whose
    /// bodies differ, the others standing as declarations.
    #[test]
    fn the_second_backend_run_covers_only_the_functions_that_differ() {
        let (one, other) = (module("1"), module("2"));
        let cut = differing(&one, &other);
        assert_eq!(cut, BTreeSet::from(["f".to_owned()]));
        let kept = only(&one, &cut);
        let defined = |name: &str| {
            kept.globals
                .iter()
                .find(|g| g.name.as_deref() == Some(name))
                .and_then(|g| g.function())
                .is_some_and(|f| !f.is_declaration())
        };
        assert!(defined("f") && !defined("g"));
        assert!(differing(&one, &module("1")).is_empty());
    }
}

#[cfg(test)]
mod unroll_keep_tests {
    use super::*;

    /// x86_64 -Os kept an unroll that grew long_double_union_pun by 13 B (53 -> 66; clang 53) because the code ran
    /// less: the one strict axis at -Os is bytes. A target that says so keeps only the smaller object.
    #[test]
    fn a_bytes_strict_unroll_trial_keeps_only_a_smaller_object() {
        assert!(keeps_unroll_at_size(10, 3, 1, false), "m32 keeps a few bytes for less work");
        assert!(!keeps_unroll_at_size(10, 3, 1, true), "strict: a grown object is put back");
        assert!(keeps_unroll_at_size(0, -1, 1, true), "strict: a smaller one stays");
        assert!(!keeps_unroll_at_size(10, 0, 1, true), "strict: a tie keeps the pass's own result");
    }
}

/// DEVIATE UNROLL-SPEED-TRIAL (beyond the pin; owner z80, exit 2026-11-24): a decision of the full unroll at -O2 and
/// -O3 that a driver compiles the module again without, or with, and keeps the smaller object. Neither the pin's
/// threshold nor the price (bytes added, clocks saved) can see what an unroll frees after it: one loop's copies fold
/// the loads of the next and a whole array is deleted (z80 loop_fusion -O2: priced "adds 108 B" a loop, the object
/// 831 B smaller), or its copies fold nothing and are copied again by the inliner (memmove_overlap -O3: `fill`, 32
/// trips, 589 B rolled, 1329 B unrolled).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedUnroll {
    /// A full unroll is made only where it adds no bytes, and the inlinable-callee bypass of the price is off.
    NoGrow,
    /// The full unroll's threshold is this one, where the pin's -O2 is 150 and its -O3 300.
    Wide(u32),
}

/// The threshold of [`SpeedUnroll::Wide`]: the pin's at -O3.
pub const WIDE_THRESHOLD: u32 = 300;

/// The loops of a compile at -O2 and up whose decision a variant changes, asked of each as it is decided: those
/// unrolled that `NoGrow` would not unroll (`made`), those refused that `Wide` would unroll (`refused`). A driver
/// compiles a variant only where its count is not 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnrollDecisions {
    pub made: usize,
    pub refused: usize,
}

thread_local! {
    /// The variant of UNROLL-SPEED-TRIAL running, if any.
    static SPEED_UNROLL: Cell<Option<SpeedUnroll>> = const { Cell::new(None) };
    /// The decisions since [`take_speed_unrolls`].
    static SPEED_UNROLLS: Cell<UnrollDecisions> = const { Cell::new(UnrollDecisions { made: 0, refused: 0 }) };
}

/// The UNROLL-SPEED-TRIAL variant in force, if a trial compile is running.
pub fn speed_unroll() -> Option<SpeedUnroll> {
    SPEED_UNROLL.with(Cell::get)
}

/// Runs `body` under `variant` (`None`: the pin's decisions).
pub fn with_speed_unroll<T>(
    variant: Option<SpeedUnroll>,
    body: impl FnOnce() -> T,
) -> T {
    let before = SPEED_UNROLL.with(|cell| cell.replace(variant));
    let value = body();
    SPEED_UNROLL.with(|cell| cell.set(before));
    value
}

/// Notes a loop unrolled (`made`) or, with known trips, refused.
pub(crate) fn note_speed_unroll(made: bool) {
    SPEED_UNROLLS.with(|cell| {
        let mut seen = cell.get();
        if made {
            seen.made += 1;
        } else {
            seen.refused += 1;
        }
        cell.set(seen);
    });
}

/// The decisions [`note_speed_unroll`] saw since the last call.
pub fn take_speed_unrolls() -> UnrollDecisions {
    SPEED_UNROLLS.with(Cell::take)
}
