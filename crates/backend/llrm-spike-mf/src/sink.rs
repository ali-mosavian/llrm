//! Moves a computation into the one successor that uses it, breaking a
//! critical edge where that is what lets it go. Translated from
//! [MachineSink.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp)
//! (`MachineSinking::run`, before phi elimination, on SSA), over
//! [`MFunction`].
//!
//! Each block with several successors is walked bottom-up. An instruction that
//! is safe to move goes to the successor that dominates every use of the
//! values it defines (`FindSuccToSinkTo`); a use in a phi counts at the end of
//! the phi's predecessor. Where the successor has other predecessors, or all
//! the uses are phis of it, the edge is marked to be split
//! (`PostponeSplitCriticalEdge`); the marked edges get a block each and the
//! walk runs again, until nothing changes.
//!
//! DEVIATE: `PerformSinkAndFold`, `SinkInstsIntoCycle` and debug values are
//! not translated (off by default or no debug info in the spike). Reason: the
//! pin leaves them off for this target. Exit: when a target turns them on.
//! DEVIATE: successors are ordered by cycle depth alone and an edge's
//! probability is `1 / successors`. Reason: no `MachineBlockFrequencyInfo` or
//! `MachineBranchProbabilityInfo` here. Exit: the neutral-LIR task. The pin
//! sorts by cycle depth when both frequencies are 0 and when optimizing for
//! size.
//! DEVIATE: register pressure is the most virtual registers of a class live at
//! once in a block, against the number of registers the class allows; the pin
//! counts per pressure set. Reason: no `RegisterClassInfo` here. Exit: the
//! neutral-LIR task.
//! DEVIATE: `mayAlias` is always true, so a load stays above every store, call
//! and ordered access it could pass (the pin's `AA` is null for this target's
//! machine operands).

use std::collections::{HashMap, HashSet};

use crate::cfg::Cfg;
use crate::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::Target;

/// `SinkLoadInstsPerBlockThreshold` and `SinkLoadBlocksThreshold`.
const LOAD_INSTS_PER_BLOCK: usize = 30;
const LOAD_BLOCKS: usize = 20;
/// `SplitEdgeProbabilityThreshold`, percent.
const SPLIT_EDGE_PROBABILITY: usize = 40;

/// How a target spells a jump to a block.
pub trait Jump {
    fn jump(block: u32) -> Inst;

    /// `reverseBranchCondition`: the conditional branch that goes where
    /// `branch` falls through and falls through where it goes, if there is one.
    fn reverse(_branch: &Inst) -> Option<Inst> {
        None
    }
}

/// Where a virtual register is read: the block holding the instruction or
/// phi, and the block the read happens in (a phi reads at the end of the
/// predecessor it names).
#[derive(Clone, Copy)]
struct Use {
    holder: usize,
    at: usize,
    phi: bool,
}

struct Sink<'a, T: Target> {
    function: &'a mut MFunction<T::Class>,
    cfg: Cfg,
    def_block: Vec<Option<usize>>,
    def_is_copy: Vec<bool>,
    copy_source: Vec<Option<u32>>,
    uses: Vec<Vec<Use>>,
    /// `CEBCandidates`, `CEMergeCandidates` and `ToSplit`, by block id.
    considered: HashSet<(u32, u32)>,
    merge: HashMap<(u32, u32), u32>,
    to_split: Vec<(u32, u32)>,
    live_out: Option<Vec<HashSet<u32>>>,
    /// Whether an edge may get a block of its own for an instruction to land in.
    allow_split: bool,
    /// Whether a copy or a rematerializable instruction stays where it is.
    keep_cheap: bool,
}

fn virt(reg: Reg) -> Option<u32> {
    match reg {
        Reg::Virt(v) => Some(v),
        Reg::Phys(_) => None,
    }
}

fn uses_of(inst: &Inst) -> impl Iterator<Item = Reg> + '_ {
    inst.operands
        .iter()
        .filter_map(
            |operand| match operand {
                Operand::Use(reg) => Some(*reg),
                _ => None,
            },
        )
}

fn defs_of(inst: &Inst) -> impl Iterator<Item = Reg> + '_ {
    inst.operands
        .iter()
        .filter_map(
            |operand| match operand {
                Operand::Def(reg) => Some(*reg),
                _ => None,
            },
        )
}

impl<'a, T: Target + Jump> Sink<'a, T> {
    fn new(function: &'a mut MFunction<T::Class>) -> Self {
        let mut sink = Sink {
            cfg: Cfg::new(function),
            function,
            def_block: Vec::new(),
            def_is_copy: Vec::new(),
            copy_source: Vec::new(),
            uses: Vec::new(),
            considered: HashSet::new(),
            merge: HashMap::new(),
            to_split: Vec::new(),
            live_out: None,
            allow_split: true,
            keep_cheap: false,
        };
        sink.index();
        sink
    }

    /// The def-use chains (`MachineRegisterInfo`), from the function.
    fn index(&mut self) {
        let count = self.function.classes.len();
        self.cfg = Cfg::new(self.function);
        self.def_block = vec![None; count];
        self.def_is_copy = vec![false; count];
        self.copy_source = vec![None; count];
        self.uses = vec![Vec::new(); count];
        self.live_out = None;
        for (at, block) in self.function.blocks.iter().enumerate() {
            for phi in &block.phis {
                if let Some(v) = virt(phi.result) {
                    self.def_block[v as usize] = Some(at);
                }
                for (pred, reg) in &phi.incoming {
                    if let Some(v) = virt(*reg) {
                        self.uses[v as usize].push(Use { holder: at, at: *pred, phi: true });
                    }
                }
            }
            for inst in &block.insts {
                for reg in defs_of(inst) {
                    if let Some(v) = virt(reg) {
                        self.def_block[v as usize] = Some(at);
                        self.def_is_copy[v as usize] = inst.kind == Kind::Copy;
                        if let (Kind::Copy, Some(Reg::Virt(src))) = (inst.kind, uses_of(inst).next()) {
                            self.copy_source[v as usize] = Some(src);
                        }
                    }
                }
                for reg in uses_of(inst) {
                    if let Some(v) = virt(reg) {
                        self.uses[v as usize].push(Use { holder: at, at, phi: false });
                    }
                }
            }
        }
    }

    /// `isDereferenceableInvariantLoad`: a load of an incoming argument slot.
    fn invariant_load(
        &self,
        inst: &Inst,
    ) -> bool {
        inst.may_load
            && !inst.may_store
            && inst.operands.iter().any(|operand| {
                matches!(
                    operand,
                    Operand::Frame(index) if self.function.fixed.contains_key(index)
                )
            })
    }

    /// `MachineInstr::isSafeToMove`: `saw_store` learns of a store passed.
    fn safe_to_move(
        &self,
        inst: &Inst,
        saw_store: &mut bool,
    ) -> bool {
        if inst.may_store || inst.call || inst.side_effects {
            *saw_store = true;
            return false;
        }
        if inst.terminator {
            return false;
        }
        if inst.may_load && !self.invariant_load(inst) && *saw_store {
            return false;
        }
        true
    }

    /// Whether the condition codes `inst` sets are read before the next
    /// instruction sets them again.
    fn flags_live_after(
        &self,
        block: usize,
        at: usize,
    ) -> bool {
        for later in &self.function.blocks[block].insts[at + 1..] {
            if later.reads_flags {
                return true;
            }
            if later.sets_flags {
                return false;
            }
        }
        false
    }

    /// `AllUsesDominatedByBlock`.
    fn all_uses_dominated(
        &self,
        reg: u32,
        block: usize,
        def_block: usize,
        break_phi_edge: &mut bool,
        local_use: &mut bool,
    ) -> bool {
        let uses = &self.uses[reg as usize];
        if uses.is_empty() {
            return true;
        }
        if uses.iter().all(|one| one.holder == block && one.phi && one.at == def_block) {
            *break_phi_edge = true;
            return true;
        }
        for one in uses {
            if !one.phi && one.at == def_block {
                *local_use = true;
                return false;
            }
            if !self.cfg.dominates(block, one.at) {
                return false;
            }
        }
        true
    }

    /// `GetAllSortedSuccessors`.
    fn sorted_successors(
        &self,
        block: usize,
        parent: usize,
        cache: &mut HashMap<usize, Vec<usize>>,
    ) -> Vec<usize> {
        if let Some(found) = cache.get(&block) {
            return found.clone();
        }
        let mut all = self.cfg.succs[block].clone();
        for child in self.cfg.dominated_children(block) {
            if self.cfg.immediate_dominator(child) == Some(parent) && !self.cfg.succs[block].contains(&child) {
                all.push(child);
            }
        }
        all.sort_by_key(|one| self.cfg.cycle_depth(*one));
        cache.insert(block, all.clone());
        all
    }

    /// The most virtual registers of each class live at once in `block`.
    fn pressure(
        &mut self,
        block: usize,
    ) -> Vec<(T::Class, usize)> {
        if self.live_out.is_none() {
            self.live_out = Some(self.liveness());
        }
        let mut live: HashSet<u32> = self.live_out.as_ref().unwrap()[block].clone();
        let mut peak: Vec<(T::Class, usize)> = Vec::new();
        let mut note = |live: &HashSet<u32>, classes: &[T::Class]| {
            let mut now: Vec<(T::Class, usize)> = Vec::new();
            for v in live {
                let class = classes[*v as usize];
                match now.iter_mut().find(|(one, _)| *one == class) {
                    Some((_, count)) => *count += 1,
                    None => now.push((class, 1)),
                }
            }
            for (class, count) in now {
                match peak.iter_mut().find(|(one, _)| *one == class) {
                    Some((_, most)) => *most = (*most).max(count),
                    None => peak.push((class, count)),
                }
            }
        };
        note(&live, &self.function.classes);
        for inst in self.function.blocks[block].insts.iter().rev() {
            for reg in defs_of(inst) {
                if let Some(v) = virt(reg) {
                    live.remove(&v);
                }
            }
            for reg in uses_of(inst) {
                if let Some(v) = virt(reg) {
                    live.insert(v);
                }
            }
            note(&live, &self.function.classes);
        }
        peak
    }

    /// The virtual registers live out of each block.
    fn liveness(&self) -> Vec<HashSet<u32>> {
        let count = self.function.blocks.len();
        let mut live_in: Vec<HashSet<u32>> = vec![HashSet::new(); count];
        let mut live_out: Vec<HashSet<u32>> = vec![HashSet::new(); count];
        let mut changed = true;
        while changed {
            changed = false;
            for block in (0..count).rev() {
                let mut out: HashSet<u32> = HashSet::new();
                for &succ in &self.cfg.succs[block] {
                    let results: HashSet<u32> =
                        self.function.blocks[succ].phis.iter().filter_map(|phi| virt(phi.result)).collect();
                    out.extend(live_in[succ].iter().filter(|v| !results.contains(v)));
                    for phi in &self.function.blocks[succ].phis {
                        for (pred, reg) in &phi.incoming {
                            if *pred == block {
                                out.extend(virt(*reg));
                            }
                        }
                    }
                }
                let mut live = out.clone();
                for inst in self.function.blocks[block].insts.iter().rev() {
                    for reg in defs_of(inst) {
                        live.remove(&virt(reg).unwrap_or(u32::MAX));
                    }
                    live.extend(uses_of(inst).filter_map(virt));
                }
                if out != live_out[block] || live != live_in[block] {
                    live_out[block] = out;
                    live_in[block] = live;
                    changed = true;
                }
            }
        }
        live_out
    }

    /// `registerPressureSetExceedsLimit(1, class, block)`.
    fn pressure_exceeds(
        &mut self,
        class: T::Class,
        block: usize,
    ) -> bool {
        let limit = T::candidates(class).len();
        let have = self.pressure(block).iter().find(|(one, _)| *one == class).map_or(0, |(_, count)| *count);
        1 + have >= limit
    }

    /// `isProfitableToSinkTo`.
    fn profitable_to_sink_to(
        &mut self,
        reg: u32,
        inst: &Inst,
        block: usize,
        to: usize,
        parent: usize,
        cache: &mut HashMap<usize, Vec<usize>>,
    ) -> bool {
        if block == to {
            return false;
        }
        if !self.cfg.post_dominates(to, block) {
            return true;
        }
        if self.cfg.cycle_depth(block) > self.cfg.cycle_depth(to) {
            return true;
        }
        let non_phi_use = self.uses[reg as usize].iter().any(|one| one.holder == to && !one.phi);
        if !non_phi_use {
            return true;
        }
        let mut break_phi_edge = false;
        if let Some(next) = self.find_succ_to_sink_to(inst, to, parent, &mut break_phi_edge, cache) {
            return self.profitable_to_sink_to(reg, inst, to, next, parent, cache);
        }
        let Some(cycle) = self.cfg.cycle(block) else { return false };
        for operand in &inst.operands {
            let (Operand::Def(r) | Operand::Use(r)) = operand else { continue };
            let Some(v) = virt(*r) else {
                if matches!(operand, Operand::Use(_)) {
                    return false;
                }
                continue;
            };
            if matches!(operand, Operand::Def(_)) {
                let (mut break_phi, mut local) = (false, false);
                if !self.all_uses_dominated(v, to, block, &mut break_phi, &mut local) {
                    return false;
                }
            } else {
                let Some(def) = self.def_block[v as usize] else { continue };
                let def_cycle = self.cfg.cycle(def);
                let header_phi = def_cycle.is_some()
                    && self.cfg.reducible()
                    && def_cycle == Some(def)
                    && self.function.blocks[def].phis.iter().any(|phi| phi.result == Reg::Virt(v));
                if def_cycle != Some(cycle) || header_phi {
                    continue;
                }
                let class = self.function.classes[v as usize];
                if self.pressure_exceeds(class, to) {
                    return false;
                }
            }
        }
        true
    }

    /// `FindSuccToSinkTo`; `parent` is the block the instruction is in.
    fn find_succ_to_sink_to(
        &mut self,
        inst: &Inst,
        block: usize,
        parent: usize,
        break_phi_edge: &mut bool,
        cache: &mut HashMap<usize, Vec<usize>>,
    ) -> Option<usize> {
        let mut found: Option<usize> = None;
        for operand in &inst.operands {
            match operand {
                Operand::Use(Reg::Phys(_)) | Operand::Def(Reg::Phys(_)) => return None,
                Operand::Def(Reg::Virt(reg)) => {
                    if let Some(to) = found {
                        let mut local = false;
                        if !self.all_uses_dominated(*reg, to, block, break_phi_edge, &mut local) {
                            return None;
                        }
                        continue;
                    }
                    for candidate in self.sorted_successors(block, parent, cache) {
                        let mut local = false;
                        if self.all_uses_dominated(*reg, candidate, block, break_phi_edge, &mut local) {
                            found = Some(candidate);
                            break;
                        }
                        if local {
                            return None;
                        }
                    }
                    let to = found?;
                    if !self.profitable_to_sink_to(*reg, inst, block, to, parent, cache) {
                        return None;
                    }
                }
                _ => {}
            }
        }
        if found == Some(block) {
            return None;
        }
        found
    }

    /// `hasStoreBetween`: a store, call or ordered access that may sit between
    /// `from` and `to`, which a load must not cross.
    fn has_store_between(
        &self,
        from: usize,
        to: usize,
    ) -> bool {
        if !self.cfg.dominates(from, to) || !self.cfg.post_dominates(to, from) {
            return true;
        }
        let mut seen = vec![false; self.function.blocks.len()];
        let mut stack = vec![from];
        let mut handled = 0;
        while let Some(block) = stack.pop() {
            if std::mem::replace(&mut seen[block], true) {
                continue;
            }
            stack.extend(self.cfg.succs[block].iter().copied());
            if block == to || block == from || !self.cfg.post_dominates(to, block) {
                continue;
            }
            handled += 1;
            let insts = &self.function.blocks[block].insts;
            if insts.len() > LOAD_INSTS_PER_BLOCK || handled > LOAD_BLOCKS {
                return true;
            }
            if insts.iter().any(|inst| inst.call || inst.side_effects || inst.may_store) {
                return true;
            }
        }
        false
    }

    /// `isLegalToBreakCriticalEdge`.
    fn legal_to_break(
        &self,
        from: usize,
        to: usize,
        break_phi_edge: bool,
    ) -> bool {
        if from == to || !self.cfg.succs[from].contains(&to) {
            return false;
        }
        let (from_cycle, to_cycle) = (self.cfg.cycle(from), self.cfg.cycle(to));
        if from_cycle == to_cycle && from_cycle.is_some() && (!self.cfg.reducible() || from_cycle == Some(to)) {
            return false;
        }
        if !break_phi_edge {
            for &pred in &self.cfg.preds[to] {
                if pred != from && !self.cfg.dominates(to, pred) {
                    return false;
                }
            }
        }
        true
    }

    /// `isWorthBreakingCriticalEdge`; also names the other edge that was held
    /// off for the same value, if any.
    fn worth_breaking(
        &mut self,
        inst: &Inst,
        from: usize,
        to: usize,
        deferred: &mut Option<usize>,
    ) -> bool {
        let (from_id, to_id) = (self.function.blocks[from].id, self.function.blocks[to].id);
        if !self.considered.insert((from_id, to_id)) {
            return true;
        }
        if inst.kind != Kind::Copy && !inst.cheap_as_move {
            return true;
        }
        for reg in defs_of(inst) {
            let Some(mut v) = virt(reg) else { continue };
            while let Some(source) = self.copy_source[v as usize] {
                v = source;
            }
            match self.merge.get(&(v, to_id)) {
                Some(&first) => {
                    *deferred = self.function.blocks.iter().position(|block| block.id == first);
                    return true;
                }
                None => {
                    self.merge.insert((v, to_id), from_id);
                }
            }
        }
        if self.cfg.succs[from].contains(&to) && 100 / self.cfg.succs[from].len() <= SPLIT_EDGE_PROBABILITY {
            return true;
        }
        for reg in uses_of(inst) {
            let Some(v) = virt(reg) else { continue };
            if self.uses[v as usize].len() == 1 && self.def_block[v as usize] == Some(from) {
                return true;
            }
        }
        false
    }

    /// `PostponeSplitCriticalEdge`.
    fn postpone_split(
        &mut self,
        inst: &Inst,
        from: usize,
        to: usize,
        break_phi_edge: bool,
    ) -> bool {
        if !self.allow_split {
            return false;
        }
        let mut deferred = None;
        if !self.worth_breaking(inst, from, to, &mut deferred) {
            return false;
        }
        let id = |at: usize, this: &Self| this.function.blocks[at].id;
        let to_id = id(to, self);
        let deferred_ok = match deferred {
            None => true,
            Some(other) => {
                self.to_split.contains(&(id(other, self), to_id)) || self.legal_to_break(other, to, break_phi_edge)
            }
        };
        if !(deferred_ok && self.legal_to_break(from, to, break_phi_edge)) {
            return false;
        }
        let pair = (id(from, self), to_id);
        if !self.to_split.contains(&pair) {
            self.to_split.push(pair);
        }
        if let Some(other) = deferred {
            let pair = (id(other, self), to_id);
            if !self.to_split.contains(&pair) {
                self.to_split.push(pair);
            }
        }
        true
    }

    /// `PerformTrivialForwardCoalescing`.
    fn forward_coalesce(
        &mut self,
        block: usize,
        at: usize,
    ) -> bool {
        let inst = &self.function.blocks[block].insts[at];
        if inst.kind != Kind::Copy {
            return false;
        }
        let (Some(Operand::Def(Reg::Virt(dst))), Some(Operand::Use(Reg::Virt(src)))) =
            (inst.operands.first(), inst.operands.get(1))
        else {
            return false;
        };
        let (dst, src) = (*dst, *src);
        if self.uses[src as usize].len() != 1
            || self.function.classes[src as usize] != self.function.classes[dst as usize]
            || self.def_block[src as usize].is_none()
            || self.def_is_copy[src as usize]
        {
            return false;
        }
        self.function.blocks[block].insts.remove(at);
        if let Some(copy) = self.uses[src as usize].iter().position(|one| one.holder == block && !one.phi) {
            self.uses[src as usize].remove(copy);
        }
        for site in std::mem::take(&mut self.uses[dst as usize]) {
            let holder = &mut self.function.blocks[site.holder];
            for phi in &mut holder.phis {
                for (_, reg) in &mut phi.incoming {
                    if *reg == Reg::Virt(dst) {
                        *reg = Reg::Virt(src);
                    }
                }
            }
            for other in &mut holder.insts {
                for operand in &mut other.operands {
                    if let Operand::Use(reg) = operand {
                        if *reg == Reg::Virt(dst) {
                            *reg = Reg::Virt(src);
                        }
                    }
                }
            }
            self.uses[src as usize].push(site);
        }
        self.live_out = None;
        true
    }

    /// `SinkInstruction`.
    fn sink_instruction(
        &mut self,
        block: usize,
        at: usize,
        saw_store: &mut bool,
        cache: &mut HashMap<usize, Vec<usize>>,
    ) -> bool {
        let inst = self.function.blocks[block].insts[at].clone();
        if self.keep_cheap && (inst.kind == Kind::Copy || inst.rematerializable) {
            return false;
        }
        if !self.safe_to_move(&inst, saw_store) {
            return false;
        }
        if inst.reads_flags || (inst.sets_flags && self.flags_live_after(block, at)) {
            return false;
        }
        let mut break_phi_edge = false;
        let Some(to) = self.find_succ_to_sink_to(&inst, block, block, &mut break_phi_edge, cache) else {
            return false;
        };
        if self.cfg.preds[to].len() > 1 {
            let store = if inst.may_load { self.has_store_between(block, to) } else { true };
            let mut copy = store;
            let mut break_edge = !self.safe_to_move(&inst, &mut copy);
            if !break_edge && !self.cfg.dominates(block, to) {
                break_edge = true;
            }
            if !break_edge && self.cfg.cycle(to).is_some() && (!self.cfg.reducible() || self.cfg.cycle(to) == Some(to))
            {
                break_edge = true;
            }
            if break_edge {
                self.postpone_split(&inst, block, to, break_phi_edge);
                return false;
            }
        }
        if break_phi_edge {
            self.postpone_split(&inst, block, to, break_phi_edge);
            return false;
        }
        self.function.blocks[block].insts.remove(at);
        for reg in defs_of(&inst) {
            if let Some(v) = virt(reg) {
                self.def_block[v as usize] = Some(to);
            }
        }
        for reg in uses_of(&inst) {
            if let Some(v) = virt(reg) {
                if let Some(site) = self.uses[v as usize].iter_mut().find(|one| one.holder == block && !one.phi) {
                    site.holder = to;
                    site.at = to;
                }
            }
        }
        self.function.blocks[to].insts.insert(0, inst);
        self.live_out = None;
        true
    }

    /// `ProcessBlock`.
    fn process_block(
        &mut self,
        block: usize,
    ) -> bool {
        if self.cfg.succs[block].len() <= 1
            || self.function.blocks[block].insts.is_empty()
            || !self.cfg.reachable(block)
        {
            return false;
        }
        let mut changed = false;
        let mut cache = HashMap::new();
        let mut saw_store = false;
        let mut at = self.function.blocks[block].insts.len();
        while at > 0 {
            at -= 1;
            if self.forward_coalesce(block, at) {
                changed = true;
                continue;
            }
            if self.sink_instruction(block, at, &mut saw_store, &mut cache) {
                changed = true;
            }
        }
        changed
    }

    /// The splits `ToSplit` holds, each as `SplitCriticalEdge`.
    fn split_edges(&mut self) -> bool {
        let mut changed = false;
        for (from_id, to_id) in std::mem::take(&mut self.to_split) {
            let position = |function: &MFunction<T::Class>, id: u32| function.blocks.iter().position(|b| b.id == id);
            let (Some(from), Some(to)) = (position(self.function, from_id), position(self.function, to_id)) else {
                continue;
            };
            if split_critical_edge(self.function, from, to, &T::jump).is_some() {
                changed = true;
            }
        }
        changed
    }

    fn run(&mut self) -> bool {
        let mut ever = false;
        loop {
            self.considered.clear();
            self.merge.clear();
            self.to_split.clear();
            let mut changed = false;
            for block in 0..self.function.blocks.len() {
                changed |= self.process_block(block);
            }
            if !self.to_split.is_empty() {
                // the indices move with the new blocks: index again
                changed |= self.split_edges();
                self.index();
            }
            if !changed {
                return ever;
            }
            ever = true;
        }
    }
}

/// `MachineBasicBlock::SplitCriticalEdge`: a block of its own on the edge
/// `from` -> `to`, laid out right after `from`. Returns its index, or `None`
/// where a conditional branch names `to` twice.
pub fn split_critical_edge<C>(
    function: &mut MFunction<C>,
    from: usize,
    to: usize,
    jump: &dyn Fn(u32) -> Inst,
) -> Option<usize> {
    let to_id = function.blocks[to].id;
    let falls = !function.blocks[from].insts.last().is_some_and(|last| last.barrier) && from + 1 == to;
    let mut named = 0;
    for inst in function.blocks[from].insts.iter().rev().take_while(|inst| inst.terminator) {
        named += inst.operands.iter().filter(|operand| matches!(operand, Operand::Block(id) if *id == to_id)).count();
    }
    if named + usize::from(falls) != 1 {
        return None;
    }
    let new_id = function.fresh_block_id();
    for inst in function.blocks[from].insts.iter_mut().rev().take_while(|inst| inst.terminator) {
        for operand in &mut inst.operands {
            if matches!(operand, Operand::Block(id) if *id == to_id) {
                *operand = Operand::Block(new_id);
            }
        }
    }
    let falls_through = !function.blocks[from].insts.last().is_some_and(|last| last.barrier);
    function.insert_block(from + 1, Block { id: new_id, insts: Vec::new(), phis: Vec::new() });
    let to = if to > from { to + 1 } else { to };
    // `updateTerminator`: the block `from` fell into is no longer next
    if falls_through && !falls {
        let next = function.blocks[from + 2].id;
        function.blocks[from].insts.push(jump(next));
        let last = function.blocks[from].insts.last_mut().unwrap();
        last.terminator = true;
        last.barrier = true;
    }
    if to != from + 2 {
        let mut branch = jump(to_id);
        branch.terminator = true;
        branch.barrier = true;
        function.blocks[from + 1].insts.push(branch);
    }
    for phi in &mut function.blocks[to].phis {
        for (pred, _) in &mut phi.incoming {
            if *pred == from {
                *pred = from + 1;
            }
        }
    }
    Some(from + 1)
}

/// `MachineSinking`: true when anything moved.
pub fn machine_sink<T: Target + Jump>(function: &mut MFunction<T::Class>) -> bool {
    Sink::<T>::new(function).run()
}

/// [`machine_sink`] where no edge is split and a copy or a rematerializable instruction stays.
/// DEVIATE: the pin splits when `PostponeSplitCriticalEdge` finds it worth it, and sinks a copy
/// or a constant into the one successor that reads it. Reason: on a target whose jump costs
/// bytes (the 6502's `jmp` is three) a block made for a one-move instruction costs more than the
/// move saves on the other path (lz4 grew 16 bytes). Exit: the neutral-LIR task, with a
/// block-cost hook.
pub fn machine_sink_in_place<T: Target + Jump>(function: &mut MFunction<T::Class>) -> bool {
    let mut sink = Sink::<T>::new(function);
    sink.allow_split = false;
    sink.keep_cheap = true;
    sink.run()
}
