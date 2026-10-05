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
//!
//! Straight-line stores of one repeated byte to adjacent bytes of one
//! object are one memset too: LLVM's MemCpyOpt, `tryMergingIntoMemset`.

use std::collections::BTreeSet;

use llrm_analysis::induction::{self, AffineOperand, CountedLoop};
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
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
use num_traits::ToPrimitive;

use crate::counting::{self, Seeds};
use crate::profit::{self, OperationCosts};
use crate::edges;
use crate::lcssa::{arms, from_arms, operations};

/// `size`: priced in code bytes, as under `-Os`.
pub struct Fill {
    pub size: bool,
}

impl FunctionPass for Fill {
    fn name(&self) -> &'static str {
        "fill"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        if filled(unit.context, unit.layout, analyses.outer().callees(), unit.function, analyses.outer(), unit.declared, self.size) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// The straight-line fills, once the scalar passes have settled: no pass
/// asks a memset what a cell holds, so merged sooner a store's value is
/// lost to the forwarding after it. LLVM merges stores in codegen, too.
pub struct Merge;

impl FunctionPass for Merge {
    fn name(&self) -> &'static str {
        "merge"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        if merged(unit.context, unit.layout, analyses.outer().callees(), unit.function, analyses.outer(), unit.declared) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// One store a merge may take: its object's address, the bytes it covers
/// there, the byte it repeats, its pointer.
struct _Cell {
    store: InstId,
    /// Its place in its block.
    order: usize,
    root: Operand,
    low: i64,
    high: i64,
    byte: u128,
    pointer: Operand,
}

/// `function` with each run of at least two straight-line stores of one
/// repeated byte to adjacent bytes of one object made one memset where the
/// last of them stood; whether any was. Between them only work that touches
/// no memory, and stores to other bytes, may stand.
pub fn merged(context: &mut Context, layout: &DataLayout, callees: &Callees, function: &mut Function, outer: &Outer, declared: &mut Declared) -> bool {
    let mut runs: Vec<(Vec<_Cell>, u32, u32)> = Vec::new();
    {
        let unit = Unit::within(context, layout, function, outer);
        for block in function.layout() {
            let mut open: Vec<_Cell> = Vec::new();
            for (order, &inst) in function.block(*block).instructions().iter().enumerate() {
                if let Some(cell) = _cell(&unit, inst, order) {
                    open.push(cell);
                } else if !_pure(unit.context, callees, function, inst) || matches!(function.instruction(inst).opcode, Opcode::Store { .. }) {
                    runs.extend(_adjacent(&unit, std::mem::take(&mut open)));
                }
            }
            runs.extend(_adjacent(&unit, open));
        }
    }
    for (run, space, width) in &runs {
        let length: i64 = run.iter().map(|one| one.high - one.low).sum();
        let (callee, function_type) = _memset(context, declared, *space, *width);
        let ty = context.types.ptr(0);
        let callee = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Global(callee) }));
        let byte = counting::constant(context, &BigInt::from(run[0].byte), 8);
        let count = counting::constant(context, &BigInt::from(length), *width);
        let off = counting::constant(context, &BigInt::from(0), 1);
        let void = context.types.void();
        let info = CallInfo { function_type, calling_convention: 0, return_attrs: Vec::new(), argument_attrs: vec![Vec::new(); 4], attrs: Vec::new(), tail: Default::default() };
        let lowest = run.iter().min_by_key(|one| one.low).expect("a run has stores");
        let last = run.iter().max_by_key(|one| one.order).expect("a run has stores").store;
        let call = function.create_instruction(Opcode::Call(Box::new(info)), void, vec![lowest.pointer, byte, count, off, callee], Flags::default(), None);
        function.insert(call, Position::Before(last)).expect("a placed store");
        for one in run {
            function.erase(one.store).expect("a store has no result");
        }
    }
    !runs.is_empty()
}

/// The store `inst` is, where it writes one repeated byte at a constant
/// displacement in an object.
fn _cell(unit: &Unit, inst: InstId, order: usize) -> Option<_Cell> {
    let op = unit.function.instruction(inst);
    let Opcode::Store { volatile: false, .. } = op.opcode else { return None };
    let (value, pointer) = (op.operands[0], op.operands[1]);
    let width = unit.int_bits(value)?;
    let byte = _repeated(unit, value, width)?;
    let reference = MemRef::at(unit, pointer, width / 8);
    let root = reference.root.filter(|_| reference.object && reference.base.is_none() && reference.segment.is_none())?;
    Some(_Cell { store: inst, order, root, low: reference.disp, high: reference.disp + i64::from(width / 8), byte, pointer })
}

/// Whether `run` is better one memset, as LLVM's `isProfitableToUseMemset`
/// judges: four stores or 16 bytes are; fewer only where the memset needs
/// fewer stores of the widest native integer, `widest` bytes, since the
/// code generator pairs stores itself.
fn _profitable(run: &[_Cell], widest: i64) -> bool {
    let bytes: i64 = run.iter().map(|one| one.high - one.low).sum();
    if run.len() >= 4 || bytes >= 16 {
        return true;
    }
    run.len() > 1 && run.len() as i64 > bytes / widest + bytes % widest
}

/// The byte `value`, `width` bits of one byte repeated, is made of.
fn _repeated(unit: &Unit, value: Operand, width: u32) -> Option<u128> {
    if width % 8 != 0 || width == 0 {
        return None;
    }
    let bits = unit.int_constant(value)?;
    let byte = bits & 0xFF;
    (0..width / 8).all(|at| (bits >> (8 * at)) & 0xFF == byte).then_some(byte)
}

/// The runs `open`'s stores make: those of one object and byte that tile
/// its bytes without a gap, where no other store in `open` touches them.
fn _adjacent(unit: &Unit, open: Vec<_Cell>) -> Vec<(Vec<_Cell>, u32, u32)> {
    let mut objects: Vec<Vec<_Cell>> = Vec::new();
    for cell in open {
        match objects.iter_mut().find(|object| object[0].root == cell.root) {
            Some(object) => object.push(cell),
            None => objects.push(vec![cell]),
        }
    }
    // Two stores to one byte keep their order: leave the object alone.
    let mut groups: Vec<Vec<_Cell>> = Vec::new();
    for mut object in objects {
        object.sort_by_key(|one| one.low);
        if object.windows(2).any(|pair| pair[1].low < pair[0].high) {
            continue;
        }
        // A near and a far pointer to one object are two memsets' operands.
        for cell in object {
            match groups.iter_mut().find(|group| group[0].root == cell.root && unit.space(group[0].pointer) == unit.space(cell.pointer)) {
                Some(group) => group.push(cell),
                None => groups.push(vec![cell]),
            }
        }
    }
    let mut runs = Vec::new();
    for group in groups {
        let Some(space) = unit.space(group[0].pointer) else { continue };
        let width = unit.layout.pointer(space).index_bits;
        let widest = i64::from(unit.layout.largest_legal_integer() / 8).max(1);
        let mut run: Vec<_Cell> = Vec::new();
        for cell in group {
            if run.last().is_some_and(|last| last.high != cell.low || last.byte != cell.byte) {
                if _profitable(&run, widest) {
                    runs.push((std::mem::take(&mut run), space, width));
                }
                run.clear();
            }
            run.push(cell);
        }
        if _profitable(&run, widest) {
            runs.push((run, space, width));
        }
    }
    runs
}

/// `function` with every such loop's body made one fill; whether any was.
pub fn filled(context: &mut Context, layout: &DataLayout, callees: &Callees, function: &mut Function, outer: &Outer, declared: &mut Declared, size: bool) -> bool {
    let costs = if size { outer.target().size_costs() } else { outer.target().costs() };
    let mut changed = false;
    'again: loop {
        for loop_ in cfg::Shape::of(function).loops {
            let found = _fill(&Unit::within(context, layout, function, outer), callees, &loop_, &costs, size);
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
    /// A cell of this many bytes, which no one byte repeats: `memset.pattern`.
    Pattern(Operand, u32),
}

/// The fill `loop_` is, if it is one.
fn _fill(unit: &Unit, callees: &Callees, loop_: &Loop, costs: &OperationCosts, size: bool) -> Option<_Found> {
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

    let facts = unit.registers();
    let Operand::Value(address) = pointer else { return None };
    let walk = induction::recurrences(unit, loop_, &counters);
    let formula = walk.values.get(&address).filter(|one| one.pointer.is_some())?;
    let width = formula.width();
    // A huge pointer carries into its selector: `rep stos` through es:di wraps at 64K.
    if unit.layout.carries(unit.space(pointer)?) {
        return None;
    }
    let stride = formula.step.known()?;
    if stride != bytes || proof.width() != width || unit.layout.pointer(unit.space(pointer)?).index_bits != width {
        return None;
    }
    // A pattern is `rep stosw` or `stosd`: priced, as a short loop beats its setup.
    if matches!(byte, Byte::Pattern(..)) && !_pays(unit, callees, &chain, header, &proof, costs, size) {
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

/// Whether the one fill is cheaper than the loop for its trips, the proven
/// count or the profit model's estimate: `rep stos` pays its setup and each
/// cell, and a count of few cells is stores. Under `size` it is the code bytes
/// of the loop against the fill's.
fn _pays(unit: &Unit, callees: &Callees, chain: &[BlockId], header: BlockId, proof: &CountedLoop, costs: &OperationCosts, size: bool) -> bool {
    let function = unit.function;
    let each: i64 = std::iter::once(&header)
        .chain(chain)
        .flat_map(|&block| operations(function, block))
        // In bytes the loop's counter work is the step and its branch: the phi is a register, the address an operand, the test the step's flags.
        .filter(|&inst| !size || !matches!(function.instruction(inst).opcode, Opcode::Phi | Opcode::GetElementPtr { .. } | Opcode::ICmp(_)))
        .map(|inst| profit::operation(unit.context, unit.layout, function, callees, inst, costs).unwrap_or(costs.add))
        .sum();
    let known = proof.count.as_ref().and_then(ToPrimitive::to_i64);
    let most = proof.maximum.as_ref().and_then(ToPrimitive::to_i64);
    _cheaper(each, known, most, costs, size)
}

/// Whether a fill beats a loop of `each` per trip, over `known` trips or, where
/// there are none, up to `most` of them.
fn _cheaper(each: i64, known: Option<i64>, most: Option<i64>, costs: &OperationCosts, size: bool) -> bool {
    let trips = if size { 1 } else { known.unwrap_or_else(|| most.unwrap_or(i64::MAX).min(profit::UNKNOWN_TRIPS)) };
    let string = costs.fill + trips * costs.fill_cell;
    let fill = match known {
        Some(count) if count <= 16 => string.min(count * costs.store),
        _ => string,
    };
    trips * each > fill
}

/// `llvm.memset` for pointers of `space` and lengths `width` bits wide,
/// declared where the module has none.
fn _memset(context: &mut Context, declared: &mut Declared, space: u32, width: u32) -> (GlobalId, TypeId) {
    let types = &mut context.types;
    let (void, pointer, byte, length, flag) = (types.void(), types.ptr(space), types.int(8), types.int(width), types.int(1));
    let ty = types.intern(Type::Function { returns: void, parameters: vec![pointer, byte, length, flag], variadic: false });
    (declared.declare(&format!("llvm.memset.p{space}.i{width}"), ty), ty)
}

/// `llvm.experimental.memset.pattern` of cells of type `cell` for pointers of
/// `space` and counts `width` bits wide, declared where the module has none.
fn _pattern(context: &mut Context, declared: &mut Declared, space: u32, cell: TypeId, width: u32) -> (GlobalId, TypeId) {
    let bits = context.types.int_bits(cell).expect("an integer cell");
    let types = &mut context.types;
    let (void, pointer, count, flag) = (types.void(), types.ptr(space), types.int(width), types.int(1));
    let ty = types.intern(Type::Function { returns: void, parameters: vec![pointer, cell, count, flag], variadic: false });
    (declared.declare(&format!("llvm.experimental.memset.pattern.p{space}.i{bits}.i{width}"), ty), ty)
}

/// The loop made one trip that fills.
fn _filled(context: &mut Context, declared: &mut Declared, function: &mut Function, found: &_Found) {
    let width = found.proof.width();
    let mut seeds = Seeds { context, function, at: found.effect, width };
    let trips = induction::trips(&found.proof, &mut |kind, args| seeds.computed(kind, args)).expect("a pre-tested proof");
    let cells = matches!(found.byte, Byte::Pattern(..));
    let count = if found.bytes == BigInt::from(1) || cells { trips.clone() } else { seeds.computed(BinaryOp::Mul, vec![trips.clone(), AffineOperand::constant(found.bytes.clone(), width)]) };
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
    let (callee, function_type) = match found.byte {
        Byte::Pattern(value, _) => {
            let cell = seeds.function.operand_type(seeds.context, value).expect("a typed cell");
            _pattern(seeds.context, declared, found.memset.0, cell, found.memset.1)
        }
        _ => _memset(seeds.context, declared, found.memset.0, found.memset.1),
    };
    let ty = seeds.context.types.ptr(0);
    let callee = Operand::Constant(seeds.context.constant(Constant { ty, kind: ConstantKind::Global(callee) }));
    let off = counting::constant(seeds.context, &BigInt::from(0), 1);
    let byte = match found.byte {
        Byte::Operand(byte) => byte,
        Byte::Number(byte) => counting::constant(seeds.context, &BigInt::from(byte), 8),
        Byte::Pattern(value, _) => value,
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
    if let Some(byte) = _repeated(unit, value, width) {
        return Some((pointer, Byte::Number(byte), bytes));
    }
    // LLVM's memset_pattern16: the cell is a word or dword; the stored value need not be a constant.
    matches!(width, 16 | 32).then_some((pointer, Byte::Pattern(value, width / 8), bytes))
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
