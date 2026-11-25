//! `BranchFolder` tail merging after the frame is made (`BranchFolderPass`,
//! `TargetPassConfig::addMachineLateOptimization`, after prologue/epilogue
//! insertion): blocks that end in the same instructions share one copy. The
//! algorithm is [`llrm_spike_mf::tailmerge::tail_merge_blocks`]; this module
//! gives it the blocks of the expanded lines.
//!
//! DEVIATE: the instructions are `mc` ones, so only tail merging runs here
//! (the pin's folder also does `OptimizeBranches` and hoisting on this form).
//! Table jumps name every block a table names (the lines drop which table).
//! Instructions are all priced one in `EstimateRuntime`. At size two
//! instructions in common merge (the pin's `getTailMergeSize` is 3): row
//! M68K-LATEFOLD-MIN2. Blocks of different loops merge only at size, as the
//! early fold's `Crossing::Never` has it elsewhere. Exit: 2026-12-12.

use std::cell::RefCell;

use llrm_spike_mf::tailmerge::{Exits, TailMergeFunction, tail_merge_blocks};
use llrm_support::hash::{HashMap, HashSet};

use crate::expand::{Line, Row};
use crate::mc::{self, Operand};

/// `B<cc><width>` branches and their opposites.
const CONDITIONAL: [(&str, &str); 21] = [
    ("Bhi8", "Bls8"),
    ("Bhi16", "Bls16"),
    ("Bhi32", "Bls32"),
    ("Bcc8", "Bcs8"),
    ("Bcc16", "Bcs16"),
    ("Bcc32", "Bcs32"),
    ("Bne8", "Beq8"),
    ("Bne16", "Beq16"),
    ("Bne32", "Beq32"),
    ("Bvc8", "Bvs8"),
    ("Bvc16", "Bvs16"),
    ("Bvc32", "Bvs32"),
    ("Bpl8", "Bmi8"),
    ("Bpl16", "Bmi16"),
    ("Bpl32", "Bmi32"),
    ("Bge8", "Blt8"),
    ("Bge16", "Blt16"),
    ("Bge32", "Blt32"),
    ("Bgt8", "Ble8"),
    ("Bgt16", "Ble16"),
    ("Bgt32", "Ble32"),
];

#[derive(Clone, Copy, Eq, PartialEq)]
enum Ending {
    /// Not a terminator.
    Plain,
    /// `bra`: to a block, or out of the function (a tail call).
    Jump,
    /// A conditional branch that reverses.
    Cond,
    /// A branch to a block that falls through too and cannot reverse (`dbf`).
    Loop,
    /// `rts`, `rte`, `rtr`.
    Return,
    /// A jump through a register or table.
    Table,
    Trap,
}

fn ending(inst: &mc::Inst) -> Ending {
    let name = inst.opcode.as_bytes();
    match name {
        [b'R', b'T', b'S' | b'E' | b'R'] => Ending::Return,
        [b'B', b'R', b'A', ..] => Ending::Jump,
        [b'J', b'M', b'P', ..] => Ending::Table,
        [b'T', b'R', b'A', b'P', ..] => Ending::Trap,
        [b'D', b'B', ..] => Ending::Loop,
        // `B<cc><width>`: a condition is two letters and a width follows
        [b'B', _, _, b'8'] | [b'B', _, _, b'1', b'6'] | [b'B', _, _, b'3', b'2'] if opposite(inst.opcode).is_some() => {
            Ending::Cond
        }
        _ => Ending::Plain,
    }
}

fn opposite(name: &str) -> Option<&'static str> {
    CONDITIONAL
        .iter()
        .find_map(
            |&(one, two)| match () {
                _ if one == name => Some(two),
                _ if two == name => Some(one),
                _ => None,
            },
        )
}

thread_local! {
    /// Instructions, then blocks, the edge scans of this thread looked at.
    static EDGE_SCANS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

/// The instructions and the blocks this thread's edge scans looked at. The
/// graph is rebuilt after every merge; a scan that read every instruction of
/// a block made the pass quadratic (`synth.c` at 68000 -O1 -funroll-loops,
/// 60 % of 100 s).
pub fn edge_scans() -> (usize, usize) {
    EDGE_SCANS.with(std::cell::Cell::get)
}

struct Block {
    id: usize,
    label: Option<usize>,
    insts: Vec<mc::Inst>,
    /// Rows by the index of the instruction they start at.
    rows: Vec<(usize, Row)>,
}

/// Where each block is and who reaches it, indexed by block id. Kept across
/// the edits of a merge (`relink`, as LLVM's `addSuccessor` and
/// `removeSuccessor` keep a block's predecessor list); the passes that rewrite
/// many branches at once drop it for a rebuild.
struct Graph {
    position: Vec<usize>,
    /// Label to block id, `usize::MAX` for none.
    by_label: Vec<usize>,
    /// The blocks each block goes to, sorted.
    successors: Vec<Vec<u32>>,
    /// The blocks that go to each block, in no order.
    predecessors: Vec<Vec<u32>>,
}

impl Graph {
    /// Block `id` goes to `to` and no other block.
    fn set_successors(
        &mut self,
        id: usize,
        to: &[usize],
    ) {
        let old = std::mem::take(&mut self.successors[id]);
        for &gone in old.iter().filter(|&&one| !to.contains(&(one as usize))) {
            let list = &mut self.predecessors[gone as usize];
            if let Some(at) = list.iter().position(|&one| one as usize == id) {
                list.swap_remove(at);
            }
        }
        for &new in to.iter().filter(|&&one| !old.contains(&(one as u32))) {
            self.predecessors[new].push(id as u32);
        }
        self.successors[id] = to.iter().map(|&one| one as u32).collect();
    }

    /// A block `id` with `label` is at `at`; the blocks after it moved up.
    fn insert_block(
        &mut self,
        blocks: &[Block],
        id: usize,
        label: usize,
        at: usize,
    ) {
        if self.position.len() <= id {
            self.position.resize(id + 1, usize::MAX);
            self.successors.resize(id + 1, Vec::new());
            self.predecessors.resize(id + 1, Vec::new());
        }
        if self.by_label.len() <= label {
            self.by_label.resize(label + 1, usize::MAX);
        }
        self.by_label[label] = id;
        for (index, block) in blocks.iter().enumerate().skip(at) {
            self.position[block.id] = index;
        }
    }
}

struct Late {
    prefix: String,
    blocks: Vec<Block>,
    next_label: usize,
    /// The first label this pass made.
    first_new: usize,
    /// Every label a table jump may go to.
    tables: Vec<usize>,
    for_size: bool,
    /// Block to the header of its innermost loop; empty at size, where blocks
    /// of different loops merge.
    loops: HashMap<usize, usize>,
    graph: RefCell<Option<Graph>>,
}

impl Late {
    fn new(
        lines: Vec<Line>,
        number: usize,
        fresh: usize,
        tables: Vec<usize>,
        for_size: bool,
    ) -> Self {
        let mut blocks: Vec<Block> = Vec::new();
        let mut open = |label: Option<usize>, blocks: &mut Vec<Block>| {
            blocks.push(Block { id: blocks.len(), label, insts: Vec::new(), rows: Vec::new() });
        };
        open(None, &mut blocks);
        for line in lines {
            match line {
                Line::Label(at) => {
                    // a label right after a block with no instruction names it
                    match blocks.last_mut() {
                        Some(last) if last.label.is_none() && last.insts.is_empty() => last.label = Some(at),
                        _ => open(Some(at), &mut blocks),
                    }
                }
                Line::Loc(row) => {
                    let last = blocks.last_mut().expect("a block");
                    last.rows.push((last.insts.len(), row));
                }
                Line::Inst(inst) => {
                    let kind = ending(&inst);
                    // a conditional branch and the jump after it are one
                    // block's two terminators
                    // (`analyzeBranch`); a branch alone ends it
                    if blocks
                        .last()
                        .and_then(|last| last.insts.last())
                        .is_some_and(|before| ending(before) == Ending::Cond)
                        && kind != Ending::Jump
                    {
                        open(None, &mut blocks);
                    }
                    let last = blocks.last_mut().expect("a block");
                    last.insts.push(inst);
                    if !matches!(kind, Ending::Plain | Ending::Cond) {
                        open(None, &mut blocks);
                    }
                }
            }
        }
        // the block after a final terminator is empty and unreachable
        if blocks.len() > 1 && blocks.last().is_some_and(|last| last.label.is_none() && last.insts.is_empty()) {
            let last = blocks.pop().expect("a block");
            if let Some(before) = blocks.last_mut() {
                before.rows.extend(last.rows.into_iter().map(|(_, row)| (before.insts.len(), row)));
            }
        }
        Late {
            prefix: format!(".LBB{number}_"),
            blocks,
            next_label: fresh,
            first_new: fresh,
            tables,
            for_size,
            loops: HashMap::default(),
            graph: RefCell::new(None),
        }
    }

    /// The loop each block is in, from the blocks as they are now.
    fn find_loops(&mut self) {
        let graph = self.build_graph();
        let succs: Vec<Vec<usize>> = (0..self.blocks.len())
            .map(|at| self.edges(at, &graph.by_label).into_iter().map(|id| graph.position[id]).collect())
            .collect();
        let cfg = llrm_spike_mf::cfg::Cfg::from_successors(succs);
        self.loops = (0..self.blocks.len())
            .filter_map(|at| cfg.cycle(at).map(|header| (self.blocks[at].id, self.blocks[header].id)))
            .collect();
    }

    /// The lines again; a label this pass made that no branch names is left
    /// out.
    fn into_lines(self) -> Vec<Line> {
        let named: HashSet<usize> =
            self.blocks.iter().flat_map(|block| &block.insts).filter_map(|inst| self.label_of(inst)).collect();
        let mut out = Vec::new();
        for block in self.blocks {
            if let Some(label) = block.label
                && (label < self.first_new || named.contains(&label))
            {
                out.push(Line::Label(label));
            }
            let mut rows = block.rows.into_iter().peekable();
            for (at, inst) in block.insts.into_iter().enumerate() {
                while let Some((_, row)) = rows.next_if(|(index, _)| *index <= at) {
                    out.push(Line::Loc(row));
                }
                out.push(Line::Inst(inst));
            }
            out.extend(rows.map(|(_, row)| Line::Loc(row)));
        }
        out
    }

    fn label_of(
        &self,
        inst: &mc::Inst,
    ) -> Option<usize> {
        inst.operands
            .iter()
            .find_map(
                |operand| match operand {
                    Operand::Symbol(name, 0) => name.strip_prefix(&self.prefix)?.parse().ok(),
                    _ => None,
                },
            )
    }

    fn symbol(
        &self,
        label: usize,
    ) -> Operand {
        Operand::Symbol(format!("{}{label}", self.prefix), 0)
    }

    fn graph<R>(
        &self,
        read: impl FnOnce(&Graph) -> R,
    ) -> R {
        let mut cached = self.graph.borrow_mut();
        read(cached.get_or_insert_with(|| self.build_graph()))
    }

    fn build_graph(&self) -> Graph {
        let size = self.blocks.iter().map(|block| block.id + 1).max().unwrap_or(0);
        let labels = self.blocks.iter().filter_map(|block| block.label).max().map_or(0, |last| last + 1);
        let mut position = vec![usize::MAX; size];
        let mut by_label = vec![usize::MAX; labels];
        for (at, block) in self.blocks.iter().enumerate() {
            position[block.id] = at;
            if let Some(label) = block.label {
                by_label[label] = block.id;
            }
        }
        let mut successors = vec![Vec::new(); size];
        let mut predecessors = vec![Vec::new(); size];
        let mut out = Vec::new();
        for (at, block) in self.blocks.iter().enumerate() {
            self.edges_into(at, &by_label, &mut out);
            for &to in &out {
                predecessors[to].push(block.id as u32);
            }
            successors[block.id] = out.iter().map(|&to| to as u32).collect();
        }
        Graph { position, by_label, successors, predecessors }
    }

    /// The kept graph is the graph of the blocks, as a rebuild would make it
    /// (debug builds, after every edit).
    #[cfg(debug_assertions)]
    fn check_graph(&self) {
        let cached = self.graph.borrow();
        let Some(kept) = cached.as_ref() else { return };
        let fresh = self.build_graph();
        let sorted = |lists: &[Vec<u32>]| -> Vec<Vec<u32>> {
            lists
                .iter()
                .map(|list| {
                    let mut list = list.clone();
                    list.sort_unstable();
                    list
                })
                .collect()
        };
        let live = |graph: &Graph| -> Vec<usize> { self.blocks.iter().map(|block| graph.position[block.id]).collect() };
        assert_eq!(live(kept), live(&fresh), "the kept positions are stale");
        assert_eq!(sorted(&kept.successors), sorted(&fresh.successors), "the kept successors are stale");
        assert_eq!(sorted(&kept.predecessors), sorted(&fresh.predecessors), "the kept predecessors are stale");
        for block in &self.blocks {
            if let Some(label) = block.label {
                assert_eq!(kept.by_label[label], block.id, "the kept label {label} is stale");
            }
        }
    }

    /// The graph after `ids` changed where they go (their branches or the
    /// block after them), when it is kept.
    fn relink(
        &mut self,
        ids: &[usize],
    ) {
        let Some(mut graph) = self.graph.get_mut().take() else { return };
        let mut out = Vec::new();
        for &id in ids {
            self.edges_into(graph.position[id], &graph.by_label, &mut out);
            graph.set_successors(id, &out);
        }
        *self.graph.get_mut() = Some(graph);
        #[cfg(debug_assertions)]
        self.check_graph();
    }

    fn edges(
        &self,
        at: usize,
        by_label: &[usize],
    ) -> Vec<usize> {
        let mut out = Vec::new();
        self.edges_into(at, by_label, &mut out);
        out
    }

    /// The blocks `blocks[at]` goes to, sorted: its branches and the block
    /// after it.
    fn edges_into(
        &self,
        at: usize,
        by_label: &[usize],
        out: &mut Vec<usize>,
    ) {
        out.clear();
        EDGE_SCANS.with(|count| count.set((count.get().0, count.get().1 + 1)));
        let block = &self.blocks[at];
        let named = |label: usize| by_label.get(label).copied().filter(|&id| id != usize::MAX);
        // the terminators are the block's last instructions
        for inst in block.insts.iter().rev().take_while(|inst| {
            EDGE_SCANS.with(|count| count.set((count.get().0 + 1, count.get().1)));
            ending(inst) != Ending::Plain
        }) {
            match ending(inst) {
                Ending::Plain | Ending::Return | Ending::Trap => {}
                Ending::Table => out.extend(self.tables.iter().filter_map(|&label| named(label))),
                _ => out.extend(self.label_of(inst).and_then(named)),
            }
        }
        if self.falls_through(at) {
            out.push(self.blocks[at + 1].id);
        }
        out.sort_unstable();
        out.dedup();
    }

    fn falls_through(
        &self,
        at: usize,
    ) -> bool {
        at + 1 < self.blocks.len()
            && !self.blocks[at].insts.last().is_some_and(|last| {
                matches!(
                    ending(last),
                    Ending::Jump | Ending::Return | Ending::Table | Ending::Trap
                )
            })
    }

    fn at(
        &self,
        block: usize,
    ) -> usize {
        self.graph(|graph| graph.position[block])
    }

    fn changed(&mut self) {
        *self.graph.get_mut() = None;
    }

    fn label_for(
        &mut self,
        block: usize,
    ) -> usize {
        let at = self.at(block);
        if let Some(label) = self.blocks[at].label {
            return label;
        }
        let label = self.next_label;
        self.next_label += 1;
        self.blocks[at].label = Some(label);
        if let Some(graph) = self.graph.get_mut() {
            if graph.by_label.len() <= label {
                graph.by_label.resize(label + 1, usize::MAX);
            }
            graph.by_label[label] = block;
        }
        label
    }

    /// Whether the instruction is a branch to a block of this function.
    fn branches_to_block(
        &self,
        inst: &mc::Inst,
    ) -> bool {
        matches!(ending(inst), Ending::Jump | Ending::Cond) && self.label_of(inst).is_some()
    }

    /// A block that is only a jump goes nowhere its target does not: the
    /// branches to it go to the target (`OptimizeBlock`), and a block no one
    /// reaches is dropped.
    fn forward_jump_only_blocks(
        &mut self,
        pinned: &HashSet<usize>,
    ) {
        let mut forward: HashMap<usize, usize> = HashMap::default();
        for block in &self.blocks {
            let (Some(label), [only]) = (block.label, &block.insts[..]) else { continue };
            if pinned.contains(&label) || ending(only) != Ending::Jump {
                continue;
            }
            if let Some(target) = self.label_of(only).filter(|target| *target != label) {
                forward.insert(label, target);
            }
        }
        if forward.is_empty() {
            return;
        }
        let end_of = |mut label: usize| {
            // a loop of jumps stops after as many steps as there are blocks
            for _ in 0..forward.len() {
                match forward.get(&label) {
                    Some(&next) => label = next,
                    None => break,
                }
            }
            label
        };
        for index in 0..self.blocks.len() {
            for at in 0..self.blocks[index].insts.len() {
                let inst = &self.blocks[index].insts[at];
                let Some(label) = self.label_of(inst).filter(|label| forward.contains_key(label)) else { continue };
                if !self.branches_to_block(inst) {
                    continue;
                }
                let symbol = self.symbol(end_of(label));
                for operand in &mut self.blocks[index].insts[at].operands {
                    if matches!(operand, Operand::Symbol(..)) {
                        *operand = symbol.clone();
                    }
                }
            }
        }
        self.changed();
        // blocks nothing reaches
        let dead: Vec<usize> = self
            .blocks
            .iter()
            .skip(1)
            .filter(|block| {
                block.label.is_some_and(|label| forward.contains_key(&label))
                    && self.graph(|graph| graph.predecessors[block.id].is_empty())
            })
            .map(|block| block.id)
            .collect();
        self.blocks.retain(|block| !dead.contains(&block.id));
        self.changed();
    }
}

impl Late {
    /// A jump-only block that its layout predecessor's conditional branch
    /// falls into becomes that block's second terminator, the form
    /// `analyzeBranch` reads (`OptimizeBlock`).
    fn join_jumps_after_branches(
        &mut self,
        pinned: &HashSet<usize>,
    ) {
        let mut at = 1;
        while at < self.blocks.len() {
            let block = &self.blocks[at];
            let jump_only = matches!(&block.insts[..], [only] if ending(only) == Ending::Jump);
            let before_branches = self.blocks[at - 1].insts.last().is_some_and(|last| ending(last) == Ending::Cond);
            let reached_only_from_before = self.predecessors(block.id) == [self.blocks[at - 1].id];
            let named = block.label.is_some_and(|label| pinned.contains(&label));
            if !(jump_only && before_branches && reached_only_from_before && !named) {
                at += 1;
                continue;
            }
            let joined = self.blocks.remove(at);
            let before = &mut self.blocks[at - 1];
            let base = before.insts.len();
            before.insts.extend(joined.insts);
            before.rows.extend(joined.rows.into_iter().map(|(index, row)| (base + index, row)));
            self.changed();
        }
    }

    fn tidy(
        &mut self,
        pinned: &HashSet<usize>,
    ) {
        self.forward_jump_only_blocks(pinned);
        self.join_jumps_after_branches(pinned);
    }
}

impl TailMergeFunction for Late {
    type Block = usize;
    type Inst = mc::Inst;
    type Cond = mc::Inst;

    fn blocks(&self) -> Vec<usize> {
        self.blocks.iter().map(|block| block.id).collect()
    }

    fn successors(
        &self,
        block: usize,
    ) -> Vec<usize> {
        self.graph(|graph| graph.successors[block].iter().map(|&to| to as usize).collect())
    }

    fn predecessors(
        &self,
        block: usize,
    ) -> Vec<usize> {
        #[cfg(debug_assertions)]
        {
            self.graph(|_| ());
            self.check_graph();
        }
        self.graph(|graph| {
            let mut from: Vec<usize> = graph
                .predecessors
                .get(block)
                .map_or_else(Vec::new, |list| list.iter().map(|&one| one as usize).collect());
            from.sort_unstable_by_key(|&one| graph.position[one]);
            from
        })
    }

    fn insts(
        &self,
        block: usize,
    ) -> &[mc::Inst] {
        &self.blocks[self.at(block)].insts
    }

    fn same_inst(
        &self,
        one: &mc::Inst,
        other: &mc::Inst,
    ) -> bool {
        one == other
    }

    fn hash_inst(
        &self,
        inst: &mc::Inst,
    ) -> u32 {
        format!("{inst:?}").bytes().fold(2166136261u32, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(16777619))
    }

    fn is_terminator(
        &self,
        inst: &mc::Inst,
    ) -> bool {
        ending(inst) != Ending::Plain
    }

    fn is_barrier(
        &self,
        inst: &mc::Inst,
    ) -> bool {
        matches!(
            ending(inst),
            Ending::Jump | Ending::Return | Ending::Table | Ending::Trap
        )
    }

    fn is_call(
        &self,
        inst: &mc::Inst,
    ) -> bool {
        inst.opcode.starts_with("CALL") || inst.opcode.starts_with("BSR")
    }

    fn may_load_or_store(
        &self,
        _inst: &mc::Inst,
    ) -> bool {
        false
    }

    fn is_return_or_indirect(
        &self,
        inst: &mc::Inst,
    ) -> bool {
        match ending(inst) {
            Ending::Return | Ending::Table => true,
            // a tail call leaves the function
            Ending::Jump => self.label_of(inst).is_none(),
            _ => false,
        }
    }

    fn can_fall_through(
        &self,
        block: usize,
    ) -> bool {
        self.falls_through(self.at(block))
    }

    fn optimize_for_size(
        &self,
        _block: usize,
    ) -> bool {
        self.for_size
    }

    fn loop_header(
        &self,
        block: usize,
    ) -> Option<usize> {
        self.loops.get(&block).copied()
    }

    /// `getTailMergeSize`: at size two instructions in common are worth the
    /// jump they leave.
    fn tail_merge_size(&self) -> usize {
        if self.for_size { 2 } else { 3 }
    }

    fn analyze_branch(
        &self,
        block: usize,
    ) -> Option<Exits<usize, mc::Inst>> {
        let insts = &self.blocks[self.at(block)].insts;
        let target = |inst: &mc::Inst| {
            self.label_of(inst)
                .and_then(|label| self.graph(|g| g.by_label.get(label).copied().filter(|&id| id != usize::MAX)))
        };
        let ends: Vec<&mc::Inst> =
            insts.iter().rev().take_while(|inst| ending(inst) != Ending::Plain).take(3).collect();
        match ends[..] {
            [] => Some(Exits { taken: None, other: None, cond: None }),
            [last] => match ending(last) {
                Ending::Jump => Some(Exits { taken: Some(target(last)?), other: None, cond: None }),
                Ending::Cond => Some(Exits { taken: Some(target(last)?), other: None, cond: Some(last.clone()) }),
                _ => None,
            },
            [last, before] if ending(last) == Ending::Jump && ending(before) == Ending::Cond => {
                Some(Exits { taken: Some(target(before)?), other: Some(target(last)?), cond: Some(before.clone()) })
            }
            _ => None,
        }
    }

    fn reverse_branch_condition(
        &self,
        cond: &mc::Inst,
    ) -> Option<mc::Inst> {
        Some(mc::Inst { opcode: opposite(cond.opcode)?, operands: cond.operands.clone() })
    }

    fn remove_branch(
        &mut self,
        block: usize,
    ) {
        let at = self.at(block);
        while self.blocks[at].insts.last().is_some_and(|last| self.branches_to_block(last)) {
            self.blocks[at].insts.pop();
        }
        let len = self.blocks[at].insts.len();
        self.blocks[at].rows.iter_mut().for_each(|(index, _)| *index = (*index).min(len));
        self.relink(&[block]);
    }

    fn insert_branch(
        &mut self,
        block: usize,
        taken: usize,
        other: Option<usize>,
        cond: Option<&mc::Inst>,
    ) {
        let label = self.label_for(taken);
        let at = self.at(block);
        match cond {
            Some(cond) => {
                let mut branch = cond.clone();
                for operand in &mut branch.operands {
                    if matches!(operand, Operand::Symbol(..)) {
                        *operand = self.symbol(label);
                    }
                }
                self.blocks[at].insts.push(branch);
                if let Some(other) = other {
                    let label = self.label_for(other);
                    let jump = mc::Inst { opcode: "BRA8", operands: vec![self.symbol(label)] };
                    self.blocks[at].insts.push(jump);
                }
            }
            None => {
                let jump = mc::Inst { opcode: "BRA8", operands: vec![self.symbol(label)] };
                self.blocks[at].insts.push(jump);
            }
        }
        self.relink(&[block]);
    }

    fn split_block_at(
        &mut self,
        block: usize,
        at: usize,
    ) -> Option<usize> {
        let position = self.at(block);
        let id = self.blocks.iter().map(|block| block.id + 1).max().unwrap_or(0);
        let label = self.next_label;
        self.next_label += 1;
        if let Some(header) = self.loops.get(&block).copied() {
            self.loops.insert(id, header);
        }
        let tail = self.blocks[position].insts.split_off(at);
        let (kept, moved): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.blocks[position].rows).into_iter().partition(|(index, _)| *index < at);
        self.blocks[position].rows = kept;
        let rows = moved.into_iter().map(|(index, row)| (index - at, row)).collect();
        self.blocks.insert(position + 1, Block { id, label: Some(label), insts: tail, rows });
        if let Some(graph) = self.graph.get_mut() {
            graph.insert_block(&self.blocks, id, label, position + 1);
        }
        self.relink(&[block, id]);
        Some(id)
    }

    fn replace_tail_with_branch_to(
        &mut self,
        block: usize,
        at: usize,
        dest: usize,
    ) {
        let label = self.label_for(dest);
        let position = self.at(block);
        self.blocks[position].insts.truncate(at);
        self.blocks[position].rows.retain(|(index, _)| *index < at);
        if self.blocks.get(position + 1).map(|next| next.id) != Some(dest) {
            let jump = mc::Inst { opcode: "BRA8", operands: vec![self.symbol(label)] };
            self.blocks[position].insts.push(jump);
        }
        self.relink(&[block]);
    }
}

/// `lines` with the common tails of its blocks shared. `tables` are the labels
/// a table jump may go to and `address_taken` those a symbol names; `fresh` is
/// the first label no block has.
pub fn merge_tails(
    lines: Vec<Line>,
    number: usize,
    fresh: usize,
    tables: Vec<usize>,
    address_taken: Vec<usize>,
    for_size: bool,
) -> Vec<Line> {
    let mut late = Late::new(lines, number, fresh, tables.clone(), for_size);
    // labels something besides a branch names stay
    let mut pinned: HashSet<usize> = tables.into_iter().chain(address_taken).collect();
    for block in &late.blocks {
        for inst in &block.insts {
            if ending(inst) == Ending::Plain
                && let Some(label) = late.label_of(inst)
            {
                pinned.insert(label);
            }
        }
    }
    late.tidy(&pinned);
    if !for_size {
        late.find_loops();
    }
    while tail_merge_blocks(&mut late) {
        late.tidy(&pinned);
    }
    let lines = late.into_lines();
    drop_jumps_to_next(lines, number)
}

/// A jump to the line after it goes nowhere.
fn drop_jumps_to_next(
    lines: Vec<Line>,
    number: usize,
) -> Vec<Line> {
    let prefix = format!(".LBB{number}_");
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    let mut at = 0;
    while at < lines.len() {
        if let Line::Inst(inst) = &lines[at]
            && inst.opcode.starts_with("BRA")
            && let [Operand::Symbol(name, 0)] = &inst.operands[..]
            && let Some(label) = name.strip_prefix(&prefix).and_then(|rest| rest.parse::<usize>().ok())
        {
            let mut next = at + 1;
            while matches!(lines.get(next), Some(Line::Loc(_))) {
                next += 1;
            }
            if matches!(lines.get(next), Some(Line::Label(shown)) if *shown == label) {
                at += 1;
                continue;
            }
        }
        out.push(lines[at].clone());
        at += 1;
    }
    out
}
