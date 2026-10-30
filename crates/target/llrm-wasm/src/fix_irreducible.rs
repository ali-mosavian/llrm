//! `WebAssemblyFixIrreducibleControlFlow`: a loop with several entries gets one, a block that
//! dispatches (`br_table`) on a label register to each former entry; the edges into the entries
//! become a set of the label and a branch to the dispatch block.
//!
//! # How it works
//! A region is a set of blocks with one entry and no branch back to it. Its strongly connected
//! components are found, and each loop whose entries (its blocks with a predecessor outside it)
//! are mutually reachable gets a dispatch block; this repeats until none is left, then each loop is
//! a region of its own. Blocks are numbered in the order they were made, not by layout, until the
//! end renumbers them.
//!
//! DEVIATE: a block's predecessors come in the order of their numbers, not in the order their
//! branches were added; a block's loop entries are visited in order of number. Reason: the pin's
//! orders are those of pointer sets and of the instruction selector's visit. Exit: none; only the
//! numbers of the new blocks, and so the order of the routing blocks at the end, can differ.
//!
//! # See also
//! `WebAssemblyFixIrreducibleControlFlow.cpp` at the pin d1106deb71cc: `makeSingleEntryLoop`
//! (L275), `processRegion` (L426), `addImplicitDefs` (L524).

use std::collections::{BTreeMap, BTreeSet};

use crate::dagisel::generated::ids::opcode as op;
use crate::mir::{Function, IMPLICIT_DEF, Inst, Operand, ValType, is_argument};

type Blocks = BTreeSet<usize>;

/// `ReachabilityGraph`: the SCC of each block of a region, and the loop entries.
struct Graph {
    scc: BTreeMap<usize, usize>,
    loop_entries: Blocks,
    entries_by_scc: Vec<Blocks>,
}

fn predecessors(f: &Function) -> Vec<Vec<usize>> {
    let mut preds = vec![Vec::new(); f.blocks.len()];
    for (from, to) in f.succs.iter().enumerate() {
        for &block in to {
            preds[block].push(from);
        }
    }
    preds
}

impl Graph {
    /// `ReachabilityGraph::calculate`: Tarjan's walk from `entry` over the branches inside `blocks`
    /// that are not to `entry`.
    fn of(
        f: &Function,
        entry: usize,
        blocks: &Blocks,
    ) -> Graph {
        let succs = |block: usize| -> Vec<usize> {
            f.succs[block].iter().copied().filter(|to| *to != entry && blocks.contains(to)).collect()
        };
        let preds = predecessors(f);
        let mut graph = Graph { scc: BTreeMap::new(), loop_entries: Blocks::new(), entries_by_scc: Vec::new() };
        let components = tarjan(entry, &succs);
        for (id, component) in components.iter().enumerate() {
            for &member in component {
                graph.scc.insert(member, id);
            }
            graph.entries_by_scc.push(Blocks::new());
            let self_loop = component.len() == 1 && succs(component[0]).contains(&component[0]);
            if component.len() > 1 || self_loop {
                for &member in component {
                    if member == entry {
                        continue;
                    }
                    // A predecessor outside the region has no component: it is outside this one.
                    if preds[member].iter().any(|pred| graph.scc.get(pred) != Some(&id)) {
                        graph.loop_entries.insert(member);
                        graph.entries_by_scc[id].insert(member);
                    }
                }
            }
        }
        graph
    }
}

/// The state of [`tarjan`]'s walk.
#[derive(Default)]
struct Walk {
    index: BTreeMap<usize, usize>,
    low: BTreeMap<usize, usize>,
    on_stack: Blocks,
    stack: Vec<usize>,
    /// The block, its successors and how many of them were taken.
    work: Vec<(usize, Vec<usize>, usize)>,
}

impl Walk {
    fn open(
        &mut self,
        block: usize,
        children: Vec<usize>,
    ) {
        self.index.insert(block, self.index.len());
        self.low.insert(block, self.index.len() - 1);
        self.stack.push(block);
        self.on_stack.insert(block);
        self.work.push((block, children, 0));
    }

    fn lower(
        &mut self,
        block: usize,
        to: usize,
    ) {
        let lowered = self.low[&block].min(to);
        self.low.insert(block, lowered);
    }
}

/// The strongly connected components reachable from `entry`, each after the ones it reaches
/// (`scc_iterator`'s order).
fn tarjan(
    entry: usize,
    succs: &dyn Fn(usize) -> Vec<usize>,
) -> Vec<Vec<usize>> {
    let mut walk = Walk::default();
    let mut components: Vec<Vec<usize>> = Vec::new();
    walk.open(entry, succs(entry));
    while let Some((block, children, next)) = walk.work.last_mut() {
        let block = *block;
        if let Some(&child) = children.get(*next) {
            *next += 1;
            if !walk.index.contains_key(&child) {
                walk.open(child, succs(child));
            } else if walk.on_stack.contains(&child) {
                walk.lower(block, walk.index[&child]);
            }
            continue;
        }
        walk.work.pop();
        if let Some(&(parent, _, _)) = walk.work.last() {
            walk.lower(parent, walk.low[&block]);
        }
        if walk.low[&block] == walk.index[&block] {
            let mut component = Vec::new();
            while let Some(member) = walk.stack.pop() {
                walk.on_stack.remove(&member);
                component.push(member);
                if member == block {
                    break;
                }
            }
            components.push(component);
        }
    }
    components
}

/// `makeSingleEntryLoop`: a dispatch block for `entries`, and a routing block for the edges into them.
fn make_single_entry_loop(
    f: &mut Function,
    layout: &mut Vec<usize>,
    entries: &Blocks,
    blocks: &mut Blocks,
    graph: &Graph,
) {
    let sorted: Vec<usize> = entries.iter().copied().collect();
    let new_block = |f: &mut Function, layout: &mut Vec<usize>, before: Option<usize>| {
        f.blocks.push(Vec::new());
        f.succs.push(Vec::new());
        let id = f.blocks.len() - 1;
        match before.and_then(|entry| layout.iter().position(|&b| b == entry)) {
            Some(at) => layout.insert(at, id),
            None => layout.push(id),
        }
        id
    };
    let dispatch = new_block(f, layout, None);
    blocks.insert(dispatch);
    let reg = f.new_vreg(ValType::I32);
    let mut table = vec![Operand::Use(reg)];
    let mut indices: BTreeMap<usize, i64> = BTreeMap::new();
    for (index, &entry) in sorted.iter().enumerate() {
        indices.insert(entry, index as i64);
        table.push(Operand::Block(entry as u32));
        f.succs[dispatch].push(entry);
    }
    // A fake default label: `br_table` requires one.
    table.push(*table.last().expect("an entry"));
    f.blocks[dispatch].push(Inst::new(op::BR_TABLE_I32, &table));

    let preds = predecessors(f);
    let all_preds: Vec<usize> =
        sorted.iter().flat_map(|entry| preds[*entry].iter().copied().filter(|pred| *pred != dispatch)).collect();

    // The predecessors inside the loop.
    let mut in_loop = Blocks::new();
    for &pred in &all_preds {
        let pred_scc = graph.scc.get(&pred);
        for entry in &f.succs[pred] {
            if entries.contains(entry) && graph.scc.get(entry) == pred_scc {
                in_loop.insert(pred);
                break;
            }
        }
    }

    let layout_successor = |layout: &[usize], block: usize, of: usize| {
        layout.iter().position(|&b| b == block).and_then(|at| layout.get(at + 1)) == Some(&of)
    };
    let mut layout_pred: BTreeMap<(usize, bool), usize> = BTreeMap::new();
    for &pred in &all_preds {
        let inside = in_loop.contains(&pred);
        for &entry in &f.succs[pred] {
            if entries.contains(&entry) && layout_successor(layout, pred, entry) {
                layout_pred.insert((entry, inside), pred);
            }
        }
    }

    // At most two routing blocks per entry: one for the predecessors outside the loop, one inside.
    let mut routing: BTreeMap<(usize, bool), usize> = BTreeMap::new();
    for &pred in &all_preds {
        let inside = in_loop.contains(&pred);
        for entry in f.succs[pred].clone() {
            if !entries.contains(&entry) || routing.contains_key(&(entry, inside)) {
                continue;
            }
            if layout_pred.get(&(entry, inside)).is_some_and(|other| *other != pred) {
                continue;
            }
            let before = layout_successor(layout, pred, entry).then_some(entry);
            let block = new_block(f, layout, before);
            blocks.insert(block);
            f.blocks[block].push(Inst::new(op::CONST_I32, &[Operand::Def(reg), Operand::Imm(indices[&entry])]));
            f.blocks[block].push(Inst::new(op::BR, &[Operand::Block(dispatch as u32)]));
            f.succs[block].push(dispatch);
            routing.insert((entry, inside), block);
        }
    }

    for &pred in &all_preds {
        let inside = in_loop.contains(&pred);
        for inst in f.blocks[pred].iter_mut().filter(|inst| inst.is_terminator()) {
            for operand in &mut inst.operands {
                if let Operand::Block(target) = operand
                    && indices.contains_key(&(*target as usize))
                {
                    *target = routing[&(*target as usize, inside)] as u32;
                }
            }
        }
        let mut replaced: Vec<usize> = Vec::new();
        for &succ in &f.succs[pred] {
            let next = if entries.contains(&succ) { routing[&(succ, inside)] } else { succ };
            if !replaced.contains(&next) {
                replaced.push(next);
            }
        }
        f.succs[pred] = replaced;
    }
}

/// `processRegion`: whether anything changed.
fn process_region(
    f: &mut Function,
    layout: &mut Vec<usize>,
    entry: usize,
    blocks: &mut Blocks,
) -> bool {
    let mut changed = false;
    loop {
        let graph = Graph::of(f, entry, blocks);
        let mut found = false;
        for &loop_entry in &graph.loop_entries {
            let mutual = &graph.entries_by_scc[graph.scc[&loop_entry]];
            if mutual.len() > 1 {
                make_single_entry_loop(f, layout, &mutual.clone(), blocks, &graph);
                found = true;
                changed = true;
                break;
            }
        }
        // The graph changed: start over before going into the inner loops.
        if found {
            continue;
        }
        for &loop_entry in &graph.loop_entries {
            let scc = graph.scc[&loop_entry];
            let mut inner: Blocks = blocks.iter().copied().filter(|block| graph.scc.get(block) == Some(&scc)).collect();
            changed |= process_region(f, layout, loop_entry, &mut inner);
        }
        return changed;
    }
}

/// `addImplicitDefs`: every register used and not defined by an `ARGUMENT` gets an `IMPLICIT_DEF` at
/// the top of the entry block, for the uses that now have a path with no definition.
fn add_implicit_defs(f: &mut Function) {
    let mut used = vec![false; f.num_vregs()];
    let mut argument = vec![false; f.num_vregs()];
    for inst in f.blocks.iter().flatten() {
        for operand in &inst.operands {
            match *operand {
                Operand::Use(reg) => used[reg as usize] = true,
                Operand::Def(reg) if is_argument(inst.opcode) => argument[reg as usize] = true,
                _ => {}
            }
        }
    }
    for reg in 0..f.num_vregs() {
        if used[reg] && !argument[reg] {
            f.blocks[0].insert(0, Inst::new(IMPLICIT_DEF, &[Operand::Def(reg as u32)]));
        }
    }
    // The arguments move to the top, each in front of the last.
    let mut at = 0;
    while at < f.blocks[0].len() {
        if is_argument(f.blocks[0][at].opcode) {
            let inst = f.blocks[0].remove(at);
            f.blocks[0].insert(0, inst);
        }
        at += 1;
    }
}

/// Whether anything changed.
pub fn fix_irreducible_control_flow(f: &mut Function) -> bool {
    let mut layout: Vec<usize> = (0..f.blocks.len()).collect();
    let mut blocks: Blocks = layout.iter().copied().collect();
    if !process_region(f, &mut layout, 0, &mut blocks) {
        return false;
    }
    // `RenumberBlocks`.
    f.apply_order(&layout);
    add_implicit_defs(f);
    true
}
