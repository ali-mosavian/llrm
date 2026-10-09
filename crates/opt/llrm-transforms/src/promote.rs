//! Write-through promotion: a load of a cell a store left its value in on
//! every path becomes that value, a phi where paths join; every store
//! stays. `Sroa` does it for aggregates' leaves alone.
//! LLVM's counterpart: LICM's scalar promotion, function-wide; `Sroa`'s, SROA.
//!
//! Adapted from llrm-core's `optimize/promote.rs`. What an instruction
//! names and writes is `memoryssa::Accesses`' answer, and whether a write
//! reaches a cell
//! `regions::overlapping`'s. Globals and callees' attributes come through
//! the outer proxy.
//!
//! What changed with the IR:
//! - A cell is its exact leaf, or else its pointer decomposed (`Key::Ref`)
//!   over any root: the old `Addr` key, and `_allocation_leaves`' grouping
//!   by affine root. A leaf of one start takes any stride.
//! - One LLVM type per cell stands for the old width and float checks
//!   (`_float_cells`, `_stores_value`, `_converts_integer`,
//!   `_forwards_float`): a store keeps its type's value.
//! - A load becomes the stored operand itself, so `_restated` and `_order`
//!   have nothing to do; the phis are placed here, as `ssa::constructed`
//!   was not ported.
//! - `_canonical_leaf_types` types a leaf in both passes: the old `Sroa`
//!   rewrote the body the old `Promote` then read.
//!
//! Dropped, with no rich MIR counterpart:
//! - `READS`, `_separated`, `split_updates`: an arithmetic memory operand.
//! - `CELLS`, `Bounds`: the x86 spaces and landmarks `regions` dropped.
//! - `_initializers`: every object access carries exact provenance, so a
//!   narrower store into a wider cell blocks both.
//! - `_allocation_leaves`, `_affine_values`, `_signed`, `_rewritten_refs`:
//!   no array request; `MemRef::at` decomposes an address.
//! - `_bounded_leaves`, `_bounded_ref`, `_pointed_ref`: `alias::annotated`
//!   narrows constant indices and follows exact pointers.
//! - `_split_copies` and its helpers: `splitcopy`, for a `llvm.memcpy`.
//! - `loop_only`: no caller set it.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::manager::Held;
use llrm_analysis::memory::{Identity, MemRef, ObjectRef, Provenance, Slice, Unit};
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::{cfg, regions, ssa};
use llrm_analysis::graph::loops;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_mir::types::TypeId;
use llrm_mir::{Constant, ConstantKind, Context};
use llrm_support::bits::Bits;
use llrm_support::hash::{HashMap, HashSet, IndexMap};

/// One exact scalar leaf of a memory object, and the restrict roots the
/// pointers to it are based on.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct _Leaf {
    pub object: ObjectRef,
    pub low: i64,
    pub high: i64,
    pub type_class: Option<std::rc::Rc<str>>,
    pub restrict: BTreeSet<Identity>,
}

/// What a promoted cell is.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Key {
    Leaf(_Leaf),
    /// An access that is no leaf, as its pointer decomposes, at no width.
    Ref(MemRef),
}

/// The one explicit type each leaf's bytes are accessed as.
pub type Canonical = HashMap<(ObjectRef, i64, i64), std::rc::Rc<str>>;

fn only_slice(provenance: &Provenance) -> Option<&Slice> {
    if provenance.slices.len() == 1 { provenance.slices.iter().next() } else { None }
}

fn slice(object: ObjectRef, low: i64, high: i64) -> Slice {
    Slice::new(object, low, high, 1, 1).expect("a nonempty exact byte range")
}

/// The leaf `reference` is, typed as `canonical` says where it has no type.
pub fn _leaf(reference: &MemRef, canonical: &Canonical) -> Option<_Leaf> {
    let span = only_slice(reference.provenance.as_ref()?)?;
    // One access is either the dense byte range `[low, low + width)` or one
    // start whose own width is the access's, whatever its stride.
    let width = i64::from(reference.width);
    let contiguous = (span.width == 1 && span.stride == 1 && span.high - span.low == width) || (span.high - span.low == 1 && span.width == width);
    if !contiguous {
        return None;
    }
    let high = span.low + width;
    if span.object.extent.is_some_and(|extent| !(0 <= span.low && span.low < high && high <= extent)) {
        return None;
    }
    let type_class = reference.typed.clone().or_else(|| canonical.get(&(span.object, span.low, high)).cloned());
    let restrict = reference.provenance.as_ref().map(|provenance| provenance.restrict.clone()).unwrap_or_default();
    Some(_Leaf { object: span.object, low: span.low, high, type_class, restrict })
}

/// Bytes whose accesses cannot form disjoint scalar leaves.
///
/// Equal ranges are one leaf, disjoint ranges independent leaves. A proper
/// overlap keeps both ranges in memory and leaves the rest of their object
/// alone; ambiguous multi-object provenance keeps every slice it names.
pub fn _blocked<'a>(refs: impl IntoIterator<Item = &'a MemRef>, canonical: &Canonical) -> BTreeSet<Slice> {
    let mut accesses = IndexMap::<ObjectRef, Vec<_Leaf>>::default();
    let mut blocked = BTreeSet::new();
    for reference in refs {
        let Some(provenance) = &reference.provenance else { continue };
        let objects = provenance.slices.iter().map(|span| &span.object).collect::<BTreeSet<_>>();
        if objects.len() != 1 || provenance.slices.len() != 1 {
            blocked.extend(provenance.slices.iter().cloned());
            continue;
        }
        if let Some(leaf) = _leaf(reference, canonical) {
            accesses.entry(leaf.object.clone()).or_default().push(leaf);
        }
    }
    for (object, leaves) in &accesses {
        for (index, one) in leaves.iter().enumerate() {
            for other in &leaves[index + 1..] {
                let overlaps = one.low.max(other.low) < one.high.min(other.high);
                let same_range = (one.low, one.high) == (other.low, other.high);
                if overlaps && (!same_range || one.type_class != other.type_class) {
                    blocked.insert(slice(object.clone(), one.low, one.high));
                    blocked.insert(slice(object.clone(), other.low, other.high));
                }
            }
        }
    }
    blocked
}

/// Whether the reference reaches a byte of its own object that is blocked.
fn _touches(reference: &MemRef, blocked: &BTreeSet<Slice>) -> bool {
    reference.provenance.as_ref().is_some_and(|provenance| {
        provenance.slices.iter().any(|span| blocked.iter().any(|one| span.object == one.object && span.intersects(one)))
    })
}

pub fn _key(reference: &MemRef, blocked: &BTreeSet<Slice>, canonical: &Canonical) -> Option<Key> {
    if reference.volatile || _touches(reference, blocked) {
        return None;
    }
    if let Some(leaf) = _leaf(reference, canonical) {
        return Some(Key::Leaf(leaf));
    }
    reference.root?;
    // One pointer, decomposed, names the same bytes however it was spelled.
    Some(Key::Ref(MemRef { pointer: None, width: 0, inbounds: false, ..reference.clone() }))
}

/// The access a cell is, `width` bytes wide.
pub fn _reference(key: &Key, width: u32) -> MemRef {
    match key {
        Key::Leaf(leaf) => MemRef::reach(width, Provenance { slices: BTreeSet::from([slice(leaf.object.clone(), leaf.low, leaf.high)]), restrict: leaf.restrict.clone() }),
        Key::Ref(reference) => MemRef { width, ..reference.clone() },
    }
}

/// Objects known to contain more than the scalar leaf being accessed.
pub fn _aggregate_objects<'a>(leaves: impl IntoIterator<Item = &'a Key>) -> BTreeSet<ObjectRef> {
    let mut ranges = IndexMap::<ObjectRef, BTreeSet<(i64, i64)>>::default();
    for leaf in leaves {
        if let Key::Leaf(leaf) = leaf {
            ranges.entry(leaf.object.clone()).or_default().insert((leaf.low, leaf.high));
        }
    }
    ranges
        .into_iter()
        .filter(|(object, parts)| parts.len() > 1 || object.extent.is_some_and(|extent| parts.iter().any(|(low, high)| extent > high - low)))
        .map(|(object, _)| object)
        .collect()
}

/// An untyped leaf takes the one explicit type its bytes are accessed as:
/// a byte-typed move and a field access are the same bytes. Two explicit,
/// distinct types keep the union and type-pun rejection.
pub fn _canonical_leaf_types<'a>(refs: impl IntoIterator<Item = &'a MemRef>) -> Canonical {
    let untyped = Canonical::default();
    let mut types = HashMap::<(ObjectRef, i64, i64), BTreeSet<std::rc::Rc<str>>>::default();
    for reference in refs {
        if let Some(leaf) = _leaf(reference, &untyped)
            && let Some(type_class) = leaf.type_class
        {
            types.entry((leaf.object, leaf.low, leaf.high)).or_default().insert(type_class);
        }
    }
    types.into_iter().filter(|(_, classes)| classes.len() == 1).map(|(key, classes)| (key, classes.into_iter().next().expect("one"))).collect()
}

pub struct Promote;

impl FunctionPass for Promote {
    fn name(&self) -> &'static str {
        "promote"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        run(unit, analyses, false, "promote")
    }
}

/// Scalarize proven aggregate leaves before scalar simplification.
pub struct Sroa;

impl FunctionPass for Sroa {
    fn name(&self) -> &'static str {
        "sroa"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        run(unit, analyses, true, "sroa")
    }
}

fn run(unit: &mut passes::Unit, analyses: &mut Analyses, aggregate_only: bool, name: &str) -> PreservedAnalyses {
    // A copy of an aggregate is its leaves' loads and stores before they are promoted.
    let split = aggregate_only && crate::splitcopy::split(unit.context, unit.layout, unit.function, analyses.outer());
    match _promoted(unit.context, unit.layout, unit.function, analyses, aggregate_only) {
        Ok(true) => PreservedAnalyses::none(),
        Ok(false) if split => PreservedAnalyses::none(),
        Ok(false) => PreservedAnalyses::all(),
        Err(error) => panic!("{name}: {error}"),
    }
}

/// `function` promoted, or with `aggregate_only` only its aggregates'
/// leaves; `outer` is its module and target. Whether it changed.
pub fn promoted(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &Outer, aggregate_only: bool) -> Result<bool, String> {
    _promoted(context, layout, function, &mut Analyses::new(std::rc::Rc::new(outer.clone())), aggregate_only)
}

/// `promoted`, `analyses` holding what is known of `function`.
fn _promoted(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &mut Analyses, aggregate_only: bool) -> Result<bool, String> {
    let plan = {
        let accesses = Accesses::managed(context, layout, function, analyses)?;
        let held = Held::of(context, layout, function, analyses, true);
        let unit = held.unit(context, layout, function, analyses.outer());
        let mut plan = plan(&unit, &accesses, aggregate_only)?;
        plan.variables = named_cells(&unit, &plan, &analyses.outer().metadata);
        plan
    };
    if plan.loads.is_empty() {
        return Ok(false);
    }
    rewrite(context, function, &plan);
    Ok(true)
}

/// What one function's promotion does: the type each cell holds, the
/// stores that define one, and the loads that read one's stored value.
#[derive(Debug, Default)]
struct Plan {
    /// The variable `-g` names that each cell is, where it is one (`llrm_mir::DebugRecord`).
    variables: Vec<Option<Named>>,
    types: Vec<TypeId>,
    stores: HashMap<InstId, usize>,
    loads: HashMap<InstId, usize>,
}

/// What `-g` declared that each cell of `plan` is: the variable whose frame object holds its bytes, and where in the variable it
/// is, none where it is all of it.
fn named_cells(unit: &Unit, plan: &Plan, metadata: &[llrm_mir::module::MetadataNode]) -> Vec<Option<Named>> {
    let declared: Vec<(Operand, llrm_mir::MetadataId, i64)> = unit
        .function
        .debug_records()
        .iter()
        .filter_map(|one| match one.what {
            llrm_mir::DebugWhat::Declare(address) => Some((address, one.variable, llrm_mir::debuginfo::variable_offset(metadata, unit.context, one.variable)?)),
            _ => None,
        })
        .collect();
    let mut named: Vec<Option<(llrm_mir::MetadataId, i64, u32)>> = vec![None; plan.types.len()];
    if declared.is_empty() {
        return Vec::new();
    }
    for (&inst, &slot) in &plan.stores {
        let pointer = unit.function.instruction(inst).operands[1];
        let (base, offset) = llrm_mir::valuetracking::underlying(unit.context, unit.layout, unit.function, pointer);
        let Some(offset) = offset else { continue };
        // The variable of that object that starts last at or before the cell.
        let owner = declared.iter().filter(|(address, _, at)| *address == base && *at <= offset).max_by_key(|(_, _, at)| *at);
        if let Some(&(_, variable, at)) = owner {
            let bytes = unit.layout.alloc_size(&unit.context.types, plan.types[slot]) as u32;
            named[slot] = Some((variable, offset - at, bytes));
        }
    }
    // A variable one cell is all of is that value; a variable of several cells has each as a piece.
    named
        .iter()
        .map(|one| {
            let (variable, offset, bytes) = (*one)?;
            let cells = named.iter().flatten().filter(|(other, ..)| *other == variable).count();
            Some(Named { variable, piece: (cells > 1 || offset != 0).then_some((offset as u32, bytes)) })
        })
        .collect()
}

/// A variable `-g` declared, and the bytes of it that a cell is when it is not all.
#[derive(Clone, Copy, Debug)]
struct Named {
    variable: llrm_mir::MetadataId,
    piece: Option<(u32, u32)>,
}

impl Named {
    /// What a debugger is told when the cell has `value`.
    fn is(&self, value: Operand) -> llrm_mir::DebugWhat {
        match self.piece {
            Some((offset, bytes)) => llrm_mir::DebugWhat::Piece { value, offset, bytes },
            None => llrm_mir::DebugWhat::Value(value),
        }
    }
}

/// The type `inst` loads or stores.
fn accessed(unit: &Unit, inst: InstId) -> Option<TypeId> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { .. } => Some(instruction.ty),
        Opcode::Store { .. } => unit.operand_type(instruction.operands[0]),
        _ => None,
    }
}

fn is_load(unit: &Unit, inst: InstId) -> bool {
    matches!(unit.function.instruction(inst).opcode, Opcode::Load { .. })
}

/// Cells whose reads can use a known stored value: touched more than once,
/// loaded as one type, and available along every path to some load.
fn plan(unit: &Unit, accesses: &Accesses, aggregate_only: bool) -> Result<Plan, String> {
    let refs = &accesses.references;
    if !refs.keys().any(|&inst| is_load(unit, inst)) {
        return Ok(Plan::default());
    }
    let canonical = _canonical_leaf_types(refs.values());
    let blocked = _blocked(refs.values(), &canonical);
    let keys = refs.iter().filter_map(|(&inst, reference)| _key(reference, &blocked, &canonical).map(|key| (inst, key))).collect::<IndexMap<_, _>>();
    let aggregates = aggregate_only.then(|| _aggregate_objects(keys.values()));
    if aggregates.as_ref().is_some_and(BTreeSet::is_empty) {
        return Ok(Plan::default());
    }

    let mut seen = IndexMap::<&Key, usize>::default();
    let mut types = HashMap::<&Key, BTreeSet<TypeId>>::default();
    for (&inst, key) in &keys {
        *seen.entry(key).or_default() += 1;
        if is_load(unit, inst) {
            types.entry(key).or_default().extend(accessed(unit, inst));
        }
    }
    let candidates = seen
        .into_iter()
        .filter(|(key, times)| *times > 1 && types.get(key).is_some_and(|found| found.len() == 1))
        .filter(|(key, _)| aggregates.as_ref().is_none_or(|objects| matches!(key, Key::Leaf(leaf) if objects.contains(&leaf.object))))
        .map(|(key, _)| (key.clone(), *types[key].first().expect("one type")))
        .collect::<IndexMap<_, _>>();
    if candidates.is_empty() {
        return Ok(Plan::default());
    }

    // An access of another type is no definition or use, only a write.
    let slots = keys
        .iter()
        .filter_map(|(&inst, key)| candidates.get_full(key).filter(|(_, _, ty)| accessed(unit, inst) == Some(**ty)).map(|(slot, ..)| (inst, slot)))
        .collect::<HashMap<_, _>>();
    // A cell's `!tbaa` type, where every access of it agrees: what keeps a
    // write of another type from reaching it.
    let mut typed = IndexMap::<&Key, Option<Option<(std::rc::Rc<str>, std::rc::Rc<[String]>)>>>::default();
    for (inst, key) in keys.iter().filter(|(_, key)| candidates.contains_key(*key)) {
        let one = refs[inst].typed.clone().map(|name| (name, refs[inst].lineage.clone()));
        let agreed = typed.entry(key).or_insert_with(|| Some(one.clone()));
        if agreed.as_ref() != Some(&one) {
            *agreed = None;
        }
    }
    let typed = candidates.keys().map(|key| typed.get(key).cloned().flatten().flatten()).collect::<Vec<_>>();
    let usable = _available(unit, accesses, &candidates, &typed, &slots);

    let used = usable.iter().map(|inst| slots[inst]).collect::<BTreeSet<_>>();
    let renumbered = used.iter().enumerate().map(|(new, &old)| (old, new)).collect::<HashMap<_, _>>();
    let types = used.iter().map(|&slot| candidates[slot]).collect();
    let stores = slots.iter().filter(|(inst, slot)| !is_load(unit, **inst) && used.contains(slot)).map(|(&inst, slot)| (inst, renumbered[slot])).collect();
    let loads = usable.into_iter().map(|inst| (inst, renumbered[&slots[&inst]])).collect();
    Ok(Plan { variables: Vec::new(), types, stores, loads })
}

/// Loads a stored value reaches on every path, with no write between that
/// may reach its cell.
fn _available(unit: &Unit, accesses: &Accesses, cells: &IndexMap<Key, TypeId>, typed: &[Option<(std::rc::Rc<str>, std::rc::Rc<[String]>)>], slots: &HashMap<InstId, usize>) -> HashSet<InstId> {
    let function = unit.function;
    let Some(entry) = function.entry().map(cfg::id) else { return HashSet::default() };
    let graph = cfg::graph(function);
    let dominance = cfg::Dominance::of(function);
    let reachable = graph.iter().map(|block| block.at).filter(|&at| dominance.reachable(at)).collect::<BTreeSet<_>>();
    let predecessors = loops::predecessors(&graph);
    let refs = cells.iter().zip(typed).map(|((key, &ty), typed)| MemRef { typed: typed.as_ref().map(|one| one.0.clone()), lineage: typed.as_ref().map(|one| one.1.clone()).unwrap_or_default(), .._reference(key, unit.layout.store_size(&unit.context.types, ty) as u32) }).collect::<Vec<_>>();
    // The cells whose address a value is part of: its definition, a phi's
    // on a back edge among them, moves them.
    let mut based = HashMap::<ValueId, Vec<usize>>::default();
    for (at, reference) in refs.iter().enumerate() {
        let operands = [reference.root, reference.segment, reference.base.map(Operand::Value)];
        for value in operands.into_iter().flatten().filter_map(|one| if let Operand::Value(value) = one { Some(value) } else { None }) {
            based.entry(value).or_default().push(at);
        }
    }
    let mut every = Bits::new(cells.len());
    (0..cells.len()).for_each(|at| every.insert(at));
    let mut leaving = reachable.iter().map(|at| (*at, every.clone())).collect::<BTreeMap<_, _>>();
    // Whether a write may reach a cell, per (instruction, cell): every round asks again.
    let clobbered = RefCell::new(HashMap::<(InstId, usize), bool>::default());

    let entering = |at: i64, leaving: &BTreeMap<i64, Bits>| -> Bits {
        let parents = predecessors.get(&at).into_iter().flatten().filter(|parent| reachable.contains(parent)).collect::<Vec<_>>();
        if parents.is_empty() || at == entry {
            return Bits::new(cells.len());
        }
        let mut result = leaving[parents[0]].clone();
        for parent in &parents[1..] {
            result.intersect_with(&leaving[*parent]);
        }
        result
    };

    let through = |at: i64, mut available: Bits, mut reads: Option<&mut HashSet<InstId>>| {
        for &inst in function.block(cfg::block(at)).instructions() {
            let instruction = function.instruction(inst);
            let store = matches!(instruction.opcode, Opcode::Store { .. });
            // A volatile access writes its own bytes: it orders, it does not clobber.
            let Some(writes) = accesses.writes(inst) else {
                available = Bits::new(cells.len());
                continue;
            };
            let slot = slots.get(&inst).copied();
            if let (Some(reads), Some(slot)) = (reads.as_deref_mut(), slot)
                && !store
                && available.contains(slot)
            {
                reads.insert(inst);
            }
            if let Some(moved) = instruction.result.and_then(|result| based.get(&result)) {
                moved.iter().for_each(|&at| available.remove(at));
            }
            if !writes.is_empty() {
                let gone = available
                    .iter()
                    .filter(|&at| {
                        *clobbered.borrow_mut().entry((inst, at)).or_insert_with(|| {
                            writes.iter().any(|written| regions::overlapping(&refs[at], written, None, None, unit.program).unwrap_or(true))
                        })
                    })
                    .collect::<Vec<_>>();
                gone.into_iter().for_each(|at| available.remove(at));
            }
            if let Some(slot) = slot.filter(|_| store) {
                available.insert(slot);
            }
        }
        available
    };

    loop {
        let mut changed = false;
        for &at in &reachable {
            let result = through(at, entering(at, &leaving), None);
            if leaving.get(&at) != Some(&result) {
                leaving.insert(at, result);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut reads = HashSet::default();
    for &at in &reachable {
        through(at, entering(at, &leaving), Some(&mut reads));
    }
    reads
}

/// Each edge into `block`, by the block it leaves.
fn edges(function: &Function, block: BlockId) -> Vec<BlockId> {
    function.block_users(block).iter().filter(|one| function.instruction(one.user).opcode.is_terminator()).filter_map(|one| function.parent(one.user)).collect()
}

/// `function` with each planned load its cell's stored value: phis where
/// the stores' dominance frontiers put them, named down the dominator tree.
fn rewrite(context: &mut Context, function: &mut Function, plan: &Plan) {
    let entry = cfg::id(function.entry().expect("a defined function"));
    let dominance = cfg::Dominance::of(function);
    let frontiers = dominance.frontiers(function);
    let idom = dominance.immediate_dominators(function);
    let existing = function.walk().map(|(_, inst)| function.instruction(inst)).filter(|one| one.opcode == Opcode::Phi).filter_map(|one| one.result).collect::<BTreeSet<_>>();
    let poison = plan.types.iter().map(|&ty| Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }))).collect::<Vec<_>>();

    // Each phi placed, by block and cell, with the block each input comes from.
    let mut phis = HashMap::<(i64, usize), (InstId, Vec<BlockId>)>::default();
    let mut named_phis = Vec::new();
    for (slot, &ty) in plan.types.iter().enumerate() {
        let defining = plan.stores.iter().filter(|(_, one)| **one == slot).filter_map(|(inst, _)| function.parent(*inst)).map(cfg::id).collect::<BTreeSet<_>>();
        let mut work = defining.iter().copied().collect::<Vec<_>>();
        let mut placed = BTreeSet::new();
        while let Some(at) = work.pop() {
            for &frontier in frontiers.get(&at).into_iter().flatten() {
                if !placed.insert(frontier) {
                    continue;
                }
                let block = cfg::block(frontier);
                let from = edges(function, block);
                let operands = from.iter().flat_map(|&one| [poison[slot], Operand::Block(one)]).collect();
                let phi = function.create_instruction(Opcode::Phi, ty, operands, Flags::default(), None);
                let first = function.block(block).instructions().first().copied();
                function.insert(phi, first.map_or(Position::End(block), Position::Before)).expect("a placed block");
                phis.insert((frontier, slot), (phi, from));
                if let Some(named) = plan.variables.get(slot).copied().flatten() {
                    named_phis.push((block, phi, named));
                }
                if !defining.contains(&frontier) {
                    work.push(frontier);
                }
            }
        }
    }

    // What the debugger is told of a variable: the phi that merges its paths, from the top of the block on.
    for (block, phi, named) in named_phis {
        let value = function.instruction(phi).result.expect("a phi's value");
        let first = function.block(block).instructions().iter().copied().find(|&one| function.instruction(one).opcode != Opcode::Phi);
        if let Some(first) = first {
            function.add_debug_record_first(first, named.variable, named.is(Operand::Value(value)));
        }
    }

    let mut children = BTreeMap::<i64, Vec<i64>>::new();
    for (&at, &parent) in &idom {
        if let Some(parent) = parent {
            children.entry(parent).or_default().push(at);
        }
    }
    let mut dead = Vec::new();
    let mut stack = vec![(entry, poison.clone())];
    while let Some((at, mut current)) = stack.pop() {
        let block = cfg::block(at);
        for (slot, value) in current.iter_mut().enumerate() {
            if let Some((phi, _)) = phis.get(&(at, slot)) {
                *value = Operand::Value(function.instruction(*phi).result.expect("a phi's value"));
            }
        }
        for inst in function.block(block).instructions().to_vec() {
            if let Some(&slot) = plan.loads.get(&inst) {
                function.replace_value(function.instruction(inst).result.expect("a load's value"), current[slot]);
                dead.push(inst);
            } else if let Some(&slot) = plan.stores.get(&inst) {
                current[slot] = function.instruction(inst).operands[0];
                // And the value it stores, from the instruction after it on.
                if let Some(named) = plan.variables.get(slot).copied().flatten()
                    && let Some(next) = function.block(block).instructions().iter().copied().skip_while(|&one| one != inst).nth(1)
                {
                    function.add_debug_record_first(next, named.variable, named.is(current[slot]));
                }
            }
        }
        for successor in function.successors(block) {
            for (slot, &value) in current.iter().enumerate() {
                let Some((phi, from)) = phis.get(&(cfg::id(successor), slot)) else { continue };
                for (index, _) in from.iter().enumerate().filter(|(_, one)| **one == block) {
                    function.set_operand(*phi, 2 * index, value);
                }
            }
        }
        stack.extend(children.get(&at).into_iter().flatten().map(|&child| (child, current.clone())));
    }
    for inst in dead {
        function.erase(inst).expect("a promoted load has no users left");
    }
    // A phi no promoted load reached goes, and any input it had from a path
    // the cell was not stored on with it.
    ssa::pruned_phis(function, &existing);
}

#[cfg(test)]
#[path = "promote_tests.rs"]
mod tests;
