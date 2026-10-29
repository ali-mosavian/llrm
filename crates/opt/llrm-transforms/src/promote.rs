//! Write-through promotion: a load of a cell that a store left its value in on
//! every path becomes that value, a phi where paths join. Every store stays;
//! `dse` removes the ones nothing reads. `Sroa` is the same rewrite for the
//! leaves of aggregates alone.
//!
//! # How it works
//! A cell is a scalar memory location, named by a `Key`: an exact leaf of a
//! memory object (its object, byte range, `!tbaa` type and `restrict` roots)
//! or else its pointer decomposed (`MemRef`, any root). Accesses of one leaf
//! through distinct but equal pointers are one cell. `memoryssa::Accesses`
//! says what each instruction names and writes.
//!
//! A cell is blocked, and stays in memory, when
//! - an access is volatile;
//! - its bytes partly overlap another leaf's (`_blocked`): a narrower store
//!   into a wider cell, the halves of one, a wider cell read in part;
//! - it is loaded as more than one type (an access of another type is only a
//!   write).
//!
//! A cell is a candidate when it is accessed more than once. For `Sroa` it
//! must also be a leaf of an object that holds more than that leaf, and
//! `splitcopy` first turns a copy of an aggregate into its leaves' loads and
//! stores.
//!
//! `_available` is a forward must-analysis over the reachable blocks: the set
//! of cells whose last write on every path is a store of the cell's own type.
//! A block's entry set is the intersection of its predecessors' exits (empty
//! at the entry). Walking a block, a store of a cell adds it; a write that may
//! reach the cell (`regions::overlapping`, calls by their effects) or a
//! definition of a value the cell's address is made from removes it; an
//! instruction with no known writes removes every cell. A load of a cell in
//! the set is planned. A load reached before any store keeps its memory value.
//!
//! `rewrite` then places phis as LLVM's `PromoteMemToReg` does: at the
//! dominance frontier of each cell's stores, only where the cell is live on
//! entry, and renames down the dominator tree. Each planned load is replaced
//! by the stored operand or the phi. With `-g`, the debug record of the
//! variable a cell is gets the value, or a piece of it.
//!
//! ```text
//!  b1: store 7, %a   b2: store %d, %a       b1: store 7, %a   b2: store %d, %a
//!  b3: %x = load %a             =>          b3: %x = phi [7, b1], [%d, b2]
//!      %y = add %x, 1                           %y = add %x, 1
//! ```
//!
//! # Example
//! Before:
//! ```mir
//! define ... @pipeline.body() ... {
//! b1:
//!   %0 = alloca i16
//!   %1 = alloca i16
//!   store i16 0, ptr %1, !tbaa !9
//!   br label %b2
//!
//! b2:
//!   %2 = load i16, ptr %1, !tbaa !9
//!   %3 = icmp slt i16 %2, 10
//!   %4 = zext i1 %3 to i16
//!   br i1 %3, label %b4, label %b3
//!
//! b3:
//!   ret void
//!
//! b4:
//!   store i16 %2, ptr @_n, !tbaa !9
//!   %5 = add nsw i16 %2, 1
//!   store i16 %5, ptr %1, !tbaa !9
//!   br label %b2
//! }
//! ```
//! After:
//! ```mir
//! define ... @pipeline.body() ... {
//! b1:
//!   %0 = alloca i16
//!   %1 = alloca i16
//!   store i16 0, ptr %1, !tbaa !9
//!   br label %b2
//!
//! b2:
//!   %2 = phi i16 [ 0, %b1 ], [ %5, %b4 ]
//!   %3 = icmp slt i16 %2, 10
//!   %4 = zext i1 %3 to i16
//!   br i1 %3, label %b4, label %b3
//!
//! b3:
//!   ret void
//!
//! b4:
//!   store i16 %2, ptr @_n, !tbaa !9
//!   %5 = add nsw i16 %2, 1
//!   store i16 %5, ptr %1, !tbaa !9
//!   br label %b2
//! }
//! ```
//! The load of the counter's slot `%1` is the phi `%2`. Every store stays;
//! `dse` removes the ones nothing reads.
//!
//! From `--dump` of `ten` (`for (i = 0; i < 10; i++) n = i;`) at -O2 -m16,
//! stages `gvn` and `promote` of the first body.
//!
//! # Runs
//! In [`crate::pipeline`]'s per-body fixed point: `Sroa` at the boundaries of
//! `Fixed` (start, after a peel, an unroll or a jam), before any pass asks
//! what is constant or invariant; `Promote` each round, after `gvn`. Both are
//! skipped on a body with an irreducible loop, which has no dominators. On at
//! every level above -O0 (`promote`, `-ftree-sra`).
//!
//! # Needs
//! `memoryssa::Accesses`, `Held`, `regions`, [`crate::splitcopy`], the
//! dominator tree.
//!
//! # Keeps
//! Nothing, where it changes a body.
//!
//! # See also
//! LLVM's LICM scalar promotion (`promoteLoopAccessesToScalars`), function-wide
//! here, and SROA; gcc's `-ftree-sra`.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops;
use llrm_analysis::manager::Held;
use llrm_analysis::memory::{Identity, MemRef, ObjectRef, Provenance, Slice, Unit};
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::{cfg, regions, ssa};
use llrm_mir::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::footprint::Footprint;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_mir::types::TypeId;
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

fn slice(
    object: ObjectRef,
    low: i64,
    high: i64,
) -> Slice {
    Slice::new(object, low, high, 1, 1).expect("a nonempty exact byte range")
}

/// The leaf `reference` is, typed as `canonical` says where it has no type.
pub fn _leaf(
    reference: &MemRef,
    canonical: &Canonical,
) -> Option<_Leaf> {
    let span = only_slice(reference.provenance.as_ref()?)?;
    // One access is either the dense byte range `[low, low + width)` or one
    // start whose own width is the access's, whatever its stride.
    let width = i64::from(reference.width);
    let contiguous = (span.width == 1 && span.stride == 1 && span.high - span.low == width)
        || (span.high - span.low == 1 && span.width == width);
    if !contiguous {
        return None;
    }
    let high = span.low + width;
    if span.object.extent.is_some_and(|extent| !(0 <= span.low && span.low < high && high <= extent)) {
        return None;
    }
    let type_class = reference.typed.clone().or_else(|| canonical.get(&(span.object, span.low, high)).cloned());
    let restrict = reference.provenance.as_ref().map(|provenance| provenance.restrict.to_set()).unwrap_or_default();
    Some(_Leaf { object: span.object, low: span.low, high, type_class, restrict })
}

// Pairs of leaves `_blocked` has compared.
#[cfg(test)]
thread_local! {
    pub(crate) static PAIRS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Bytes whose accesses cannot form disjoint scalar leaves.
///
/// Equal ranges are one leaf, disjoint ranges independent leaves. A proper
/// overlap keeps both ranges in memory and leaves the rest of their object
/// alone; ambiguous multi-object provenance keeps every slice it names.
pub fn _blocked<'a>(
    refs: impl IntoIterator<Item = &'a MemRef>,
    canonical: &Canonical,
) -> BTreeSet<Slice> {
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
    for (object, mut leaves) in accesses {
        // A leaf is its bytes and type, however many accesses make it: accesses
        // of one leaf never block each other, and a pair of leaves is asked
        // once, not once for each pair of accesses. In order of first byte a
        // leaf meets only the ones that start before it ends.
        leaves.sort_by(|one, other| {
            (one.low, one.high, &one.type_class).cmp(&(other.low, other.high, &other.type_class))
        });
        leaves
            .dedup_by(|one, other| (one.low, one.high, &one.type_class) == (other.low, other.high, &other.type_class));
        for (index, one) in leaves.iter().enumerate() {
            for other in leaves[index + 1..].iter().take_while(|other| other.low < one.high) {
                #[cfg(test)]
                PAIRS.with(|asked| asked.set(asked.get() + 1));
                let same_range = (one.low, one.high) == (other.low, other.high);
                if !same_range || one.type_class != other.type_class {
                    blocked.insert(slice(object.clone(), one.low, one.high));
                    blocked.insert(slice(object.clone(), other.low, other.high));
                }
            }
        }
    }
    blocked
}

/// Whether the reference reaches a byte of its own object that is blocked.
fn _touches(
    reference: &MemRef,
    blocked: &BTreeSet<Slice>,
) -> bool {
    reference.provenance.as_ref().is_some_and(|provenance| {
        provenance.slices.iter().any(|span| blocked.iter().any(|one| span.object == one.object && span.intersects(one)))
    })
}

pub fn _key(
    reference: &MemRef,
    blocked: &BTreeSet<Slice>,
    canonical: &Canonical,
) -> Option<Key> {
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
pub fn _reference(
    key: &Key,
    width: u32,
) -> MemRef {
    match key {
        Key::Leaf(leaf) => MemRef::reach(
            width,
            Provenance {
                slices: llrm_analysis::memory::Sorted::from([slice(leaf.object.clone(), leaf.low, leaf.high)]),
                restrict: leaf.restrict.clone().into(),
            },
        ),
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
        .filter(|(object, parts)| {
            parts.len() > 1 || object.extent.is_some_and(|extent| parts.iter().any(|(low, high)| extent > high - low))
        })
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
    types
        .into_iter()
        .filter(|(_, classes)| classes.len() == 1)
        .map(|(key, classes)| (key, classes.into_iter().next().expect("one")))
        .collect()
}

pub struct Promote;

impl FunctionPass for Promote {
    fn name(&self) -> &'static str {
        "promote"
    }

    /// A load becomes the value a store left in its cell: memory operations
    /// (and what addresses them) and the CFG.
    fn reads(&self) -> Footprint {
        Footprint::MEMORY | Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        run(unit, analyses, false, "promote")
    }
}

/// Scalarize proven aggregate leaves before scalar simplification.
pub struct Sroa;

impl FunctionPass for Sroa {
    fn name(&self) -> &'static str {
        "sroa"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        run(unit, analyses, true, "sroa")
    }
}

fn run(
    unit: &mut passes::Unit,
    analyses: &mut Analyses,
    aggregate_only: bool,
    name: &str,
) -> PreservedAnalyses {
    // A copy of an aggregate is its leaves' loads and stores before they are
    // promoted.
    let split = aggregate_only && crate::splitcopy::split(unit.context, unit.layout, unit.function, analyses.outer());
    // What is asked of the manager next is asked of the body the split left.
    if split {
        analyses.invalidate(unit.function, &PreservedAnalyses::none());
    }
    match _promoted(unit.context, unit.layout, unit.function, analyses, aggregate_only) {
        Ok(true) => PreservedAnalyses::none(),
        Ok(false) if split => PreservedAnalyses::none(),
        Ok(false) => PreservedAnalyses::all(),
        Err(error) => panic!("{name}: {error}"),
    }
}

/// The alloca nothing loads, deleted with its stores, as a pass of its own:
/// LLVM's PromoteMemToReg and SROA leave none (`removeIntrinsicUsers`, and
/// `deleteDeadInstructions` for a slot whose loads were all forwarded). It runs
/// with the slots of `llvm_order`.
pub struct DeadSlots;

impl FunctionPass for DeadSlots {
    fn name(&self) -> &'static str {
        "deadslots"
    }

    fn reads(&self) -> Footprint {
        Footprint::MEMORY
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        if emptied_slots(unit.function) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// Every alloca that nothing loads and whose address goes nowhere (only stores
/// through it, and addresses derived by `gep` or a bitcast that are the same),
/// erased with those stores and addresses; whether any was.
pub fn emptied_slots(function: &mut Function) -> bool {
    let allocas: Vec<(InstId, ValueId)> = function
        .walk()
        .filter_map(|(_, inst)| {
            let op = function.instruction(inst);
            matches!(op.opcode, Opcode::Alloca { .. }).then(|| op.result.map(|result| (inst, result))).flatten()
        })
        .collect();
    let mut changed = false;
    for (alloca, root) in allocas {
        let mut erase: Vec<InstId> = Vec::new();
        let mut pending = vec![root];
        let mut dead = true;
        'walk: while let Some(value) = pending.pop() {
            for one in function.users(value) {
                let user = function.instruction(one.user);
                match &user.opcode {
                    Opcode::Store { volatile: false, .. }
                        if one.index == 1 && user.operands[0] != Operand::Value(value) =>
                    {
                        erase.push(one.user);
                    }
                    Opcode::GetElementPtr { .. } | Opcode::Cast(llrm_mir::opcode::CastOp::BitCast) | Opcode::Freeze
                        if one.index == 0 =>
                    {
                        erase.push(one.user);
                        pending.extend(user.result);
                    }
                    _ => {
                        dead = false;
                        break 'walk;
                    }
                }
            }
        }
        if !dead {
            continue;
        }
        // Stores first, then the addresses (a user before what it uses), then
        // the alloca.
        for inst in erase.into_iter().rev().chain([alloca]) {
            if !function.is_erased(inst) {
                function.erase(inst).expect("nothing reads it");
            }
        }
        changed = true;
    }
    changed
}

/// `function` promoted, or with `aggregate_only` only its aggregates'
/// leaves; `outer` is its module and target. Whether it changed.
pub fn promoted(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    aggregate_only: bool,
) -> Result<bool, String> {
    _promoted(context, layout, function, &mut Analyses::new(std::rc::Rc::new(outer.clone())), aggregate_only)
}

/// `promoted`, `analyses` holding what is known of `function`.
fn _promoted(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    aggregate_only: bool,
) -> Result<bool, String> {
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
    /// The variable `-g` names that each cell is, where it is one
    /// (`llrm_mir::DebugRecord`).
    variables: Vec<Option<Named>>,
    types: Vec<TypeId>,
    stores: HashMap<InstId, usize>,
    loads: HashMap<InstId, usize>,
}

/// What `-g` declared that each cell of `plan` is: the variable whose frame
/// object holds its bytes, and where in the variable it is, none where it is
/// all of it.
fn named_cells(
    unit: &Unit,
    plan: &Plan,
    metadata: &[llrm_mir::module::MetadataNode],
) -> Vec<Option<Named>> {
    let declared: Vec<(Operand, llrm_mir::MetadataId, i64)> = unit
        .function
        .debug_records()
        .iter()
        .filter_map(|one| match one.what {
            llrm_mir::DebugWhat::Declare(address) => Some((
                address,
                one.variable,
                llrm_mir::debuginfo::variable_offset(metadata, unit.context, one.variable)?,
            )),
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
        let owner =
            declared.iter().filter(|(address, _, at)| *address == base && *at <= offset).max_by_key(|(_, _, at)| *at);
        if let Some(&(_, variable, at)) = owner {
            let bytes = unit.layout.alloc_size(&unit.context.types, plan.types[slot]) as u32;
            named[slot] = Some((variable, offset - at, bytes));
        }
    }
    // A variable one cell is all of is that value; a variable of several cells
    // has each as a piece.
    named
        .iter()
        .map(|one| {
            let (variable, offset, bytes) = (*one)?;
            let cells = named.iter().flatten().filter(|(other, ..)| *other == variable).count();
            Some(Named { variable, piece: (cells > 1 || offset != 0).then_some((offset as u32, bytes)) })
        })
        .collect()
}

/// A variable `-g` declared, and the bytes of it that a cell is when it is not
/// all.
#[derive(Clone, Copy, Debug)]
struct Named {
    variable: llrm_mir::MetadataId,
    piece: Option<(u32, u32)>,
}

impl Named {
    /// What a debugger is told when the cell has `value`.
    fn is(
        &self,
        value: Operand,
    ) -> llrm_mir::DebugWhat {
        match self.piece {
            Some((offset, bytes)) => llrm_mir::DebugWhat::Piece { value, offset, bytes },
            None => llrm_mir::DebugWhat::Value(value),
        }
    }
}

/// The type `inst` loads or stores.
fn accessed(
    unit: &Unit,
    inst: InstId,
) -> Option<TypeId> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { .. } => Some(instruction.ty),
        Opcode::Store { .. } => unit.operand_type(instruction.operands[0]),
        _ => None,
    }
}

fn is_load(
    unit: &Unit,
    inst: InstId,
) -> bool {
    matches!(unit.function.instruction(inst).opcode, Opcode::Load { .. })
}

/// Cells whose reads can use a known stored value: touched more than once,
/// loaded as one type, and available along every path to some load.
fn plan(
    unit: &Unit,
    accesses: &Accesses,
    aggregate_only: bool,
) -> Result<Plan, String> {
    let refs = &accesses.references;
    if !refs.keys().any(|&inst| is_load(unit, inst)) {
        return Ok(Plan::default());
    }
    let canonical = _canonical_leaf_types(refs.values());
    let blocked = _blocked(refs.values(), &canonical);
    let keys = refs
        .iter()
        .filter_map(|(&inst, reference)| _key(reference, &blocked, &canonical).map(|key| (inst, key)))
        .collect::<IndexMap<_, _>>();
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
        .filter(|(key, _)| {
            aggregates.as_ref().is_none_or(|objects| matches!(key, Key::Leaf(leaf) if objects.contains(&leaf.object)))
        })
        .map(|(key, _)| (key.clone(), *types[key].first().expect("one type")))
        .collect::<IndexMap<_, _>>();
    if candidates.is_empty() {
        return Ok(Plan::default());
    }

    // An access of another type is no definition or use, only a write.
    let slots = keys
        .iter()
        .filter_map(|(&inst, key)| {
            candidates
                .get_full(key)
                .filter(|(_, _, ty)| accessed(unit, inst) == Some(**ty))
                .map(|(slot, ..)| (inst, slot))
        })
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
    let stores = slots
        .iter()
        .filter(|(inst, slot)| !is_load(unit, **inst) && used.contains(slot))
        .map(|(&inst, slot)| (inst, renumbered[slot]))
        .collect();
    let loads = usable.into_iter().map(|inst| (inst, renumbered[&slots[&inst]])).collect();
    Ok(Plan { variables: Vec::new(), types, stores, loads })
}

/// Loads a stored value reaches on every path, with no write between that
/// may reach its cell.
fn _available(
    unit: &Unit,
    accesses: &Accesses,
    cells: &IndexMap<Key, TypeId>,
    typed: &[Option<(std::rc::Rc<str>, std::rc::Rc<[String]>)>],
    slots: &HashMap<InstId, usize>,
) -> HashSet<InstId> {
    let function = unit.function;
    let Some(entry) = function.entry().map(cfg::id) else { return HashSet::default() };
    let graph = cfg::graph(function);
    let dominance = cfg::Dominance::of(function);
    let reachable = graph
        .iter()
        .map(|block| block.at)
        .filter(|&at| dominance.reachable(unit.function, at))
        .collect::<BTreeSet<_>>();
    let predecessors = loops::predecessors(&graph);
    let refs = cells
        .iter()
        .zip(typed)
        .map(|((key, &ty), typed)| MemRef {
            typed: typed.as_ref().map(|one| one.0.clone()),
            lineage: typed.as_ref().map(|one| one.1.clone()).unwrap_or_default(),
            .._reference(key, unit.layout.store_size(&unit.context.types, ty) as u32)
        })
        .collect::<Vec<_>>();
    // The cells whose address a value is part of: its definition, a phi's
    // on a back edge among them, moves them.
    let mut based = HashMap::<ValueId, Vec<usize>>::default();
    for (at, reference) in refs.iter().enumerate() {
        let operands = [reference.root, reference.segment, reference.base.map(Operand::Value)];
        for value in operands
            .into_iter()
            .flatten()
            .filter_map(|one| if let Operand::Value(value) = one { Some(value) } else { None })
        {
            based.entry(value).or_default().push(at);
        }
    }
    // The cells a write may reach, by the buckets of the objects it may alias:
    // a store asked every cell that was available, N cells at each of N
    // stores. A cell outside the buckets the write reaches is one
    // `overlapping` rules out unseen.
    // Made when a write finds many cells available: putting every cell in a
    // bucket costs more than asking the few that are (host.c: promote 245
    // -> 605 Minstr when it was made for each function).
    let indexed = std::cell::OnceCell::new();
    let reached = RefCell::new(HashMap::<InstId, Option<Vec<usize>>>::default());
    let reach = |inst: InstId, writes: &[MemRef]| -> Option<Vec<usize>> {
        let index = indexed.get_or_init(|| {
            let mut buckets = regions::OverlapBuckets::default();
            llrm_analysis::cellmap::CellMap::<usize, (), regions::OverlapBucket>::new(
                (0..refs.len()).map(|at| (at, ())).collect(),
                |at| {
                    (
                        regions::overlap_bucket(&mut buckets, &refs[*at]),
                        regions::overlap_span(&refs[*at]).or_else(|| regions::hull_span(&refs[*at])),
                    )
                },
            )
        });
        reached
            .borrow_mut()
            .entry(inst)
            .or_insert_with(|| {
                let mut cells = Vec::new();
                for written in writes {
                    match regions::overlap_buckets(written, &index.parts) {
                        None => return None,
                        // A cell at fixed bytes of an object (or of a frame) is
                        // asked only if its bytes meet the write's: N cells of
                        // one array, N writes, were N squared questions.
                        Some(picked) => {
                            let mut near = regions::hull_buckets(written, &index.parts);
                            near.extend(regions::displaced_buckets(written, &index.parts));
                            let mut rest = picked.clone();
                            for (held, span) in near {
                                let (here, others): (Vec<_>, Vec<_>) =
                                    rest.into_iter().partition(|bucket| held.contains(bucket));
                                rest = others;
                                cells.extend(index.asked(Some(here), Some((held, span))).into_iter().copied());
                            }
                            cells.extend(index.asked(Some(rest), None).into_iter().copied());
                        }
                    }
                }
                cells.sort_unstable();
                cells.dedup();
                Some(cells)
            })
            .clone()
    };
    let mut every = Bits::new(cells.len());
    (0..cells.len()).for_each(|at| every.insert(at));
    let mut leaving = reachable.iter().map(|at| (*at, every.clone())).collect::<BTreeMap<_, _>>();
    // Whether a write may reach a cell, per (instruction, cell): every round
    // asks again.
    let clobbered = RefCell::new(HashMap::<(InstId, usize), bool>::default());

    let entering = |at: i64, leaving: &BTreeMap<i64, Bits>| -> Bits {
        let parents =
            predecessors.get(&at).into_iter().flatten().filter(|parent| reachable.contains(parent)).collect::<Vec<_>>();
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
        for inst in function.iter_block(cfg::block(at)) {
            let instruction = function.instruction(inst);
            let store = matches!(instruction.opcode, Opcode::Store { .. });
            // A volatile access writes its own bytes: it orders, it does not
            // clobber.
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
                // The cells available are asked one by one while they are few;
                // past `MANY`, the buckets the write reaches
                // say which could be hit.
                const MANY: usize = 24;
                let near = if available.iter().nth(MANY).is_some() { reach(inst, writes) } else { None };
                let asked: Vec<usize> = match &near {
                    Some(cells) => cells.iter().copied().filter(|&at| available.contains(at)).collect(),
                    None => available.iter().collect(),
                };
                if llrm_support::env_flag!("LLRM_CHECK_PROMOTED") && near.is_some() {
                    let asked: HashSet<usize> = asked.iter().copied().collect();
                    for at in available.iter().filter(|at| !asked.contains(at)) {
                        assert!(
                            !writes.iter().any(|written| regions::overlapping(
                                &refs[at],
                                written,
                                None,
                                None,
                                unit.program
                            )
                            .unwrap_or(true)),
                            "LLRM_CHECK_PROMOTED: a cell the write reaches was not asked"
                        );
                    }
                }
                let gone = asked
                    .into_iter()
                    .filter(|&at| {
                        *clobbered
                            .borrow_mut()
                            .entry((inst, at))
                            .or_insert_with(
                                || writes.iter().any(|written| {
                                    regions::overlapping(&refs[at], written, None, None, unit.program).unwrap_or(true)
                                }),
                            )
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
fn edges(
    function: &Function,
    block: BlockId,
) -> Vec<BlockId> {
    function
        .block_users(block)
        .iter()
        .filter(|one| function.instruction(one.user).opcode.is_terminator())
        .filter_map(|one| function.parent(one.user))
        .collect()
}

#[cfg(test)]
thread_local! {
    /// Blocks `live_in` has worked out, for a test that the work is not a
    /// block's visit for each cell.
    pub(crate) static LIVE_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// For each block, the cells live on entering it: where a load of a cell is
/// reached before any store of it, along the paths that have none.
struct Live {
    place: HashMap<i64, usize>,
    on_entry: Vec<llrm_support::bits::Bits>,
}

impl Live {
    fn contains(
        &self,
        cell: usize,
        block: i64,
    ) -> bool {
        self.place.get(&block).is_some_and(|&at| self.on_entry[at].contains(cell))
    }
}

/// Where each cell is live: a backward dataflow of cells over the blocks (a
/// block reads the cells it loads before storing, kills the cells it stores,
/// and is entered with what its successors are entered with), one bit set a
/// block and not one set of blocks a cell.
fn live_in(
    function: &Function,
    plan: &Plan,
) -> Live {
    let cells = plan.types.len();
    let blocks = function.layout().to_vec();
    let place: HashMap<i64, usize> = blocks.iter().enumerate().map(|(at, &block)| (cfg::id(block), at)).collect();
    let mut reads = vec![llrm_support::bits::Bits::new(cells); blocks.len()];
    let mut kills = vec![llrm_support::bits::Bits::new(cells); blocks.len()];
    for (at, &block) in blocks.iter().enumerate() {
        for inst in function.iter_block(block) {
            if let Some(&slot) = plan.loads.get(&inst) {
                if !kills[at].contains(slot) {
                    reads[at].insert(slot);
                }
            } else if let Some(&slot) = plan.stores.get(&inst) {
                kills[at].insert(slot);
            }
        }
    }
    let mut on_entry = reads.clone();
    let mut work: Vec<usize> = (0..blocks.len()).collect();
    let mut queued = vec![true; blocks.len()];
    while let Some(at) = work.pop() {
        queued[at] = false;
        #[cfg(test)]
        LIVE_VISITS.with(|visits| visits.set(visits.get() + 1));
        let mut now = llrm_support::bits::Bits::new(cells);
        for after in function.successors(blocks[at]) {
            if let Some(&next) = place.get(&cfg::id(after)) {
                now.union_with(&on_entry[next]);
            }
        }
        now.subtract(&kills[at]);
        now.union_with(&reads[at]);
        if now != on_entry[at] {
            on_entry[at] = now;
            for before in function.predecessors(blocks[at]) {
                if let Some(&previous) = place.get(&cfg::id(before))
                    && !std::mem::replace(&mut queued[previous], true)
                {
                    work.push(previous);
                }
            }
        }
    }
    Live { place, on_entry }
}

/// `function` with each planned load its cell's stored value: phis where
/// the stores' dominance frontiers put them, named down the dominator tree.
fn rewrite(
    context: &Context,
    function: &mut Function,
    plan: &Plan,
) {
    let entry = cfg::id(function.entry().expect("a defined function"));
    let dominance = cfg::Dominance::of(function);
    let frontiers = dominance.frontiers(function);
    let idom = dominance.immediate_dominators(function);
    let existing = function
        .walk()
        .map(|(_, inst)| function.instruction(inst))
        .filter(|one| one.opcode == Opcode::Phi)
        .filter_map(|one| one.result)
        .collect::<llrm_mir::dense::IdSet<ValueId>>();
    // The entry value of a cell is `UndefValue` of its type.
    let poison = plan
        .types
        .iter()
        .map(|&ty| {
            Operand::Constant(
                context.constant(llrm_mir::context::Constant { ty, kind: llrm_mir::context::ConstantKind::Undef }),
            )
        })
        .collect::<Vec<_>>();

    // Where each cell is live on entering a block: LLVM's PromoteMemToReg
    // places a phi only there. A cell stored and read inside one arm of a
    // switch of N cases is live at no join, and a phi of N inputs at the
    // join for each such cell was N^2 data made to be pruned.
    let live = live_in(function, plan);
    // Each phi placed, by block and cell, with the block each input comes from.
    let mut phis = HashMap::<(i64, usize), (InstId, Vec<BlockId>)>::default();
    let mut named_phis = Vec::new();
    for (slot, &ty) in plan.types.iter().enumerate() {
        let defining = plan
            .stores
            .iter()
            .filter(|(_, one)| **one == slot)
            .filter_map(|(inst, _)| function.parent(*inst))
            .map(cfg::id)
            .collect::<BTreeSet<_>>();
        let mut work = defining.iter().copied().collect::<Vec<_>>();
        let mut placed = BTreeSet::new();
        while let Some(at) = work.pop() {
            for &frontier in frontiers.get(&at).into_iter().flatten() {
                // A cell `-g` names keeps its phis, live or not: the debugger
                // is told where the variable is by the phi, and
                // by the record it leaves when it goes.
                let pruned = plan.variables.get(slot).copied().flatten().is_none() && !live.contains(slot, frontier);
                if pruned || !placed.insert(frontier) {
                    continue;
                }
                let block = cfg::block(frontier);
                let from = edges(function, block);
                let operands = from.iter().flat_map(|&one| [poison[slot], Operand::Block(one)]).collect();
                let phi = function.create_instruction(Opcode::Phi, ty, operands, Flags::default(), None);
                let first = function.block(block).first();
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

    // What the debugger is told of a variable: the phi that merges its paths,
    // from the top of the block on.
    for (block, phi, named) in named_phis {
        let value = function.instruction(phi).result.expect("a phi's value");
        let first = function.iter_block(block).find(|&one| function.instruction(one).opcode != Opcode::Phi);
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
    // The phis of each block, by cell, so that a block asks its own and not
    // every cell's.
    let mut phis_at = HashMap::<i64, Vec<usize>>::default();
    for &(at, slot) in phis.keys() {
        phis_at.entry(at).or_default().push(slot);
    }
    phis_at.values_mut().for_each(|slots| slots.sort_unstable());
    let mut dead = Vec::new();
    // The value each cell has, down the dominator tree: a block changes a few,
    // which are put back on leaving it. A copy of every cell's value for
    // each child was blocks x cells.
    enum Visit {
        Enter(i64),
        Leave(usize),
    }
    let mut current = poison.clone();
    let mut undo = Vec::<(usize, Operand)>::new();
    let mut stack = vec![Visit::Enter(entry)];
    while let Some(visit) = stack.pop() {
        let at = match visit {
            Visit::Leave(marked) => {
                while undo.len() > marked {
                    let (slot, old) = undo.pop().expect("an entry above the mark");
                    current[slot] = old;
                }
                continue;
            }
            Visit::Enter(at) => at,
        };
        stack.push(Visit::Leave(undo.len()));
        let block = cfg::block(at);
        for &slot in phis_at.get(&at).into_iter().flatten() {
            let (phi, _) = &phis[&(at, slot)];
            undo.push((slot, current[slot]));
            current[slot] = Operand::Value(function.instruction(*phi).result.expect("a phi's value"));
        }
        for inst in function.iter_block(block).collect::<Vec<_>>() {
            if let Some(&slot) = plan.loads.get(&inst) {
                function.replace_value(function.instruction(inst).result.expect("a load's value"), current[slot]);
                dead.push(inst);
            } else if let Some(&slot) = plan.stores.get(&inst) {
                undo.push((slot, current[slot]));
                current[slot] = function.instruction(inst).operands[0];
                // And the value it stores, from the instruction after it on.
                if let Some(named) = plan.variables.get(slot).copied().flatten()
                    && let Some(next) = function.next_in_block(inst)
                {
                    function.add_debug_record_first(next, named.variable, named.is(current[slot]));
                }
            }
        }
        for successor in function.successors(block) {
            for &slot in phis_at.get(&cfg::id(successor)).into_iter().flatten() {
                let (phi, from) = &phis[&(cfg::id(successor), slot)];
                for (index, _) in from.iter().enumerate().filter(|(_, one)| **one == block) {
                    function.set_operand(*phi, 2 * index, current[slot]);
                }
            }
        }
        stack.extend(children.get(&at).into_iter().flatten().map(|&child| Visit::Enter(child)));
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
