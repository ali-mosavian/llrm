//! The stores that reach a load of a frame slot nothing else can write.
//!
//! # How it works
//! A frame object whose address is not exposed (`frameescape`) is accessed by
//! loads and stores named by its own address alone, so no call and no store
//! through another pointer can change it. One whose address is exposed can
//! also be changed by a call that may write and by a store through a pointer
//! of unknown root; a store through another object's address cannot. The
//! state of such a slot is the state of its cells, which a store sets and an
//! overlapping store or one of those writes kills: the memory SSA of a single
//! object, with no pointer fact needed to build it. For each load of a cell it
//! names the stores whose value the load may read: every store on the paths
//! that reach it, where every path has stored the cell exactly and none since
//! wrote another part of it. That is the exact content of a spill slot, which
//! the pointer solve takes as a copy of what was stored instead of everything
//! stored into the object anywhere.
//!
//! Each cell a pointer load names is one variable, put in SSA form as
//! `PromoteMemToReg` does (phis at the iterated dominance frontier of its
//! stores, renamed down the dominator tree): its definitions are the stores of
//! exactly that cell, and a store of another, overlapping part of the slot, a
//! store at an index the body does not know and the entry are definitions of
//! nothing. A load's stores are those the phis it reads join, and it has none
//! when any path reaches it with nothing stored.
//!
//! # Example
//! A pointer spilled and reloaded, with a second store on one arm:
//!
//! ```mir
//! define void @f(i1 %c, ptr %p, ptr %q) {
//! b0:
//!   %slot = alloca ptr
//!   store ptr %p, ptr %slot
//!   br i1 %c, label %b1, label %b2
//! b1:
//!   store ptr %q, ptr %slot
//!   br label %b2
//! b2:
//!   %r = load ptr, ptr %slot
//!   ret void
//! }
//! ```
//!
//! `%r` is read from either store: `%p` or `%q`.
//!
//! # Provides
//! `reaching`: for each load asked, its stores, or none where its source is
//! not exact. Consumer: [`crate::alias`] (`point_values`).
//!
//! # Needs
//! [`crate::memory`] (`MemRef`, `Unit::exposed`), [`crate::cfg`].
//!
//! # See also
//! LLVM `PromoteMemToReg`
//! (`llvm/lib/Transforms/Utils/PromoteMemoryToRegister.cpp`)
//! and gcc `execute_update_addresses_taken` for the same slots.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::ConstantKind;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::{HashMap, HashSet};

use crate::cfg;
use crate::memory::{MemRef, Unit};

/// A cell of a slot: its bytes, `low..high`.
type Cell = (i64, i64);

/// What a variable holds at a point.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Token {
    /// The value of this store.
    Store(InstId),
    /// Nothing known: the entry, or a write that left the cell no value.
    Nothing,
    /// The join in block `.0` of the variable `.1`.
    Phi(i64, usize),
}

/// What one instruction does to the slots.
enum Event {
    /// A store to the slot: of exactly the cell, or an overlapping one, or at
    /// an unknown index.
    Store {
        slot: ValueId,
        cell: Option<Cell>,
        inst: InstId,
    },
    Load {
        slot: ValueId,
        cell: Cell,
        inst: InstId,
    },
    /// A write to memory of no known object: it changes every exposed slot.
    Smash,
}

/// The stores that may supply each of `loads`, where every path to the load
/// has stored its cell and nothing that may write the slot lies between. A
/// load absent from the result has no exact source.
pub fn reaching(
    unit: &Unit,
    loads: &IdSet<InstId>,
) -> IdMap<InstId, Vec<InstId>> {
    let found = ssa_reaching(unit, loads, false);
    if llrm_support::env_set("LLRM_CHECK_SLOTS") {
        let by_dataflow = ssa_reaching(unit, loads, true);
        assert!(
            found == by_dataflow,
            "LLRM_CHECK_SLOTS: the stores that reach a load of a slot, found by SSA form, are not those a dataflow over the blocks finds"
        );
    }
    found
}

/// `reaching`, by SSA form, or where `dataflow`, by iterating the state of each
/// slot's cells over the blocks until it settles: the reference the check
/// compares with.
fn ssa_reaching(
    unit: &Unit,
    loads: &IdSet<InstId>,
    dataflow: bool,
) -> IdMap<InstId, Vec<InstId>> {
    let mut found = IdMap::new();
    if loads.is_empty() {
        return found;
    }
    let function = unit.function;
    let graph = cfg::graph(function);
    let dominance = &unit.shape().dominance;
    let derived;
    let exposed = match unit.exposed {
        Some(exposed) => exposed,
        None => {
            derived = crate::memory::exposed_frames(unit);
            &derived
        }
    };
    // The accesses of every slot in order, by block, with what may write one.
    let mut by_block: BTreeMap<i64, Vec<Event>> = BTreeMap::new();
    let mut cells: IdMap<ValueId, BTreeSet<Cell>> = IdMap::new();
    for block in &graph {
        let events = by_block.entry(block.at).or_default();
        for &inst in function.block(cfg::block(block.at)).instructions() {
            let op = function.instruction(inst);
            match op.opcode {
                Opcode::Load { .. } | Opcode::Store { .. } => {
                    let is_load = matches!(op.opcode, Opcode::Load { .. });
                    let Some(reference) = MemRef::of(unit, inst) else {
                        if !is_load {
                            events.push(Event::Smash);
                        }
                        continue;
                    };
                    let slot = match reference.root {
                        Some(Operand::Value(slot)) if is_alloca(unit, slot) => slot,
                        root => {
                            // Another object's address is no slot's; any other
                            // pointer may be.
                            let elsewhere = matches!(
                                root,
                                Some(Operand::Constant(id)) if matches!(unit.context.get(id).kind, ConstantKind::Global(_))
                            );
                            if !is_load && !elsewhere {
                                events.push(Event::Smash);
                            }
                            continue;
                        }
                    };
                    let cell = (reference.disp, reference.disp + i64::from(reference.width));
                    match (is_load, reference.base.is_some()) {
                        (true, true) => {}
                        (true, false) => {
                            if loads.contains(&inst) {
                                cells.get_or_insert_with(slot, BTreeSet::new).insert(cell);
                                events.push(Event::Load { slot, cell, inst });
                            }
                        }
                        (false, true) => events.push(Event::Store { slot, cell: None, inst }),
                        (false, false) => events.push(Event::Store { slot, cell: Some(cell), inst }),
                    }
                }
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    if !crate::alias::only_reads(unit, inst) {
                        events.push(Event::Smash);
                    }
                }
                _ => {
                    if crate::memory::unmodeled_write(unit, inst) {
                        events.push(Event::Smash);
                    }
                }
            }
        }
    }
    if cells.is_empty() {
        return found;
    }
    // The variables: a cell of a slot that is loaded.
    let mut variables: Vec<(ValueId, Cell)> = Vec::new();
    let mut of_slot: IdMap<ValueId, Vec<usize>> = IdMap::new();
    for (slot, cells) in cells.iter() {
        for &cell in cells {
            of_slot.get_or_insert_with(slot, Vec::new).push(variables.len());
            variables.push((slot, cell));
        }
    }
    let exposed_variables: Vec<usize> =
        (0..variables.len()).filter(|&variable| exposed.contains(&variables[variable].0)).collect();
    let overlaps = |one: Cell, other: Cell| one.0 < other.1 && other.0 < one.1;
    if dataflow {
        return by_dataflow(unit, &by_block, &variables, &exposed_variables);
    }
    // The blocks that define each variable, to place its phis.
    let mut defined: Vec<BTreeSet<i64>> = vec![BTreeSet::new(); variables.len()];
    for (&at, events) in &by_block {
        for event in events {
            match event {
                Event::Store { slot, cell, .. } => {
                    for &variable in of_slot.get(slot).into_iter().flatten() {
                        if cell.is_none_or(|cell| overlaps(cell, variables[variable].1)) {
                            defined[variable].insert(at);
                        }
                    }
                }
                Event::Smash => exposed_variables.iter().for_each(|&variable| {
                    defined[variable].insert(at);
                }),
                Event::Load { .. } => {}
            }
        }
    }
    let entry = graph.first().map(|first| first.at);
    let frontiers = dominance.frontiers(function);
    // Phis at the iterated dominance frontier of the definitions and the entry.
    let mut phis: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (variable, blocks) in defined.iter().enumerate() {
        let mut work: Vec<i64> = blocks.iter().copied().chain(entry).collect();
        let mut placed: BTreeSet<i64> = BTreeSet::new();
        let mut seen: BTreeSet<i64> = work.iter().copied().collect();
        while let Some(at) = work.pop() {
            for &join in frontiers.get(&at).into_iter().flatten() {
                if placed.insert(join) {
                    phis.entry(join).or_default().push(variable);
                    if seen.insert(join) {
                        work.push(join);
                    }
                }
            }
        }
    }
    // Rename down the dominator tree.
    let successors: HashMap<i64, &Vec<i64>> = graph.iter().map(|block| (block.at, &block.succ)).collect();
    let mut children: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    for block in &graph {
        if let Some(parent) = dominance.immediate(block.at) {
            children.entry(parent).or_default().push(block.at);
        }
    }
    let mut current: Vec<Token> = vec![Token::Nothing; variables.len()];
    let mut inputs: HashMap<(i64, usize), Vec<Token>> = HashMap::default();
    let mut loaded: Vec<(InstId, Token)> = Vec::new();
    // A block is entered, then left to undo what it wrote.
    let mut stack: Vec<(i64, Option<Vec<(usize, Token)>>)> = entry.map(|first| (first, None)).into_iter().collect();
    while let Some((at, undo)) = stack.pop() {
        if let Some(undo) = undo {
            for (variable, token) in undo.into_iter().rev() {
                current[variable] = token;
            }
            continue;
        }
        let mut undo: Vec<(usize, Token)> = Vec::new();
        for &variable in phis.get(&at).into_iter().flatten() {
            undo.push((variable, current[variable]));
            current[variable] = Token::Phi(at, variable);
        }
        for event in by_block.get(&at).into_iter().flatten() {
            match event {
                Event::Store { slot, cell, inst } => {
                    for &variable in of_slot.get(slot).into_iter().flatten() {
                        let exact = cell.is_some_and(|cell| cell == variables[variable].1);
                        if exact || cell.is_none_or(|cell| overlaps(cell, variables[variable].1)) {
                            undo.push((variable, current[variable]));
                            current[variable] = if exact { Token::Store(*inst) } else { Token::Nothing };
                        }
                    }
                }
                Event::Smash => {
                    for &variable in &exposed_variables {
                        undo.push((variable, current[variable]));
                        current[variable] = Token::Nothing;
                    }
                }
                Event::Load { slot, cell, inst } => {
                    let variable = of_slot[slot].iter().copied().find(|&variable| variables[variable].1 == *cell);
                    if let Some(variable) = variable {
                        loaded.push((*inst, current[variable]));
                    }
                }
            }
        }
        for &successor in successors.get(&at).copied().into_iter().flatten() {
            for &variable in phis.get(&successor).into_iter().flatten() {
                inputs.entry((successor, variable)).or_default().push(current[variable]);
            }
        }
        stack.push((at, Some(undo)));
        for &child in children.get(&at).into_iter().flatten() {
            stack.push((child, None));
        }
    }
    // What a token comes to: the stores of every path, or nothing where a path
    // has stored none.
    for (load, token) in loaded {
        let mut stores: IdSet<InstId> = IdSet::new();
        let mut seen: HashSet<Token> = HashSet::default();
        let mut work = vec![token];
        let mut exact = true;
        while let Some(token) = work.pop() {
            if !seen.insert(token) {
                continue;
            }
            match token {
                Token::Store(store) => {
                    stores.insert(store);
                }
                Token::Nothing => {
                    exact = false;
                    break;
                }
                Token::Phi(at, variable) => work.extend(inputs.get(&(at, variable)).into_iter().flatten().copied()),
            }
        }
        if exact && !stores.is_empty() {
            found.insert(load, stores.iter().collect());
        }
    }
    found
}

/// Whether `value` is an alloca's address.
fn is_alloca(
    unit: &Unit,
    value: ValueId,
) -> bool {
    matches!(
        unit.function.value(value).def,
        ValueDef::Instruction(def) if matches!(unit.function.instruction(def).opcode, Opcode::Alloca { .. })
    )
}

/// What a variable holds on entry to a block, by the dataflow: not yet
/// reached, nothing, or the stores of every path.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Held {
    Unreached,
    Nothing,
    Stores(Vec<InstId>),
}

/// The stores that reach each load, by a forward dataflow over the blocks per
/// variable: a join of a predecessor with nothing is nothing, of two sets
/// their union.
fn by_dataflow(
    unit: &Unit,
    by_block: &BTreeMap<i64, Vec<Event>>,
    variables: &[(ValueId, Cell)],
    exposed_variables: &[usize],
) -> IdMap<InstId, Vec<InstId>> {
    let graph = cfg::graph(unit.function);
    let predecessors = llrm_support::graph::predecessors(&graph);
    let overlaps = |one: Cell, other: Cell| one.0 < other.1 && other.0 < one.1;
    let mut found = IdMap::new();
    for (variable, &(slot, cell)) in variables.iter().enumerate() {
        let mut out: HashMap<i64, Held> = graph.iter().map(|block| (block.at, Held::Unreached)).collect();
        let step = |held: &mut Held, event: &Event| match event {
            Event::Store { slot: written, cell: stored, inst } if *written == slot => {
                if *stored == Some(cell) {
                    *held = Held::Stores(vec![*inst]);
                } else if stored.is_none_or(|stored| overlaps(stored, cell)) {
                    *held = Held::Nothing;
                }
            }
            Event::Smash if exposed_variables.contains(&variable) => *held = Held::Nothing,
            _ => {}
        };
        let enter = |at: i64, out: &HashMap<i64, Held>| {
            if graph.first().is_some_and(|first| first.at == at) {
                return Held::Nothing;
            }
            let mut joined = Held::Unreached;
            for parent in predecessors.get(&at).into_iter().flatten() {
                joined = match (joined, &out[parent]) {
                    (held, Held::Unreached) => held,
                    (Held::Unreached, other) => other.clone(),
                    (Held::Nothing, _) | (_, Held::Nothing) => Held::Nothing,
                    (Held::Stores(mut one), Held::Stores(more)) => {
                        one.extend(more.iter().copied());
                        one.sort_unstable_by_key(|store| store.0);
                        one.dedup();
                        Held::Stores(one)
                    }
                };
            }
            joined
        };
        for _ in 0..graph.len() + 4 {
            let mut changed = false;
            for block in &graph {
                let mut held = enter(block.at, &out);
                for event in by_block.get(&block.at).into_iter().flatten() {
                    step(&mut held, event);
                }
                if out[&block.at] != held {
                    out.insert(block.at, held);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for block in &graph {
            let mut held = enter(block.at, &out);
            for event in by_block.get(&block.at).into_iter().flatten() {
                if let Event::Load { slot: loaded, cell: at, inst } = event {
                    if *loaded == slot && *at == cell {
                        if let Held::Stores(stores) = &held {
                            found.insert(*inst, stores.clone());
                        }
                    }
                }
                step(&mut held, event);
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "slotstores_tests.rs"]
mod tests;
