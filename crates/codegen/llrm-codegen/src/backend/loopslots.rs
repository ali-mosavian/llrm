//! Moves a loop's invariant loads, of a spill slot or of a constant, out of
//! the loop to its entry edges.
//!
//! # How it works
//! Allocation spills a value across its whole range, and reloads it in the
//! loop. A register whose every write inside a natural loop loads one source
//! the loop never writes (a spill slot, or a constant) and which nothing reads
//! before that load keeps one load, placed at the loop's entry edges; the
//! other loads of it are dropped and their values renamed to the first's.
//! Segment registers qualify too: this is what keeps `ES` set once ahead of a
//! loop of string moves (`pushw DGROUP; pop es` is a physical definition that
//! MachineLICM does not hoist, and the pin has no 16-bit target to say).
//! Loops are taken innermost first; a loop that overlaps one already taken is
//! skipped.
//!
//! DEVIATE LS1 (2026-10-25): the pass used to run a second step, `promoted`,
//! which gave a loop's word-wide spill slots registers the loop left free (a
//! push and pop around the loop for a register that is live through). Retired:
//! measured with it off against the same parent (own dists), m16 executed
//! instructions (`LLRM_DEBUG=cost`, 71 programs, -march=i386) -O1 0.000%, -O2
//! -0.000%, -O3 -0.000%, -Os -0.001%; m32 (rows, qcport.py, objsweep): QCport
//! -O1 -0.006%, -O2 -0.009%, -O3 -0.007%, -Os -0.018%; 7 of 568 objects change;
//! x_transpose +0.55% clocks (-O2) and +0.58% (-Os), code -4.9% and -1.2%. The
//! other half, `hoisted`, off: m16 executed instructions +0.13% (-O1), +0.13%
//! (-O2), +0.01% (-O3), +0.18% (-Os), shellsort +8.3%, and m32 -Os rows up to
//! +3.3% instructions, so it stays.
//!
//! # Example
//! A loop of two string moves sets `ES` ahead of the loop, once, not before
//! each move (`isel_tests::test_near_string_ops_set_es_once_and_save_none`).
//!
//! # Runs
//! After `peephole` (spill traffic is final), before `schedule`. Always on;
//! loops that qualify are rewritten and others are left as they are.
//! `LLRM_DEBUG=traffic` prints the spill traffic by cause first.
//!
//! # Needs
//! [`crate::backend::liveness`] for what each instruction writes and what is
//! live into the header and the exits, and the loop forest and block
//! frequencies from `analysis`.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_lir::registers::Regs;

use crate::analysis::frequency::Frequency;
use crate::analysis::intervals as ranges;
use crate::analysis::loops::{self, Loop};
use crate::backend::classes::RegisterClasses;
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::frame::Frame;
use crate::backend::liveness;
use crate::backend::peephole::{self, _lanes, Lanes};
use crate::model::ir::{self, Imm, Loc, Mem, Operation, Space};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
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
        llrm_support::debug!("traffic", "{} {:?}", body.name, traffic_by_cause(&body));
        Ok(hoisted(m, &self.classes.available, &body, &spills))
    }
}

/// The slot `mem` names, when it is a whole frame cell.
fn slot(mem: &Mem) -> Option<i64> {
    let addr = mem.addr?;
    (addr.space == Space::Frame
        && addr.base == (RegId::None).iced()
        && mem.through == ir::FRAME
        && mem.base.is_none()
        && mem.index.is_none()
        && mem.index_through == RegId::None)
        .then_some(addr.disp + mem.offset)
}

fn is_bp(
    regs: Regs,
    register: RegId,
) -> bool {
    register != RegId::None && regs.is_frame(register)
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
    static SEEN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn touch(
    regs: Regs,
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
        || one.clobbers.iter().chain(&one.clobbers_high).any(|register| is_bp(regs, *register));
    let moves = what.op == Operation::Move;
    for (place, written) in what.dests.iter().map(|one| (one, true)).chain(what.sources.iter().map(|one| (one, false)))
    {
        match place {
            Loc::Reg(reg) if is_bp(regs, reg.register) => out.other = true,
            Loc::Mem(mem) => {
                if is_bp(regs, mem.index_through) || mem.addr.is_some_and(|addr| addr.space == Space::Stack) {
                    out.other = true;
                } else if is_bp(regs, mem.through) {
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
            Loc::Address(address) if is_bp(regs, address.through) || is_bp(regs, address.index_through) => {
                out.other = true
            }
            _ => {}
        }
    }
    out
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
    regs: Regs,
    m: u32,
    insns: &[(&Arc<Insn>, &Lanes)],
    touches: &std::cell::OnceCell<Vec<Touch>>,
    entering: &Lanes,
    root: RegId,
    spills: &BTreeSet<i64>,
) -> Option<Source> {
    let lanes = _lanes(regs, root);
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
            let touches = touches.get_or_init(|| insns.iter().map(|(insn, _)| touch(regs, m, insn)).collect());
            !touches.iter().any(|touched| touched.writes_slot(*at))
        }
        Source::Slot(at) => insns.iter().all(|(insn, _)| spares(regs, m, insn, *at)),
        Source::Constant(_) => true,
    };
    kept.then_some(from?)
}

/// Each instruction of loop `one`, with the lanes it may write, or none where
/// an instruction's effect is not known (and no register is invariant): worked
/// out once for the loop, where `invariant` asked it again for every register.
fn writes_of<'a>(
    body: &'a LirBody,
    may_writes: &'a [Vec<Option<Lanes>>],
    one: &Loop,
    index: &BTreeMap<i64, usize>,
) -> Option<Vec<(&'a Arc<Insn>, &'a Lanes)>> {
    let mut out = Vec::new();
    for at in &one.body {
        for (insn, may_write) in body.blocks[index[at]].insns.iter().zip(&may_writes[index[at]]) {
            out.push((insn, may_write.as_ref()?));
        }
    }
    Some(out)
}

/// What each instruction may write, or none where its effect is not known:
/// worked out once for the body, not once for each loop around the instruction.
fn may_writes(body: &LirBody) -> Vec<Vec<Option<Lanes>>> {
    let regs = body.regs();
    body.blocks
        .iter()
        .map(|block| {
            block
                .insns
                .iter()
                .map(|insn| {
                    let effect = liveness::effect(regs, body.bits, insn)?;
                    // A `rep movs` steps si and di only if it runs: a
                    // conditional write, but one that makes
                    // the register unfit to hold
                    // its value from one trip to the next.
                    Some(
                        peephole::_register_effects(regs, body.bits, insn, true, false)
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
    regs: Regs,
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
                            && !is_bp(regs, mem.through)
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
    available: &[RegId],
    body: &LirBody,
    spills: &BTreeSet<i64>,
) -> LirBody {
    let regs = body.regs();
    let graph = &body.blocks;
    let predecessors = loops::predecessors(&graph);
    let index =
        body.blocks.iter().enumerate().map(|(position, block)| (block.at, position)).collect::<BTreeMap<_, _>>();
    let (live_into, _, _) = liveness::live_into(body);
    let mut blocks = body.blocks.clone();
    let mut found = loops::loops(graph, Some(body.entry));
    found.sort_by_key(|one| one.body.len());
    let mut taken = Taken::default();
    let written = may_writes(body);
    let segment_roots: Vec<RegId> =
        regs.segments().filter(|one| !regs.is_code_segment(*one) && !regs.is_stack_segment(*one)).collect();
    for one in &found {
        if taken.overlaps(one.body.forest(), one.body.index()) {
            continue;
        }
        let entries = predecessors[&one.header].iter().copied().filter(|at| !one.body.contains(at)).collect::<Vec<_>>();
        if entries.is_empty() || !entries.iter().all(|at| body.blocks[index[at]].succ == [one.header]) {
            continue;
        }
        let roots = available.iter().chain(segment_roots.iter());
        let Some(writes) = writes_of(body, &written, one, &index) else { continue };
        let touches = std::cell::OnceCell::new();
        let moved = roots
            .filter_map(|root| {
                invariant(regs, m, &writes, &touches, &live_into[&one.header], *root, spills).map(|at| (*root, at))
            })
            .collect::<Vec<_>>();
        if moved.is_empty() {
            continue;
        }
        // The first reload goes to each entry; the others' values are its.
        let mut first = BTreeMap::<RegId, Arc<Insn>>::new();
        let mut rename = IndexMap::<u32, u32>::default();
        let reloads = |insn: &Insn, root: RegId| {
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
                *block = block.with_insns(
                    block.insns.iter().map(|insn| crate::backend::bodyfacts::_renamed(insn, &rename)).collect(),
                );
            }
        }
        taken.take(one.body.forest(), one.body.index());
    }
    body.with_blocks(blocks)
}

/// The loops that `promoted` has taken, as the forest has them: a loop
/// overlaps one taken where it is inside one, holds one, or is one.
#[derive(Default)]
struct Taken {
    /// Loops taken.
    taken: BTreeSet<usize>,
    /// Loops with a taken loop inside them.
    holding: BTreeSet<usize>,
}

impl Taken {
    fn overlaps(
        &self,
        forest: &llrm_mir::forest::Forest,
        index: usize,
    ) -> bool {
        if self.holding.contains(&index) {
            return true;
        }
        let mut at = Some(index);
        while let Some(one) = at {
            if self.taken.contains(&one) {
                return true;
            }
            at = forest.loops[one].parent;
        }
        false
    }

    fn take(
        &mut self,
        forest: &llrm_mir::forest::Forest,
        index: usize,
    ) {
        self.taken.insert(index);
        let mut at = forest.loops[index].parent;
        while let Some(one) = at {
            self.holding.insert(one);
            at = forest.loops[one].parent;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn reg(register: RegId) -> Loc {
        Loc::Reg(Reg { register, width: 2 })
    }

    fn made(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new()))
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
    ) -> LirBlock {
        LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }
    }

    /// `for (...) copy(a, b)` with `mov di, a` in the loop: `rep movsd` leaves
    /// di past the cells it moved, so a hoisted `mov di, a` ran once and
    /// every later trip copied to the wrong place
    /// (tests/run/c/es_across_copy printed 15054 for 186450).
    #[test]
    fn test_a_register_a_string_move_advances_is_not_loop_invariant() {
        let word = |value: i64| Loc::Imm(Imm { value, width: 2, address: None });
        let copy = Semantics {
            name: Some("movsd".into()),
            dests: vec![Loc::Mem(crate::model::ir::Mem::new(None, 0)), reg(RegId::SI), reg(RegId::DI), reg(RegId::CX)],
            sources: vec![reg(RegId::CX), reg(RegId::SI), reg(RegId::DI), reg(RegId::DS), reg(RegId::ES)],
            ..Semantics::new(Operation::Copy)
        };
        let body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![made(0, Operation::Jump, "jmp", vec![], vec![])], vec![0x10]).into(),
                block(
                    0x10,
                    vec![
                        made(0x10, Operation::Move, "mov", vec![reg(RegId::SI)], vec![word(8)]),
                        made(0x11, Operation::Move, "mov", vec![reg(RegId::DI)], vec![word(4)]),
                        made(0x12, Operation::Move, "mov", vec![reg(RegId::CX)], vec![word(50)]),
                        made(0x12, Operation::Move, "mov", vec![reg(RegId::BX)], vec![word(7)]),
                        Arc::new(Insn::new(0x13, Some((0x13, 0x14)), Some(copy), vec![], vec![])),
                        Arc::new(Insn::new(
                            0x15,
                            Some((0x15, 0x16)),
                            Some(Semantics {
                                name: Some("jne".into()),
                                target: Some(0x10),
                                ..Semantics::new(Operation::Branch)
                            }),
                            vec![],
                            vec![],
                        )),
                    ],
                    vec![0x10, 0x20],
                )
                .into(),
                block(
                    0x20,
                    vec![Arc::new(Insn {
                        reads_complete: true,
                        ..(*made(0x20, Operation::Return, "ret", vec![], vec![])).clone()
                    })],
                    vec![],
                )
                .into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let out =
            super::hoisted(2, &crate::backend::classes::RegisterClasses::m16().available, &body, &BTreeSet::new());
        let in_loop = out.blocks.iter().find(|one| one.at == 0x10).expect("the loop");
        let sets = |register: RegId| {
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
        assert!(!sets(RegId::BX), "premise: an invariant constant leaves the loop");
        assert!(sets(RegId::DI) && sets(RegId::SI), "the loop sets si and di");
    }

    /// A loop with every register busy, reading a spill slot eight times a trip
    /// as a memory operand, had the slot parked in the frame register
    /// between a `push` and a `pop` by the second step (`promoted`),
    /// retired as neutral at m16 executed instructions and 0.01% smaller on
    /// QCport (header, DEVIATE LS1). What is left only hoists a loaded
    /// invariant, and a memory operand is no load.
    #[test]
    fn test_a_slot_read_as_a_memory_operand_in_a_loop_stays_in_memory() {
        use super::LoopSlots;
        use crate::backend::frame::Frame;
        use crate::model::ir::{Addr, Mem, Space};
        use crate::model::passes::LIRTransform;
        let regs = crate::backend::registerinfo::test_regs();
        let wide = |register: RegId| Loc::Reg(Reg { register, width: 4 });
        let one = Imm { value: 1, width: 4, address: None };
        let bump = |at: i64, register: RegId| {
            made(at, Operation::Binary, "add", vec![wide(register)], vec![wide(register), Loc::Imm(one.clone())])
        };
        let slot = Loc::Mem(Mem {
            through: RegId::BP,
            disp_width: 2,
            ..Mem::new(Some(Addr::new(Space::Frame, -4).in_slot(-4)), 4)
        });
        let busy = [RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX, RegId::ESI, RegId::EDI];
        let mut loop_body: Vec<Arc<Insn>> =
            busy.iter().enumerate().map(|(at, register)| bump(0x10 + at as i64, *register)).collect();
        for at in 0..8 {
            loop_body.push(made(
                0x20 + at,
                Operation::Binary,
                "add",
                vec![wide(RegId::EAX)],
                vec![wide(RegId::EAX), slot.clone()],
            ));
        }
        loop_body.push(Arc::new(Insn::new(
            0x30,
            Some((0x30, 0x31)),
            Some(Semantics { name: Some("jne".into()), target: Some(0x10), ..Semantics::new(Operation::Branch) }),
            vec![],
            vec![],
        )));
        let body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![made(0, Operation::Jump, "jmp", vec![], vec![])], vec![0x10]).into(),
                block(0x10, loop_body, vec![0x10, 0x40]).into(),
                block(0x40, vec![made(0x40, Operation::Return, "ret", vec![], vec![])], vec![]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let mut frame = Frame::new(0);
        frame.capacities.insert(-4, 4);
        let mut pass = LoopSlots {
            frame: Some(std::rc::Rc::new(std::cell::RefCell::new(frame))),
            cpu: crate::backend::cpu::profile("486").unwrap().clone(),
            word: 4,
            classes: crate::backend::classes::RegisterClasses::m16(),
        };
        let mentions = |body: &LirBody| {
            body.insns()
                .iter()
                .flat_map(|one| one.what.iter().flat_map(|what| what.sources.iter().chain(&what.dests)))
                .filter(|place| matches!(place, Loc::Mem(_)))
                .count()
        };
        assert_eq!(mentions(&body), 8, "the premise: eight reads of the slot in the loop");
        let out = pass.transform(body).unwrap();
        let is_bp = |insn: &Arc<Insn>, op: Operation| {
            insn.what
                .as_ref()
                .is_some_and(
                    |what| what.op == op
                        && what
                            .dests
                            .iter()
                            .chain(&what.sources)
                            .any(|place| matches!(place, Loc::Reg(reg) if super::is_bp(regs, reg.register))),
                )
        };
        let parked = out.iter_insns().filter(|insn| is_bp(insn, Operation::Push)).count();
        assert_eq!((parked, mentions(&out)), (0, 8), "the slot stays in memory");
    }
}

/// Frame traffic inside loops by cause, weighted by loop depth: the spiller's
/// reloads, stores and rematerializations, and the frame operands of x87 and
/// other instructions. The allocator can reach only the first three.
pub fn traffic_by_cause(body: &LirBody) -> std::collections::BTreeMap<&'static str, f64> {
    let deep = ranges::depths(body);
    let busy = Frequency::of(body);
    let mut out = std::collections::BTreeMap::new();
    for block in &body.blocks {
        let depth = deep.get(&block.at).copied().unwrap_or(0);
        if depth == 0 {
            continue;
        }
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            let frame = what
                .dests
                .iter()
                .chain(&what.sources)
                .any(
                    |place| matches!(
                        place,
                        Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.space == Space::Frame)
                    ),
                );
            let float = what.op.is_x87();
            let cause = match () {
                _ if one.spill_reload => "reload",
                _ if one.spill_store => "store",
                _ if one.rematerialized => "remat",
                _ if frame && float => "x87-frame",
                _ if frame => "int-frame",
                _ => continue,
            };
            *out.entry(cause).or_insert(0.0) += busy.block(block.at);
        }
    }
    out
}
