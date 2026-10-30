//! What the middle end proves of an integer value, for the instruction
//! selector to read: the bounds of an index or a dividend.
//!
//! # How it works
//! [`ValueFacts`] answers `get(at, value)` with the value as a signed
//! integer interval where it is read at `at`, or none where nothing narrower
//! than the type is proven. The interval is the intersection of what
//! ScalarEvolution proves of the value (`getUnsignedRange`, `getSignedRange`:
//! its loop bounds), what ValueTracking proves under the branches and
//! assumptions that dominate `at` (`computeConstantRange` with known bits),
//! and what LazyValueInfo proves at `at`. The selector asks no analysis
//! itself: the table is made from the function once and asked many times,
//! each (block, value) worked out at its first ask; LazyValueInfo works on a
//! copy of the context, made at that ask, because it makes constants.
//!
//! [`exact_offsets`] marks the index values every access names exactly at any
//! wider width, from the same table.
//!
//! DEVIATE V1: the pin's `X86ISelDAGToDAG::matchAddress` asks
//! `SelectionDAG::computeKnownBits` of the address operand in place, on the
//! lowered DAG. MIR is lowered to LIR before selection and there is no
//! LIR-level known-bits analysis, so the facts are the IR-level range of the
//! same value, handed over in a table. Exit: a LIR known-bits analysis, or
//! the lowering attaching the ranges to the LIR value.
//!
//! DEVIATE V2: the table knows more than the counted-loop solve it replaces
//! (the range of a `urem`, of a masked value). `isel::significant_bits` reads
//! it only for a range that needs the whole width; a narrower one prices a
//! reciprocal as an unknown dividend does, the early-out minimum, because
//! pricing by it chose `idiv` for `switch` (-8.6% clocks) where the emulator
//! counts the reciprocal cheaper. Exit: the multiply and divide pricing of
//! `cpu.rs` (lir-cleanup-port) agrees with the emulator.
//!
//! # Example
//! `a[i]` over `for i in 0..8` with `a: [8 x i16]` and `inbounds`: the range
//! of `%i` is `[0, 7]`, so `exact_offsets` holds `%i` and the address folds
//! to `a[ebx+ebx]`.
//!
//! # Runs
//! Once per function selected, in `isel::selected_with`.

use std::cell::RefCell;

use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::num::Num;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_mir::constantrange::{ConstantRange, PreferredRangeType};
use llrm_mir::context::{Context, signed};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::domtree::DomTree;
use llrm_mir::lazyvalueinfo::{Env, LazyValueInfo};
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, MetadataNode, Module, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::valuetracking::range::compute_constant_range_including_known_bits;
use llrm_mir::valuetracking::{AssumptionCache, DomConditionCache, Query};
use llrm_support::hash::IndexMap;

/// A non-wrapping mathematical interval at a fixed width.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Span {
    pub low: Num,
    pub high: Num,
    pub width: u32,
}

/// The intervals of a function's integer values, worked out at their first ask.
pub struct ValueFacts<'m> {
    context: &'m Context,
    layout: DataLayout,
    globals: &'m [GlobalValue],
    metadata: &'m [MetadataNode],
    function: &'m Function,
    session: RefCell<ScevSession>,
    tree: DomTree,
    assumptions: AssumptionCache,
    conditions: DomConditionCache,
    /// LazyValueInfo's answers, and the copy of the context it makes constants
    /// in (the module's is shared and read-only), made at the first ask.
    lvi: RefCell<LazyValueInfo>,
    scratch: RefCell<Option<Context>>,
    spans: RefCell<IndexMap<(BlockId, ValueId), Option<Span>>>,
}

impl<'m> ValueFacts<'m> {
    pub fn new(
        module: &'m Module,
        layout: &DataLayout,
        function: &'m Function,
    ) -> Self {
        let session = ScevSession::within(&module.context, &module.globals, &module.metadata, function);
        let tree = DomTree::of(function);
        let assumptions = AssumptionCache::of(&module.context, &module.globals, &module.metadata, function);
        let conditions = DomConditionCache::of(&module.context, &module.globals, &module.metadata, layout, function);
        Self {
            tree,
            assumptions,
            conditions,
            lvi: RefCell::default(),
            scratch: RefCell::default(),
            context: &module.context,
            layout: layout.clone(),
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            session: RefCell::new(session),
            spans: RefCell::default(),
        }
    }

    /// `value`'s interval where it is read at `at`, if narrower than its type.
    pub fn get(
        &self,
        at: InstId,
        value: ValueId,
    ) -> Option<Span> {
        let key = (self.function.parent(at)?, value);
        if let Some(known) = self.spans.borrow().get(&key) {
            return known.clone();
        }
        let span = self.solve(at, value);
        self.spans.borrow_mut().insert(key, span.clone());
        span
    }

    /// What LazyValueInfo proves of `operand` at `at`: the branches that
    /// dominate it narrow it.
    fn lvi_range(
        &self,
        operand: Operand,
        at: InstId,
    ) -> ConstantRange {
        let mut scratch = self.scratch.borrow_mut();
        let context = scratch.get_or_insert_with(|| self.context.clone());
        let mut env = Env::new(context, self.globals, self.metadata, self.function, &self.layout, &self.assumptions);
        self.lvi.borrow_mut().get_constant_range(&mut env, operand, at, false)
    }

    fn solve(
        &self,
        at: InstId,
        value: ValueId,
    ) -> Option<Span> {
        let operand = Operand::Value(value);
        let width = self
            .function
            .operand_type(self.context, operand)
            .and_then(|ty| self.context.types.int_bits(ty))
            .filter(|&width| width <= 64)?;
        let (unsigned_range, signed_range) = self
            .session
            .borrow_mut()
            .ask_within(
                self.context,
                &self.layout,
                self.globals,
                self.metadata,
                self.function,
                |scev, cx| {
                    if !scev.is_value_scevable(cx, operand) {
                        return None;
                    }
                    let s = scev.get_scev(cx, operand);
                    Some((scev.get_unsigned_range(cx, s), scev.get_signed_range(cx, s)))
                },
            )?;
        // What the conditions that dominate `at` and the assumptions say.
        let query = Query {
            dt: Some(&self.tree),
            assumptions: Some(&self.assumptions),
            dom_conditions: Some(&self.conditions),
            cxt: Some(at),
            ..Query::bare(self.context, self.function, &self.layout)
        };
        let query = Query { globals: self.globals, metadata: self.metadata, ..query };
        let by_branch = self.lvi_range(operand, at);
        let (unsigned_range, signed_range) = (
            unsigned_range
                .intersect_with(
                    &compute_constant_range_including_known_bits(&query, operand, false),
                    PreferredRangeType::Unsigned,
                )
                .intersect_with(&by_branch, PreferredRangeType::Unsigned),
            signed_range
                .intersect_with(
                    &compute_constant_range_including_known_bits(&query, operand, true),
                    PreferredRangeType::Signed,
                )
                .intersect_with(&by_branch, PreferredRangeType::Signed),
        );
        // The value as a signed integer: the signed envelope, narrowed by the
        // unsigned one where that lies wholly on one side of the sign bit.
        let half = 1_i128 << (width - 1);
        let (mut low, mut high) =
            (signed(signed_range.get_signed_min(), width), signed(signed_range.get_signed_max(), width));
        let (unsigned_low, unsigned_high) = (
            i128::try_from(unsigned_range.get_unsigned_min()).ok()?,
            i128::try_from(unsigned_range.get_unsigned_max()).ok()?,
        );
        if unsigned_high < half {
            (low, high) = (low.max(unsigned_low), high.min(unsigned_high));
        } else if unsigned_low >= half {
            (low, high) = (low.max(unsigned_low - 2 * half), high.min(unsigned_high - 2 * half));
        }
        if low > high {
            return None;
        }
        if (low, high) != (-half, half - 1) {
            return Some(Span { low: Num::from(low), high: Num::from(high), width });
        }
        None
    }
}

/// The values that index accesses whose offset every wider sum names exactly.
///
/// An index added to its object's own address is summed at the pointer's
/// index width, which wraps. The sum names the same bytes at any wider
/// width when every partial sum of the offset, as the affine operations
/// computing it would be cut anywhere, is a non-negative integer below
/// 2**index_bits: zero extension is then the identity. `inbounds` is the
/// promise that places the start. A value is exact only if every access it
/// indexes is.
pub fn exact_offsets(
    unit: &Unit,
    facts: &ValueFacts,
) -> IdSet<ValueId> {
    let mut verdict = IdMap::<ValueId, bool>::new();
    for (_, inst) in unit.function.walk() {
        let Some(reference) = MemRef::of(unit, inst) else { continue };
        let Some(base) = reference.base else { continue };
        let bits = reference.index_bits;
        // A far pointer made from a selector starts at offset 0 as well.
        let placed = (reference.inbounds && reference.object) || reference.segment.is_some();
        let exact = placed
            && reference.base_width == bits
            && exact_sum(unit, base, inst, facts, bits, 16).is_some_and(|(low, high)| {
                let (low, high) = (
                    Num::from(reference.disp) + low * reference.scale,
                    Num::from(reference.disp) + high * reference.scale,
                );
                low >= Num::from(0) && high < Num::from(1) << bits
            });
        *verdict.get_or_insert_with(base, || true) &= exact;
    }
    verdict.iter().filter(|(_, exact)| **exact).map(|(value, _)| value).collect()
}

/// The integer range of `value`, where it and every affine partial sum
/// computing it is a non-negative integer below 2**bits; leaves read at `at`.
fn exact_sum(
    unit: &Unit,
    value: ValueId,
    at: InstId,
    facts: &ValueFacts,
    bits: u32,
    depth: usize,
) -> Option<(Num, Num)> {
    // Past the depth, a node is unproven, not a leaf.
    if depth == 0 {
        return None;
    }
    let limit = Num::from(1) << bits;
    let inside = |(low, high): (Num, Num)| (low >= Num::from(0) && high < limit).then_some((low, high));
    let operand = |one: Operand| -> Option<(Num, Num)> {
        if unit.int_bits(one) != Some(bits) {
            return None;
        }
        match one {
            Operand::Value(value) => exact_sum(unit, value, at, facts, bits, depth - 1),
            _ => unit.int_constant(one).map(|n| (Num::from(n), Num::from(n))),
        }
    };
    let constant = |one: Operand| unit.int_constant(one).map(Num::from);
    if let Some((_, op)) = unit.defining(Operand::Value(value)) {
        let affine = match (&op.opcode, op.operands.as_slice()) {
            (Opcode::Binary(BinaryOp::Add), &[left, right]) => {
                Some(operand(left).zip(operand(right)).map(|(l, r)| (l.0 + r.0, l.1 + r.1)))
            }
            (Opcode::Binary(BinaryOp::Sub), &[left, right]) if constant(right).is_some() => {
                Some(operand(left).zip(operand(right)).map(|(l, r)| (l.0 - r.1, l.1 - r.0)))
            }
            (Opcode::Binary(BinaryOp::Mul), &[left, right]) | (Opcode::Binary(BinaryOp::Mul), &[right, left])
                if constant(right).is_some() =>
            {
                Some(operand(left).zip(operand(right)).map(|(l, r)| (l.0 * &r.0, l.1 * r.0)))
            }
            (Opcode::Binary(BinaryOp::Shl), &[left, right]) => {
                let count = constant(right).and_then(|count| u32::try_from(count).ok()).filter(|count| *count < bits);
                Some(operand(left).zip(count).map(|(l, count)| (l.0 << count, l.1 << count)))
            }
            _ => None,
        };
        if let Some(bounds) = affine {
            return bounds.and_then(inside);
        }
    }
    // A leaf: whatever it is, it is read at the access.
    let fact = facts.get(at, value)?;
    (fact.width == bits).then(|| (fact.low, fact.high)).and_then(inside)
}
