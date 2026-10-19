//! Load PRE at joins: a load whose bytes every predecessor already holds
//! becomes a phi of those values, and a predecessor holding none may get a
//! load of its own on its edge.
//!
//! # How it works
//! For each block with two or more predecessors, each load in it is a
//! candidate. For every incoming edge the pass looks for a provider: a store
//! (`avail::stored_from`) or a load (`avail::loaded_into`) of the same bytes
//! (`memoryssa::same_bytes`), of the load's type, holding a value or a
//! constant. A provider is accepted when
//! - it dominates the predecessor but not the join (one that dominates the join
//!   is `gvn`'s plain redundancy);
//! - no loop contains it without containing the join;
//! - nothing on the way from it to the edge may write the bytes (`MemoryDef`s
//!   of `memoryssa`, `available_on_edge`), calls judged by their `CallEffects`.
//!
//! Of several providers the deepest in the dominator tree wins, the latest in
//! its block. When the load's pointer is a phi of the join, the bytes are
//! translated to that phi's input on the edge before the search, and an
//! address so translated carries alias's provenance.
//!
//! The load is replaced by a phi of the providers' values when every edge has
//! one. With `insert`, an edge without a provider may instead get a copy of
//! the load, if at least one edge holds a value (`insertable`):
//! - the edge's source ends in a `br`; it is the only exit of the predecessor
//!   or one side of a conditional branch, which is then split (`edges::split`)
//!   so the other side is not touched;
//! - the edge is no back edge and crosses no loop boundary;
//! - everything before the load in the join is a value that may be computed
//!   anywhere (`llrm_mir::memory::only_value`) and no load;
//! - the pointer is defined above the edge.
//!
//! # Example
//! `@g + 32` is a cell stored in `b0`. The load in `b3` is reached along `b1`,
//! which loads it, and along the edge from `b2`, which does not; with `insert`
//! that edge gets a load of its own. Before:
//!
//! ```mir
//! define i16 @f(i1 %c, i1 %d, i16 %v, i16 %s) {
//! b0:
//!   store i16 %v, ptr getelementptr (i8, ptr @g, i16 32)
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   %y = load i16, ptr getelementptr (i8, ptr @g, i16 32)
//!   br label %b3
//!
//! b2:
//!   br i1 %d, label %b3, label %b4
//!
//! b3:
//!   %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
//!   ret i16 %x
//!
//! b4:
//!   ret i16 0
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! b2:
//!   br i1 %d, label %0, label %b4
//!
//! b3:
//!   %x1 = phi i16 [ %y, %b1 ], [ %1, %0 ]
//!   ret i16 %x1
//!
//! 0:
//!   %1 = load i16, ptr getelementptr (i8, ptr @g, i16 32)
//!   br label %b3
//! ; ... b0, b1 and b4 unchanged
//! ```
//!
//! `b2` also leads to `b4`, so its edge to `b3` is split and the new block `0`
//! loads; `b4` reads nothing new.
//!
//! From unit test `test_missing_conditional_edge_gets_its_own_load_block` in
//! `loadjoins_tests.rs`.
//!
//! # Runs
//! Not a pass of the pipeline: [`crate::gvn`] calls [`reused`] after its
//! value numbering and join PRE, with `insert` set when `pre` is on (gcc's
//! `-ftree-pre`, -O2 and up) and the round made no join PRE. `LoadJoins` is
//! the same step as a function pass, for tests.
//!
//! # Needs
//! `memoryssa::Accesses` and the managed `PointerValues` (a missing `Summaries`
//! means every call is unknown, so less is found), `cfg::Shape`.
//!
//! # Keeps
//! Nothing when it changes a body: a split edge changes the CFG.
//!
//! # See also
//! LLVM's GVN `performLoadPRE`; gcc's PRE of loads, `-ftree-pre`.
//! [`crate::gvn`] describes the surrounding pass.

use std::collections::BTreeMap;

use llrm_analysis::alias::PointValues;
use llrm_analysis::avail::{loaded_into, stored_from};
use llrm_analysis::graph::loops::{self, Loop};
use llrm_analysis::manager::PointerValues;
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::memoryssa::{self, Accesses, same_bytes};
use llrm_analysis::{cfg, ssa};
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

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let (context, layout) = (&*unit.context, unit.layout);
        let pointers = analyses.get::<PointerValues>(context, layout, unit.function);
        let changed = Accesses::managed(context, layout, unit.function, analyses).and_then(|accesses| {
            let pointers = Result::as_ref(&*pointers).map_err(String::clone)?;
            let shape = analyses.get::<llrm_analysis::cfg::Shape>(context, layout, unit.function);
            reused(
                context,
                layout,
                unit.function,
                analyses.outer(),
                analyses.outer().callees(),
                &accesses,
                pointers,
                self.insert,
                &shape,
            )
        });
        match changed {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("loadjoins: {error}"),
        }
    }
}

/// `function`'s join loads made phis, as `accesses` (of `function` as it
/// stands) says what each instruction touches and `pointers` what each
/// pointer points to; whether any was.
pub fn reused(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    callees: &Callees,
    accesses: &Accesses,
    pointers: &PointValues,
    insert: bool,
    shape: &llrm_analysis::cfg::Shape,
) -> Result<bool, String> {
    let joined =
        planned(&Unit::within(context, layout, function, outer).with_shape(shape), callees, accesses, pointers, insert);
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
fn on_edge(
    function: &Function,
    load: InstId,
    parent: BlockId,
) -> Option<Operand> {
    let pointer = function.instruction(load).operands[0];
    let join = function.parent(load)?;
    let Operand::Value(value) = pointer else { return Some(pointer) };
    match function.value(value).def {
        ValueDef::Instruction(phi)
            if function.parent(phi) == Some(join) && function.instruction(phi).opcode == Opcode::Phi =>
        {
            function.instruction(phi).operands.chunks(2).find(|arm| arm[1] == Operand::Block(parent)).map(|arm| arm[0])
        }
        _ => Some(pointer),
    }
}

/// What a load or store at `inst` leaves in its cell.
fn provided(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<(MemRef, Operand)> {
    stored_from(unit, accesses, inst)
        .or_else(|| loaded_into(unit, accesses, inst).map(|(cell, value)| (cell, Operand::Value(value))))
}

/// The CFG facts a join is judged by.
struct Shape {
    dominance: cfg::Dominance,
    depth: BTreeMap<i64, usize>,
    loops: Vec<Loop>,
}

/// Whether a load through `pointer` can go on the edge `parent` to `join`,
/// in front of `index` instructions of the join: nothing there touches
/// memory or can keep the load from running, the edge is no back edge and
/// crosses no loop boundary, and `pointer` is defined above it.
fn insertable(
    unit: &Unit,
    callees: &Callees,
    shape: &Shape,
    parent: BlockId,
    join: BlockId,
    index: usize,
    pointer: Operand,
) -> bool {
    let function = unit.function;
    let (from, to) = (cfg::id(parent), cfg::id(join));
    let branch = function.terminator(parent).is_some_and(|one| function.instruction(one).opcode == Opcode::Br);
    if !branch || (function.successors(parent) != [join] && !edges::conditional(function, parent, join)) {
        return false;
    }
    if shape.dominance.dominates(unit.function, to, from)
        || shape.loops.iter().any(|one| one.body.contains(&from) != one.body.contains(&to))
    {
        return false;
    }
    if function.block(join).iter().take(index).any(|prior| {
        !only_value(unit.context, callees, function, prior)
            || matches!(function.instruction(prior).opcode, Opcode::Load { .. })
    }) {
        return false;
    }
    match pointer {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Argument(_) => true,
            ValueDef::Instruction(def) => {
                function.parent(def).is_some_and(|block| shape.dominance.dominates(unit.function, cfg::id(block), from))
            }
        },
        Operand::Constant(_) => true,
        Operand::Block(_) => false,
    }
}

/// Every join load to replace by a phi, decided on `unit` as it stands.
fn planned(
    unit: &Unit,
    callees: &Callees,
    accesses: &Accesses,
    pointers: &PointValues,
    insert: bool,
) -> Vec<Joined> {
    let function = unit.function;
    let graph = cfg::graph(function);
    let entry = function.entry().map(cfg::id);
    let predecessors = loops::predecessors(&graph);
    if !predecessors.values().any(|parents| parents.len() > 1) {
        return Vec::new();
    }
    let memory = memoryssa::built(unit, accesses);
    let places: HashMap<InstId, (i64, usize)> = function
        .layout()
        .iter()
        .flat_map(|&block| {
            function.block(block).iter().enumerate().map(move |(index, inst)| (inst, (cfg::id(block), index)))
        })
        .collect();
    let providers: Vec<(InstId, MemRef, Operand)> = memory
        .sites
        .keys()
        .filter_map(|&site| provided(unit, accesses, site).map(|(cell, value)| (site, cell, value)))
        .collect();
    // The providers by the bytes they name, so that a load compares itself with
    // those and not with all.
    let cfg::Shape { dominance, loops: natural, .. } = unit.shape().into_owned();
    let nearest = llrm_analysis::nearest::Nearest::of(
        dominance.tree(unit.function),
        providers
            .iter()
            .enumerate()
            .flat_map(
                |(at, (source, cell, _))| {
                    let block = cfg::block(places[source].0);
                    memoryssa::byte_keys(unit, cell).list().into_iter().map(move |key| (key, block, at))
                },
            ),
    );
    let shape = Shape { depth: dominance.depths(function), dominance, loops: natural };

    let mut found = Vec::new();
    for block in &graph {
        let parents = &predecessors[&block.at];
        if Some(block.at) == entry || parents.len() < 2 {
            continue;
        }
        let join = cfg::block(block.at);
        for (index, load) in function.block(join).iter().enumerate() {
            let Some((reference, result)) = loaded_into(unit, accesses, load) else {
                continue;
            };
            let ty = function.value(result).ty;
            let mut incoming = Vec::new();
            for &parent in parents {
                let Some(pointer) = on_edge(function, load, cfg::block(parent)) else {
                    break;
                };
                let translated =
                    MemRef { typed: reference.typed.clone(), ..MemRef::at(unit, pointer, reference.width) };
                let translated = MemRef { provenance: pointers.reference(unit, &translated), ..translated };
                let accepts = |(source, cell, value): &&(InstId, MemRef, Operand)| {
                    let (at, _) = places[source];
                    at != block.at
                        && shape.dominance.dominates(unit.function, at, parent)
                        && !shape.dominance.dominates(unit.function, at, block.at)
                        && unit.operand_type(*value) == Some(ty)
                        && same_bytes(unit, cell, &translated)
                        && shape.loops.iter().all(|one| !one.body.contains(&at) || one.body.contains(&block.at))
                        && memory.available_on_edge(*source, load, parent, &reference, Some(&translated))
                };
                let keys = memoryssa::byte_keys(unit, &translated).list();
                let program = |at: usize| places[&providers[at].0].1;
                let scan = || {
                    providers
                        .iter()
                        .filter(accepts)
                        .fold(
                            None::<&(InstId, MemRef, Operand)>,
                            |best, one| match best {
                                Some(best)
                                    if (shape.depth[&places[&one.0].0], places[&one.0].1)
                                        <= (shape.depth[&places[&best.0].0], places[&best.0].1) =>
                                {
                                    Some(best)
                                }
                                _ => Some(one),
                            },
                        )
                };
                // The deepest source, the latest in its block; the first of
                // equals: the blocks that dominate `parent` from the nearest,
                // and in the first with a source, the latest.
                let best = if shape.dominance.reachable(unit.function, parent) {
                    let mut best: Option<usize> = None;
                    let mut block = None;
                    for (enter, at) in nearest.dominating(
                        shape.dominance.tree(unit.function),
                        &keys.iter().collect::<Vec<_>>(),
                        cfg::block(parent),
                    ) {
                        if block.is_some_and(|had| had != enter) {
                            break;
                        }
                        if accepts(&&providers[at]) {
                            block = Some(enter);
                            best = Some(best.map_or(at, |had| if program(at) > program(had) { at } else { had }));
                        }
                    }
                    best.map(|at| &providers[at])
                } else {
                    scan()
                };
                if llrm_support::env_flag!("LLRM_CHECK_BYTES") {
                    let every = scan();
                    assert_eq!(
                        best.map(|one| one.0),
                        every.map(|one| one.0),
                        "the provider found nearest first is not the deepest of a scan of all"
                    );
                }
                match best {
                    Some((_, _, value)) => incoming.push((cfg::block(parent), Incoming::Held(*value))),
                    None if insert && insertable(unit, callees, &shape, cfg::block(parent), join, index, pointer) => {
                        incoming.push((cfg::block(parent), Incoming::Loaded(pointer)))
                    }
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
fn applied(
    function: &mut Function,
    joined: Vec<Joined>,
) -> Result<(), String> {
    // A load already replaced names its phi.
    let mut replaced: BTreeMap<ValueId, Operand> = BTreeMap::new();
    let resolved = |replaced: &BTreeMap<ValueId, Operand>, one: Operand| {
        ssa::provider(one, replaced).expect("a phi replaces a load once")
    };
    let mut bridges: HashMap<(BlockId, BlockId), BlockId> = HashMap::default();
    for Joined { load, join, incoming } in joined {
        let mut arms = Vec::new();
        for (parent, one) in incoming {
            let (from, value) = match one {
                Incoming::Held(value) => {
                    (bridges.get(&(parent, join)).copied().unwrap_or(parent), resolved(&replaced, value))
                }
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
        function.insert(phi, Position::Before(function.block(join).first().expect("a block with an instruction")))?;
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
