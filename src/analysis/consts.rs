//! Which values are known, and what they are.
//!
//! Port of `qbopt/analysis/consts.py`.  A fact is a width as well as a
//! number -- "the low `width` bytes of this value are `n`" -- and an
//! operation only folds where the widths agree.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::rc::Rc;

use indexmap::IndexMap;
use num_bigint::BigInt;

use super::ranges::Interval;
use super::{constant_cycles, effects, loops, memoryssa, ranges};
use crate::abi::runtime;
use crate::model::mir::{self, Arg, Cell, Held, Kind, MemRef, MirBody, Op, Value};
use crate::objectfile::module::{Addr, Space};

type Binary = fn(&BigInt, &BigInt) -> BigInt;
type Unary = fn(&BigInt) -> BigInt;

/// What each operation does to two known numbers, within one width.
pub(crate) static ARITH: [(Kind, Binary); 8] = [
    (Kind::Add, |a, b| a + b),
    (Kind::Sub, |a, b| a - b),
    (Kind::And, |a, b| a & b),
    (Kind::Or, |a, b| a | b),
    (Kind::Xor, |a, b| a ^ b),
    (Kind::Shl, |a, b| a << u32::try_from(b & BigInt::from(31)).expect("five bits")),
    (Kind::Shr, |a, b| (a & BigInt::from(0xFFFF_FFFF_u32)) >> u32::try_from(b & BigInt::from(31)).expect("five bits")),
    (Kind::Mul, |a, b| a * b),
];

pub(crate) static UNARY: [(Kind, Unary); 2] = [(Kind::Neg, |a| -a), (Kind::Not, |a| !a)];

/// Memory facts are stored as bytes so partial writes and control-flow
/// joins do not discard an untouched neighbor.
pub(crate) type Cells = IndexMap<(Addr, u32), Known>;

/// Alias questions for one immutable known-value epoch.
///
/// Python keys both caches on `id(ref)`; the address of the reference in
/// the body (and of the resolved copy) is that identity here.
pub(crate) struct _MemoryQueries<'a> {
    pub known: &'a IndexMap<Value, Known>,
    pub dgroup: &'a BTreeSet<i64>,
    pub facts: BTreeMap<Value, Interval>,
    pub addressed: HashMap<usize, Rc<MemRef>>,
    pub overlaps: HashMap<((Addr, u32), usize), bool>,
}

impl<'a> _MemoryQueries<'a> {
    pub(crate) fn new(known: &'a IndexMap<Value, Known>, dgroup: &'a BTreeSet<i64>) -> Self {
        Self {
            known,
            dgroup,
            facts: _intervals(known),
            addressed: HashMap::new(),
            overlaps: HashMap::new(),
        }
    }

    pub(crate) fn resolve(&mut self, reference: &MemRef) -> Rc<MemRef> {
        let key = std::ptr::from_ref(reference) as usize;
        let known = self.known;
        self.addressed
            .entry(key)
            .or_insert_with(|| Rc::new(_addressed(reference, known)))
            .clone()
    }

    pub(crate) fn may_overlap(&mut self, where_: (Addr, u32), reference: &Rc<MemRef>) -> bool {
        let key = (where_, Rc::as_ptr(reference) as usize);
        if let Some(answer) = self.overlaps.get(&key) {
            return *answer;
        }
        // `mir.overlapping` hands `dgroup` to regions as a layout, and a
        // frozenset is not a `module.Group`: that is `layout=None`.  An
        // endpoint Rust cannot represent is taken to overlap.
        let _ = self.dgroup;
        let answer = super::regions::overlapping(
            &MemRef::new(Some(where_.0), where_.1),
            reference,
            Some(&self.facts),
            Some(&self.facts),
            None,
        )
        .unwrap_or(true);
        self.overlaps.insert(key, answer);
        answer
    }
}

type ReuseKey = (usize, Option<BTreeSet<i64>>, Option<Vec<(i64, String)>>);

thread_local! {
    #[allow(non_upper_case_globals)]
    /// Python's `_reuse` context variable.  The saved body is compared as
    /// well as its address: Python holds the body alive, Rust cannot.
    static _reuse: RefCell<Option<HashMap<ReuseKey, (MirBody, IndexMap<Value, Known>)>>> =
        const { RefCell::new(None) };
}

/// Reuse ordinary constant facts for identical bodies in one transaction.
pub(crate) fn reusing<T>(inside: impl FnOnce() -> T) -> T {
    let token = _reuse.with(|reuse| reuse.replace(Some(HashMap::new())));
    let result = inside();
    _reuse.with(|reuse| *reuse.borrow_mut() = token);
    result
}

fn _reuse_key(
    body: &MirBody,
    dgroup: Option<&BTreeSet<i64>>,
    calls: Option<&IndexMap<i64, String>>,
    edges: Option<&IndexMap<(i64, i64), Cells>>,
    initial: Option<&Cells>,
) -> Option<ReuseKey> {
    if edges.is_some() || initial.is_some() {
        return None;
    }
    let named = calls.map(|calls| {
        let mut items = calls.iter().map(|(at, name)| (*at, name.clone())).collect::<Vec<_>>();
        items.sort();
        items
    });
    Some((std::ptr::from_ref(body) as usize, dgroup.cloned(), named))
}

/// The low `width` bytes of a value are `n`. Nothing is said above them.
#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct Known {
    pub n: BigInt,
    pub width: u32,
}

impl Known {
    pub(crate) fn new(n: impl Into<BigInt>, width: u32) -> Self {
        Self { n: n.into(), width }
    }
}

impl fmt::Debug for Known {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#x}:{}", self.n, self.width)
    }
}

pub(crate) fn masked(n: &BigInt, width: u32) -> BigInt {
    n & ((BigInt::from(1) << (width * 8)) - 1)
}

/// Python's `arg.width`, for an operand that has one.
fn _width(arg: &Arg) -> u32 {
    match arg {
        Arg::Held(held) => held.width,
        Arg::Const(constant) => constant.width,
        Arg::Symbol(symbol) => symbol.width,
        Arg::FrameAddress(address) => address.width,
        Arg::FrameSelector(selector) => selector.width,
        Arg::Cell(_) => panic!("'Cell' object has no attribute 'width'"),
        Arg::Opaque(_) => panic!("'Opaque' object has no attribute 'width'"),
    }
}

/// Integer quotient and remainder, excluding the faulting cases.
pub(crate) fn division(op: &Op, known: &IndexMap<Value, Known>, here: &Cells) -> Option<(BigInt, BigInt)> {
    if !matches!(op.kind, Kind::Divmod | Kind::Udivmod)
        || op.args.len() != 2
        || op.results.len() != 2
        || !op.stores.is_empty()
    {
        return None;
    }
    if op.results.iter().any(|result| !matches!(result, Arg::Held(_))) {
        return None;
    }
    let widths = op.results.iter().map(_width).collect::<BTreeSet<_>>();
    if widths.len() != 1 || !widths.iter().all(|width| matches!(width, 2 | 4 | 8)) {
        return None;
    }
    let width = *widths.first().expect("one width");
    let operands = op
        .args
        .iter()
        .map(|arg| _operand(op, arg, known, Some(here)))
        .collect::<Vec<_>>();
    if operands.iter().any(|fact| fact.as_ref().is_none_or(|fact| fact.width < width)) {
        return None;
    }
    let operands = operands.into_iter().flatten().collect::<Vec<_>>();
    let (dividend, divisor) = if op.kind == Kind::Divmod {
        let sign = BigInt::from(1) << (width * 8 - 1);
        (
            (masked(&operands[0].n, width) ^ &sign) - &sign,
            (masked(&operands[1].n, width) ^ &sign) - &sign,
        )
    } else {
        (masked(&operands[0].n, width), masked(&operands[1].n, width))
    };
    let zero = BigInt::from(0);
    if divisor == zero
        || (op.kind == Kind::Divmod && dividend == -(BigInt::from(1) << (width * 8 - 1)) && divisor == BigInt::from(-1))
    {
        return None;
    }
    let absolute = |n: &BigInt| if *n < BigInt::from(0) { -n } else { n.clone() };
    let mut quotient = absolute(&dividend) / absolute(&divisor);
    if op.kind == Kind::Divmod && (dividend < zero) != (divisor < zero) {
        quotient = -quotient;
    }
    let remainder = &dividend - &quotient * &divisor;
    Some((masked(&quotient, width), masked(&remainder, width)))
}

/// What this store puts in the cell, where that is a number.
fn _put(op: &Op, known: &IndexMap<Value, Known>) -> Option<Known> {
    if op.kind != Kind::Store || op.args.len() != 1 {
        return None;
    }
    match &op.args[0] {
        Arg::Const(source) => Some(Known::new(masked(&source.n, source.width), source.width)),
        Arg::Held(source) => known.get(&source.value).cloned(),
        _ => None,
    }
}

/// The complete value a direct constant store writes to a contained cell.
pub(crate) fn initialized(op: &Op, reference: &MemRef) -> Option<Known> {
    if op.kind != Kind::Store || !op.loads.is_empty() || op.barrier() || op.stores.len() != 1 {
        return None;
    }
    let written = mir::symbolic_ref(&op.stores[0]);
    if written.addr.is_none() || written.base.is_some() || written.segment.is_some() {
        return None;
    }
    let fact = _put(op, &IndexMap::new())?;
    _cell(&Cells::from([((written.addr?, written.width), fact)]), reference)
}

/// The value of an exact scalar read-modify-write, before its store kills the facts.
pub(crate) fn updated(op: &Op, known: &IndexMap<Value, Known>, here: &Cells) -> Option<Known> {
    if op.barrier()
        || op.floating.is_some()
        || !op.merges.is_empty()
        || op.stores.len() != 1
        || op.loads != op.stores
        || op.results
            != [Arg::Cell(Cell {
                r#ref: op.stores[0].clone(),
            })]
        || op.defines.iter().any(|value| !value.flags)
    {
        return None;
    }
    let width = op.stores[0].width;
    if !matches!(width, 2 | 4) {
        return None;
    }
    let parts = op
        .args
        .iter()
        .map(|arg| _operand(op, arg, known, Some(here)))
        .collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|fact| fact.as_ref().is_none_or(|fact| fact.width < width)) {
        return None;
    }
    let parts = parts.into_iter().flatten().collect::<Vec<_>>();
    let arith = ARITH.iter().find(|(kind, _)| *kind == op.kind);
    let unary = UNARY.iter().find(|(kind, _)| *kind == op.kind);
    let result = if let (Some((_, arith)), 2) = (arith, parts.len()) {
        arith(&parts[0].n, &parts[1].n)
    } else if let (Some((_, unary)), 1) = (unary, parts.len()) {
        unary(&parts[0].n)
    } else {
        let step = if parts.len() == 1 {
            mir::stepping(&Op {
                loads: Vec::new(),
                stores: Vec::new(),
                ..op.clone()
            })
        } else {
            None
        };
        let Some((_, step)) = step else {
            return None;
        };
        let Arg::Const(step) = step else {
            return None;
        };
        &parts[0].n + &step.n
    };
    Some(Known::new(masked(&result, width), width))
}

pub(crate) fn _fragments(reference: &MemRef, fact: &Known) -> Cells {
    let addr = reference.addr.expect("a fragment is of an addressed cell");
    (0..reference.width.min(fact.width))
        .map(|offset| {
            (
                (addr.plus(i64::from(offset)), 1),
                Known::new((&fact.n >> (offset * 8)) & BigInt::from(255), 1),
            )
        })
        .collect()
}

/// A far store's selector, where nothing yet says which segment it is
/// and it is still one this run may take on faith.
fn _selector(
    reference: &MemRef,
    known: &IndexMap<Value, Known>,
    allowed: Option<&BTreeSet<Value>>,
) -> Option<Value> {
    let addr = reference.addr?;
    let segment = reference.segment?;
    if addr.space != Space::Far {
        return None;
    }
    if known.contains_key(&segment) || allowed.is_some_and(|allowed| !allowed.contains(&segment)) {
        return None;
    }
    Some(segment)
}

/// What each value is, as the alias lattice asks for it.
fn _intervals(known: &IndexMap<Value, Known>) -> BTreeMap<Value, Interval> {
    known
        .iter()
        .map(|(value, fact)| {
            (
                *value,
                Interval {
                    low: fact.n.clone(),
                    high: fact.n.clone(),
                    width: fact.width,
                },
            )
        })
        .collect()
}

/// The cell facts still standing after this operation.
///
/// `assume` collects the far selectors this took on faith; the caller
/// checks afterwards that every one of them did resolve.
#[allow(clippy::too_many_arguments)]
pub(crate) fn _kills<'a>(
    here: &Cells,
    op: &Op,
    known: &'a IndexMap<Value, Known>,
    dgroup: &'a BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    mut assume: Option<&mut BTreeSet<Value>>,
    allowed: Option<&BTreeSet<Value>>,
    edge_facts: bool,
    queries: Option<&mut _MemoryQueries<'a>>,
) -> Cells {
    let mut here = here.clone();
    // A fact supplied for one CFG edge is a proof about reaching that edge,
    // not a durable summary of a callee.
    if edge_facts && op.kind == Kind::Call {
        here = Cells::new();
    }
    if effects::unmodeled_write(op) && (op.barrier() || !calls.contains_key(&op.at)) {
        here = Cells::new();
    }
    if op.kind == Kind::Call && op.stores.is_empty() {
        if let Some(name) = calls.get(&op.at) {
            // Only a call with no stores has to be taken at its word.
            let contract = runtime::contract(Some(name));
            if runtime::barrier(&contract) || runtime::writes_caller_memory(&contract) {
                here = Cells::new();
            }
        }
    }
    let put = if op.kind == Kind::Store {
        _put(op, known)
    } else {
        updated(op, known, &here)
    };
    let mut local;
    let queries = match queries {
        Some(queries) => queries,
        None => {
            local = _MemoryQueries::new(known, dgroup);
            &mut local
        }
    };
    for reference in &op.stores {
        let reference = queries.resolve(reference);
        if let Some(assume) = assume.as_deref_mut() {
            if let Some(selector) = _selector(&reference, known, allowed) {
                // A cell in `here` is always a static, so an absolute
                // segment reaches none of them.
                assume.insert(selector);
                continue;
            }
        }
        here = here
            .into_iter()
            .filter(|(where_, _)| !queries.may_overlap(*where_, &reference))
            .collect();
        if let Some(put) = &put {
            if reference.addr.is_some() && reference.base.is_none() && reference.segment.is_none() {
                for (where_, fact) in _fragments(&reference, put) {
                    here.insert(where_, fact);
                }
            }
        }
    }
    if op.kind == Kind::Call && !op.memory_values.is_empty() {
        for (reference, value) in &op.memory_values {
            if reference.addr.is_some() && reference.base.is_none() && reference.segment.is_none() {
                let fact = Known::new(masked(&value.n, value.width), value.width);
                for (where_, fact) in _fragments(reference, &fact) {
                    here.insert(where_, fact);
                }
            }
        }
    }
    here
}

/// What each memory cell holds before each operation, where it is a number.
///
/// Forward to a fixed point, meeting at a join on agreement.  A block none
/// of whose predecessors have been visited yet is deferred, not treated as
/// knowing nothing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cells(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    known: Option<&IndexMap<Value, Known>>,
    initial: Option<&Cells>,
    edges: Option<&IndexMap<(i64, i64), Cells>>,
    mut assume: Option<&mut BTreeSet<Value>>,
    allowed: Option<&BTreeSet<Value>>,
) -> IndexMap<(i64, usize), Cells> {
    let empty = IndexMap::new();
    let known = known.unwrap_or(&empty);
    let mut queries = _MemoryQueries::new(known, dgroup);
    let initial = match initial {
        Some(initial) => initial.clone(),
        None => {
            let mut initial = Cells::new();
            for (reference, value) in &body.initial {
                for (where_, fact) in _fragments(reference, &Known::new(value.n.clone(), value.width)) {
                    initial.insert(where_, fact);
                }
            }
            initial
        }
    };
    let mut outof = body
        .blocks
        .iter()
        .map(|block| (block.at, None))
        .collect::<IndexMap<i64, Option<Cells>>>();
    let preds = body
        .blocks
        .iter()
        .map(|block| {
            (
                block.at,
                body.blocks
                    .iter()
                    .filter(|one| one.succ.contains(&block.at))
                    .map(|one| one.at)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<IndexMap<_, _>>();
    let no_edges = IndexMap::new();
    let edge_map = edges.unwrap_or(&no_edges);
    let edge_facts = edges.is_some_and(|edges| !edges.is_empty());

    let entering = |outof: &IndexMap<i64, Option<Cells>>, at: i64| -> Option<Cells> {
        if preds[&at].is_empty() {
            return Some(if at == body.entry { initial.clone() } else { Cells::new() });
        }
        let mut seen = Vec::new();
        for one in &preds[&at] {
            let Some(here) = &outof[one] else {
                continue;
            };
            let none = Cells::new();
            let extra = edge_map.get(&(*one, at)).unwrap_or(&none);
            if extra.is_empty() {
                seen.push(here.clone());
                continue;
            }
            let mut here = here
                .iter()
                .filter(|(where_, _)| {
                    !(0..where_.1).any(|offset| extra.contains_key(&(where_.0.plus(i64::from(offset)), 1)))
                })
                .map(|(where_, fact)| (*where_, fact.clone()))
                .collect::<Cells>();
            for (where_, fact) in extra {
                here.insert(*where_, fact.clone());
            }
            seen.push(here);
        }
        if at == body.entry {
            seen.push(initial.clone());
        }
        let first = seen.first()?;
        Some(
            first
                .iter()
                .filter(|(where_, fact)| seen[1..].iter().all(|one| one.get(*where_) == Some(*fact)))
                .map(|(where_, fact)| (*where_, fact.clone()))
                .collect(),
        )
    };

    let mut changing = true;
    while changing {
        changing = false;
        for block in &body.blocks {
            let Some(mut here) = entering(&outof, block.at) else {
                continue;
            };
            for op in &block.ops {
                here = _kills(
                    &here,
                    op,
                    known,
                    dgroup,
                    calls,
                    assume.as_deref_mut(),
                    allowed,
                    edge_facts,
                    Some(&mut queries),
                );
            }
            if outof[&block.at].as_ref() != Some(&here) {
                outof.insert(block.at, Some(here));
                changing = true;
            }
        }
    }

    let mut found = IndexMap::new();
    for block in &body.blocks {
        let mut here = entering(&outof, block.at).unwrap_or_default();
        for (index, op) in block.ops.iter().enumerate() {
            found.insert((block.at, index), here.clone());
            here = _kills(
                &here,
                op,
                known,
                dgroup,
                calls,
                assume.as_deref_mut(),
                allowed,
                edge_facts,
                Some(&mut queries),
            );
        }
    }
    found
}

fn _read(fact: Option<&Known>, width: u32) -> Option<Known> {
    let fact = fact?;
    if fact.width < width {
        return None;
    }
    Some(Known::new(masked(&fact.n, width), width))
}

pub(crate) fn _cell(here: &Cells, reference: &MemRef) -> Option<Known> {
    let reference = mir::symbolic_ref(reference);
    let addr = reference.addr?;
    if reference.base.is_some() || reference.segment.is_some() {
        return None;
    }
    if let Some(exact) = _read(here.get(&(addr, reference.width)), reference.width) {
        return Some(exact);
    }
    let mut number = BigInt::from(0);
    for offset in 0..reference.width {
        let wanted = addr.plus(i64::from(offset));
        let fragments = here
            .iter()
            .flat_map(|((address, width), fact)| {
                (0..(*width).min(fact.width))
                    .filter(move |byte| address.plus(i64::from(*byte)) == wanted)
                    .map(move |byte| (&fact.n >> (8 * byte)) & BigInt::from(255))
            })
            .collect::<BTreeSet<_>>();
        if fragments.len() != 1 {
            return None;
        }
        number |= fragments.into_iter().next().expect("one fragment") << (8 * offset);
    }
    Some(Known::new(number, reference.width))
}

/// Resolve one proven constant offset using the existing no-wrap address proof.
fn _addressed(reference: &MemRef, known: &IndexMap<Value, Known>) -> MemRef {
    let reference = mir::symbolic_ref(reference);
    let Some(base) = reference.base else {
        return reference;
    };
    let interval = ranges::_operand(
        &Arg::Held(Held {
            value: base,
            width: reference.base_width,
        }),
        &IndexMap::new(),
        known,
    );
    let Some(interval) = interval else {
        return reference;
    };
    ranges::covering(&reference, &BTreeMap::from([(base, interval)]))
}

/// One operand as a number, if it is one.
pub(crate) fn _operand(
    _op: &Op,
    one: &Arg,
    known: &IndexMap<Value, Known>,
    here: Option<&Cells>,
) -> Option<Known> {
    match one {
        Arg::Const(one) => Some(Known::new(masked(&one.n, one.width), one.width)),
        Arg::Held(one) => _read(known.get(&one.value), one.width),
        // A cell whose content is known is as good as a constant.
        Arg::Cell(one) => here.and_then(|here| _cell(here, &_addressed(&one.r#ref, known))),
        _ => None,
    }
}

/// The value this operation's first result gets, flags aside.
pub(crate) fn _defined(op: &Op) -> Option<Value> {
    let real = op.defines.iter().copied().filter(|one| !one.flags).collect::<Vec<_>>();
    if real.len() == 1 {
        return Some(real[0]);
    }
    let first = op.results.iter().find_map(|one| match one {
        Arg::Held(held) => Some(held.value),
        _ => None,
    })?;
    real.contains(&first).then_some(first)
}

/// What this operation computes, where every input is known.
pub(crate) fn _result(
    op: &Op,
    known: &IndexMap<Value, Known>,
    here: Option<&Cells>,
    carries: Option<&IndexMap<Value, BigInt>>,
) -> Option<Known> {
    _defined(op)?;
    if op.kind == Kind::Extract && op.args.len() == 2 && op.results.len() == 1 {
        let (source, offset) = (&op.args[0], &op.args[1]);
        let fact = _operand(op, source, known, here);
        let width = _width(&op.results[0]);
        if let (Arg::Const(offset), Some(fact)) = (offset, fact) {
            if offset.n >= BigInt::from(0) && BigInt::from(fact.width * 8) >= &offset.n + BigInt::from(width * 8) {
                let shift = u32::try_from(&offset.n).expect("an offset inside the fact");
                return Some(Known::new(masked(&(&fact.n >> shift), width), width));
            }
        }
        return None;
    }
    if matches!(op.kind, Kind::Xor | Kind::Sub)
        && op.args.len() == 2
        && matches!(&op.args[0], Arg::Held(_))
        && op.args[0] == op.args[1]
    {
        return Some(Known::new(0, _width(&op.args[0])));
    }
    let mut parts = Vec::new();
    for one in &op.args {
        parts.push(_operand(op, one, known, here)?);
    }
    if parts.is_empty() {
        return None;
    }
    if matches!(op.kind, Kind::SignExtend | Kind::ZeroExtend) && parts.len() == 1 && op.results.len() == 1 {
        let (source, result) = (&op.args[0], &op.results[0]);
        let source_width = match source {
            Arg::Cell(cell) => cell.r#ref.width,
            Arg::Opaque(_) => 0,
            other => _width(other),
        };
        let Arg::Held(result) = result else {
            return None;
        };
        if !matches!(source, Arg::Held(_) | Arg::Const(_) | Arg::Cell(_))
            || !(0 < source_width && source_width < result.width && result.width <= 8)
            || parts[0].width < source_width
        {
            return None;
        }
        let mut number = masked(&parts[0].n, source_width);
        if op.kind == Kind::SignExtend {
            let sign = BigInt::from(1) << (source_width * 8 - 1);
            number = (number ^ &sign) - sign;
        }
        return Some(Known::new(masked(&number, result.width), result.width));
    }
    if op.kind == Kind::Concat && parts.len() == 2 && op.results.len() == 1 {
        let (high, low) = (&op.args[0], &op.args[1]);
        let width = _width(high) + _width(low);
        if _width(&op.results[0]) != width
            || parts.iter().zip(&op.args).any(|(fact, arg)| fact.width < _width(arg))
        {
            return None;
        }
        return Some(Known::new(
            (masked(&parts[0].n, _width(high)) << (_width(low) * 8)) | masked(&parts[1].n, _width(low)),
            width,
        ));
    }
    if matches!(op.kind, Kind::Shl | Kind::Shr) && parts.len() == 2 && op.results.len() == 1 {
        let (source, result) = (&op.args[0], &op.results[0]);
        let Arg::Held(result) = result else {
            return None;
        };
        if !matches!(source, Arg::Held(_) | Arg::Const(_))
            || _width(source) != result.width
            || parts[0].width < _width(source)
        {
            return None;
        }
        let count = u32::try_from(&parts[1].n & BigInt::from(result.width * 8 - 1)).expect("a masked count");
        let number = masked(&parts[0].n, result.width);
        let shifted = if op.kind == Kind::Shl {
            number << count
        } else {
            number >> count
        };
        return Some(Known::new(masked(&shifted, result.width), result.width));
    }
    let width = parts.iter().map(|one| one.width).min().expect("parts");
    if op.kind == Kind::Smulhi && parts.len() == 2 && op.results.len() == 1 {
        let Arg::Held(result) = &op.results[0] else {
            return None;
        };
        if !matches!(result.width, 2 | 4)
            || op
                .args
                .iter()
                .any(|arg| !matches!(arg, Arg::Held(_) | Arg::Const(_)) || _width(arg) != result.width)
            || width < result.width
        {
            return None;
        }
        let width = result.width;
        let sign = BigInt::from(1) << (width * 8 - 1);
        let first = (masked(&parts[0].n, width) ^ &sign) - &sign;
        let second = (masked(&parts[1].n, width) ^ &sign) - &sign;
        return Some(Known::new(masked(&((first * second) >> (width * 8)), width), width));
    }
    if op.kind == Kind::AddCarry && parts.len() == 2 {
        let flags = op.uses.iter().filter(|value| value.flags).collect::<Vec<_>>();
        if flags.len() == 1 {
            if let Some(carry) = carries.and_then(|carries| carries.get(flags[0])) {
                return Some(Known::new(masked(&(&parts[0].n + &parts[1].n + carry), width), width));
            }
        }
        return None;
    }

    if parts.len() == 1 {
        if let Some((_, Arg::Const(step))) = mir::stepping(op) {
            return Some(Known::new(masked(&(&parts[0].n + &step.n), width), width));
        }
    }

    if matches!(op.kind, Kind::Copy | Kind::Load) && parts.len() == 1 {
        return Some(Known::new(masked(&parts[0].n, width), width));
    }
    if let (Some((_, arith)), 2) = (ARITH.iter().find(|(kind, _)| *kind == op.kind), parts.len()) {
        return Some(Known::new(masked(&arith(&parts[0].n, &parts[1].n), width), width));
    }
    if let (Some((_, unary)), 1) = (UNARY.iter().find(|(kind, _)| *kind == op.kind), parts.len()) {
        return Some(Known::new(masked(&unary(&parts[0].n), width), width));
    }
    None
}

fn _carry(op: &Op, facts: &IndexMap<Value, Known>, here: &Cells) -> Option<BigInt> {
    if op.kind != Kind::Add || op.args.len() != 2 || op.results.len() != 1 {
        return None;
    }
    let Arg::Held(result) = &op.results[0] else {
        return None;
    };
    let width = result.width;
    let mut sum = BigInt::from(0);
    for arg in &op.args {
        let fact = _operand(op, arg, facts, Some(here)).filter(|fact| fact.width >= width)?;
        sum += masked(&fact.n, width);
    }
    Some(BigInt::from(u8::from(sum >= BigInt::from(1) << (width * 8))))
}

/// Dominating, exact stores supplying whole-pointer loads outside the static-cell lattice.
fn _pointer_stores(body: &MirBody, _dgroup: &BTreeSet<i64>) -> IndexMap<Value, Arg> {
    let candidates = body
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .ops
                .iter()
                .enumerate()
                .map(move |(index, op)| (memoryssa::Site { block: block.at, index }, op))
        })
        .filter(|(_, op)| {
            op.kind == Kind::Load
                && !op.barrier()
                && op.floating.is_none()
                && op.stores.is_empty()
                && op.loads.len() == 1
                && op.args.len() == 1
                && op.results.len() == 1
                && op.loads[0].pointer
                && op.args
                    == [Arg::Cell(Cell {
                        r#ref: op.loads[0].clone(),
                    })]
                && matches!(&op.results[0], Arg::Held(held) if held.width == op.loads[0].width)
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return IndexMap::new();
    }
    let graph = memoryssa::built(body);
    let accesses = graph
        .accesses
        .iter()
        .map(|access| (access.id, access))
        .collect::<IndexMap<_, _>>();
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let mut providers = IndexMap::new();
    for (site, op) in candidates {
        // A plain `dgroup` set is no layout to `regions`; only a `Group` is.
        let clobbers = graph.clobbers(site, &op.loads[0], None);
        let access = if clobbers.len() == 1 {
            Some(accesses[clobbers.first().expect("one clobber")])
        } else {
            None
        };
        let Some(source) = access
            .filter(|access| access.kind == memoryssa::Kind::Def)
            .and_then(|access| access.site)
        else {
            continue;
        };
        if !dominators[&site.block].contains(&source.block) || source.block == site.block && source.index >= site.index
        {
            continue;
        }
        let store = graph.operations[&source];
        if store.kind == Kind::Store
            && !store.barrier()
            && store.floating.is_none()
            && store.loads.is_empty()
            && store.defines.is_empty()
            && store.merges.is_empty()
            && store.stores.len() == 1
            && store.args.len() == 1
            && graph.pointers.same_bytes(&op.loads[0], &store.stores[0])
            && matches!(&store.args[0], Arg::Const(_) | Arg::Held(_))
            && _width(&store.args[0]) == op.loads[0].width
        {
            let Arg::Held(result) = &op.results[0] else {
                unreachable!("a candidate's result is held");
            };
            providers.insert(result.value, store.args[0].clone());
        }
    }
    providers
}

/// Every value this body computes that is a number, to a fixed point.
///
/// Optimistic, then shrinking: a run may assume every selector it does not
/// know is some absolute segment; the ones that came out numbers keep the
/// assumption and the rest lose it, until every one still assumed resolved.
pub(crate) fn known(
    body: &MirBody,
    dgroup: Option<&BTreeSet<i64>>,
    calls: Option<&IndexMap<i64, String>>,
    edges: Option<&IndexMap<(i64, i64), Cells>>,
    initial: Option<&Cells>,
) -> IndexMap<Value, Known> {
    let key = _reuse_key(body, dgroup, calls, edges, initial);
    if let Some(key) = &key {
        let saved = _reuse.with(|reuse| {
            reuse.borrow().as_ref().and_then(|cache| {
                cache
                    .get(key)
                    .filter(|(saved, _)| saved == body)
                    .map(|(_, facts)| facts.clone())
            })
        });
        if let Some(saved) = saved {
            return saved;
        }
    }
    let mut allowed: Option<BTreeSet<Value>> = None;
    loop {
        let (got, assumed) = _solved(body, dgroup, calls, edges, initial, Some(BTreeSet::new()), allowed.as_ref());
        let resolved = assumed
            .iter()
            .filter(|value| got.contains_key(*value))
            .copied()
            .collect::<BTreeSet<_>>();
        if resolved == assumed {
            if let Some(key) = key {
                _reuse.with(|reuse| {
                    if let Some(cache) = reuse.borrow_mut().as_mut() {
                        cache.insert(key, (body.clone(), got.clone()));
                    }
                });
            }
            return got;
        }
        allowed = Some(resolved);
    }
}

fn _solved(
    body: &MirBody,
    dgroup: Option<&BTreeSet<i64>>,
    calls: Option<&IndexMap<i64, String>>,
    edges: Option<&IndexMap<(i64, i64), Cells>>,
    initial: Option<&Cells>,
    mut assume: Option<BTreeSet<Value>>,
    allowed: Option<&BTreeSet<Value>>,
) -> (IndexMap<Value, Known>, BTreeSet<Value>) {
    let mut facts = IndexMap::<Value, Known>::new();
    let mut carries = IndexMap::<Value, BigInt>::new();
    let mut held = IndexMap::<(i64, usize), Cells>::new();
    let pointer_stores = match (dgroup, calls) {
        (Some(dgroup), Some(_)) => _pointer_stores(body, dgroup),
        _ => IndexMap::new(),
    };
    let empty = Cells::new();
    let mut changing = true;
    while changing {
        changing = false;
        // What memory holds, recomputed from what is known so far: the two
        // feed each other and run to one fixed point together.
        if let (Some(dgroup), Some(calls)) = (dgroup, calls) {
            held = cells(body, dgroup, calls, Some(&facts), initial, edges, assume.as_mut(), allowed);
        }
        for block in &body.blocks {
            // A join is known where every path into it agrees.
            for phi in &block.phis {
                if facts.contains_key(&phi.result) || phi.incoming.is_empty() {
                    continue;
                }
                let seen = phi.incoming.values().map(|one| facts.get(one)).collect::<Vec<_>>();
                let known = seen.iter().flatten().collect::<Vec<_>>();
                if known.len() != seen.len()
                    || known.iter().map(|one| (&one.n, one.width)).collect::<BTreeSet<_>>().len() != 1
                {
                    continue;
                }
                let fact = (*known[0]).clone();
                facts.insert(phi.result, fact);
                changing = true;
            }
            for (index, op) in block.ops.iter().enumerate() {
                let here = held.get(&(block.at, index)).unwrap_or(&empty);
                if let Some(carry) = _carry(op, &facts, here) {
                    for value in &op.defines {
                        if value.flags && !carries.contains_key(value) {
                            carries.insert(*value, carry.clone());
                            changing = true;
                        }
                    }
                }
                let Some(target) = _defined(op).filter(|target| !facts.contains_key(target)) else {
                    continue;
                };
                let mut found = _result(op, &facts, Some(here), Some(&carries));
                if found.is_none() {
                    if let Some(source) = pointer_stores.get(&target) {
                        found = _operand(op, source, &facts, None);
                    }
                }
                if let Some(found) = found {
                    facts.insert(target, found);
                    changing = true;
                }
            }
        }
    }
    (constant_cycles::propagated(body, &facts, None), assume.unwrap_or_default())
}

#[cfg(test)]
#[path = "consts_tests.rs"]
mod tests;
