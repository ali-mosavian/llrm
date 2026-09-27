//! A counted loop that stores one byte value into consecutive bytes is one
//! `llvm.memset`: LLVM's LoopIdiomRecognize. Adapted from llrm-core's
//! `optimize/fill.rs`, the port of `qbopt/optimize/fill.py`.
//!
//! The loop keeps one trip, guarded by its header's test, which fills what
//! every trip stored; its counters leave with their exit values.
//!
//! What changed with the IR:
//! - The fill is `llvm.memset`, of bytes and a byte count: a wider store
//!   fills only where each of its bytes is one number. A word fill of any
//!   other value, `rep stosw`, is isel's shape; the rich MIR has none.
//! - Where each trip stores is induction's `derived` address, a GEP off an
//!   invariant pointer; its bytes per trip must be the element's. The old
//!   `_offset` walked adds of a register base, and `_stepping` compared the
//!   counter's step itself.
//! - The fill's address is the store's own pointer, which the one trip
//!   left computes from the counters' starts. The old one rebuilt it from
//!   the cell's base, displacement, segment and storage class.
//! - Its count in bytes must not wrap the index: a byte's trips never do,
//!   and wider cells need `inbounds` GEPs or the proof's `maximum`. The
//!   counter must be the index's width.
//! - The memset is declared where the module has none, through the pass
//!   manager's `Declared`.
//! - `_pure` is `memory::only_value` less loads and allocas, and less
//!   divisions, which trap.
//!
//! llrm-mir has no idiom pass.

use std::collections::BTreeSet;

use llrm_analysis::consts;
use llrm_analysis::induction::{self, AffineOperand, CountedLoop};
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::cfg;
use llrm_graph::loops::{self, Loop};
use llrm_mir::context::{Constant, ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CallInfo, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Declared, FunctionPass, Outer, PreservedAnalyses};
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use crate::counting::{self, Seeds};
use crate::edges;
use crate::lcssa::{arms, from_arms, operations};

pub struct Fill;

impl FunctionPass for Fill {
    fn name(&self) -> &'static str {
        "fill"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        if filled(unit.context, unit.layout, unit.callees, unit.function, analyses.outer(), unit.declared) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// `function` with every such loop's body made one fill; whether any was.
pub fn filled(context: &mut Context, layout: &DataLayout, callees: &Callees, function: &mut Function, outer: &Outer, declared: &mut Declared) -> bool {
    let mut changed = false;
    'again: loop {
        for loop_ in loops::loops(&cfg::graph(function), None) {
            let found = _fill(&Unit::within(context, layout, function, outer), callees, &loop_);
            if let Some(found) = found {
                _filled(context, declared, function, &found);
                changed = true;
                continue 'again;
            }
        }
        return changed;
    }
}

/// What makes a loop one fill.
struct _Found {
    proof: CountedLoop,
    header: BlockId,
    first: BlockId,
    latch: BlockId,
    exit: BlockId,
    /// The store or memset each trip makes.
    effect: InstId,
    pointer: Operand,
    byte: Byte,
    /// Bytes each trip fills.
    bytes: BigInt,
    /// Counters the exit reads from the header, with their steps.
    left: Vec<(ValueId, BigInt)>,
    /// The memset's pointer space and length width.
    memset: (u32, u32),
}

/// What a fill sets each byte to.
enum Byte {
    Operand(Operand),
    Number(u128),
}

/// The fill `loop_` is, if it is one.
fn _fill(unit: &Unit, callees: &Callees, loop_: &Loop) -> Option<_Found> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    let successors = function.successors(header);
    if loop_.latches.len() != 1 || successors.len() != 2 {
        return None;
    }
    // The body is one straight line back to the header, in whatever order its blocks lie.
    let chain = _chain(function, header, loop_)?;
    let latch = *chain.last().expect("a chain has blocks");
    let exit = *successors.iter().find(|to| !loop_.body.contains(&cfg::id(**to)))?;

    // How many trips is `induction`'s to prove, whatever the counter's step or test.
    let tested = operations(function, header);
    let pure = |inst: InstId| _pure(unit.context, callees, function, inst);
    let proof = induction::counted(unit, loop_, None, true).into_iter().find(|proof| {
        !proof.posttested && tested.last() == Some(&proof.branch) && tested.contains(&proof.compare) && tested.iter().all(|&one| one == proof.branch || one == proof.compare || pure(one))
    })?;
    let counters = induction::basics(unit, loop_);
    let phis = edges::phis(function, header);
    if counters.len() != phis.len() {
        return None;
    }

    let work = chain.iter().flat_map(|&block| operations(function, block).into_iter().filter(move |&inst| function.terminator(block) != Some(inst))).collect::<Vec<_>>();
    let effects = work.iter().copied().filter(|&inst| !pure(inst)).collect::<Vec<_>>();
    let steps = work.iter().copied().filter_map(|inst| _stepped(unit, &phis, latch, inst)).collect::<BTreeSet<_>>();
    let [effect] = effects[..] else { return None };
    if steps.len() != phis.len() {
        return None;
    }
    let still = induction::invariant(function, &loop_.body);
    let (pointer, byte, bytes) = _stored(unit, callees, effect, &still)?;

    let facts = consts::known(unit, None, None, None);
    let Operand::Value(address) = pointer else { return None };
    let formula = induction::derived(unit, loop_, Some(&counters)).into_iter().find(|one| one.pointer.is_some() && function.instruction(one.op).result == Some(address))?;
    let width = formula.of.start.width();
    let stride = induction::_signed(&formula.by, &facts, width)? * induction::_signed(&formula.of.step, &facts, width)?;
    if stride != bytes || proof.width() != width || unit.layout.pointer(unit.space(pointer)?).index_bits != width {
        return None;
    }
    // Trips of a byte never wrap the index; wider cells need a promise.
    let modulus = BigInt::from(1) << width;
    let bounded = proof.maximum.as_ref().is_some_and(|maximum| maximum * &bytes < modulus);
    if bytes != BigInt::from(1) && !MemRef::at(unit, pointer, 1).inbounds && !bounded {
        return None;
    }

    // Nothing after the loop may read what it computed, but a counter the exit's phis take from the header.
    let mut left = Vec::new();
    for &at in &loop_.body {
        for &inst in function.block(cfg::block(at)).instructions() {
            let Some(result) = function.instruction(inst).result else { continue };
            for one in function.users(result) {
                let user = function.instruction(one.user);
                if function.parent(one.user).is_some_and(|block| loop_.body.contains(&cfg::id(block))) {
                    continue;
                }
                let from = user.operands.get(one.index as usize + 1);
                let counter = counters.get(&result).filter(|counter| counter.start.width() == proof.width());
                let AffineOperand::Const(step) = &counter?.step else { return None };
                if user.opcode != Opcode::Phi || function.parent(one.user) != Some(exit) || from != Some(&Operand::Block(header)) {
                    return None;
                }
                left.push((result, induction::_signed(&AffineOperand::Const(step.clone()), &facts, width)?));
            }
        }
    }
    left.dedup();
    let memset = (unit.space(pointer)?, width);
    Some(_Found { proof, header, first: chain[0], latch, exit, effect, pointer, byte, bytes, left, memset })
}

/// `llvm.memset` for pointers of `space` and lengths `width` bits wide,
/// declared where the module has none.
fn _memset(context: &mut Context, declared: &mut Declared, space: u32, width: u32) -> (GlobalId, TypeId) {
    let types = &mut context.types;
    let (void, pointer, byte, length, flag) = (types.void(), types.ptr(space), types.int(8), types.int(width), types.int(1));
    let ty = types.intern(Type::Function { returns: void, parameters: vec![pointer, byte, length, flag], variadic: false });
    (declared.declare(&format!("llvm.memset.p{space}.i{width}"), ty), ty)
}

/// The loop made one trip that fills.
fn _filled(context: &mut Context, declared: &mut Declared, function: &mut Function, found: &_Found) {
    let width = found.proof.width();
    let mut seeds = Seeds { context, function, at: found.effect, width };
    let trips = induction::trips(&found.proof, &mut |kind, args| seeds.computed(kind, args)).expect("a pre-tested proof");
    let count = if found.bytes == BigInt::from(1) { trips.clone() } else { seeds.computed(BinaryOp::Mul, vec![trips.clone(), AffineOperand::constant(found.bytes.clone(), width)]) };
    let finals = found
        .left
        .iter()
        .map(|(value, step)| {
            let moved = if *step == BigInt::from(1) { trips.clone() } else { seeds.computed(BinaryOp::Mul, vec![trips.clone(), AffineOperand::constant(step.clone(), width)]) };
            (*value, seeds.computed(BinaryOp::Add, vec![AffineOperand::Value(*value, width), moved]))
        })
        .collect::<IndexMap<_, _>>();
    let count = seeds.operand(&count);
    let finals = finals.into_iter().map(|(value, sum)| (value, seeds.operand(&sum))).collect::<IndexMap<_, _>>();
    let (callee, function_type) = _memset(seeds.context, declared, found.memset.0, found.memset.1);
    let ty = seeds.context.types.ptr(0);
    let callee = Operand::Constant(seeds.context.constant(Constant { ty, kind: ConstantKind::Global(callee) }));
    let off = counting::constant(seeds.context, &BigInt::from(0), 1);
    let byte = match found.byte {
        Byte::Operand(byte) => byte,
        Byte::Number(byte) => counting::constant(seeds.context, &BigInt::from(byte), 8),
    };
    let void = seeds.context.types.void();
    let info = CallInfo { function_type, calling_convention: 0, return_attrs: Vec::new(), argument_attrs: vec![Vec::new(); 4], attrs: Vec::new(), tail: Default::default() };
    let call = function.create_instruction(Opcode::Call(Box::new(info)), void, vec![found.pointer, byte, count, off, callee], Flags::default(), None);
    function.insert(call, Position::Before(found.effect)).expect("a placed effect");
    function.erase(found.effect).expect("an effect has no result");

    // The exit's phis take each counter's exit value from the one trip.
    for phi in edges::phis(function, found.exit) {
        let mut incoming = arms(function, phi);
        let Some(&(value, _)) = incoming.iter().find(|(_, from)| *from == found.header) else { continue };
        let value = match value {
            Operand::Value(one) => finals.get(&one).copied().unwrap_or(value),
            _ => value,
        };
        incoming.push((value, found.latch));
        function.set_operands(phi, from_arms(&incoming));
    }
    let back = function.terminator(found.latch).expect("a latch branch");
    function.set_operands(back, vec![Operand::Block(found.exit)]);
    for phi in edges::phis(function, found.header) {
        let [(start, _)] = arms(function, phi).into_iter().filter(|(_, from)| *from != found.latch).collect::<Vec<_>>()[..] else { unreachable!("one preheader") };
        let result = function.instruction(phi).result.expect("a phi's value");
        function.replace_all_uses_with(result, start);
        function.set_operands(phi, Vec::new());
        function.erase(phi).expect("its uses were replaced");
    }
    // A proven positive count means the header's test passes on entry: it guards nothing.
    if found.proof.count.as_ref().is_some_and(|count| *count != BigInt::from(0)) {
        let test = function.terminator(found.header).expect("a header branch");
        function.set_operands(test, vec![Operand::Block(found.first)]);
        for phi in edges::phis(function, found.exit) {
            let kept = arms(function, phi).into_iter().filter(|(_, from)| *from != found.header).collect::<Vec<_>>();
            function.set_operands(phi, from_arms(&kept));
        }
    }
}

/// The loop's blocks after its header, when each has one way in and one out and the last goes back.
fn _chain(function: &Function, header: BlockId, loop_: &Loop) -> Option<Vec<BlockId>> {
    let mut chain = Vec::new();
    let mut at = function.successors(header).into_iter().find(|to| loop_.body.contains(&cfg::id(*to)));
    while let Some(here) = at.filter(|here| *here != header) {
        let successors = function.successors(here);
        if chain.contains(&here) || successors.len() != 1 || !edges::phis(function, here).is_empty() {
            return None;
        }
        chain.push(here);
        at = Some(successors[0]);
    }
    (!chain.is_empty() && chain.len() + 1 == loop_.body.len()).then_some(chain)
}

/// A store's or a memset's pointer, the byte it fills with, and its bytes,
/// where its value is invariant and one byte repeated.
fn _stored(unit: &Unit, callees: &Callees, effect: InstId, still: &induction::Invariant) -> Option<(Operand, Byte, BigInt)> {
    let function = unit.function;
    let op = function.instruction(effect);
    if let Some((pointer, byte, length)) = memory::memset(unit.context, callees, function, effect) {
        let bytes = unit.int_constant(length)?;
        let volatile = unit.int_constant(op.operands[3])?;
        return (volatile == 0 && still.operand(byte)).then(|| (pointer, Byte::Operand(byte), BigInt::from(bytes)));
    }
    let Opcode::Store { volatile: false, .. } = op.opcode else { return None };
    let (value, pointer) = (op.operands[0], op.operands[1]);
    let width = unit.int_bits(value)?;
    if !still.operand(value) || width % 8 != 0 {
        return None;
    }
    let bytes = BigInt::from(width / 8);
    if width == 8 {
        return Some((pointer, Byte::Operand(value), bytes));
    }
    let bits = unit.int_constant(value)?;
    let byte = bits & 0xFF;
    if (0..width / 8).any(|at| (bits >> (8 * at)) & 0xFF != byte) {
        return None;
    }
    Some((pointer, Byte::Number(byte), bytes))
}

/// The header phi `inst` steps, if it is one's step: a constant added, the
/// phi's value from the latch.
fn _stepped(unit: &Unit, phis: &[InstId], latch: BlockId, inst: InstId) -> Option<ValueId> {
    let function = unit.function;
    let op = function.instruction(inst);
    let (AffineOperand::Value(stepped, _), AffineOperand::Const(_)) = induction::stepping(unit, op)? else { return None };
    let phi = phis.iter().copied().find(|&phi| function.instruction(phi).result == Some(stepped))?;
    (arms(function, phi).iter().any(|&(value, from)| from == latch && Some(value) == op.result.map(Operand::Value))).then_some(stepped)
}

/// Work that stores nothing, reads nothing and cannot trap.
fn _pure(context: &Context, callees: &Callees, function: &Function, inst: InstId) -> bool {
    let op = function.instruction(inst);
    let traps = matches!(op.opcode, Opcode::Binary(BinaryOp::SDiv | BinaryOp::UDiv | BinaryOp::SRem | BinaryOp::URem));
    let reads = matches!(op.opcode, Opcode::Load { .. } | Opcode::Alloca { .. } | Opcode::Call(_));
    !traps && !reads && memory::only_value(context, callees, function, inst)
}

#[cfg(test)]
#[path = "fill_tests.rs"]
mod tests;
