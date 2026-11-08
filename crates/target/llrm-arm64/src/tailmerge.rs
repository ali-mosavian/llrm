//! `BranchFolder::TailMergeBlocks` for arm64: blocks that end in the same
//! instructions share one copy. The algorithm is the shared one
//! (`llrm_spike_mf::tail_merge_blocks`, as wasm's); this file is arm64's side of
//! it, the `analyzeBranch`, `removeBranch` and `insertBranch` of
//! `AArch64InstrInfo` over the emitted [`Lowered`] blocks, every fall-through an
//! explicit branch.
//!
//! DEVIATE: no loop info ([`TailMergeFunction::loop_header`]), so blocks of
//! different loops can merge as the pin's do, and no jump-table or EH blocks:
//! a block with an indirect branch is not understood. The directives and marks
//! that tell the frame and the line table what the code did are never part of a
//! shared tail. Exit: the shared branch folder over the neutral machine
//! function, no later than 2026-11-30.

use std::cell::RefCell;

use llrm_spike_mf::tailmerge::{Exits, TailMergeFunction, tail_merge_blocks};
use llrm_support::hash::HashMap;

use crate::frame::Lowered;
use crate::jumptable::Table;
use crate::layout::label_of;
use crate::mc::inst::{Inst, Target};

fn is_cond(inst: &Inst) -> bool {
    matches!(inst, Inst::BranchCond { .. } | Inst::CompareBranch { .. } | Inst::TestBranch { .. })
}

fn is_barrier(inst: &Inst) -> bool {
    matches!(
        inst,
        Inst::Branch { link: false, .. } | Inst::BranchReg { link: false, .. } | Inst::Ret { .. } | Inst::Brk { .. }
    )
}

fn is_terminator(inst: &Inst) -> bool {
    is_cond(inst) || is_barrier(inst)
}

fn is_meta(inst: &Inst) -> bool {
    matches!(inst, Inst::Mark(_) | Inst::Label(_) | Inst::Cfi(_))
}

fn jump(to: &str) -> Inst {
    Inst::Branch { link: false, target: Target::Label(to.to_owned()) }
}

fn retargeted(
    cond: &Inst,
    to: &str,
) -> Inst {
    let target = Target::Label(to.to_owned());
    match cond {
        Inst::BranchCond { cond, .. } => Inst::BranchCond { cond: *cond, target },
        Inst::CompareBranch { nonzero, width, rt, .. } => {
            Inst::CompareBranch { nonzero: *nonzero, width: *width, rt: *rt, target }
        }
        Inst::TestBranch { nonzero, rt, bit, .. } => {
            Inst::TestBranch { nonzero: *nonzero, rt: *rt, bit: *bit, target }
        }
        other => other.clone(),
    }
}

struct Tails<'a> {
    blocks: Vec<Lowered>,
    /// The blocks in layout order, by position in `blocks`.
    layout: Vec<usize>,
    index: HashMap<String, usize>,
    tables: &'a [Table],
    for_size: bool,
    preds: RefCell<Option<Vec<Vec<usize>>>>,
    made: usize,
}

impl Tails<'_> {
    fn position(
        &self,
        block: usize,
    ) -> usize {
        self.layout.iter().position(|&one| one == block).expect("a block in the layout")
    }

    fn edges(
        &self,
        block: usize,
    ) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for inst in &self.blocks[block].insts {
            if let Some(to) = label_of(inst).and_then(|label| self.index.get(label)) {
                if !out.contains(to) {
                    out.push(*to);
                }
            }
        }
        out
    }
}

impl TailMergeFunction for Tails<'_> {
    type Block = usize;
    type Inst = Inst;
    type Cond = Inst;

    fn blocks(&self) -> Vec<usize> {
        self.layout.clone()
    }

    fn successors(
        &self,
        block: usize,
    ) -> Vec<usize> {
        self.edges(block)
    }

    fn predecessors(
        &self,
        block: usize,
    ) -> Vec<usize> {
        let mut cache = self.preds.borrow_mut();
        let preds = cache.get_or_insert_with(|| {
            let mut preds: Vec<Vec<usize>> = vec![Vec::new(); self.blocks.len()];
            for &from in &self.layout {
                for to in self.edges(from) {
                    preds[to].push(from);
                }
            }
            preds
        });
        preds[block].clone()
    }

    fn insts(
        &self,
        block: usize,
    ) -> &[Inst] {
        &self.blocks[block].insts
    }

    fn same_inst(
        &self,
        one: &Inst,
        other: &Inst,
    ) -> bool {
        !is_meta(one) && one == other
    }

    fn hash_inst(
        &self,
        inst: &Inst,
    ) -> u32 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{inst:?}").hash(&mut hasher);
        hasher.finish() as u32
    }

    fn is_terminator(
        &self,
        inst: &Inst,
    ) -> bool {
        is_terminator(inst)
    }

    fn is_barrier(
        &self,
        inst: &Inst,
    ) -> bool {
        is_barrier(inst)
    }

    fn is_call(
        &self,
        inst: &Inst,
    ) -> bool {
        matches!(inst, Inst::Branch { link: true, .. } | Inst::BranchReg { link: true, .. })
    }

    fn may_load_or_store(
        &self,
        inst: &Inst,
    ) -> bool {
        matches!(inst, Inst::Load { .. } | Inst::Store { .. } | Inst::LoadPair { .. } | Inst::StorePair { .. })
    }

    fn is_return_or_indirect(
        &self,
        inst: &Inst,
    ) -> bool {
        matches!(inst, Inst::Ret { .. } | Inst::BranchReg { link: false, .. })
    }

    fn can_fall_through(
        &self,
        block: usize,
    ) -> bool {
        self.blocks[block].insts.last().is_none_or(|last| !is_barrier(last))
            && self.layout.last() != Some(&block)
    }

    fn optimize_for_size(
        &self,
        _block: usize,
    ) -> bool {
        self.for_size
    }

    fn analyze_branch(
        &self,
        block: usize,
    ) -> Option<Exits<usize, Inst>> {
        let mut exits = Exits { taken: None, other: None, cond: None };
        let to = |inst: &Inst| label_of(inst).and_then(|label| self.index.get(label)).copied();
        let insts = &self.blocks[block].insts;
        let start = insts.iter().rposition(|inst| !is_terminator(inst)).map_or(0, |at| at + 1);
        for inst in &insts[start..] {
            match inst {
                Inst::Ret { .. } | Inst::Brk { .. } => return Some(exits),
                Inst::BranchReg { .. } => return None,
                Inst::Branch { link: false, .. } if exits.cond.is_none() => exits.taken = Some(to(inst)?),
                Inst::Branch { link: false, .. } => exits.other = Some(to(inst)?),
                cond if is_cond(cond) => {
                    if exits.cond.is_some() {
                        return None;
                    }
                    exits.taken = Some(to(cond)?);
                    exits.cond = Some(cond.clone());
                }
                _ => return None,
            }
        }
        Some(exits)
    }

    fn reverse_branch_condition(
        &self,
        cond: &Inst,
    ) -> Option<Inst> {
        // The target is put right by `insert_branch`.
        match cond {
            Inst::BranchCond { cond, target } => Some(Inst::BranchCond { cond: cond.inverted(), target: target.clone() }),
            Inst::CompareBranch { nonzero, width, rt, target } => {
                Some(Inst::CompareBranch { nonzero: !nonzero, width: *width, rt: *rt, target: target.clone() })
            }
            Inst::TestBranch { nonzero, rt, bit, target } => {
                Some(Inst::TestBranch { nonzero: !nonzero, rt: *rt, bit: *bit, target: target.clone() })
            }
            _ => None,
        }
    }

    fn remove_branch(
        &mut self,
        block: usize,
    ) {
        while self.blocks[block].insts.last().is_some_and(|last| is_cond(last) || matches!(last, Inst::Branch { link: false, .. })) {
            self.blocks[block].insts.pop();
            self.blocks[block].locs.pop();
        }
        *self.preds.borrow_mut() = None;
    }

    fn insert_branch(
        &mut self,
        block: usize,
        taken: usize,
        other: Option<usize>,
        cond: Option<&Inst>,
    ) {
        let name = |at: usize| self.blocks[at].label.clone();
        let mut add: Vec<Inst> = Vec::new();
        match cond {
            Some(cond) => {
                add.push(retargeted(cond, &name(taken)));
                if let Some(other) = other {
                    add.push(jump(&name(other)));
                }
            }
            None => add.push(jump(&name(taken))),
        }
        for inst in add {
            self.blocks[block].insts.push(inst);
            self.blocks[block].locs.push(None);
        }
        *self.preds.borrow_mut() = None;
    }

    fn split_block_at(
        &mut self,
        block: usize,
        at: usize,
    ) -> Option<usize> {
        let new = self.blocks.len();
        self.made += 1;
        let label = format!("{}.m{}", self.blocks[block].label, self.made);
        let insts = self.blocks[block].insts.split_off(at);
        let locs = self.blocks[block].locs.split_off(at);
        self.blocks.push(Lowered { label: label.clone(), insts, locs });
        self.index.insert(label.clone(), new);
        let place = self.position(block) + 1;
        self.layout.insert(place, new);
        self.blocks[block].insts.push(jump(&label));
        self.blocks[block].locs.push(None);
        *self.preds.borrow_mut() = None;
        Some(new)
    }

    fn replace_tail_with_branch_to(
        &mut self,
        block: usize,
        at: usize,
        dest: usize,
    ) {
        let label = self.blocks[dest].label.clone();
        self.blocks[block].insts.truncate(at);
        self.blocks[block].locs.truncate(at);
        self.blocks[block].insts.push(jump(&label));
        self.blocks[block].locs.push(None);
        *self.preds.borrow_mut() = None;
    }
}

/// Whether any tail was shared; the blocks are in layout order after.
pub fn tail_merge(
    blocks: Vec<Lowered>,
    tables: &[Table],
    for_size: bool,
) -> (Vec<Lowered>, bool) {
    if blocks.len() < 3 || !tables.is_empty() {
        return (blocks, false);
    }
    let layout = (0..blocks.len()).collect();
    let index = blocks.iter().enumerate().map(|(at, block)| (block.label.clone(), at)).collect();
    let mut tails = Tails { blocks, layout, index, tables, for_size, preds: RefCell::new(None), made: 0 };
    let changed = tail_merge_blocks(&mut tails);
    let _ = tails.tables;
    // `fold` places blocks itself and needs every fall-through spelled out.
    for pair in tails.layout.windows(2) {
        let (from, next) = (pair[0], pair[1]);
        if tails.blocks[from].insts.last().is_none_or(|last| !is_barrier(last)) {
            let to = jump(&tails.blocks[next].label);
            tails.blocks[from].insts.push(to);
            tails.blocks[from].locs.push(None);
        }
    }
    let Tails { blocks, layout, .. } = tails;
    let mut slots: Vec<Option<Lowered>> = blocks.into_iter().map(Some).collect();
    let ordered = layout.into_iter().map(|at| slots[at].take().expect("a block once")).collect();
    (ordered, changed)
}
