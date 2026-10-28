//! The x87 phase after register allocation: copy propagation, the stackifier
//! and the `fisttp` mode switching, with the helpers they share.
//!
//! # How it works
//! `FloatAlloc::transform` runs `fpcopyprop::propagated`, then
//! [`crate::backend::fpstack::converted`] (the translation of LLVM's
//! `X86FloatingPoint`, which turns the flat registers `RegAlloc` chose into
//! stack form), then `_truncating`.
//!
//! `_truncating` makes `fisttp` as a 387 has it: `fistp` between `fldcw` of a
//! control word with rounding set toward zero and a `fldcw` of the saved one.
//! The two words are saved once at entry.
//!
//! Where the switches go is gcc's mode switching, in one analysis and one
//! placement over the graph (`_needs`, `_modes`;
//! [`optimize_mode_switching`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/mode-switching.cc#L495),
//! the i386 entity
//! [`I387_TRUNC`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/config/i386/i386.cc#L14607)).
//! Each instruction needs a mode: truncating for a conversion, the caller's for
//! whatever rounds, a call or a return, none for exact work. A block assumes on
//! entry the mode its first need names, or its successors' (the heavier by loop
//! depth), and a switch goes on each edge whose ends disagree, so a loop of
//! conversions switches on its entry edge and its exits, not twice a trip
//! (`fldcw` is 4 clocks on a 486, 7 on a P5). Unlike gcc's lazy code motion
//! this does not solve for the least number of insertions: it follows demand
//! and takes a critical edge's source mode.
//!
//! # Example
//! A `fisttp` in a loop body gets one `fldcw` pair on the loop's entry edge
//! and exit, not one per trip.
//!
//! # Runs
//! After `RegAlloc` in the machine phases, once per function, on bodies with
//! floating values only (the truncation fix-up runs where there is a `fisttp`).
//! Not gated by an -O level.
//!
//! # Needs
//! [`crate::backend::fpstack`], `fpcopyprop`, [`crate::backend::floatregions`]
//! for the instruction boundaries.
//!
//! # See also
//! `docs/optimizations/floating-pop-reuse.md`; LLVM `X86FloatingPoint.cpp`.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use crate::analysis::loops;
use crate::backend::classes::RegisterClasses;
#[cfg(test)]
use crate::backend::constpool::Pool;
#[cfg(test)]
use crate::backend::cpu::ProfileOrName;
use crate::backend::floatregions::Raised;
use crate::backend::floatregions::Unlowered;
use crate::backend::frame::Frame;
use crate::backend::liverows::Live;
use crate::backend::select;
use crate::backend::spillplacement;
use crate::model::ir::{Held, Imm, Loc, Mem, Operation, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;
use crate::support::hash::{HashMap, HashSet};

pub(super) fn unlowered(message: &str) -> Raised {
    Raised::Unlowered(Unlowered(message.to_owned()))
}

pub(super) fn st(index: usize) -> Loc {
    Loc::st(index as u32)
}

pub(super) fn semantics(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Semantics {
    Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
}

/// `lir.Insn(at, (at, at), what, (), ())`.
pub(super) fn inserted(
    at: i64,
    what: Semantics,
) -> Insn {
    Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new())
}

pub(super) fn name_is(
    what: &Semantics,
    name: &str,
) -> bool {
    what.name.as_deref() == Some(name)
}

/// Python's `not what.name`.
pub(super) fn unnamed(what: &Semantics) -> bool {
    what.name.as_deref().is_none_or(str::is_empty)
}

pub(super) fn width_of(arg: &Loc) -> Option<u32> {
    match arg {
        Loc::Held(held) => Some(held.width),
        Loc::Imm(imm) => Some(imm.width),
        _ => None,
    }
}

/// A home load's cell: every home is a load from memory.
pub(super) fn cell_of(load: &Insn) -> &Mem {
    match &load.what.as_ref().expect("a home load has semantics").sources[0] {
        Loc::Mem(cell) => cell,
        _ => unreachable!("a home load reads memory"),
    }
}

const _ARITHMETIC: [&str; 4] = ["fadd", "fsub", "fmul", "fdiv"];
// `left op right` with the operands' places swapped: `st(i) := st(0) - st(i)`
// is `fsubr st(i),st(0)`.
const _REVERSED: [(&str, &str); 4] = [("fadd", "fadd"), ("fmul", "fmul"), ("fsub", "fsubr"), ("fdiv", "fdivr")];

pub(super) fn _reversed(name: &str) -> &'static str {
    _REVERSED.iter().find(|(key, _)| *key == name).map(|(_, value)| *value).expect("_REVERSED[name]")
}

pub(super) fn _floating(arg: &Loc) -> bool {
    matches!(arg, Loc::Held(held) if held.width == 10)
}

pub(super) fn _loads_memory(what: Option<&Semantics>) -> bool {
    what.is_some_and(|what| {
        what.op == Operation::FloatLoad
            && (name_is(what, "fld") || name_is(what, "fild"))
            && what.sources.len() == 1
            && matches!(what.sources[0], Loc::Mem(_))
            && what.dests.len() == 1
            && matches!(what.dests[0], Loc::Held(_))
    })
}

/// Arithmetic on two floating values, as `result := left name right` with name
/// in _ARITHMETIC.
pub(super) fn _two_values(what: Option<&Semantics>) -> Option<(String, Held, Held)> {
    let what = what?;
    if !matches!(what.op, Operation::FloatArith | Operation::FloatArithPop)
        || what.sources.len() != 2
        || !what.sources.iter().all(_floating)
    {
        return None;
    }
    let full = what.name.as_deref()?;
    let name = if what.op == Operation::FloatArithPop { &full[..full.len() - 1] } else { full };
    let (Loc::Held(left), Loc::Held(right)) = (&what.sources[0], &what.sources[1]) else {
        unreachable!("both sources are floating")
    };
    if name == "fsubr" || name == "fdivr" {
        return Some((name[..name.len() - 1].to_owned(), *right, *left));
    }
    _ARITHMETIC.contains(&name).then(|| (name.to_owned(), *left, *right))
}

/// The instruction computing `name` with one operand read from `load`'s cell,
/// if x87 has one.
pub(super) fn _memory_name(
    name: &str,
    cell_is_left: bool,
    load: &Insn,
) -> Option<String> {
    let mut name = if cell_is_left { _reversed(name) } else { name }.to_owned();
    if name_is(load.what.as_ref().expect("a home load has semantics"), "fild") {
        name = format!("fi{}", &name[1..]);
    }
    select::float_memory(&name, cell_of(load), select::At::bits16(0)).is_some().then_some(name)
}

/// The compare of a value against `load`'s cell: `fcomp`, or `ficomp` where the
/// cell holds an integer `fild` reads.
pub(super) fn _compare_name(load: &Insn) -> &'static str {
    if name_is(load.what.as_ref().expect("a home load has semantics"), "fild") { "ficomp" } else { "fcomp" }
}

/// `(result, source)` of a copy between floating values.
pub(super) fn _float_copy(one: &Insn) -> Option<(u32, u32)> {
    let what = one.what.as_ref()?;
    match (what.op, what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::FloatLoad | Operation::Move, [Loc::Held(result)], [Loc::Held(source)])
            if result.width == 10 && source.width == 10 =>
        {
            Some((result.value, source.value))
        }
        _ => None,
    }
}

/// The blocks in reverse postorder from the entry, then the unreachable ones,
/// and how many are reached.
fn _reverse_postorder(body: &LirBody) -> (Vec<i64>, usize) {
    let at_of: HashMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let (mut seen, mut order) = (HashSet::default(), Vec::new());
    let mut pending: Vec<(i64, usize)> = vec![(body.entry, 0)];
    seen.insert(body.entry);
    while let Some((at, next)) = pending.pop() {
        let successors = at_of.get(&at).map_or(&[][..], |block| block.succ.as_slice());
        if let Some(successor) = successors.get(next) {
            pending.push((at, next + 1));
            if at_of.contains_key(successor) && seen.insert(*successor) {
                pending.push((*successor, 0));
            }
        } else if at_of.contains_key(&at) {
            order.push(at);
        }
    }
    order.reverse();
    let reached = order.len();
    order.extend(body.blocks.iter().map(|block| block.at).filter(|at| !seen.contains(at)));
    (order, reached)
}

/// `body` with a block of its own on each critical forward edge into a
/// block that reads fewer floating values than its bundle holds: the
/// values it does not read are popped there, on the one edge that brings
/// them, and its other entries no longer push fillers to match.
pub(super) fn _split_for_the_stack(
    body: &LirBody,
    floating: &HashSet<u32>,
    (live_in, live_out): (&Live, &Live),
    bundles: &spillplacement::Bundles,
) -> Option<LirBody> {
    let floats = |set: &llrm_mir::dense::IdSet<u32>| -> BTreeSet<u32> {
        set.iter().filter(|value| floating.contains(value)).collect()
    };
    let mut held: IndexMap<usize, BTreeSet<u32>> = IndexMap::default();
    let mut entries: IndexMap<i64, usize> = IndexMap::default();
    for block in &body.blocks {
        let (entry, exit) = bundles.of[&block.at];
        held.entry(entry).or_default().extend(floats(&live_in[&block.at]));
        held.entry(exit).or_default().extend(floats(&live_out[&block.at]));
        for successor in &block.succ {
            *entries.entry(*successor).or_default() += 1;
        }
    }
    let (order, _) = _reverse_postorder(body);
    let rank: IndexMap<i64, usize> = order.iter().enumerate().map(|(rank, at)| (*at, rank)).collect();
    let mut edges: IndexMap<(i64, i64), Vec<Arc<Insn>>> = IndexMap::default();
    for block in body.blocks.iter().filter(|block| block.succ.len() > 1) {
        for &into in &block.succ {
            let forward = rank.get(&block.at).zip(rank.get(&into)).is_some_and(|(from, to)| from < to);
            let unread = held[&bundles.of[&into].0] != floats(&live_in[&into]);
            if forward && entries[&into] > 1 && unread {
                edges.insert((block.at, into), Vec::new());
            }
        }
    }
    (!edges.is_empty()).then(|| crate::backend::phielim::placed_on_edges(body, &edges))
}

/// Where the instructions leaving the block begin; the stack shuffles there.
pub(super) fn _terminators(block: &LirBlock) -> usize {
    let mut cut = block.insns.len();
    while cut > 0
        && block.insns[cut - 1]
            .what
            .as_ref()
            .is_some_and(|what| matches!(
                what.op,
                Operation::Jump | Operation::Branch | Operation::Return
            ))
    {
        cut -= 1;
    }
    cut
}

/// Stack form of a body whose x87 values are in registers, as the pipeline
/// hands it over.
#[cfg(test)]
pub fn allocated<'a>(
    body: &LirBody,
    frame: Option<&mut Frame>,
    _pool: Option<&mut Pool>,
    _basic_semantics: bool,
    _cpu: impl Into<ProfileOrName<'a>>,
) -> Result<LirBody, Raised> {
    _truncating(&crate::backend::fpstack::converted(body, &RegisterClasses::of(&llrm_x86_m16::M16))?, frame)
}

/// The control word an x87 instruction needs: the one place the fact lives.
/// `fisttp` as a 387 has it is `fistp` with the word set to round toward zero
/// (`I387_TRUNC` in gcc's i386 `ix86_mode_needed`); anything that rounds
/// needs the caller's, and so does a call or a return; exact work (integer
/// instructions, x87 loads, exchanges, compares, sign changes, branches)
/// needs none.
fn _needs(one: &Insn) -> Option<Mode> {
    if one
        .what
        .as_ref()
        .is_some_and(|what| what.op == Operation::FloatStore && (name_is(what, "fisttp") || name_is(what, "fistt")))
    {
        return Some(Mode::Chop);
    }
    let branches = one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch));
    (!branches && !_exact(one)).then_some(Mode::Caller)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Caller,
    Chop,
}

/// What each block assumes of the control word on entry, and whether each
/// restores the caller's before it ends: gcc's `optimize_mode_switching`
/// ([mode-switching.cc](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/mode-switching.cc)),
/// whose lazy code motion puts a mode where it is wanted ahead of its uses and
/// out of loops. Here a block wants the mode its first instruction needs, or
/// the one its successors want, most weighted by loop depth; an edge whose two
/// ends disagree gets the switch, at the end of its source if that has one way
/// on, else at the start of its target if that has one way in. A critical edge
/// takes its source's mode for its target instead, and where the modes cannot
/// all agree every block restores the caller's before it ends.
struct Modes {
    entering: HashMap<i64, Mode>,
    restore: bool,
}

fn _modes(body: &LirBody) -> Modes {
    let graph = &body.blocks;
    let predecessors = loops::predecessors(graph);
    let index = graph.iter().enumerate().map(|(position, block)| (block.at, position)).collect::<HashMap<_, _>>();
    let mut weight = HashMap::<i64, i64>::default();
    for one in loops::loops(graph, Some(body.entry)) {
        for at in &one.body {
            *weight.entry(*at).or_insert(1) *= 8;
        }
    }
    let (first, last): (HashMap<_, _>, HashMap<_, _>) = graph
        .iter()
        .map(|block| {
            let needs = block.insns.iter().filter_map(|one| _needs(one)).collect::<Vec<_>>();
            ((block.at, needs.first().copied()), (block.at, needs.last().copied()))
        })
        .unzip();
    let mut want = first.clone();
    for _ in 0..=graph.len() {
        let mut changed = false;
        for block in graph.iter().filter(|block| first[&block.at].is_none()) {
            let (mut caller, mut chop) = (0, 0);
            for to in &block.succ {
                match want[to] {
                    Some(Mode::Caller) => caller += weight.get(to).copied().unwrap_or(1),
                    Some(Mode::Chop) => chop += weight.get(to).copied().unwrap_or(1),
                    None => {}
                }
            }
            let voted = match caller.cmp(&chop) {
                std::cmp::Ordering::Less => Some(Mode::Chop),
                std::cmp::Ordering::Greater => Some(Mode::Caller),
                std::cmp::Ordering::Equal => None,
            };
            changed |= std::mem::replace(want.get_mut(&block.at).expect("every block"), voted) != voted;
        }
        if !changed {
            break;
        }
    }
    let mut entering = graph
        .iter()
        .map(|block| {
            (block.at, if block.at == body.entry { Mode::Caller } else { want[&block.at].unwrap_or(Mode::Caller) })
        })
        .collect::<HashMap<_, _>>();
    let leaving = |entering: &HashMap<i64, Mode>, at: i64| last[&at].unwrap_or(entering[&at]);
    let placeable = |entering: &HashMap<i64, Mode>, from: i64, to: i64| {
        leaving(entering, from) == entering[&to]
            || graph[index[&from]].succ.len() == 1
            || (predecessors[&to].len() == 1 && to != body.entry)
    };
    for _ in 0..=2 * graph.len() {
        let mut changed = false;
        for block in graph {
            for &to in &block.succ {
                if !placeable(&entering, block.at, to) && to != body.entry {
                    entering.insert(to, leaving(&entering, block.at));
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // The switches an assignment costs, each weighted by its block's depth:
    // none if an edge cannot take its switch.
    let cost = |entering: &HashMap<i64, Mode>, restore: bool| -> Option<i64> {
        let mut total = 0;
        for block in graph {
            let at = weight.get(&block.at).copied().unwrap_or(1);
            let mut current = entering[&block.at];
            let mut switches = 0;
            for one in &block.insns {
                if let Some(needed) = _needs(one).filter(|needed| *needed != current) {
                    switches += 1;
                    current = needed;
                }
            }
            if (restore || block.succ.is_empty()) && current != Mode::Caller {
                switches += 1;
            }
            total += at * switches;
            for &to in block.succ.iter().filter(|_| !restore) {
                if current != entering[&to] {
                    if !placeable(entering, block.at, to) {
                        return None;
                    }
                    total += at;
                }
            }
        }
        Some(total)
    };
    let restoring = graph.iter().map(|block| (block.at, Mode::Caller)).collect::<HashMap<_, _>>();
    // Following demand can cost more than restoring at the end of each block: a
    // loop that converts and rounds alike in a trip gains nothing from entering
    // in one mode and pays the switch on its entry and exit.
    match (cost(&entering, false), cost(&restoring, true)) {
        (Some(following), Some(restored)) if following <= restored => Modes { entering, restore: false },
        (Some(_), None) => Modes { entering, restore: false },
        _ => Modes { entering: restoring, restore: true },
    }
}

/// `fisttp` as a 387 has it: `fistp` with the control word set to round
/// toward zero and put back. After allocation, so the control-word barriers
/// split no region.
///
/// The caller's control word and its truncating form are saved once, at
/// entry: a callee leaves the control word as it found it, and nothing else
/// in the body writes it. The switches are placed by `_modes`.
pub(super) fn _truncating(
    body: &LirBody,
    frame: Option<&mut Frame>,
) -> Result<LirBody, Raised> {
    if !body.iter_insns().any(|one| _needs(one) == Some(Mode::Chop)) {
        return Ok(body.clone());
    }
    let Some(frame) = frame else {
        return Err(unlowered("rounding toward zero requires an owned frame"));
    };
    // Python keys these slots by the 1-tuples ("control",) and ("chop",).
    let (saved, chop) = (frame.cell(("control", 0), 2)?, frame.cell(("chop", 0), 2)?);
    // The registers are allocated by now: no value is left to hold the word, so
    // it goes through memory alone.
    let insn = |what: Semantics, at: i64| -> Arc<Insn> {
        let held = |args: &[Loc]| -> Vec<Held> {
            args.iter()
                .filter_map(|arg| match arg {
                    Loc::Held(held) => Some(*held),
                    _ => None,
                })
                .collect()
        };
        let defines = held(&what.dests).iter().map(|arg| arg.value).collect();
        let uses = held(&what.sources).iter().map(|arg| arg.value).collect();
        let widths = held(&what.dests).iter().chain(&held(&what.sources)).map(|arg| (arg.value, arg.width)).collect();
        let mut made = Insn::new(at, Some((at, at)), Some(what), defines, uses);
        made.widths = widths;
        Arc::new(made)
    };
    let switch = |mode: Mode, at: i64| {
        let cell = if mode == Mode::Chop { &chop } else { &saved };
        insn(semantics(Operation::Barrier, "fldcw", Vec::new(), vec![Loc::Mem(cell.clone())]), at)
    };

    let modes = _modes(body);
    let mut blocks = Vec::new();
    // Each block's insns with the mode it leaves in.
    let mut walked = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        let arrivals =
            if block.at == body.entry { block.insns.iter().take_while(|one| one.arrival()).count() } else { 0 };
        if block.at == body.entry {
            let at = block.insns.first().map_or(block.at, |first| first.at);
            insns.extend(block.insns[..arrivals].iter().cloned());
            insns.extend([
                insn(semantics(Operation::Barrier, "fnstcw", vec![Loc::Mem(saved.clone())], Vec::new()), at),
                insn(semantics(Operation::Barrier, "fnstcw", vec![Loc::Mem(chop.clone())], Vec::new()), at),
                insn(
                    semantics(
                        Operation::Binary,
                        "or",
                        vec![Loc::Mem(chop.clone())],
                        vec![Loc::Mem(chop.clone()), Loc::Imm(Imm { value: 0x0C00, width: 2, address: None })],
                    ),
                    at,
                ),
            ]);
        }
        let mut current = modes.entering[&block.at];
        for one in &block.insns[arrivals..] {
            if let Some(needed) = _needs(one) {
                if needed != current {
                    insns.push(switch(needed, one.at));
                    current = needed;
                }
            }
            if one.what.as_ref().is_some_and(|what| {
                what.op == Operation::FloatStore && (name_is(what, "fisttp") || name_is(what, "fistt"))
            }) {
                let mut made = (**one).clone();
                let kept = name_is(one.what.as_ref().expect("checked above"), "fistt");
                made.what = Some(Semantics {
                    name: Some(if kept { "fist" } else { "fistp" }.into()),
                    ..one.what.clone().expect("checked above")
                });
                insns.push(Arc::new(made));
            } else {
                insns.push(Arc::clone(one));
            }
        }
        walked.push((insns, current));
    }
    // The switch on each edge whose ends disagree.
    let mut ends = HashMap::<i64, Vec<Mode>>::default();
    let mut starts = HashMap::<i64, Mode>::default();
    for (block, (_, current)) in body.blocks.iter().zip(&walked) {
        if modes.restore {
            continue;
        }
        for &to in &block.succ {
            if *current == modes.entering[&to] {
                continue;
            }
            if block.succ.len() == 1 {
                ends.entry(block.at).or_default().push(modes.entering[&to]);
            } else {
                starts.insert(to, modes.entering[&to]);
            }
        }
    }
    for (block, (mut insns, current)) in body.blocks.iter().zip(walked) {
        let at = block.insns.last().map_or(block.at, |last| last.at);
        let mut tail = ends.remove(&block.at).unwrap_or_default();
        // A block with no way on leaves the function: the caller's word again.
        if (modes.restore || block.succ.is_empty()) && current != Mode::Caller {
            tail.push(Mode::Caller);
        }
        // Before the branches that end the block: they must stay last.
        let branches = insns
            .iter()
            .rev()
            .take_while(|one| {
                one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch))
            })
            .count();
        for mode in tail {
            insns.insert(insns.len() - branches, switch(mode, at));
        }
        if let Some(mode) = starts.get(&block.at) {
            let first = block.insns.first().map_or(block.at, |first| first.at);
            insns.insert(0, switch(*mode, first));
        }
        let mut made = block.clone();
        made.insns = insns.into();
        blocks.push(made);
    }
    let mut out = body.clone();
    out.blocks = blocks;
    Ok(out)
}

/// Whether `one` neither rounds nor leaves the block: integer work, and
/// the x87 loads, exchanges, compares and sign changes, which are exact.
fn _exact(one: &Insn) -> bool {
    let Some(what) = &one.what else { return false };
    let name = what.name.as_deref().unwrap_or_default();
    match what.op {
        Operation::Jump
        | Operation::Branch
        | Operation::Call
        | Operation::Return
        | Operation::Escape
        | Operation::Barrier
        | Operation::Restore
        | Operation::Data => false,
        Operation::FloatLoad => !matches!(name, "fldpi" | "fldl2e" | "fldl2t" | "fldlg2" | "fldln2"),
        Operation::FloatStore => false,
        Operation::FloatArith | Operation::FloatArithPop | Operation::FloatUnary => {
            matches!(
                name,
                "fxch" | "fcom" | "fcomp" | "fcompp" | "fucom" | "fucomp" | "fucompp" | "ftst" | "fabs" | "fchs"
            )
        }
        _ => true,
    }
}

/// The post-allocation x87 phase: stack form, then `fisttp` mode switching.
pub struct FloatAlloc {
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub classes: Rc<RegisterClasses>,
}

impl LIRTransform for FloatAlloc {
    fn class_name(&self) -> &'static str {
        "FloatAlloc"
    }

    fn name(&self) -> &str {
        "floatalloc"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        let mut frame = self.frame.as_ref().map(|frame| frame.borrow_mut());
        let propagated = crate::backend::fpcopyprop::propagated(&body);
        crate::backend::fpstack::converted(propagated.as_ref().unwrap_or(&body), &self.classes)
            .and_then(|converted| _truncating(&converted, frame.as_deref_mut()))
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[path = "floatalloc_tests.rs"]
pub(crate) mod tests;
