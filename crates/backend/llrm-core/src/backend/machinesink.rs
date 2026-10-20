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
//! DEVIATE M1 (#2903): a load is sunk only into a successor with no other
//! predecessor, or a block made for the edge. LLVM also sinks it along a
//! critical edge when `hasStoreBetween` finds no store on any path between.
//! Exit: that function translated.
//!
//! DEVIATE M2 (#2903): `isProfitableToSinkTo` ends by asking the register
//! pressure of the target (`registerPressureSetExceedsLimit`) for an
//! instruction inside a cycle; no instruction here sinks on that ground.
//! Exit: the target description's pressure limits.
//!
//! DEVIATE M4 (#2903): in a build for size only an instruction that reads no
//! value moves (a constant). LLVM moves any, and its allocator splits the
//! operands' ranges around the stretch. Rows with every instruction movable
//! at -Os: `bench/jpeg_single` m16 +61 bytes, `jpeg_fixed` m16 +50, `huge_runs`
//! +29 (all 36 -Os objects that differ: 8 larger, +163, 21 smaller, -998).
//! Otherwise (-O1 to -O3) every safe instruction moves; `bench/quicksort`
//! -O2 m16 costs +985 clocks (+0.3%, a reload more per trip). Exit: the
//! greedy allocator with SplitKit as the default allocator (target 2026-11-05).
//!
//! DEVIATE M5 (#2903): in a build for size no edge is broken. The block
//! costs a jump (`bench/recpow` -Os +5 bytes with the edge split; over the
//! 834-object sweep at head 1b78847ae, breaking it at -Os is 37 objects larger,
//! +228 bytes, and 26 smaller, -106 bytes). `bench/hanoi` -Os m32 keeps the
//! `xor` and a copy before its test, and a frame, where the pin sinks the
//! constant (195 bytes against 190 with the edge broken). Exit, decided
//! 2026-10-22 with jumps' block-by-block diff: a placement rule jumps names,
//! or sinking with shrink-wrapping (lir-cleanup-port for the sinking half).
//!
//! DEVIATE M6 (no number): an instruction that reads or writes any physical
//! register stays, where `FindSuccToSinkTo` lets a physical def marked dead
//! through (and a use of a constant register). Effect: none seen, 0 sinks
//! missed over d_faces and the 820-object corpus at -O2 (counted: no
//! instruction that passes `safe_to_move` has a physical write and no read).
//! Exit: honor a dead implicit def when LIR marks one.
//!
//! DEVIATE M3 (#2903): no `PerformSinkAndFold`, no sinking into cycles
//! (`SinkInstsIntoCycle`, off by default), no debug-value handling (an
//! instruction has none).
//!
//! `pressuresink` (a load made where its reader is, within a block) has no
//! pin counterpart: LLVM orders loads by the scheduler. It is deleted when the
//! pre-RA machine scheduler is translated (target 2026-11-20).
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

use crate::analysis::frequency::Frequency;
use crate::analysis::loops;
use crate::backend::liveness;
use crate::liveness::cfg::{Cfg, DomTree};
use crate::model::ir::{Imm, Loc, Operation};
use crate::model::lir::{Block, Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet, IndexMap};

/// `SplitEdgeProbabilityThreshold`
/// ([MachineSink.cpp#L74](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L74)):
/// an edge taken this much of the time or less is worth breaking for a cheap
/// instruction.
const SPLIT_EDGE_PROBABILITY_THRESHOLD: f64 = 0.40;

pub struct MachineSink {
    /// A build for size: successors are tried by loop depth alone
    /// (`shouldOptimizeForSize` in `GetAllSortedSuccessors`).
    pub size: bool,
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
        Ok(sunk(&body, self.size))
    }
}

/// How an instruction that is not a store, a call or a barrier may move:
/// `MachineInstr::isSafeToMove`. A store seen sets `saw_store`; a load is
/// movable only while none has been seen.
fn safe_to_move(
    one: &Insn,
    saw_store: &mut bool,
) -> bool {
    let Some(what) = &one.what else { return false };
    let writes_memory = what.dests.iter().any(|place| matches!(place, Loc::Mem(_)));
    let reads_memory = what.sources.iter().any(|place| matches!(place, Loc::Mem(_)));
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
        *saw_store |= writes_memory || one.call.is_some() || what.op != Operation::Nothing;
        return false;
    }
    !(reads_memory && *saw_store)
        && one.group.is_none()
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.spread.is_empty()
        && !one.defines.is_empty()
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

/// Where a value is read.
#[derive(Clone, Copy)]
enum Use {
    /// By the instruction with this identity.
    Insn(u64),
    /// By a phi of `block`, from the predecessor `from`.
    Phi { block: usize, from: i64 },
}

/// One walk over the body: the blocks as lists of instructions, and what
/// `MachineRegisterInfo`, the dominator trees and the cycle info answer.
struct Sinker {
    size: bool,
    insns: Vec<Vec<Arc<Insn>>>,
    at: Vec<i64>,
    position: IndexMap<i64, usize>,
    cfg: Cfg,
    dom: DomTree,
    post: DomTree,
    /// The block of each instruction, by identity.
    owner: HashMap<u64, usize>,
    uses: HashMap<u32, Vec<Use>>,
    depth: Vec<u32>,
    /// The innermost loop of each block, as its header.
    cycle: Vec<Option<usize>>,
    irreducible: Vec<bool>,
    frequency: Arc<Frequency>,
    /// `CEBCandidates` and `CEMergeCandidates`.
    considered: HashSet<(usize, usize)>,
    /// `GetAllSortedSuccessors` of each block, made once: it asks nothing of
    /// the instruction.
    sorted: Vec<Vec<usize>>,
    merged: HashMap<(u32, usize), usize>,
    /// `ToSplit`.
    to_split: Vec<(usize, usize)>,
    changed: bool,
    regs: llrm_lir::registers::Regs,
    bits: u32,
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
    fn new(
        body: &LirBody,
        size: bool,
    ) -> Self {
        let cfg = Cfg::of(body);
        let dom = DomTree::of(&cfg);
        let post = post_dominators(&cfg);
        let position: IndexMap<i64, usize> =
            body.blocks.iter().enumerate().map(|(number, block)| (block.at, number)).collect();
        let natural = loops::loops(&body.blocks, Some(body.entry));
        let hard = loops::irreducible(&body.blocks, Some(body.entry));
        // The first loop of the list that holds a block is its cycle; made by
        // walking each loop's body once, not asking every loop of every block.
        let mut cycle: Vec<Option<usize>> = vec![None; body.blocks.len()];
        for one in &natural {
            let header = position.get(&one.header).copied();
            for at in one.body.iter() {
                if let Some(&number) = position.get(at) {
                    cycle[number] = cycle[number].or(header);
                }
            }
        }
        let depths = crate::analysis::intervals::depths_shared(body);
        let mut owner = HashMap::default();
        let mut uses: HashMap<u32, Vec<Use>> = HashMap::default();
        for (number, block) in body.blocks.iter().enumerate() {
            for one in block.insns.iter() {
                owner.insert(one.id(), number);
                for value in &one.uses {
                    uses.entry(*value).or_default().push(Use::Insn(one.id()));
                }
            }
            for phi in block.phis.iter() {
                for &(from, value) in &phi.incoming {
                    uses.entry(value).or_default().push(Use::Phi { block: number, from });
                }
            }
        }
        let mut sinker = Self {
            size,
            insns: body.blocks.iter().map(|block| block.insns.to_vec()).collect(),
            at: body.blocks.iter().map(|block| block.at).collect(),
            irreducible: body.blocks.iter().map(|block| hard.contains(&block.at)).collect(),
            depth: body.blocks.iter().map(|block| depths.get(&block.at).copied().unwrap_or(0)).collect(),
            frequency: Frequency::of(body),
            position,
            cfg,
            dom,
            post,
            owner,
            uses,
            cycle,
            considered: HashSet::default(),
            sorted: Vec::new(),
            merged: HashMap::default(),
            to_split: Vec::new(),
            changed: false,
            regs: body.regs(),
            bits: body.bits,
        };
        sinker.sort_successors();
        sinker
    }

    /// The block a use reads its value in: a phi reads it at the end of the
    /// predecessor it names.
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
    /// no later instruction of its block reads it before another sets it
    /// (nothing reads flags across blocks); any other physical def counts as
    /// live (DEVIATE M6). An unknown effect reads and writes them all.
    fn touches_machine_state(
        &self,
        block: usize,
        index: usize,
    ) -> bool {
        let flags = |lanes: &crate::backend::lanes::Lanes| {
            lanes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None)
        };
        let physical = |lanes: &crate::backend::lanes::Lanes| {
            lanes.iter().any(|&(register, _)| register != llrm_lir::registers::RegId::None)
        };
        let Some(effect) = liveness::effect(self.regs, self.bits, &self.insns[block][index]) else { return true };
        if physical(&effect.all_reads()) || physical(&effect.writes) || flags(&effect.all_reads()) {
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
        false
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
        let busy = |one: usize| self.frequency.block(self.at[one]);
        let sorted = (0..n)
            .map(|block| {
                let mut all = self.cfg.succs[block].clone();
                for &child in &children[block] {
                    if !all.contains(&child) {
                        all.push(child);
                    }
                }
                let unknown = all.iter().all(|&one| busy(one) == 0.0);
                all.sort_by(|&left, &right| {
                    if self.size || unknown {
                        self.depth[left].cmp(&self.depth[right])
                    } else {
                        busy(left).total_cmp(&busy(right))
                    }
                });
                all
            })
            .collect();
        self.sorted = sorted;
    }

    /// `FindSuccToSinkTo`.
    fn find_succ_to_sink_to(
        &self,
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
        &self,
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
        // Outside a cycle, sinking to a post-dominator shortens nothing; inside
        // one it could, if the register pressure allowed it (DEVIATE M2).
        false
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
        // A block on an edge costs a jump the size build does not get back.
        if self.size {
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
        let busy = self.frequency.block(self.at[from]);
        if self.cfg.succs[from].contains(&to)
            && busy > 0.0
            && self.frequency.edge(self.at[from], self.at[to]) / busy <= SPLIT_EDGE_PROBABILITY_THRESHOLD
        {
            return true;
        }
        // Cheap, but breaking the edge may let its operands' definitions go
        // too: when this is their only user and they are in this block.
        one.uses
            .iter()
            .any(
                |operand| {
                    let users = self.uses.get(operand).map_or(0, Vec::len);
                    users == 1 && self.defined_in(*operand) == Some(self.owner[&one.id()])
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

    /// `SinkInstruction`: the instruction at `index` of `block`, moved into a
    /// successor or its edge marked for splitting. Whether it moved.
    fn sink_instruction(
        &mut self,
        block: usize,
        index: usize,
        saw_store: &mut bool,
    ) -> bool {
        let one = Arc::clone(&self.insns[block][index]);
        if !safe_to_move(&one, saw_store) {
            return false;
        }
        if self.touches_machine_state(block, index) {
            return false;
        }
        let mut break_phi_edge = false;
        let Some(sink_to) = self.find_succ_to_sink_to(&one, block, &mut break_phi_edge) else { return false };
        if self.size && !one.uses.is_empty() {
            return false;
        }
        // A successor with other predecessors: sink along the edge, or break
        // it.
        if self.cfg.preds[sink_to].len() > 1 {
            let loads =
                one.what.as_ref().is_some_and(|what| what.sources.iter().any(|place| matches!(place, Loc::Mem(_))));
            let enters_cycle = self.cycle[sink_to].is_some_and(|header| self.irreducible[header] || header == sink_to);
            // A load never goes along a critical edge (DEVIATE M1).
            if loads || !self.dom.dominates(block, sink_to) || enters_cycle {
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
        self.owner.insert(one.id(), sink_to);
        self.owner.insert(moved.id(), sink_to);
        // Identity follows the instruction: its values' uses name `one`, and
        // `moved` is `one` in its new block.
        for used in self.uses.values_mut().flatten() {
            if matches!(*used, Use::Insn(id) if id == one.id()) {
                *used = Use::Insn(moved.id());
            }
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

/// `body` with what `MachineSinking::run` moves moved.
pub fn sunk(
    body: &LirBody,
    size: bool,
) -> LirBody {
    let mut body = body.clone();
    // Each round walks the blocks, then breaks the edges it marked; it ends
    // when nothing moved and nothing was marked.
    loop {
        let mut sinker = Sinker::new(&body, size);
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
        let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
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
        let names: Vec<_> =
            sunk(&body, false).blocks[0].insns.iter().filter_map(|one| one.what.as_ref()?.name.clone()).collect();
        assert_eq!(names, ["mov", "or", "je"], "the flag-setting or stays in its block");
    }
}
