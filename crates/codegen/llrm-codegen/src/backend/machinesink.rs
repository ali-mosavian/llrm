//! Moves a computation into the one successor block that uses it, breaking a
//! critical edge where that is what lets it go. Translated from
//! [MachineSink.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp)
//! (`MachineSinking`, before PHI elimination, on SSA).
//!
//! # How it works
//! Each block with several successors is walked bottom-up. An instruction that
//! is safe to move ([`safe_to_move`]: no store, call or volatile access, a
//! load only above every store of its block, no register but virtual ones)
//! goes to the successor that dominates every use of the values it defines
//! (`FindSuccToSinkTo`): a use in a phi counts at the end of the phi's
//! predecessor. Successors are tried by block frequency, then by loop depth,
//! least first. A sink that leaves the block it started in on every path is
//! kept only where `isProfitableToSinkTo` says it shortens the execution of
//! some path.
//!
//! Where the successor has other predecessors the instruction would run on
//! paths that never used it, so the edge is marked to be split
//! (`PostponeSplitCriticalEdge`). After the walk the marked edges get a block
//! of their own and the walk runs again: the instruction now sinks into the new
//! block, which is its successor's only predecessor.
//!
//! ```text
//!  before                          after
//!  b1: v = mov 0                   b1: cmp; je e
//!      cmp; je b4                  b2: ...        e: v = mov 0; jmp b4
//!  b2: ...; jmp b4                 b4: r = phi [b2: x] [e: v]
//!  b4: r = phi [b1: v] [b2: x]
//! ```
//!
//! DEVIATE M7 (row 44): `hasStoreBetween` counts every store, call and
//! ordered access as one that may alias the load: no alias analysis reaches
//! LIR (`mayAlias(AA, MI)` is always true). Retires M1, which sank no load
//! along a critical edge. Exit: memory operands with an alias query on LIR.
//!
//! Row M2 (was DEVIATE M2 #2903, retired 2026-10-25): inside a cycle an
//! instruction goes to a post-dominating block when no in-cycle operand it
//! reads would be held there past the block's pressure (the limit is the number
//! of allocatable registers, one for every set). x_bubble -O2 m32: the inner
//! loop's `inc` goes to the latch as the pin has it.
//!
//! Row M4 (was DEVIATE M4, retired 2026-10-25 on the greedy allocator): a build
//! for size moves what the pin moves; the rule that kept an instruction in
//! place when its operand lives on, is read below the arm or dies in the block
//! is gone. Measured (matched pair, own dists, rows, qcport.py, objsweep):
//! x_collatz -Os clocks -7.2%, instructions -11.7%, code -4.4% (the pin's
//! shape: `lea` into the odd arm, `shr` into the even one), sieve -10.3%
//! clocks, matmul -2.9% clocks and -5.3% code; objects -Os: x_mergesort m16
//! -219 B, x_collatz m16 -105, matmul m32 -113, sieve m16 +82; QCport -Os
//! 263235 -> 263448 (+213 B, +0.081%, 40 modules changed; -O1 to -O3
//! identical). The rule had bought -85 B of QCport -Os on the old allocator.
//!
//! DEVIATE M5 (#2903): in a build for size no edge is broken, except where the
//! instruction is in a loop (row M6, below); the pin breaks it at any size
//! (`isWorthBreakingCriticalEdge`, MachineSink.cpp:1040-1105, has no size
//! check). Without the rule m16 -Os grows 66 B in 22 objects (recgcd +5,
//! recsum +5, switch +4, bintree +4), m32 79 B. The block
//! costs a jump (`bench/recpow` -Os +5 bytes with the edge split; over the
//! 834-object sweep at head 1b78847ae, breaking it at -Os is 37 objects larger,
//! +228 bytes, and 26 smaller, -106 bytes). `bench/hanoi` -Os m32 keeps the
//! `xor` and a copy before its test, and a frame, where the pin sinks the
//! constant (195 bytes against 190 with the edge broken). Exit, decided
//! 2026-10-22 with jumps' block-by-block diff: a placement rule jumps names,
//! or sinking with shrink-wrapping (lir-cleanup-port for the sinking half).
//!
//! Row M6: an edge of a loop is broken in a build for size too (x_collatz -Os
//! m32 313 -> 309 B, m16 340 -> 334, no object grows): the
//! pin's `mov ecx, 32` of an exit phi runs once, in a block of its own, not
//! every trip in the latch. Measured (own dist pair, sizes.py, clocksweep, m16
//! -Os, 312 programs): 185933 -> 185910 B, 10 programs changed (6 smaller, 4
//! larger by 1 to 2 B), no program slower; mandel.nib 185 -> 178 B and
//! -3.96% clocks, mandel.c -2.2%, mandel.bas -1.5%.
//!
//! DEVIATE M6 (no number): an instruction that reads or writes any physical
//! register stays, where `FindSuccToSinkTo` lets a physical def marked dead
//! through (and a use of a constant register). Effect: none seen, 0 sinks
//! missed over d_faces and the 820-object corpus at -O2 (counted: no
//! instruction that passes `safe_to_move` has a physical write and no read).
//! The flags are the pin's rule in full: a def moves only if dead (read by no
//! later instruction of its block and no successor), and not into a block that
//! reads them (`SinkInstruction`'s zombie check).
//! Exit: honor a dead implicit def when LIR marks one.
//!
//! DEVIATE M7 (no number): an instruction whose pinned operands are all
//! sources (the count of `shld`/`shl r, cl`) stays, as the pin's `$cl = COPY`
//! makes it read a physical register; ours names the pin only in the register
//! class, so `touches_machine_state` asks the classes. One with a pinned result
//! (a divide: `ax`, `dx`) still sinks: the pin's does not, but holding a divide
//! in its block grew switch.c 16 B at m16 -Os. Exit: none.
//!
//! DEVIATE M3 (#2903): no `PerformSinkAndFold`, no sinking into cycles
//! (`SinkInstsIntoCycle`, off by default), no debug-value handling (an
//! instruction has none).
//!
//! # Example
//! `b1: v = mov 0; cmp x, 0; je b4` and `b4: r = phi [b1: v] [b2: w]` leave
//! `v = mov 0` in a block on the edge b1 to b4: the path through b2 no longer
//! runs it.
//!
//! # Runs
//! A machine phase before `phielim` (`flow::machine_with`), in every machine
//! at -O1 and above.

use std::sync::Arc;

use llrm_mir::blockfrequency::BranchProbability;

use crate::backend::liveness;
use crate::liveness::blockfrequency::{Graph, MachineBlockFrequencyInfo, MachineBranchProbabilityInfo};
use crate::liveness::cfg::{Cfg, DomTree};
use crate::model::ir::{Imm, Loc, Operation};
use crate::model::lir::{Block, Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet, IndexMap};

/// `SplitEdgeProbabilityThreshold`
/// ([MachineSink.cpp#L74](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L74)):
/// an edge taken this much of the time or less is worth breaking for a cheap
/// instruction.
/// `SplitEdgeProbabilityThreshold`, 40 percent
/// ([MachineSink.cpp#L82](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L82)).
// 0.4 * 2^31, rounded
const SPLIT_EDGE_PROBABILITY_THRESHOLD: BranchProbability = BranchProbability::get_raw(858_993_459);
/// `SinkLoadInstsPerBlockThreshold` and `SinkLoadBlocksThreshold`
/// ([MachineSink.cpp#L91](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L91)).
const SINK_LOAD_INSTS_PER_BLOCK_THRESHOLD: usize = 2000;
const SINK_LOAD_BLOCKS_THRESHOLD: usize = 20;

pub struct MachineSink {
    /// Which operands the encoding pins to a register (`$cl = COPY` in the
    /// pin).
    pub classes: std::rc::Rc<crate::backend::classes::RegisterClasses>,
    /// A build for size: successors are tried by loop depth alone
    /// (`shouldOptimizeForSize` in `GetAllSortedSuccessors`).
    pub size: bool,
    /// The registers a block may hold at once (`getRegPressureSetLimit`; the
    /// number of allocatable registers, not the pin's per-set limits).
    pub limit: usize,
}

impl LIRTransform for MachineSink {
    fn class_name(&self) -> &'static str {
        "MachineSink"
    }

    fn name(&self) -> &str {
        "machinesink"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(sunk_within(&body, self.size, self.limit, Some(&self.classes)))
    }
}

/// How an instruction that is not a store, a call or a barrier may move:
/// `MachineInstr::isSafeToMove`. A store seen sets `saw_store`; a load is
/// movable only while none has been seen.
fn safe_to_move(
    one: &Insn,
    invariant: bool,
    saw_store: &mut bool,
) -> bool {
    let Some(what) = &one.what else { return false };
    let writes_memory = what.dests.iter().any(|place| matches!(place, Loc::Mem(_)));
    // `lea` names a cell and reads none: the pin's LEA32r is not `mayLoad`.
    // Counted as a load it stayed above every branch (x_collatz: `lea
    // edi,[ecx+ecx*2]` run on each trip, the pin's sunk into the odd arm).
    let reads_memory = what.op != Operation::Address && what.sources.iter().any(|place| matches!(place, Loc::Mem(_)));
    let effects = writes_memory
        || one.volatile()
        || one.call.is_some()
        || matches!(
            what.op,
            Operation::Call
                | Operation::Branch
                | Operation::Jump
                | Operation::Return
                | Operation::Barrier
                | Operation::Escape
                | Operation::Fill
                | Operation::Copy
                | Operation::Nothing
        );
    if effects {
        // A terminator is not a store (`isSafeToMove` sets `SawStore` for a
        // store or a call).
        *saw_store |= may_store(one);
        return false;
    }
    // `isSafeToMove`: a load that is not invariant waits for no store to pass.
    !(reads_memory && *saw_store && !invariant && !one.invariant())
        && one.group.is_none()
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.spread.is_empty()
        && !one.defines.is_empty()
}

/// Whether `lane` is a bit of EFLAGS: the other lanes of `RegId::None` are
/// x87 state, which `fld` writes without touching the flags.
fn is_flag(lane: crate::backend::lanes::Lane) -> bool {
    lane.0 == llrm_lir::registers::RegId::None && lane.1 < 6
}

/// `MBPI->getEdgeProbability(From, To)` of every edge of `body`, by block
/// index: the sum over the successor positions that lead to `To`.
fn edge_probabilities(
    body: &LirBody,
    found: &MachineBranchProbabilityInfo<'_>,
) -> HashMap<(usize, usize), BranchProbability> {
    let position: HashMap<i64, usize> =
        body.blocks.iter().enumerate().map(|(number, block)| (block.at, number)).collect();
    let mut all = HashMap::default();
    for (from, block) in body.blocks.iter().enumerate() {
        let mut seen: Vec<usize> = Vec::new();
        for to in block.succ.iter().filter_map(|at| position.get(at).copied()) {
            if !seen.contains(&to) {
                let odds = found.get_edge_probability(from, seen.len());
                seen.push(to);
                all.insert((from, to), odds);
            }
        }
    }
    all
}

/// `mayStore() || isCall() || hasOrderedMemoryRef()`: what `hasStoreBetween`
/// counts. No alias analysis reaches here, so any of these may alias a load
/// (DEVIATE M7).
pub(crate) fn may_store(one: &Insn) -> bool {
    let Some(what) = &one.what else { return true };
    one.volatile()
        || one.call.is_some()
        || what.dests.iter().any(|place| matches!(place, Loc::Mem(_)))
        || matches!(
            what.op,
            Operation::Call | Operation::Barrier | Operation::Escape | Operation::Fill | Operation::Copy
        )
}

/// `TII->isAsCheapAsAMove`: a copy or a move of a constant.
pub(crate) fn as_cheap_as_a_move(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| what.op == Operation::Move
                && matches!(
                    (what.dests.as_slice(), what.sources.as_slice()),
                    ([Loc::Held(_)], [Loc::Held(_) | Loc::Imm(Imm { address: None, .. })])
                ),
        )
}

thread_local! {
    /// The steps taken to keep what reads a value naming the instruction that
    /// moved: one for each move. Counted, for the test that fixes it linear.
    static IDENTITY_STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Where a value is read.
#[derive(Clone, Copy)]
enum Use {
    /// By the instruction with this identity.
    Insn(u64),
    /// By a phi of `block`, from the predecessor `from`.
    Phi { block: usize, from: i64 },
}

/// Where each value is read, in the body's order: one array cut by value,
/// found by counting (`MachineRegisterInfo`'s use lists, without a list for
/// each value).
struct UseTable {
    /// `starts[v]..starts[v + 1]` of `items` are the reads of value `v`.
    starts: Vec<u32>,
    items: Vec<Use>,
}

impl UseTable {
    fn of(body: &LirBody) -> Self {
        let mut starts: Vec<u32> = Vec::new();
        let mut count = |value: u32| {
            if value as usize + 2 > starts.len() {
                starts.resize(value as usize + 2, 0);
            }
            starts[value as usize + 1] += 1;
        };
        for block in body.blocks.iter() {
            for one in block.insns.iter() {
                one.uses.iter().for_each(|&value| count(value));
            }
            for phi in block.phis.iter() {
                phi.incoming.iter().for_each(|&(_, value)| count(value));
            }
        }
        for at in 1..starts.len() {
            starts[at] += starts[at - 1];
        }
        let mut next = starts.clone();
        let mut items = vec![Use::Insn(0); starts.last().copied().unwrap_or(0) as usize];
        let mut put = |value: u32, used: Use| {
            items[next[value as usize] as usize] = used;
            next[value as usize] += 1;
        };
        for (number, block) in body.blocks.iter().enumerate() {
            for one in block.insns.iter() {
                one.uses.iter().for_each(|&value| put(value, Use::Insn(one.id())));
            }
            for phi in block.phis.iter() {
                phi.incoming.iter().for_each(|&(from, value)| put(value, Use::Phi { block: number, from }));
            }
        }
        Self { starts, items }
    }

    /// The reads of `value`, none when it has none.
    fn get(
        &self,
        value: &u32,
    ) -> Option<&[Use]> {
        let at = *value as usize;
        let (from, to) = (*self.starts.get(at)? as usize, *self.starts.get(at + 1)? as usize);
        (from < to).then(|| &self.items[from..to])
    }
}

/// One walk over the body: the blocks as lists of instructions, and what
/// `MachineRegisterInfo`, the dominator trees and the cycle info answer.
struct Sinker {
    size: bool,
    /// The values loads of an incoming stack argument define: the pin's
    /// immutable fixed objects, which `isDereferenceableInvariantLoad` takes
    /// and which are a frame index, not a register read.
    arguments: std::collections::BTreeSet<u32>,
    insns: Vec<Vec<Arc<Insn>>>,
    at: Vec<i64>,
    position: IndexMap<i64, usize>,
    cfg: Cfg,
    dom: DomTree,
    post: DomTree,
    /// The block of each instruction, by the identity it had when the walk
    /// began (`canonical`): a moved instruction is a copy with a new identity,
    /// and what reads its values still names the old one.
    owner: HashMap<u64, usize>,
    /// The first identity of an instruction that was moved, by its present one.
    canon: HashMap<u64, u64>,
    uses: UseTable,
    depth: Vec<u32>,
    /// The innermost loop of each block, as its header.
    cycle: Vec<Option<usize>>,
    irreducible: Vec<bool>,
    /// `MBFI->getBlockFreq(B).getFrequency()` of each block.
    frequency: Vec<u64>,
    /// `MBPI->getEdgeProbability(From, To)` of each edge.
    probability: HashMap<(usize, usize), BranchProbability>,
    /// `CEBCandidates` and `CEMergeCandidates`.
    considered: HashSet<(usize, usize)>,
    /// `GetAllSortedSuccessors` of each block, made once: it asks nothing of
    /// the instruction.
    sorted: Vec<Vec<usize>>,
    merged: HashMap<(u32, usize), usize>,
    /// `HasStoreCache`.
    has_store_cache: HashMap<(usize, usize), bool>,
    /// `ToSplit`.
    to_split: Vec<(usize, usize)>,
    changed: bool,
    regs: llrm_lir::registers::Regs,
    bits: u32,
    /// Whether the flags are read before they are written from each block's
    /// entry on (live in, as `isLiveIn` says of EFLAGS).
    flags_live_in: Vec<bool>,
    /// `RegClassInfo->getRegPressureSetLimit`.
    limit: usize,
    /// Instructions with an operand the encoding pins to a register (DEVIATE
    /// M7).
    pinned: HashSet<usize>,
    /// The values of the float operations, which the general registers do not
    /// hold; made when the pressure tail first asks.
    stack: Option<HashSet<u32>>,
    /// The block of each value a phi defines.
    phi_blocks: HashMap<u32, usize>,
    /// The block of each other value; made with `stack`, kept up as
    /// instructions move.
    defined: Option<HashMap<u32, usize>>,
    /// `CachedRegisterPressure`.
    pressure: HashMap<usize, usize>,
}

/// The post-dominator tree: the dominator tree of the reversed graph with a
/// virtual exit (the block after the last) that every block without a
/// successor leads to.
fn post_dominators(cfg: &Cfg) -> DomTree {
    let n = cfg.len();
    let mut succs = vec![Vec::new(); n + 1];
    let mut preds = vec![Vec::new(); n + 1];
    for block in 0..n {
        // The reversed edge: a predecessor becomes a successor.
        succs[block] = cfg.preds[block].clone();
        preds[block] = cfg.succs[block].clone();
        if cfg.succs[block].is_empty() {
            succs[n].push(block);
            preds[block].push(n);
        }
    }
    DomTree::of(&Cfg { preds, succs, entry: n })
}

impl Sinker {
    /// The walk of `body`. Where `before` is the previous round's, and it broke
    /// no edge, the graph is the same and its analyses are taken from it (the
    /// pin computes them once and updates them after a split).
    fn new(
        body: &LirBody,
        size: bool,
        before: Option<Sinker>,
    ) -> Self {
        let mut owner = HashMap::with_capacity_and_hasher(
            body.blocks.iter().map(|block| block.insns.len()).sum(),
            Default::default(),
        );
        for (number, block) in body.blocks.iter().enumerate() {
            for one in block.insns.iter() {
                owner.insert(one.id(), number);
            }
        }
        let uses = UseTable::of(body);
        let insns = body.blocks.iter().map(|block| block.insns.to_vec()).collect();
        let phi_blocks: HashMap<u32, usize> = body
            .blocks
            .iter()
            .enumerate()
            .flat_map(|(number, block)| block.phis.iter().map(move |phi| (phi.result, number)))
            .collect();
        if let Some(before) = before {
            return Self {
                insns,
                owner,
                defined: None,
                stack: None,
                phi_blocks,
                pressure: HashMap::default(),
                canon: HashMap::default(),
                uses,
                considered: HashSet::default(),
                merged: HashMap::default(),
                to_split: Vec::new(),
                has_store_cache: HashMap::default(),
                changed: false,
                ..before
            };
        }
        let cfg = Cfg::of(body);
        let dom = DomTree::of(&cfg);
        let post = post_dominators(&cfg);
        let position: IndexMap<i64, usize> =
            body.blocks.iter().enumerate().map(|(number, block)| (block.at, number)).collect();
        // One graph and one cycle info of the body (`MachineCycleInfo`, as the
        // pin's sink asks it: `getCycle`, `isReducible`, `getHeader`)
        // feed the cycles here, the block frequencies and the
        // edge probabilities.
        let graph = std::rc::Rc::new(Graph::of(body));
        let cycles = graph.cycle_info();
        let mut cycle: Vec<Option<usize>> = vec![None; body.blocks.len()];
        let mut hard = vec![false; body.blocks.len()];
        for number in 0..body.blocks.len() {
            let found = cycles.get_cycle(llrm_mir::module::BlockId(number as u32));
            if found.is_valid() {
                let header = cycles.get_header(found).0 as usize;
                cycle[number] = Some(header);
                hard[header] = !cycles.is_reducible(found);
            }
        }
        let depths = crate::analysis::intervals::depths_shared(body);
        let mut sinker = Self {
            size,
            arguments: crate::greedy::remat::argument_loads(body),
            insns,
            at: body.blocks.iter().map(|block| block.at).collect(),
            irreducible: hard,
            depth: body.blocks.iter().map(|block| depths.get(&block.at).copied().unwrap_or(0)).collect(),
            frequency: {
                let found = MachineBlockFrequencyInfo::of_with(&graph, &cycles);
                (0..body.blocks.len()).map(|block| found.get_block_freq(block).get_frequency()).collect()
            },
            probability: edge_probabilities(body, &MachineBranchProbabilityInfo::of_with(std::rc::Rc::clone(&graph))),
            position,
            cfg,
            dom,
            post,
            owner,
            canon: HashMap::default(),
            uses,
            cycle,
            considered: HashSet::default(),
            sorted: Vec::new(),
            merged: HashMap::default(),
            to_split: Vec::new(),
            has_store_cache: HashMap::default(),
            changed: false,
            regs: body.regs(),
            bits: body.bits,
            flags_live_in: Vec::new(),
            limit: usize::MAX,
            pinned: HashSet::default(),
            stack: None,
            phi_blocks,
            defined: None,
            pressure: HashMap::default(),
        };
        sinker.flags_live_in = sinker.solve_flags();
        sinker.sort_successors();
        sinker
    }

    /// The block a use reads its value in: a phi reads it at the end of the
    /// predecessor it names.
    /// The identity the walk began with of the instruction now called `id`.
    fn canonical(
        &self,
        id: u64,
    ) -> u64 {
        self.canon.get(&id).copied().unwrap_or(id)
    }

    fn use_block(
        &self,
        used: Use,
    ) -> usize {
        match used {
            Use::Insn(id) => self.owner[&id],
            Use::Phi { from, .. } => self.position[&from],
        }
    }

    /// `AllUsesDominatedByBlock`. `break_phi_edge` is set when every use is a
    /// phi of `block` reading from `def_block`; `local_use` when a use is in
    /// `def_block` itself.
    fn all_uses_dominated_by(
        &self,
        value: u32,
        block: usize,
        def_block: usize,
        break_phi_edge: &mut bool,
        local_use: &mut bool,
    ) -> bool {
        let Some(uses) = self.uses.get(&value) else { return true };
        if uses.iter().all(|used| {
            matches!(
                *used,
                Use::Phi { block: user, from } if user == block && self.position[&from] == def_block
            )
        }) {
            *break_phi_edge = true;
            return true;
        }
        for &used in uses {
            let in_block = self.use_block(used);
            if matches!(used, Use::Insn(_)) && in_block == def_block {
                *local_use = true;
                return false;
            }
            if !self.dom.dominates(block, in_block) {
                return false;
            }
        }
        true
    }

    /// The physical-register cases of `FindSuccToSinkTo` for the instruction at
    /// `index` of `block`: it stays if it reads a physical register (the flags
    /// included), or defines one that is not dead. A flag it sets is dead when
    /// no later instruction of its block reads it before another sets it and
    /// no successor reads it (`flags_live_in`); any other physical def counts
    /// as live (DEVIATE M6). An unknown effect reads and writes them all.
    fn touches_machine_state(
        &self,
        block: usize,
        index: usize,
    ) -> bool {
        let flags = |lanes: &crate::backend::lanes::Lanes| lanes.iter().any(|&lane| is_flag(lane));
        let physical = |lanes: &crate::backend::lanes::Lanes| {
            lanes.iter().any(|&(register, _)| register != llrm_lir::registers::RegId::None)
        };
        let one = &self.insns[block][index];
        if self.pinned.contains(&crate::backend::peephole::id(one)) {
            return true;
        }
        let Some(effect) = liveness::effect(self.regs, self.bits, one) else { return true };
        // An incoming argument's cell is a fixed stack object in the pin, a
        // frame index with no register: the frame register the address is
        // spelled through is not a read of one.
        let fixed_stack = one.defines.first().is_some_and(|value| self.arguments.contains(value));
        let reads = effect.all_reads();
        let physical_reads = reads
            .iter()
            .any(
                |&(register, _)| register != llrm_lir::registers::RegId::None
                    && !(fixed_stack && (self.regs.is_frame(register) || self.regs.is_stack_segment(register))),
            );
        if physical_reads || physical(&effect.writes) || flags(&reads) {
            return true;
        }
        if !flags(&effect.writes) {
            return false;
        }
        for later in &self.insns[block][index + 1..] {
            let Some(effect) = liveness::effect(self.regs, self.bits, later) else { return true };
            if flags(&effect.all_reads()) {
                return true;
            }
            if flags(&effect.writes) {
                return false;
            }
        }
        // Not touched again here: dead unless a successor reads them.
        self.cfg.succs[block].iter().any(|&succ| self.flags_live_in[succ])
    }

    /// Whether `one` writes the flags.
    fn writes_flags(
        &self,
        one: &Insn,
    ) -> bool {
        liveness::effect(self.regs, self.bits, one).is_none_or(|effect| effect.writes.iter().any(|&lane| is_flag(lane)))
    }

    /// The flags live at each block's entry: read before written there, or
    /// untouched there and live at a successor's.
    fn solve_flags(&self) -> Vec<bool> {
        let n = self.cfg.len();
        let flags = |lanes: &crate::backend::lanes::Lanes| lanes.iter().any(|&lane| is_flag(lane));
        // What each block does first: reads the flags (live in), writes them
        // (not), or neither (as its successors). An unknown effect reads them.
        let first: Vec<Option<bool>> = (0..n)
            .map(|block| {
                self.insns[block]
                    .iter()
                    .find_map(
                        |one| match liveness::effect(self.regs, self.bits, one) {
                            None => Some(true),
                            Some(effect) => {
                                let reads = flags(&effect.all_reads());
                                (reads || flags(&effect.writes)).then_some(reads)
                            }
                        },
                    )
            })
            .collect();
        let mut live: Vec<bool> = first.iter().map(|first| *first == Some(true)).collect();
        let mut work: Vec<usize> = (0..n).filter(|&block| live[block]).collect();
        while let Some(block) = work.pop() {
            for &pred in &self.cfg.preds[block] {
                if !live[pred] && first[pred].is_none() {
                    live[pred] = true;
                    work.push(pred);
                }
            }
        }
        live
    }

    /// `GetAllSortedSuccessors`: the successors of `block`, and the blocks it
    /// immediately dominates that are not successors, the least often run
    /// first.
    fn sorted_successors(
        &self,
        block: usize,
    ) -> &[usize] {
        &self.sorted[block]
    }

    /// `sorted_successors` of every block: the dominator tree's children are
    /// gathered in one pass, not asked of each block among all of them.
    fn sort_successors(&mut self) {
        let n = self.cfg.len();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
        for child in 0..n {
            if let Some(parent) = self.dom.idom(child) {
                if parent != child {
                    children[parent].push(child);
                }
            }
        }
        let busy = |one: usize| self.frequency[one];
        let sorted = (0..n)
            .map(|block| {
                let mut all = self.cfg.succs[block].clone();
                for &child in &children[block] {
                    if !all.contains(&child) {
                        all.push(child);
                    }
                }
                let unknown = all.iter().all(|&one| busy(one) == 0);
                all.sort_by(|&left, &right| {
                    if self.size || unknown {
                        self.depth[left].cmp(&self.depth[right])
                    } else {
                        busy(left).cmp(&busy(right))
                    }
                });
                all
            })
            .collect();
        self.sorted = sorted;
    }

    /// `FindSuccToSinkTo`.
    fn find_succ_to_sink_to(
        &mut self,
        one: &Insn,
        block: usize,
        break_phi_edge: &mut bool,
    ) -> Option<usize> {
        let mut sink_to: Option<usize> = None;
        for &value in &one.defines {
            if let Some(chosen) = sink_to {
                // A previous value picked a block: this one must go there too.
                let mut local = false;
                if !self.all_uses_dominated_by(value, chosen, block, break_phi_edge, &mut local) {
                    return None;
                }
                continue;
            }
            for &succ in self.sorted_successors(block) {
                let mut local = false;
                if self.all_uses_dominated_by(value, succ, block, break_phi_edge, &mut local) {
                    sink_to = Some(succ);
                    break;
                }
                if local {
                    // Used in its own block: never safe to move.
                    return None;
                }
            }
            let chosen = sink_to?;
            if !self.profitable_to_sink_to(value, one, block, chosen) {
                return None;
            }
        }
        sink_to.filter(|&chosen| chosen != block)
    }

    /// `isProfitableToSinkTo`.
    fn profitable_to_sink_to(
        &mut self,
        value: u32,
        one: &Insn,
        block: usize,
        sink_to: usize,
    ) -> bool {
        if block == sink_to {
            return false;
        }
        // Profitable if the successor does not post-dominate the block.
        if !self.post.dominates(sink_to, block) {
            return true;
        }
        // From a deeper cycle to a shallower one, even if it post-dominates.
        if self.depth[block] > self.depth[sink_to] {
            return true;
        }
        // Only a phi reads it in the post-dominating block.
        let non_phi_use = self
            .uses
            .get(&value)
            .into_iter()
            .flatten()
            .any(|&used| matches!(used, Use::Insn(_)) && self.use_block(used) == sink_to);
        if !non_phi_use {
            return true;
        }
        // It may be profitable if it can sink further next round.
        let mut break_phi_edge = false;
        if let Some(further) = self.find_succ_to_sink_to(one, sink_to, &mut break_phi_edge) {
            return self.profitable_to_sink_to(value, one, sink_to, further);
        }
        // Outside a cycle, sinking to a post-dominator shortens nothing.
        let Some(cycle) = self.cycle[block] else { return false };
        // Inside one it can, if no operand it brings in lives beyond the
        // block's pressure limit.
        for &defined in &one.defines {
            let (mut phi_edge, mut local) = (false, false);
            if !self.all_uses_dominated_by(defined, sink_to, block, &mut phi_edge, &mut local) {
                return false;
            }
        }
        self.ready_pressure();
        for &used in &one.uses {
            let by_phi = self.phi_blocks.contains_key(&used);
            let Some(home) = self
                .phi_blocks
                .get(&used)
                .or_else(|| self.defined.as_ref().and_then(|found| found.get(&used)))
                .copied()
            else {
                continue;
            };
            // Defined outside the cycle, or by a phi of its header: no live
            // range of the cycle is longer for it.
            if self.cycle[home] != Some(cycle) || (by_phi && home == cycle && !self.irreducible[cycle]) {
                continue;
            }
            if !self.stack_has(used) && 1 + self.block_pressure(sink_to) >= self.limit {
                return false;
            }
        }
        true
    }

    /// The float values and the home of each value, once.
    fn ready_pressure(&mut self) {
        if self.stack.is_some() {
            return;
        }
        self.stack = Some(crate::backend::machinesched::x87_values(self.insns.iter().flatten()).into_iter().collect());
        self.defined = Some(
            self.insns
                .iter()
                .enumerate()
                .flat_map(|(number, block)| {
                    block.iter().flat_map(move |one| one.defines.iter().map(move |&value| (value, number)))
                })
                .collect(),
        );
    }

    fn stack_has(
        &self,
        value: u32,
    ) -> bool {
        self.stack.as_ref().is_some_and(|stack| stack.contains(&value))
    }

    /// `getBBRegisterPressure`: the most values read in `block` that are live
    /// at once there, counted bottom-up from nothing live at its end, as the
    /// pin's tracker is started without live-outs.
    fn block_pressure(
        &mut self,
        block: usize,
    ) -> usize {
        if let Some(&known) = self.pressure.get(&block) {
            return known;
        }
        let mut live: HashSet<u32> = HashSet::default();
        let mut most = 0;
        for one in self.insns[block].iter().rev() {
            // A value defined and never read still takes a register there.
            let dead = one.defines.iter().filter(|value| !self.stack_has(**value) && !live.contains(value)).count();
            most = most.max(live.len() + dead);
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied().filter(|value| !self.stack_has(*value)));
            most = most.max(live.len());
        }
        self.pressure.insert(block, most);
        most
    }

    /// `isLegalToBreakCriticalEdge`.
    fn legal_to_break_critical_edge(
        &self,
        from: usize,
        to: usize,
        break_phi_edge: bool,
    ) -> bool {
        // Not a back edge, nor an edge into a cycle's header.
        if from == to || !self.cfg.succs[from].contains(&to) {
            return false;
        }
        let (from_cycle, to_cycle) = (self.cycle[from], self.cycle[to]);
        if from_cycle == to_cycle && from_cycle.is_some_and(|header| self.irreducible[header] || header == to) {
            return false;
        }
        // The new block must dominate the uses: every other predecessor of
        // `to` must be dominated by `to`. Not needed where phis alone read it.
        break_phi_edge || self.cfg.preds[to].iter().all(|&pred| pred == from || self.dom.dominates(to, pred))
    }

    /// `isWorthBreakingCriticalEdge`: the second candidate edge for the same
    /// value is returned in `deferred`.
    fn worth_breaking_critical_edge(
        &mut self,
        one: &Insn,
        from: usize,
        to: usize,
        deferred: &mut Option<usize>,
    ) -> bool {
        // A block on an edge costs a jump the size build does not get back,
        // unless the instruction runs in a loop: there the other path stops
        // paying for it every trip.
        if self.size && self.cycle[from].is_none() {
            return false;
        }
        // An edge already considered this walk: sink several cheap
        // instructions into one block.
        if !self.considered.insert((from, to)) {
            return true;
        }
        if !as_cheap_as_a_move(one) {
            return true;
        }
        for &value in &one.defines {
            // The value two edges to one block both want is worth the block.
            if let Some(&earlier) = self.merged.get(&(value, to)) {
                *deferred = Some(earlier);
                return true;
            }
            self.merged.insert((value, to), from);
        }
        if self.cfg.succs[from].contains(&to)
            && self.probability.get(&(from, to)).is_some_and(|&odds| odds <= SPLIT_EDGE_PROBABILITY_THRESHOLD)
        {
            return true;
        }
        // Cheap, but breaking the edge may let its operands' definitions go
        // too: when this is their only user and they are in this block.
        one.uses
            .iter()
            .any(
                |operand| {
                    let users = self.uses.get(operand).map_or(0, <[Use]>::len);
                    users == 1 && self.defined_in(*operand) == Some(self.owner[&self.canonical(one.id())])
                },
            )
    }

    /// The block that defines `value`.
    fn defined_in(
        &self,
        value: u32,
    ) -> Option<usize> {
        self.insns.iter().position(|block| block.iter().any(|one| one.defines.contains(&value)))
    }

    /// `PostponeSplitCriticalEdge`.
    fn postpone_split_critical_edge(
        &mut self,
        one: &Insn,
        from: usize,
        to: usize,
        break_phi_edge: bool,
    ) -> bool {
        let mut deferred = None;
        if !self.worth_breaking_critical_edge(one, from, to, &mut deferred) {
            return false;
        }
        let other_ok = deferred.is_none_or(|other| {
            self.to_split.contains(&(other, to)) || self.legal_to_break_critical_edge(other, to, break_phi_edge)
        });
        if other_ok && self.legal_to_break_critical_edge(from, to, break_phi_edge) {
            self.to_split.push((from, to));
            if let Some(other) = deferred {
                self.to_split.push((other, to));
            }
            return true;
        }
        false
    }

    /// `hasStoreBetween`
    /// ([#L1660](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L1660)):
    /// whether a store, call or ordered access may run between `from` and
    /// `to`, which must be in a straight line. Past the thresholds it says yes.
    fn has_store_between(
        &mut self,
        from: usize,
        to: usize,
    ) -> bool {
        if !self.dom.dominates(from, to) || !self.post.dominates(to, from) {
            return true;
        }
        if let Some(&known) = self.has_store_cache.get(&(from, to)) {
            return known;
        }
        let mut handled = HashSet::default();
        let mut dominated: Vec<usize> = Vec::new();
        let mut stack = vec![from];
        let mut saw_store = false;
        while let Some(block) = stack.pop() {
            if !handled.insert(block) {
                continue;
            }
            stack.extend(self.cfg.succs[block].iter().rev().copied());
            if block == to || block == from || !self.post.dominates(to, block) {
                continue;
            }
            dominated.push(block);
            let too_big = self.insns[block].len() > SINK_LOAD_INSTS_PER_BLOCK_THRESHOLD
                || dominated.len() > SINK_LOAD_BLOCKS_THRESHOLD;
            if too_big || self.insns[block].iter().any(|one| may_store(one)) {
                // What sits above or below the block in the straight line
                // shares the answer.
                for &other in &dominated {
                    if other != block && self.dom.dominates(other, block) {
                        self.has_store_cache.insert((other, to), true);
                    } else if other != block && self.dom.dominates(block, other) {
                        self.has_store_cache.insert((from, other), true);
                    }
                }
                saw_store = true;
                break;
            }
        }
        self.has_store_cache.insert((from, to), saw_store);
        saw_store
    }

    /// `SinkInstruction`: the instruction at `index` of `block`, moved into a
    /// successor or its edge marked for splitting. Whether it moved.
    fn sink_instruction(
        &mut self,
        block: usize,
        index: usize,
        saw_store: &mut bool,
    ) -> bool {
        let one = Arc::clone(&self.insns[block][index]);
        let argument = one.defines.first().is_some_and(|value| self.arguments.contains(value));
        if !safe_to_move(&one, argument, saw_store) {
            return false;
        }
        if self.touches_machine_state(block, index) {
            return false;
        }
        let mut break_phi_edge = false;
        let Some(sink_to) = self.find_succ_to_sink_to(&one, block, &mut break_phi_edge) else { return false };
        // A dead flags def that is live into the block it moves to would turn
        // into a zombie def there (`SinkInstruction`).
        if self.flags_live_in[sink_to] && self.writes_flags(&one) {
            return false;
        }
        // A successor with other predecessors: sink along the edge, or break
        // it.
        if self.cfg.preds[sink_to].len() > 1 {
            let loads =
                one.what.as_ref().is_some_and(|what| what.sources.iter().any(|place| matches!(place, Loc::Mem(_))));
            let enters_cycle = self.cycle[sink_to].is_some_and(|header| self.irreducible[header] || header == sink_to);
            // A load goes along a critical edge only past no store.
            let store_between = loads && self.has_store_between(block, sink_to);
            if store_between || !self.dom.dominates(block, sink_to) || enters_cycle {
                self.postpone_split_critical_edge(&one, block, sink_to, break_phi_edge);
                return false;
            }
        }
        if break_phi_edge {
            self.postpone_split_critical_edge(&one, block, sink_to, break_phi_edge);
            return false;
        }
        // After the phis, which are not instructions here: the top.
        // Instructions of one block stay in order by their `at`.
        let mut moved = (*self.insns[block].remove(index)).clone();
        if let Some(first) = self.insns[sink_to].first() {
            moved.at = first.at;
        }
        let moved = Arc::new(moved);
        // Identity follows the instruction: its values' uses name the first
        // identity, and `moved` is `one` in its new block.
        let first = self.canonical(one.id());
        self.canon.insert(moved.id(), first);
        IDENTITY_STEPS.with(|steps| steps.set(steps.get() + 1));
        self.owner.insert(first, sink_to);
        if let Some(defined) = &mut self.defined {
            defined.extend(one.defines.iter().map(|&value| (value, sink_to)));
        }
        self.insns[sink_to].insert(0, moved);
        self.changed = true;
        true
    }

    /// `ProcessBlock`: the block bottom-up.
    fn process_block(
        &mut self,
        block: usize,
    ) {
        if self.cfg.succs[block].len() <= 1 || self.insns[block].is_empty() || !self.dom.is_reachable(block) {
            return;
        }
        let mut saw_store = false;
        for index in (0..self.insns[block].len()).rev() {
            self.sink_instruction(block, index, &mut saw_store);
        }
    }
}

/// `body` with what `MachineSinking::run` moves moved, the registers held at
/// once unbounded.
pub fn sunk(
    body: &LirBody,
    size: bool,
) -> LirBody {
    sunk_within(body, size, usize::MAX, None)
}

/// `body` with what `MachineSinking::run` moves moved, no block to hold more
/// than `limit` values.
pub fn sunk_within(
    body: &LirBody,
    size: bool,
    limit: usize,
    classes: Option<&crate::backend::classes::RegisterClasses>,
) -> LirBody {
    let mut body = body.clone();
    // Each round walks the blocks, then breaks the edges it marked; it ends
    // when nothing moved and nothing was marked.
    let mut before: Option<Sinker> = None;
    loop {
        let mut sinker = Sinker::new(&body, size, before.take());
        sinker.limit = limit;
        if let Some(classes) = classes {
            sinker.pinned = body
                .blocks
                .iter()
                .flat_map(|block| block.insns.iter())
                .filter(|one| {
                    one.what
                        .as_ref()
                        .is_some_and(
                            |what| {
                                let pins = classes.requirements(what);
                                !pins.is_empty() && pins.keys().all(|place| place.side != "dest")
                            },
                        )
                })
                .map(crate::backend::peephole::id)
                .collect();
        }
        for block in 0..sinker.cfg.len() {
            sinker.process_block(block);
        }
        let marked = std::mem::take(&mut sinker.to_split);
        if !sinker.changed && marked.is_empty() {
            return body;
        }
        let blocks: Vec<Block> = body
            .blocks
            .iter()
            .enumerate()
            .map(|(number, block)| block.with_insns(sinker.insns[number].clone()))
            .collect();
        body = body.with_blocks(blocks);
        let mut broke = false;
        let mut seen = HashSet::default();
        for (from, to) in marked {
            if !seen.insert((from, to)) {
                continue;
            }
            if let Some((split, _)) = crate::backend::edges::split(&body, sinker.at[from], sinker.at[to]) {
                body = split;
                broke = true;
            }
        }
        if !broke && !sinker.changed {
            return body;
        }
        if !broke {
            before = Some(sinker);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Held, Semantics};
    use crate::model::lir::{Insn, LirBlock};
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    /// `if (n == 0)` on a long in a register pair tests `or v9, v2` and
    /// branches on its flags; the value v9 is read by nothing, so the `or` was
    /// sunk into a successor and the `je` read the flags of whatever ran last
    /// (tests/run/c/register_upper_half.c -mabi=watcom printed 1382385 for 9).
    #[test]
    fn test_an_instruction_whose_flags_the_branch_reads_stays_before_it() {
        let entry = LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(
                0,
                vec![
                    insn(0, Operation::Move, "mov", vec![held(8)], vec![held(1)], vec![8], vec![1]),
                    insn(1, Operation::Binary, "or", vec![held(9)], vec![held(8), held(2)], vec![9], vec![8, 2]),
                    insn(2, Operation::Branch, "je", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let rest = |at: i64| {
            LirBlock::new(
                at,
                vec![
                    insn(
                        at,
                        Operation::Move,
                        "mov",
                        vec![held(20 + at as u32)],
                        vec![held(1)],
                        vec![20 + at as u32],
                        vec![1],
                    ),
                    insn(at, Operation::Return, "ret", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let mut body = LirBody::new(
            "flags",
            0,
            vec![entry.into(), rest(1).into(), rest(2).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let names: Vec<_> = sunk(&body, false).blocks[0]
            .insns
            .iter()
            .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
            .collect();
        assert_eq!(names, ["mov", "or", "je"], "the flag-setting or stays in its block");
    }

    /// An instruction sunk into a block that itself sinks it on is one
    /// instruction: what defines the values it reads sees it in its last block
    /// and follows it there. Sinking renamed every use of every value in the
    /// body on each move; the identity the uses name is now kept instead.
    #[test]
    fn test_what_a_sunk_instruction_reads_follows_it_to_its_last_block() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let ret = |at: i64| insn(at, Operation::Return, "ret", vec![], vec![], vec![], vec![]);
        let mov = |at: i64, to: u32, from: u32| {
            insn(at, Operation::Move, "mov", vec![held(to)], vec![held(from)], vec![to], vec![from])
        };
        let mut body = LirBody::new(
            "twice",
            0,
            vec![
                block(0, vec![1, 2], vec![mov(0, 1, 5), mov(1, 8, 1), jump(2)]).into(),
                block(1, vec![3, 4], vec![jump(3)]).into(),
                block(2, vec![], vec![ret(4)]).into(),
                block(3, vec![], vec![mov(5, 20, 8), ret(6)]).into(),
                block(4, vec![], vec![ret(7)]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [5].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let after = sunk(&body, false);
        let defines =
            |at: usize| -> Vec<u32> { after.blocks[at].insns.iter().flat_map(|one| one.defines.clone()).collect() };
        assert!(defines(0).is_empty() && defines(1).is_empty(), "both movs left blocks 0 and 1");
        assert!(defines(3).contains(&8) && defines(3).contains(&1), "and both are in block 3, where v8 is read");
    }

    /// Sinking renamed every use of every value in the body on each move, so n
    /// sinks over n instructions took n times n steps (d_faces -O2: 122,898
    /// map steps in 9 bodies). Counted, not timed: the steps are one a move.
    #[test]
    fn test_sinking_n_instructions_takes_a_number_of_steps_linear_in_n() {
        let steps_for = |n: u32| -> (usize, usize) {
            let block = |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock {
                succ: succ.into(),
                ..LirBlock::new(at, insns)
            };
            let jump = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
            let ret = |at: i64| insn(at, Operation::Return, "ret", vec![], vec![], vec![], vec![]);
            let mov = |at: i64, to: u32, from: u32| {
                insn(at, Operation::Move, "mov", vec![held(to)], vec![held(from)], vec![to], vec![from])
            };
            let movs: Vec<_> = (0..n).map(|i| mov(i64::from(i), 100 + i, 5)).collect();
            let reads: Vec<_> = (0..n).map(|i| mov(1000 + i64::from(i), 200 + i, 100 + i)).collect();
            let mut first = movs;
            first.push(jump(500));
            let mut last = reads;
            last.push(ret(2000));
            let mut body = LirBody::new(
                "many",
                0,
                vec![
                    block(0, vec![1, 2], first).into(),
                    block(1, vec![3, 4], vec![jump(501)]).into(),
                    block(2, vec![], vec![ret(502)]).into(),
                    block(3, vec![], last).into(),
                    block(4, vec![], vec![ret(503)]).into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [5].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            IDENTITY_STEPS.with(|steps| steps.set(0));
            let after = sunk(&body, false);
            let moved = after.blocks[3].insns.len() - 1 - n as usize;
            (IDENTITY_STEPS.with(std::cell::Cell::get), moved)
        };
        let (small, _) = steps_for(10);
        let (large, _) = steps_for(40);
        assert!(small > 0, "the instructions sink");
        assert!(large <= 4 * small + 8, "4 times the instructions took {large} steps, 10 took {small}");
    }

    /// The flags a `cmp` sets may be read by a branch in the next block (one
    /// `cmp`, two branches): the def is live out and does not sink either.
    #[test]
    fn test_an_instruction_whose_flags_a_successor_reads_stays() {
        let entry = LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(
                0,
                vec![
                    insn(0, Operation::Binary, "or", vec![held(9)], vec![held(1), held(2)], vec![9], vec![1, 2]),
                    insn(1, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let reader = LirBlock {
            succ: vec![2].into(),
            ..LirBlock::new(1, vec![insn(2, Operation::Branch, "je", vec![], vec![], vec![], vec![])])
        };
        let leaf = LirBlock::new(2, vec![insn(3, Operation::Return, "ret", vec![], vec![], vec![], vec![])]);
        let mut body = LirBody::new(
            "flags_out",
            0,
            vec![entry.into(), reader.into(), leaf.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let names: Vec<_> = sunk(&body, false).blocks[0]
            .insns
            .iter()
            .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
            .collect();
        assert_eq!(names, ["or", "jcxz"], "the or stays: its flags are read in the next block");
    }

    /// bench/mandel.nib -Os m16: the exit phi `[7, 32]` of the inner loop made
    /// its `mov ecx, 32` in the latch, run every trip (+3.9% clocks over the
    /// unrotated loop, +7 B). A build for size broke no edge; the edge that
    /// leaves a loop is broken now, and the constant is made once on it.
    #[test]
    fn test_in_a_size_build_a_constant_of_the_loop_exit_phi_leaves_the_latch() {
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let branch = |at: i64, name: &str| insn(at, Operation::Branch, name, vec![], vec![], vec![], vec![]);
        let cmp = |at: i64| insn(at, Operation::Compare, "cmp", vec![], vec![held(1), imm(32)], vec![], vec![1]);
        let exit = LirBlock {
            phis: vec![crate::model::lir::Phi { result: 6, incoming: vec![(1, 2), (2, 5)] }].into(),
            ..LirBlock::new(3, vec![insn(30, Operation::Return, "ret", vec![], vec![], vec![], vec![])])
        };
        let mut body = LirBody::new(
            "latch",
            0,
            vec![
                block(0, vec![1], vec![insn(0, Operation::Move, "mov", vec![held(2)], vec![imm(0)], vec![2], vec![])])
                    .into(),
                block(1, vec![2, 3], vec![cmp(10), branch(11, "jbe")]).into(),
                block(
                    2,
                    vec![1, 3],
                    vec![
                        insn(20, Operation::Move, "mov", vec![held(5)], vec![imm(32)], vec![5], vec![]),
                        cmp(21),
                        branch(22, "jne"),
                    ],
                )
                .into(),
                exit.into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        body.odds.taken.insert((2, 3), (0.03 * crate::model::lir::BlockOdds::CERTAIN) as u32);
        body.odds.taken.insert((2, 1), (0.97 * crate::model::lir::BlockOdds::CERTAIN) as u32);
        let names = |body: &LirBody, at: i64| -> Vec<String> {
            body.blocks
                .iter()
                .find(|one| one.at == at)
                .expect("the block")
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        let moved = sunk(&body, true);
        assert!(!names(&moved, 2).contains(&"mov".to_owned()), "the constant left the latch: {:?}", names(&moved, 2));
        assert!(moved.blocks.len() == 5, "and went into a block of its own on the exit edge");
    }

    /// x_collatz -Os m32: `x & 1 ? 3x + 1 : x >> 1` in a loop computed the
    /// `shr` above the test and copied it, where the pin sinks it into a block
    /// on the edge it alone takes (inner loop 10 instructions against 4). A
    /// size build broke no edge inside a loop.
    #[test]
    fn test_in_a_size_build_an_instruction_of_a_loop_sinks_onto_its_critical_edge() {
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let phi = |result: u32, incoming: Vec<(i64, u32)>| crate::model::lir::Phi { result, incoming };
        let header = LirBlock {
            phis: vec![phi(2, vec![(0, 1), (3, 5)])].into(),
            ..block(
                1,
                vec![2, 3],
                vec![
                    insn(10, Operation::Binary, "shr", vec![held(3)], vec![held(2), imm(1)], vec![3], vec![2]),
                    insn(11, Operation::Compare, "test", vec![], vec![held(2), imm(1)], vec![], vec![2]),
                    insn(12, Operation::Branch, "je", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let join = LirBlock {
            phis: vec![phi(5, vec![(2, 4), (1, 3)])].into(),
            ..block(
                3,
                vec![1, 4],
                vec![
                    insn(30, Operation::Compare, "cmp", vec![], vec![held(5), imm(1)], vec![], vec![5]),
                    insn(31, Operation::Branch, "jne", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let mut body = LirBody::new(
            "collatz",
            0,
            vec![
                block(0, vec![1], vec![insn(0, Operation::Move, "mov", vec![held(1)], vec![imm(7)], vec![1], vec![])])
                    .into(),
                header.into(),
                block(
                    2,
                    vec![3],
                    vec![insn(20, Operation::Binary, "lea", vec![held(4)], vec![held(2), imm(1)], vec![4], vec![2])],
                )
                .into(),
                join.into(),
                block(4, vec![], vec![insn(40, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let moved = sunk(&body, true);
        let header: Vec<_> = moved.blocks[1]
            .insns
            .iter()
            .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
            .collect();
        assert!(!header.iter().any(|name| &**name == "shr"), "the shr left the header: {header:?}");
        assert_eq!(moved.blocks.len(), 6, "and went into a block of its own on the edge to the join");
    }

    /// `x > 65520 ? x - 65521 : x` in a build for size: the `add` was computed
    /// above the branch and copied in the arm (`lea; cmp; jbe; mov`, x_adler
    /// -Os 13 to 17 instructions a trip), because only an instruction that
    /// read nothing moved. Its operand is read by the phi on the other
    /// edge, so moving it into the arm extends nothing.
    #[test]
    fn test_in_a_size_build_an_instruction_whose_operand_lives_on_moves() {
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let entry = LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(
                0,
                vec![
                    insn(0, Operation::Binary, "add", vec![held(3)], vec![held(1), imm(-65521)], vec![3], vec![1]),
                    insn(1, Operation::Compare, "cmp", vec![], vec![held(1), imm(65520)], vec![], vec![1]),
                    insn(2, Operation::Branch, "jbe", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let arm = LirBlock {
            succ: vec![2].into(),
            ..LirBlock::new(1, vec![insn(3, Operation::Move, "mov", vec![held(5)], vec![held(2)], vec![5], vec![2])])
        };
        let join = LirBlock {
            phis: vec![crate::model::lir::Phi { result: 4, incoming: vec![(1, 3), (0, 1)] }].into(),
            ..LirBlock::new(2, vec![insn(4, Operation::Move, "mov", vec![held(6)], vec![held(4)], vec![6], vec![4])])
        };
        let mut body = LirBody::new(
            "adler",
            0,
            vec![entry.into(), arm.into(), join.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let names = |body: &LirBody, block: usize| -> Vec<String> {
            body.blocks[block]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        // The premise: no block reads the flags the add sets.
        assert_eq!(Sinker::new(&body, true, None).flags_live_in, [false, false, false]);
        let moved = sunk(&body, true);
        assert_eq!(names(&moved, 0), ["cmp", "jbe"], "the add left the block");
        assert_eq!(names(&moved, 1), ["add", "mov"], "and went into the arm");
    }

    /// qcport pl_trace -Os +125 bytes: an `fsub` moved into an arm took its two
    /// operands, loaded in the entry, across the edge, and the stackifier
    /// held them in memory (`fld; fstp` pairs for what were `fsub m` folds).
    /// It moves now (numbered row M4, below).
    #[test]
    fn test_in_a_size_build_an_instruction_whose_operand_dies_in_the_block_moves() {
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let wide = |value: u32| Loc::Held(Held { value, width: 10 });
        let entry = LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(
                0,
                vec![
                    insn(0, Operation::FloatArith, "fsub", vec![wide(3)], vec![wide(7), wide(8)], vec![3], vec![7, 8]),
                    insn(1, Operation::Compare, "cmp", vec![], vec![held(1), imm(65520)], vec![], vec![1]),
                    insn(2, Operation::Branch, "jbe", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let arm = LirBlock {
            succ: vec![2].into(),
            ..LirBlock::new(1, vec![insn(3, Operation::Move, "mov", vec![wide(5)], vec![wide(3)], vec![5], vec![3])])
        };
        let join = LirBlock {
            phis: vec![crate::model::lir::Phi { result: 4, incoming: vec![(1, 3), (0, 7)] }].into(),
            ..LirBlock::new(2, vec![insn(4, Operation::Move, "mov", vec![wide(6)], vec![wide(4)], vec![6], vec![4])])
        };
        let mut body = LirBody::new(
            "trace",
            0,
            vec![entry.into(), arm.into(), join.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 7, 8].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let names = |body: &LirBody, block: usize| -> Vec<String> {
            body.blocks[block]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        assert_eq!(Sinker::new(&body, true, None).flags_live_in, [false, false, false]);
        let moved = sunk(&body, true);
        assert_eq!(names(&moved, 0), ["cmp", "jbe"], "the fsub moved, as the pin has it");
    }

    /// x_collatz ran `lea edi,[ecx+ecx*2]` on every trip of its inner loop,
    /// above the branch whose odd arm alone read it: the sinker took the
    /// `lea`'s address operand for a memory read and refused to move it
    /// past the branch. The pin's sunk it.
    #[test]
    fn test_a_lea_is_not_a_load_and_sinks_past_a_branch() {
        use crate::model::ir::Mem;
        let cell = Mem { base: Some(Held { value: 1, width: 4 }), ..Mem::new(None, 4) };
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let entry = LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(
                0,
                vec![
                    insn(0, Operation::Address, "lea", vec![held(3)], vec![Loc::Mem(cell)], vec![3], vec![1]),
                    insn(1, Operation::Compare, "cmp", vec![], vec![held(2), imm(5)], vec![], vec![2]),
                    insn(2, Operation::Branch, "jbe", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let arm = LirBlock {
            succ: vec![2].into(),
            ..LirBlock::new(1, vec![insn(3, Operation::Move, "mov", vec![held(5)], vec![held(3)], vec![5], vec![3])])
        };
        let join = LirBlock::new(2, vec![insn(4, Operation::Return, "ret", vec![], vec![], vec![], vec![])]);
        let mut body = LirBody::new(
            "collatz",
            0,
            vec![entry.into(), arm.into(), join.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let names = |body: &LirBody, block: usize| -> Vec<String> {
            body.blocks[block]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        let moved = sunk(&body, false);
        assert_eq!(names(&moved, 0), ["cmp", "jbe"], "the lea left the block");
        assert_eq!(names(&moved, 1), ["lea", "mov"], "and went into the arm");
    }

    /// The same, with the operand read again in the arm: the `add` then needs
    /// a copy there (`mov; add` for the `lea` it was: bench/control -Os +2
    /// bytes). The pin sinks it, and so does a build for size (the rule that
    /// kept it was DEVIATE M4: x_collatz -Os +17% clocks).
    #[test]
    fn test_in_a_size_build_an_instruction_whose_operand_is_read_below_moves() {
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let entry = LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(
                0,
                vec![
                    insn(0, Operation::Binary, "add", vec![held(3)], vec![held(1), imm(7)], vec![3], vec![1]),
                    insn(1, Operation::Compare, "cmp", vec![], vec![held(2), imm(0)], vec![], vec![2]),
                    insn(2, Operation::Branch, "je", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let arm = LirBlock {
            succ: vec![2].into(),
            ..LirBlock::new(1, vec![insn(3, Operation::Move, "mov", vec![held(5)], vec![held(1)], vec![5], vec![1])])
        };
        let join = LirBlock {
            phis: vec![crate::model::lir::Phi { result: 4, incoming: vec![(1, 3), (0, 2)] }].into(),
            ..LirBlock::new(2, vec![insn(4, Operation::Move, "mov", vec![held(6)], vec![held(4)], vec![6], vec![4])])
        };
        let mut body = LirBody::new(
            "stays",
            0,
            vec![entry.into(), arm.into(), join.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let names = |body: &LirBody, block: usize| -> Vec<String> {
            body.blocks[block]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        assert_eq!(Sinker::new(&body, true, None).flags_live_in, [false, false, false]);
        let moved = sunk(&body, true);
        assert_eq!(names(&moved, 0), ["cmp", "je"], "the add moved, as the pin has it");
        // Outside a build for size it moves, so the premise is the rule's.
        assert_eq!(names(&sunk(&body, false), 0), ["cmp", "je"], "the premise: it can move");
    }

    /// The operand is read below the arm, in the join (a loop's next trip reads
    /// it as well): the `add` needs a copy in the arm (bench/parity/control
    /// -Os, `lea esi, [ebx+7]` made `mov; add`); it moves all the same, as the
    /// pin has it.
    #[test]
    fn test_in_a_size_build_an_instruction_whose_operand_is_live_out_of_the_arm_moves() {
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let entry = LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(
                0,
                vec![
                    insn(0, Operation::Binary, "add", vec![held(3)], vec![held(1), imm(7)], vec![3], vec![1]),
                    insn(1, Operation::Compare, "cmp", vec![], vec![held(2), imm(0)], vec![], vec![2]),
                    insn(2, Operation::Branch, "je", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let arm = LirBlock {
            succ: vec![2].into(),
            ..LirBlock::new(1, vec![insn(3, Operation::Move, "mov", vec![held(5)], vec![held(2)], vec![5], vec![2])])
        };
        let join = LirBlock {
            phis: vec![crate::model::lir::Phi { result: 4, incoming: vec![(1, 3), (0, 2)] }].into(),
            ..LirBlock::new(2, vec![insn(4, Operation::Move, "mov", vec![held(6)], vec![held(1)], vec![6], vec![1])])
        };
        let mut body = LirBody::new(
            "live_out",
            0,
            vec![entry.into(), arm.into(), join.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let names = |body: &LirBody, block: usize| -> Vec<String> {
            body.blocks[block]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        assert_eq!(names(&sunk(&body, true), 0), ["cmp", "je"], "the add moved, as the pin has it");
        assert_eq!(names(&sunk(&body, false), 0), ["cmp", "je"], "the premise: it can move");
    }

    /// A load in a loop, read only in the exit that the loop's two paths both
    /// reach, was left in the loop whatever lay between (DEVIATE M1: no load
    /// along a critical edge); the pin sinks it when `hasStoreBetween` finds
    /// none. With a store on the way it stays.
    #[test]
    fn test_a_load_goes_along_a_critical_edge_past_no_store() {
        use crate::model::ir::Mem;
        let cell = Mem { base: Some(Held { value: 1, width: 4 }), ..Mem::new(None, 4) };
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let build = |between: Arc<Insn>| {
            // entry -> head; head -> latch | exit; latch -> head | side; side
            // -> exit.
            let mut body = LirBody::new(
                "critical",
                0,
                vec![
                    block(0, vec![1], vec![]).into(),
                    block(
                        1,
                        vec![2, 4],
                        vec![
                            insn(
                                10,
                                Operation::Move,
                                "mov",
                                vec![held(3)],
                                vec![Loc::Mem(cell.clone())],
                                vec![3],
                                vec![1],
                            ),
                            jump(11),
                        ],
                    )
                    .into(),
                    block(2, vec![1, 3], vec![jump(12)]).into(),
                    block(3, vec![4], vec![between]).into(),
                    block(
                        4,
                        vec![],
                        vec![
                            insn(13, Operation::Move, "mov", vec![held(5)], vec![held(3)], vec![5], vec![3]),
                            insn(14, Operation::Return, "ret", vec![], vec![], vec![], vec![]),
                        ],
                    )
                    .into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            sunk(&body, false)
        };
        let nop = insn(15, Operation::Move, "mov", vec![held(6)], vec![held(1)], vec![6], vec![1]);
        let store = insn(15, Operation::Move, "mov", vec![Loc::Mem(cell.clone())], vec![held(1)], vec![], vec![1]);
        let loads = |body: &LirBody, at: usize| body.blocks[at].insns.iter().any(|one| one.defines.contains(&3));
        let clear = build(nop);
        assert!(loads(&clear, 4) && !loads(&clear, 1), "no store between: the load left the loop");
        let blocked = build(store);
        assert!(loads(&blocked, 1) && !loads(&blocked, 4), "a store between: the load stays");
    }

    /// The edge a loop is left by is split-worthy (at most the pin's 40
    /// percent), the edge back into it is not: they come from the body's
    /// odds through `MachineBranchProbabilityInfo`, as the pin asks it, not
    /// from a walk of the sinker's own.
    #[test]
    fn test_a_loop_exit_is_under_the_split_threshold_and_its_back_edge_is_over() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let ret = |at: i64| insn(at, Operation::Return, "ret", vec![], vec![], vec![], vec![]);
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![]).into(),
                block(1, vec![1, 2], vec![jump(1)]).into(),
                block(2, vec![], vec![ret(2)]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        body.odds.taken.insert((1, 2), (0.1 * crate::model::lir::BlockOdds::CERTAIN) as u32);
        body.odds.taken.insert((1, 1), (0.9 * crate::model::lir::BlockOdds::CERTAIN) as u32);
        let odds = edge_probabilities(&body, &MachineBranchProbabilityInfo::of(&body));
        assert!(odds[&(1, 2)] <= SPLIT_EDGE_PROBABILITY_THRESHOLD, "the exit");
        assert!(odds[&(1, 1)] > SPLIT_EDGE_PROBABILITY_THRESHOLD, "the back edge");
    }

    /// The sink asked a graph of the body for its frequencies, another for its
    /// edge probabilities, and found its loops a third and fourth time
    /// (`loops`, `irreducible`): d_faces, 9 rounds, 4 M instructions. One
    /// graph and one cycle info feed all of them. Counted, not timed.
    #[test]
    fn test_a_round_of_the_sink_builds_one_graph_of_the_body() {
        let jump = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let ret = |at: i64| insn(at, Operation::Return, "ret", vec![], vec![], vec![], vec![]);
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![]).into(),
                block(1, vec![1, 2], vec![jump(1)]).into(),
                block(2, vec![], vec![ret(2)]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let before = crate::liveness::blockfrequency::GRAPH_BUILDS.with(std::cell::Cell::get);
        let _ = Sinker::new(&body, false, None);
        let built = crate::liveness::blockfrequency::GRAPH_BUILDS.with(std::cell::Cell::get) - before;
        assert_eq!(built, 1, "graphs built for one round");
    }

    /// x_bubble's inner loop: `j + 1` is computed in the header, read by the
    /// phi and by the compare in the latch, and the latch post-dominates the
    /// header. It stayed in the header (row M2: an instruction inside a
    /// loop never went to a post-dominating block), so the loop carried both
    /// `j` and `j + 1` (`mov ebp, eax` in the latch, a spill in the outer
    /// loop). The pin sinks it unless the target block is full.
    #[test]
    fn test_a_loop_increment_read_in_the_latch_sinks_into_it() {
        use crate::model::ir::{Addr, Mem, Space};
        let imm = |value: i64| Loc::Imm(crate::model::ir::Imm { value, width: 4, address: None });
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64, name: &str| insn(at, Operation::Branch, name, vec![], vec![], vec![], vec![]);
        // The add reads the loop's phi (v10) or the value its header loads
        // (v12).
        let bubble = |reads: u32| {
            let cell = Mem {
                index: Some(Held { value: 10, width: 4 }),
                scale: 4,
                ..Mem::new(Some(Addr::new(Space::Segment, 4)), 4)
            };
            let header = LirBlock {
                phis: vec![crate::model::lir::Phi { result: 10, incoming: vec![(0, 1), (3, 11)] }].into(),
                ..block(
                    1,
                    vec![2, 3],
                    vec![
                        insn(10, Operation::Move, "mov", vec![held(12)], vec![Loc::Mem(cell)], vec![12], vec![10]),
                        insn(
                            11,
                            Operation::Binary,
                            "add",
                            vec![held(11)],
                            vec![held(reads), imm(1)],
                            vec![11],
                            vec![reads],
                        ),
                        insn(12, Operation::Compare, "cmp", vec![], vec![held(12), held(2)], vec![], vec![12, 2]),
                        jump(13, "jle"),
                    ],
                )
            };
            let arm = block(
                2,
                vec![3],
                vec![insn(20, Operation::Move, "mov", vec![held(13)], vec![held(12)], vec![13], vec![12])],
            );
            let latch = block(
                3,
                vec![1, 4],
                vec![
                    insn(30, Operation::Compare, "cmp", vec![], vec![held(11), held(3)], vec![], vec![11, 3]),
                    jump(31, "jne"),
                ],
            );
            let exit = block(4, vec![], vec![insn(40, Operation::Return, "ret", vec![], vec![], vec![], vec![])]);
            let mut body = LirBody::new(
                "bubble",
                0,
                vec![block(0, vec![1], vec![]).into(), header.into(), arm.into(), latch.into(), exit.into()],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1, 2, 3].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m32::REGISTER_INFO));
            body.bits = 32;
            body
        };
        let names = |body: &LirBody, block: usize| -> Vec<String> {
            body.blocks[block]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        let moved = sunk_within(&bubble(10), false, 7, None);
        assert_eq!(names(&moved, 1), ["mov", "cmp", "jle"], "the add left the header");
        assert_eq!(names(&moved, 3), ["add", "cmp", "jne"], "and went into the latch");
        // A value of the loop that the add would keep live to the latch counts
        // against the latch's pressure: room for it, and none.
        assert_eq!(names(&sunk_within(&bubble(12), false, 7, None), 3), ["add", "cmp", "jne"], "room");
        assert_eq!(names(&sunk_within(&bubble(12), false, 3, None), 1), ["mov", "add", "cmp", "jle"], "full");
    }
}
