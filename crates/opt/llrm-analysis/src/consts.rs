//! Which values are known, and what they are: integer constant propagation and
//! folding, with memory read through MemorySSA.
//!
//! # How it works
//! A fact is `Known { n, width }`: the low `width` bits of the value are `n`,
//! nothing is said above them. An operation folds only where the widths
//! agree (`_result`): integer `add`, `sub`, `mul`, `and`, `or`, `xor`, shifts,
//! divisions that do not fault (`folds`), `sext`, `zext`, `trunc`, `icmp`
//! (`holds`), `x ^ x` and `x - x`, the fixed-point intrinsics, and a pointer
//! compare of two addresses of one global. A width is in bits; a memory
//! cell's, in bytes.
//!
//! Registers. `known` runs a fixed point over the body in layout order: each
//! integer result not yet known is folded from the facts so far, and a phi
//! is known where every incoming value is known and they agree. It then
//! hands the facts to [`crate::constant_cycles`], which settles the phis whose
//! incoming values depend on each other around a loop.
//!
//! Memory. `known_walked` starts from the register facts and, for each load,
//! asks [`crate::memoryssa`] which stores reach it (`load_providers`). If
//! they all put the same number in the bytes the load reads, so does the
//! load (`provided`; a store may serve a load of another type, or several
//! stores serve one byte at a time). No state is held per block, so the cost
//! is the walks, not cells times joins. A load whose index only memory taught
//! is asked again with the index placed, up to four rounds. `known_walked_over`
//! adds what counted float loops leave in memory at their exits
//! ([`crate::floatfacts`]).
//!
//! ```text
//!    store i16 7, @g          known:        %a is not known
//!    %a = load i16, @g        known_walked: %a = 7, %b = 8
//!    %b = add i16 %a, 1       (the walk finds the store)
//! ```
//!
//! The caller states what each call writes (`Calls`, from
//! `alias::calls_by_attributes`); a call not listed writes what
//! `memory::unmodeled_write` says. A far store through a segment selector
//! nothing yet resolves is taken to write no program object, and the solve is
//! made again without any such selector that did not resolve to a number.
//!
//! `memory_queries` and `_MemoryQueries` answer alias questions about the cells
//! of one solved epoch, each cell carrying its object, for passes that walk
//! instructions with a fact table ([`crate::cellmap`] indexes the cells by
//! overlap bucket).
//!
//! # Example
//! A load of a global that a constant-index store wrote:
//!
//! ```mir
//! @g = global [128 x i8] zeroinitializer
//!
//! define i16 @f() {
//! b0:
//!   %index = add i16 2, 2
//!   %p = getelementptr i8, ptr @g, i16 %index
//!   store i16 9, ptr %p
//!   %loaded = load i16, ptr getelementptr (i8, ptr @g, i16 4)
//!   ret i16 %loaded
//! }
//! ```
//!
//! | Value | `known` | `known_walked` |
//! | --- | --- | --- |
//! | `%loaded` | none | `Known { n: 9, width: 16 }` |
//!
//! From unit test `a_store_through_a_constant_index_is_read_back` in
//! `consts_tests.rs`.
//!
//! # Provides
//! `Known`, `known` (registers, cached as [`crate::manager::Registers`]),
//! `known_walked` and `known_walked_over` (memory; cached as
//! [`crate::manager::ThroughMemory`]), `memory_queries`, `holds`, `folds`.
//! Consumers: `gvn`, `algebraic`, `fold`, `decide`, `unswitch`, `floatloop` and
//! `interprocedural` in `llrm-transforms`, the backend's `isel`, and most
//! analyses of this crate.
//!
//! # Needs
//! [`crate::memoryssa`], [`crate::alias`] (the call table), [`crate::ranges`],
//! [`crate::regions`], [`crate::cellmap`], [`crate::constant_cycles`].
//!
//! # See also
//! LLVM `SCCP`/`ConstantFolding`; GCC FRE walks the same virtual use-def chain
//! for a load's value (`vn_reference_lookup`). `LLRM_CHECK_CYCLES` in
//! [`crate::constant_cycles`].

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use llrm_mir::context::{ConstantExpr, ConstantKind};
use llrm_mir::dense::IdMap;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_mir::types::{FloatKind, Type};
use llrm_support::hash::{HashMap, IndexMap};

use crate::avail;
use crate::cellmap::CellMap;
use crate::cfg;
use crate::constant_cycles;
use crate::memory::{Addr, MemRef, Provenance, Unit, object_of, unmodeled_write};
use crate::memoryssa::{self, Accesses};
use crate::num::Num as BigInt;
use crate::ranges::{self, Interval};
use crate::regions::{
    ByteRange, OverlapBucket, OverlapBuckets, displaced_buckets, object_bucket, overlap_buckets, overlap_span,
    overlapping,
};

type Binary = fn(&BigInt, &BigInt) -> BigInt;

/// What each operation does to two known numbers, within one width.
pub static ARITH: [(BinaryOp, Binary); 8] = [
    (BinaryOp::Add, |a, b| a + b),
    (BinaryOp::Sub, |a, b| a - b),
    (BinaryOp::And, |a, b| a & b),
    (BinaryOp::Or, |a, b| a | b),
    (BinaryOp::Xor, |a, b| a ^ b),
    (BinaryOp::Shl, |a, b| a << u32::try_from(b & BigInt::from(31)).expect("five bits")),
    (BinaryOp::LShr, |a, b| {
        (a & BigInt::from(0xFFFF_FFFF_u32)) >> u32::try_from(b & BigInt::from(31)).expect("five bits")
    }),
    (BinaryOp::Mul, |a, b| a * b),
];

/// Memory facts are stored as bytes so partial writes and control-flow
/// joins do not discard an untouched neighbor. A cell is its address and
/// its width in bytes.
pub type Cells = IndexMap<(Addr, u32), Known>;

/// What each call writes, where `alias::calls_by_attributes` found it. A call
/// not here writes what `memory::unmodeled_write` says: everything, or
/// nothing.
pub type Calls = IndexMap<InstId, std::rc::Rc<[MemRef]>>;

/// A reference as resolved for one epoch's queries, with the number it was
/// given: a stable identity to key answers by, where its address varied from
/// run to run and so did the work of the passes asking (`mir hoist`, up to
/// 0.9%).
pub struct Resolved {
    pub id: u32,
    reference: MemRef,
}

impl std::ops::Deref for Resolved {
    type Target = MemRef;
    fn deref(&self) -> &MemRef {
        &self.reference
    }
}

thread_local! {
    /// Queries built, for the tests that count them.
    pub static QUERY_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Alias questions for one immutable known-value epoch.
pub struct _MemoryQueries<'a> {
    pub unit: Unit<'a>,
    pub known: IndexMap<ValueId, Known>,
    pub facts: BTreeMap<ValueId, Interval>,
    /// Each reference asked, resolved, numbered in the order asked: the number
    /// is its identity in `overlaps`.
    pub addressed: HashMap<MemRef, Rc<Resolved>>,
    /// Each call's shared list of references, resolved once: calls with the
    /// same effect hold one list, and hashing every reference of it again
    /// at every call was 12% of through-memory. Keyed by the list's address,
    /// which its clone here keeps.
    listed: HashMap<usize, (Rc<[MemRef]>, Rc<[Rc<Resolved>]>)>,
    pub overlaps: HashMap<((Addr, u32), u32), bool>,
    pub places: HashMap<(Addr, u32), (OverlapBucket, Option<ByteRange>)>,
    /// The buckets `places` names, this epoch's own.
    pub buckets: OverlapBuckets,
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
    pub fn new(
        unit: Unit<'a>,
        known: &IndexMap<ValueId, Known>,
    ) -> Self {
        QUERY_BUILDS.with(|built| built.set(built.get() + 1));
        Self {
            unit,
            known: known.clone(),
            facts: _intervals(known),
            addressed: HashMap::default(),
            listed: HashMap::default(),
            overlaps: HashMap::default(),
            places: HashMap::default(),
            buckets: OverlapBuckets::default(),
        }
    }

    /// These values as `known` now has them, the epoch moved on: what was
    /// asked is forgotten, and what the queries hold is the `changed` values'
    /// alone to redo (a store of a simulated unroll asks again after every
    /// instruction, and copying all `known` each time was its square).
    pub fn advance(
        &mut self,
        known: &IndexMap<ValueId, Known>,
        changed: impl IntoIterator<Item = ValueId>,
    ) {
        for value in changed {
            match known.get(&value) {
                Some(fact) => {
                    self.known.insert(value, fact.clone());
                    self.facts.insert(value, Interval { low: fact.n.clone(), high: fact.n.clone(), width: fact.width });
                }
                None => {
                    self.known.shift_remove(&value);
                    self.facts.remove(&value);
                }
            }
        }
        self.addressed.clear();
        self.listed.clear();
        self.overlaps.clear();
        self.places.clear();
        self.buckets = OverlapBuckets::default();
    }

    /// `resolve` of every reference of a list `calls` shares among its calls.
    pub fn resolve_list(
        &mut self,
        list: &Rc<[MemRef]>,
    ) -> Rc<[Rc<Resolved>]> {
        let key = Rc::as_ptr(list).cast::<MemRef>() as usize;
        if let Some((_, resolved)) = self.listed.get(&key) {
            return Rc::clone(resolved);
        }
        let resolved: Rc<[Rc<Resolved>]> = list.iter().map(|one| self.resolve(one)).collect();
        self.listed.insert(key, (Rc::clone(list), Rc::clone(&resolved)));
        resolved
    }

    pub fn resolve(
        &mut self,
        reference: &MemRef,
    ) -> Rc<Resolved> {
        #[cfg(test)]
        RESOLVED.with(|asked| asked.set(asked.get() + 1));
        if let Some(saved) = self.addressed.get(reference) {
            return Rc::clone(saved);
        }
        let made = Rc::new(Resolved {
            id: self.addressed.len() as u32,
            reference: _addressed(&self.unit, reference, &self.known),
        });
        self.addressed.insert(reference.clone(), Rc::clone(&made));
        made
    }

    /// The access cell `where_` is, carrying its object.
    fn cell(
        &self,
        where_: (Addr, u32),
    ) -> MemRef {
        let (addr, width) = where_;
        let mut cell = MemRef { disp: addr.disp, ..MemRef::at(&self.unit, addr.root, width) };
        cell.provenance = object_of(&self.unit, addr.root).map(|object| {
            Provenance::one_with_slice(object, addr.disp, addr.disp + i64::from(width), 1, 1, BTreeSet::new())
                .expect("a cell is at least one byte")
        });
        cell
    }

    /// Python `bucket` and `span`: cell `where_`'s `overlap_bucket`, from
    /// its root's object, and its `overlap_span`.
    ///
    /// Remembered: interning a bucket hashes its object.
    pub fn place(
        &mut self,
        where_: (Addr, u32),
    ) -> (OverlapBucket, Option<ByteRange>) {
        if let Some(place) = self.places.get(&where_) {
            return place.clone();
        }
        let bucket =
            object_bucket(&mut self.buckets, object_of(&self.unit, where_.0.root), Some((where_.0.root, None)));
        let place = (bucket, overlap_span(&self.cell(where_)));
        self.places.insert(where_, place.clone());
        place
    }

    /// `here` indexed by this epoch's buckets, for one operation to change.
    pub fn owned(
        &mut self,
        here: Here,
    ) -> IndexedCells {
        match here {
            Here::Indexed(cells) => cells,
            Here::Plain(cells) => CellMap::new(cells, |where_| self.place(*where_)),
        }
    }

    pub fn may_overlap(
        &mut self,
        where_: (Addr, u32),
        reference: &Resolved,
    ) -> bool {
        #[cfg(test)]
        MAY_OVERLAP.with(|asked| asked.set(asked.get() + 1));
        let key = (where_, reference.id);
        if let Some(&answer) = self.overlaps.get(&key) {
            return answer;
        }
        let answer = self._overlap(where_, reference);
        self.overlaps.insert(key, answer);
        answer
    }

    fn _overlap(
        &mut self,
        where_: (Addr, u32),
        reference: &Resolved,
    ) -> bool {
        // An answer Rust cannot represent is taken to overlap.
        overlapping(&self.cell(where_), reference, Some(&self.facts), Some(&self.facts), self.unit.program)
            .unwrap_or(true)
    }
}

/// The low `width` bits of a value are `n`. Nothing is said above them.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct Known {
    pub n: BigInt,
    pub width: u32,
}

impl Known {
    pub fn new(
        n: impl Into<BigInt>,
        width: u32,
    ) -> Self {
        Self { n: n.into(), width }
    }
}

impl fmt::Debug for Known {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(formatter, "{:#x}:{}", self.n, self.width)
    }
}

pub fn masked(
    n: &BigInt,
    width: u32,
) -> BigInt {
    n & ((BigInt::from(1) << width) - 1)
}

/// An integer operand's width in bits.
fn _width(
    unit: &Unit,
    operand: Operand,
) -> Option<u32> {
    unit.int_bits(operand)
}

/// Integer quotient and remainder of the `sdiv`, `udiv`, `srem` or `urem`
/// `inst`, excluding the faulting cases.
pub fn division(
    unit: &Unit,
    inst: InstId,
    known: &IndexMap<ValueId, Known>,
) -> Option<(BigInt, BigInt)> {
    let op = unit.function.instruction(inst);
    let signed = match op.opcode {
        Opcode::Binary(BinaryOp::SDiv | BinaryOp::SRem) => true,
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem) => false,
        _ => return None,
    };
    let width = _width(unit, Operand::Value(op.result?))?;
    let operands = op.operands.iter().map(|&one| _operand(unit, one, known, None)).collect::<Option<Vec<_>>>()?;
    _divided(&operands, signed, width)
}

/// `parts[0]` divided by `parts[1]` at `width`: quotient and remainder,
/// none where the division faults.
fn _divided(
    parts: &[Known],
    signed: bool,
    width: u32,
) -> Option<(BigInt, BigInt)> {
    if parts.iter().any(|fact| fact.width < width) {
        return None;
    }
    let (dividend, divisor) = if signed {
        (_signed(&parts[0].n, width), _signed(&parts[1].n, width))
    } else {
        (masked(&parts[0].n, width), masked(&parts[1].n, width))
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

/// `n` at `width` read as two's complement.
fn _signed(
    n: &BigInt,
    width: u32,
) -> BigInt {
    let sign = BigInt::from(1) << (width - 1);
    (masked(n, width) ^ &sign) - sign
}

/// Whether consts computes `kind` of two known numbers: every integer
/// operation, a division where it does not fault.
pub fn folds(kind: BinaryOp) -> bool {
    !matches!(
        kind,
        BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem
    )
}

/// What this store puts in its cell, where that is a number: a float's
/// bits too, which floatfacts supplies for a computed value.
pub fn _put(
    unit: &Unit,
    inst: InstId,
    known: &IndexMap<ValueId, Known>,
) -> Option<Known> {
    let op = unit.function.instruction(inst);
    if !matches!(op.opcode, Opcode::Store { .. }) {
        return None;
    }
    _operand(unit, op.operands[0], known, None).or_else(|| _float_bits(unit, op.operands[0], known))
}

/// A float operand's bits: a constant's, or what `known` says of a value.
fn _float_bits(
    unit: &Unit,
    operand: Operand,
    known: &IndexMap<ValueId, Known>,
) -> Option<Known> {
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
pub fn initialized(
    unit: &Unit,
    inst: InstId,
    reference: &MemRef,
) -> Option<Known> {
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
pub fn _fragments(
    reference: &MemRef,
    fact: &Known,
) -> Cells {
    let addr = reference.addr().expect("a fragment is of an addressed cell");
    (0..reference.width.min(fact.width / 8))
        .map(|offset| ((addr.plus(i64::from(offset)), 1), Known::new((&fact.n >> (offset * 8)) & BigInt::from(255), 8)))
        .collect()
}

/// A far store's selector, where nothing yet says which segment it is
/// and it is still one this run may take on faith.
fn _selector(
    unit: &Unit,
    reference: &MemRef,
    known: &IndexMap<ValueId, Known>,
    allowed: Option<&BTreeSet<ValueId>>,
) -> Option<ValueId> {
    let Some(Operand::Value(segment)) = reference.segment else {
        return None;
    };
    if reference.space != unit.spaces().far
        || known.contains_key(&segment)
        || allowed.is_some_and(|allowed| !allowed.contains(&segment))
    {
        return None;
    }
    Some(segment)
}

/// Every value an instruction that writes memory reads: `_kills` asks
/// `known` of no other.
fn _memory_reads(unit: &Unit) -> llrm_mir::dense::IdSet<ValueId> {
    let function = unit.function;
    let mut read = llrm_mir::dense::IdSet::new();
    for (_, inst) in function.walk() {
        let op = function.instruction(inst);
        if !matches!(
            op.opcode,
            Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_)
        ) {
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
    known
        .iter()
        .map(|(value, fact)| (*value, Interval { low: fact.n.clone(), high: fact.n.clone(), width: fact.width }))
        .collect()
}

/// Alias questions about `unit`'s cells, each cell carrying its object.
pub fn memory_queries<'a>(
    unit: Unit<'a>,
    known: &IndexMap<ValueId, Known>,
) -> _MemoryQueries<'a> {
    _MemoryQueries::new(unit, known)
}

/// Whether `inst` is a call or invoke.
fn is_call(
    unit: &Unit,
    inst: InstId,
) -> bool {
    matches!(
        unit.function.instruction(inst).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    )
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
    let stores: Rc<[Rc<Resolved>]> = match calls.get(&inst) {
        Some(stores) => queries.resolve_list(stores),
        None if call => Rc::from([]),
        None => unit
            .reference(inst)
            .filter(|_| matches!(unit.function.instruction(inst).opcode, Opcode::Store { .. }))
            .iter()
            .map(|one| queries.resolve(one))
            .collect(),
    };
    // Only a write changes a cell.
    if stores.is_empty() {
        return here;
    }
    let put = _put(&unit, inst, known);
    for reference in stores.iter() {
        let reference = Rc::clone(reference);
        if let Some(assume) = assume.as_deref_mut() {
            if let Some(selector) = _selector(&unit, &reference, known, allowed) {
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

fn _read(
    fact: Option<&Known>,
    width: u32,
) -> Option<Known> {
    let fact = fact?;
    if fact.width < width {
        return None;
    }
    Some(Known::new(masked(&fact.n, width), width))
}

/// What `here` says the bytes `reference` reads hold, where every one is known.
pub fn _cell(
    here: &Cells,
    reference: &MemRef,
) -> Option<Known> {
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
fn _addressed(
    unit: &Unit,
    reference: &MemRef,
    known: &IndexMap<ValueId, Known>,
) -> MemRef {
    let Some(base) = reference.base else {
        return reference.clone();
    };
    let interval = ranges::_operand(unit, Operand::Value(base), &ranges::Intervals::default(), known);
    let Some(interval) = interval else {
        return reference.clone();
    };
    ranges::covering(reference, &BTreeMap::from([(base, interval)])).into_owned()
}

/// One operand as a number, if it is one.
pub fn _operand(
    unit: &Unit,
    one: Operand,
    known: &IndexMap<ValueId, Known>,
    _here: Option<&Cells>,
) -> Option<Known> {
    let width = _width(unit, one)?;
    if let Some(bits) = unit.int_constant(one) {
        return Some(Known::new(masked(&BigInt::from(bits), width), width));
    }
    match one {
        Operand::Value(value) => _read(known.get(&value), width),
        _ => None,
    }
}

/// A constant pointer as the global it names, if any, and its offset: null,
/// an integer made a pointer, a global. Two with one global compare by offset.
fn _address(
    unit: &Unit,
    operand: Operand,
) -> Option<(Option<llrm_mir::context::GlobalId>, u128)> {
    let Operand::Constant(id) = operand else { return None };
    let context = unit.context;
    match &context.get(id).kind {
        ConstantKind::Null | ConstantKind::Zero => Some((None, 0)),
        ConstantKind::Global(global) => Some((Some(*global), 0)),
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) => match context.get(*value).kind {
            ConstantKind::Int(bits) => Some((None, bits)),
            _ => None,
        },
        _ => None,
    }
}

/// The integer value this instruction defines.
pub fn _defined(
    unit: &Unit,
    inst: InstId,
) -> Option<ValueId> {
    let result = unit.function.instruction(inst).result?;
    _width(unit, Operand::Value(result)).map(|_| result)
}

/// What this instruction computes, where every input is known.
pub fn _result(
    unit: &Unit,
    inst: InstId,
    known: &IndexMap<ValueId, Known>,
    here: Option<&Cells>,
) -> Option<Known> {
    let result = _defined(unit, inst)?;
    let width = _width(unit, Operand::Value(result))?;
    let op = unit.function.instruction(inst);
    if matches!(op.opcode, Opcode::Load { volatile: false, .. }) {
        let reference = _addressed(unit, &MemRef::of(unit, inst)?, known);
        if let Some(bits) = crate::memory::constant_bits(unit, &reference) {
            return Some(Known::new(bits, width));
        }
        return _read(_cell(here?, &reference).as_ref(), width);
    }
    if op.opcode == Opcode::Cast(CastOp::Trunc) {
        return _read(_operand(unit, op.operands[0], known, here).as_ref(), width);
    }
    if let Opcode::ICmp(predicate @ (IntPredicate::Eq | IntPredicate::Ne)) = op.opcode
        && let (Some(left), Some(right)) = (_address(unit, op.operands[0]), _address(unit, op.operands[1]))
        && left.0 == right.0
    {
        return Some(Known::new(u8::from((left.1 == right.1) == (predicate == IntPredicate::Eq)), 1));
    }
    if let Opcode::ICmp(predicate) = op.opcode {
        let (left, right) =
            (_operand(unit, op.operands[0], known, here)?, _operand(unit, op.operands[1], known, here)?);
        return Some(Known::new(u8::from(holds(predicate, &left, &right)), 1));
    }
    if matches!(op.opcode, Opcode::Binary(BinaryOp::Xor | BinaryOp::Sub))
        && matches!(op.operands[0], Operand::Value(_))
        && op.operands[0] == op.operands[1]
    {
        return Some(Known::new(0, width));
    }
    if let Some(Intrinsic::Fixed { divide }) = unit.intrinsic(inst) {
        let signed = |at: usize| {
            _operand(unit, op.operands[at], known, here)
                .filter(|one| one.width >= width)
                .and_then(|one| i128::try_from(_signed(&one.n, width)).ok())
        };
        let scale = u32::try_from(&_operand(unit, op.operands[2], known, here)?.n).ok()?;
        let value = Intrinsic::fixed(divide, width, signed(0)?, signed(1)?, scale)?;
        return Some(Known::new(masked(&BigInt::from(value), width), width));
    }
    let supported = matches!(op.opcode, Opcode::Cast(CastOp::SExt | CastOp::ZExt))
        || matches!(op.opcode, Opcode::Binary(kind) if folds(kind));
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
    if matches!(kind, BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr) {
        if parts[0].width < width {
            return None;
        }
        let count = u32::try_from(&parts[1].n & BigInt::from(width - 1)).expect("a masked count");
        let shifted = match kind {
            BinaryOp::Shl => masked(&parts[0].n, width) << count,
            BinaryOp::LShr => masked(&parts[0].n, width) >> count,
            _ => _signed(&parts[0].n, width) >> count,
        };
        return Some(Known::new(masked(&shifted, width), width));
    }
    if let BinaryOp::SDiv | BinaryOp::SRem | BinaryOp::UDiv | BinaryOp::URem = kind {
        let (quotient, remainder) = _divided(&parts, matches!(kind, BinaryOp::SDiv | BinaryOp::SRem), width)?;
        return Some(Known::new(
            if matches!(kind, BinaryOp::SDiv | BinaryOp::UDiv) { quotient } else { remainder },
            width,
        ));
    }
    let width = parts.iter().map(|one| one.width).min().expect("parts").min(width);
    let (_, arith) = ARITH.iter().find(|(one, _)| *one == kind)?;
    Some(Known::new(masked(&arith(&parts[0].n, &parts[1].n), width), width))
}

/// Whether `predicate` holds of two known numbers: an `icmp`'s answer.
pub fn holds(
    predicate: IntPredicate,
    left: &Known,
    right: &Known,
) -> bool {
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

/// The loads that read a `constant` global at an index: the fact of each is the
/// bytes the index can only be one value to read, which `ranges` finds from the
/// index's whole expression (the facts and the bounds of every value it is made
/// of, those that are known numbers and those that are not), so no change to a
/// value it is made from leaves one as it was.
pub fn table_loads(unit: &Unit) -> Vec<InstId> {
    TABLE_WALKS.with(|count| count.set(count.get() + 1));
    unit.function.walk().map(|(_, inst)| inst).filter(|&inst| is_table_load(unit, inst)).collect()
}

/// Whether `inst` is a load from a constant global by an index.
pub fn is_table_load(
    unit: &Unit,
    inst: InstId,
) -> bool {
    let function = unit.function;
    matches!(
        function.instruction(inst).opcode,
        Opcode::Load { volatile: false, .. }
    )
        && MemRef::of(unit, inst).is_some_and(|reference| {
            reference.base.is_some()
                && matches!(
                    reference.root,
                    Some(Operand::Constant(root)) if matches!(
                        unit.context.get(root).kind,
                        ConstantKind::Global(global) if matches!(
                            unit.globals.get(global.0 as usize).map(|one| &one.kind),
                            Some(llrm_mir::module::GlobalKind::Variable(variable)) if variable.constant
                        )
                    )
                )
        })
}

/// Every value this function computes that is a number, to a fixed point,
/// without memory (`known_walked` has it).
pub fn known(unit: &Unit) -> IndexMap<ValueId, Known> {
    SCRATCH_DERIVATIONS.with(|count| count.set(count.get() + 1));
    known_from(unit, IndexMap::default(), None, &[])
}

/// `known`, from the facts `kept` of the values `dirty` leaves out (all of them
/// where it is none): what a value is known to be is a function of the values
/// it is made from alone, so a value no change reached has the fact it had, and
/// the others are worked out again.
pub fn known_from(
    unit: &Unit,
    kept: IndexMap<ValueId, Known>,
    dirty: Option<&llrm_mir::dense::IdSet<ValueId>>,
    tables: &[InstId],
) -> IndexMap<ValueId, Known> {
    REGISTER_DERIVATIONS.with(|count| count.set(count.get() + 1));
    // Each access asks whether its frame object is exposed: found once for the
    // body, if no caller has.
    let exposed = unit.exposed.is_none().then(|| crate::memory::exposed_frames(unit));
    let unit = &exposed.as_ref().map_or(*unit, |table| unit.with_exposed(table));
    #[cfg(test)]
    SOLVED.with(|solved| solved.set(solved.get() + 1));
    let function = unit.function;
    let mut facts = kept;
    let empty = Cells::default();
    // A value's fact from the facts of the values it is made from: a join is
    // known where every path into it agrees.
    let settled = |inst: InstId, facts: &IndexMap<ValueId, Known>| -> Option<Known> {
        let op = function.instruction(inst);
        if op.opcode == Opcode::Phi {
            let seen = op.operands.iter().step_by(2).map(|&one| incoming(unit, one, facts)).collect::<Vec<_>>();
            let known = seen.iter().flatten().collect::<Vec<_>>();
            if seen.is_empty()
                || known.len() != seen.len()
                || known.iter().map(|one| (&one.n, one.width)).collect::<BTreeSet<_>>().len() != 1
            {
                return None;
            }
            return Some((*known[0]).clone());
        }
        _result(unit, inst, facts, Some(&empty))
    };
    if let Some(dirty) = dirty {
        // From the facts that stand, each value to settle in turn, and the
        // users of each that came to a fact: the facts only grow, so it
        // ends where the fixed point does.
        let defining = |value: ValueId| match function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(inst) => function.parent(inst).map(|_| inst),
            llrm_mir::module::ValueDef::Argument(_) => None,
        };
        let mut region = dirty.clone();
        let mut queue: Vec<InstId> = dirty.iter().filter_map(defining).collect();
        loop {
            while let Some(inst) = queue.pop() {
                let Some(target) = _defined(unit, inst).filter(|target| !facts.contains_key(target)) else { continue };
                let Some(found) = settled(inst, &facts) else { continue };
                facts.insert(target, found);
                for user in function.users(target) {
                    if function.parent(user.user).is_some() {
                        region.extend(function.instruction(user.user).result);
                        queue.push(user.user);
                    }
                }
            }
            // A table load is known by the bounds of its whole index, so a fact
            // that came to a value the index is made of (and not to
            // the load's own operand) asks it again.
            queue.extend(tables.iter().copied().filter(|&inst| {
                _defined(unit, inst)
                    .is_some_and(|target| !facts.contains_key(&target) && settled(inst, &facts).is_some())
            }));
            if queue.is_empty() {
                break;
            }
        }
        DERIVED_WORK.with(|count| count.set(count.get() + region.iter().count()));
        return constant_cycles::propagated(unit, facts, None, Some(&region));
    }
    let walked: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    DERIVED_WORK.with(|count| count.set(count.get() + walked.len()));
    let mut changing = true;
    while changing {
        changing = false;
        for &inst in &walked {
            let Some(target) = _defined(unit, inst).filter(|target| !facts.contains_key(target)) else { continue };
            if let Some(found) = settled(inst, &facts) {
                facts.insert(target, found);
                changing = true;
            }
        }
    }
    constant_cycles::propagated(unit, facts, None, None)
}

thread_local! {
    static SCRATCH_DERIVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static REGISTER_DERIVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DERIVED_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static MEMORY_DERIVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static TABLE_WALKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has walked a whole body for its table loads, for
/// a test that a change asks only of the loads it reaches.
pub fn table_walks() -> usize {
    TABLE_WALKS.with(std::cell::Cell::get)
}

/// How many times this thread has derived what is known of a body through
/// memory, for a test that two passes that ask of one body share the answer.
pub fn memory_derivations() -> usize {
    MEMORY_DERIVATIONS.with(std::cell::Cell::get)
}

/// The values `known_from` had to settle on this thread: a whole body for a
/// derivation from nothing, the values an edit reached for one from the facts
/// that stand. For a test that an edit costs what it reached.
pub fn derived_work() -> usize {
    DERIVED_WORK.with(std::cell::Cell::get)
}

/// How many times this thread has derived what is known of a body without
/// memory from nothing (`known`), not brought up to date from what was held.
pub fn scratch_derivations() -> usize {
    SCRATCH_DERIVATIONS.with(std::cell::Cell::get)
}

/// How many times this thread has derived what is known of a body without
/// memory, for a test that a pass asks of the manager, or of itself once for
/// each state of the body, and not once for each loop.
pub fn register_derivations() -> usize {
    REGISTER_DERIVATIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! {
    /// Fixed points solved.
    pub static SOLVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// `may_overlap` questions, for the test that pins the cell index.
    pub static MAY_OVERLAP: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// References resolved, hashed or not.
    pub static RESOLVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A phi's or an operand's fact.
fn incoming(
    unit: &Unit,
    one: Operand,
    facts: &IndexMap<ValueId, Known>,
) -> Option<Known> {
    _operand(unit, one, facts, None)
}

/// Dominating, exact stores supplying loads outside the cell lattice: a
/// load through a pointer whose one clobber, as MemorySSA's walker answers it
/// ([`crate::mssa`], LLVM's `getClobberingMemoryAccess`), is a
/// store of its bytes and its type.
fn _pointer_stores(
    unit: &Unit,
    calls: &Calls,
) -> llrm_mir::dense::IdMap<ValueId, Operand> {
    let function = unit.function;
    let accesses = Accesses::shared_or(unit, || Accesses::plain(unit, calls));
    let candidates = function
        .walk()
        .filter_map(|(block, inst)| Some((block, inst, avail::loaded_into(unit, &accesses, inst)?)))
        .filter(|(_, _, (reference, result))| {
            !(reference.object && reference.addr().is_some()) && _width(unit, Operand::Value(*result)).is_some()
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Default::default();
    }
    let shape = unit.shape();
    let dominance = &shape.dominance;
    let graph = {
        let mssa = crate::mssa::Mssa::build(function, &dominance, &crate::mssa::shadow::effect_of(&accesses, function));
        crate::mssa::view::Translated::new(unit, &accesses, mssa)
    };
    let before = |source: InstId, block: BlockId, inst: InstId| match function.parent(source) {
        Some(at) if at == block => function.comes_before(source, inst),
        Some(at) => dominance.dominates(unit.function, cfg::id(at), cfg::id(block)),
        None => false,
    };
    let mut providers = llrm_mir::dense::IdMap::default();
    for (block, inst, (reference, result)) in candidates {
        let Some(source) = graph.clobbering_access(inst).and_then(|access| graph.def_instruction(access)) else {
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

/// The cell a store writes and what it puts there, a volatile store's too: the
/// cell holds the number a volatile store wrote until something else writes it.
fn _stored_cell(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<(MemRef, Operand)> {
    let instruction = unit.function.instruction(inst);
    matches!(instruction.opcode, Opcode::Store { .. })
        .then(|| Some((accesses.references.get(&inst)?.clone(), instruction.operands[0])))
        .flatten()
}

/// What serves a load: stores of all its bytes, or stores of each byte.
pub enum Provider {
    Whole(Vec<InstId>),
    Bytes(Vec<Vec<(InstId, u32)>>),
}

/// The loads that read what a counted float loop left at its exit: those the
/// exit's block dominates, whose stores are the ones the end of the loop's
/// header reaches, and what `exits` says the loop left in their bytes.
fn exit_loads(
    unit: &Unit,
    accesses: &Accesses,
    registers: &IndexMap<ValueId, Known>,
    exits: &[crate::floatfacts::LoopExit],
) -> IdMap<ValueId, Known> {
    let mut found = IdMap::default();
    if exits.is_empty() {
        return found;
    }
    let function = unit.function;
    let graph = memoryssa::built(unit, accesses).with_known(_intervals(registers));
    let blocks = cfg::graph(function);
    let successors = blocks.iter().map(|block| (block.at, &block.succ)).collect::<IndexMap<_, _>>();
    let mut queries = memory_queries(*unit, registers);
    let shape = unit.shape();
    for exit in exits {
        let Some(body) = shape.loops.iter().find(|one| one.header == exit.header).map(|one| &one.body) else {
            continue;
        };
        let leaving = successors[&exit.header].iter().copied().filter(|at| !body.contains(at)).collect::<Vec<_>>();
        let [after] = leaving[..] else { continue };
        if function.predecessors(cfg::block(after))[..] != [cfg::block(exit.header)] {
            continue;
        }
        let mut cells = Cells::default();
        for (reference, fact) in &exit.stores {
            cells.extend(_fragments(&queries.resolve(reference), fact));
        }
        for (_, inst) in function.walk() {
            let Some((reference, result)) = avail::loaded_into(unit, accesses, inst) else { continue };
            let block = cfg::id(function.parent(inst).expect("a placed load"));
            if !shape.dominance.dominates(unit.function, after, block) {
                continue;
            }
            let reference = _addressed(unit, &reference, registers);
            let Some(fact) = _cell(&cells, &queries.resolve(&reference)) else { continue };
            if graph.clobbers_ignoring_invariance(inst, &reference) == graph.clobbers_at_end(exit.header, &reference) {
                found.insert(result, fact);
            }
        }
    }
    found
}

thread_local! {
    static STORES_DECODED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many stores' cells this thread has decoded, for a test that a store is
/// decoded once however many loads it reaches.
pub fn stores_decoded() -> usize {
    STORES_DECODED.with(std::cell::Cell::get)
}

/// The cell each store writes, and that cell addressed by the indices `placed`
/// proves constant, found once for the store and not again for each load it
/// reaches: a scalar written in each arm of N joins is reached by up to N
/// stores at every load after, N^2 over the body.
#[derive(Default)]
struct Stores {
    found: RefCell<IdMap<InstId, Option<Rc<(MemRef, MemRef)>>>>,
}

impl Stores {
    fn of(
        &self,
        unit: &Unit,
        accesses: &Accesses,
        placed: &IndexMap<ValueId, Known>,
        site: InstId,
    ) -> Option<Rc<(MemRef, MemRef)>> {
        if let Some(held) = self.found.borrow().get(&site) {
            return held.clone();
        }
        let made = _stored_cell(unit, accesses, site).map(|(cell, _)| {
            STORES_DECODED.with(|count| count.set(count.get() + 1));
            let addressed = _addressed(unit, &cell, placed);
            Rc::new((cell, addressed))
        });
        self.found.borrow_mut().insert(site, made.clone());
        made
    }
}

/// What serves a read of `reference` (its index placed by `placed`): the stores
/// `ask` says reach it, whole or byte by byte. None where a path meets no
/// write.
fn provider_of(
    unit: &Unit,
    accesses: &Accesses,
    graph: &memoryssa::MemorySSA,
    reference: &MemRef,
    ask: &dyn Fn(&MemRef) -> BTreeSet<usize>,
    stores: &Stores,
) -> Option<Provider> {
    let placed = unit.registers.expect("a provider is found over the registers that place the indices");
    let mut values = Vec::new();
    let mut exact = true;
    let reaching = ask(reference);
    // Past this many nearest writes a load is not served: a scalar written in
    // each arm of N joins is reached by N of them at the last, N^2 over the
    // body (`joins` 256: 10% of the compile). LLVM's walker stops at
    // `memssa-check-limit` (100) checks, gcc's value numbering at
    // `sccvn-max-alias-queries-per-access` (1000).
    if reaching.len() > memoryssa::MAX_NEAREST_WRITES {
        return None;
    }
    // A path to the entry that no write meets leaves every byte as it
    // was found, so no byte is a store's.
    if reaching.iter().any(|id| graph.access(*id).kind == memoryssa::Kind::Live) {
        return None;
    }
    for id in reaching {
        let access = graph.access(id);
        let stored = access
            .site
            .filter(|_| access.kind == memoryssa::Kind::Def)
            .and_then(|site| Some((site, stores.of(unit, accesses, placed, site)?)));
        match stored {
            Some((site, held)) if memoryssa::same_bytes(unit, reference, &held.1) => values.push(site),
            _ => {
                exact = false;
                break;
            }
        }
    }
    if exact && !values.is_empty() {
        Some(Provider::Whole(values))
    } else if let Some(address) = reference.addr().filter(|_| reference.width <= 8) {
        // Some of the bytes are another store's, or a wider one's: each
        // byte is asked of the walk, and read from the
        // stores that reach it.
        let mut bytes: Vec<Vec<(InstId, u32)>> = Vec::new();
        'bytes: for k in 0..reference.width {
            // The byte, with the provenance moved to it.
            let one = MemRef {
                disp: reference.disp + i64::from(k),
                width: 1,
                provenance: reference.provenance.as_ref().map(|found| found.shifted(i64::from(k))),
                ..reference.clone()
            };
            let mut from = Vec::new();
            let nearest = ask(&one);
            if nearest.len() > memoryssa::MAX_NEAREST_WRITES {
                bytes.clear();
                break 'bytes;
            }
            for id in nearest {
                let access = graph.access(id);
                let Some((site, held)) = access
                    .site
                    .filter(|_| access.kind == memoryssa::Kind::Def)
                    .and_then(|site| Some((site, stores.of(unit, accesses, placed, site)?)))
                else {
                    bytes.clear();
                    break 'bytes;
                };
                let cell = &held.0;
                let Some(there) = held.1.addr().filter(|there| there.root == address.root) else {
                    bytes.clear();
                    break 'bytes;
                };
                let at = address.disp + i64::from(k) - there.disp;
                if !(0..i64::from(cell.width)).contains(&at) {
                    bytes.clear();
                    break 'bytes;
                }
                from.push((site, at as u32));
            }
            if from.is_empty() {
                bytes.clear();
                break;
            }
            bytes.push(from);
        }
        if bytes.len() == reference.width as usize { Some(Provider::Bytes(bytes)) } else { None }
    } else {
        None
    }
}

/// The stores that serve each load `wanted` picks, found by MemorySSA walks,
/// with the indices `unit.registers` proves constant taken into every address.
pub fn load_providers(
    unit: &Unit,
    accesses: &Accesses,
    wanted: &dyn Fn(ValueId) -> bool,
) -> llrm_mir::dense::IdMap<ValueId, Provider> {
    let Some(placed) = unit.registers else { return Default::default() };
    let function = unit.function;
    let graph = memoryssa::built(unit, accesses).with_known(_intervals(placed));
    let mut providers: llrm_mir::dense::IdMap<ValueId, Provider> = Default::default();
    let stores = Stores::default();
    for (_, inst) in function.walk() {
        let Some((reference, result)) = avail::loaded_into(unit, accesses, inst) else { continue };
        if !wanted(result) {
            continue;
        }
        // An index the registers prove constant is part of the address, in
        // the load and in each store alike.
        let reference = _addressed(unit, &reference, placed);
        if crate::memory::constant_bits(unit, &reference).is_some() {
            continue;
        }
        let ask = |one: &MemRef| graph.nearest_writes(inst, one);
        if let Some(provider) = provider_of(unit, accesses, &graph, &reference, &ask, &stores) {
            providers.insert(result, provider);
        }
    }
    providers
}

/// Where `cells_read` reads: the end of a block, or before an instruction.
#[derive(Clone, Copy)]
pub enum ReadAt {
    End(i64),
    Before(InstId),
}

/// The cells `references` hold at `at`, each as the stores MemorySSA says reach
/// it put it (`known` says what a store puts): the cells of one point that a
/// caller needs, found by walks of those references alone rather than by a
/// solve of every cell of the body.
pub fn cells_read(
    unit: &Unit,
    calls: &Calls,
    known: &IdMap<ValueId, Known>,
    at: ReadAt,
    references: &[MemRef],
    queries: &mut _MemoryQueries,
) -> Cells {
    let ask = |graph: &memoryssa::MemorySSA, one: &MemRef| match at {
        ReadAt::End(block) => graph.clobbers_at_end(block, one),
        ReadAt::Before(site) => graph.clobbers_ignoring_invariance(site, one),
    };
    let exposed = unit.exposed.is_none().then(|| crate::memory::exposed_frames(unit));
    let unit = &exposed.as_ref().map_or(*unit, |table| unit.with_exposed(table));
    let accesses = Accesses::shared_or(unit, || Accesses::plain(unit, calls));
    let known = known.iter().map(|(value, fact)| (value, fact.clone())).collect::<IndexMap<_, _>>();
    let unit = &unit.with_registers(&known);
    let graph = memoryssa::built(unit, &accesses).with_known(_intervals(&known));
    let mut cells = Cells::default();
    let stores = Stores::default();
    for reference in references {
        let reference = _addressed(unit, reference, &known);
        if reference.addr().is_none() || crate::memory::constant_bits(unit, &reference).is_some() {
            continue;
        }
        let Some(provider) = provider_of(unit, &accesses, &graph, &reference, &|one| ask(&graph, one), &stores) else {
            continue;
        };
        let Some(fact) = provided(&provider, 8 * reference.width, &|site| _put(unit, site, &known)) else { continue };
        cells.extend(_fragments(&queries.resolve(&reference), &fact));
    }
    cells
}

/// What `provider`'s stores put in the `width` bits a load reads, where all
/// agree: `put` is the number a store puts.
pub fn provided(
    provider: &Provider,
    width: u32,
    put: &dyn Fn(InstId) -> Option<Known>,
) -> Option<Known> {
    match provider {
        Provider::Whole(sites) => {
            let mut numbers = sites.iter().map(|site| put(*site));
            let first = numbers.next()??;
            numbers
                .all(|one| one.as_ref().is_some_and(|one| one.n == first.n && one.width == first.width))
                .then(|| if first.width == width { _read(Some(&first), width) } else { None })
                .flatten()
        }
        Provider::Bytes(bytes) => {
            let mut number = BigInt::from(0);
            for (k, from) in bytes.iter().enumerate() {
                let mut seen = BTreeSet::new();
                for (site, at) in from {
                    let put = put(*site).filter(|put| put.width > 8 * at)?;
                    seen.insert((&put.n >> (8 * at)) & BigInt::from(255));
                }
                if seen.len() != 1 {
                    return None;
                }
                number |= seen.into_iter().next().expect("one byte") << (8 * k);
            }
            Some(Known::new(number, width))
        }
    }
}

/// What is known of a body through memory: the registers' facts, then each load
/// by what the stores MemorySSA says reach it put in its bytes, where they all
/// put the same (GVN's forwarding with constants; GCC's FRE walks the same
/// virtual use-def chain for a load's value, `vn_reference_lookup`). A store
/// may serve a load of its bytes of another type, or a load of bytes several
/// stores wrote, which is asked of the walk a byte at a time. No state is held
/// per block: the cost is the walks, not cells times joins.
pub fn known_walked(
    unit: &Unit,
    calls: &Calls,
) -> IndexMap<ValueId, Known> {
    known_walked_over(unit, calls, &[])
}

/// `known_walked`, with what counted float loops leave in memory when they exit
/// (`floatfacts::loop_exits`): a load after one, reaching what the loop's exit
/// reaches, holds what it left there.
pub fn known_walked_over(
    unit: &Unit,
    calls: &Calls,
    exits: &[crate::floatfacts::LoopExit],
) -> IndexMap<ValueId, Known> {
    MEMORY_DERIVATIONS.with(|count| count.set(count.get() + 1));
    llrm_mir::counts::bump(llrm_mir::counts::Event::Knowns);
    // What the caller did not bring, as `known` finds it.
    let exposed = unit.exposed.is_none().then(|| crate::memory::exposed_frames(unit));
    let unit = &exposed.as_ref().map_or(*unit, |table| unit.with_exposed(table));
    let registers = unit.registers.cloned().unwrap_or_else(|| known(unit));
    let annotated = unit.references.is_none().then(|| unit.annotated().ok()).flatten();
    let unit = &annotated.as_ref().map_or(*unit, |references| unit.with_references(references));
    let function = unit.function;
    let plain = Accesses::shared_or(unit, || Accesses::plain(unit, calls));
    let left = exit_loads(unit, &plain, &registers, exits);
    let wanted = |result: ValueId| _width(unit, Operand::Value(result)).is_some();
    let solve = |accesses: &Accesses| {
        let mut facts = registers.clone();
        let mut placed = registers.clone();
        let mut providers = load_providers(&unit.with_registers(&placed), accesses, &wanted);
        for _round in 0..4 {
            let mut changing = true;
            while changing {
                changing = false;
                for (_, inst) in function.walk() {
                    let op = function.instruction(inst);
                    let Some(target) = _defined(unit, inst).filter(|target| !facts.contains_key(target)) else {
                        continue;
                    };
                    if op.opcode == Opcode::Phi {
                        let seen =
                            op.operands.iter().step_by(2).map(|&one| incoming(unit, one, &facts)).collect::<Vec<_>>();
                        let known = seen.iter().flatten().collect::<Vec<_>>();
                        if seen.is_empty()
                            || known.len() != seen.len()
                            || known.iter().map(|one| (&one.n, one.width)).collect::<BTreeSet<_>>().len() != 1
                        {
                            continue;
                        }
                        facts.insert(target, (*known[0]).clone());
                        changing = true;
                        continue;
                    }
                    let found = if matches!(op.opcode, Opcode::Load { volatile: false, .. }) {
                        let loaded = _result(unit, inst, &facts, None);
                        loaded
                            .or_else(|| {
                                let values = providers.get(&target)?;
                                let width = _width(unit, Operand::Value(target))?;
                                provided(values, width, &|site| _put(unit, site, &facts))
                            })
                            .or_else(|| left.get(&target).cloned())
                    } else {
                        _result(unit, inst, &facts, None)
                    };
                    if let Some(found) = found {
                        facts.insert(target, found);
                        changing = true;
                    }
                }
            }
            // A load whose index only memory taught us is asked again with it
            // placed.
            let learned = facts.iter().any(|(value, _)| !placed.contains_key(value));
            let unplaced = learned
                && function.walk().any(|(_, inst)| {
                    avail::loaded_into(unit, accesses, inst).is_some_and(|(reference, result)| {
                        !facts.contains_key(&result)
                            && reference
                                .base
                                .is_some_and(|base| facts.contains_key(&base) && !placed.contains_key(&base))
                    })
                });
            if !unplaced {
                break;
            }
            placed = facts.clone();
            providers = load_providers(&unit.with_registers(&placed), accesses, &wanted);
        }
        facts
    };
    // A far store through a selector nothing yet says the value of is taken to
    // write no program object (an absolute segment: BASIC's POKE after DEF
    // SEG); the assumption stands where every such selector came out a
    // number, and the solve is made again without the ones that did not, as
    // `known` does.
    let far_stores: Vec<(InstId, ValueId)> = function
        .walk()
        .filter_map(|(_, inst)| {
            let reference = plain.references.get(&inst)?;
            let store = matches!(function.instruction(inst).opcode, Opcode::Store { .. });
            let Some(Operand::Value(segment)) = reference.segment else { return None };
            (store && reference.space == unit.spaces().far && !registers.contains_key(&segment))
                .then_some((inst, segment))
        })
        .collect();
    let mut allowed: Option<llrm_mir::dense::IdSet<ValueId>> = None;
    let facts = loop {
        let assumed: Vec<(InstId, ValueId)> = far_stores
            .iter()
            .copied()
            .filter(|(_, segment)| allowed.as_ref().is_none_or(|allowed| allowed.contains(segment)))
            .collect();
        // With none assumed the accesses are the shared ones, and so is their
        // graph.
        let facts = if assumed.is_empty() {
            solve(&plain)
        } else {
            solve(&plain.clone().into_owned().without_absolute_writes(&assumed.iter().map(|(inst, _)| *inst).collect()))
        };
        let resolved: llrm_mir::dense::IdSet<ValueId> =
            assumed.iter().map(|(_, segment)| *segment).filter(|segment| facts.contains_key(segment)).collect();
        if assumed.iter().all(|(_, segment)| resolved.contains(segment)) {
            break facts;
        }
        allowed = Some(resolved);
    };
    constant_cycles::propagated(unit, facts, None, None)
}

#[cfg(test)]
#[path = "consts_tests.rs"]
mod tests;
