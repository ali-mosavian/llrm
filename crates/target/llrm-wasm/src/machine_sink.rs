//! `MachineSinking`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineSink.cpp)):
//! an instruction whose results are used only below one successor of its
//! block moves into that successor, so the other path no longer computes it.
//!
//! Runs on the wasm MIR in SSA form, before phi elimination, as in the pin.
//!
//! An edge into a block with several predecessors is split when the
//! instruction cannot sink along it as it is and splitting is worth it
//! (`PostponeSplitCriticalEdge`); the instruction sinks into the new block on
//! the next round.
//!
//! DEVIATE 1: a load never sinks into a block with several predecessors (the
//! pin looks for a store between the two blocks, `hasStoreBetween`); the
//! register-pressure case of `isProfitableToSinkTo` is "not profitable"; the
//! successors are ordered by loop depth where the pin uses block frequency;
//! an edge with no IR probability (a block a switch made) is "unlikely"
//! (`SplitEdgeProbabilityThreshold`) when it leaves a loop; a function with an
//! irreducible loop is left alone (`FixIrreducible` runs later here). Exit:
//! the shared live intervals, block frequencies and branch probabilities
//! (S4), no later than 2026-11-20.
//!
//! # See also
//! `MachineSinking::{ProcessBlock, SinkInstruction, FindSuccToSinkTo,
//! isProfitableToSinkTo, AllUsesDominatedByBlock}`,
//! `MachineInstr::isSafeToMove`.

use llrm_mir::domtree::{DomTree, PostDomTree};
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::BlockId;
use llrm_support::hash::{HashMap, HashSet};

use crate::cfg::Cfg;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{EH_LABEL, Function, IMPLICIT_DEF, Inst, Operand, PHI, VReg, is_copy};

/// `BranchProbability(40, 100)` as a numerator over 2^31.
const SPLIT_EDGE_PROBABILITY_THRESHOLD: u32 = 858_993_459;

/// A read of a register: the block of the instruction and, for a `PHI`, the
/// predecessor the value comes from (where the read happens).
#[derive(Clone, Copy)]
struct Use {
    block: usize,
    phi_pred: Option<usize>,
}

struct Sink {
    succs: Vec<Vec<usize>>,
    pred_count: Vec<usize>,
    tree: DomTree,
    post: PostDomTree,
    loops: LoopInfo<BlockId>,
    /// The blocks whose immediate dominator is the index.
    children: Vec<Vec<usize>>,
    eh_pad: Vec<bool>,
    uses: Vec<Vec<Use>>,
    defs: Vec<u32>,
    /// The block of each register's definition and whether it is a `PHI`.
    def_site: Vec<(usize, bool)>,
    /// `CEBCandidates`: the edges already considered this round.
    considered: HashSet<(usize, usize)>,
    /// `CEMergeCandidates`: the block that first held back sinking a register
    /// into a block, by `(register, block)`; a physical register is
    /// `(true, id)`.
    merge_candidates: HashMap<((bool, u32), usize), usize>,
    /// The register each copy reads, for `lookThruCopyLike`.
    copy_source: HashMap<VReg, VReg>,
    /// `ToSplit`: the edges to split at the end of the round.
    to_split: Vec<(usize, usize)>,
}

/// Sinks instructions into the successors that use them. Whether anything
/// moved.
pub fn machine_sink(f: &mut Function) -> bool {
    let mut ever = false;
    // Each round recomputes the analyses, as the pin's `run` does.
    for _ in 0..f.blocks.len() + 8 {
        let Some(mut sink) = Sink::of(f) else { return ever };
        let mut changed = false;
        for b in 0..f.blocks.len() {
            changed |= sink.process_block(f, b);
        }
        // From the last predecessor back, so the blocks still to split keep
        // their numbers.
        let mut edges = std::mem::take(&mut sink.to_split);
        edges.sort_unstable_by(|a, b| b.cmp(a));
        let mut split = false;
        for at in 0..edges.len() {
            let (pred, block) = edges[at];
            // `SplitCriticalEdge` returns null where `canSplitCriticalEdge`
            // says no ("Not legal to break critical edge"): a
            // `BR_TABLE` block keeps its edges.
            if f.is_eh_pad(block) || !crate::phi_elimination::can_split_critical_edge(f, pred) {
                continue;
            }
            crate::phi_elimination::split_edge(f, pred, block);
            split = true;
            for later in &mut edges[at + 1..] {
                if later.1 > pred {
                    later.1 += 1;
                }
            }
        }
        changed |= split;
        if !changed {
            return ever;
        }
        ever = true;
    }
    ever
}

/// `MachineInstr::isSafeToMove`; `saw_store` is the pin's `SawStore`.
pub(crate) fn is_safe_to_move(
    inst: &Inst,
    saw_store: &mut bool,
) -> bool {
    if inst.opcode == PHI {
        *saw_store = true;
        return false;
    }
    if inst.opcode == EH_LABEL {
        return false;
    }
    if inst.opcode == IMPLICIT_DEF {
        return true;
    }
    let Some(info) = desc(inst.opcode) else { return false };
    let may_load = info.has(flag::MAY_LOAD);
    let unmodeled = info.has(flag::UNMODELED_SIDE_EFFECTS);
    // `hasOrderedMemoryRef`.
    let ordered = (may_load || info.has(flag::MAY_STORE) || info.has(flag::CALL) || unmodeled)
        && inst.mem.is_none_or(|mem| mem.ordered);
    if info.has(flag::MAY_STORE) || info.has(flag::CALL) || ordered {
        *saw_store = true;
        return false;
    }
    if info.has(flag::TERMINATOR) || info.has(flag::MAY_RAISE_FP_EXCEPTION) || unmodeled {
        return false;
    }
    // A use of a physical register (`ARGUMENTS`) pins the instruction.
    if !info.implicit_uses.is_empty() {
        return false;
    }
    !(may_load && *saw_store)
}

impl Sink {
    /// The analyses of `f`, None when it has an irreducible loop.
    fn of(f: &Function) -> Option<Sink> {
        let cfg = Cfg::of(f);
        let (tree, loops) = cfg.analyses();
        if cfg.irreducible_edge(&tree).is_some() {
            return None;
        }
        let n = f.blocks.len();
        let mut post = PostDomTree::new();
        post.recalculate(&cfg);
        let mut succs: Vec<Vec<usize>> = Vec::with_capacity(n);
        let mut pred_count = vec![0; n];
        for to in &f.succs {
            let mut seen: Vec<usize> = Vec::new();
            for &s in to {
                if !seen.contains(&s) {
                    seen.push(s);
                    pred_count[s] += 1;
                }
            }
            succs.push(seen);
        }
        let mut children = vec![Vec::new(); n];
        for b in 0..n {
            if let Some(up) = tree.immediate_dominator(BlockId(b as u32)) {
                children[up.0 as usize].push(b);
            }
        }
        let eh_pad = (0..n).map(|b| f.is_eh_pad(b)).collect();
        let mut sink = Sink {
            succs,
            pred_count,
            tree,
            post,
            loops,
            children,
            eh_pad,
            uses: Vec::new(),
            defs: Vec::new(),
            def_site: Vec::new(),
            considered: HashSet::default(),
            merge_candidates: HashMap::default(),
            copy_source: HashMap::default(),
            to_split: Vec::new(),
        };
        sink.index_registers(f);
        Some(sink)
    }

    fn index_registers(
        &mut self,
        f: &Function,
    ) {
        let n = f.num_vregs();
        self.uses = vec![Vec::new(); n];
        self.defs = vec![0; n];
        self.def_site = vec![(usize::MAX, false); n];
        for (b, insts) in f.blocks.iter().enumerate() {
            for inst in insts {
                let is_phi = inst.opcode == PHI;
                if let (true, [Operand::Def(d), Operand::Use(s)]) = (is_copy(inst.opcode), &inst.operands[..]) {
                    self.copy_source.insert(*d, *s);
                }
                for (at, operand) in inst.operands.iter().enumerate() {
                    match *operand {
                        Operand::Def(r) => {
                            self.defs[r as usize] += 1;
                            self.def_site[r as usize] = (b, is_phi);
                        }
                        Operand::Use(r) => {
                            let phi_pred = match (is_phi, inst.operands.get(at + 1)) {
                                (true, Some(&Operand::Block(p))) => Some(p as usize),
                                _ => None,
                            };
                            self.uses[r as usize].push(Use { block: b, phi_pred });
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn depth(
        &self,
        block: usize,
    ) -> u32 {
        self.loops.get_loop_depth(BlockId(block as u32))
    }

    fn dominates(
        &self,
        a: usize,
        b: usize,
    ) -> bool {
        self.tree.dominates(BlockId(a as u32), BlockId(b as u32))
    }

    /// `ProcessBlock`.
    fn process_block(
        &mut self,
        f: &mut Function,
        b: usize,
    ) -> bool {
        if self.succs[b].len() <= 1 || f.blocks[b].is_empty() || !self.tree.is_reachable_from_entry(BlockId(b as u32)) {
            return false;
        }
        let mut changed = false;
        let mut saw_store = false;
        let mut at = f.blocks[b].len();
        while at > 0 {
            at -= 1;
            changed |= self.sink_instruction(f, b, at, &mut saw_store);
        }
        changed
    }

    /// `GetAllSortedSuccessors`: the successors of `block`, and the blocks it
    /// immediately dominates when `parent` is that dominator too, by loop
    /// depth.
    fn sorted_successors(
        &self,
        block: usize,
        parent: usize,
    ) -> Vec<usize> {
        let mut all = self.succs[block].clone();
        for &child in &self.children[block] {
            if self.tree.immediate_dominator(BlockId(child as u32)) == Some(BlockId(parent as u32))
                && !self.succs[block].contains(&child)
            {
                all.push(child);
            }
        }
        all.sort_by_key(|&s| self.depth(s));
        all
    }

    /// `AllUsesDominatedByBlock`. `break_phi_edge` is set when every use is a
    /// `PHI` of `block` reading `def_block`'s edge.
    fn all_uses_dominated_by_block(
        &self,
        reg: VReg,
        block: usize,
        def_block: usize,
        break_phi_edge: &mut bool,
        local_use: &mut bool,
    ) -> bool {
        let uses = &self.uses[reg as usize];
        *break_phi_edge = uses.iter().all(|u| u.block == block && u.phi_pred == Some(def_block));
        if *break_phi_edge {
            return true;
        }
        for u in uses {
            let use_block = match u.phi_pred {
                Some(pred) => pred,
                None if u.block == def_block => {
                    *local_use = true;
                    return false;
                }
                None => u.block,
            };
            if !self.dominates(block, use_block) {
                return false;
            }
        }
        true
    }

    /// `FindSuccToSinkTo`.
    fn find_succ(
        &self,
        defs: &[VReg],
        reads: &[VReg],
        parent: usize,
        block: usize,
        break_phi_edge: &mut bool,
        depth: u32,
    ) -> Option<usize> {
        let mut to: Option<usize> = None;
        for &reg in defs {
            if let Some(chosen) = to {
                let mut local = false;
                if !self.all_uses_dominated_by_block(reg, chosen, block, break_phi_edge, &mut local) {
                    return None;
                }
                continue;
            }
            for candidate in self.sorted_successors(block, parent) {
                let mut local = false;
                if self.all_uses_dominated_by_block(reg, candidate, block, break_phi_edge, &mut local) {
                    to = Some(candidate);
                    break;
                }
                if local {
                    return None;
                }
            }
            let chosen = to?;
            if !self.is_profitable_to_sink_to(reg, defs, reads, parent, block, chosen, depth) {
                return None;
            }
        }
        let to = to?;
        (to != block && !self.eh_pad[to]).then_some(to)
    }

    /// `isProfitableToSinkTo`.
    fn is_profitable_to_sink_to(
        &self,
        reg: VReg,
        defs: &[VReg],
        reads: &[VReg],
        parent: usize,
        block: usize,
        to: usize,
        depth: u32,
    ) -> bool {
        if block == to {
            return false;
        }
        if !self.post.dominates(BlockId(to as u32), BlockId(block as u32)) {
            return true;
        }
        if self.depth(block) > self.depth(to) {
            return true;
        }
        if !self.uses[reg as usize].iter().any(|u| u.block == to && u.phi_pred.is_none()) {
            return true;
        }
        if depth < 16
            && let Some(next) = self.find_succ(defs, reads, parent, to, &mut false, depth + 1)
        {
            return self.is_profitable_to_sink_to(reg, defs, reads, parent, to, next, depth + 1);
        }
        // In a cycle, sinking still pays when it shortens the live ranges of
        // the results and no operand defined in the cycle adds pressure.
        let Some(cycle) = self.loops.get_loop_for(BlockId(block as u32)) else { return false };
        let header = cycle.get_header().0 as usize;
        let shortened = defs.iter().all(|&d| self.all_uses_dominated_by_block(d, to, block, &mut false, &mut false));
        shortened
            && reads.iter().all(|&r| {
                let (at, is_phi) = self.def_site[r as usize];
                let in_cycle =
                    self.loops.get_loop_for(BlockId(at as u32)).is_some_and(|l| l.get_header().0 as usize == header);
                // DEVIATE 1: `registerPressureSetExceedsLimit` for an operand
                // defined in the cycle is "exceeds".
                !in_cycle || (is_phi && at == header)
            })
    }

    /// `PostponeSplitCriticalEdge`: note the edge to split when that is worth
    /// it and legal.
    fn postpone_split_critical_edge(
        &mut self,
        f: &Function,
        from: usize,
        at: usize,
        to: usize,
        break_phi_edge: bool,
    ) {
        let (worth, deferred) = self.is_worth_breaking_critical_edge(f, from, at, to);
        // With a deferred edge, `from` is considered only if both are legal.
        if worth
            && deferred.is_none_or(|d| {
                self.to_split.contains(&(d, to)) || self.is_legal_to_break_critical_edge(d, to, break_phi_edge)
            })
            && self.is_legal_to_break_critical_edge(from, to, break_phi_edge)
        {
            for edge in std::iter::once((from, to)).chain(deferred.map(|d| (d, to))) {
                if !self.to_split.contains(&edge) {
                    self.to_split.push(edge);
                }
            }
        }
    }

    /// `lookThruCopyLike`.
    fn look_thru_copies(
        &self,
        mut reg: VReg,
    ) -> VReg {
        while let Some(&source) = self.copy_source.get(&reg) {
            reg = source;
        }
        reg
    }

    /// `isWorthBreakingCriticalEdge`.
    fn is_worth_breaking_critical_edge(
        &mut self,
        f: &Function,
        from: usize,
        at: usize,
        to: usize,
    ) -> (bool, Option<usize>) {
        // Already considered this round: go ahead, so several cheap
        // instructions sink into the same block.
        if !self.considered.insert((from, to)) {
            return (true, None);
        }
        let inst = &f.blocks[from][at];
        let cheap = is_copy(inst.opcode) || desc(inst.opcode).is_some_and(|info| info.has(flag::CHEAP_AS_A_MOVE));
        if !cheap {
            return (true, None);
        }
        // The same register, or the same implicit def (`ARGUMENTS`, which
        // every instruction here defines), held back for the same block from
        // another edge: sink both.
        let implicit = desc(inst.opcode).map_or(&[][..], |info| info.implicit_defs);
        let keys = inst
            .defs()
            .map(|r| (false, self.look_thru_copies(r)))
            .chain(implicit.iter().map(|&p| (true, u32::from(p))))
            .collect::<Vec<_>>();
        for key in keys {
            if let Some(&held) = self.merge_candidates.get(&(key, to)) {
                return (true, Some(held));
            }
            self.merge_candidates.insert((key, to), from);
        }
        // An unlikely edge (`SplitEdgeProbabilityThreshold`, 40%).
        match Self::edge_probability(f, from, to) {
            Some(prob) => {
                if prob <= SPLIT_EDGE_PROBABILITY_THRESHOLD {
                    return (true, None);
                }
            }
            None => {
                if self.loops.get_loop_for(BlockId(from as u32)).is_some_and(|l| !l.contains(BlockId(to as u32))) {
                    return (true, None);
                }
            }
        }
        // Cheap, but its only-use source is defined here, so both may sink.
        let enables_source = inst.operands.iter().any(|operand| match *operand {
            Operand::Use(r) => {
                self.uses[r as usize].len() == 1 && f.blocks[from][..at].iter().any(|i| i.defs().any(|d| d == r))
            }
            _ => false,
        });
        (enables_source, None)
    }

    /// `getEdgeProbability(from, to)`, a numerator over 2^31; none when an
    /// edge of `from` has none.
    fn edge_probability(
        f: &Function,
        from: usize,
        to: usize,
    ) -> Option<u32> {
        let probs = f.edge_probs.get(from)?;
        if probs.len() != f.succs[from].len() || probs.contains(&0) {
            return None;
        }
        Some(f.succs[from].iter().zip(probs).filter(|(s, _)| **s == to).map(|(_, p)| p).sum())
    }

    /// `isLegalToBreakCriticalEdge`.
    fn is_legal_to_break_critical_edge(
        &self,
        from: usize,
        to: usize,
        break_phi_edge: bool,
    ) -> bool {
        if from == to || !self.succs[from].contains(&to) {
            return false;
        }
        // The back edge of a loop stays whole.
        let (from_loop, to_loop) =
            (self.loops.get_loop_for(BlockId(from as u32)), self.loops.get_loop_for(BlockId(to as u32)));
        if from_loop.is_some() && from_loop == to_loop && from_loop.is_some_and(|l| l.get_header().0 as usize == to) {
            return false;
        }
        // The new block must dominate every use: all other predecessors of
        // `to` are dominated by `to` (unless the uses are the PHIs).
        if !break_phi_edge {
            for pred in 0..self.succs.len() {
                if pred != from && self.succs[pred].contains(&to) && !self.dominates(to, pred) {
                    return false;
                }
            }
        }
        true
    }

    /// `SinkInstruction`.
    fn sink_instruction(
        &mut self,
        f: &mut Function,
        b: usize,
        at: usize,
        saw_store: &mut bool,
    ) -> bool {
        let inst = &f.blocks[b][at];
        if !is_safe_to_move(inst, saw_store) {
            return false;
        }
        let defs: Vec<VReg> = inst.defs().collect();
        let reads: Vec<VReg> =
            inst.operands.iter().filter_map(|o| if let Operand::Use(r) = *o { Some(r) } else { None }).collect();
        if defs.is_empty() || defs.iter().any(|&r| self.defs[r as usize] != 1 || self.uses[r as usize].is_empty()) {
            return false;
        }
        let may_load = desc(inst.opcode).is_some_and(|info| info.has(flag::MAY_LOAD));
        let mut break_phi_edge = false;
        let Some(to) = self.find_succ(&defs, &reads, b, b, &mut break_phi_edge, 0) else { return false };
        // A block with several predecessors: sink along the edge if it is
        // safe, else try to break it.
        if self.pred_count[to] > 1 {
            let header = self.loops.get_loop_for(BlockId(to as u32)).is_some_and(|l| l.get_header().0 as usize == to);
            if may_load || !self.dominates(b, to) || header {
                self.postpone_split_critical_edge(f, b, at, to, break_phi_edge);
                return false;
            }
        }
        if break_phi_edge {
            self.postpone_split_critical_edge(f, b, at, to, break_phi_edge);
            return false;
        }
        let inst = f.blocks[b].remove(at);
        for operand in &inst.operands {
            if let Operand::Use(r) = *operand {
                let list = &mut self.uses[r as usize];
                if let Some(held) = list.iter().position(|u| u.block == b && u.phi_pred.is_none()) {
                    list[held].block = to;
                }
            }
        }
        let first =
            f.blocks[to].iter().position(|i| i.opcode != PHI && i.opcode != EH_LABEL).unwrap_or(f.blocks[to].len());
        f.blocks[to].insert(first, inst);
        true
    }
}
