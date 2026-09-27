//! LLVM's GVN load PRE at joins: llrm-core's `optimize/loadjoins.rs`, the
//! port of `qbopt/optimize/loadjoins.py`, adapted to the rich MIR. A load at
//! a join whose bytes every predecessor already holds becomes a phi of those
//! values; with `insert`, a predecessor holding none gets its own load, on
//! its edge, where that speculates nothing.
//!
//! What changed with the IR:
//! - A provider is a load or a store, what it holds a value or a constant,
//!   of the load's type; the old width check is the type.
//! - A missing load goes on either edge of a conditional branch. The old MIR
//!   could split only the explicit (taken) one.
//! - The join's prefix may hold what `llrm_mir::memory::only_value` allows,
//!   loads aside, where the old one allowed moves. Division is C's, so the
//!   old guard against a trapping one has no MIR meaning.
//! - An address is translated on an edge when the load's pointer is a phi
//!   of the join, as when the old pointer reference's base was.
//! - Dropped: stack slots, x87 operations, `merges`, and the `symbol`,
//!   `source_backed` and `raised` marks.
//!
//! A call's footprint is its `CallEffects`: the pass needs `Summaries`
//! required.

use std::collections::BTreeMap;
use std::rc::Rc;

use llrm_analysis::avail::{loaded_into, stored_from};
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::memoryssa::{self, Accesses, same_bytes};
use llrm_analysis::{cfg, ssa};
use llrm_graph::loops::{self, Dominance, Loop};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::{Callees, only_value};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_support::hash::HashMap;

use crate::edges;

pub struct LoadJoins {
    /// Whether a predecessor holding no value gets a load of its own.
    pub insert: bool,
}

impl FunctionPass for LoadJoins {
    fn name(&self) -> &'static str {
        "loadjoins"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let (context, layout) = (&*unit.context, unit.layout);
        let changed = Accesses::managed(context, layout, unit.function, analyses)
            .and_then(|accesses| reused(context, layout, unit.function, analyses.outer(), unit.callees, &accesses, self.insert));
        match changed {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("loadjoins: {error}"),
        }
    }
}

/// `function`'s join loads made phis, as `accesses` (of `function` as it
/// stands) says what each instruction touches; whether any was.
pub fn reused(context: &Context, layout: &DataLayout, function: &mut Function, outer: &Outer, callees: &Callees, accesses: &Accesses, insert: bool) -> Result<bool, String> {
    let joined = planned(&Unit::within(context, layout, function, outer), callees, accesses, insert);
    if joined.is_empty() {
        return Ok(false);
    }
    applied(function, joined)?;
    Ok(true)
}

/// Where a join load's value comes from on one edge.
#[derive(Debug)]
enum Incoming {
    /// A value the predecessor already holds.
    Held(Operand),
    /// A new load there, through this pointer.
    Loaded(Operand),
}

/// A load at a join, and its value on each incoming edge.
#[derive(Debug)]
struct Joined {
    load: InstId,
    join: BlockId,
    incoming: Vec<(BlockId, Incoming)>,
}

/// The load's pointer on the edge from `parent`: a phi of the join's input
/// from there, else the pointer itself.
fn on_edge(function: &Function, load: InstId, parent: BlockId) -> Option<Operand> {
    let pointer = function.instruction(load).operands[0];
    let join = function.parent(load)?;
    let Operand::Value(value) = pointer else { return Some(pointer) };
    match function.value(value).def {
        ValueDef::Instruction(phi) if function.parent(phi) == Some(join) && function.instruction(phi).opcode == Opcode::Phi => {
            function.instruction(phi).operands.chunks(2).find(|arm| arm[1] == Operand::Block(parent)).map(|arm| arm[0])
        }
        _ => Some(pointer),
    }
}

/// What a load or store at `inst` leaves in its cell.
fn provided(unit: &Unit, accesses: &Accesses, inst: InstId) -> Option<(MemRef, Operand)> {
    stored_from(unit, accesses, inst).or_else(|| loaded_into(unit, accesses, inst).map(|(cell, value)| (cell, Operand::Value(value))))
}

/// The CFG facts a join is judged by.
struct Shape {
    dominance: Rc<Dominance>,
    depth: BTreeMap<i64, usize>,
    loops: Vec<Loop>,
}

/// Whether a load through `pointer` can go on the edge `parent` to `join`,
/// in front of `index` instructions of the join: nothing there touches
/// memory or can keep the load from running, the edge is no back edge and
/// crosses no loop boundary, and `pointer` is defined above it.
fn insertable(unit: &Unit, callees: &Callees, shape: &Shape, parent: BlockId, join: BlockId, index: usize, pointer: Operand) -> bool {
    let function = unit.function;
    let (from, to) = (cfg::id(parent), cfg::id(join));
    let branch = function.terminator(parent).is_some_and(|one| function.instruction(one).opcode == Opcode::Br);
    if !branch || (function.successors(parent) != [join] && !edges::conditional(function, parent, join)) {
        return false;
    }
    if shape.dominance.dominates(to, from) || shape.loops.iter().any(|one| one.body.contains(&from) != one.body.contains(&to)) {
        return false;
    }
    let prefix = &function.block(join).instructions()[..index];
    if prefix.iter().any(|&prior| !only_value(unit.context, callees, function, prior) || matches!(function.instruction(prior).opcode, Opcode::Load { .. })) {
        return false;
    }
    match pointer {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Argument(_) => true,
            ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| shape.dominance.dominates(cfg::id(block), from)),
        },
        Operand::Constant(_) => true,
        Operand::Block(_) => false,
    }
}

/// Every join load to replace by a phi, decided on `unit` as it stands.
fn planned(unit: &Unit, callees: &Callees, accesses: &Accesses, insert: bool) -> Vec<Joined> {
    let function = unit.function;
    let graph = cfg::graph(function);
    let entry = function.entry().map(cfg::id);
    let predecessors = loops::predecessors(&graph);
    if !predecessors.values().any(|parents| parents.len() > 1) {
        return Vec::new();
    }
    let memory = memoryssa::built(unit, accesses);
    let places: HashMap<InstId, (i64, usize)> =
        function.layout().iter().flat_map(|&block| function.block(block).instructions().iter().enumerate().map(move |(index, &inst)| (inst, (cfg::id(block), index)))).collect();
    let providers: Vec<(InstId, MemRef, Operand)> = memory.sites.keys().filter_map(|&site| provided(unit, accesses, site).map(|(cell, value)| (site, cell, value))).collect();
    let shape = Shape {
        dominance: loops::dominance(&graph, entry),
        depth: loops::dominators(&graph, entry).into_iter().map(|(at, above)| (at, above.len())).collect(),
        loops: loops::loops(&graph, entry),
    };

    let mut found = Vec::new();
    for block in &graph {
        let parents = &predecessors[&block.at];
        if Some(block.at) == entry || parents.len() < 2 {
            continue;
        }
        let join = cfg::block(block.at);
        for (index, &load) in function.block(join).instructions().iter().enumerate() {
            let Some((reference, result)) = loaded_into(unit, accesses, load) else {
                continue;
            };
            let ty = function.value(result).ty;
            let mut incoming = Vec::new();
            for &parent in parents {
                let Some(pointer) = on_edge(function, load, cfg::block(parent)) else {
                    break;
                };
                let translated = MemRef { typed: reference.typed.clone(), ..MemRef::at(unit, pointer, reference.width) };
                let candidates = providers.iter().filter(|(source, cell, value)| {
                    let (at, _) = places[source];
                    at != block.at
                        && shape.dominance.dominates(at, parent)
                        && !shape.dominance.dominates(at, block.at)
                        && unit.operand_type(*value) == Some(ty)
                        && same_bytes(unit, cell, &translated)
                        && shape.loops.iter().all(|one| !one.body.contains(&at) || one.body.contains(&block.at))
                        && memory.available_on_edge(*source, load, parent, &reference, Some(&translated))
                });
                // The deepest source, the latest in its block; the first of equals.
                let best = candidates.fold(None::<&(InstId, MemRef, Operand)>, |best, one| match best {
                    Some(best) if (shape.depth[&places[&one.0].0], places[&one.0].1) <= (shape.depth[&places[&best.0].0], places[&best.0].1) => Some(best),
                    _ => Some(one),
                });
                match best {
                    Some((_, _, value)) => incoming.push((cfg::block(parent), Incoming::Held(*value))),
                    None if insert && insertable(unit, callees, &shape, cfg::block(parent), join, index, pointer) => incoming.push((cfg::block(parent), Incoming::Loaded(pointer))),
                    None => break,
                }
            }
            if incoming.len() == parents.len() && incoming.iter().any(|(_, one)| matches!(one, Incoming::Held(_))) {
                found.push(Joined { load, join, incoming });
            }
        }
    }
    found
}

/// `joined` made: each load a phi of its edges' values.
fn applied(function: &mut Function, joined: Vec<Joined>) -> Result<(), String> {
    // A load already replaced names its phi.
    let mut replaced: BTreeMap<ValueId, Operand> = BTreeMap::new();
    let resolved = |replaced: &BTreeMap<ValueId, Operand>, one: Operand| ssa::provider(one, replaced).expect("a phi replaces a load once");
    let mut bridges: HashMap<(BlockId, BlockId), BlockId> = HashMap::default();
    for Joined { load, join, incoming } in joined {
        let mut arms = Vec::new();
        for (parent, one) in incoming {
            let (from, value) = match one {
                Incoming::Held(value) => (bridges.get(&(parent, join)).copied().unwrap_or(parent), resolved(&replaced, value)),
                Incoming::Loaded(pointer) => {
                    let copy = function.clone_instruction(load);
                    function.set_operand(copy, 0, resolved(&replaced, pointer));
                    let from = match bridges.get(&(parent, join)) {
                        _ if function.successors(parent) == [join] => {
                            function.insert(copy, Position::Before(function.terminator(parent).expect("a branch")))?;
                            parent
                        }
                        Some(&bridge) => {
                            function.insert(copy, Position::Before(function.terminator(bridge).expect("a jump")))?;
                            bridge
                        }
                        None => {
                            let bridge = edges::split(function, parent, join, vec![copy])?;
                            bridges.insert((parent, join), bridge);
                            bridge
                        }
                    };
                    (from, Operand::Value(function.instruction(copy).result.expect("a load's value")))
                }
            };
            arms.extend([value, Operand::Block(from)]);
        }
        let result = function.instruction(load).result.expect("a load's value");
        let (ty, name) = (function.value(result).ty, function.value(result).name.clone());
        let phi = function.create_instruction(Opcode::Phi, ty, arms, Flags::default(), name.as_deref());
        function.insert(phi, Position::Before(function.block(join).instructions()[0]))?;
        let value = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        function.replace_all_uses_with(result, value);
        function.erase(load)?;
        replaced.insert(result, value);
    }
    Ok(())
}

#[cfg(test)]
#[path = "loadjoins_tests.rs"]
mod tests;
