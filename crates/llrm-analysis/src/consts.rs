//! Which values are known, and what they are: llrm-core's
//! `analysis/consts.rs`, a port of `qbopt/analysis/consts.py`, adapted to
//! the rich MIR. A fact is a width as well as a number -- "the low `width`
//! bits of this value are `n`" -- and an operation only folds where the
//! widths agree. A width is in bits; a memory cell's in bytes.
//!
//! What memory holds is solved alongside, when the caller says what each
//! call writes (`Calls`): a cell is a fixed address in an object, its root
//! an alloca or a global, so its object is its root's.
//!
//! Old operations with no rich MIR counterpart, and what used them: a
//! read-modify-write of a cell (`updated`), a carry between halves
//! (`AddCarry`, `_carry`), `Concat`, `Smulhi`, a memory operand of an
//! arithmetic operation (a `load` is its own instruction), the known values
//! a call leaves in memory (`memory_values`), and the constants memory held
//! at program start (`body.initial`: a rich MIR function is no program's
//! start). `named_bytes` found which object an address was in; a cell's
//! root is its object. `reusing`, `_reuse` and `shared_cells` cached a
//! solve by body address, which a function changed in place no longer
//! keeps: `manager` caches through the pass manager instead.
//!
//! Tests skipped: `test_constant_analysis_scope_reuses_an_unchanged_body_without_sharing_mutation`
//! (`reusing`); `test_pointer_displacement_constants_preserve_order_and_width`,
//! `test_constant_subtraction_preserves_operand_order`,
//! `test_constant_operand_keeps_its_memory_address_dependency`,
//! `test_a_known_factor_becomes_a_multiply_operand` and
//! `test_signed_widening_produces_a_whole_long_constant`'s folding half
//! (optimize/transform's, whose port they wait for);
//! `test_recovered_argument_constants` (`Concat`);
//! `test_flags_do_not_stop_an_operation_being_folded` (a flags result);
//! BC object corpora: `test_relocated_descriptor_address_is_not_integer_zero`,
//! `test_folded_extraction_has_no_implicit_machine_result`,
//! `test_a_fact_never_claims_more_bytes_than_the_instruction_wrote`,
//! `test_every_known_value_is_defined_by_an_operation_that_computes_it`,
//! `test_a_comparison_result_folds_to_basics_own_true`,
//! `test_nothing_is_folded_through_a_phi`.
//! `test_a_call_reaching_nonlocal_keeps_an_uncaptured_static_constant` is
//! ported with a frame object, which a callee's nonlocal reach misses: no
//! rich MIR global is uncaptured.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_mir::types::{FloatKind, Type};
use llrm_support::hash::{HashMap, HashSet, IndexMap};
use num_bigint::BigInt;

use crate::avail;
use crate::cellmap::CellMap;
use crate::cfg;
use crate::constant_cycles;
use crate::memory::{Addr, MemRef, Provenance, Unit, object_of, unmodeled_write};
use crate::memoryssa::{self, Accesses};
use crate::ranges::{self, Interval};
use crate::regions::{ByteRange, OverlapBucket, displaced_buckets, object_bucket, overlap_buckets, overlap_span, overlapping};

type Binary = fn(&BigInt, &BigInt) -> BigInt;

/// What each operation does to two known numbers, within one width.
pub static ARITH: [(BinaryOp, Binary); 8] = [
    (BinaryOp::Add, |a, b| a + b),
    (BinaryOp::Sub, |a, b| a - b),
    (BinaryOp::And, |a, b| a & b),
    (BinaryOp::Or, |a, b| a | b),
    (BinaryOp::Xor, |a, b| a ^ b),
    (BinaryOp::Shl, |a, b| a << u32::try_from(b & BigInt::from(31)).expect("five bits")),
    (BinaryOp::LShr, |a, b| (a & BigInt::from(0xFFFF_FFFF_u32)) >> u32::try_from(b & BigInt::from(31)).expect("five bits")),
    (BinaryOp::Mul, |a, b| a * b),
];

/// Memory facts are stored as bytes so partial writes and control-flow
/// joins do not discard an untouched neighbor. A cell is its address and
/// its width in bytes.
pub type Cells = IndexMap<(Addr, u32), Known>;

/// What each instruction sees in memory; runs of instructions share one map.
pub type HeldCells = IndexMap<InstId, Rc<Cells>>;

/// What each call writes, where `alias::calls_annotated` found it. A call
/// not here writes what `memory::unmodeled_write` says: everything, or
/// nothing.
pub type Calls = IndexMap<InstId, Vec<MemRef>>;

/// Alias questions for one immutable known-value epoch.
pub struct _MemoryQueries<'a> {
    pub unit: Unit<'a>,
    pub known: IndexMap<ValueId, Known>,
    pub facts: BTreeMap<ValueId, Interval>,
    /// Each reference asked, resolved; kept, so a resolved one's address
    /// is its identity in `overlaps`.
    pub addressed: HashMap<MemRef, Rc<MemRef>>,
    pub overlaps: HashMap<((Addr, u32), usize), bool>,
    pub places: HashMap<(Addr, u32), (OverlapBucket, Option<ByteRange>)>,
}

/// Cells indexed by this epoch's buckets; see `_MemoryQueries::owned`.
pub type IndexedCells = CellMap<(Addr, u32), Known, OverlapBucket>;

/// What `_kills` is handed: Python's `here` is a dict, or a `CellMap` an
/// earlier operation of the same walk already indexed.
pub enum Here {
    Plain(Cells),
    Indexed(IndexedCells),
}

impl Here {
    pub fn cells(&self) -> &Cells {
        match self {
            Here::Plain(cells) => cells,
            Here::Indexed(cells) => cells,
        }
    }

    pub fn into_cells(self) -> Cells {
        match self {
            Here::Plain(cells) => cells,
            Here::Indexed(cells) => cells.into_items(),
        }
    }
}

impl<'a> _MemoryQueries<'a> {
    pub fn new(unit: Unit<'a>, known: &IndexMap<ValueId, Known>) -> Self {
        Self {
            unit,
            known: known.clone(),
            facts: _intervals(known),
            addressed: HashMap::default(),
            overlaps: HashMap::default(),
            places: HashMap::default(),
        }
    }

    pub fn resolve(&mut self, reference: &MemRef) -> Rc<MemRef> {
        if let Some(saved) = self.addressed.get(reference) {
            return Rc::clone(saved);
        }
        let made = Rc::new(_addressed(&self.unit, reference, &self.known));
        self.addressed.insert(reference.clone(), Rc::clone(&made));
        made
    }

    /// The access cell `where_` is, carrying its object.
    fn cell(&self, where_: (Addr, u32)) -> MemRef {
        let (addr, width) = where_;
        let mut cell = MemRef { disp: addr.disp, ..MemRef::at(&self.unit, addr.root, width) };
        cell.provenance = object_of(&self.unit, addr.root).map(|object| {
            Provenance::one_with_slice(object, addr.disp, addr.disp + i64::from(width), 1, 1, BTreeSet::new()).expect("a cell is at least one byte")
        });
        cell
    }

    /// Python `bucket` and `span`: cell `where_`'s `overlap_bucket`, from
    /// its root's object, and its `overlap_span`.
    ///
    /// Remembered: interning a bucket hashes its object.
    pub fn place(&mut self, where_: (Addr, u32)) -> (OverlapBucket, Option<ByteRange>) {
        if let Some(place) = self.places.get(&where_) {
            return *place;
        }
        let bucket = object_bucket(object_of(&self.unit, where_.0.root), Some((where_.0.root, None)));
        let place = (bucket, overlap_span(&self.cell(where_)));
        self.places.insert(where_, place);
        place
    }

    /// `here` indexed by this epoch's buckets, for one operation to change.
    pub fn owned(&mut self, here: Here) -> IndexedCells {
        match here {
            Here::Indexed(cells) => cells,
            Here::Plain(cells) => CellMap::new(cells, |where_| self.place(*where_)),
        }
    }

    pub fn may_overlap(&mut self, where_: (Addr, u32), reference: &Rc<MemRef>) -> bool {
        #[cfg(test)]
        MAY_OVERLAP.with(|asked| asked.set(asked.get() + 1));
        let key = (where_, Rc::as_ptr(reference) as usize);
        if let Some(&answer) = self.overlaps.get(&key) {
            return answer;
        }
        let answer = self._overlap(where_, reference);
        self.overlaps.insert(key, answer);
        answer
    }

    fn _overlap(&mut self, where_: (Addr, u32), reference: &Rc<MemRef>) -> bool {
        // An answer Rust cannot represent is taken to overlap.
        overlapping(&self.cell(where_), reference, Some(&self.facts), Some(&self.facts), self.unit.machine).unwrap_or(true)
    }
}

/// The low `width` bits of a value are `n`. Nothing is said above them.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct Known {
    pub n: BigInt,
    pub width: u32,
}

impl Known {
    pub fn new(n: impl Into<BigInt>, width: u32) -> Self {
        Self { n: n.into(), width }
    }
}

impl fmt::Debug for Known {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#x}:{}", self.n, self.width)
    }
}

pub fn masked(n: &BigInt, width: u32) -> BigInt {
    n & ((BigInt::from(1) << width) - 1)
}

/// An integer operand's width in bits.
fn _width(unit: &Unit, operand: Operand) -> Option<u32> {
    unit.int_bits(operand)
}

/// Integer quotient and remainder of the `sdiv`, `udiv`, `srem` or `urem`
/// `inst`, excluding the faulting cases.
pub fn division(unit: &Unit, inst: InstId, known: &IndexMap<ValueId, Known>) -> Option<(BigInt, BigInt)> {
    let op = unit.function.instruction(inst);
    let signed = match op.opcode {
        Opcode::Binary(BinaryOp::SDiv | BinaryOp::SRem) => true,
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem) => false,
        _ => return None,
    };
    let width = _width(unit, Operand::Value(op.result?))?;
    let operands = op.operands.iter().map(|&one| _operand(unit, one, known, None)).collect::<Option<Vec<_>>>()?;
    if operands.iter().any(|fact| fact.width < width) {
        return None;
    }
    let (dividend, divisor) = if signed {
        let sign = BigInt::from(1) << (width - 1);
        ((masked(&operands[0].n, width) ^ &sign) - &sign, (masked(&operands[1].n, width) ^ &sign) - &sign)
    } else {
        (masked(&operands[0].n, width), masked(&operands[1].n, width))
    };
    let zero = BigInt::from(0);
    if divisor == zero || (signed && dividend == -(BigInt::from(1) << (width - 1)) && divisor == BigInt::from(-1)) {
        return None;
    }
    let absolute = |n: &BigInt| if *n < BigInt::from(0) { -n } else { n.clone() };
    let mut quotient = absolute(&dividend) / absolute(&divisor);
    if signed && (dividend < zero) != (divisor < zero) {
        quotient = -quotient;
    }
    let remainder = &dividend - &quotient * &divisor;
    Some((masked(&quotient, width), masked(&remainder, width)))
}

/// What this store puts in its cell, where that is a number: a float's
/// bits too, which floatfacts supplies for a computed value.
fn _put(unit: &Unit, inst: InstId, known: &IndexMap<ValueId, Known>) -> Option<Known> {
    let op = unit.function.instruction(inst);
    if !matches!(op.opcode, Opcode::Store { .. }) {
        return None;
    }
    _operand(unit, op.operands[0], known, None).or_else(|| _float_bits(unit, op.operands[0], known))
}

/// A float operand's bits: a constant's, or what `known` says of a value.
fn _float_bits(unit: &Unit, operand: Operand, known: &IndexMap<ValueId, Known>) -> Option<Known> {
    let width = match unit.context.types.get(unit.operand_type(operand)?) {
        Type::Float(FloatKind::Float) => 32,
        Type::Float(FloatKind::Double) => 64,
        _ => return None,
    };
    match operand {
        Operand::Constant(id) => match unit.context.get(id).kind {
            ConstantKind::Float(bits) => Some(Known::new(bits, width)),
            _ => None,
        },
        Operand::Value(value) => _read(known.get(&value), width),
        Operand::Block(_) => None,
    }
}

/// The complete value a direct constant store writes to a contained cell.
pub fn initialized(unit: &Unit, inst: InstId, reference: &MemRef) -> Option<Known> {
    let op = unit.function.instruction(inst);
    if !matches!(op.opcode, Opcode::Store { volatile: false, .. }) {
        return None;
    }
    let written = MemRef::of(unit, inst)?;
    let addr = written.addr()?;
    let fact = _put(unit, inst, &IndexMap::default())?;
    _cell(&Cells::from_iter([((addr, written.width), fact)]), reference)
}

/// `fact` as byte cells at `reference`'s fixed address.
pub fn _fragments(reference: &MemRef, fact: &Known) -> Cells {
    let addr = reference.addr().expect("a fragment is of an addressed cell");
    (0..reference.width.min(fact.width / 8))
        .map(|offset| ((addr.plus(i64::from(offset)), 1), Known::new((&fact.n >> (offset * 8)) & BigInt::from(255), 8)))
        .collect()
}

/// A far store's selector, where nothing yet says which segment it is
/// and it is still one this run may take on faith.
fn _selector(reference: &MemRef, known: &IndexMap<ValueId, Known>, allowed: Option<&BTreeSet<ValueId>>) -> Option<ValueId> {
    let Some(Operand::Value(segment)) = reference.segment else {
        return None;
    };
    if reference.space != 1 || known.contains_key(&segment) || allowed.is_some_and(|allowed| !allowed.contains(&segment)) {
        return None;
    }
    Some(segment)
}

/// Every value an instruction that writes memory reads: `_kills` asks
/// `known` of no other.
fn _memory_reads(unit: &Unit) -> HashSet<ValueId> {
    let function = unit.function;
    let mut read = HashSet::default();
    for (_, inst) in function.walk() {
        let op = function.instruction(inst);
        if !matches!(op.opcode, Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_)) {
            continue;
        }
        read.extend(op.operands.iter().filter_map(|operand| match operand {
            Operand::Value(value) => Some(*value),
            _ => None,
        }));
        if let Some(reference) = MemRef::of(unit, inst) {
            read.extend(reference.base);
            if let Some(Operand::Value(segment)) = reference.segment {
                read.insert(segment);
            }
        }
    }
    read
}

/// What each value is, as the alias lattice asks for it.
fn _intervals(known: &IndexMap<ValueId, Known>) -> BTreeMap<ValueId, Interval> {
    known.iter().map(|(value, fact)| (*value, Interval { low: fact.n.clone(), high: fact.n.clone(), width: fact.width })).collect()
}

/// Alias questions about `unit`'s cells, each cell carrying its object.
pub fn memory_queries<'a>(unit: Unit<'a>, known: &IndexMap<ValueId, Known>) -> _MemoryQueries<'a> {
    _MemoryQueries::new(unit, known)
}

/// Whether `inst` is a call or invoke.
fn is_call(unit: &Unit, inst: InstId) -> bool {
    matches!(unit.function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_))
}

/// The cell facts still standing after this instruction.
///
/// `assume` collects the far selectors this took on faith; the caller
/// checks afterwards that every one of them did resolve.
#[allow(clippy::too_many_arguments)]
pub fn _kills(
    here: Cells,
    inst: InstId,
    known: &IndexMap<ValueId, Known>,
    calls: &Calls,
    assume: Option<&mut BTreeSet<ValueId>>,
    allowed: Option<&BTreeSet<ValueId>>,
    edge_facts: bool,
    queries: &mut _MemoryQueries,
) -> Cells {
    _killed(Here::Plain(here), inst, known, calls, assume, allowed, edge_facts, queries).into_cells()
}

/// `_kills` over what the walk holds: Python's `here` is a dict or a
/// `CellMap`, and a `CellMap` goes back out.
#[allow(clippy::too_many_arguments)]
fn _killed(
    mut here: Here,
    inst: InstId,
    known: &IndexMap<ValueId, Known>,
    calls: &Calls,
    mut assume: Option<&mut BTreeSet<ValueId>>,
    allowed: Option<&BTreeSet<ValueId>>,
    edge_facts: bool,
    queries: &mut _MemoryQueries,
) -> Here {
    let unit = queries.unit;
    let call = is_call(&unit, inst);
    // A fact supplied for one CFG edge is a proof about reaching that edge,
    // not a durable summary of a callee.
    if edge_facts && call {
        here = Here::Plain(Cells::default());
    }
    if unmodeled_write(&unit, inst) && !calls.contains_key(&inst) {
        here = Here::Plain(Cells::default());
    }
    let stores = match calls.get(&inst) {
        Some(stores) => stores.clone(),
        None if call => Vec::new(),
        None => unit.reference(inst).filter(|_| matches!(unit.function.instruction(inst).opcode, Opcode::Store { .. })).into_iter().collect(),
    };
    // Only a write changes a cell.
    if stores.is_empty() {
        return here;
    }
    let put = _put(&unit, inst, known);
    for reference in &stores {
        let reference = queries.resolve(reference);
        if let Some(assume) = assume.as_deref_mut() {
            if let Some(selector) = _selector(&reference, known, allowed) {
                // A cell in `here` is always in a program object, so an
                // absolute segment reaches none of them.
                assume.insert(selector);
                continue;
            }
        }
        let mut owned = queries.owned(here);
        let reached = overlap_buckets(&reference, &owned.parts);
        let displaced = displaced_buckets(&reference, &owned.parts);
        owned.kill(reached, |where_| queries.may_overlap(*where_, &reference), displaced);
        if let Some(put) = &put {
            if reference.object && reference.addr().is_some() {
                for (where_, fact) in _fragments(&reference, put) {
                    owned.insert(where_, fact, |where_| queries.place(*where_));
                }
            }
        }
        here = Here::Indexed(owned);
    }
    here
}

/// What each memory cell holds before each instruction, where it is a
/// number.
///
/// Forward to a fixed point, meeting at a join on agreement. A block none
/// of whose predecessors have been visited yet is deferred, not treated as
/// knowing nothing.
#[allow(clippy::too_many_arguments)]
pub fn cells(
    unit: &Unit,
    calls: &Calls,
    known: Option<&IndexMap<ValueId, Known>>,
    initial: Option<&Cells>,
    edges: Option<&IndexMap<(i64, i64), Cells>>,
    mut assume: Option<&mut BTreeSet<ValueId>>,
    allowed: Option<&BTreeSet<ValueId>>,
) -> HeldCells {
    let function = unit.function;
    let Some(entry) = function.entry().map(cfg::id) else {
        return HeldCells::default();
    };
    let empty = IndexMap::default();
    let known = known.unwrap_or(&empty);
    let mut queries = memory_queries(*unit, known);
    let initial = initial.cloned().unwrap_or_default();
    // Kept indexed: an edge from a lone predecessor hands its map on as it is.
    let graph = cfg::graph(function);
    let mut outof = graph.iter().map(|block| (block.at, None)).collect::<IndexMap<i64, Option<IndexedCells>>>();
    let preds = graph.iter().map(|block| (block.at, graph.iter().filter(|one| one.succ.contains(&block.at)).map(|one| one.at).collect::<Vec<_>>())).collect::<IndexMap<_, _>>();
    let no_edges = IndexMap::default();
    let edge_map = edges.unwrap_or(&no_edges);
    let edge_facts = edges.is_some_and(|edges| !edges.is_empty());

    let entering = |outof: &IndexMap<i64, Option<IndexedCells>>, at: i64| -> Option<Here> {
        if preds[&at].is_empty() {
            return Some(Here::Plain(if at == entry { initial.clone() } else { Cells::default() }));
        }
        let mut seen: Vec<std::borrow::Cow<Cells>> = Vec::new();
        let mut sole = None;
        for one in &preds[&at] {
            let Some(here) = &outof[one] else {
                continue;
            };
            let none = Cells::default();
            let extra = edge_map.get(&(*one, at)).unwrap_or(&none);
            if extra.is_empty() {
                sole = Some(here);
                seen.push(std::borrow::Cow::Borrowed(&**here));
                continue;
            }
            let mut here = here
                .iter()
                .filter(|(where_, _)| !(0..where_.1).any(|offset| extra.contains_key(&(where_.0.plus(i64::from(offset)), 1))))
                .map(|(where_, fact)| (*where_, fact.clone()))
                .collect::<Cells>();
            for (where_, fact) in extra {
                here.insert(*where_, fact.clone());
            }
            seen.push(std::borrow::Cow::Owned(here));
        }
        if at == entry {
            seen.push(std::borrow::Cow::Borrowed(&initial));
        }
        if let ([std::borrow::Cow::Borrowed(_)], Some(sole)) = (seen.as_slice(), sole) {
            if at != entry {
                return Some(Here::Indexed(sole.clone()));
            }
        }
        let first = seen.first()?;
        Some(Here::Plain(
            first.iter().filter(|(where_, fact)| seen[1..].iter().all(|one| one.get(*where_) == Some(*fact))).map(|(where_, fact)| (*where_, fact.clone())).collect(),
        ))
    };

    // A worklist in reverse postorder, as LLVM's dataflow solvers drain theirs:
    // a block runs again only when what enters it changed, not every round.
    let order = loops_order(&graph, entry);
    let rank = order.iter().enumerate().map(|(rank, at)| (*at, rank)).collect::<HashMap<_, _>>();
    let successors = graph.iter().map(|block| (block.at, block.succ.clone())).collect::<HashMap<_, _>>();
    let mut waiting = (0..order.len()).collect::<BTreeSet<_>>();
    while let Some(next) = waiting.pop_first() {
        let at = order[next];
        let Some(mut here) = entering(&outof, at) else {
            continue;
        };
        for &inst in function.block(cfg::block(at)).instructions() {
            here = _killed(here, inst, known, calls, assume.as_deref_mut(), allowed, edge_facts, &mut queries);
        }
        if outof[&at].as_deref() != Some(here.cells()) {
            outof.insert(at, Some(queries.owned(here)));
            waiting.extend(successors[&at].iter().filter_map(|at| rank.get(at).copied()));
        }
    }

    let mut found = IndexMap::default();
    for block in &graph {
        let mut here = entering(&outof, block.at).unwrap_or(Here::Plain(Cells::default()));
        // Instructions between two writes see one map, shared rather than copied per instruction.
        let mut shared: Option<Rc<Cells>> = None;
        for &inst in function.block(cfg::block(block.at)).instructions() {
            found.insert(inst, Rc::clone(shared.get_or_insert_with(|| Rc::new(here.cells().clone()))));
            let writes = is_call(unit, inst) || unmodeled_write(unit, inst) || matches!(function.instruction(inst).opcode, Opcode::Store { .. });
            if writes {
                shared = None;
            }
            here = _killed(here, inst, known, calls, assume.as_deref_mut(), allowed, edge_facts, &mut queries);
        }
    }
    found
}

fn loops_order(graph: &[cfg::Block], entry: i64) -> Vec<i64> {
    llrm_graph::loops::reverse_postorder(graph, entry)
}

fn _read(fact: Option<&Known>, width: u32) -> Option<Known> {
    let fact = fact?;
    if fact.width < width {
        return None;
    }
    Some(Known::new(masked(&fact.n, width), width))
}

/// What `here` says the bytes `reference` reads hold, where every one is known.
pub fn _cell(here: &Cells, reference: &MemRef) -> Option<Known> {
    let addr = reference.addr()?;
    let bits = reference.width * 8;
    if let Some(exact) = _read(here.get(&(addr, reference.width)), bits) {
        return Some(exact);
    }
    let mut number = BigInt::from(0);
    for offset in 0..reference.width {
        let wanted = addr.plus(i64::from(offset));
        let fragments = here
            .iter()
            .flat_map(|((address, width), fact)| {
                (0..(*width).min(fact.width / 8))
                    .filter(move |byte| address.plus(i64::from(*byte)) == wanted)
                    .map(move |byte| (&fact.n >> (8 * byte)) & BigInt::from(255))
            })
            .collect::<BTreeSet<_>>();
        if fragments.len() != 1 {
            return None;
        }
        number |= fragments.into_iter().next().expect("one fragment") << (8 * offset);
    }
    Some(Known::new(number, bits))
}

/// Resolve one proven constant index using the no-wrap address proof.
fn _addressed(unit: &Unit, reference: &MemRef, known: &IndexMap<ValueId, Known>) -> MemRef {
    let Some(base) = reference.base else {
        return reference.clone();
    };
    let interval = ranges::_operand(unit, Operand::Value(base), &IndexMap::default(), known);
    let Some(interval) = interval else {
        return reference.clone();
    };
    ranges::covering(reference, &BTreeMap::from([(base, interval)])).into_owned()
}

/// One operand as a number, if it is one.
pub fn _operand(unit: &Unit, one: Operand, known: &IndexMap<ValueId, Known>, _here: Option<&Cells>) -> Option<Known> {
    let width = _width(unit, one)?;
    if let Some(bits) = unit.int_constant(one) {
        return Some(Known::new(masked(&BigInt::from(bits), width), width));
    }
    match one {
        Operand::Value(value) => _read(known.get(&value), width),
        _ => None,
    }
}

/// The integer value this instruction defines.
pub fn _defined(unit: &Unit, inst: InstId) -> Option<ValueId> {
    let result = unit.function.instruction(inst).result?;
    _width(unit, Operand::Value(result)).map(|_| result)
}

/// What this instruction computes, where every input is known.
pub fn _result(unit: &Unit, inst: InstId, known: &IndexMap<ValueId, Known>, here: Option<&Cells>) -> Option<Known> {
    let result = _defined(unit, inst)?;
    let width = _width(unit, Operand::Value(result))?;
    let op = unit.function.instruction(inst);
    if matches!(op.opcode, Opcode::Load { volatile: false, .. }) {
        let reference = MemRef::of(unit, inst)?;
        return _read(_cell(here?, &_addressed(unit, &reference, known)).as_ref(), width);
    }
    if op.opcode == Opcode::Cast(CastOp::Trunc) {
        return _read(_operand(unit, op.operands[0], known, here).as_ref(), width);
    }
    if let Opcode::ICmp(predicate) = op.opcode {
        let (left, right) = (_operand(unit, op.operands[0], known, here)?, _operand(unit, op.operands[1], known, here)?);
        return Some(Known::new(u8::from(holds(predicate, &left, &right)), 1));
    }
    if matches!(op.opcode, Opcode::Binary(BinaryOp::Xor | BinaryOp::Sub)) && matches!(op.operands[0], Operand::Value(_)) && op.operands[0] == op.operands[1] {
        return Some(Known::new(0, width));
    }
    let supported = matches!(op.opcode, Opcode::Cast(CastOp::SExt | CastOp::ZExt)) || matches!(op.opcode, Opcode::Binary(kind) if ARITH.iter().any(|(one, _)| *one == kind));
    if !supported {
        return None;
    }
    let mut parts = Vec::new();
    for &one in &op.operands {
        parts.push(_operand(unit, one, known, here)?);
    }
    if let Opcode::Cast(extension) = op.opcode {
        let source_width = _width(unit, op.operands[0])?;
        if !(0 < source_width && source_width < width && width <= 64) || parts[0].width < source_width {
            return None;
        }
        let mut number = masked(&parts[0].n, source_width);
        if extension == CastOp::SExt {
            let sign = BigInt::from(1) << (source_width - 1);
            number = (number ^ &sign) - sign;
        }
        return Some(Known::new(masked(&number, width), width));
    }
    let Opcode::Binary(kind) = op.opcode else {
        return None;
    };
    if matches!(kind, BinaryOp::Shl | BinaryOp::LShr) {
        if parts[0].width < width {
            return None;
        }
        let count = u32::try_from(&parts[1].n & BigInt::from(width - 1)).expect("a masked count");
        let number = masked(&parts[0].n, width);
        let shifted = if kind == BinaryOp::Shl { number << count } else { number >> count };
        return Some(Known::new(masked(&shifted, width), width));
    }
    let width = parts.iter().map(|one| one.width).min().expect("parts").min(width);
    let (_, arith) = ARITH.iter().find(|(one, _)| *one == kind)?;
    Some(Known::new(masked(&arith(&parts[0].n, &parts[1].n), width), width))
}

/// Whether `predicate` holds of two known numbers: an `icmp`'s answer.
pub fn holds(predicate: IntPredicate, left: &Known, right: &Known) -> bool {
    let width = left.width.max(right.width);
    let signed = |fact: &Known| {
        let top = BigInt::from(1) << (fact.width - 1);
        let number = masked(&fact.n, fact.width);
        if (&number & &top) != BigInt::from(0) { number - (top << 1) } else { number }
    };
    let (a, b) = (signed(left), signed(right));
    let (ua, ub) = (masked(&left.n, width), masked(&right.n, width));
    match predicate {
        IntPredicate::Eq => a == b,
        IntPredicate::Ne => a != b,
        IntPredicate::Slt => a < b,
        IntPredicate::Sle => a <= b,
        IntPredicate::Sgt => a > b,
        IntPredicate::Sge => a >= b,
        IntPredicate::Ult => ua < ub,
        IntPredicate::Ule => ua <= ub,
        IntPredicate::Ugt => ua > ub,
        IntPredicate::Uge => ua >= ub,
    }
}

/// Every value this function computes that is a number, to a fixed point;
/// memory too, where `calls` says what each call writes.
///
/// Optimistic, then shrinking: a run may assume every selector it does not
/// know is some absolute segment; the ones that came out numbers keep the
/// assumption and the rest lose it, until every one still assumed resolved.
pub fn known(unit: &Unit, calls: Option<&Calls>, edges: Option<&IndexMap<(i64, i64), Cells>>, initial: Option<&Cells>) -> IndexMap<ValueId, Known> {
    // A store kills the cells alias's provenance leaves it able to reach.
    let annotated = (calls.is_some() && unit.references.is_none()).then(|| crate::alias::annotated(unit).ok()).flatten();
    let unit = &annotated.as_ref().map_or(*unit, |references| unit.with_references(references));
    // No edge facts is no edges: the solve reads only a nonempty map.
    let edges = edges.filter(|edges| !edges.is_empty());
    let mut allowed: Option<BTreeSet<ValueId>> = None;
    loop {
        let (got, assumed) = _solved(unit, calls, edges, initial, Some(BTreeSet::new()), allowed.as_ref());
        let resolved = assumed.iter().filter(|value| got.contains_key(*value)).copied().collect::<BTreeSet<_>>();
        if resolved == assumed {
            return got;
        }
        allowed = Some(resolved);
    }
}

#[cfg(test)]
thread_local! {
    /// Fixed points solved.
    pub static SOLVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// `may_overlap` questions, for the test that pins the cell index.
    pub static MAY_OVERLAP: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A phi's or an operand's fact.
fn incoming(unit: &Unit, one: Operand, facts: &IndexMap<ValueId, Known>) -> Option<Known> {
    _operand(unit, one, facts, None)
}

/// Dominating, exact stores supplying loads outside the cell lattice: a
/// load through a pointer whose one clobber, as MemorySSA walks it, is a
/// store of its bytes and its type.
fn _pointer_stores(unit: &Unit, calls: &Calls) -> IndexMap<ValueId, Operand> {
    let function = unit.function;
    let accesses = Accesses::plain(unit, calls);
    let candidates = function
        .walk()
        .filter_map(|(block, inst)| Some((block, inst, avail::loaded_into(unit, &accesses, inst)?)))
        .filter(|(_, _, (reference, result))| !(reference.object && reference.addr().is_some()) && _width(unit, Operand::Value(*result)).is_some())
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return IndexMap::default();
    }
    let graph = memoryssa::built(unit, &accesses);
    let dominance = llrm_graph::loops::dominance(&cfg::graph(function), function.entry().map(cfg::id));
    let before = |source: InstId, block: BlockId, inst: InstId| {
        let order = function.block(block).instructions();
        match function.parent(source) {
            Some(at) if at == block => order.iter().position(|&one| one == source) < order.iter().position(|&one| one == inst),
            Some(at) => dominance.dominates(cfg::id(at), cfg::id(block)),
            None => false,
        }
    };
    let mut providers = IndexMap::default();
    for (block, inst, (reference, result)) in candidates {
        let clobbers = graph.clobbers(inst, &reference);
        let single = if clobbers.len() == 1 { clobbers.first().map(|id| graph.access(*id)) } else { None };
        let Some(source) = single.filter(|access| access.kind == memoryssa::Kind::Def).and_then(|access| access.site) else {
            continue;
        };
        if let Some((stored, value)) = avail::stored_from(unit, &accesses, source)
            && before(source, block, inst)
            && memoryssa::same_bytes(unit, &reference, &stored)
            && unit.operand_type(value) == Some(function.value(result).ty)
        {
            providers.insert(result, value);
        }
    }
    providers
}

fn _solved(
    unit: &Unit,
    calls: Option<&Calls>,
    edges: Option<&IndexMap<(i64, i64), Cells>>,
    initial: Option<&Cells>,
    mut assume: Option<BTreeSet<ValueId>>,
    allowed: Option<&BTreeSet<ValueId>>,
) -> (IndexMap<ValueId, Known>, BTreeSet<ValueId>) {
    #[cfg(test)]
    SOLVED.with(|solved| solved.set(solved.get() + 1));
    let function = unit.function;
    let mut facts = IndexMap::<ValueId, Known>::default();
    let mut held = HeldCells::default();
    let pointer_stores = calls.map(|calls| _pointer_stores(unit, calls)).unwrap_or_default();
    let empty = Cells::default();
    // The cells read only what writes name; until a round learns one of
    // those, solving them again gives the same answer.
    let read = match calls {
        Some(_) => _memory_reads(unit),
        None => HashSet::default(),
    };
    // Registers go first, as SCCP learns them before memory: cells solved
    // with what registers alone prove need solving again only when a value
    // learned from memory is one a write names.
    let mut learned = false;
    let mut remembered = calls.is_none();
    let mut rounds = 0;
    let mut changing = true;
    while changing || !remembered {
        changing = false;
        rounds += 1;
        // What memory holds, recomputed from what is known so far: the two
        // feed each other and run to one fixed point together.
        if let Some(calls) = calls {
            if rounds > 1 && (learned || !remembered) {
                held = cells(unit, calls, Some(&facts), initial, edges, assume.as_mut(), allowed);
                remembered = true;
            }
            learned = false;
        }
        for (_, inst) in function.walk() {
            let op = function.instruction(inst);
            let Some(target) = _defined(unit, inst).filter(|target| !facts.contains_key(target)) else {
                continue;
            };
            // A join is known where every path into it agrees.
            if op.opcode == Opcode::Phi {
                let seen = op.operands.iter().step_by(2).map(|&one| incoming(unit, one, &facts)).collect::<Vec<_>>();
                let known = seen.iter().flatten().collect::<Vec<_>>();
                if seen.is_empty() || known.len() != seen.len() || known.iter().map(|one| (&one.n, one.width)).collect::<BTreeSet<_>>().len() != 1 {
                    continue;
                }
                let fact = (*known[0]).clone();
                learned |= read.contains(&target);
                facts.insert(target, fact);
                changing = true;
                continue;
            }
            let here = held.get(&inst).map(|here| &**here).unwrap_or(&empty);
            let found = _result(unit, inst, &facts, Some(here)).or_else(|| _operand(unit, *pointer_stores.get(&target)?, &facts, None));
            if let Some(found) = found {
                learned |= read.contains(&target);
                facts.insert(target, found);
                changing = true;
            }
        }
    }
    (constant_cycles::propagated(unit, &facts, None), assume.unwrap_or_default())
}

#[cfg(test)]
#[path = "consts_tests.rs"]
mod tests;
