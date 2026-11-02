//! `MachineSinking`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineSink.cpp)):
//! an instruction whose results are used only below one successor of its
//! block moves into that successor, so the other path no longer computes it.
//!
//! Runs on the wasm MIR in SSA form, before phi elimination, as in the pin.
//!
//! DEVIATE 1: no critical-edge splitting. The pin splits an edge when the
//! target has several predecessors and does not take the instruction as it
//! is; here that instruction stays. Exit: the shared machine function (W15),
//! no later than 2026-11-13.
//!
//! DEVIATE 2: a load never sinks into a block with several predecessors (the
//! pin looks for a store between the two blocks, `hasStoreBetween`); the
//! register-pressure case of `isProfitableToSinkTo` is "not profitable"; the
//! successors are ordered by loop depth where the pin uses block frequency;
//! a function with an irreducible loop is left alone (`FixIrreducible` runs
//! later here). Exit: the shared live intervals and block frequencies (S4), no
//! later than 2026-11-20.
//!
//! # See also
//! `MachineSinking::{ProcessBlock, SinkInstruction, FindSuccToSinkTo,
//! isProfitableToSinkTo, AllUsesDominatedByBlock}`,
//! `MachineInstr::isSafeToMove`.

use llrm_mir::domtree::{DomTree, PostDomTree};
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::BlockId;

use crate::cfg::Cfg;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{EH_LABEL, Function, IMPLICIT_DEF, Inst, Operand, PHI, VReg};

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
}

/// Sinks instructions into the successors that use them. Whether anything
/// moved.
pub fn machine_sink(f: &mut Function) -> bool {
    let cfg = Cfg::of(f);
    let (tree, loops) = cfg.analyses();
    if cfg.irreducible_edge(&tree).is_some() {
        return false;
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
    let mut sink = Sink { succs, pred_count, tree, post, loops, children, eh_pad, uses: Vec::new(), defs: Vec::new() };
    sink.index_registers(f);
    let mut ever = false;
    loop {
        let mut changed = false;
        for b in 0..n {
            changed |= sink.process_block(f, b);
        }
        if !changed {
            return ever;
        }
        ever = true;
    }
}

/// `MachineInstr::isSafeToMove`; `saw_store` is the pin's `SawStore`.
fn is_safe_to_move(
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
    fn index_registers(
        &mut self,
        f: &Function,
    ) {
        let n = f.num_vregs();
        self.uses = vec![Vec::new(); n];
        self.defs = vec![0; n];
        for (b, insts) in f.blocks.iter().enumerate() {
            for inst in insts {
                let is_phi = inst.opcode == PHI;
                for (at, operand) in inst.operands.iter().enumerate() {
                    match *operand {
                        Operand::Def(r) => self.defs[r as usize] += 1,
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
            if !self.is_profitable_to_sink_to(reg, defs, parent, block, chosen, depth) {
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
            && let Some(next) = self.find_succ(defs, parent, to, &mut false, depth + 1)
        {
            return self.is_profitable_to_sink_to(reg, defs, parent, to, next, depth + 1);
        }
        false
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
        if defs.is_empty() || defs.iter().any(|&r| self.defs[r as usize] != 1 || self.uses[r as usize].is_empty()) {
            return false;
        }
        let may_load = desc(inst.opcode).is_some_and(|info| info.has(flag::MAY_LOAD));
        let mut break_phi_edge = false;
        let Some(to) = self.find_succ(&defs, b, b, &mut break_phi_edge, 0) else { return false };
        if break_phi_edge {
            return false;
        }
        if self.pred_count[to] > 1 {
            let header = self.loops.get_loop_for(BlockId(to as u32)).is_some_and(|l| l.get_header().0 as usize == to);
            if may_load || !self.dominates(b, to) || header {
                return false;
            }
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
