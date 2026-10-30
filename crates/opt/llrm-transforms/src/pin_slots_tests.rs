//! The pin's `-print-pipeline-passes` against llrm's slot registry: every slot
//! of the pin's pipeline at -O1, -O2, -O3 and -Os (tests/fixtures/pin-pipeline,
//! made by tools/pairs/pin_pipeline.py) is matched, in order, to the slots
//! `llvm_order` builds, or is listed as a gap. `LLRM_PIN_SLOT_TABLE=FILE`
//! writes the table.

use crate::parity::{self, Entry, Phase};
use crate::pipeline::{self, Applied, Options};

const FIXTURES: [(&str, &str); 4] = [
    ("-O1", include_str!("../../../../tests/fixtures/pin-pipeline/O1.txt")),
    ("-O2", include_str!("../../../../tests/fixtures/pin-pipeline/O2.txt")),
    ("-O3", include_str!("../../../../tests/fixtures/pin-pipeline/O3.txt")),
    ("-Os", include_str!("../../../../tests/fixtures/pin-pipeline/Os.txt")),
];

/// One pass of the pin's printed pipeline.
#[derive(Clone, Debug)]
struct PinSlot {
    /// The phase of llrm's registry the nest it stands in is: the first
    /// top-level function group is Early, the second Cleanup, the function
    /// groups of the CGSCC Simplify, those after it Late, the rest Module.
    phase: Phase,
    /// Its place in the nest, as the pin prints it
    /// (`cgscc/devirt<4>/function`).
    path: String,
    /// The pass without its parameters.
    name: String,
    text: String,
}

/// `pass<params>` as (`pass`, whole).
fn split_params(text: &str) -> (&str, &str) {
    (text.split('<').next().unwrap_or(text), text)
}

/// The passes of one printed pipeline, in order, with their nest.
fn parse(fixture: &str) -> Vec<PinSlot> {
    let line = fixture.lines().rev().find(|one| !one.starts_with(';')).expect("a pipeline line");
    let mut tokens = Vec::new();
    let (mut angle, mut buffer) = (0_i32, String::new());
    for ch in line.chars() {
        match ch {
            '<' => angle += 1,
            '>' => angle -= 1,
            _ => {}
        }
        if angle == 0 && matches!(ch, ',' | '(' | ')') {
            tokens.push((std::mem::take(&mut buffer), ch));
        } else {
            buffer.push(ch);
        }
    }
    tokens.push((buffer, ' '));
    let mut path: Vec<String> = Vec::new();
    let mut out = Vec::new();
    let mut top_functions = 0;
    let mut seen_cgscc = false;
    // The top-level function groups, by order, once their analyses-only
    // members are left out.
    let mut group_phase: Vec<Phase> = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let (name, ch) = tokens[index].clone();
        index += 1;
        if ch == '(' {
            if path.is_empty() && split_params(&name).0 == "function" {
                // Phase of a top-level function group: decided when its first
                // real pass shows; a group with only analyses is Module.
                top_functions += 1;
                group_phase.push(Phase::Module);
            }
            if path.is_empty() && name == "cgscc" {
                seen_cgscc = true;
            }
            path.push(name);
            continue;
        }
        if !name.is_empty() {
            let (bare, whole) = split_params(&name);
            let noise = matches!(bare, "require" | "invalidate" | "verify" | "print");
            if !noise {
                let in_cgscc_function = path.first().is_some_and(|one| one == "cgscc")
                    && path.iter().any(|one| split_params(one).0 == "function");
                let phase = if in_cgscc_function {
                    Phase::Simplify
                } else if path.first().is_some_and(|one| split_params(one).0 == "function") {
                    let at = top_functions - 1;
                    if group_phase[at] == Phase::Module {
                        let real_groups = group_phase.iter().filter(|one| **one != Phase::Module).count();
                        group_phase[at] = if seen_cgscc {
                            Phase::Late
                        } else if real_groups == 0 {
                            Phase::Early
                        } else {
                            Phase::Cleanup
                        };
                    }
                    group_phase[at]
                } else {
                    Phase::Module
                };
                out.push(PinSlot { phase, path: path.join("/"), name: bare.to_owned(), text: whole.to_owned() });
            }
        }
        if ch == ')' {
            path.pop();
        }
    }
    out
}

/// The pass names an entry's `llvm` field names (`simplifycfg, jump-threading`
/// is two; `licm<allowspeculation>` is `licm`).
fn llvm_names(entry: &Entry) -> Vec<String> {
    entry
        .llvm
        .split(',')
        .filter_map(|part| part.trim().split_whitespace().next())
        .map(|word| word.split('<').next().unwrap_or(word).to_owned())
        .collect()
}

/// The line a slot stands at (`combine@1242` is 1242): the slots of one line
/// are one slot of the pin's.
fn group_of(slot: &str) -> String {
    slot.split_once('@').map_or_else(|| slot.to_owned(), |split| split.1.to_owned())
}

fn applied(level: &str) -> Applied {
    let options = match level {
        "-O1" => Options::basic(),
        "-O3" => Options::speed(),
        "-Os" => Options::size(),
        _ => Options::standard(),
    };
    Applied { options, ..Applied::default() }
}

/// The registry entries the pipeline of `level` builds in `phase`, grouped by
/// the line they stand at, in the order built.
fn built_groups(
    level: &str,
    phase: Phase,
) -> Vec<(String, Vec<Entry>)> {
    let mut groups: Vec<(String, Vec<Entry>)> = Vec::new();
    for slot in pipeline::llvm_order(&applied(level), phase).iter().map(|one| one.name()) {
        let Some(entry) = parity::REGISTRY.iter().find(|one| one.slot == slot && one.phase == phase) else { continue };
        let group = group_of(slot);
        match groups.iter_mut().find(|(name, _)| *name == group) {
            Some((_, entries)) => entries.push(*entry),
            None => groups.push((group, vec![*entry])),
        }
    }
    groups
}

/// What a slot of the pin's pipeline that ours does not match stands as.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Why {
    /// Not applicable at this target or to our frontends; the reason.
    NotApplicable,
    /// A deliberate difference, with where it is stated.
    Deviate,
    /// Not translated or not in the registry: the row has an owner and a date.
    Missing,
    /// Ours stands earlier or later than the pin's; the owner fixes the order.
    Order,
    /// Run by a stage of ours that is outside the registry (the executed order
    /// of a compile, `--dump` of d_faces -O1/-O2/-Os: globalopt, ports, stamp,
    /// the early function list, ipo-early, the cleanup list, walk, the
    /// late list, interprocedural, writeonly, globaldce,
    /// available-externally, calleepop, ...; pipeline.rs `recorded`).
    Executed,
}

struct Disposition {
    /// The pin pass's phase and name; for ours-only groups the group's text.
    phase: Phase,
    name: &'static str,
    why: Why,
    /// The reason, the stage, or the effect measured by
    /// tools/pairs/pin_ablate.py (the pin's -O2 pipeline over the 12 bench
    /// C programs, one instance removed).
    note: &'static str,
    /// For Missing and Order: who and by when; `unassigned` until the
    /// coordinator does.
    owner: &'static str,
}

const NA: Why = Why::NotApplicable;
const UNASSIGNED: &str = "unassigned (coordinator, 10-26)";

const fn row(
    phase: Phase,
    name: &'static str,
    why: Why,
    note: &'static str,
    owner: &'static str,
) -> Disposition {
    Disposition { phase, name, why, note, owner }
}

use Phase::{Early, Late, Module, Simplify};

const ROWS: &[Disposition] = &[
    row(
        Early,
        "ee-instrument",
        NA,
        "-pg/-finstrument-functions: no frontend asks for it Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(Early, "callsite-splitting", Why::Missing, "-O3 only; not measured", "funcattrs-port, 11-09"),
    row(
        Simplify,
        "speculative-execution",
        NA,
        "runs only on a target with divergent branches; x86 has none Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Simplify,
        "libcalls-shrinkwrap",
        Why::Missing,
        "errno-setting libm calls; pin ablation (2026-10-27): 0 IRs of 65 QCport (i486 -O2 and m16 -Os), 0 of 58 QB (m16 -Os; the pin parses 58 of 101), 0 of 49 C examples/bench (m16 -Os and i486 -O2); DEFERRED, revisit after parity-green",
        "w4",
    ),
    row(
        Simplify,
        "extra-simple-loop-unswitch-passes",
        Why::Missing,
        "ablation 0 of 12 IRs",
        "w-3 (w-5 if silent at 09:00), 11-09",
    ),
    row(
        Simplify,
        "vector-combine",
        NA,
        "vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out.",
        "",
    ),
    row(
        Simplify,
        "move-auto-init",
        NA,
        "needs -ftrivial-auto-var-init metadata; no frontend emits it Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Simplify,
        "coro-elide",
        NA,
        "coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Late,
        "drop-unnecessary-assumes",
        Why::Missing,
        "pin ablation (2026-10-27): 0 IRs of 65 QCport (i486 -O2 and m16 -Os), 0 of 58 QB (m16 -Os; the pin parses 58 of 101), 0 of 49 C examples/bench (m16 -Os and i486 -O2); DEFERRED, revisit after parity-green",
        "w4",
    ),
    row(
        Late,
        "alignment-from-assumptions",
        Why::Missing,
        "pin ablation (2026-10-27): 0 IRs of 65 QCport (i486 -O2 and m16 -Os), 0 of 58 QB (m16 -Os; the pin parses 58 of 101), 0 of 49 C examples/bench (m16 -Os and i486 -O2); DEFERRED, revisit after parity-green",
        "w4",
    ),
    row(
        Late,
        "loop-rotate",
        Why::Missing,
        "the late loop nest's rotate; DEFERRED: pin ablation (2026-10-27) QCport 65 IRs 3 IRs / -3 instr (i486 -O2), 4 / -2 (m16 -Os); QB 58 IRs 1 / +1 (the pin parses 58 of 101); C examples/bench 49 IRs 3 / -14 (i486 -O2), 3 / -12 (m16 -Os), the pass adds instructions there; revisit after parity-green",
        "w4",
    ),
    row(
        Late,
        "loop-deletion",
        Why::Missing,
        "pin ablation (2026-10-27, entry kept alive): late loop-deletion changes 0 of 65 QCport, 0 of 49 C examples/bench, 0 of 58 QB (m16 -Os; the pin parses 58 of 101); DEFERRED, revisit after parity-green",
        "w4",
    ),
    row(
        Late,
        "loop-distribute",
        Why::Missing,
        "pin ablation (2026-10-27): 0 IRs of 65 QCport (i486 -O2 and m16 -Os), 0 of 58 QB (m16 -Os; the pin parses 58 of 101), 0 of 49 C examples/bench (m16 -Os and i486 -O2); DEFERRED, revisit after parity-green",
        "w4",
    ),
    row(
        Late,
        "inject-tli-mappings",
        NA,
        "maps scalar libcalls to vector ones; vectors deferred Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out.",
        "",
    ),
    row(
        Late,
        "loop-vectorize",
        NA,
        "vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out.",
        "",
    ),
    row(
        Late,
        "transform-warning",
        NA,
        "diagnostic for failed pragmas; no output Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Late,
        "loop-sink",
        Why::Missing,
        "pin ablation (2026-10-27): 0 IRs of 65 QCport (i486 -O2 and m16 -Os), 0 of 58 QB (m16 -Os; the pin parses 58 of 101), 0 of 49 C examples/bench (m16 -Os and i486 -O2); DEFERRED, revisit after parity-green",
        "w4",
    ),
    row(
        Late,
        "mergeicmps",
        Why::Missing,
        "DEFERRED, 0/77 IRs (12 bench + 65 QCport); revisit after parity-green",
        "w-4, 11-09",
    ),
    row(
        Late,
        "expand-memcmp",
        Why::Missing,
        "DEFERRED, 0/77 IRs (12 bench + 65 QCport); revisit after parity-green",
        "w-4, 11-09",
    ),
    row(
        Late,
        "vector-combine",
        NA,
        "vectors deferred (user ruling 10-20), -sse target Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang); with SSE on, removing the vectorizers changes 47 of 76 (O2), so the no-op is this target's (i486, -sse) and the deferral (user ruling 10-20) keeps them out.",
        "",
    ),
    row(
        Late,
        "annotation-remarks",
        NA,
        "remarks off Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Late,
        "chr",
        NA,
        "control-height-reduction needs profile data Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "memprof-remove-attributes",
        NA,
        "memprof: no profile Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "annotation2metadata",
        NA,
        "no source annotations Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "forceattrs",
        NA,
        "command-line forced attributes: none given Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "inferattrs",
        Why::Executed,
        "inferattrs (inferfunctionattrs.rs, translated), the first module pass; the stamp (interprocedural::Stamp) states what a body does, llrm's own form",
        "",
    ),
    row(
        Module,
        "coro-early",
        NA,
        "coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "coro-split",
        NA,
        "coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "coro-annotation-elide",
        NA,
        "coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "coro-cleanup",
        NA,
        "coroutines: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "openmp-opt",
        NA,
        "OpenMP: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "openmp-opt-cgscc",
        NA,
        "OpenMP: no frontend Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "ipsccp",
        Why::Deviate,
        "DEVIATE (ipsccp.rs): ipo-early (walk::ModuleIpo: run_ipsccp); no FunctionSpecializer, ipa-cp clones stand for it (ipsccp.rs DEVIATE 1)",
        "",
    ),
    row(Module, "called-value-propagation", Why::Executed, "ipo-early (after ipsccp; calledvaluepropagation)", ""),
    row(Module, "globalopt", Why::Executed, "globalopt (first), writeonly, calleepop: a subset of GlobalOpt", ""),
    row(Module, "always-inline", Why::Executed, "walk (the CGSCC walk's inliner)", ""),
    row(Module, "inline", Why::Executed, "walk, and interprocedural (inlines again after the clones)", ""),
    row(
        Module,
        "function-attrs",
        Why::Executed,
        "interprocedural (states what each body does); not compared pass by pass",
        "",
    ),
    row(
        Module,
        "rpo-function-attrs",
        Why::Executed,
        "rpo-function-attrs (norecurse only; nofpclass is not a MIR attribute)",
        "",
    ),
    row(
        Module,
        "deadargelim",
        Why::Deviate,
        "DEVIATE W1 (walk.rs): ipo-early (walk::ModuleIpo: ipa-deadargs) runs before the Simplify list, the pin's after the CGSCC block. Measured 2026-10-27 (early run removed, the later one kept): QCport -18 B at -Os (2 of 65), 0 at -O2; bench/hanoi +51..+64 B (m16/m32, O2/Os) toward the pin's 62 instructions from our 30. Kept: ours is the smaller",
        "",
    ),
    row(
        Module,
        "argpromotion",
        Why::Missing,
        "-O3 only; argpromotion.rs exists, where it runs is not in the registry: unverified",
        "w-6, 11-09",
    ),
    row(
        Module,
        "recompute-globalsaa",
        NA,
        "an analysis recompute; ours holds and freezes GlobalsAA (pipeline.rs) Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "globaldce",
        Why::Executed,
        "globaldce (once). The pin's second globaldce, after the Late list, has no stage of ours: 0 B on 65 QCport modules (-Os and -O2) and 0 of 284 bench/example objects (m16/m32, O2/Os) with it added before constmerge (2026-10-27), so N/A until a row moves (owner w-6)",
        "",
    ),
    row(Module, "elim-avail-extern", Why::Executed, "available-externally", ""),
    row(Module, "constmerge", Why::Executed, "constmerge.rs, the last module pass (llvm_order, optimizing)", ""),
    row(
        Module,
        "alloc-token",
        NA,
        "clang option, off Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "cg-profile",
        NA,
        "profile data Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "rel-lookup-table-converter",
        NA,
        "relative lookup tables are a PIC feature; no PIC targets Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Module,
        "lower-comment-string",
        NA,
        "#pragma comment strings Evidence (tools/pairs/pin_noop.py): the printed default<O2>, <Os> and <O3> pipelines with this pass removed leave the final IR identical on all 76 IRs (12 bench C programs, 64 QCport modules through the pin's clang).",
        "",
    ),
    row(
        Simplify,
        "memcpyopt",
        Why::Order,
        "-O1 only: ours places it where the O2 list does; the pin's O1 list orders it differently (the one slot off the pin's order, by position)",
        UNASSIGNED,
    ),
];

/// The built slot groups no pin slot is matched to, with why.
const OURS_ONLY: &[(&str, &str)] = &[
    ("1474: loop-unroll-and-jam@1474", "-O3: the pin's printed list has no unroll-and-jam in this fixture; unverified"),
    (
        "764: loopsimplify@764 + lcssa@764",
        "llrm's loop-simplify/lcssa stand-ins that LLVM's loop pass adaptor makes implicit (parity.rs RUN_COUNTS_OPEN)",
    ),
    ("772: loopsimplify@772 + lcssa@772", "as 764"),
    (
        "669: cvp@669",
        "-O1 only: the pin's O1 list is buildO1FunctionSimplificationPipeline, a shorter one; ours runs the O2 order minus ABSENT_AT_O1 (unverified slot by slot)",
    ),
    (
        "787: inferspace@787",
        "-O1 only: the pin's O1 list is buildO1FunctionSimplificationPipeline, a shorter one; ours runs the O2 order minus ABSENT_AT_O1 (unverified slot by slot)",
    ),
    (
        "810: cvp@810",
        "-O1 only: the pin's O1 list is buildO1FunctionSimplificationPipeline, a shorter one; ours runs the O2 order minus ABSENT_AT_O1 (unverified slot by slot)",
    ),
    (
        "820: dse@820",
        "-O1 only: the pin's O1 list is buildO1FunctionSimplificationPipeline, a shorter one; ours runs the O2 order minus ABSENT_AT_O1 (unverified slot by slot)",
    ),
    ("823: loopsimplify@823 + lcssa@823 + licm@823", "-O1 only, as 669 (unverified slot by slot)"),
];

#[derive(Debug)]
struct Row {
    level: &'static str,
    pin: PinSlot,
    /// The group of ours it stands against, or why none does.
    ours: String,
    state: &'static str,
}

impl Row {
    /// The row that accounts for an unmatched slot of the pin, if any.
    fn account(&self) -> Option<&'static Disposition> {
        ROWS.iter().find(|one| one.phase == self.pin.phase && one.name == self.pin.name)
    }
}

/// The indexes of a longest run of `matched` (its `Some` values, in order)
/// that increases: the slots that stand in the pin's order.
fn longest_increasing(matched: &[Option<usize>]) -> Vec<usize> {
    // tails[k]: the index into `matched` of the smallest value ending a run of
    // k + 1.
    let mut tails: Vec<usize> = Vec::new();
    let mut parent: Vec<Option<usize>> = vec![None; matched.len()];
    for (index, value) in matched.iter().enumerate() {
        let Some(value) = value else { continue };
        let at = tails.partition_point(|&tail| matched[tail].is_some_and(|one| one < *value));
        parent[index] = at.checked_sub(1).map(|before| tails[before]);
        if at == tails.len() {
            tails.push(index);
        } else {
            tails[at] = index;
        }
    }
    let mut run = Vec::new();
    let mut at = tails.last().copied();
    while let Some(index) = at {
        run.push(index);
        at = parent[index];
    }
    run
}

fn rows() -> Vec<Row> {
    let mut rows = Vec::new();
    for (level, fixture) in FIXTURES {
        let pin = parse(fixture);
        for phase in [Phase::Early, Phase::Cleanup, Phase::Simplify, Phase::Late, Phase::Module] {
            let groups = built_groups(level, phase);
            let mut taken = vec![false; groups.len()];
            // Each pin slot takes the first group of ours that names its pass
            // and is not taken, whatever the order; the order is
            // judged after, by position: the slots off the longest
            // run that is in the pin's order are the ones that
            // moved (the others stand where they should).
            let pins: Vec<&PinSlot> = pin.iter().filter(|one| one.phase == phase).collect();
            let mut matched: Vec<Option<usize>> = Vec::new();
            for slot in &pins {
                let found = groups.iter().enumerate().position(|(at, (_, entries))| {
                    !taken[at] && entries.iter().any(|entry| llvm_names(entry).iter().any(|name| *name == slot.name))
                });
                if let Some(at) = found {
                    taken[at] = true;
                }
                matched.push(found);
            }
            let in_order = longest_increasing(&matched);
            for (index, slot) in pins.iter().enumerate() {
                match matched[index] {
                    Some(at) => {
                        let kinds: Vec<String> =
                            groups[at].1.iter().map(|one| format!("{}:{:?}", one.slot, one.kind)).collect();
                        let state = if in_order.contains(&index) { "match" } else { "inverted" };
                        rows.push(Row { level, pin: (*slot).clone(), ours: kinds.join(" + "), state });
                    }
                    None => rows.push(Row { level, pin: (*slot).clone(), ours: String::new(), state: "none" }),
                }
            }
            for (at, (group, entries)) in groups.iter().enumerate() {
                if !taken[at] {
                    let names: Vec<String> = entries.iter().map(|one| one.slot.to_owned()).collect();
                    let pin_name = PinSlot { phase, path: String::new(), name: String::new(), text: String::new() };
                    rows.push(Row {
                        level,
                        pin: pin_name,
                        ours: format!("{group}: {}", names.join(" + ")),
                        state: "ours-only",
                    });
                }
            }
        }
    }
    rows
}

/// The one state of a row: MATCH, or what accounts for it (N/A, EXECUTED,
/// DEVIATE, MISSING, ORDER); `UNACCOUNTED` is a gap with no row, which the test
/// refuses.
fn published_state(row: &Row) -> &'static str {
    match row.state {
        "match" => "MATCH",
        "ours-only" => "DEVIATE",
        _ => row.account().map_or("UNACCOUNTED", |one| why_word(one.why)),
    }
}

/// The note and the owner of a row's state: its own row's, or an ours-only
/// group's.
fn row_note(row: &Row) -> (&'static str, &'static str) {
    match row.state {
        "match" => ("", ""),
        "ours-only" => {
            OURS_ONLY.iter().find(|(group, _)| row.ours.starts_with(group)).map_or(("", ""), |(_, note)| (note, ""))
        }
        _ => row.account().map_or(("", ""), |one| (one.note, one.owner)),
    }
}

fn table(rows: &[Row]) -> String {
    let mut text = String::from(
        "| level | phase | pin slot | nest | state | ours | note | owner |\n|---|---|---|---|---|---|---|---|\n",
    );
    for row in rows {
        let (note, owner) = row_note(row);
        text.push_str(&format!(
            "| {} | {:?} | {} | {} | {} | {} | {} | {} |\n",
            row.level,
            row.pin.phase,
            row.pin.text,
            row.pin.path,
            published_state(row),
            row.ours,
            note,
            owner
        ));
    }
    text
}

/// How many rows of `level` are in each state.
fn counts(
    rows: &[Row],
    level: &str,
) -> std::collections::BTreeMap<&'static str, usize> {
    let mut found = std::collections::BTreeMap::new();
    for row in rows.iter().filter(|row| row.level == level) {
        *found.entry(published_state(row)).or_insert(0) += 1;
    }
    found
}

fn why_word(why: Why) -> &'static str {
    match why {
        Why::NotApplicable => "N/A",
        Why::Deviate => "DEVIATE",
        Why::Missing => "MISSING",
        Why::Order => "ORDER",
        Why::Executed => "EXECUTED",
    }
}

#[test]
fn the_pin_slot_table_is_written_on_request() {
    let rows = rows();
    assert!(rows.iter().any(|one| one.state == "match"), "nothing matched: the fixtures or the registry moved");
    if let Ok(path) = std::env::var("LLRM_PIN_SLOT_TABLE") {
        std::fs::write(&path, table(&rows)).expect("table written");
        let mut summary = String::new();
        for (level, _) in FIXTURES {
            summary.push_str(&format!("{level} {:?}\n", counts(&rows, level)));
        }
        std::fs::write(format!("{path}.counts"), summary).expect("counts written");
    }
}

/// A slot of the pin's pipeline that ours does not match in position, a slot
/// of ours that no pin slot is matched to, and a row that accounts for neither
/// are errors: a gap is a row with an owner and a date, a difference is a
/// DEVIATE, and a row nothing needs is removed.
#[test]
fn every_pin_slot_is_matched_or_accounted_for() {
    let rows = rows();
    let mut errors = Vec::new();
    for row in &rows {
        match row.state {
            "match" => {}
            "none" | "inverted" => match row.account() {
                None => errors
                    .push(format!("{} {:?} {}: {} with no row", row.level, row.pin.phase, row.pin.text, row.state)),
                Some(one) if row.state == "inverted" && one.why != Why::Order => {
                    errors.push(format!("{} {}: inverted, but its row is not an ORDER row", row.level, row.pin.text))
                }
                Some(one) if matches!(one.why, Why::Missing | Why::Order) && one.owner.is_empty() => {
                    errors.push(format!("{}: a gap row without an owner", one.name))
                }
                Some(_) => {}
            },
            "ours-only" => {
                if !OURS_ONLY.iter().any(|(group, _)| row.ours.starts_with(group)) {
                    errors.push(format!("{}: ours-only with no row ({})", row.level, row.ours));
                }
            }
            other => errors.push(format!("unknown state {other}")),
        }
    }
    for one in ROWS {
        let used =
            rows.iter().any(|row| row.state != "match" && row.pin.phase == one.phase && row.pin.name == one.name);
        if !used {
            errors.push(format!(
                "{:?} {}: a row nothing needs (the slot matches now, or the pin lost it)",
                one.phase, one.name
            ));
        }
    }
    for (group, _) in OURS_ONLY {
        if !rows.iter().any(|row| row.state == "ours-only" && row.ours.starts_with(group)) {
            errors.push(format!("{group}: an ours-only row nothing needs"));
        }
    }
    errors.sort();
    errors.dedup();
    assert!(errors.is_empty(), "{} problems:\n{}", errors.len(), errors.join("\n"));
}

/// `simplifycfg@2376` is LateFPM of buildLTODefaultPipeline, not of
/// `default<O2>`: it ran a ninth SimplifyCFG on every function (+1.4 M Ir on
/// d_faces -O2, object unchanged).
#[test]
fn test_o2_has_the_pins_simplifycfg_slots_and_no_more() {
    let pin = parse(FIXTURES[1].1).iter().filter(|slot| slot.name == "simplifycfg").count();
    let ours: usize = [Phase::Early, Phase::Cleanup, Phase::Simplify, Phase::Late]
        .into_iter()
        .map(|phase| {
            built_groups("-O2", phase)
                .iter()
                .filter(|(_, entries)| {
                    entries.iter().any(|entry| llvm_names(entry).iter().any(|name| name == "simplifycfg"))
                })
                .count()
        })
        .sum();
    assert_eq!(ours, pin);
}

/// Every pin slot of every level is in exactly one of six states, and the
/// states add up to the slots: `none` is not a state a reader sees.
#[test]
fn every_row_has_exactly_one_state() {
    let rows = rows();
    let states = ["MATCH", "N/A", "EXECUTED", "DEVIATE", "MISSING", "ORDER"];
    for row in &rows {
        assert!(states.contains(&published_state(row)), "{} {}: {}", row.level, row.pin.text, published_state(row));
    }
    for (level, _) in FIXTURES {
        let total: usize = counts(&rows, level).values().sum();
        assert_eq!(total, rows.iter().filter(|row| row.level == level).count(), "{level}");
    }
}
