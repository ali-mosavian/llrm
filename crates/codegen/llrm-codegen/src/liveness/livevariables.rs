//! `LiveVariables`: where each virtual register of an SSA body dies, translated
//! from
//! [LiveVariables.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveVariables.cpp)
//! and its header. The pin builds it before PHIElimination, which keeps it up
//! to date, and TwoAddress asks it (`findKill`, `removeKill`) where a source
//! dies.
//!
//! # How it works
//! Each value has a [`VarInfo`]: the blocks it is alive through, and its
//! kills, one per block where it is last used and does not live on. The blocks
//! are walked depth first from the entry, so a definition is seen before its
//! uses; a use in a block other than the definition's marks every block back
//! to the definition alive (`MarkVirtRegAliveInBlock`), which takes away the
//! kills of the blocks it passes through. A phi's operand is a use at the end
//! of its predecessor.
//!
//! DEVIATE LV1 (owner w-1, exit with `Insn::requires`/`delivers`/`clobbers` as
//! operands): the physical-register half (`HandlePhysReg*`,
//! `UpdatePhysRegDefs`, `HandleRegMask`) is absent. LIR carries a fixed
//! register as `requires`, `delivers` and `clobbers` of the instruction, which
//! TwoAddress reads from it (`processCopy`).
//!
//! DEVIATE LV4 (owner w-1, exit with LV3): the body that TwoAddress reads has
//! values with several definitions (a phi's temporary, the copy of a tied
//! pair), so a block can hold several kills of one value, one for each segment
//! that dies in it; the pin's holds one per block and PHIElimination and
//! TwoAddress keep it by explicit calls.
//!
//! DEVIATE LV2 (owner w-1, no exit): a kill is an instruction id with the block
//! it is in (the pin's pointer and `getParent`), or a phi's result for the dead
//! definition of a phi (the pin's PHI instruction).
//!
//! `recomputeForSingleDefVirtReg` and `addNewBlock` are PHIElimination's and
//! the edge splitter's; they come with their callers.
//!
//! # Example
//! `b0: v1 = ...; br b1, b2`, `b1: use v1; jmp b2`, `b2: use v1`: v1 is alive
//! through b1 (b2 reads it after) and killed in b2 by its use.

use llrm_support::bits::Bits;

use crate::liveness::cfg::Cfg;
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::HashMap;

/// An instruction's id.
pub type InstrKey = u64;

/// Where a value is last used in a block, or defined and never used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    Insn(InstrKey),
    /// The result of a phi of the block: a PHI instruction in the pin.
    Phi(u32),
}

/// One entry of `Kills`: the site and the block it is in (`getParent`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kill {
    pub site: Site,
    pub block: usize,
}

/// `LiveVariables::VarInfo`.
#[derive(Clone, Debug)]
pub struct VarInfo {
    /// The blocks the value is alive completely through.
    pub alive_blocks: Bits,
    /// The last uses, one per block.
    pub kills: Vec<Kill>,
}

impl VarInfo {
    fn new(blocks: usize) -> Self {
        Self { alive_blocks: Bits::new(blocks), kills: Vec::new() }
    }

    /// `removeKill`: whether there was a kill at `site`.
    pub fn remove_kill(
        &mut self,
        site: Site,
    ) -> bool {
        match self.kills.iter().position(|kill| kill.site == site) {
            Some(at) => {
                self.kills.remove(at);
                true
            }
            None => false,
        }
    }

    /// Whether `site` kills the value (with several segments in a block, which
    /// of them it ends does not matter to what asks).
    pub fn is_killed_by(
        &self,
        site: Site,
    ) -> bool {
        self.kills.iter().any(|kill| kill.site == site)
    }

    /// `findKill`: the kill in `block`.
    pub fn find_kill(
        &self,
        block: usize,
    ) -> Option<Site> {
        self.kills.iter().find(|kill| kill.block == block).map(|kill| kill.site)
    }
}

/// `LiveVariables`.
pub struct LiveVariables {
    cfg: Cfg,
    blocks: usize,
    info: HashMap<u32, VarInfo>,
    /// `MRI->getDefBlock`: the block of each value's definition.
    def_block: HashMap<u32, usize>,
    /// `PHIVarInfo`: the values the phis of the successors read at the end of
    /// each block.
    phi_var_info: Vec<Vec<u32>>,
    /// Per block, the values live in and out (`from_dataflow` only): what edits
    /// inside a block leave as they were.
    live_in: Vec<Bits>,
    live_out: Vec<Bits>,
    defined: Vec<Bits>,
    /// The values the bit rows hold (one more than the largest).
    top: usize,
}

// The instructions `segment_kills` walked, for a test that an edit walks the
// ones naming the value and not the block.
#[cfg(test)]
thread_local! {
    pub(crate) static WALKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The kills of one value in a block, in order: the instructions that end a
/// segment of it (see [`LiveVariables::from_dataflow`]).
pub fn segment_kills<'a>(
    reg: u32,
    insns: impl Iterator<Item = &'a Insn>,
    live_in: bool,
    live_out: bool,
) -> Vec<Site> {
    let mut kills = Vec::new();
    // The last instruction naming the segment now open; none while the segment
    // is the live-in one, not named yet.
    let mut open: Option<Site> = None;
    let mut is_open = live_in;
    for one in insns {
        #[cfg(test)]
        WALKED.with(|walked| walked.set(walked.get() + 1));
        let here = Site::Insn(one.id());
        if one.uses.contains(&reg) {
            open = Some(here);
            is_open = true;
        }
        if one.defines.contains(&reg) {
            if is_open && let Some(previous) = open {
                kills.push(previous);
            }
            open = Some(here);
            is_open = true;
        }
    }
    if is_open
        && !live_out
        && let Some(last) = open
    {
        kills.push(last);
    }
    kills
}

impl LiveVariables {
    /// `LiveVariables::analyze` of `body`, which is in SSA form.
    pub fn analyze(body: &LirBody) -> Self {
        let cfg = Cfg::of(body);
        let blocks = cfg.len();
        let number: HashMap<i64, usize> = body.blocks.iter().enumerate().map(|(n, block)| (block.at, n)).collect();
        let mut def_block: HashMap<u32, usize> = HashMap::default();
        for (at, block) in body.blocks.iter().enumerate() {
            for phi in block.phis.iter() {
                def_block.entry(phi.result).or_insert(at);
            }
            for one in block.insns.iter() {
                for &value in &one.defines {
                    def_block.entry(value).or_insert(at);
                }
            }
        }
        let mut this = Self {
            cfg,
            blocks,
            info: HashMap::default(),
            def_block,
            phi_var_info: vec![Vec::new(); blocks],
            live_in: Vec::new(),
            live_out: Vec::new(),
            defined: Vec::new(),
            top: 0,
        };
        this.analyze_phi_nodes(body, &number);
        // Depth first from the entry, successors in order.
        let mut visited = vec![false; blocks];
        let mut stack = vec![this.cfg.entry];
        while let Some(block) = stack.pop() {
            if std::mem::replace(&mut visited[block], true) {
                continue;
            }
            this.run_on_block(body, block);
            stack.extend(this.cfg.succs[block].iter().rev().copied().filter(|&next| !visited[next]));
        }
        this
    }

    /// DEVIATE LV3 (owner w-1, exit 2026-12-11, `livevariables-plan.md`): the
    /// same `VarInfo`s of a body that is not in SSA form, which the pin never
    /// recomputes (PHIElimination keeps its own up to date). A value is alive
    /// through a block that it enters and leaves without defining it; it is
    /// killed in a block that does not leave it, by the last instruction that
    /// names it there (a definition no one reads is its own kill).
    pub fn from_dataflow(body: &LirBody) -> Self {
        let cfg = Cfg::of(body);
        let blocks = cfg.len();
        let number: HashMap<i64, usize> = body.blocks.iter().enumerate().map(|(n, block)| (block.at, n)).collect();
        let top = body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .flat_map(|one| one.defines.iter().chain(&one.uses))
            .chain(body.blocks.iter().flat_map(|block| block.phis.iter().map(|phi| &phi.result)))
            .chain(
                body.blocks
                    .iter()
                    .flat_map(|block| block.phis.iter().flat_map(|phi| phi.incoming.iter().map(|edge| &edge.1))),
            )
            .map(|&value| value as usize + 1)
            .max()
            .unwrap_or(0);
        let (mut gen_, mut def, mut forced) =
            (vec![Bits::new(top); blocks], vec![Bits::new(top); blocks], vec![Bits::new(top); blocks]);
        for (at, block) in body.blocks.iter().enumerate() {
            for phi in block.phis.iter() {
                def[at].insert(phi.result as usize);
                for &(from, value) in &phi.incoming {
                    if let Some(&pred) = number.get(&from) {
                        forced[pred].insert(value as usize);
                    }
                }
            }
            for one in block.insns.iter() {
                for &value in &one.uses {
                    if !def[at].contains(value as usize) {
                        gen_[at].insert(value as usize);
                    }
                }
                for &value in &one.defines {
                    def[at].insert(value as usize);
                }
            }
        }
        // `live_in = gen + (live_out - def)`, `live_out = forced + live_in of
        // the successors`.
        let mut live_in = gen_;
        let mut live_out = forced;
        loop {
            let mut changed = false;
            for at in (0..blocks).rev() {
                for &next in &cfg.succs[at] {
                    // A block is its own successor in a loop: the row is read
                    // before it grows.
                    if next == at {
                        let row = live_in[at].clone();
                        changed |= live_out[at].union_changed(&row);
                    } else {
                        changed |= live_out[at].union_changed(&live_in[next]);
                    }
                }
                let (out, into) = (&live_out[at], &mut live_in[at]);
                changed |= into.union_difference(out, &def[at]);
            }
            if !changed {
                break;
            }
        }
        let mut this = Self {
            cfg,
            blocks,
            info: HashMap::default(),
            def_block: HashMap::default(),
            phi_var_info: vec![Vec::new(); blocks],
            live_in: Vec::new(),
            live_out: Vec::new(),
            defined: Vec::new(),
            top: 0,
        };
        // The values named in a block: the last instruction naming each in the
        // segment now open (a dense row reused for every block).
        let mut open: Vec<Option<Site>> = vec![None; top];
        let mut touched: Vec<u32> = Vec::new();
        for (at, block) in body.blocks.iter().enumerate() {
            // Each value's segments in the block, in order: one starts where it
            // is defined (or at the entry, if it is live in) and ends at its
            // last use before the next definition or the end of the block. A
            // segment no later instruction reads ends at its last name (a
            // definition no one reads is its own kill); the last one is no kill
            // where the value is live out. A segment live in and not named
            // ends where it is redefined or leaves, with no kill.
            let mut kills: Vec<(u32, Site)> = Vec::new();
            for phi in block.phis.iter() {
                if open[phi.result as usize].is_none() {
                    touched.push(phi.result);
                }
                open[phi.result as usize] = Some(Site::Phi(phi.result));
            }
            for one in block.insns.iter() {
                let here = Some(Site::Insn(one.id()));
                for &value in &one.uses {
                    if open[value as usize].is_none() {
                        touched.push(value);
                    }
                    open[value as usize] = here;
                }
                for &value in &one.defines {
                    match open[value as usize] {
                        Some(previous) => kills.push((value, previous)),
                        None => touched.push(value),
                    }
                    open[value as usize] = here;
                }
            }
            for value in touched.drain(..) {
                let site = open[value as usize].take();
                if let Some(site) = site
                    && !live_out[at].contains(value as usize)
                {
                    kills.push((value, site));
                }
            }
            for (value, site) in kills {
                this.get_var_info(value).kills.push(Kill { site, block: at });
            }
        }
        this.live_in = live_in;
        this.live_out = live_out;
        this.defined = def;
        this.top = top;
        this
    }

    /// Whether `reg` is live into `block` (`from_dataflow`); a value made since
    /// is local to its block.
    pub fn is_live_in_block(
        &self,
        reg: u32,
        block: usize,
    ) -> bool {
        self.live_in.get(block).is_some_and(|bits| (reg as usize) < self.top && bits.contains(reg as usize))
    }

    /// Whether `reg` is alive through `block` (`from_dataflow`: the rows, not
    /// `VarInfo::alive_blocks`, which it leaves empty): live in and out, and
    /// not defined there.
    pub fn is_alive_through(
        &self,
        reg: u32,
        block: usize,
    ) -> bool {
        self.is_live_in_block(reg, block)
            && self.is_live_out_block(reg, block)
            && !self.defined.get(block).is_some_and(|bits| (reg as usize) < self.top && bits.contains(reg as usize))
    }

    /// Whether `reg` is live out of `block` (`from_dataflow`).
    pub fn is_live_out_block(
        &self,
        reg: u32,
        block: usize,
    ) -> bool {
        self.live_out.get(block).is_some_and(|bits| (reg as usize) < self.top && bits.contains(reg as usize))
    }

    /// What the kills of every value are, to compare two `LiveVariables` of one
    /// body: `(value, kills by site and block)`, values with none left out.
    pub fn shape(&self) -> Vec<(u32, Vec<(Site, usize)>)> {
        let mut rows: Vec<(u32, Vec<(Site, usize)>)> = self
            .info
            .iter()
            .map(|(&value, info)| {
                let mut kills: Vec<(Site, usize)> = info.kills.iter().map(|kill| (kill.site, kill.block)).collect();
                kills.sort_by_key(|&(site, block)| (block, format!("{site:?}")));
                (value, kills)
            })
            .filter(|row| !row.1.is_empty())
            .collect();
        rows.sort_by_key(|row| row.0);
        rows
    }

    /// `analyzePHINodes`: a phi's operand is read at the end of the block it
    /// comes from.
    fn analyze_phi_nodes(
        &mut self,
        body: &LirBody,
        number: &HashMap<i64, usize>,
    ) {
        for block in body.blocks.iter() {
            for phi in block.phis.iter() {
                for &(from, value) in &phi.incoming {
                    if let Some(&pred) = number.get(&from) {
                        self.phi_var_info[pred].push(value);
                    }
                }
            }
        }
    }

    /// `runOnBlock`.
    fn run_on_block(
        &mut self,
        body: &LirBody,
        block: usize,
    ) {
        // A phi is only defined here; its operands are read in the blocks they
        // come from.
        for phi in body.blocks[block].phis.iter() {
            self.handle_virt_reg_def(phi.result, Kill { site: Site::Phi(phi.result), block });
        }
        for one in body.blocks[block].insns.iter() {
            self.run_on_instr(one, block);
        }
        // The phis of the successors read their operands here.
        let reads = std::mem::take(&mut self.phi_var_info[block]);
        for &value in &reads {
            let def = self.def_block[&value];
            self.mark_virt_reg_alive_in_block(value, def, block);
        }
        self.phi_var_info[block] = reads;
    }

    /// `runOnInstr`: the uses, then the definitions.
    fn run_on_instr(
        &mut self,
        one: &Insn,
        block: usize,
    ) {
        let kill = Kill { site: Site::Insn(one.id()), block };
        for &value in &one.uses {
            self.handle_virt_reg_use(value, block, kill);
        }
        for &value in &one.defines {
            self.handle_virt_reg_def(value, kill);
        }
    }

    /// `getVarInfo`.
    pub fn get_var_info(
        &mut self,
        reg: u32,
    ) -> &mut VarInfo {
        let blocks = self.blocks;
        self.info.entry(reg).or_insert_with(|| VarInfo::new(blocks))
    }

    /// The info of a register that has been asked for, or none.
    pub fn var_info(
        &self,
        reg: u32,
    ) -> Option<&VarInfo> {
        self.info.get(&reg)
    }

    /// `HandleVirtRegDef`: a value alive in no block is dead where it is
    /// defined.
    fn handle_virt_reg_def(
        &mut self,
        reg: u32,
        kill: Kill,
    ) {
        let info = self.get_var_info(reg);
        if info.alive_blocks.is_empty() {
            info.kills.push(kill);
        }
    }

    /// `HandleVirtRegUse`.
    fn handle_virt_reg_use(
        &mut self,
        reg: u32,
        block: usize,
        kill: Kill,
    ) {
        let def_block = self.def_block.get(&reg).copied();
        let blocks = self.blocks;
        let info = self.info.entry(reg).or_insert_with(|| VarInfo::new(blocks));
        // The block already kills it: the later use takes over.
        if let Some(last) = info.kills.last_mut()
            && last.block == block
        {
            *last = kill;
            return;
        }
        // A use in a phi of a predecessor of the defining block does not make
        // the value alive in the predecessors.
        if def_block == Some(block) {
            return;
        }
        // Already alive here: alive in a successor, not a kill.
        if !info.alive_blocks.contains(block) {
            info.kills.push(kill);
        }
        let Some(def) = def_block else { return };
        for pred in self.cfg.preds[block].clone() {
            self.mark_virt_reg_alive_in_block(reg, def, pred);
        }
    }

    /// `MarkVirtRegAliveInBlock`: the value is alive in `block` and, back to
    /// `def_block`, in every block that reaches it.
    pub fn mark_virt_reg_alive_in_block(
        &mut self,
        reg: u32,
        def_block: usize,
        block: usize,
    ) {
        let mut work = vec![block];
        let blocks = self.blocks;
        let info = self.info.entry(reg).or_insert_with(|| VarInfo::new(blocks));
        while let Some(block) = work.pop() {
            // A killing block stops killing.
            if let Some(at) = info.kills.iter().position(|kill| kill.block == block) {
                info.kills.remove(at);
            }
            if block == def_block || info.alive_blocks.contains(block) {
                continue;
            }
            info.alive_blocks.insert(block);
            work.extend(self.cfg.preds[block].iter().rev().copied());
        }
    }

    /// `VarInfo::isLiveIn`: alive through `block`, or killed in it; a value
    /// defined in it is not live in.
    pub fn is_live_in(
        &mut self,
        reg: u32,
        block: usize,
    ) -> bool {
        let defined_here = self.def_block.get(&reg) == Some(&block)
            || self.defined.get(block).is_some_and(|bits| (reg as usize) < self.top && bits.contains(reg as usize));
        let info = self.get_var_info(reg);
        if info.alive_blocks.contains(block) {
            return true;
        }
        if defined_here {
            return false;
        }
        info.find_kill(block).is_some()
    }

    /// `isLiveOut`: alive in a successor, or killed in one.
    pub fn is_live_out(
        &mut self,
        reg: u32,
        block: usize,
    ) -> bool {
        let blocks = self.blocks;
        let info = self.info.entry(reg).or_insert_with(|| VarInfo::new(blocks));
        let succs = &self.cfg.succs[block];
        succs.iter().any(|&next| info.alive_blocks.contains(next) || info.kills.iter().any(|kill| kill.block == next))
    }

    /// `addVirtualRegisterKilled`: `one`, in `block`, is a kill of `reg` when
    /// it reads it (or `add_if_not_found`).
    pub fn add_virtual_register_killed(
        &mut self,
        reg: u32,
        one: &Insn,
        block: usize,
        add_if_not_found: bool,
    ) {
        if add_if_not_found || one.uses.contains(&reg) {
            self.get_var_info(reg).kills.push(Kill { site: Site::Insn(one.id()), block });
        }
    }

    /// `removeVirtualRegisterKilled`: whether `one` was a kill of `reg`.
    pub fn remove_virtual_register_killed(
        &mut self,
        reg: u32,
        one: &Insn,
    ) -> bool {
        self.get_var_info(reg).remove_kill(Site::Insn(one.id()))
    }

    /// `addVirtualRegisterDead`: `one` is the kill of a value it defines and
    /// nothing reads.
    pub fn add_virtual_register_dead(
        &mut self,
        reg: u32,
        one: &Insn,
        block: usize,
        add_if_not_found: bool,
    ) {
        if add_if_not_found || one.defines.contains(&reg) {
            self.get_var_info(reg).kills.push(Kill { site: Site::Insn(one.id()), block });
        }
    }

    /// `removeVirtualRegisterDead`.
    pub fn remove_virtual_register_dead(
        &mut self,
        reg: u32,
        one: &Insn,
    ) -> bool {
        self.get_var_info(reg).remove_kill(Site::Insn(one.id()))
    }

    /// `removeVirtualRegistersKilled`: every kill `one` is.
    pub fn remove_virtual_registers_killed(
        &mut self,
        one: &Insn,
    ) {
        for &reg in &one.uses {
            self.get_var_info(reg).remove_kill(Site::Insn(one.id()));
        }
    }

    /// `replaceKillInstruction`: `new` is the kill `old` was.
    pub fn replace_kill_instruction(
        &mut self,
        reg: u32,
        old: InstrKey,
        new: InstrKey,
    ) {
        for kill in &mut self.get_var_info(reg).kills {
            if kill.site == Site::Insn(old) {
                kill.site = Site::Insn(new);
            }
        }
    }

    /// `MRI->getDefBlock` of a value, for the blocks that make one.
    pub fn def_block(
        &self,
        reg: u32,
    ) -> Option<usize> {
        self.def_block.get(&reg).copied()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Held, Loc, Operation, Semantics};
    use crate::model::lir::{LirBlock, Phi};
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    /// `defines := name uses` at address `at`.
    fn insn(
        at: i64,
        name: &str,
        defines: &[u32],
        uses: &[u32],
    ) -> Arc<Insn> {
        let op = if defines.is_empty() { Operation::Return } else { Operation::Binary };
        let what = Semantics {
            name: Some(name.into()),
            dests: defines.iter().map(|&value| held(value)).collect(),
            sources: uses.iter().map(|&value| held(value)).collect(),
            ..Semantics::new(op)
        };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines.to_vec(), uses.to_vec()))
    }

    fn block(
        at: i64,
        succ: &[i64],
        insns: Vec<Arc<Insn>>,
    ) -> LirBlock {
        LirBlock { succ: succ.to_vec().into(), ..LirBlock::new(at, insns) }
    }

    fn body(blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new(
            "lv",
            blocks[0].at,
            blocks.into_iter().map(crate::model::lir::Block::from).collect::<Vec<_>>(),
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    fn kills(
        lv: &LiveVariables,
        reg: u32,
    ) -> Vec<(Site, usize)> {
        lv.var_info(reg).map(|info| info.kills.iter().map(|kill| (kill.site, kill.block)).collect()).unwrap_or_default()
    }

    fn alive(
        lv: &LiveVariables,
        reg: u32,
    ) -> Vec<usize> {
        lv.var_info(reg).map(|info| info.alive_blocks.iter().collect()).unwrap_or_default()
    }

    /// The pin's example: a value read in b1 and again in b2, which b1 reaches,
    /// is alive through b1 and killed once, in b2.
    #[test]
    fn test_a_value_read_in_a_block_and_in_the_one_after_is_alive_through_the_first() {
        let (def, in1, in2) = (insn(0, "mov", &[1], &[]), insn(1, "add", &[2], &[1]), insn(2, "ret", &[], &[1]));
        let lv = LiveVariables::analyze(&body(vec![
            block(0, &[1, 2], vec![Arc::clone(&def)]),
            block(1, &[2], vec![Arc::clone(&in1)]),
            block(2, &[], vec![Arc::clone(&in2)]),
        ]));
        assert_eq!(alive(&lv, 1), [1]);
        assert_eq!(kills(&lv, 1), [(Site::Insn(in2.id()), 2)]);
    }

    /// Two reads in one block: the last is the kill.
    #[test]
    fn test_the_last_read_in_a_block_is_the_kill() {
        let (def, first, last) = (insn(0, "mov", &[1], &[]), insn(1, "add", &[2], &[1]), insn(2, "add", &[3], &[1, 2]));
        let lv = LiveVariables::analyze(&body(vec![block(0, &[], vec![def, first, Arc::clone(&last)])]));
        assert_eq!(kills(&lv, 1), [(Site::Insn(last.id()), 0)]);
        assert_eq!(alive(&lv, 1), Vec::<usize>::new());
    }

    /// A value nothing reads is killed where it is defined
    /// (`HandleVirtRegDef`).
    #[test]
    fn test_a_value_nothing_reads_is_killed_at_its_definition() {
        let def = insn(0, "mov", &[7], &[]);
        let lv = LiveVariables::analyze(&body(vec![block(0, &[], vec![Arc::clone(&def)])]));
        assert_eq!(kills(&lv, 7), [(Site::Insn(def.id()), 0)]);
    }

    /// A phi's operand is read at the end of its predecessor: v1 lives out of
    /// b0 and has no kill there, v2 none in b1; the phi's own value is killed
    /// by its reader.
    #[test]
    fn test_a_phi_operand_is_live_out_of_its_predecessor_and_has_no_kill_there() {
        let (d1, d2, ret) = (insn(0, "mov", &[1], &[]), insn(1, "mov", &[2], &[]), insn(2, "ret", &[], &[3]));
        let mut join = block(2, &[], vec![Arc::clone(&ret)]);
        join.phis = vec![Phi { result: 3, incoming: vec![(0, 1), (1, 2)] }].into();
        let lv = LiveVariables::analyze(&body(vec![block(0, &[1, 2], vec![d1]), block(1, &[2], vec![d2]), join]));
        assert_eq!(kills(&lv, 1), []);
        assert_eq!(kills(&lv, 2), []);
        assert_eq!(kills(&lv, 3), [(Site::Insn(ret.id()), 2)]);
    }

    /// `isLiveOut` and `isLiveIn` follow the alive blocks and the kills.
    #[test]
    fn test_live_in_and_live_out_follow_the_alive_blocks_and_kills() {
        let (def, in1, in2) = (insn(0, "mov", &[1], &[]), insn(1, "add", &[2], &[1]), insn(2, "ret", &[], &[1]));
        let mut lv = LiveVariables::analyze(&body(vec![
            block(0, &[1, 2], vec![def]),
            block(1, &[2], vec![in1]),
            block(2, &[], vec![in2]),
        ]));
        assert!(lv.is_live_out(1, 0) && lv.is_live_out(1, 1) && !lv.is_live_out(1, 2));
        assert!(!lv.is_live_in(1, 0), "defined there");
        assert!(lv.is_live_in(1, 1) && lv.is_live_in(1, 2));
    }

    /// The dataflow finds what the pin's analysis finds on an SSA body: the
    /// diamond, a phi, and a straight line.
    #[test]
    fn test_the_dataflow_finds_what_analyze_finds_on_ssa_bodies() {
        let bodies = {
            let (def, in1, in2) = (insn(0, "mov", &[1], &[]), insn(1, "add", &[2], &[1]), insn(2, "ret", &[], &[1]));
            let diamond = body(vec![block(0, &[1, 2], vec![def]), block(1, &[2], vec![in1]), block(2, &[], vec![in2])]);
            let (d1, d2, ret) = (insn(0, "mov", &[1], &[]), insn(1, "mov", &[2], &[]), insn(2, "ret", &[], &[3]));
            let mut join = block(2, &[], vec![ret]);
            join.phis = vec![Phi { result: 3, incoming: vec![(0, 1), (1, 2)] }].into();
            let phi = body(vec![block(0, &[1, 2], vec![d1]), block(1, &[2], vec![d2]), join]);
            let line = body(vec![block(
                0,
                &[],
                vec![
                    insn(0, "mov", &[1], &[]),
                    insn(1, "add", &[2], &[1]),
                    insn(2, "add", &[3], &[1, 2]),
                    insn(3, "mov", &[7], &[]),
                ],
            )]);
            [diamond, phi, line]
        };
        for body in &bodies {
            assert_eq!(LiveVariables::from_dataflow(body).shape(), LiveVariables::analyze(body).shape());
        }
    }

    /// A value defined in two blocks and read after them (the temporary of an
    /// eliminated phi) is killed by its reader only, and is alive through no
    /// block: the pin's analysis takes the first definition's block for both.
    #[test]
    fn test_a_value_defined_in_two_blocks_is_killed_by_its_one_reader() {
        let (d1, d2, ret) = (insn(0, "mov", &[1], &[]), insn(1, "mov", &[1], &[]), insn(2, "ret", &[], &[1]));
        let lv = LiveVariables::from_dataflow(&body(vec![
            block(0, &[1, 2], vec![d1]),
            block(1, &[2], vec![d2]),
            block(2, &[], vec![Arc::clone(&ret)]),
        ]));
        assert_eq!(kills(&lv, 1), [(Site::Insn(ret.id()), 2)]);
        assert_eq!(alive(&lv, 1), Vec::<usize>::new());
    }

    /// The update API TwoAddress and PHIElimination call: a kill is replaced,
    /// removed, added and found by block.
    #[test]
    fn test_the_kill_of_a_value_is_replaced_removed_and_added() {
        let (def, user, other) = (insn(0, "mov", &[1], &[]), insn(1, "add", &[2], &[1]), insn(2, "add", &[3], &[1]));
        let mut lv = LiveVariables::analyze(&body(vec![block(0, &[], vec![def, Arc::clone(&user)])]));
        assert_eq!(lv.get_var_info(1).find_kill(0), Some(Site::Insn(user.id())));
        lv.replace_kill_instruction(1, user.id(), other.id());
        assert_eq!(lv.get_var_info(1).find_kill(0), Some(Site::Insn(other.id())));
        assert!(lv.remove_virtual_register_killed(1, &other));
        assert_eq!(lv.get_var_info(1).find_kill(0), None);
        lv.add_virtual_register_killed(1, &user, 0, false);
        assert_eq!(lv.get_var_info(1).find_kill(0), Some(Site::Insn(user.id())));
        lv.add_virtual_register_killed(9, &user, 0, false);
        assert_eq!(kills(&lv, 9), [], "an instruction that does not read it is no kill");
    }
}
