//! Port of `qbopt/optimize/transform.py`: MIR transforms, a body in, an
//! optimised body out.

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use crate::analysis::loops::{self as loopy, Loop};
use crate::analysis::occurrence::OpOccurrence;
use crate::analysis::ssa::{provider as _provider, substituted as _substituted};
use crate::model::mir::{self, Arg, Held, Kind, MirBlock, MirBody, Op, OrderedMap, Phi, Value};

// ==== BEGIN S0: transform.py 60-129 (primary) ====
/// Erase the operations whose address is in `gone`.
pub(crate) fn _absorb(ops: &[Op], gone: &BTreeSet<i64>) -> Vec<Op> {
    _without(ops, |one| gone.contains(&one.at))
}

/// Remove selected computation while retaining exact source ownership.
///
/// A deleted source occurrence becomes an inert marker owning the same
/// opaque ids; source-free operations disappear completely.
pub(crate) fn _without(ops: &[Op], drop: impl Fn(&Op) -> bool) -> Vec<Op> {
    let mut out = Vec::new();
    for op in ops {
        if !drop(op) {
            out.push(op.clone());
        } else if !op.absorbed.is_empty() || op.floating_origin.is_some() {
            out.push(_empty_operation(op));
        }
    }
    out
}

/// One phi taking the provider along any edge that named the load.
pub(crate) fn _phi_reading(phi: &Phi, swap: &BTreeMap<u32, Value>) -> Result<Phi, String> {
    if !phi.incoming.values().any(|one| swap.contains_key(&one.id)) {
        return Ok(phi.clone());
    }
    let incoming = phi
        .incoming
        .iter()
        .map(|(at, one)| _provider(*one, swap).map(|value| (*at, value)))
        .collect::<Result<OrderedMap<_, _>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(Phi { result: phi.result, incoming })
}

// CSE only ever removes an operation whose whole answer is in its operands.
pub(crate) const _PURE: [Kind; 30] = [
    Kind::Add,
    Kind::Sub,
    Kind::Mul,
    Kind::FixedMul,
    Kind::Smulhi,
    Kind::PtrOffset,
    Kind::Div,
    Kind::Rem,
    Kind::And,
    Kind::Or,
    Kind::Xor,
    Kind::Shl,
    Kind::Shr,
    Kind::Sar,
    Kind::Neg,
    Kind::Not,
    Kind::Convert,
    Kind::SignExtend,
    Kind::Extract,
    Kind::Concat,
    Kind::Copy,
    Kind::Address,
    Kind::Lt,
    Kind::Le,
    Kind::Gt,
    Kind::Ge,
    Kind::Eq,
    Kind::Ne,
    Kind::Below,
    Kind::Above,
];

// ==== END S0 ====

// ==== BEGIN A: transform.py 130-784 (agent A) ====

pub(crate) fn _unchanged_float_environment(op: &Op) -> bool {
    use crate::model::floating::Exceptions;

    if op.barrier() || matches!(op.kind, Kind::Call | Kind::Opaque | Kind::Fcheck) {
        return false;
    }
    match &op.floating {
        None => op.stack.is_none(),
        Some(floating) => floating.exceptions == Exceptions::Deferred,
    }
}

// ==== END A ====

/// Keep opaque source ownership, but no computation or memory effect.
pub(crate) fn _empty_operation(op: &crate::model::mir::Op) -> crate::model::mir::Op {
    use crate::model::mir::{Kind, OpCode, OrderedMap};
    let mut result = op.clone();
    result.op = Some(OpCode::nothing());
    result.name = String::new();
    result.kind = Kind::Nothing;
    result.defines = Vec::new();
    result.uses = Vec::new();
    result.array = None;
    result.memory_values = Vec::new();
    result.floating = None;
    result.floating_origin = None;
    result.args = Vec::new();
    result.results = Vec::new();
    result.loads = Vec::new();
    result.stores = Vec::new();
    result.merges = OrderedMap::new();
    result.source_backed = false;
    result.raised = None;
    result.target = None;
    result.cases = Vec::new();
    result.symbol = Some(false);
    result.args_known = true;
    result.memory_complete = true;
    result.reads_complete = true;
    result.opaque_defs = Some(std::collections::BTreeSet::new());
    result.opaque_uses = Some(std::collections::BTreeSet::new());
    result.stack = None;
    result.test = None;
    result.indirect = false;
    result
}

// ==== BEGIN B: transform.py 819-1004 (agent B) ====
// ==== END B ====

/// Direct port of `qbopt.optimize.transform:_preheader`.
///
/// The one block entering `loop_` from outside it, if exactly one source
/// block occurrence does.  This deliberately walks `body.blocks` rather
/// than a predecessor map: Python preserves both source order and duplicate
/// block occurrences in the list it counts.
pub(crate) fn _preheader(body: &MirBody, loop_: &Loop) -> Option<i64> {
    let outside = body
        .blocks
        .iter()
        .filter(|block| block.succ.contains(&loop_.header) && !loop_.body.contains(&block.at))
        .map(|block| block.at)
        .collect::<Vec<_>>();
    if outside.len() == 1 {
        Some(outside[0])
    } else {
        None
    }
}

// ==== BEGIN C1: transform.py 1018-1396 (agent C) ====
// ==== END C1 ====

/// Direct port of `qbopt/optimize/transform.py:_OBSERVED`.
pub(crate) const _OBSERVED: [crate::model::mir::Kind; 20] = {
    use crate::model::mir::Kind;
    [
        Kind::Call,
        Kind::Return,
        Kind::Jump,
        Kind::Branch,
        Kind::Switch,
        Kind::Escape,
        Kind::Arg,
        Kind::Result,
        Kind::Opaque,
        Kind::Fload,
        Kind::Fstore,
        Kind::Fadd,
        Kind::Fsub,
        Kind::Fmul,
        Kind::Fdiv,
        Kind::Fneg,
        Kind::Fabs,
        Kind::Fsqrt,
        Kind::Fcompare,
        Kind::Fcheck,
    ]
};

/// Direct port of `qbopt/optimize/transform.py:_leaving`.
pub(crate) fn _leaving(body: &MirBody) -> std::collections::BTreeSet<crate::model::mir::Value> {
    crate::model::mir::exposed(body)
}

/// A value is a 32-bit register and this machine's code is 16-bit.
pub(crate) const LOW: u8 = 0;
pub(crate) const HIGH: u8 = 1;

// ==== BEGIN S1: transform.py 1433-1447 (primary) ====
type _HalvesReuse = std::collections::HashMap<usize, (MirBody, BTreeSet<(Value, u8)>)>;

thread_local! {
    /// Python's `_halves_reuse` context variable.  The saved body is compared
    /// as well as its address: Python holds the body alive, Rust cannot.
    #[allow(non_upper_case_globals)]
    static _halves_reuse: std::cell::RefCell<Option<_HalvesReuse>> = const { std::cell::RefCell::new(None) };
}

/// Share half-liveness for immutable states in one transaction.
pub(crate) fn _reusing_halves<T>(inside: impl FnOnce() -> T) -> T {
    let token = _halves_reuse.with(|reuse| reuse.replace(Some(std::collections::HashMap::new())));
    let result = inside();
    _halves_reuse.with(|reuse| *reuse.borrow_mut() = token);
    result
}
// ==== END S1 ====

/// Which half of which value something reads, to a fixed point.
///
/// Both halves of everything reaching an exit are live.
pub(crate) fn halves(body: &MirBody) -> BTreeSet<(Value, u8)> {
    let key = std::ptr::from_ref(body) as usize;
    let saved = _halves_reuse.with(|reuse| {
        reuse
            .borrow()
            .as_ref()
            .and_then(|reused| reused.get(&key))
            .filter(|saved| saved.0 == *body)
            .map(|saved| saved.1.clone())
    });
    if let Some(saved) = saved {
        return saved;
    }

    let mut out: BTreeSet<(Value, u8)> = BTreeSet::new();
    for value in _leaving(body) {
        out.insert((value, LOW));
        out.insert((value, HIGH));
    }

    let widths = |op: &Op| -> BTreeMap<Value, u32> {
        let mut found: BTreeMap<Value, u32> = BTreeMap::new();
        for one in &op.args {
            if let Arg::Held(held) = one {
                let widest = (*found.get(&held.value).unwrap_or(&0)).max(held.width);
                found.insert(held.value, widest);
            }
        }
        for reference in op.loads.iter().chain(&op.stores) {
            if let Some(base) = reference.base {
                let widest = (*found.get(&base).unwrap_or(&0)).max(reference.base_width);
                found.insert(base, widest);
            }
        }
        found
    };

    let mut changing = true;
    while changing {
        let before = out.len();
        for block in &body.blocks {
            for op in &block.ops {
                if !_kept(op)
                    && !op
                        .defines
                        .iter()
                        .any(|one| [LOW, HIGH].iter().any(|half| out.contains(&(*one, *half))))
                {
                    continue;
                }
                let carried = &op.merges;
                let read = widths(op);
                let described = op.kind != Kind::Opaque && !op.barrier();
                for one in &op.uses {
                    if let Some(into) = carried.get(one) {
                        if out.contains(&(*into, HIGH)) {
                            out.insert((*one, HIGH));
                        }
                        if !read.contains_key(one) {
                            continue;
                        }
                    }
                    if !described || !read.contains_key(one) {
                        out.insert((*one, LOW));
                        out.insert((*one, HIGH));
                        continue;
                    }
                    out.insert((*one, LOW));
                    if read[one] >= 4 {
                        out.insert((*one, HIGH));
                    }
                }
                for reference in op.loads.iter().chain(&op.stores) {
                    for one in [reference.base, reference.segment].into_iter().flatten() {
                        out.insert((one, LOW));
                        if Some(one) == reference.segment || reference.base_width >= 4 {
                            out.insert((one, HIGH));
                        }
                    }
                }
            }
            for phi in &block.phis {
                for half in [LOW, HIGH] {
                    if out.contains(&(phi.result, half)) {
                        out.extend(phi.incoming.values().map(|one| (*one, half)));
                    }
                }
            }
        }
        changing = out.len() != before;
    }
    _halves_reuse.with(|reuse| {
        if let Some(reused) = reuse.borrow_mut().as_mut() {
            reused.insert(key, (body.clone(), out.clone()));
        }
    });
    out
}

/// Values some half of which something reads.
///
/// Direct port of `qbopt/optimize/transform.py:live`.
pub(crate) fn live(body: &MirBody) -> std::collections::BTreeSet<crate::model::mir::Value> {
    halves(body).into_iter().map(|(one, _)| one).collect()
}

// ==== BEGIN D: transform.py 1551-2090 (agent D) ====
/// Whether each branch test is taken, given (a, b, unsigned view).
#[allow(clippy::type_complexity)]
pub(crate) const _TAKEN: [(
    crate::model::mir::Kind,
    fn(&num_bigint::BigInt, &num_bigint::BigInt, &dyn Fn(&num_bigint::BigInt) -> num_bigint::BigInt) -> bool,
); 10] = {
    use crate::model::mir::Kind;
    [
        (Kind::Eq, |a, b, _| a == b),
        (Kind::Ne, |a, b, _| a != b),
        (Kind::Lt, |a, b, _| a < b),
        (Kind::Le, |a, b, _| a <= b),
        (Kind::Gt, |a, b, _| a > b),
        (Kind::Ge, |a, b, _| a >= b),
        (Kind::Below, |a, b, u| u(a) < u(b)),
        (Kind::BelowEq, |a, b, u| u(a) <= u(b)),
        (Kind::Above, |a, b, u| u(a) > u(b)),
        (Kind::AboveEq, |a, b, u| u(a) >= u(b)),
    ]
};

/// The modeled comparison supplying this branch's condition value.
pub(crate) fn _comparison<'a>(
    block: &'a crate::model::mir::MirBlock,
    op: &crate::model::mir::Op,
) -> Option<(usize, &'a crate::model::mir::Op)> {
    use crate::model::mir::{Arg, Kind};
    if op.kind != Kind::Branch || !op.test.is_some_and(|test| _TAKEN.iter().any(|(kind, _)| *kind == test)) {
        return None;
    }
    let reads = op.uses.iter().filter(|one| one.flags).collect::<Vec<_>>();
    if reads.len() != 1 {
        return None;
    }

    // The comparison this branch reads, which must be the last thing to
    // write the flags before it -- SSA says so by naming the value.
    let (index, compare) = block.ops.iter().enumerate().find(|(_, one)| one.defines.contains(reads[0]))?;
    if compare.kind == Kind::Sub && compare.args.len() == 2 && compare.results.is_empty() {
        return Some((index, compare));
    }
    if matches!(compare.kind, Kind::And | Kind::Or | Kind::Xor)
        && matches!(op.test, Some(Kind::Eq | Kind::Ne))
        && !compare.barrier()
        && compare.args.len() == 2
        && compare.results.len() == 1
    {
        if let Arg::Held(result) = &compare.results[0] {
            if [2, 4, 8].contains(&result.width)
                && compare.args.iter().all(|arg| match arg {
                    Arg::Held(held) => held.width == result.width,
                    Arg::Const(constant) => constant.width == result.width,
                    _ => false,
                })
            {
                return Some((index, compare));
            }
        }
    }
    None
}

/// Dead blocks retain byte ownership, but no instructions or outgoing edges.
pub(crate) fn _unreachable(body: &MirBody) -> MirBody {
    use std::collections::{BTreeMap, BTreeSet};
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let (mut reached, mut pending) = (BTreeSet::new(), vec![body.entry]);
    while let Some(at) = pending.pop() {
        if reached.contains(&at) || !blocks.contains_key(&at) {
            continue;
        }
        reached.insert(at);
        pending.extend(blocks[&at].succ.iter().copied());
    }
    let kept = body
        .blocks
        .iter()
        .filter(|block| reached.contains(&block.at) || !block.ops.is_empty())
        .map(|block| {
            if reached.contains(&block.at) {
                block.clone()
            } else {
                crate::model::mir::MirBlock {
                    succ: Vec::new(),
                    phis: Vec::new(),
                    ops: block.ops.iter().map(_empty_operation).collect(),
                    ..block.clone()
                }
            }
        })
        .collect();
    MirBody { blocks: kept, ..body.clone() }
}

/// Resolve single-valued joins after an edge disappears, without discarding
/// byte ownership.  Python's `ValueError` is the `Err` text.
pub(crate) fn _trivial_phis(body: &MirBody) -> Result<MirBody, String> {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::analysis::{loops as loopy, ssa};
    use crate::model::mir::{MirBlock, Phi};
    let mut body = body.clone();
    let predecessors = loopy::predecessors(&body.blocks);
    let mut swaps = BTreeMap::new();
    loop {
        let mut changed = false;
        let mut out = Vec::new();
        for block in &body.blocks {
            let mut phis = Vec::new();
            for phi in &block.phis {
                let incoming = phi
                    .incoming
                    .iter()
                    .filter(|(at, _)| predecessors[&block.at].contains(at))
                    .map(|(&at, &value)| ssa::provider(value, &swaps).map(|one| (at, one)))
                    .collect::<Result<crate::model::mir::OrderedMap<_, _>, _>>()
                    .map_err(|error| error.to_string())?;
                let mut values = incoming.values().copied().collect::<BTreeSet<_>>();
                values.remove(&phi.result);
                if values.len() == 1 {
                    swaps.insert(phi.result.id, *values.iter().next().expect("one"));
                    changed = true;
                } else {
                    phis.push(Phi { result: phi.result, incoming });
                }
            }
            let ops = block
                .ops
                .iter()
                .map(|op| ssa::substituted(op, &swaps))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            out.push(MirBlock { phis, ops, ..block.clone() });
        }
        body = MirBody { blocks: out, ..body };
        if !changed {
            return Ok(body);
        }
    }
}

/// Whether this operation stays whatever the liveness says.
///
/// Direct port of `qbopt/optimize/transform.py:_kept`.
pub(crate) fn _kept(op: &crate::model::mir::Op) -> bool {
    use crate::model::mir::Kind;
    if _OBSERVED.contains(&op.kind) || !op.stores.is_empty() || op.barrier() {
        return true;
    }
    if op.kind == Kind::Opaque {
        return true;
    }
    if op.kind == Kind::Sub && op.results.is_empty() && !op.defines.is_empty() {
        return false;
    }
    op.defines.iter().all(|one| one.flags)
}
// ==== END D ====

// ==== BEGIN E: transform.py 2091-2563 (agent E) ====
// ==== END E ====

// ==== BEGIN C2: transform.py 2564-2869 (agent C) ====
// ==== END C2 ====

// ==== BEGIN F: transform.py 2870-3297 (primary) ====
// Every transform, as the one thing a transform is. The functions above stay
// because they are what each class does and are what the tests name; what
// changes is that the pipeline can only reach them through `transform`.
pub(crate) struct Fold {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Fold {
    fn class_name(&self) -> &'static str {
        "Fold"
    }

    fn name(&self) -> &str {
        "fold"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        folded(&body, &self.r#where.dgroup, &self.r#where.named())
    }
}

pub(crate) struct Decide {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Decide {
    fn class_name(&self) -> &'static str {
        "Decide"
    }

    fn name(&self) -> &str {
        "decide"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        crate::optimize::cfg::merged(&decided(&body, &self.r#where.dgroup, &self.r#where.named())?)
            .map_err(|error| error.to_string())
    }
}

pub(crate) struct Dead;

impl crate::model::passes::MIRTransform for Dead {
    fn class_name(&self) -> &'static str {
        "Dead"
    }

    fn name(&self) -> &str {
        "dead"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        dead(&body)
    }
}

/// Specialize a proven strict-FP recurrence before LICM changes its shape.
pub(crate) struct FloatLoop {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for FloatLoop {
    fn class_name(&self) -> &'static str {
        "FloatLoop"
    }

    fn name(&self) -> &str {
        "floatloop"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        crate::optimize::floatloop::specialized(&body, &self.r#where.dgroup, &self.r#where.named())
    }
}

pub(crate) struct Hoist {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Hoist {
    fn class_name(&self) -> &'static str {
        "Hoist"
    }

    fn name(&self) -> &str {
        "hoist"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        let body = hoisted(&body, &self.r#where.dgroup, &self.r#where.named(), self.r#where.bounds.as_ref())?;
        crate::optimize::loopmotion::sunk_stores(
            &body,
            &self.r#where.dgroup,
            self.r#where.bounds.as_ref(),
            _handles_errors(&self.r#where),
        )
    }
}

pub(crate) struct DropStores {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for DropStores {
    fn class_name(&self) -> &'static str {
        "DropStores"
    }

    fn name(&self) -> &str {
        "drop_stores"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        let private =
            crate::analysis::observers::private(&body, self.r#where.found.as_ref(), self.r#where.blocks.as_ref())?;
        without_dead_stores(
            &body,
            &self.r#where.dgroup,
            &self.r#where.named(),
            private.as_deref(),
            self.r#where.bounds.as_ref(),
            _handles_errors(&self.r#where),
        )
    }
}

pub(crate) fn _handles_errors(r#where: &crate::model::passes::Where) -> bool {
    use crate::abi::runtime;

    let contracts = r#where.named().values().map(|name| runtime::contract(Some(name))).collect::<Vec<_>>();
    runtime::handles_errors(&contracts)
}

/// The single value-reuse pass: scalar GVN and memory-aware PRE.
pub(crate) struct Gvn {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Gvn {
    fn class_name(&self) -> &'static str {
        "Gvn"
    }

    fn name(&self) -> &str {
        "gvn"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        crate::optimize::gvn::optimized(&body, &self.r#where)
    }
}

pub(crate) struct Place {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Place {
    fn class_name(&self) -> &'static str {
        "Place"
    }

    fn name(&self) -> &str {
        "place"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        placed(&body, &self.r#where.dgroup, &self.r#where.named())
    }
}

pub(crate) struct Algebraic;

impl crate::model::passes::MIRTransform for Algebraic {
    fn class_name(&self) -> &'static str {
        "Algebraic"
    }

    fn name(&self) -> &str {
        "algebraic"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        let demanded = halves(&body);
        crate::optimize::algebraic::simplified(
            &body,
            &demanded.iter().map(|(value, _)| *value).collect(),
            &demanded.iter().filter(|(_, part)| *part == HIGH).map(|(value, _)| *value).collect(),
        )
    }
}

/// Expose packed far addresses as independently allocatable MIR values.
pub(crate) struct SplitPointers;

impl crate::model::passes::MIRTransform for SplitPointers {
    fn class_name(&self) -> &'static str {
        "SplitPointers"
    }

    fn name(&self) -> &str {
        "split_pointers"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        Ok(crate::optimize::pointeraccess::split(body))
    }
}

/// Canonicalize indirect objects from current SSA pointer facts.
pub(crate) struct PointerProvenance;

impl crate::model::passes::MIRTransform for PointerProvenance {
    fn class_name(&self) -> &'static str {
        "PointerProvenance"
    }

    fn name(&self) -> &str {
        "provenance"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        crate::analysis::alias::annotated(&body)
    }
}

/// The passes, in order, that `wanted` leaves on.
///
/// A pass that is off is not in it, rather than in it and skipped, so what
/// runs is what this returns.
pub(crate) fn pipeline(
    r#where: &crate::model::passes::Where,
    wanted: &IndexMap<&str, bool>,
) -> Vec<Box<dyn crate::model::passes::MIRTransform>> {
    use crate::optimize::{fill, lcssa, loopsimplify, peel, promote, strength, unroll};

    let every: Vec<Box<dyn crate::model::passes::MIRTransform>> = vec![
        // Pointer identity is a solved program fact: resolve it before a
        // packed pointer becomes independent offset and selector values.
        Box::new(PointerProvenance),
        Box::new(SplitPointers),
        // Aggregate/object leaves become ordinary SSA before any scalar or
        // CFG pass asks what is constant, redundant, or loop invariant.
        Box::new(promote::Sroa::new(r#where.clone())),
        Box::new(Fold { r#where: r#where.clone() }),
        Box::new(Decide { r#where: r#where.clone() }),
        Box::new(loopsimplify::LoopSimplify),
        Box::new(lcssa::LoopClosedSSA),
        // Strict floating recurrences must retain their original iteration
        // order; LICM may move invariant x87 preparation out afterwards.
        Box::new(FloatLoop { r#where: r#where.clone() }),
        Box::new(Hoist { r#where: r#where.clone() }),
        Box::new(DropStores { r#where: r#where.clone() }),
        Box::new(Gvn { r#where: r#where.clone() }),
        // Ordinary scalar write-through promotion remains after memory GVN.
        Box::new(promote::Promote::new(r#where.clone())),
        Box::new(strength::Strength::new(r#where.clone())),
        Box::new(Algebraic),
        Box::new(Dead),
        Box::new(Place { r#where: r#where.clone() }),
        Box::new(unroll::Unroll::new(r#where.clone())),
        Box::new(peel::Peel::new(r#where.clone())),
        Box::new(fill::Fill),
    ];
    every.into_iter().filter(|one| wanted.get(one.name()).copied().unwrap_or(true)).collect()
}

// The order, from the pipeline itself rather than beside it.
pub(crate) static PASSES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
    pipeline(&crate::model::passes::Where::default(), &IndexMap::new())
        .iter()
        .map(|one| one.name().to_owned())
        .collect()
});
// What runs with no caller asking otherwise: a pass may exist, be correct,
// and be off because it costs.
#[allow(dead_code)]
pub(crate) static PASSES_ON: std::sync::LazyLock<Vec<String>> =
    std::sync::LazyLock::new(|| PASSES.iter().filter(|one| *one != "strength").cloned().collect());

/// `applied`'s keyword arguments, with Python's defaults.
pub(crate) struct Applied<'a> {
    pub blocks: Option<Vec<std::sync::Arc<dyn std::any::Any + Send + Sync>>>,
    pub found: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    pub fold: bool,
    pub lcssa_: bool,
    pub decide: bool,
    pub dead: bool,
    pub hoist: bool,
    pub forward: bool,
    pub drop_loads: bool,
    pub drop_stores: bool,
    pub promote_: bool,
    pub strength_: bool,
    pub floatloop_: bool,
    pub unroll_: bool,
    pub peel_: bool,
    pub fill_: bool,
    pub unswitch_: bool,
    pub only: Option<String>,
    pub registers: Option<i64>,
    pub call_registers: i64,
    pub index_scales: Option<BTreeSet<i64>>,
    pub address_forms: Option<Vec<crate::model::passes::AddressForm>>,
    pub costs: Option<crate::model::passes::OperationCosts>,
    pub max_unroll_iterations: i64,
    pub max_unrolled_operations: i64,
    pub watch: Option<&'a mut dyn FnMut(&str, &MirBody)>,
}

impl Default for Applied<'_> {
    fn default() -> Self {
        Self {
            blocks: None,
            found: None,
            fold: true,
            lcssa_: true,
            decide: true,
            dead: true,
            hoist: true,
            forward: true,
            drop_loads: true,
            drop_stores: true,
            promote_: true,
            strength_: true,
            floatloop_: true,
            unroll_: true,
            peel_: true,
            fill_: true,
            unswitch_: false,
            only: None,
            registers: None,
            call_registers: 0,
            index_scales: None,
            address_forms: None,
            costs: None,
            max_unroll_iterations: crate::model::passes::DEFAULT_MAX_UNROLL_ITERATIONS,
            max_unrolled_operations: crate::model::passes::DEFAULT_MAX_UNROLLED_OPERATIONS,
            watch: None,
        }
    }
}

/// Every transform this module has, or the one `only` names.
pub(crate) fn applied(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    options: Applied<'_>,
) -> Result<MirBody, String> {
    crate::analysis::consts::reusing(|| _reusing_halves(|| _applied(body, dgroup, calls, options)))
}

/// The closure state `applied`'s nested `scalarized`, `fixed` and
/// `structural_candidate` share; they recurse through unroll and peel.
struct _Transaction<'w, 'a> {
    r#where: &'w crate::model::passes::Where,
    boundary: std::cell::RefCell<Vec<Box<dyn crate::model::passes::MIRTransform>>>,
    passes: std::cell::RefCell<Vec<Box<dyn crate::model::passes::MIRTransform>>>,
    unrollers: std::cell::RefCell<Vec<Box<dyn crate::model::passes::MIRTransform>>>,
    only: bool,
    watch: std::cell::RefCell<Option<&'a mut dyn FnMut(&str, &MirBody)>>,
}

impl _Transaction<'_, '_> {
    fn watching(&self) -> bool {
        self.watch.borrow().is_some()
    }

    fn watch(&self, stage: &str, state: &MirBody) {
        if let Some(watch) = self.watch.borrow_mut().as_mut() {
            watch(stage, state);
        }
    }

    fn scalarized(&self, state: MirBody, stage: &str) -> Result<MirBody, String> {
        let mut state = state;
        let mut boundary = self.boundary.borrow_mut();
        for one in boundary.iter_mut() {
            state = one.transform(state)?;
            self.watch(&format!("{stage}-{}", one.name()), &state);
        }
        Ok(state)
    }

    fn fixed(&self, state: MirBody, consider_unroll: bool, prefix: &str) -> Result<MirBody, String> {
        // A monotone chain may expose one simplification per operation.
        // Scale with the body and separately reject a repeated state, so an
        // oscillator fails immediately instead of consuming that allowance.
        let size = state.blocks.iter().map(|block| 1 + block.phis.len() + block.ops.len()).sum::<usize>();
        let limit = std::cmp::max(16, size + 1);
        let mut state = state;
        let mut history = vec![state.clone()];
        for iteration in 0..limit {
            let before = state.clone();
            {
                let mut passes = self.passes.borrow_mut();
                for one in passes.iter_mut() {
                    state = one.transform(state)?;
                    self.watch(&format!("{prefix}r{:02}-{}", iteration + 1, one.name()), &state);
                }
            }
            // Ask at the original pipeline boundary; accepting the candidate
            // still requires a separately converged result.
            if consider_unroll && !self.unrollers.borrow().is_empty() {
                let stage = format!("{prefix}candidate-unroll");
                let inner = format!("{prefix}candidate-unroll-");
                let mut optimize = |candidate: MirBody| self.structural_candidate(candidate, &stage, &inner, false);
                let mut watch = |stage: &str, candidate: &MirBody| self.watch(&format!("{prefix}{stage}"), candidate);
                let watching = self.watching();
                state = crate::optimize::unroll::optimized(
                    &state,
                    self.r#where,
                    &mut optimize,
                    if watching { Some(&mut watch) } else { None },
                )?;
            }
            if self.only || state == before {
                // A structural candidate can make its last cloned region
                // unreachable on the same round that reaches the scalar fixed
                // point, so normalize the public boundary itself.
                return Ok(_unreachable(&state));
            }
            if history.iter().any(|previous| state == *previous) {
                return Err(format!("MIR optimization did not converge: cycle after {} rounds", iteration + 1));
            }
            history.push(state.clone());
        }
        Err(format!("MIR optimization did not converge after {limit} size-scaled rounds"))
    }

    /// Normalize addresses and newly exact leaves before pricing a CFG clone.
    fn structural_candidate(
        &self,
        candidate: MirBody,
        stage: &str,
        prefix: &str,
        consider_unroll: bool,
    ) -> Result<MirBody, String> {
        let state = self.fixed(self.scalarized(candidate, stage)?, consider_unroll, prefix)?;
        let scalar = self.scalarized(state.clone(), &format!("{stage}-settled"))?;
        if scalar == state {
            return Ok(state);
        }
        // The unscalarized side already pays for each explicit aggregate
        // load/store; once SROA removes those homes, every retained SSA leaf
        // has to fit the finite register file or be recreated in a spill slot.
        let before = crate::optimize::profit::weighted(&state, &self.r#where.costs, None);
        let settled = self.fixed(scalar, false, &format!("{prefix}settled-"))?;
        let after = crate::optimize::profit::pressure_adjusted(&settled, &self.r#where.costs, self.r#where.registers, None);
        if let (Some(before), Some(after)) = (before, after) {
            if after > before {
                self.watch(&format!("{stage}-settled-rejected-pressure"), &settled);
                return Ok(state);
            }
        }
        Ok(settled)
    }
}

fn _applied(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    options: Applied<'_>,
) -> Result<MirBody, String> {
    let Applied {
        blocks,
        found,
        fold,
        lcssa_,
        decide,
        dead,
        hoist,
        forward,
        drop_loads,
        drop_stores,
        promote_,
        strength_,
        floatloop_,
        unroll_,
        peel_,
        fill_,
        unswitch_,
        only,
        registers,
        call_registers,
        index_scales,
        address_forms,
        costs,
        max_unroll_iterations,
        max_unrolled_operations,
        watch,
    } = options;
    // Every pass can be turned off, which is how a miscompile is bisected.
    let wanted = IndexMap::from([
        ("lcssa", lcssa_),
        ("floatloop", floatloop_),
        ("fold", fold),
        ("decide", decide),
        ("dead", dead),
        ("hoist", hoist),
        ("gvn", forward && drop_loads),
        ("drop_stores", drop_stores),
        ("sroa", promote_),
        ("promote", promote_),
        ("strength", strength_),
        ("unroll", unroll_),
        ("peel", peel_),
        ("fill", fill_),
    ]);
    if found.is_some() {
        return Err("not yet ported: qbopt.objectfile.module.landmarks".to_owned());
    }
    let r#where = crate::model::passes::Where {
        dgroup: dgroup.clone(),
        calls: Some(calls.clone()),
        bounds: None,
        blocks,
        found,
        registers: registers.unwrap_or(mir::TRACKED.len() as i64),
        call_registers,
        // Existing direct MIR callers retain native medium-model addressing.
        index_scales: index_scales.unwrap_or_else(|| BTreeSet::from([1])),
        address_forms: address_forms.unwrap_or_default(),
        costs: costs.unwrap_or_default(),
        max_unroll_iterations,
        max_unrolled_operations,
    };
    // Public debugging selectors from before value reuse became one pass.
    let only = match only.as_deref() {
        Some("forward" | "drop_loads" | "reuse" | "cse") => Some("gvn".to_owned()),
        _ => only,
    };
    let passes = pipeline(&r#where, &wanted)
        .into_iter()
        .filter(|one| only.as_deref().is_none_or(|only| one.name() == only))
        .collect::<Vec<_>>();
    // Pointer decomposition and SROA establish the scalar memory shape at
    // structural boundaries; they are not members of the scalar fixed point.
    let structural = ["PointerProvenance", "SplitPointers", "Sroa"];
    let (boundary, passes): (Vec<_>, Vec<_>) =
        passes.into_iter().partition(|one| structural.contains(&one.class_name()));
    let (mut unrollers, passes): (Vec<_>, Vec<_>) =
        passes.into_iter().partition(|one| one.class_name() == "Unroll");
    let (mut peelers, passes): (Vec<_>, Vec<_>) = passes.into_iter().partition(|one| one.class_name() == "Peel");

    let has_unrollers = !unrollers.is_empty();
    let has_boundary = !boundary.is_empty();
    let transaction = _Transaction {
        r#where: &r#where,
        boundary: std::cell::RefCell::new(boundary),
        passes: std::cell::RefCell::new(passes),
        unrollers: std::cell::RefCell::new(Vec::new()),
        only: only.is_some(),
        watch: std::cell::RefCell::new(watch),
    };

    let mut body = transaction.scalarized(body.clone(), "r01")?;
    if only.is_some() && has_boundary {
        return Ok(_unreachable(&body));
    }
    if only.is_some() && has_unrollers {
        body = unrollers[0].transform(body)?;
        transaction.watch("r01-unroll", &body);
        return Ok(_unreachable(&body));
    }
    if only.is_some() && !peelers.is_empty() {
        body = peelers[0].transform(body)?;
        transaction.watch("r01-peel", &body);
        return Ok(_unreachable(&body));
    }
    *transaction.unrollers.borrow_mut() = std::mem::take(&mut unrollers);

    body = transaction.fixed(body, has_unrollers, "")?;
    if !peelers.is_empty() {
        let mut optimize =
            |candidate: MirBody| transaction.structural_candidate(candidate, "candidate-peel", "candidate-peel-", has_unrollers);
        let mut watch = |stage: &str, candidate: &MirBody| transaction.watch(stage, candidate);
        let watching = transaction.watching();
        body = crate::optimize::peel::optimized(
            &body,
            &r#where,
            &mut optimize,
            if watching { Some(&mut watch) } else { None },
        )?;
    }
    if unswitch_ {
        let mut watch = |stage: &str, candidate: &MirBody| transaction.watch(stage, candidate);
        let watching = transaction.watching();
        body = crate::optimize::unswitch::optimized(
            &body,
            dgroup,
            calls,
            crate::optimize::unswitch::Optimized {
                registers: Some(r#where.registers),
                call_registers: r#where.call_registers,
                index_scales: Some(r#where.index_scales.clone()),
                address_forms: Some(r#where.address_forms.clone()),
                costs: Some(r#where.costs.clone()),
                max_unroll_iterations: r#where.max_unroll_iterations,
                max_unrolled_operations: r#where.max_unrolled_operations,
                watch: if watching { Some(&mut watch) } else { None },
            },
        )?;
    }
    Ok(_unreachable(&body))
}
// ==== END F ====

#[cfg(test)]
#[path = "transform_tests.rs"]
mod transform_tests;
