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
//!
//! A register live through the loop but untouched in it is one more, parked
//! on the stack: pushed on entry and popped on exit.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::Register;

use crate::analysis::frequency::Frequency;
use crate::analysis::loops::{self, Loop};
use crate::backend::classes::RegisterClasses;
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::frame::Frame;
use crate::backend::peephole::{self, _lanes, Lanes};
use crate::backend::target;
use crate::backend::{liveness, select, spiller};
use crate::model::ir::{self, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::{LIRTransform, OperationCosts};
use crate::objectfile::module::Addr;
use crate::support::hash::IndexMap;

pub struct LoopSlots {
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub cpu: Profile,
    /// The bytes of a slot and of the register that stands for it: the target's
    /// stack slot.
    pub word: u32,
    pub classes: Rc<RegisterClasses>,
}

impl LoopSlots {
    pub fn new<'a>(
        frame: Option<Rc<RefCell<Frame>>>,
        cpu: impl Into<ProfileOrName<'a>>,
        word: u32,
        classes: &Rc<RegisterClasses>,
    ) -> Result<Self, String> {
        Ok(Self { frame, cpu: targets::profile(cpu)?.clone(), word, classes: Rc::clone(classes) })
    }
}

impl LIRTransform for LoopSlots {
    fn class_name(&self) -> &'static str {
        "LoopSlots"
    }

    fn name(&self) -> &str {
        "loopslots"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        let Some(frame) = &self.frame else {
            return Ok(body);
        };
        let m = self.word;
        let spills =
            frame.borrow().capacities.iter().filter(|(_, width)| **width == i64::from(m)).map(|(at, _)| *at).collect();
        let park = self.cpu.cost("push_r")? + self.cpu.cost("pop_r")?;
        llrm_support::debug!("traffic", "{} {:?}", body.name, crate::backend::allocate::traffic_by_cause(&body));
        Ok(promoted(
            m,
            &self.classes.available,
            &hoisted(m, &self.classes.available, &body, &spills),
            &spills,
            &self.cpu.operations,
            park,
        ))
    }
}

/// The slot `mem` names, when it is a whole frame cell.
fn slot(mem: &Mem) -> Option<i64> {
    let addr = mem.addr?;
    (addr.space == Space::Frame
        && addr.base == Register::None
        && mem.through == ir::FRAME
        && mem.base.is_none()
        && mem.index.is_none()
        && mem.index_through == Register::None)
        .then_some(addr.disp + mem.offset)
}

fn is_bp(register: Register) -> bool {
    register != Register::None && crate::backend::registerinfo::is_frame(register)
}

/// The word slot `mem` names, when it is exactly one.
fn word_slot(
    m: u32,
    mem: &Mem,
) -> Option<i64> {
    slot(mem).filter(|_| mem.width == m)
}

/// How one instruction reaches the frame.
#[derive(Default)]
struct Touch {
    /// The slot width the accesses were sorted by.
    word: u32,
    reads: BTreeSet<i64>,
    writes: BTreeSet<i64>,
    /// Frame bytes it reaches other than as exactly one word, which no
    /// register can stand for: a dword store, a far pointer load.
    wide: Vec<Reach>,
    /// A frame access that is no word slot, or a use of BP itself.
    other: bool,
    /// Something that needs BP to be the frame pointer while it runs.
    traps: bool,
}

/// Frame bytes `[from, to)` one access reads or writes.
struct Reach {
    from: i64,
    to: i64,
    read: bool,
    written: bool,
}

impl Touch {
    /// Whether an access that is not exactly word slot `at` reads it, or writes
    /// it.
    fn reaches(
        &self,
        at: i64,
        writing: bool,
    ) -> bool {
        self.wide.iter().any(|one| {
            one.from < at + i64::from(self.word) && at < one.to && if writing { one.written } else { one.read }
        })
    }

    /// Whether this reads word slot `at` in whole or in part.
    fn reads_slot(
        &self,
        at: i64,
    ) -> bool {
        self.reads.contains(&at) || self.reaches(at, false)
    }

    /// Whether this writes word slot `at` in whole or in part.
    fn writes_slot(
        &self,
        at: i64,
    ) -> bool {
        self.writes.contains(&at) || self.reaches(at, true)
    }
}

#[cfg(test)]
thread_local! {
    static TOUCHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FITTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RANKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn touch(
    m: u32,
    one: &Insn,
) -> Touch {
    #[cfg(test)]
    TOUCHED.with(|count| count.set(count.get() + 1));
    let mut out = Touch { word: m, ..Touch::default() };
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
    for (place, written) in what.dests.iter().map(|one| (one, true)).chain(what.sources.iter().map(|one| (one, false)))
    {
        match place {
            Loc::Reg(reg) if is_bp(reg.register) => out.other = true,
            Loc::Mem(mem) => {
                if is_bp(mem.index_through) || mem.addr.is_some_and(|addr| addr.space == Space::Stack) {
                    out.other = true;
                } else if is_bp(mem.through) {
                    // An address of the frame is a pointer into it.
                    match slot(mem).filter(|_| mem.width == m && what.op != Operation::Address) {
                        Some(at) if written => {
                            out.writes.insert(at);
                            if !moves {
                                out.reads.insert(at);
                            }
                        }
                        Some(at) => {
                            out.reads.insert(at);
                        }
                        None => {
                            out.other = true;
                            if let Some(at) = slot(mem).filter(|_| what.op != Operation::Address) {
                                out.wide.push(Reach {
                                    from: at,
                                    to: at + i64::from(mem.width),
                                    read: !written || !moves,
                                    written,
                                });
                            }
                        }
                    }
                }
            }
            Loc::Address(address) if is_bp(address.through) || is_bp(address.index_through) => out.other = true,
            _ => {}
        }
    }
    out
}

/// Blocks where slot `at` may be read before it is written.
fn live_in(
    m: u32,
    body: &LirBody,
    at: i64,
) -> BTreeSet<i64> {
    let first = body
        .blocks
        .iter()
        .map(|block| {
            let seen = block
                .insns
                .iter()
                .find_map(
                    |one| {
                        let touched = touch(m, one);
                        if touched.reads_slot(at) { Some(true) } else { touched.writes.contains(&at).then_some(false) }
                    },
                );
            (block.at, seen)
        })
        .collect::<BTreeMap<_, _>>();
    let mut live =
        first.iter().filter(|(_, seen)| **seen == Some(true)).map(|(block, _)| *block).collect::<BTreeSet<_>>();
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
fn word(
    m: u32,
    root: Register,
) -> Loc {
    Loc::Reg(Reg { register: target::named(root, i64::from(m)), width: m })
}

fn plain_word(
    m: u32,
    place: &Loc,
    root: Register,
) -> bool {
    matches!(
        place,
        Loc::Reg(reg) if reg.width == m && ir::root(reg.register) == ir::root(root)
    )
}

/// A reload of a slot into `root`: `mov r,[slot]`.
fn reload_of(
    m: u32,
    one: &Insn,
    root: Register,
) -> Option<i64> {
    let what = one.what.as_ref()?;
    match (what.op, what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Move, [dest], [Loc::Mem(mem)]) if plain_word(m, dest, root) && mem.width == m => slot(mem),
        _ => None,
    }
}

/// Whether `one` reads `root` only as a plain word source and writes none of
/// it.
fn reads_plainly(
    m: u32,
    one: &Insn,
    root: Register,
    effect: &liveness::Effect,
) -> bool {
    let Some(what) = one.what.as_ref() else {
        return false;
    };
    let lanes = _lanes(root);
    let low = _lanes(target::named(root, i64::from(m)));
    effect.writes.is_disjoint(&lanes)
        && effect.reads.iter().filter(|lane| lanes.contains(lane)).all(|lane| low.contains(lane))
        && !what.dests.iter().any(|place| {
            matches!(
                place,
                Loc::Reg(reg) if ir::root(reg.register) == ir::root(root)
            )
        })
        && what.sources.iter().any(|place| plain_word(m, place, root))
        && what.sources.iter().chain(&what.dests).all(|place| match place {
            Loc::Reg(reg) if ir::root(reg.register) == ir::root(root) => plain_word(m, place, root),
            Loc::Mem(mem) => [mem.through, mem.index_through].iter().all(|one| ir::root(*one) != ir::root(root)),
            _ => true,
        })
}

/// How the loop leaves a register free.
enum Hold {
    Untouched,
    /// It only reloads a slot for the next use: the reloads and their readers.
    Folded(Vec<(usize, usize, i64)>),
    /// Untouched but live through the loop: saved around it, at this width.
    Parked(u32),
}

/// Registers the loop leaves free, untouched ones first, parked ones last.
fn free(
    m: u32,
    roots: &[Register],
    body: &LirBody,
    effects: &[Vec<Option<liveness::Effect>>],
    one: &Loop,
    index: &BTreeMap<i64, usize>,
    live_into: &IndexMap<i64, Lanes>,
    exits: &BTreeSet<i64>,
) -> Vec<(Register, Hold)> {
    let mut out = Vec::new();
    'roots: for root in roots.iter().copied() {
        let lanes = _lanes(root);
        let through = [one.header].iter().chain(exits).any(|at| !live_into[at].is_disjoint(&lanes));
        let mut folded = Vec::new();
        for at in &one.body {
            let block = &body.blocks[index[at]];
            let mut reloaded = None;
            for (position, insn) in block.insns.iter().enumerate() {
                let Some(effect) = &effects[index[at]][position] else {
                    continue 'roots;
                };
                if effect.reads.is_disjoint(&lanes) && effect.writes.is_disjoint(&lanes) {
                    continue;
                }
                if through {
                    continue 'roots;
                }
                if let Some(from) = reload_of(m, insn, root) {
                    reloaded = Some(from);
                    folded.push((index[at], position, from));
                } else if let Some(from) = reloaded.filter(|_| reads_plainly(m, insn, root, effect)) {
                    folded.push((index[at], position, from));
                } else {
                    continue 'roots;
                }
            }
        }
        let wide = [one.header]
            .iter()
            .chain(exits)
            .any(|at| !live_into[at].is_disjoint(&lanes.minus(&_lanes(target::named(root, i64::from(m))))));
        out.push((
            root,
            match (through, folded.is_empty()) {
                (true, _) => Hold::Parked(if wide { 4 } else { m }),
                (false, true) => Hold::Untouched,
                (false, false) => Hold::Folded(folded),
            },
        ));
    }
    // Untouched registers cost nothing to take; parked ones a push and a pop.
    out.sort_by_key(|(_, hold)| match hold {
        Hold::Untouched => 0,
        Hold::Folded(_) => 1,
        Hold::Parked(_) => 2,
    });
    out
}

fn folds(hold: &Hold) -> &[(usize, usize, i64)] {
    match hold {
        Hold::Folded(folded) => folded,
        _ => &[],
    }
}

/// `one` no longer naming the values in `gone`, which dropped reloads defined.
fn unread(
    one: &Arc<Insn>,
    gone: &BTreeSet<u32>,
) -> Arc<Insn> {
    if !one.uses.iter().chain(one.requires.iter().map(|(held, _)| &held.value)).any(|value| gone.contains(value)) {
        return Arc::clone(one);
    }
    Arc::new(Insn {
        uses: one.uses.iter().copied().filter(|value| !gone.contains(value)).collect(),
        requires: one.requires.iter().copied().filter(|(held, _)| !gone.contains(&held.value)).collect(),
        ..(**one).clone()
    })
}

fn made(
    at: i64,
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Arc<Insn> {
    let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new()))
}

fn cell(
    m: u32,
    at: i64,
) -> Mem {
    // A word the frame handed out as a slot of its own: `at` is its first byte.
    Mem { through: Register::BP, disp_width: 2, ..Mem::new(Some(Addr::new(Space::Frame, at).in_slot(at)), m) }
}

/// `one` with each slot access made its register.
fn rewritten(
    m: u32,
    one: &Arc<Insn>,
    homes: &BTreeMap<i64, Loc>,
) -> Arc<Insn> {
    let Some(what) = one.what.as_ref() else {
        return Arc::clone(one);
    };
    let swap = |place: &Loc| match place {
        Loc::Mem(mem) => word_slot(m, mem).and_then(|at| homes.get(&at)).cloned().unwrap_or_else(|| place.clone()),
        other => other.clone(),
    };
    if !what.dests.iter().chain(&what.sources).any(|place| {
        matches!(
            place,
            Loc::Mem(mem) if word_slot(m, mem).is_some_and(|at| homes.contains_key(&at))
        )
    }) {
        return Arc::clone(one);
    }
    let what = Semantics {
        dests: what.dests.iter().map(swap).collect(),
        sources: what.sources.iter().map(swap).collect(),
        ..what.clone()
    };
    Arc::new(Insn { what: Some(what), spill_reload: false, spill_store: false, ..(**one).clone() })
}

/// For each of `slots`, the instructions of `seen` (by position in it) that
/// read or write it, in whole or in part. An instruction that reaches no slot
/// of the loop is in none of the lists, and is what every question of a slot
/// skips.
fn reaching_slots(
    m: u32,
    slots: &BTreeSet<i64>,
    seen: &[(usize, usize, &Arc<Insn>, &Touch)],
) -> BTreeMap<i64, Vec<usize>> {
    let mut by_slot: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (i, (_, _, _, touched)) in seen.iter().enumerate() {
        for at in touched.reads.iter().chain(&touched.writes) {
            by_slot.entry(*at).or_default().push(i);
        }
        for reach in &touched.wide {
            // `Touch::reaches`: the word at `at` overlaps the bytes `[from,
            // to)`.
            for at in slots.range(reach.from - i64::from(m) + 1..reach.to) {
                by_slot.entry(*at).or_default().push(i);
            }
        }
    }
    for list in by_slot.values_mut() {
        list.dedup();
    }
    by_slot
}

/// Whether `one` still encodes with slot `at` in a register: an x87 store
/// or a far-pointer load takes only memory.
fn registrable(
    m: u32,
    bits: u32,
    home: Register,
    one: &Arc<Insn>,
    touched: &Touch,
    at: i64,
) -> bool {
    #[cfg(test)]
    FITTED.with(|count| count.set(count.get() + 1));
    if touched.reaches(at, false) || touched.reaches(at, true) {
        return false;
    }
    if !touched.reads.contains(&at) && !touched.writes.contains(&at) {
        return true;
    }
    let homes = BTreeMap::from([(at, word(m, home))]);
    rewritten(m, one, &homes)
        .what
        .as_ref()
        .is_some_and(|what| select::priced_in(bits, what, 0, None, false, false, None).is_some())
}

/// What holding a slot in a register saves each trip: a reload whose
/// register is freed goes, one kept becomes a register move.
fn saving(
    touched: &Touch,
    one: &Insn,
    at: i64,
    folded: bool,
    costs: &OperationCosts,
) -> i64 {
    let (reads, writes) = (touched.reads_slot(at), touched.writes_slot(at));
    if !reads && !writes {
        return 0;
    }
    let plain = !folded && one.what.as_ref().is_some_and(|what| what.op == Operation::Move);
    i64::from(reads) * costs.load + i64::from(writes) * costs.store - i64::from(plain) * costs.r#move
}

/// What a hoistable register is loaded from: a frame slot, or a constant.
#[derive(PartialEq)]
enum Source {
    Slot(i64),
    Constant(Imm),
}

/// A register every write of which, inside `one`, loads one source the loop
/// never writes -- a slot, or a constant -- and which nothing reads before
/// that load. A constant counts: a segment register takes one only as
/// `push / pop`, which qbdemo's PLASMA ran every pixel.
fn invariant(
    m: u32,
    insns: &[(&Arc<Insn>, Lanes)],
    touches: &std::cell::OnceCell<Vec<Touch>>,
    entering: &Lanes,
    root: Register,
    spills: &BTreeSet<i64>,
) -> Option<Source> {
    let lanes = _lanes(root);
    if !entering.is_disjoint(&lanes) {
        return None;
    }
    let mut from = None;
    for (insn, may_write) in insns {
        if may_write.is_disjoint(&lanes) {
            continue;
        }
        let what = insn.what.as_ref()?;
        let source = match (what.op, what.dests.as_slice(), what.sources.as_slice()) {
            (Operation::Move, [Loc::Reg(reg)], [Loc::Mem(mem)])
                if ir::root(reg.register) == ir::root(root) && reg.width == m && mem.width == m =>
            {
                Source::Slot(slot(mem)?)
            }
            (Operation::Move, [Loc::Reg(reg)], [Loc::Imm(imm)])
                if ir::root(reg.register) == ir::root(root) && reg.width == m =>
            {
                Source::Constant(imm.clone())
            }
            _ => return None,
        };
        if from.as_ref().is_some_and(|one| *one != source) {
            return None;
        }
        from = Some(source);
    }
    let kept = match from.as_ref()? {
        Source::Slot(at) if spills.contains(at) => {
            let touches = touches.get_or_init(|| insns.iter().map(|(insn, _)| touch(m, insn)).collect());
            !touches.iter().any(|touched| touched.writes_slot(*at))
        }
        Source::Slot(at) => insns.iter().all(|(insn, _)| spares(m, insn, *at)),
        Source::Constant(_) => true,
    };
    kept.then_some(from?)
}

/// Each instruction of loop `one`, with the lanes it may write, or none where
/// an instruction's effect is not known (and no register is invariant): worked
/// out once for the loop, where `invariant` asked it again for every register.
fn writes_of<'a>(
    body: &'a LirBody,
    may_writes: &[Vec<Option<Lanes>>],
    one: &Loop,
    index: &BTreeMap<i64, usize>,
) -> Option<Vec<(&'a Arc<Insn>, Lanes)>> {
    let mut out = Vec::new();
    for at in &one.body {
        for (insn, may_write) in body.blocks[index[at]].insns.iter().zip(&may_writes[index[at]]) {
            out.push((insn, may_write.clone()?));
        }
    }
    Some(out)
}

/// What each instruction may write, or none where its effect is not known:
/// worked out once for the body, not once for each loop around the instruction.
fn may_writes(body: &LirBody) -> Vec<Vec<Option<Lanes>>> {
    body.blocks
        .iter()
        .map(|block| {
            block
                .insns
                .iter()
                .map(|insn| {
                    let effect = liveness::effect(body.bits, insn)?;
                    // A `rep movs` steps si and di only if it runs: a
                    // conditional write, but one that makes
                    // the register unfit to hold
                    // its value from one trip to the next.
                    Some(
                        peephole::_register_effects(body.bits, insn, true, false)
                            .map_or(effect.writes, |(_, writes)| effect.writes.or(&writes)),
                    )
                })
                .collect()
        })
        .collect()
}

/// Whether `one` cannot write the frame cell at `at`: it writes memory only
/// in far segments, at globals, or at other frame cells.
fn spares(
    m: u32,
    one: &Insn,
    at: i64,
) -> bool {
    let Some(what) = one.what.as_ref() else {
        return false;
    };
    if matches!(
        what.op,
        Operation::Call
            | Operation::Barrier
            | Operation::Escape
            | Operation::Fill
            | Operation::Copy
            | Operation::Restore
    ) {
        return false;
    }
    what.dests
        .iter()
        .all(
            |place| match place {
                Loc::Mem(mem) => match (slot(mem), mem.addr) {
                    (Some(cell), _) => (cell - at).abs() >= i64::from(m) && mem.width <= m,
                    (None, Some(addr)) => {
                        matches!(
                            addr.space,
                            Space::Far | Space::Segment | Space::Group | Space::External
                        )
                            && !is_bp(mem.through)
                    }
                    (None, None) => false,
                },
                _ => true,
            },
        )
}

/// `body` with each loop-invariant reload moved to where its loop is entered.
pub fn hoisted(
    m: u32,
    available: &[Register],
    body: &LirBody,
    spills: &BTreeSet<i64>,
) -> LirBody {
    let graph = &body.blocks;
    let predecessors = loops::predecessors(&graph);
    let index =
        body.blocks.iter().enumerate().map(|(position, block)| (block.at, position)).collect::<BTreeMap<_, _>>();
    let (live_into, _, _) = liveness::live_into(body);
    let mut blocks = body.blocks.clone();
    let mut found = loops::loops(graph, Some(body.entry));
    found.sort_by_key(|one| one.body.len());
    let mut taken = BTreeSet::<i64>::new();
    let written = may_writes(body);
    for one in &found {
        if one.body.iter().any(|at| taken.contains(at)) {
            continue;
        }
        let entries = predecessors[&one.header].iter().copied().filter(|at| !one.body.contains(at)).collect::<Vec<_>>();
        if entries.is_empty() || !entries.iter().all(|at| body.blocks[index[at]].succ == [one.header]) {
            continue;
        }
        let roots =
            available.iter().chain(target::SEGMENTS.iter().filter(|one| ![Register::CS, Register::SS].contains(one)));
        let Some(writes) = writes_of(body, &written, one, &index) else { continue };
        let touches = std::cell::OnceCell::new();
        let moved = roots
            .filter_map(|root| {
                invariant(m, &writes, &touches, &live_into[&one.header], *root, spills).map(|at| (*root, at))
            })
            .collect::<Vec<_>>();
        if moved.is_empty() {
            continue;
        }
        // The first reload goes to each entry; the others' values are its.
        let mut first = BTreeMap::<Register, Arc<Insn>>::new();
        let mut rename = IndexMap::<u32, u32>::default();
        let reloads = |insn: &Insn, root: Register| {
            insn.what
                .as_ref()
                .is_some_and(
                    |what| what.op == Operation::Move
                        && matches!(
                            what.dests.as_slice(),
                            [Loc::Reg(reg)] if ir::root(reg.register) == ir::root(root)
                        ),
                )
        };
        for at in &one.body {
            let block = &blocks[index[at]];
            let mut kept = Vec::new();
            for insn in &block.insns {
                match moved.iter().find(|(root, _)| reloads(insn, *root)) {
                    Some((root, _)) => match first.get(root) {
                        Some(chosen) => rename.extend(insn.defines.iter().copied().zip(chosen.defines.iter().copied())),
                        None => {
                            first.insert(*root, Arc::clone(insn));
                        }
                    },
                    None => kept.push(Arc::clone(insn)),
                }
            }
            blocks[index[at]] = block.with_insns(kept);
        }
        for at in &entries {
            let block = &blocks[index[at]];
            let mut insns = block.insns.to_vec();
            let jumps = insns.last().and_then(|last| last.what.as_ref()).is_some_and(|what| what.op == Operation::Jump);
            for load in first.values() {
                insns.insert(insns.len() - usize::from(jumps), Arc::clone(load));
            }
            blocks[index[at]] = block.with_insns(insns);
        }
        if !rename.is_empty() {
            for block in &mut blocks {
                *block = block.with_insns(block.insns.iter().map(|insn| spiller::_renamed(insn, &rename)).collect());
            }
        }
        taken.extend(one.body.iter().copied());
    }
    body.with_blocks(blocks)
}

/// `body` with each loop's spill slots in the registers it leaves free.
pub fn promoted(
    m: u32,
    available: &[Register],
    body: &LirBody,
    spills: &BTreeSet<i64>,
    costs: &OperationCosts,
    park: i64,
) -> LirBody {
    let graph = &body.blocks;
    let predecessors = loops::predecessors(&graph);
    let mut found = loops::loops(graph, Some(body.entry));
    // Innermost first: it runs most often.
    found.sort_by_key(|one| one.body.len());
    let index =
        body.blocks.iter().enumerate().map(|(position, block)| (block.at, position)).collect::<BTreeMap<_, _>>();
    let (live_into, _, _) = liveness::live_into(body);
    let busy = Frequency::of(body);
    let mut blocks = body.blocks.clone();
    let mut taken = BTreeSet::<i64>::new();
    // How each instruction reaches the frame, worked out once, not once for
    // each loop it is in: a nest of d loops asked every instruction of the
    // innermost d times.
    let effects: Vec<Vec<Option<liveness::Effect>>> = body
        .blocks
        .iter()
        .map(|block| block.insns.iter().map(|insn| liveness::effect(body.bits, insn)).collect())
        .collect();
    let fits = RefCell::new(BTreeMap::<(usize, usize, i64), bool>::new());
    let touches: Vec<Vec<Touch>> =
        body.blocks.iter().map(|block| block.insns.iter().map(|insn| touch(m, insn)).collect()).collect();
    for one in &found {
        if one.body.iter().any(|at| taken.contains(at)) {
            continue;
        }
        // How each instruction of the loop reaches the frame, worked out once:
        // each slot asks of the instructions that reach it, not of all
        // of them (a loop of d nested levels asked d slots of d levels'
        // instructions, d times over).
        let mut seen: Vec<(usize, usize, &Arc<Insn>, &Touch)> = Vec::new();
        for block in &one.body {
            for (position, insn) in body.blocks[index[block]].insns.iter().enumerate() {
                seen.push((index[block], position, insn, &touches[index[block]][position]));
            }
        }
        let (mut slots, mut other, mut traps) = (BTreeSet::new(), false, false);
        for (_, _, _, touched) in &seen {
            other |= touched.other;
            traps |= touched.traps;
            slots.extend(touched.reads.iter().chain(&touched.writes).copied());
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
        let registers = free(m, available, body, &effects, one, &index, &live_into, &exits);
        let reloads = registers
            .iter()
            .flat_map(|(_, hold)| folds(hold).iter().map(|(block, position, _)| (*block, *position)))
            .collect::<BTreeSet<_>>();
        // Most saved first.
        let reaching = reaching_slots(m, &slots, &seen);
        let none = Vec::new();
        #[cfg(test)]
        RANKED.with(|count| count.set(count.get() + 1));
        let mut ranked = slots
            .iter()
            .filter(|at| {
                reaching
                    .get(*at)
                    .unwrap_or(&none)
                    .iter()
                    .all(
                        |i| {
                            // Whether an instruction still encodes with the
                            // slot in a register does not depend on the loop:
                            // asked once for each instruction and slot, not
                            // once for each loop around it.
                            *fits
                                .borrow_mut()
                                .entry((seen[*i].0, seen[*i].1, **at))
                                .or_insert_with(
                                    || registrable(
                                        m,
                                        body.bits,
                                        *available.last().expect("a register to hold a slot"),
                                        seen[*i].2,
                                        seen[*i].3,
                                        **at,
                                    ),
                                )
                        },
                    )
            })
            .map(|at| {
                let mut saved = 0;
                for i in reaching.get(at).unwrap_or(&none) {
                    let (block, position, insn, touched) = &seen[*i];
                    saved += saving(touched, insn, *at, reloads.contains(&(*block, *position)), costs);
                }
                (saved, *at)
            })
            .collect::<Vec<_>>();
        ranked.sort_by_key(|(saved, at)| (std::cmp::Reverse(*saved), *at));
        let (entered, left) = (entries.len() as i64, exits.len() as i64);
        // The trips a saving repeats: the header's frequency per entry.
        let entering: f64 = entries.iter().map(|from| busy.edge(*from, one.header)).sum();
        let trips = if entering > 0.0 { busy.block(one.header) / entering } else { 1.0 };
        let written = |at: i64| reaching.get(&at).unwrap_or(&none).iter().any(|i| seen[*i].3.writes_slot(at));
        let stored = |at: i64| {
            written(at) && {
                let live = live_in(m, body, at);
                exits.iter().any(|to| live.contains(to))
            }
        };
        // BP takes the last slot when the rest fill every free register.
        let bp = !other && !traps && ranked.len() == registers.len() + 1 && !stored(ranked.last().expect("a slot").1);
        let mut homes = BTreeMap::<i64, Loc>::new();
        let mut hosts = BTreeMap::<i64, usize>::new();
        for (host, ((saved, at), (root, hold))) in ranked.iter().zip(&registers).enumerate() {
            let parked = if matches!(hold, Hold::Parked(_)) { (entered + left) * park } else { 0 };
            let cost = entered * costs.load + if stored(*at) { left * costs.store } else { 0 } + parked;
            if *saved as f64 * trips > cost as f64 {
                homes.insert(*at, word(m, *root));
                hosts.insert(*at, host);
            }
        }
        let bp_slot = ranked.last().map(|(_, at)| *at).filter(|_| bp && homes.len() == registers.len());
        if let Some(at) = bp_slot {
            let saved = ranked.last().expect("a slot").0;
            if saved as f64 * trips > (entered * (costs.store + costs.load) + left * costs.load) as f64 {
                homes.insert(at, Loc::Reg(Reg { register: target::named(Register::BP, i64::from(m)), width: m }));
            }
        }
        // A register freed by folding its reloads is free only when each
        // slot it reloads has a home; BP only when the frame is left alone.
        loop {
            let before = homes.len();
            let unfolded = hosts
                .iter()
                .filter(|(_, host)| folds(&registers[**host].1).iter().any(|(_, _, from)| !homes.contains_key(from)))
                .map(|(at, _)| *at)
                .collect::<Vec<_>>();
            for at in unfolded {
                homes.remove(&at);
                hosts.remove(&at);
            }
            if homes.len() < slots.len() {
                homes.retain(|_, home| !matches!(home, Loc::Reg(reg) if is_bp(reg.register)));
            }
            if homes.len() == before {
                break;
            }
        }
        let folds = hosts
            .values()
            .flat_map(|host| {
                let root = registers[*host].0;
                folds(&registers[*host].1).iter().map(move |(block, position, from)| (*block, *position, *from, root))
            })
            .collect::<Vec<_>>();
        if homes.is_empty() {
            continue;
        }
        let with_bp = homes.values().any(|home| matches!(home, Loc::Reg(reg) if is_bp(reg.register)));
        let parked = hosts
            .values()
            .filter_map(|host| match &registers[*host] {
                (root, Hold::Parked(width)) => {
                    Some(Loc::Reg(Reg { register: target::named(*root, i64::from(*width)), width: *width }))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for at in &one.body {
            let block = &blocks[index[at]];
            let mut insns = Vec::new();
            // What the dropped reloads defined, which their readers no longer
            // read.
            let mut gone = BTreeSet::new();
            for (position, insn) in block.insns.iter().enumerate() {
                let fold = folds.iter().find(|(inside, place, ..)| *inside == index[at] && *place == position);
                match fold {
                    Some((_, _, from, root)) => {
                        let home = homes[from].clone();
                        if reload_of(m, insn, *root).is_some() {
                            gone.extend(insn.defines.iter().copied());
                            continue;
                        }
                        let what = insn.what.as_ref().expect("a plain reader");
                        let swap = |place: &Loc| if plain_word(m, place, *root) { home.clone() } else { place.clone() };
                        let what = Semantics { sources: what.sources.iter().map(swap).collect(), ..what.clone() };
                        insns.push(rewritten(
                            m,
                            &unread(&Arc::new(Insn { what: Some(what), ..(**insn).clone() }), &gone),
                            &homes,
                        ));
                    }
                    None => insns.push(rewritten(m, &unread(insn, &gone), &homes)),
                }
            }
            blocks[index[at]] = block.with_insns(insns);
        }
        for at in &entries {
            let block = &blocks[index[at]];
            let mut insns = block.insns.to_vec();
            let jumps = insns.last().and_then(|last| last.what.as_ref()).is_some_and(|what| what.op == Operation::Jump);
            let mut place = insns.len() - usize::from(jumps);
            let near = insns.get(place.saturating_sub(1)).map_or(*at, |insn| insn.at);
            for register in &parked {
                insns.insert(place, made(near, Operation::Push, "push", vec![], vec![register.clone()]));
                place += 1;
            }
            for (slot, home) in &homes {
                let load = made(near, Operation::Move, "mov", vec![home.clone()], vec![Loc::Mem(cell(m, *slot))]);
                if matches!(home, Loc::Reg(reg) if is_bp(reg.register)) {
                    let bp = home.clone();
                    insns.insert(
                        insns.len() - usize::from(jumps),
                        made(near, Operation::Push, "push", vec![], vec![bp]),
                    );
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
            let mut insns = block.insns.to_vec();
            let near = insns.first().map_or(*to, |insn| insn.at);
            let mut place = 0;
            if with_bp {
                insns.insert(
                    0,
                    made(
                        near,
                        Operation::Pop,
                        "pop",
                        vec![Loc::Reg(Reg { register: target::named(Register::BP, i64::from(m)), width: m })],
                        vec![],
                    ),
                );
                place = 1;
            }
            for (slot, home) in &homes {
                if stored(*slot) {
                    insns.insert(
                        place,
                        made(near, Operation::Move, "mov", vec![Loc::Mem(cell(m, *slot))], vec![home.clone()]),
                    );
                    place += 1;
                }
            }
            for register in parked.iter().rev() {
                insns.insert(place, made(near, Operation::Pop, "pop", vec![register.clone()], vec![]));
                place += 1;
            }
            blocks[index[to]] = block.with_insns(insns);
        }
        taken.extend(one.body.iter().copied());
    }
    body.with_blocks(blocks)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use iced_x86::Register;

    use super::{cell, made, promoted};
    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn reg(register: Register) -> Loc {
        Loc::Reg(Reg { register, width: 2 })
    }

    fn with(
        one: Arc<Insn>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        Arc::new(Insn { defines, uses, ..(*one).clone() })
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
    ) -> LirBlock {
        LirBlock { succ, ..LirBlock::new(at, insns) }
    }

    /// qbdemo's PLASMA once DS stopped being a selector: a slot's reload in
    /// a loop was folded into the register made its home, and an empty
    /// marker after it still named the reload's value. Verification refused
    /// "value#255 is read but never defined".
    #[test]
    fn test_a_dropped_reload_leaves_no_reader_of_its_value() {
        let one = Imm { value: 1, width: 2, address: None };
        let bump = |at: i64, register: Register| {
            made(at, Operation::Binary, "add", vec![reg(register)], vec![reg(register), Loc::Imm(one.clone())])
        };
        let body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![made(0, Operation::Jump, "jmp", vec![], vec![])], vec![0x10]),
                block(
                    0x10,
                    vec![
                        bump(0x10, Register::BX),
                        bump(0x11, Register::DX),
                        bump(0x12, Register::DI),
                        with(
                            made(0x13, Operation::Move, "mov", vec![reg(Register::SI)], vec![Loc::Mem(cell(2, -4))]),
                            vec![5],
                            vec![],
                        ),
                        with(
                            Arc::new(Insn::new(
                                0x13,
                                Some((0x13, 0x13)),
                                Some(Semantics { name: Some(String::new()), ..Semantics::new(Operation::Nothing) }),
                                vec![],
                                vec![],
                            )),
                            vec![],
                            vec![5],
                        ),
                        with(
                            made(
                                0x14,
                                Operation::Binary,
                                "add",
                                vec![reg(Register::AX)],
                                vec![reg(Register::AX), reg(Register::SI)],
                            ),
                            vec![],
                            vec![5],
                        ),
                        with(
                            made(0x15, Operation::Move, "mov", vec![reg(Register::SI)], vec![Loc::Mem(cell(2, -4))]),
                            vec![6],
                            vec![],
                        ),
                        with(
                            made(
                                0x15,
                                Operation::Binary,
                                "add",
                                vec![reg(Register::BX)],
                                vec![reg(Register::BX), reg(Register::SI)],
                            ),
                            vec![],
                            vec![6],
                        ),
                        bump(0x15, Register::CX),
                        Arc::new(Insn::new(
                            0x16,
                            Some((0x16, 0x17)),
                            Some(Semantics {
                                name: Some("jne".to_owned()),
                                target: Some(0x10),
                                ..Semantics::new(Operation::Branch)
                            }),
                            vec![],
                            vec![],
                        )),
                    ],
                    vec![0x10, 0x20],
                ),
                block(
                    0x20,
                    vec![
                        made(
                            0x20,
                            Operation::Move,
                            "mov",
                            vec![Loc::Reg(Reg { register: Register::ESI, width: 4 })],
                            vec![Loc::Imm(Imm { width: 4, ..one.clone() })],
                        ),
                        made(0x21, Operation::Return, "ret", vec![], vec![]),
                    ],
                    vec![],
                ),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let costs = &crate::backend::cpu::profile("486").unwrap().operations;
        let out = promoted(
            2,
            &crate::backend::classes::RegisterClasses::m16().available,
            &body,
            &BTreeSet::from([-4]),
            costs,
            2,
        );
        let reloads = |body: &LirBody| body.insns().iter().filter(|one| one.defines.contains(&5)).count();
        assert_eq!(reloads(&body), 1);
        assert_eq!(reloads(&out), 0, "the reload stays: the test does not reach the fold");
        let said = crate::backend::verify::verify(&out, false);
        assert!(said.is_empty(), "{said:?}");
    }

    /// `for (...) copy(a, b)` with `mov di, a` in the loop: `rep movsd` leaves
    /// di past the cells it moved, so a hoisted `mov di, a` ran once and
    /// every later trip copied to the wrong place
    /// (tests/run/c/es_across_copy printed 15054 for 186450).
    #[test]
    fn test_a_register_a_string_move_advances_is_not_loop_invariant() {
        let word = |value: i64| Loc::Imm(Imm { value, width: 2, address: None });
        let copy = Semantics {
            name: Some("movsd".to_owned()),
            dests: vec![
                Loc::Mem(crate::model::ir::Mem::new(None, 0)),
                reg(Register::SI),
                reg(Register::DI),
                reg(Register::CX),
            ],
            sources: vec![
                reg(Register::CX),
                reg(Register::SI),
                reg(Register::DI),
                reg(Register::DS),
                reg(Register::ES),
            ],
            ..Semantics::new(Operation::Copy)
        };
        let body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![made(0, Operation::Jump, "jmp", vec![], vec![])], vec![0x10]),
                block(
                    0x10,
                    vec![
                        made(0x10, Operation::Move, "mov", vec![reg(Register::SI)], vec![word(8)]),
                        made(0x11, Operation::Move, "mov", vec![reg(Register::DI)], vec![word(4)]),
                        made(0x12, Operation::Move, "mov", vec![reg(Register::CX)], vec![word(50)]),
                        made(0x12, Operation::Move, "mov", vec![reg(Register::BX)], vec![word(7)]),
                        Arc::new(Insn::new(0x13, Some((0x13, 0x14)), Some(copy), vec![], vec![])),
                        Arc::new(Insn::new(
                            0x15,
                            Some((0x15, 0x16)),
                            Some(Semantics {
                                name: Some("jne".to_owned()),
                                target: Some(0x10),
                                ..Semantics::new(Operation::Branch)
                            }),
                            vec![],
                            vec![],
                        )),
                    ],
                    vec![0x10, 0x20],
                ),
                block(
                    0x20,
                    vec![Arc::new(Insn {
                        reads_complete: true,
                        ..(*made(0x20, Operation::Return, "ret", vec![], vec![])).clone()
                    })],
                    vec![],
                ),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let out =
            super::hoisted(2, &crate::backend::classes::RegisterClasses::m16().available, &body, &BTreeSet::new());
        let in_loop = out.blocks.iter().find(|one| one.at == 0x10).expect("the loop");
        let sets = |register: Register| {
            in_loop
                .insns
                .iter()
                .any(
                    |one| matches!(
                        one.what.as_ref().map(|what| what.dests.as_slice()),
                        Some([Loc::Reg(reg)]) if reg.register == register
                    ),
                )
        };
        assert!(!sets(Register::BX), "premise: an invariant constant leaves the loop");
        assert!(sets(Register::DI) && sets(Register::SI), "the loop sets si and di");
    }

    /// m32's slots are dwords: LoopSlots was dropped there. Its BP parking once
    /// found no BP to pop, because EBP is not the register BP: the loop
    /// left its exit with EBP holding a slot's value and a program ran
    /// quicksort into a general protection fault.
    #[test]
    fn test_a_dword_slot_parked_in_the_frame_register_is_pushed_and_popped() {
        let wide = |register: Register| Loc::Reg(Reg { register, width: 4 });
        let one = Imm { value: 1, width: 4, address: None };
        let bump = |at: i64, register: Register| {
            made(at, Operation::Binary, "add", vec![wide(register)], vec![wide(register), Loc::Imm(one.clone())])
        };
        let slot = |at: i64| Loc::Mem(cell(4, at));
        let busy = [Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI];
        let mut loop_body: Vec<Arc<Insn>> =
            busy.iter().enumerate().map(|(at, register)| bump(0x10 + at as i64, *register)).collect();
        for at in 0..8 {
            loop_body.push(made(
                0x20 + at,
                Operation::Binary,
                "add",
                vec![wide(Register::EAX)],
                vec![wide(Register::EAX), slot(-4)],
            ));
        }
        loop_body.push(Arc::new(Insn::new(
            0x30,
            Some((0x30, 0x31)),
            Some(Semantics { name: Some("jne".to_owned()), target: Some(0x10), ..Semantics::new(Operation::Branch) }),
            vec![],
            vec![],
        )));
        let body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![made(0, Operation::Jump, "jmp", vec![], vec![])], vec![0x10]),
                block(0x10, loop_body, vec![0x10, 0x40]),
                block(0x40, vec![made(0x40, Operation::Return, "ret", vec![], vec![])], vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let costs = &crate::backend::cpu::profile("486").unwrap().operations;
        let out = promoted(
            4,
            &crate::backend::classes::RegisterClasses::m16().available,
            &body,
            &BTreeSet::from([-4]),
            costs,
            2,
        );
        let is_bp = |insn: &Arc<Insn>, op: Operation| {
            insn.what
                .as_ref()
                .is_some_and(
                    |what| what.op == op
                        && what
                            .dests
                            .iter()
                            .chain(&what.sources)
                            .any(|place| matches!(place, Loc::Reg(reg) if super::is_bp(reg.register))),
                )
        };
        let pushes = out.insns().iter().filter(|insn| is_bp(insn, Operation::Push)).count();
        let pops = out.insns().iter().filter(|insn| is_bp(insn, Operation::Pop)).count();
        assert_eq!((pushes, pops), (1, 1), "the slot lives in EBP between a push and a pop");
    }

    /// Ranking a loop's slots asked every instruction of the loop how it
    /// reaches the frame, once for each slot, and each ask allocated its
    /// sets: a nest of d levels (d loops of d levels' slots and instructions)
    /// cost d cubed, 215 M for a 16-deep nest of which 169 M was this. Each
    /// instruction is asked once and each slot is asked of those that reach
    /// it.
    #[test]
    fn test_a_loops_slots_are_ranked_without_asking_every_instruction_for_each_slot() {
        let slot = |at: i64| Loc::Mem(cell(2, at));
        let mut loop_body: Vec<Arc<Insn>> = (0..12)
            .map(|at| {
                made(
                    0x20 + at,
                    Operation::Binary,
                    "add",
                    vec![reg(Register::AX)],
                    vec![reg(Register::AX), slot(-2 * (at + 1))],
                )
            })
            .collect();
        loop_body.push(Arc::new(Insn::new(
            0x30,
            Some((0x30, 0x31)),
            Some(Semantics { name: Some("jne".to_owned()), target: Some(0x10), ..Semantics::new(Operation::Branch) }),
            vec![],
            vec![],
        )));
        let count = loop_body.len();
        let body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![made(0, Operation::Jump, "jmp", vec![], vec![])], vec![0x10]),
                block(0x10, loop_body, vec![0x10, 0x40]),
                block(0x40, vec![made(0x40, Operation::Return, "ret", vec![], vec![])], vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let costs = &crate::backend::cpu::profile("486").unwrap().operations;
        let before = super::TOUCHED.with(std::cell::Cell::get);
        promoted(
            2,
            &crate::backend::classes::RegisterClasses::m16().available,
            &body,
            &(1..=12).map(|at| -2 * at).collect(),
            costs,
            2,
        );
        let asked = super::TOUCHED.with(std::cell::Cell::get) - before;
        assert!(
            asked <= 2 * count,
            "{asked} asks of how an instruction reaches the frame for a loop of {count} instructions and 12 slots"
        );
    }

    /// A loop that reads a far pointer's low word and also loads the pointer
    /// with `les` from the same slot: LoopSlots held the word's slot in a
    /// register and rewrote the `les` to read it ("les si, dx":
    /// no encoding, llrm-nib stopped, #107). The `les` must keep reading
    /// memory.
    #[test]
    fn test_a_far_pointer_load_keeps_its_slot_in_memory_when_a_loop_holds_the_low_word() {
        let one = Imm { value: 1, width: 2, address: None };
        let bump = |at: i64, register: Register| {
            made(at, Operation::Binary, "add", vec![reg(register)], vec![reg(register), Loc::Imm(one.clone())])
        };
        let les =
            made(0x14, Operation::Move, "les", vec![reg(Register::BX), reg(Register::ES)], vec![Loc::Mem(cell(4, -4))]);
        let body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![made(0, Operation::Jump, "jmp", vec![], vec![])], vec![0x10]),
                block(
                    0x10,
                    vec![
                        bump(0x10, Register::DX),
                        made(
                            0x13,
                            Operation::Binary,
                            "add",
                            vec![reg(Register::AX)],
                            vec![reg(Register::AX), Loc::Mem(cell(2, -4))],
                        ),
                        les,
                        made(
                            0x17,
                            Operation::Binary,
                            "add",
                            vec![reg(Register::AX)],
                            vec![reg(Register::AX), Loc::Mem(cell(2, -4))],
                        ),
                        made(
                            0x17,
                            Operation::Binary,
                            "add",
                            vec![reg(Register::AX)],
                            vec![reg(Register::AX), Loc::Mem(cell(2, -4))],
                        ),
                        made(
                            0x17,
                            Operation::Binary,
                            "add",
                            vec![reg(Register::AX)],
                            vec![reg(Register::AX), Loc::Mem(cell(2, -4))],
                        ),
                        made(
                            0x17,
                            Operation::Binary,
                            "add",
                            vec![reg(Register::AX)],
                            vec![reg(Register::AX), Loc::Mem(cell(2, -4))],
                        ),
                        Arc::new(Insn::new(
                            0x18,
                            Some((0x18, 0x19)),
                            Some(Semantics {
                                name: Some("jne".to_owned()),
                                target: Some(0x10),
                                ..Semantics::new(Operation::Branch)
                            }),
                            vec![],
                            vec![],
                        )),
                    ],
                    vec![0x10, 0x20],
                ),
                block(0x20, vec![made(0x20, Operation::Return, "ret", vec![], vec![])], vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let costs = &crate::backend::cpu::profile("486").unwrap().operations;
        let out = promoted(
            2,
            &crate::backend::classes::RegisterClasses::m16().available,
            &body,
            &BTreeSet::from([-4]),
            costs,
            2,
        );
        let reads = |body: &LirBody| {
            body.insns()
                .iter()
                .filter_map(|one| one.what.as_ref())
                .filter(|what| what.sources.iter().any(|place| matches!(place, Loc::Mem(mem) if mem.width == 2)))
                .count()
        };
        assert_eq!(
            reads(&out),
            reads(&body),
            "the word reads stay in memory: the les reads the slot there, a register copy would be stale"
        );
    }

    /// `nest(d)` as the front end's pipeline hands it to isel: whether an
    /// instruction fits a slot in a register was asked again for each loop
    /// around it, so that the asks grew with the square of the depth. Asked
    /// once for each instruction and slot, they grow with the nest.
    #[test]
    fn test_the_nest_asks_what_fits_a_slot_once_however_deep_the_loops() {
        use crate::backend::regalloc_input::{Calls, before_phase};
        let asked = |depth: usize| {
            let (body, mut phases) =
                before_phase(Calls::C, &format!("loopslots_nest{depth}.ll"), "_fn", "486", "LoopSlots");
            super::FITTED.with(|count| count.set(0));
            super::RANKED.with(|count| count.set(0));
            phases[0].transform(body).expect("loop slots");
            (super::RANKED.with(std::cell::Cell::get), super::FITTED.with(std::cell::Cell::get))
        };
        let ((ranked4, fitted4), (ranked8, fitted8), (ranked16, fitted16)) = (asked(4), asked(8), asked(16));
        assert!(
            ranked4 >= 3 && ranked8 >= 7 && ranked16 >= 15,
            "premise: every loop of the nest but the outermost, which the function's exit bounds, is ranked ({ranked4}, {ranked8}, {ranked16})"
        );
        assert!(
            fitted16 < 3 * fitted8 && fitted8 < 3 * fitted4,
            "{fitted4}, {fitted8}, {fitted16} asks for nests 4, 8 and 16 deep"
        );
    }
}
