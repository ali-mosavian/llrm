//! A loop's spill slots in the registers the loop leaves free.
//!
//! Allocation spills a value across its whole range; inside one loop,
//! registers may still be free: untouched there, or used only to reload a
//! slot for the next instruction. Each slot the loop reaches moves into one,
//! loaded on entry and stored on exit where it was written and is read after.
//!
//! BP is one more when the loop reaches no other frame cell: it enters with
//! `push bp; mov bp,[bp+d]` and leaves with `pop bp`. Such a loop may not
//! call, trap or touch x87 state, since the runtime walks the BP chain when
//! it raises an error. A spill slot's address is never taken, so no pointer
//! reaches any slot this moves.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::Register;

use crate::analysis::intervals::{self as ranges, _graph};
use crate::analysis::loops::{self, Loop};
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::frame::Frame;
use crate::backend::liveness;
use crate::backend::peephole::{_lanes, Lanes};
use crate::backend::target;
use crate::support::hash::IndexMap;
use crate::model::ir::{self, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::{LIRTransform, OperationCosts};
use crate::objectfile::module::Addr;

pub struct LoopSlots {
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub cpu: Profile,
}

impl LoopSlots {
    pub fn new<'a>(frame: Option<Rc<RefCell<Frame>>>, cpu: impl Into<ProfileOrName<'a>>) -> Result<Self, String> {
        Ok(Self { frame, cpu: targets::profile(cpu)?.clone() })
    }
}

impl LIRTransform for LoopSlots {
    fn class_name(&self) -> &'static str {
        "LoopSlots"
    }

    fn name(&self) -> &str {
        "loopslots"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        let Some(frame) = &self.frame else {
            return Ok(body);
        };
        let spills = frame.borrow().capacities.iter().filter(|(_, width)| **width == 2).map(|(at, _)| *at).collect();
        Ok(promoted(&body, &spills, &self.cpu.operations))
    }
}

const WORD: u32 = 2;
const ROOTS: [Register; 6] = [Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI];

/// The slot `mem` names, when it is a whole frame cell.
fn slot(mem: &Mem) -> Option<i64> {
    let addr = mem.addr?;
    (addr.space == Space::Frame
        && addr.base == Register::None
        && mem.through == Register::BP
        && mem.base.is_none()
        && mem.index.is_none()
        && mem.index_through == Register::None)
        .then_some(addr.disp + mem.offset)
}

fn is_bp(register: Register) -> bool {
    register != Register::None && ir::root(register) == ir::root(Register::BP)
}

/// How one instruction reaches the frame.
#[derive(Default)]
struct Touch {
    reads: BTreeSet<i64>,
    writes: BTreeSet<i64>,
    /// A frame access that is no word slot, or a use of BP itself.
    other: bool,
    /// Something that needs BP to be the frame pointer while it runs.
    traps: bool,
}

fn touch(one: &Insn) -> Touch {
    let mut out = Touch::default();
    let Some(what) = one.what.as_ref() else {
        out.other = true;
        out.traps = true;
        return out;
    };
    out.traps = matches!(
        what.op,
        Operation::Call
            | Operation::Return
            | Operation::Leave
            | Operation::Divide
            | Operation::Escape
            | Operation::Barrier
            | Operation::Data
            | Operation::Restore
            | Operation::FloatLoad
            | Operation::FloatStore
            | Operation::FloatArith
            | Operation::FloatArithPop
            | Operation::FloatUnary
    ) || what.name.as_deref().is_some_and(|name| ["int", "into", "iret", "enter", "bound"].contains(&name))
        || one.clobbers.iter().chain(&one.clobbers_high).any(|register| is_bp(*register));
    let moves = what.op == Operation::Move;
    for (place, written) in what.dests.iter().map(|one| (one, true)).chain(what.sources.iter().map(|one| (one, false))) {
        match place {
            Loc::Reg(reg) if is_bp(reg.register) => out.other = true,
            Loc::Mem(mem) => {
                if is_bp(mem.index_through) || mem.addr.is_some_and(|addr| addr.space == Space::Stack) {
                    out.other = true;
                } else if is_bp(mem.through) {
                    // An address of the frame is a pointer into it.
                    match slot(mem).filter(|_| mem.width == WORD && what.op != Operation::Address) {
                        Some(at) if written => {
                            out.writes.insert(at);
                            if !moves {
                                out.reads.insert(at);
                            }
                        }
                        Some(at) => {
                            out.reads.insert(at);
                        }
                        None => out.other = true,
                    }
                }
            }
            Loc::Address(address) if is_bp(address.through) || is_bp(address.index) => out.other = true,
            _ => {}
        }
    }
    out
}

/// Blocks where slot `at` may be read before it is written.
fn live_in(body: &LirBody, at: i64) -> BTreeSet<i64> {
    let first = body
        .blocks
        .iter()
        .map(|block| {
            let seen = block.insns.iter().find_map(|one| {
                let touched = touch(one);
                if touched.reads.contains(&at) {
                    Some(true)
                } else {
                    touched.writes.contains(&at).then_some(false)
                }
            });
            (block.at, seen)
        })
        .collect::<BTreeMap<_, _>>();
    let mut live = first.iter().filter(|(_, seen)| **seen == Some(true)).map(|(block, _)| *block).collect::<BTreeSet<_>>();
    loop {
        let before = live.len();
        for block in &body.blocks {
            if first[&block.at].is_none() && block.succ.iter().any(|to| live.contains(to)) {
                live.insert(block.at);
            }
        }
        if live.len() == before {
            return live;
        }
    }
}

/// The word register `root` names, as a plain operand.
fn word(root: Register) -> Loc {
    Loc::Reg(Reg { register: target::named(root, 2), width: WORD })
}

fn plain_word(place: &Loc, root: Register) -> bool {
    matches!(place, Loc::Reg(reg) if reg.width == WORD && ir::root(reg.register) == ir::root(root))
}

/// A reload of a slot into `root`: `mov r,[slot]`.
fn reload_of(one: &Insn, root: Register) -> Option<i64> {
    let what = one.what.as_ref()?;
    match (what.op, what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Move, [dest], [Loc::Mem(mem)]) if plain_word(dest, root) && mem.width == WORD => slot(mem),
        _ => None,
    }
}

/// Whether `one` reads `root` only as a plain word source and writes none of it.
fn reads_plainly(one: &Insn, root: Register, effect: &liveness::Effect) -> bool {
    let Some(what) = one.what.as_ref() else {
        return false;
    };
    let lanes = _lanes(root);
    let low = _lanes(target::named(root, 2));
    effect.writes.is_disjoint(&lanes)
        && effect.reads.iter().filter(|lane| lanes.contains(lane)).all(|lane| low.contains(lane))
        && !what.dests.iter().any(|place| matches!(place, Loc::Reg(reg) if ir::root(reg.register) == ir::root(root)))
        && what.sources.iter().any(|place| plain_word(place, root))
        && what.sources.iter().chain(&what.dests).all(|place| match place {
            Loc::Reg(reg) if ir::root(reg.register) == ir::root(root) => plain_word(place, root),
            Loc::Mem(mem) => [mem.through, mem.index_through].iter().all(|one| ir::root(*one) != ir::root(root)),
            _ => true,
        })
}

/// Registers the loop leaves free: `None` for untouched, or `Some` of the
/// reloads and their readers when it only reloads a slot for the next use.
fn free(
    body: &LirBody,
    one: &Loop,
    index: &BTreeMap<i64, usize>,
    live_into: &IndexMap<i64, Lanes>,
    exits: &BTreeSet<i64>,
) -> Vec<(Register, Option<Vec<(usize, usize, i64)>>)> {
    let mut out = Vec::new();
    'roots: for root in ROOTS {
        let lanes = _lanes(root);
        if [one.header].iter().chain(exits).any(|at| !live_into[at].is_disjoint(&lanes)) {
            continue;
        }
        let mut folded = Vec::new();
        for at in &one.body {
            let block = &body.blocks[index[at]];
            let mut reloaded = None;
            for (position, insn) in block.insns.iter().enumerate() {
                let Some(effect) = liveness::effect(insn) else {
                    continue 'roots;
                };
                if effect.reads.is_disjoint(&lanes) && effect.writes.is_disjoint(&lanes) {
                    continue;
                }
                if let Some(from) = reload_of(insn, root) {
                    reloaded = Some(from);
                    folded.push((index[at], position, from));
                } else if let Some(from) = reloaded.filter(|_| reads_plainly(insn, root, &effect)) {
                    folded.push((index[at], position, from));
                } else {
                    continue 'roots;
                }
            }
        }
        out.push((root, (!folded.is_empty()).then_some(folded)));
    }
    // Untouched registers first: they cost nothing to take.
    out.sort_by_key(|(_, folded)| folded.is_some());
    out
}

fn made(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Arc<Insn> {
    let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new()))
}

fn cell(at: i64) -> Mem {
    Mem { through: Register::BP, disp_width: 2, ..Mem::new(Some(Addr::new(Space::Frame, at)), WORD) }
}

/// `one` with each slot access made its register.
fn rewritten(one: &Arc<Insn>, homes: &BTreeMap<i64, Loc>) -> Arc<Insn> {
    let Some(what) = one.what.as_ref() else {
        return Arc::clone(one);
    };
    let swap = |place: &Loc| match place {
        Loc::Mem(mem) => slot(mem).and_then(|at| homes.get(&at)).cloned().unwrap_or_else(|| place.clone()),
        other => other.clone(),
    };
    if !what.dests.iter().chain(&what.sources).any(|place| matches!(place, Loc::Mem(mem) if slot(mem).is_some_and(|at| homes.contains_key(&at)))) {
        return Arc::clone(one);
    }
    let what = Semantics {
        dests: what.dests.iter().map(swap).collect(),
        sources: what.sources.iter().map(swap).collect(),
        ..what.clone()
    };
    Arc::new(Insn { what: Some(what), spill_reload: false, spill_store: false, ..(**one).clone() })
}

/// What holding a slot in a register saves each trip: a reload whose
/// register is freed goes, one kept becomes a register move.
fn saving(one: &Insn, at: i64, folded: bool, costs: &OperationCosts) -> i64 {
    let touched = touch(one);
    let (reads, writes) = (touched.reads.contains(&at), touched.writes.contains(&at));
    if !reads && !writes {
        return 0;
    }
    let plain = !folded && one.what.as_ref().is_some_and(|what| what.op == Operation::Move);
    i64::from(reads) * costs.load + i64::from(writes) * costs.store - i64::from(plain) * costs.r#move
}

/// `body` with each loop's spill slots in the registers it leaves free.
pub fn promoted(body: &LirBody, spills: &BTreeSet<i64>, costs: &OperationCosts) -> LirBody {
    let graph = _graph(&body.blocks);
    let predecessors = loops::predecessors(&graph);
    let mut found = loops::loops(&graph, Some(body.entry));
    // Innermost first: it runs most often.
    found.sort_by_key(|one| one.body.len());
    let index = body.blocks.iter().enumerate().map(|(position, block)| (block.at, position)).collect::<BTreeMap<_, _>>();
    let (live_into, _, _) = liveness::live_into(body);
    let mut blocks = body.blocks.clone();
    let mut taken = BTreeSet::<i64>::new();
    for one in &found {
        if one.body.iter().any(|at| taken.contains(at)) {
            continue;
        }
        let insns = || one.body.iter().flat_map(|at| body.blocks[index[at]].insns.iter());
        let (mut slots, mut other, mut traps) = (BTreeSet::new(), false, false);
        for insn in insns() {
            let touched = touch(insn);
            other |= touched.other;
            traps |= touched.traps;
            slots.extend(touched.reads.into_iter().chain(touched.writes));
        }
        if slots.is_empty() || !slots.iter().all(|at| spills.contains(at)) {
            continue;
        }
        let entries = predecessors[&one.header].iter().copied().filter(|at| !one.body.contains(at)).collect::<Vec<_>>();
        let exits = one
            .body
            .iter()
            .flat_map(|at| body.blocks[index[at]].succ.iter().copied())
            .filter(|to| !one.body.contains(to))
            .collect::<BTreeSet<_>>();
        if entries.is_empty()
            || !entries.iter().all(|at| body.blocks[index[at]].succ == [one.header])
            || !exits.iter().all(|to| predecessors[to].iter().all(|from| one.body.contains(from)))
            || one.body.iter().any(|at| body.blocks[index[at]].succ.is_empty())
        {
            continue;
        }
        let registers = free(body, one, &index, &live_into, &exits);
        let reloads = registers
            .iter()
            .flat_map(|(_, folded)| folded.iter().flatten().map(|(block, position, _)| (*block, *position)))
            .collect::<BTreeSet<_>>();
        // Most saved first.
        let mut ranked = slots
            .iter()
            .map(|at| {
                let mut saved = 0;
                for block in &one.body {
                    for (position, insn) in body.blocks[index[block]].insns.iter().enumerate() {
                        saved += saving(insn, *at, reloads.contains(&(index[block], position)), costs);
                    }
                }
                (saved, *at)
            })
            .collect::<Vec<_>>();
        ranked.sort_by_key(|(saved, at)| (std::cmp::Reverse(*saved), *at));
        let (entered, left) = (entries.len() as i64, exits.len() as i64);
        let written = |at: i64| insns().any(|insn| touch(insn).writes.contains(&at));
        let stored = |at: i64| written(at) && { let live = live_in(body, at); exits.iter().any(|to| live.contains(to)) };
        // BP takes the last slot when the rest fill every free register.
        let bp = !other && !traps && ranked.len() == registers.len() + 1 && !stored(ranked.last().expect("a slot").1);
        let mut homes = BTreeMap::<i64, Loc>::new();
        let mut hosts = BTreeMap::<i64, usize>::new();
        for (host, ((saved, at), (root, _))) in ranked.iter().zip(&registers).enumerate() {
            let cost = entered * costs.load + if stored(*at) { left * costs.store } else { 0 };
            if saved * ranges::PER_LEVEL > cost {
                homes.insert(*at, word(*root));
                hosts.insert(*at, host);
            }
        }
        let bp_slot = ranked.last().map(|(_, at)| *at).filter(|_| bp && homes.len() == registers.len());
        if let Some(at) = bp_slot {
            let saved = ranked.last().expect("a slot").0;
            if saved * ranges::PER_LEVEL > entered * (costs.store + costs.load) + left * costs.load {
                homes.insert(at, Loc::Reg(Reg { register: Register::BP, width: WORD }));
            }
        }
        // A register freed by folding its reloads is free only when each
        // slot it reloads has a home; BP only when the frame is left alone.
        loop {
            let before = homes.len();
            let unfolded = hosts
                .iter()
                .filter(|(_, host)| registers[**host].1.iter().flatten().any(|(_, _, from)| !homes.contains_key(from)))
                .map(|(at, _)| *at)
                .collect::<Vec<_>>();
            for at in unfolded {
                homes.remove(&at);
                hosts.remove(&at);
            }
            if homes.len() < slots.len() {
                homes.retain(|_, home| !matches!(home, Loc::Reg(reg) if reg.register == Register::BP));
            }
            if homes.len() == before {
                break;
            }
        }
        let folds = hosts
            .values()
            .flat_map(|host| {
                let root = registers[*host].0;
                registers[*host].1.iter().flatten().map(move |(block, position, from)| (*block, *position, *from, root))
            })
            .collect::<Vec<_>>();
        if homes.is_empty() {
            continue;
        }
        let with_bp = homes.values().any(|home| matches!(home, Loc::Reg(reg) if reg.register == Register::BP));
        for at in &one.body {
            let block = &blocks[index[at]];
            let mut insns = Vec::new();
            // What the dropped reloads defined, which their readers no longer read.
            let mut gone = BTreeSet::new();
            for (position, insn) in block.insns.iter().enumerate() {
                let fold = folds.iter().find(|(inside, place, ..)| *inside == index[at] && *place == position);
                match fold {
                    Some((_, _, from, root)) => {
                        let home = homes[from].clone();
                        if reload_of(insn, *root).is_some() {
                            gone.extend(insn.defines.iter().copied());
                            continue;
                        }
                        let what = insn.what.as_ref().expect("a plain reader");
                        let swap = |place: &Loc| if plain_word(place, *root) { home.clone() } else { place.clone() };
                        let what = Semantics { sources: what.sources.iter().map(swap).collect(), ..what.clone() };
                        let reader = Insn {
                            what: Some(what),
                            uses: insn.uses.iter().copied().filter(|value| !gone.contains(value)).collect(),
                            requires: insn.requires.iter().copied().filter(|(held, _)| !gone.contains(&held.value)).collect(),
                            ..(**insn).clone()
                        };
                        insns.push(rewritten(&Arc::new(reader), &homes));
                    }
                    None => insns.push(rewritten(insn, &homes)),
                }
            }
            blocks[index[at]] = block.with_insns(insns);
        }
        for at in &entries {
            let block = &blocks[index[at]];
            let mut insns = block.insns.clone();
            let jumps = insns.last().and_then(|last| last.what.as_ref()).is_some_and(|what| what.op == Operation::Jump);
            let mut place = insns.len() - usize::from(jumps);
            let near = insns.get(place.saturating_sub(1)).map_or(*at, |insn| insn.at);
            for (slot, home) in &homes {
                let load = made(near, Operation::Move, "mov", vec![home.clone()], vec![Loc::Mem(cell(*slot))]);
                if matches!(home, Loc::Reg(reg) if reg.register == Register::BP) {
                    let bp = home.clone();
                    insns.insert(insns.len() - usize::from(jumps), made(near, Operation::Push, "push", vec![], vec![bp]));
                    insns.insert(insns.len() - usize::from(jumps), load);
                } else {
                    insns.insert(place, load);
                    place += 1;
                }
            }
            blocks[index[at]] = block.with_insns(insns);
        }
        for to in &exits {
            let block = &blocks[index[to]];
            let mut insns = block.insns.clone();
            let near = insns.first().map_or(*to, |insn| insn.at);
            let mut place = 0;
            if with_bp {
                insns.insert(0, made(near, Operation::Pop, "pop", vec![Loc::Reg(Reg { register: Register::BP, width: WORD })], vec![]));
                place = 1;
            }
            for (slot, home) in &homes {
                if stored(*slot) {
                    insns.insert(place, made(near, Operation::Move, "mov", vec![Loc::Mem(cell(*slot))], vec![home.clone()]));
                    place += 1;
                }
            }
            blocks[index[to]] = block.with_insns(insns);
        }
        taken.extend(one.body.iter().copied());
    }
    body.with_blocks(blocks)
}
