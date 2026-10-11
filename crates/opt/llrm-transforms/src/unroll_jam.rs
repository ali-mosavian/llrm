//! Unroll-and-jam of a loop nest at -O3: gcc's `pass_loop_jam`
//! ([`gimple-loop-jam.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc)),
//! LLVM's `LoopUnrollAndJamPass`.
//!
//! The outer loop is unrolled by N (`loopclone::chained`, the partial unroll
//! of an exact count) and the N copies of the inner loop that results are fused
//! into one, so what the outer iterations share is used by all N in one pass
//! of the inner loop. What the nest may be, as gcc's `unroll_jam_possible_p`
//! has it: a perfect nest of two header-tested loops, the inner one leaving
//! only from its header, its count and its counters the same in every outer
//! iteration, producing nothing the outer loop reads; between the loops only
//! pure work, which moves ahead of the fused loop. N is the largest the
//! dependences allow (`llrm_analysis::dependence`), up to `max_unroll`, that
//! divides the outer loop's exact trip count.
//!
//! Not yet, narrower than gcc: a trip count that N does not divide (gcc runs an
//! epilogue), loads between the loops, a rotated (guarded) inner loop, and an
//! access whose address is not an affine function of the two counters over
//! an identified object.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::dependence::{self, Access, Subscript};
use llrm_analysis::graph::loops::{self, Loop};
use llrm_analysis::manager::Registers;
use llrm_analysis::{cfg, induction, memory};
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use num_traits::ToPrimitive;

use crate::lcssa::{arms, from_arms};
use crate::loopclone::chained;

/// gcc's `unroll-jam-max-unroll` and `unroll-jam-min-percent`
/// ([`params.opt:1146`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/params.opt#L1146)).
pub const MAX_UNROLL: u32 = 4;
pub const MIN_PERCENT: u32 = 1;

pub struct UnrollJam {
    pub max_unroll: u32,
    pub min_percent: u32,
}

impl Default for UnrollJam {
    fn default() -> Self {
        Self { max_unroll: MAX_UNROLL, min_percent: MIN_PERCENT }
    }
}

impl FunctionPass for UnrollJam {
    fn name(&self) -> &'static str {
        "unroll-jam"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        match jammed(unit, analyses, self) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("unroll-jam: {error}"),
        }
    }
}

/// Every nest this can jam, once each; whether any was.
pub fn jammed(
    unit: &mut passes::Unit,
    analyses: &Analyses,
    params: &UnrollJam,
) -> Result<bool, String> {
    let mut changed = false;
    let mut refused = BTreeSet::new();
    while one(unit, analyses, params, &mut refused)? {
        llrm_support::debug!("unroll-jam", "jammed a nest");
        changed = true;
    }
    Ok(changed)
}

/// A nest of two loops: `outer`'s header, then `pre` (pure blocks, in a
/// chain) to the inner loop's header, then the inner loop, whose exit starts
/// `post`, a chain to the outer loop's latch.
struct Nest {
    outer: Loop,
    inner: Loop,
    pre: Vec<BlockId>,
    post: Vec<BlockId>,
}

impl Nest {
    fn outer_header(&self) -> BlockId {
        cfg::block(self.outer.header)
    }

    fn inner_header(&self) -> BlockId {
        cfg::block(self.inner.header)
    }

    fn inner_latch(&self) -> BlockId {
        cfg::block(*self.inner.latches.first().expect("one latch"))
    }

    /// The last of the blocks before the inner loop: where it is entered.
    fn entry(&self) -> BlockId {
        *self.pre.last().expect("a block before the inner loop")
    }
}

/// Work with no memory access and no fault, which may run earlier or later in
/// the outer iteration.
fn pure(opcode: &Opcode) -> bool {
    match opcode {
        Opcode::Binary(op) => !matches!(
            op,
            BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem | BinaryOp::FDiv | BinaryOp::FRem
        ),
        Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::Select | Opcode::GetElementPtr { .. } => true,
        Opcode::Freeze | Opcode::FNeg => true,
        _ => false,
    }
}

fn work(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .filter(|&inst| {
            let op = &function.instruction(inst).opcode;
            *op != Opcode::Phi && !op.is_terminator()
        })
        .collect()
}

fn phis(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .collect()
}

/// `inner`'s nest in `outer`, where `outer` holds nothing else.
fn nest_of(
    function: &Function,
    graph: &[cfg::Block],
    outer: &Loop,
    inner: &Loop,
) -> Option<Nest> {
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(graph);
    let [lo] = outer.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let [li] = inner.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let (ho, hi) = (outer.header, inner.header);
    if ho == hi || li == hi || blocks[&lo].succ != [ho] {
        return None;
    }
    // The outer header tests, one way into the loop.
    let [first] = blocks[&ho].succ.iter().copied().filter(|at| outer.body.contains(at)).collect::<Vec<_>>()[..] else {
        return None;
    };
    if blocks[&ho].succ.len() != 2 {
        return None;
    }
    let inside = |at: &i64| outer.body.contains(at) && !inner.body.contains(at);
    let mut pre = Vec::new();
    let mut at = first;
    while at != hi {
        if !inside(&at) || at == ho || blocks[&at].succ.len() != 1 || pre.contains(&at) {
            return None;
        }
        pre.push(at);
        at = blocks[&at].succ[0];
    }
    if pre.is_empty() || predecessors[&hi] != BTreeSet::from([*pre.last().expect("one"), li]) {
        return None;
    }
    // The inner loop leaves from its header alone, into a block only it enters.
    let leaving = inner.body.iter().filter(|at| blocks[at].succ.iter().any(|to| !inner.body.contains(to))).count();
    let [xi] = blocks[&hi].succ.iter().copied().filter(|at| !inner.body.contains(at)).collect::<Vec<_>>()[..] else {
        return None;
    };
    if leaving != 1 || blocks[&hi].succ.len() != 2 || predecessors[&xi] != BTreeSet::from([hi]) {
        return None;
    }
    let mut post = Vec::new();
    let mut at = xi;
    loop {
        if !inside(&at) || at == ho || blocks[&at].succ.len() != 1 || post.contains(&at) {
            return None;
        }
        post.push(at);
        if at == lo {
            break;
        }
        at = blocks[&at].succ[0];
    }
    let covered = std::iter::once(ho)
        .chain(pre.iter().copied())
        .chain(post.iter().copied())
        .chain(inner.body.iter().copied())
        .collect::<BTreeSet<_>>();
    if covered != outer.body {
        return None;
    }
    let block = cfg::block;
    let pre: Vec<BlockId> = pre.into_iter().map(block).collect();
    let post: Vec<BlockId> = post.into_iter().map(block).collect();
    // Between the loops: pure work and no phis. The outer header's own work is
    // pure too, and runs again ahead of the fused loop.
    let between = pre.iter().chain(&post).copied().chain([block(ho)]);
    for at in between {
        if at != block(ho) && !phis(function, at).is_empty() {
            return None;
        }
        if work(function, at).iter().any(|&inst| !pure(&function.instruction(inst).opcode)) {
            return None;
        }
    }
    if !phis(function, block(xi)).is_empty() {
        return None;
    }
    Some(Nest { outer: outer.clone(), inner: inner.clone(), pre, post })
}

/// Whether `value` is computed in the outer loop outside the inner one.
fn outer_only(
    function: &Function,
    nest: &Nest,
    value: ValueId,
) -> bool {
    defined(function, value)
        .and_then(|inst| function.parent(inst))
        .is_some_and(
            |block| {
                let at = cfg::id(block);
                nest.outer.body.contains(&at) && !nest.inner.body.contains(&at)
            },
        )
}

/// The inner loop counts the same in every outer iteration: its counters start
/// at, and its test reads, only values the nest does not change, and nothing
/// it computes is read outside it.
fn inner_is_invariant(
    function: &Function,
    nest: &Nest,
) -> bool {
    let hi = nest.inner_header();
    let entry = nest.entry();
    // The control cone: the test and the counters' starts and steps.
    let mut pending: Vec<Operand> = Vec::new();
    let branch = function.terminator(hi).expect("a terminated header");
    pending.push(function.instruction(branch).operands[0]);
    for phi in phis(function, hi) {
        for (value, from) in arms(function, phi) {
            if from == entry {
                if matches!(
                    value,
                    Operand::Value(one) if defined(function, one).is_some_and(|inst| function.parent(inst).is_some_and(|block| nest.outer.body.contains(&cfg::id(block))))
                ) {
                    return false;
                }
            } else {
                pending.push(value);
            }
        }
        pending.push(Operand::Value(function.instruction(phi).result.expect("a phi's value")));
    }
    let mut seen = IdSet::new();
    while let Some(operand) = pending.pop() {
        let Operand::Value(value) = operand else { continue };
        if !seen.insert(value) {
            continue;
        }
        if outer_only(function, nest, value) {
            return false;
        }
        let Some(inst) = defined(function, value) else { continue };
        let inside = function.parent(inst).is_some_and(|block| nest.inner.body.contains(&cfg::id(block)));
        if !inside {
            continue;
        }
        let op = function.instruction(inst);
        if op.opcode == Opcode::Phi {
            continue;
        }
        if !pure(&op.opcode) {
            return false;
        }
        pending.extend(op.operands.iter().copied());
    }
    // Nothing it computes is read outside it.
    for &at in &nest.inner.body {
        for &inst in function.block(cfg::block(at)).instructions() {
            if let Some(result) = function.instruction(inst).result {
                let read_outside = function.users(result).iter().any(|used| {
                    function.parent(used.user).is_some_and(|block| !nest.inner.body.contains(&cfg::id(block)))
                });
                if read_outside {
                    return false;
                }
            }
        }
    }
    true
}

/// The instruction that defines `value`, if one does.
fn defined(
    function: &Function,
    value: ValueId,
) -> Option<InstId> {
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// A number the signed integer `bits` of `width` bits is.
fn signed(
    bits: u128,
    width: u32,
) -> i64 {
    let width = width.min(64);
    let shift = 128 - width;
    (((bits << shift) as i128) >> shift) as i64
}

/// An address or an index as an affine function of the counters: bytes (or
/// units) `c + outer * i + inner * j + sum of invariant terms`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Lin {
    c: i64,
    outer: i64,
    inner: i64,
    symbols: BTreeMap<u32, i64>,
}

impl Lin {
    fn constant(c: i64) -> Self {
        Self { c, ..Self::default() }
    }

    fn is_constant(&self) -> bool {
        self.outer == 0 && self.inner == 0 && self.symbols.is_empty()
    }

    fn plus(
        &self,
        other: &Self,
        sign: i64,
    ) -> Self {
        let mut symbols = self.symbols.clone();
        for (&symbol, &factor) in &other.symbols {
            *symbols.entry(symbol).or_insert(0) += sign * factor;
        }
        symbols.retain(|_, factor| *factor != 0);
        Self {
            c: self.c + sign * other.c,
            outer: self.outer + sign * other.outer,
            inner: self.inner + sign * other.inner,
            symbols,
        }
    }

    fn scaled(
        &self,
        by: i64,
    ) -> Self {
        Self {
            c: self.c * by,
            outer: self.outer * by,
            inner: self.inner * by,
            symbols: self
                .symbols
                .iter()
                .map(|(&symbol, &factor)| (symbol, factor * by))
                .filter(|(_, factor)| *factor != 0)
                .collect(),
        }
    }
}

struct Addresses<'a> {
    unit: &'a memory::Unit<'a>,
    nest: &'a Nest,
    /// The counters of the two headers, as functions of the iteration number.
    counters: IdMap<ValueId, Lin>,
}

impl<'a> Addresses<'a> {
    fn new(
        unit: &'a memory::Unit<'a>,
        nest: &'a Nest,
    ) -> Self {
        let function = unit.function;
        let mut counters = IdMap::new();
        let mut me = Self { unit, nest, counters: IdMap::new() };
        for (header, latch, outer) in [
            (nest.outer_header(), cfg::block(*nest.outer.latches.first().expect("one")), true),
            (nest.inner_header(), nest.inner_latch(), false),
        ] {
            for phi in phis(function, header) {
                let result = function.instruction(phi).result.expect("a phi's value");
                let mut start = None;
                let mut next = None;
                for (value, from) in arms(function, phi) {
                    if from == latch {
                        next = Some(value);
                    } else {
                        start = Some(value);
                    }
                }
                let (Some(start), Some(next)) = (start, next) else { continue };
                let Some(start) = me.lin(start, 0) else { continue };
                let Some(step) = me.step(next, result) else { continue };
                let counter = if outer { Lin { outer: step, ..start } } else { Lin { inner: step, ..start } };
                counters.insert(result, counter);
            }
        }
        me.counters = counters;
        me
    }

    /// The constant `next` adds to `phi`.
    fn step(
        &self,
        next: Operand,
        phi: ValueId,
    ) -> Option<i64> {
        let (_, op) = self.unit.defining(next)?;
        let Opcode::Binary(kind @ (BinaryOp::Add | BinaryOp::Sub)) = op.opcode else { return None };
        let width = self.unit.int_bits(next)?;
        let constant = |operand: Operand| self.unit.int_constant(operand).map(|bits| signed(bits, width));
        match (op.operands[0] == Operand::Value(phi), op.operands[1] == Operand::Value(phi)) {
            (true, false) => constant(op.operands[1]).map(|step| if kind == BinaryOp::Add { step } else { -step }),
            (false, true) if kind == BinaryOp::Add => constant(op.operands[0]),
            _ => None,
        }
    }

    fn lin(
        &self,
        operand: Operand,
        depth: u32,
    ) -> Option<Lin> {
        if depth > 12 {
            return None;
        }
        let width = self.unit.int_bits(operand)?;
        if let Some(bits) = self.unit.int_constant(operand) {
            return Some(Lin::constant(signed(bits, width)));
        }
        let Operand::Value(value) = operand else { return None };
        if let Some(counter) = self.counters.get(&value) {
            return Some(counter.clone());
        }
        let function = self.unit.function;
        let Some(inst) = defined(function, value) else {
            return Some(Lin { symbols: BTreeMap::from([(value.0, 1)]), ..Lin::default() });
        };
        let at = cfg::id(function.parent(inst)?);
        if !self.nest.outer.body.contains(&at) {
            return Some(Lin { symbols: BTreeMap::from([(value.0, 1)]), ..Lin::default() });
        }
        let op = function.instruction(inst);
        let one = |i: usize| self.lin(op.operands[i], depth + 1);
        match &op.opcode {
            Opcode::Binary(BinaryOp::Add) => Some(one(0)?.plus(&one(1)?, 1)),
            Opcode::Binary(BinaryOp::Sub) => Some(one(0)?.plus(&one(1)?, -1)),
            Opcode::Binary(BinaryOp::Mul) => {
                let (left, right) = (one(0)?, one(1)?);
                if right.is_constant() {
                    Some(left.scaled(right.c))
                } else if left.is_constant() {
                    Some(right.scaled(left.c))
                } else {
                    None
                }
            }
            Opcode::Binary(BinaryOp::Shl) => {
                let (left, right) = (one(0)?, one(1)?);
                (right.is_constant() && (0..32).contains(&right.c)).then(|| left.scaled(1 << right.c))
            }
            Opcode::Cast(CastOp::SExt) => one(0),
            _ => None,
        }
    }

    /// `pointer` as an identified object and the bytes into it.
    fn address(
        &self,
        pointer: Operand,
    ) -> Option<(Operand, Lin)> {
        let function = self.unit.function;
        if let Some((_, op)) = self.unit.defining(pointer) {
            if let Opcode::GetElementPtr { source } = op.opcode {
                let [base, index] = op.operands[..] else { return None };
                let size = self.unit.layout.alloc_size(&self.unit.context.types, source) as i64;
                let (root, bytes) = self.address(base)?;
                return Some((root, bytes.plus(&self.lin(index, 0)?.scaled(size), 1)));
            }
        }
        if let Operand::Value(value) = pointer {
            if defined(function, value)
                .and_then(|inst| function.parent(inst))
                .is_some_and(|block| self.nest.outer.body.contains(&cfg::id(block)))
            {
                return None;
            }
        }
        memory::object_of(self.unit, pointer)?;
        Some((pointer, Lin::default()))
    }
}

/// The accesses of the inner loop, for the dependence tester; None where one
/// is not affine over an identified object, or the loop calls out.
fn accesses(
    unit: &memory::Unit,
    nest: &Nest,
) -> Option<Vec<Access>> {
    let function = unit.function;
    let addresses = Addresses::new(unit, nest);
    let mut roots: Vec<Operand> = Vec::new();
    let mut out = Vec::new();
    for &at in &nest.inner.body {
        for &inst in function.block(cfg::block(at)).instructions() {
            let op = function.instruction(inst);
            let (pointer, ty, write) = match op.opcode {
                Opcode::Load { volatile: false, .. } => (op.operands[0], op.ty, false),
                Opcode::Store { volatile: false, .. } => (op.operands[1], unit.operand_type(op.operands[0])?, true),
                Opcode::Load { .. }
                | Opcode::Store { .. }
                | Opcode::Call(_)
                | Opcode::Invoke(_)
                | Opcode::Alloca { .. } => {
                    return None;
                }
                _ => continue,
            };
            let (root, bytes) = addresses.address(pointer)?;
            let object = roots.iter().position(|one| *one == root).unwrap_or_else(|| {
                roots.push(root);
                roots.len() - 1
            }) as u64;
            out.push(Access {
                object,
                subscripts: vec![Subscript { constant: bytes.c, outer: bytes.outer, inner: bytes.inner }],
                symbols: bytes.symbols.into_iter().collect(),
                width: unit.layout.store_size(&unit.context.types, ty) as u32,
                write,
            });
        }
    }
    Some(out)
}

/// The first nest this jams, if any; whether there was one.
fn one(
    unit: &mut passes::Unit,
    analyses: &Analyses,
    params: &UnrollJam,
    refused: &mut BTreeSet<(i64, i64)>,
) -> Result<bool, String> {
    let graph = cfg::graph(unit.function);
    let shape = cfg::Shape::of(unit.function);
    let mut chosen = None;
    // Computed for the first nest there is: most functions have none.
    let mut facts = None;
    for inner in &shape.loops {
        if shape.loops.iter().any(|other| other.header != inner.header && inner.body.is_superset(&other.body)) {
            continue;
        }
        let Some(outer) = shape
            .loops
            .iter()
            .filter(|other| other.header != inner.header && other.body.is_superset(&inner.body))
            .min_by_key(|other| other.body.len())
        else {
            continue;
        };
        if refused.contains(&(outer.header, inner.header)) {
            continue;
        }
        let Some(nest) = nest_of(unit.function, &graph, outer, inner) else { continue };
        if !inner_is_invariant(unit.function, &nest) {
            refused.insert((outer.header, inner.header));
            continue;
        }
        let facts =
            facts.get_or_insert_with(|| analyses.fresh().get::<Registers>(unit.context, unit.layout, unit.function));
        let memory_unit = memory::Unit::within(unit.context, unit.layout, unit.function, analyses.outer())
            .with_registers(facts)
            .with_shape(&shape);
        let planned = (|| {
            let trips = induction::trip_count(&memory_unit, outer, facts)?.to_i64()?;
            let inner_trips = induction::trip_count(&memory_unit, inner, facts).and_then(|count| count.to_i64());
            let list = accesses(&memory_unit, &nest)?;
            if list.is_empty() {
                return None;
            }
            let plan = dependence::factor(&list, params.max_unroll, params.min_percent, inner_trips);
            (2..=plan.unroll).rev().find(|n| trips % i64::from(*n) == 0 && trips / i64::from(*n) >= 1)
        })();
        match planned {
            Some(factor) => {
                chosen = Some((nest, factor as usize));
                break;
            }
            None => {
                refused.insert((outer.header, inner.header));
            }
        }
    }
    let Some((nest, factor)) = chosen else { return Ok(false) };
    let key = (nest.outer.header, nest.inner.header);
    let applied = apply(unit.function, &nest, factor)?;
    if !applied {
        refused.insert(key);
    }
    Ok(applied)
}

/// `nest` with its outer loop unrolled by `factor` and the inner copies fused;
/// whether it could be.
fn apply(
    function: &mut Function,
    nest: &Nest,
    factor: usize,
) -> Result<bool, String> {
    let Some(copies) = chained(function, &nest.outer, factor - 1)? else { return Ok(false) };
    let (hi, li, ho) = (nest.inner_header(), nest.inner_latch(), nest.outer_header());
    let entry0 = nest.entry();
    let inner_first = |function: &Function, header: BlockId, inside: &dyn Fn(BlockId) -> bool| {
        function.successors(header).into_iter().find(|&to| inside(to)).expect("the body's first block")
    };
    let mut tail = li;
    let mut previous_post = nest.post.clone();
    for copy in &copies {
        let hi_k = copy[&cfg::id(hi)];
        let li_k = copy[&cfg::id(li)];
        let body_k: IdSet<BlockId> = nest.inner.body.iter().map(|at| copy[at]).collect();
        let entry_k = copy[&cfg::id(entry0)];
        let xi_k = copy[&cfg::id(nest.post[0])];
        // The work between the loops, ahead of the fused loop.
        let region: Vec<BlockId> = previous_post
            .iter()
            .copied()
            .chain([copy[&cfg::id(ho)]])
            .chain(nest.pre.iter().map(|block| copy[&cfg::id(*block)]))
            .collect();
        let before = function.terminator(entry0).expect("a terminated entry");
        for block in &region {
            for inst in work(function, *block) {
                function.move_to(inst, Position::Before(before))?;
            }
        }
        // The copy's counters are the first's: the same iteration space.
        let first_phis = phis(function, hi);
        let copy_phis = phis(function, hi_k);
        for (&mine, &theirs) in first_phis.iter().zip(&copy_phis) {
            let (kept, dropped) = (
                function.instruction(mine).result.expect("a phi's value"),
                function.instruction(theirs).result.expect("a phi's value"),
            );
            function.replace_all_uses_with(dropped, Operand::Value(kept));
            function.set_operands(theirs, Vec::new());
            function.erase(theirs)?;
        }
        // The copy's test is not made: its work runs ahead of the body.
        let first_k = inner_first(function, hi_k, &|to| body_k.contains(&to));
        let at = function
            .block(first_k)
            .instructions()
            .iter()
            .copied()
            .find(|&inst| function.instruction(inst).opcode != Opcode::Phi);
        for inst in work(function, hi_k) {
            match at {
                Some(at) => function.move_to(inst, Position::Before(at))?,
                None => function.move_to(inst, Position::End(first_k))?,
            }
        }
        for phi in phis(function, first_k) {
            let incoming = arms(function, phi)
                .into_iter()
                .map(|(value, from)| (value, if from == hi_k { tail } else { from }))
                .collect::<Vec<_>>();
            function.set_operands(phi, from_arms(&incoming));
        }
        // The tail runs on into the copy's body, whose own tail loops back.
        let jump = function.terminator(tail).expect("a terminated tail");
        crate::edges::retarget(function, jump, hi, first_k);
        let back = function.terminator(li_k).expect("a terminated latch");
        crate::edges::retarget(function, back, hi_k, hi);
        // The copy's loop is never entered: its entry goes straight out.
        let entry_jump = function.terminator(entry_k).expect("a terminated entry");
        crate::edges::retarget(function, entry_jump, hi_k, xi_k);
        let test = function.terminator(hi_k).expect("a terminated header");
        function.erase(test)?;
        function.erase_block(hi_k)?;
        tail = li_k;
        previous_post = nest.post.iter().map(|block| copy[&cfg::id(*block)]).collect();
    }
    // The fused loop's counters take the last body's back edge.
    for phi in phis(function, hi) {
        let incoming = arms(function, phi)
            .into_iter()
            .map(|(value, from)| (value, if from == li { tail } else { from }))
            .collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&incoming));
    }
    Ok(true)
}

#[cfg(test)]
#[path = "unroll_jam_tests.rs"]
mod tests;
