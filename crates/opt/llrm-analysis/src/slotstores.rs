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
//! [`crate::basicaa`] (`object_cell`), [`crate::memory`] (`Unit::exposed`),
//! [`crate::cfg`].
//!
//! # See also
//! LLVM `PromoteMemToReg`
//! (`llvm/lib/Transforms/Utils/PromoteMemoryToRegister.cpp`)
//! and gcc `execute_update_addresses_taken` for the same slots.

use std::collections::BTreeSet;

use llrm_mir::context::ConstantKind;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::{HashMap, HashSet};

use crate::basicaa::object_cell;
use crate::cfg;
use crate::memory::Unit;

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
    if llrm_support::env_flag!("LLRM_CHECK_SLOTS") {
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
    // The slots loaded: a pointer load of a cell of an alloca names one.
    let mut cells: IdMap<ValueId, BTreeSet<Cell>> = IdMap::new();
    let mut loaded: IdMap<InstId, (ValueId, Cell)> = IdMap::new();
    for load in loads.iter() {
        let Some(place) = object_cell(unit, load) else { continue };
        let Operand::Value(slot) = place.object else { continue };
        if !is_alloca(unit, slot) || place.indexed {
            continue;
        }
        let cell = (place.offset, place.offset + place.size as i64);
        cells.get_or_insert_with(slot, BTreeSet::new).insert(cell);
        loaded.insert(load, (slot, cell));
    }
    // A slot nothing exposes is written only by what names it, and a store
    // through a phi or select of addresses, or one a scan of the root does not
    // resolve, names it without this scan seeing it: its loads have no exact
    // source.
    let tainted: Vec<ValueId> = cells
        .iter()
        .map(|(slot, _)| slot)
        .filter(|&slot| !exposed.contains(&slot) && !named_plainly(unit, slot))
        .collect();
    for slot in &tainted {
        cells.remove(slot);
    }
    if cells.is_empty() {
        return found;
    }
    // Only an exposed slot is written by what its address does not name.
    let smashes = cells.keys().any(|slot| exposed.contains(&slot));
    // The accesses of every slot loaded, in order, by block.
    let mut by_block: IdMap<i64, Vec<Event>> = IdMap::new();
    for block in &graph {
        let events = by_block.get_or_insert_with(block.at, Vec::new);
        for inst in function.iter_block(cfg::block(block.at)) {
            let op = function.instruction(inst);
            match op.opcode {
                Opcode::Load { .. } => {
                    if let Some(&(slot, cell)) = loaded.get(&inst).filter(|(slot, _)| cells.contains_key(slot)) {
                        events.push(Event::Load { slot, cell, inst });
                    }
                }
                Opcode::Store { .. } => match crate::memory::root_of(unit, op.operands[1]) {
                    Operand::Value(slot) if is_alloca(unit, slot) => {
                        if !cells.contains_key(&slot) {
                            continue;
                        }
                        // The cell it writes, or any of the slot where its
                        // address is not an offset of
                        // the slot's own.
                        let cell = object_cell(unit, inst)
                            .filter(|place| place.object == Operand::Value(slot) && !place.indexed)
                            .map(|place| (place.offset, place.offset + place.size as i64));
                        events.push(Event::Store { slot, cell, inst });
                    }
                    // Another object's address is no slot's.
                    Operand::Constant(id) if matches!(unit.context.get(id).kind, ConstantKind::Global(_)) => {}
                    _ => {
                        if smashes {
                            events.push(Event::Smash);
                        }
                    }
                },
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    if smashes && !crate::alias::only_reads(unit, inst) {
                        events.push(Event::Smash);
                    }
                }
                _ => {
                    if smashes && crate::memory::unmodeled_write(unit, inst) {
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
    let mut defined: Vec<IdSet<i64>> = vec![IdSet::new(); variables.len()];
    for (at, events) in by_block.iter() {
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
    let mut phis: IdMap<i64, Vec<usize>> = IdMap::new();
    for (variable, blocks) in defined.iter().enumerate() {
        let mut work: Vec<i64> = blocks.iter().chain(entry).collect();
        let mut placed: IdSet<i64> = IdSet::new();
        let mut seen: IdSet<i64> = work.iter().copied().collect();
        while let Some(at) = work.pop() {
            for &join in frontiers.get(&at).into_iter().flatten() {
                if placed.insert(join) {
                    phis.get_or_insert_with(join, Vec::new).push(variable);
                    if seen.insert(join) {
                        work.push(join);
                    }
                }
            }
        }
    }
    // Rename down the dominator tree.
    let successors: IdMap<i64, &cfg::Successors> = graph.iter().map(|block| (block.at, &block.succ)).collect();
    let mut children: IdMap<i64, Vec<i64>> = IdMap::new();
    for block in &graph {
        if let Some(parent) = dominance.immediate(unit.function, block.at) {
            children.get_or_insert_with(parent, Vec::new).push(block.at);
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

/// Whether every load and store the address of the slot `slot` reaches, through
/// offsets, casts, phis and selects, names it as its own root: where a phi or a
/// select joins its address with another's, or an access is of an address the
/// root of which the scan does not resolve, the slot is written by what does
/// not name it.
fn named_plainly(
    unit: &Unit,
    slot: ValueId,
) -> bool {
    let function = unit.function;
    let mut seen = llrm_mir::dense::IdSet::<ValueId>::new();
    let mut work = vec![slot];
    while let Some(value) = work.pop() {
        if !seen.insert(value) {
            continue;
        }
        for one in function.users(value) {
            let op = function.instruction(one.user);
            match &op.opcode {
                Opcode::Phi | Opcode::Select => return false,
                Opcode::GetElementPtr { .. } | Opcode::Cast(_) | Opcode::Freeze if one.index == 0 => {
                    work.extend(op.result);
                }
                Opcode::Load { .. } | Opcode::Store { .. }
                    if one.index as usize == usize::from(matches!(op.opcode, Opcode::Store { .. })) =>
                {
                    let named = object_cell(unit, one.user).is_some_and(|place| place.object == Operand::Value(slot));
                    if !named {
                        return false;
                    }
                }
                _ => {}
            }
        }
    }
    true
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
    by_block: &IdMap<i64, Vec<Event>>,
    variables: &[(ValueId, Cell)],
    exposed_variables: &[usize],
) -> IdMap<InstId, Vec<InstId>> {
    let graph = cfg::graph(unit.function);
    let predecessors = llrm_support::graph::predecessors(&graph);
    let overlaps = |one: Cell, other: Cell| one.0 < other.1 && other.0 < one.1;
    let mut found = IdMap::new();
    for (variable, &(slot, cell)) in variables.iter().enumerate() {
        let mut out: IdMap<i64, Held> = graph.iter().map(|block| (block.at, Held::Unreached)).collect();
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
        let enter = |at: i64, out: &IdMap<i64, Held>| {
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
