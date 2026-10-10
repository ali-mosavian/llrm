//! A counted loop that stores one byte value into consecutive bytes is one
//! `llvm.memset`: LLVM's LoopIdiomRecognize. Adapted from llrm-core's
//! `optimize/fill.rs`, the port of `qbopt/optimize/fill.py`.
//!
//! The loop keeps one trip, guarded by its header's test, which fills what
//! every trip stored; its counters leave with their exit values.
//!
//! What changed with the IR:
//! - The fill is `llvm.memset`, of bytes and a byte count: a wider store fills
//!   only where each of its bytes is one number. A word fill of any other
//!   value, `rep stosw`, is isel's shape; the rich MIR has none.
//! - Where each trip stores is induction's `derived` address, a GEP off an
//!   invariant pointer; its bytes per trip must be the element's. The old
//!   `_offset` walked adds of a register base, and `_stepping` compared the
//!   counter's step itself.
//! - The fill's address is the store's own pointer, which the one trip left
//!   computes from the counters' starts. The old one rebuilt it from the cell's
//!   base, displacement, segment and storage class.
//! - Its count in bytes must not wrap the index: a byte's trips never do, and
//!   wider cells need `inbounds` GEPs or the proof's `maximum`. The counter
//!   must be the index's width.
//! - The memset is declared where the module has none, through the pass
//!   manager's `Declared`.
//! - `pure` is `memory::only_value` less loads and allocas, and less divisions,
//!   which trap.
//!
//! llrm-mir has no idiom pass.
//!
//! Straight-line stores of one repeated byte to adjacent bytes of one
//! object are one memset too: LLVM's MemCpyOpt, `tryMergingIntoMemset`.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, AffineOperand, CountedLoop};
use llrm_analysis::memory::{MemRef, Unit};
use llrm_mir::context::{Constant, ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{BlockId, Function, InstId, MetadataNode, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CallInfo, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Declared, FunctionPass, Outer, PreservedAnalyses};
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::counting::{self, Seeds};
use crate::edges;
use crate::lcssa::{arms, from_arms, operations};
use crate::profit::{self, OperationCosts};

/// `size`: priced in code bytes, as under `-Os`.
pub struct Fill {
    pub size: bool,
}

impl FunctionPass for Fill {
    fn name(&self) -> &'static str {
        "fill"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let registers = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
        let changed = filled_with(
            unit.context,
            unit.layout,
            analyses.outer().callees(),
            unit.function,
            analyses.outer(),
            unit.declared,
            self.size,
            &mut llrm_analysis::memory::Standing::held(&registers),
        );
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
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

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if merged(unit.context, unit.layout, analyses.outer().callees(), unit.function, analyses.outer(), unit.declared)
        {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// One store a merge may take: its object's address, the bytes it covers
/// there, the byte it repeats (none for a constant of several), its value,
/// its pointer.
#[derive(Clone)]
struct _Cell {
    store: InstId,
    /// Its place in its block.
    order: usize,
    root: Operand,
    low: i64,
    high: i64,
    byte: Option<u128>,
    value: u128,
    pointer: Operand,
}

/// `function` with each run of at least two straight-line stores of one
/// repeated byte to adjacent bytes of one object made one memset where the
/// last of them stood, and each run of constants left, adjacent in one object,
/// made stores of the widest legal integer (gcc's `store-merging`, LLVM's
/// `MergeConsecutiveStores`: what lets a memset of a few bytes go unmade, since
/// the stores are then as few); whether any was. Between them only work that
/// touches no memory, and stores to other bytes, may stand.
pub fn merged(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    outer: &Outer,
    declared: &mut Declared,
) -> bool {
    let mut runs: Vec<(Vec<_Cell>, u32, u32)> = Vec::new();
    let mut wides: Vec<_Wide> = Vec::new();
    {
        let unit = Unit::within(context, layout, function, outer);
        let mut take = |open: Vec<_Cell>| {
            let (filled, rest) = _adjacent(&unit, open);
            runs.extend(filled);
            wides.extend(_wide(&unit, rest));
        };
        for block in function.layout() {
            let mut open: Vec<_Cell> = Vec::new();
            for (order, &inst) in function.block(*block).instructions().iter().enumerate() {
                if let Some(cell) = _cell(&unit, inst, order) {
                    open.push(cell);
                } else if !memory::speculatable(unit.context, callees, function, inst)
                    || matches!(function.instruction(inst).opcode, Opcode::Store { .. })
                {
                    take(std::mem::take(&mut open));
                }
            }
            take(open);
        }
    }
    for wide in &wides {
        let value = counting::constant(context, &BigInt::from(wide.value), wide.bits);
        let void = context.types.void();
        let last = wide.cells.iter().max_by_key(|one| one.order).expect("a run has stores").store;
        let store = function.create_instruction(
            Opcode::Store { align: None, volatile: false },
            void,
            vec![value, wide.pointer],
            Flags::default(),
            None,
        );
        function.insert(store, Position::Before(last)).expect("a placed store");
        for one in &wide.cells {
            function.erase(one.store).expect("a store has no result");
        }
    }
    for (run, space, width) in &runs {
        let length: i64 = run.iter().map(|one| one.high - one.low).sum();
        let (callee, function_type) = _memset(context, declared, *space, *width);
        let ty = context.types.ptr(0);
        let callee = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Global(callee) }));
        let byte = counting::constant(context, &BigInt::from(run[0].byte.expect("a repeated byte")), 8);
        let count = counting::constant(context, &BigInt::from(length), *width);
        let off = counting::constant(context, &BigInt::from(0), 1);
        let void = context.types.void();
        let info = CallInfo {
            function_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); 4],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        let lowest = run.iter().min_by_key(|one| one.low).expect("a run has stores");
        let last = run.iter().max_by_key(|one| one.order).expect("a run has stores").store;
        let call = function.create_instruction(
            Opcode::Call(Box::new(info)),
            void,
            vec![lowest.pointer, byte, count, off, callee],
            Flags::default(),
            None,
        );
        function.insert(call, Position::Before(last)).expect("a placed store");
        for one in run {
            function.erase(one.store).expect("a store has no result");
        }
    }
    !runs.is_empty() || !wides.is_empty()
}

/// The store `inst` is, where it writes one repeated byte at a constant
/// displacement in an object.
fn _cell(
    unit: &Unit,
    inst: InstId,
    order: usize,
) -> Option<_Cell> {
    let op = unit.function.instruction(inst);
    let Opcode::Store { volatile: false, .. } = op.opcode else { return None };
    let (value, pointer) = (op.operands[0], op.operands[1]);
    let width = unit.int_bits(value)?;
    let bits = unit.int_constant(value)?;
    let byte = _repeated(unit, value, width);
    let reference = MemRef::at(unit, pointer, width / 8);
    let root =
        reference.root.filter(|_| reference.object && reference.base.is_none() && reference.segment.is_none())?;
    Some(_Cell {
        store: inst,
        order,
        root,
        low: reference.disp,
        high: reference.disp + i64::from(width / 8),
        byte,
        value: bits,
        pointer,
    })
}

/// Whether `run` is better one memset, as LLVM's `isProfitableToUseMemset`
/// judges: four stores or 16 bytes are; fewer only where the memset needs
/// fewer stores of the widest native integer, `widest` bytes, since the
/// code generator pairs stores itself.
fn _profitable(
    run: &[_Cell],
    widest: i64,
) -> bool {
    let bytes: i64 = run.iter().map(|one| one.high - one.low).sum();
    if run.len() >= 4 || bytes >= 16 {
        return true;
    }
    run.len() > 1 && run.len() as i64 > bytes / widest + bytes % widest
}

/// The byte `value`, `width` bits of one byte repeated, is made of.
fn _repeated(
    unit: &Unit,
    value: Operand,
    width: u32,
) -> Option<u128> {
    if width % 8 != 0 || width == 0 {
        return None;
    }
    let bits = unit.int_constant(value)?;
    let byte = bits & 0xFF;
    (0..width / 8).all(|at| (bits >> (8 * at)) & 0xFF == byte).then_some(byte)
}

/// The runs `open`'s stores make: those of one object and byte that tile
/// its bytes without a gap, where no other store in `open` touches them; and
/// the stores left, in groups of one object and address space, by address.
fn _adjacent(
    unit: &Unit,
    open: Vec<_Cell>,
) -> (Vec<(Vec<_Cell>, u32, u32)>, Vec<Vec<_Cell>>) {
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
            match groups
                .iter_mut()
                .find(|group| group[0].root == cell.root && unit.space(group[0].pointer) == unit.space(cell.pointer))
            {
                Some(group) => group.push(cell),
                None => groups.push(vec![cell]),
            }
        }
    }
    let mut runs = Vec::new();
    let mut left = Vec::new();
    for group in groups {
        let Some(space) = unit.space(group[0].pointer) else {
            left.push(group);
            continue;
        };
        let width = unit.layout.pointer(space).index_bits;
        let widest = i64::from(unit.layout.largest_legal_integer() / 8).max(1);
        let mut rest: Vec<_Cell> = Vec::new();
        let mut run: Vec<_Cell> = Vec::new();
        let mut flush = |run: &mut Vec<_Cell>, rest: &mut Vec<_Cell>| {
            if _profitable(run, widest) {
                runs.push((std::mem::take(run), space, width));
            } else {
                rest.append(run);
            }
        };
        for cell in group {
            if run.last().is_some_and(|last| last.high != cell.low || last.byte != cell.byte) || cell.byte.is_none() {
                flush(&mut run, &mut rest);
            }
            if cell.byte.is_some() {
                run.push(cell);
            } else {
                rest.push(cell);
            }
        }
        flush(&mut run, &mut rest);
        left.push(rest);
    }
    (runs, left)
}

/// Stores of the widest legal integer in place of adjacent smaller constant
/// ones: one run's cells, the value they make and its width.
struct _Wide {
    cells: Vec<_Cell>,
    value: u128,
    bits: u32,
    /// The lowest cell's pointer.
    pointer: Operand,
}

/// The wider stores the constants left in `groups` make: from each address, the
/// widest power of two of bytes, up to the largest legal integer, that two or
/// more adjacent stores tile exactly.
fn _wide(
    unit: &Unit,
    groups: Vec<Vec<_Cell>>,
) -> Vec<_Wide> {
    let widest = i64::from(unit.layout.largest_legal_integer() / 8).max(1);
    let mut made = Vec::new();
    for group in groups {
        let mut cells = group.into_iter();
        let mut pending: Vec<_Cell> = Vec::new();
        let mut pieces: Vec<Vec<_Cell>> = Vec::new();
        // Maximal runs of stores that tile bytes without a gap.
        while let Some(cell) = cells.next() {
            if pending.last().is_some_and(|last| last.high != cell.low) {
                pieces.push(std::mem::take(&mut pending));
            }
            pending.push(cell);
        }
        pieces.push(pending);
        for piece in pieces {
            let mut at = 0;
            while at < piece.len() {
                let mut size = widest;
                let mut took = None;
                while size >= 2 {
                    let mut bytes = 0;
                    let mut end = at;
                    while end < piece.len() && bytes < size {
                        bytes += piece[end].high - piece[end].low;
                        end += 1;
                    }
                    if bytes == size && end - at >= 2 && piece[at].low % size == 0 {
                        took = Some(end);
                        break;
                    }
                    size /= 2;
                }
                let Some(end) = took else {
                    at += 1;
                    continue;
                };
                let low = piece[at].low;
                let mut value = 0u128;
                for cell in &piece[at..end] {
                    let bytes = (cell.high - cell.low) as u32;
                    let mask = if bytes >= 16 { u128::MAX } else { (1u128 << (8 * bytes)) - 1 };
                    value |= (cell.value & mask) << (8 * (cell.low - low) as u32);
                }
                let cells: Vec<_Cell> = piece[at..end].to_vec();
                made.push(_Wide {
                    cells,
                    value,
                    bits: 8 * (piece[end - 1].high - low) as u32,
                    pointer: piece[at].pointer,
                });
                at = end;
            }
        }
    }
    made
}

/// `function` with every such loop's body made one fill; whether any was.
pub fn filled(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    outer: &Outer,
    declared: &mut Declared,
    size: bool,
) -> bool {
    filled_with(
        context,
        layout,
        callees,
        function,
        outer,
        declared,
        size,
        &mut llrm_analysis::memory::Standing::underived(),
    )
}

/// `filled`, what is known of the body without memory given as `standing` says:
/// derived once for each state of the body, not once for each loop.
#[allow(clippy::too_many_arguments)]
pub fn filled_with(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    outer: &Outer,
    declared: &mut Declared,
    size: bool,
    standing: &mut llrm_analysis::memory::Standing,
) -> bool {
    let costs = if size { outer.target().size_costs() } else { outer.target().costs() };
    let mut changed = false;
    'again: loop {
        let shape = cfg::Shape::of(function);
        for loop_ in shape.loops.clone() {
            let found = {
                let unit = Unit::within(context, layout, function, outer).with_shape(&shape);
                let facts = standing.of(&unit);
                _fill(&unit.with_registers(facts), callees, &loop_, &costs, size)
            };
            if let Some(found) = found {
                _filled(context, declared, function, &found);
                standing.changed();
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
    stored: Stored,
    /// Bytes each trip fills.
    bytes: BigInt,
    /// The other fills of the loop, where it makes several that touch apart
    /// bytes: each a memset of its own (LLVM's `LoopIdiomRecognize` takes each
    /// store by itself; gcc's loop distribution splits the loop by them).
    extras: Vec<Extra>,
    /// The last of the loop's effects: where the calls go, after every
    /// address and count they read.
    anchor: InstId,
    /// What the exit reads of the counters, by the value it reads: the counter,
    /// its step, and the trips more than the loop's count the value has
    /// taken (a tested-after loop's exit takes the counter of its last trip:
    /// one fewer, or its next value: none).
    left: Vec<(ValueId, ValueId, BigInt, i64)>,
    /// The loop is tested after its trips, entered behind a copy of its test
    /// (`-ftree-ch`).
    posttested: bool,
    /// The memset's pointer space and length width.
    memset: (u32, u32),
}

/// A fill of a loop that makes several.
struct Extra {
    effect: InstId,
    pointer: Operand,
    byte: Byte,
    bytes: BigInt,
}

/// What a loop's trips make of the bytes they reach.
enum Stored {
    Fill(Byte),
    Copy(Copy),
}

/// A load and a store of one cell each trip, the stored value the loaded:
/// where it reads, how it may overlap what it writes, and whether it runs down.
struct Copy {
    load: InstId,
    source: Operand,
    how: How,
    descending: bool,
    /// The source's pointer space.
    space: u32,
}

/// What lets one copy stand for the loop's trips in order.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum How {
    /// The two ranges are apart: `llvm.memcpy`.
    Apart,
    /// They overlap, but what each trip reads is as the trips before it left it
    /// untouched: `llvm.memmove`, running the loop's way.
    Overlapping,
}

/// What a fill sets each byte to.
enum Byte {
    Operand(Operand),
    Number(u128),
    /// A cell whose bytes do not repeat: `memset.pattern`.
    Pattern(Operand),
}

/// The fill `loop_` is, if it is one.
fn _fill(
    unit: &Unit,
    callees: &Callees,
    loop_: &Loop,
    costs: &OperationCosts,
    size: bool,
) -> Option<_Found> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    if loop_.latches.len() != 1 {
        return None;
    }
    // Tested before its trips: the header tests and the body is a straight line
    // back to it, in whatever order its blocks lie. Tested after
    // them: the body is a straight line from the header to the latch, which
    // tests (a loop entered behind a copy of its test).
    let successors = function.successors(header);
    let posttested = loop_.body.len() == 1 || !successors.iter().any(|to| !loop_.body.contains(&cfg::id(*to)));
    let (chain, latch, test) = if posttested {
        let chain = _chain_after(function, header, loop_)?;
        let latch = *chain.last().expect("a chain has blocks");
        (chain, latch, latch)
    } else {
        let chain = _chain(function, header, loop_)?;
        let latch = *chain.last().expect("a chain has blocks");
        (chain, latch, header)
    };
    let exit = *function.successors(test).iter().find(|to| !loop_.body.contains(&cfg::id(**to)))?;

    // How many trips is `induction`'s to prove, whatever the counter's step or
    // test.
    let tested = operations(function, test);
    let plain = |inst: InstId| memory::speculatable(unit.context, callees, function, inst);
    let proof = induction::counted(unit, loop_, None, true)
        .into_iter()
        .find(
            |proof| proof.posttested == posttested
                && tested.last() == Some(&proof.branch)
                && tested.contains(&proof.compare)
                && (posttested || tested.iter().all(|&one| one == proof.branch || one == proof.compare || plain(one))),
        )?;
    // Its trips, placed before the loop, are what the fill is counted by.
    if posttested && induction::trips(&proof, &mut |_, args| args[0].clone()).is_none() {
        return None;
    }
    let counters = induction::basics(unit, loop_);
    let phis = edges::phis(function, header);
    if counters.len() != phis.len() {
        return None;
    }

    let work = chain
        .iter()
        .flat_map(|&block| {
            operations(function, block).into_iter().filter(move |&inst| function.terminator(block) != Some(inst))
        })
        .collect::<Vec<_>>();
    let effects = work.iter().copied().filter(|&inst| !plain(inst)).collect::<Vec<_>>();
    let steps = work.iter().copied().filter_map(|inst| _stepped(unit, &phis, latch, inst)).collect::<BTreeSet<_>>();
    if steps.len() != phis.len() {
        return None;
    }
    let still = induction::invariant(function, &loop_.body);
    let walk = induction::recurrences(unit, loop_, &counters);
    let (effect, pointer, mut stored, bytes, extras) = match effects[..] {
        [effect] => {
            let (pointer, byte, bytes) = _stored(unit, callees, effect, &still)?;
            (effect, pointer, Stored::Fill(byte), bytes, Vec::new())
        }
        [one, other] => match _copied(unit, one, other) {
            Some((effect, pointer, stored, bytes)) => (effect, pointer, stored, bytes, Vec::new()),
            None => _several(unit, callees, &effects, &still, &walk, &proof)?,
        },
        _ => _several(unit, callees, &effects, &still, &walk, &proof)?,
    };

    let facts = unit.registers();
    let Operand::Value(address) = pointer else { return None };
    let formula = walk.values.get(&address).filter(|one| one.pointer.is_some())?;
    let width = formula.width();
    // A huge pointer carries into its selector: `rep stos` through es:di wraps
    // at 64K.
    if unit.layout.carries(unit.space(pointer)?) {
        return None;
    }
    let stride = formula.step.known()?;
    let descending = matches!(stored, Stored::Copy(_)) && stride == -bytes.clone();
    if (stride != bytes && !descending)
        || proof.width() > width
        || unit.layout.pointer(unit.space(pointer)?).index_bits != width
    {
        return None;
    }
    let pattern = match &stored {
        Stored::Fill(byte) => matches!(byte, Byte::Pattern(..)),
        Stored::Copy(_) => false,
    };
    if let Stored::Copy(copy) = &mut stored {
        copy.descending = descending;
        let Operand::Value(source) = copy.source else { return None };
        let from = walk.values.get(&source).filter(|one| one.pointer.is_some())?;
        let (bits, to) = (unit.layout.pointer(unit.space(copy.source)?).index_bits, formula);
        if from.step != to.step
            || from.width() != width
            || bits != width
            || unit.layout.carries(unit.space(copy.source)?)
        {
            return None;
        }
        copy.how = _overlap(unit, copy.load, effect, from, to, &proof, &bytes, descending)?;
    }
    // A pattern is `rep stosw` or `stosd`: priced, as a short loop beats its
    // setup.
    let moved = match &stored {
        Stored::Copy(copy) => Some((bytes.to_i64()?, copy.descending && copy.how == How::Overlapping)),
        Stored::Fill(_) => None,
    };
    if (pattern || moved.is_some())
        && !_pays(unit, callees, if posttested { &chain[1..] } else { &chain }, header, &proof, costs, size, moved)
    {
        return None;
    }
    // Trips of a byte never wrap the index; wider cells need a promise.
    let modulus = BigInt::from(1) << width;
    let bounded = proof.maximum.as_ref().is_some_and(|maximum| maximum * &bytes < modulus);
    if bytes != BigInt::from(1) && !MemRef::at(unit, pointer, 1).inbounds && !bounded {
        return None;
    }

    // Nothing after the loop may read what it computed, but a counter the
    // exit's phis take from the test's block.
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
                if user.opcode != Opcode::Phi
                    || function.parent(one.user) != Some(exit)
                    || from != Some(&Operand::Block(test))
                {
                    return None;
                }
                // The counter itself, or the value of it that a trip steps to.
                let (base, taken) = match counters.get(&result).filter(|counter| counter.start.width() == proof.width())
                {
                    Some(_) => (result, -i64::from(posttested)),
                    None if posttested => (_stepped(unit, &phis, latch, inst)?, 0),
                    None => return None,
                };
                let AffineOperand::Const(step) =
                    &counters.get(&base).filter(|counter| counter.start.width() == proof.width())?.step
                else {
                    return None;
                };
                left.push((
                    result,
                    base,
                    induction::_signed(&AffineOperand::Const(step.clone()), &facts, proof.width())?,
                    taken,
                ));
            }
        }
    }
    left.dedup();
    let memset = (unit.space(pointer)?, width);
    Some(_Found {
        proof,
        header,
        first: chain[0],
        latch,
        exit,
        effect,
        pointer,
        stored,
        bytes,
        extras,
        anchor: *effects.last().expect("an effect"),
        left,
        posttested,
        memset,
    })
}

/// The first of a loop's fills as `_fill` takes the only one, and the others as
/// `Extra`s, where every effect of the loop is a fill of a plain byte, each
/// strides by its cell over bytes no other reaches. The loop's trips then fill
/// the same bytes in any order of the fills.
fn _several(
    unit: &Unit,
    callees: &Callees,
    effects: &[InstId],
    still: &induction::Invariant,
    walk: &induction::Users,
    proof: &CountedLoop,
) -> Option<(InstId, Operand, Stored, BigInt, Vec<Extra>)> {
    let mut fills = Vec::new();
    for &effect in effects {
        let (pointer, byte, bytes) = _stored(unit, callees, effect, still)?;
        if matches!(byte, Byte::Pattern(..)) {
            return None;
        }
        let Operand::Value(address) = pointer else { return None };
        let formula = walk.values.get(&address).filter(|one| one.pointer.is_some())?;
        if formula.step.known()? != bytes {
            return None;
        }
        fills.push((effect, pointer, byte, bytes));
    }
    if fills.len() < 2 {
        return None;
    }
    // At most the trips the loop can make, from each cell.
    let trips = proof.count.clone().or_else(|| proof.maximum.clone())?;
    for (at, one) in fills.iter().enumerate() {
        let (first, other) = (MemRef::at(unit, one.1, 1), &fills[at + 1..]);
        for two in other {
            let second = MemRef::at(unit, two.1, 1);
            let apart = match (&first.root, &second.root) {
                (Some(left), Some(right)) if first.object && second.object && left != right => true,
                (Some(left), Some(right)) if first.object && second.object => {
                    first.base.is_some()
                        && first.base == second.base
                        && first.scale == second.scale
                        && one.3 == two.3
                        && BigInt::from((first.disp - second.disp).abs()) >= &one.3 * &trips
                        && left == right
                }
                _ => false,
            };
            if !apart {
                return None;
            }
        }
    }
    let mut fills = fills.into_iter();
    let (effect, pointer, byte, bytes) = fills.next()?;
    let extras = fills.map(|(effect, pointer, byte, bytes)| Extra { effect, pointer, byte, bytes }).collect();
    Some((effect, pointer, Stored::Fill(byte), bytes, extras))
}

/// Whether the one fill is cheaper than the loop for its trips, the proven
/// count or the profit model's estimate: `rep stos` pays its setup and each
/// cell, and a count of few cells is stores. Under `size` it is the code bytes
/// of the loop against the fill's.
fn _pays(
    unit: &Unit,
    callees: &Callees,
    chain: &[BlockId],
    header: BlockId,
    proof: &CountedLoop,
    costs: &OperationCosts,
    size: bool,
    moved: Option<(i64, bool)>,
) -> bool {
    let function = unit.function;
    let each: i64 = std::iter::once(&header)
        .chain(chain)
        .flat_map(|&block| operations(function, block))
        // In bytes the loop's counter work is the step and its branch: the phi
        // is a register, the address an operand, the test the step's flags.
        .filter(|&inst| {
            !size
                || !matches!(
                    function.instruction(inst).opcode,
                    Opcode::Phi | Opcode::GetElementPtr { .. } | Opcode::ICmp(_)
                )
        })
        .map(|inst| profit::operation(unit.context, unit.layout, function, callees, inst, costs).unwrap_or(costs.add))
        .sum();
    let known = proof.count.as_ref().and_then(ToPrimitive::to_i64);
    let most = proof.maximum.as_ref().and_then(ToPrimitive::to_i64);
    _cheaper(each, known, most, costs, size, moved)
}

/// Whether a fill beats a loop of `each` per trip, over `known` trips or, where
/// there are none, up to `most` of them. A copy of `moved` bytes a trip, `rep
/// movs` in dwords, `.1` where it runs down, beats one a few loads and stores.
fn _cheaper(
    each: i64,
    known: Option<i64>,
    most: Option<i64>,
    costs: &OperationCosts,
    size: bool,
    moved: Option<(i64, bool)>,
) -> bool {
    let trips = if size { 1 } else { known.unwrap_or_else(|| most.unwrap_or(i64::MAX).min(profit::UNKNOWN_TRIPS)) };
    // Isel expands a few cells, whatever the target is tuned for, to stores,
    // and a few bytes of a copy to loads and stores.
    let Some((bytes, backward)) = moved else {
        let string = costs.fill + trips * costs.fill_cell;
        let fill = match known {
            Some(count) if count <= 16 => count * costs.store,
            _ => string,
        };
        return trips * each > fill;
    };
    let string = costs.copy + (trips * bytes + 3) / 4 * costs.copy_cell + if backward { costs.direction } else { 0 };
    let pairs = known.map(|count| count * bytes).map(|length| length / 4 + i64::from((length % 4).count_ones()));
    let copy = match pairs {
        Some(pairs) if pairs <= 8 => pairs * (costs.load + costs.store),
        _ => string,
    };
    trips * each > copy
}

/// `llvm.memset` for pointers of `space` and lengths `width` bits wide,
/// declared where the module has none.
fn _memset(
    context: &mut Context,
    declared: &mut Declared,
    space: u32,
    width: u32,
) -> (GlobalId, TypeId) {
    let types = &mut context.types;
    let (void, pointer, byte, length, flag) =
        (types.void(), types.ptr(space), types.int(8), types.int(width), types.int(1));
    let ty =
        types.intern(Type::Function { returns: void, parameters: vec![pointer, byte, length, flag], variadic: false });
    (declared.declare(&format!("llvm.memset.p{space}.i{width}"), ty), ty)
}

/// `llvm.experimental.memset.pattern` of cells of type `cell` for pointers of
/// `space` and counts `width` bits wide, declared where the module has none.
fn _pattern(
    context: &mut Context,
    declared: &mut Declared,
    space: u32,
    cell: TypeId,
    width: u32,
) -> (GlobalId, TypeId) {
    let bits = context.types.int_bits(cell).expect("an integer cell");
    let types = &mut context.types;
    let (void, pointer, count, flag) = (types.void(), types.ptr(space), types.int(width), types.int(1));
    let ty =
        types.intern(Type::Function { returns: void, parameters: vec![pointer, cell, count, flag], variadic: false });
    (declared.declare(&format!("llvm.experimental.memset.pattern.p{space}.i{bits}.i{width}"), ty), ty)
}

/// `llvm.memcpy`, or `llvm.memmove` where the ranges overlap, from pointers of
/// space `from` to ones of `to`, in lengths `width` bits wide, declared where
/// the module has none.
pub(crate) fn _copy(
    context: &mut Context,
    declared: &mut Declared,
    how: How,
    to: u32,
    from: u32,
    width: u32,
) -> (GlobalId, TypeId) {
    let types = &mut context.types;
    let (void, destination, source, length, flag) =
        (types.void(), types.ptr(to), types.ptr(from), types.int(width), types.int(1));
    let ty = types.intern(Type::Function {
        returns: void,
        parameters: vec![destination, source, length, flag],
        variadic: false,
    });
    let name = if how == How::Apart { "memcpy" } else { "memmove" };
    (declared.declare(&format!("llvm.{name}.p{to}.p{from}.i{width}"), ty), ty)
}

/// The loop made one trip that fills.
fn _filled(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    found: &_Found,
) {
    let counter = found.proof.width();
    let mut seeds = Seeds { context, function, at: found.effect, width: counter };
    let trips =
        induction::trips(&found.proof, &mut |kind, args| seeds.computed(kind, args)).expect("a pre-tested proof");
    // The bytes are counted in the pointer's index: a narrower counter's trips,
    // never wrapped, widen.
    let width = found.memset.1;
    let counted = trips.clone();
    let trips = seeds.widened(&trips, width);
    seeds.width = width;
    let cells = matches!(found.stored, Stored::Fill(Byte::Pattern(..)));
    let count = if found.bytes == BigInt::from(1) || cells {
        trips.clone()
    } else {
        seeds.computed(BinaryOp::Mul, vec![trips.clone(), AffineOperand::constant(found.bytes.clone(), width)])
    };
    let extra_counts = found
        .extras
        .iter()
        .map(|extra| {
            if extra.bytes == BigInt::from(1) {
                trips.clone()
            } else {
                seeds.computed(BinaryOp::Mul, vec![trips.clone(), AffineOperand::constant(extra.bytes.clone(), width)])
            }
        })
        .collect::<Vec<_>>();
    seeds.width = counter;
    let finals = found
        .left
        .iter()
        .map(|(value, base, step, taken)| {
            let trips = if *taken == 0 {
                counted.clone()
            } else {
                seeds.computed(BinaryOp::Add, vec![counted.clone(), AffineOperand::constant(*taken, counter)])
            };
            let moved = if *step == BigInt::from(1) {
                trips
            } else {
                seeds.computed(BinaryOp::Mul, vec![trips, AffineOperand::constant(step.clone(), counter)])
            };
            (*value, seeds.computed(BinaryOp::Add, vec![AffineOperand::Value(*base, counter), moved]))
        })
        .collect::<IndexMap<_, _>>();
    seeds.width = width;
    let count = seeds.operand(&count);
    let extra_counts = extra_counts.iter().map(|count| seeds.operand(count)).collect::<Vec<_>>();
    let finals = finals.into_iter().map(|(value, sum)| (value, seeds.operand(&sum))).collect::<IndexMap<_, _>>();
    let void = seeds.context.types.void();
    let off = counting::constant(seeds.context, &BigInt::from(0), 1);
    let (callee, function_type, mut operands, direction) = match &found.stored {
        Stored::Fill(byte) => {
            let (callee, function_type) = match byte {
                Byte::Pattern(value) => {
                    let cell = seeds.function.operand_type(seeds.context, *value).expect("a typed cell");
                    _pattern(seeds.context, declared, found.memset.0, cell, found.memset.1)
                }
                _ => _memset(seeds.context, declared, found.memset.0, found.memset.1),
            };
            let byte = match *byte {
                Byte::Operand(byte) => byte,
                Byte::Number(byte) => counting::constant(seeds.context, &BigInt::from(byte), 8),
                Byte::Pattern(value) => value,
            };
            (callee, function_type, vec![found.pointer, byte, count], None)
        }
        Stored::Copy(copy) => {
            let (callee, function_type) =
                _copy(seeds.context, declared, copy.how, found.memset.0, copy.space, found.memset.1);
            // A loop that runs down starts at its last cell: the copy starts at
            // its first.
            let (to, from) = if copy.descending {
                let last = seeds.computed(BinaryOp::Sub, vec![trips.clone(), AffineOperand::constant(1, width)]);
                let back =
                    seeds.computed(BinaryOp::Mul, vec![last, AffineOperand::constant(-found.bytes.clone(), width)]);
                let back = seeds.operand(&back);
                let i8 = seeds.context.types.int(8);
                let mut lowered = |pointer: Operand| {
                    let ty = seeds.function.operand_type(seeds.context, pointer).expect("a typed pointer");
                    let gep = seeds
                        .function
                        .create_instruction(
                            Opcode::GetElementPtr { source: i8 },
                            ty,
                            vec![pointer, back],
                            Flags::default(),
                            None,
                        );
                    seeds.function.insert(gep, Position::Before(found.effect)).expect("a placed effect");
                    Operand::Value(seeds.function.instruction(gep).result.expect("a pointer"))
                };
                (lowered(found.pointer), lowered(copy.source))
            } else {
                (found.pointer, copy.source)
            };
            let direction = (copy.how == How::Overlapping).then_some(if copy.descending {
                llrm_mir::intrinsics::BACKWARD
            } else {
                llrm_mir::intrinsics::FORWARD
            });
            (callee, function_type, vec![to, from, count], direction)
        }
    };
    let pointer = seeds.context.types.ptr(0);
    operands.extend([
        off,
        Operand::Constant(seeds.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) })),
    ]);
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new(); 4],
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let info_again = info.clone();
    let call = function.create_instruction(Opcode::Call(Box::new(info)), void, operands, Flags::default(), None);
    function.insert(call, Position::Before(found.anchor)).expect("a placed effect");
    if let Some(kind) = direction {
        let node = declared.node(MetadataNode { distinct: false, operands: Vec::new() });
        function.annotate(call, kind, node);
    }
    function.erase(found.effect).expect("an effect has no result");
    // The loop's other fills, each a memset of its own by the same trips.
    for (extra, count) in found.extras.iter().zip(extra_counts) {
        let (callee, function_type) = _memset(context, declared, found.memset.0, found.memset.1);
        let byte = match extra.byte {
            Byte::Operand(byte) => byte,
            Byte::Number(byte) => counting::constant(context, &BigInt::from(byte), 8),
            Byte::Pattern(value) => value,
        };
        let pointer_type = context.types.ptr(0);
        let callee =
            Operand::Constant(context.constant(Constant { ty: pointer_type, kind: ConstantKind::Global(callee) }));
        let off = counting::constant(context, &BigInt::from(0), 1);
        let info = CallInfo { function_type, ..info_again.clone() };
        let call = function.create_instruction(
            Opcode::Call(Box::new(info)),
            void,
            vec![extra.pointer, byte, count, off, callee],
            Flags::default(),
            None,
        );
        function.insert(call, Position::Before(found.anchor)).expect("a placed effect");
        function.erase(extra.effect).expect("an effect has no result");
    }
    if let Stored::Copy(copy) = &found.stored {
        function.erase(copy.load).expect("its only user is the store");
    }

    // The exit's phis take each counter's exit value from the one trip.
    for phi in edges::phis(function, found.exit) {
        let mut incoming = arms(function, phi);
        let test = if found.posttested { found.latch } else { found.header };
        let Some(&(value, _)) = incoming.iter().find(|(_, from)| *from == test) else { continue };
        let value = match value {
            Operand::Value(one) => finals.get(&one).copied().unwrap_or(value),
            _ => value,
        };
        if found.posttested {
            for arm in incoming.iter_mut().filter(|(_, from)| *from == found.latch) {
                arm.0 = value;
            }
        } else {
            incoming.push((value, found.latch));
        }
        function.set_operands(phi, from_arms(&incoming));
    }
    let back = function.terminator(found.latch).expect("a latch branch");
    function.set_operands(back, vec![Operand::Block(found.exit)]);
    for phi in edges::phis(function, found.header) {
        let [(start, _)] =
            arms(function, phi).into_iter().filter(|(_, from)| *from != found.latch).collect::<Vec<_>>()[..]
        else {
            unreachable!("one preheader")
        };
        let result = function.instruction(phi).result.expect("a phi's value");
        function.replace_all_uses_with(result, start);
        function.set_operands(phi, Vec::new());
        function.erase(phi).expect("its uses were replaced");
    }
    // A proven positive count means the header's test passes on entry: it
    // guards nothing.
    if !found.posttested && found.proof.count.as_ref().is_some_and(|count| *count != BigInt::from(0)) {
        let test = function.terminator(found.header).expect("a header branch");
        function.set_operands(test, vec![Operand::Block(found.first)]);
        for phi in edges::phis(function, found.exit) {
            let kept = arms(function, phi).into_iter().filter(|(_, from)| *from != found.header).collect::<Vec<_>>();
            function.set_operands(phi, from_arms(&kept));
        }
    }
}

/// The loop's blocks after its header, when each has one way in and one out and
/// the last goes back.
fn _chain(
    function: &Function,
    header: BlockId,
    loop_: &Loop,
) -> Option<Vec<BlockId>> {
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

/// The blocks of a loop tested after its trips: the header, then each block
/// with one way on, to the latch, which tests.
fn _chain_after(
    function: &Function,
    header: BlockId,
    loop_: &Loop,
) -> Option<Vec<BlockId>> {
    let mut chain = vec![header];
    loop {
        let here = *chain.last().expect("a chain has blocks");
        let inside: Vec<BlockId> =
            function.successors(here).into_iter().filter(|to| loop_.body.contains(&cfg::id(*to))).collect();
        match inside[..] {
            [next] if next == header => break,
            [next]
                if function.successors(here).len() == 1
                    && !chain.contains(&next)
                    && edges::phis(function, next).is_empty() =>
            {
                chain.push(next)
            }
            _ => return None,
        }
    }
    (chain.len() == loop_.body.len() && function.successors(*chain.last().expect("a chain has blocks")).len() == 2)
        .then_some(chain)
}

/// A store's or a memset's pointer, the byte it fills with, and its bytes,
/// where its value is invariant and one byte repeated.
fn _stored(
    unit: &Unit,
    callees: &Callees,
    effect: InstId,
    still: &induction::Invariant,
) -> Option<(Operand, Byte, BigInt)> {
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
    // LLVM's memset_pattern16: the cell is a word or dword; the stored value
    // need not be a constant.
    matches!(width, 16 | 32).then_some((pointer, Byte::Pattern(value), bytes))
}

/// The store and load of `one` and `other`, where the store writes the loaded
/// cell and the load has no other user: its pointer, the copy, and the bytes of
/// a cell.
fn _copied(
    unit: &Unit,
    one: InstId,
    other: InstId,
) -> Option<(InstId, Operand, Stored, BigInt)> {
    let function = unit.function;
    let (load, store) =
        if matches!(function.instruction(one).opcode, Opcode::Load { .. }) { (one, other) } else { (other, one) };
    let (Opcode::Load { volatile: false, .. }, Opcode::Store { volatile: false, .. }) =
        (&function.instruction(load).opcode, &function.instruction(store).opcode)
    else {
        return None;
    };
    let loaded = function.instruction(load).result?;
    if function.instruction(store).operands[0] != Operand::Value(loaded) || function.users(loaded).len() != 1 {
        return None;
    }
    let bits = unit.int_bits(Operand::Value(loaded))?;
    if bits % 8 != 0 || bits == 0 {
        return None;
    }
    let (source, pointer) = (function.instruction(load).operands[0], function.instruction(store).operands[1]);
    let copy = Copy { load, source, how: How::Apart, descending: false, space: unit.space(source)? };
    Some((store, pointer, Stored::Copy(copy), BigInt::from(bits / 8)))
}

/// How a loop's two walks, `from` read and `to` written, `bytes` a trip, let
/// one copy stand for the trips in order; none where they do not.
///
/// The alias analysis owns what is apart: two objects, or a `restrict`. Of
/// one object, two walks of one stride a constant `d` apart bytes (to less
/// from) are apart past the trips' span, and else run in order only when the
/// writes trail the reads: `d <= 0` going up, `d >= 0` going down. Any other
/// overlap makes each trip read what an earlier one wrote: a smear, no copy.
fn _overlap(
    unit: &Unit,
    load: InstId,
    store: InstId,
    from: &induction::Recurrence,
    to: &induction::Recurrence,
    proof: &CountedLoop,
    bytes: &BigInt,
    descending: bool,
) -> Option<How> {
    let references = unit.annotated().ok();
    if let Some((read, written)) = references.as_ref().and_then(|found| found.get(&load).zip(found.get(&store)))
        && !llrm_analysis::regions::overlapping(read, written, None, None, unit.program).unwrap_or(true)
    {
        return Some(How::Apart);
    }
    if from.pointer != to.pointer {
        return None;
    }
    let apart = to.start.minus(&from.start).known()?;
    let trips = proof.count.as_ref().or(proof.maximum.as_ref());
    if trips.is_some_and(|trips| apart.magnitude() >= (trips * bytes).magnitude()) {
        return Some(How::Apart);
    }
    let zero = BigInt::from(0);
    ((!descending && apart <= zero) || (descending && apart >= zero)).then_some(How::Overlapping)
}

/// The header phi `inst` steps, if it is one's step: a constant added, the
/// phi's value from the latch.
fn _stepped(
    unit: &Unit,
    phis: &[InstId],
    latch: BlockId,
    inst: InstId,
) -> Option<ValueId> {
    let function = unit.function;
    let op = function.instruction(inst);
    let (AffineOperand::Value(stepped, _), AffineOperand::Const(_)) = induction::stepping(unit, op)? else {
        return None;
    };
    let phi = phis.iter().copied().find(|&phi| function.instruction(phi).result == Some(stepped))?;
    (arms(function, phi).iter().any(|&(value, from)| from == latch && Some(value) == op.result.map(Operand::Value)))
        .then_some(stepped)
}

#[cfg(test)]
#[path = "fill_tests.rs"]
mod tests;
