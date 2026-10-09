//! A counted loop's huge pointers split into windows: LLVM's IRCE, with the
//! carry into the selector as the check that cannot fire inside one.
//!
//! An outer loop walks the windows. At each one every huge recurrence is
//! normalized (`Intrinsic::Window`), so its offset starts below one selector
//! step, and the loop runs as many whole trips as the target's window
//! (`Machine::huge_window`) holds for every recurrence, or the trips left,
//! on far pointers that never carry. Where the trips provably fit one
//! window there is no outer loop: the start is normalized once.
//!
//! The trips are `induction`'s proof, the recurrences its `pointers`. The
//! split is priced: a window's setup against the carries it saves.

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, CountedLoop, PointerRecurrence};
use llrm_analysis::manager::Registers;
use llrm_analysis::{cfg, memory};
use llrm_mir::context::{Constant, ConstantKind, Context, GlobalId};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, MetadataNode, MetadataOperand, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::target::OperationCosts;
use llrm_mir::types::{Type, TypeId};
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::counting::{self, Seeds};
use crate::profit;

pub struct Window {
    /// Priced in bytes, not time.
    pub size: bool,
}

impl FunctionPass for Window {
    fn name(&self) -> &'static str {
        "window"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if windowed(unit, analyses, &outer, self.size) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// A huge recurrence the windows carry, its accesses, and the bytes they
/// reach from the phi's value, `low` inclusive.
struct Walk {
    recurrence: PointerRecurrence,
    /// Each access through the phi or a constant step from it, by that step.
    reads: Vec<(InstId, usize, i64)>,
    low: i64,
    high: i64,
}

impl Walk {
    fn step(&self) -> i64 {
        self.recurrence.step.to_i64().expect("a checked step")
    }

    /// What `n` trips reach beyond `n` steps. The far recurrence follows
    /// the lowest byte the accesses read; its `n + 1` values name bytes of
    /// the window too, so the last one, carried to the next window, never
    /// wraps. A window holds `(window - fixed) / |step|` trips.
    fn fixed(&self) -> i64 {
        let (step, span) = (self.step(), self.high - self.low);
        if step > 0 { (span - step).max(1) } else { span }
    }
}

/// A loop to split.
struct Found {
    preheader: BlockId,
    header: BlockId,
    latch: BlockId,
    /// The header's successor in the loop, and its exit.
    body: BlockId,
    exit: BlockId,
    proof: CountedLoop,
    walks: Vec<Walk>,
    /// The most whole trips one window holds for every walk.
    most: i64,
    /// The trips provably fit one window.
    one: bool,
}

/// Each loop's huge recurrences split into windows, where that pays.
pub fn windowed(
    unit: &mut Unit,
    analyses: &Analyses,
    outer: &Outer,
    size: bool,
) -> bool {
    let Some((far, window)) = outer.target().huge_window() else { return false };
    let costs = if size { outer.target().size_costs() } else { outer.target().costs() };
    let mut done = std::collections::BTreeSet::new();
    let mut changed = false;
    loop {
        let mut fresh = analyses.fresh();
        let facts = fresh.get::<Registers>(unit.context, unit.layout, unit.function);
        let shape = fresh.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
        let found = {
            let view = memory::Unit::within(unit.context, unit.layout, unit.function, outer)
                .with_registers(&facts)
                .with_shape(&shape);
            let loops = shape.loops.clone();
            let mut found = None;
            for one in &loops {
                if done.insert(one.header) {
                    found = _found(&view, one, window).filter(|found| _pays(found, &costs, size));
                    if found.is_some() {
                        break;
                    }
                }
            }
            found
        };
        let Some(found) = found else { break };
        _split(unit, far, &found);
        crate::dead::dead(unit.context, outer.callees(), unit.function);
        changed = true;
    }
    changed
}

fn _preheader(
    function: &Function,
    loop_: &Loop,
) -> Option<BlockId> {
    let header = cfg::block(loop_.header);
    let outside = function
        .predecessors(header)
        .into_iter()
        .filter(|&one| !loop_.body.contains(&cfg::id(one)))
        .collect::<Vec<_>>();
    match outside[..] {
        [one] if function.successors(one) == [header] => Some(one),
        _ => None,
    }
}

/// `recurrence` and its accesses, where it is read only by them and its step.
fn _walk(
    view: &memory::Unit,
    loop_: &Loop,
    recurrence: PointerRecurrence,
) -> Option<Walk> {
    let function = view.function;
    let within = |inst: InstId| function.parent(inst).is_some_and(|block| loop_.body.contains(&cfg::id(block)));
    recurrence.step.to_i64()?;
    let stepped = function.instruction(recurrence.stepping).result?;
    if function.users(stepped).iter().any(|one| one.user != recurrence.phi) {
        return None;
    }
    let mut reads = Vec::new();
    let (mut low, mut high) = (i64::MAX, i64::MIN);
    let mut read = |user: InstId, index: usize, by: i64, reads: &mut Vec<(InstId, usize, i64)>| -> Option<()> {
        let op = function.instruction(user);
        let ty = match (&op.opcode, index) {
            (Opcode::Load { .. }, 0) => op.ty,
            (Opcode::Store { .. }, 1) => function.operand_type(view.context, op.operands[0])?,
            _ => return None,
        };
        let width = i64::try_from(view.layout.store_size(&view.context.types, ty)).ok()?;
        (low, high) = (low.min(by), high.max(by + width));
        reads.push((user, index, by));
        Some(())
    };
    for one in function.users(recurrence.value) {
        if one.user == recurrence.stepping {
            continue;
        }
        if !within(one.user) {
            return None;
        }
        let op = function.instruction(one.user);
        match op.opcode {
            Opcode::GetElementPtr { source } if one.index == 0 => {
                let indices = op.operands[1..]
                    .iter()
                    .map(|&index| {
                        view.int_constant(index)
                            .map(|bits| llrm_mir::context::signed(bits, view.int_bits(index).unwrap_or(128)))
                    })
                    .collect::<Vec<_>>();
                let (by, variable) = view.layout.collect_offset(&view.context.types, source, &indices);
                if !variable.is_empty() || indices.iter().any(Option::is_none) {
                    return None;
                }
                let address = op.result?;
                for access in function.users(address) {
                    read(access.user, access.index as usize, i64::try_from(by).ok()?, &mut reads)?;
                }
            }
            _ => read(one.user, one.index as usize, 0, &mut reads)?,
        }
    }
    (!reads.is_empty()).then_some(Walk { recurrence, reads, low, high })
}

fn _found(
    view: &memory::Unit,
    loop_: &Loop,
    window: i64,
) -> Option<Found> {
    let function = view.function;
    let preheader = _preheader(function, loop_)?;
    let header = cfg::block(loop_.header);
    let latches: Vec<i64> = loop_.latches.iter().copied().collect();
    let [latch] = latches[..] else { return None };
    let latch = cfg::block(latch);
    let inside = |block: BlockId| loop_.body.contains(&cfg::id(block));
    let walks: Vec<Walk> = induction::pointers(view, loop_)
        .into_iter()
        .filter(|one| view.space(Operand::Value(one.value)).is_some_and(|space| view.layout.carries(space)))
        .filter_map(|one| _walk(view, loop_, one))
        .collect();
    if walks.is_empty() {
        return None;
    }
    // The proof's test leaves: the header's before each trip, or the latch's
    // after.
    let proofs = induction::counted(view, loop_, Some(&view.registers()), false);
    let tested = |one: &CountedLoop, block: BlockId| function.terminator(block) == Some(one.branch);
    let proof = proofs
        .into_iter()
        .find(|one| !one.stops && (tested(one, header) && !one.posttested || tested(one, latch) && one.posttested))?;
    let most = walks.iter().map(|walk| (window - walk.fixed()).div_euclid(walk.step().abs())).min()?;
    if most < 2 {
        return None;
    }
    // One window changes no block: the loop keeps its own test.
    if proof.count.as_ref().and_then(ToPrimitive::to_i64).is_some_and(|count| count <= most) {
        return Some(Found { preheader, header, latch, body: header, exit: header, proof, walks, most, one: true });
    }
    // The trips must be placeable before the loop.
    if proof.count.is_none() && induction::trips(&proof, &mut |_, args| args[0].clone()).is_none() {
        return None;
    }
    // The test's block alone leaves, to one exit.
    let test = if proof.posttested { latch } else { header };
    let (body, exit) = match function.successors(test)[..] {
        [one, other] if inside(one) && !inside(other) => (one, other),
        [one, other] if inside(other) && !inside(one) => (other, one),
        _ => return None,
    };
    for &block in &loop_.body {
        let block = cfg::block(block);
        if block != test && function.successors(block).into_iter().any(|one| !inside(one)) {
            return None;
        }
    }
    // What the loop computes is read outside only by the exit's phis. A
    // window leaves from its latch: before each trip, only a header phi's
    // value is known there; after, anything the latch reaches.
    for &block in &loop_.body {
        for &inst in function.block(cfg::block(block)).instructions() {
            let Some(result) = function.instruction(inst).result else { continue };
            for one in function.users(result) {
                let Some(at) = function.parent(one.user) else { continue };
                if inside(at) {
                    continue;
                }
                let phi = function.instruction(one.user);
                let header_phi =
                    function.parent(inst) == Some(header) && function.instruction(inst).opcode == Opcode::Phi;
                if at != exit
                    || phi.opcode != Opcode::Phi
                    || phi.operands.get(one.index as usize + 1) != Some(&Operand::Block(test))
                    || !(header_phi || proof.posttested)
                {
                    return None;
                }
            }
        }
    }
    Some(Found { preheader, header, latch, body, exit, proof, walks, most, one: false })
}

/// Whether the carries a split saves outweigh its windows' setup: each
/// window normalizes every walk and, past the first, steps it on as a huge
/// pointer, besides its count. Time is weighed over the trips, or the loop's
/// estimate where they are unknown; size once.
fn _pays(
    found: &Found,
    costs: &OperationCosts,
    size: bool,
) -> bool {
    let walks = found.walks.len() as i64;
    let saved = walks * costs.carry_step;
    let normalize = walks * 3 * costs.add;
    let setup =
        if found.one { normalize } else { normalize + walks * costs.carry_step + 4 * costs.add + 2 * costs.branch };
    if size {
        return saved > setup;
    }
    let trips = _expected(found);
    let windows = (trips + found.most - 1) / found.most;
    trips * saved > windows.max(1) * setup
}

/// The trips the loop makes, or is expected to.
fn _expected(found: &Found) -> i64 {
    let known = found.proof.count.as_ref().and_then(ToPrimitive::to_i64);
    known.unwrap_or_else(|| {
        found.proof.maximum.as_ref().and_then(ToPrimitive::to_i64).unwrap_or(i64::MAX).min(profit::UNKNOWN_TRIPS)
    })
}

/// `branch` taken to its first target `staying` times for each time to its
/// second.
fn _weighed(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    branch: InstId,
    staying: i64,
) {
    let word = context.types.int(32);
    let weight = |context: &mut Context, n: i64| MetadataOperand::Constant(context.int(word, i128::from(n.max(0))));
    let operands =
        vec![MetadataOperand::String("branch_weights".to_owned()), weight(context, staying), weight(context, 1)];
    let node = declared.node(MetadataNode { distinct: false, operands });
    function.annotate(branch, "prof", node);
}

fn _placed(
    function: &mut Function,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
    at: Position,
) -> Operand {
    let inst = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(inst, at).expect("a placed position");
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

fn _phi(
    function: &mut Function,
    ty: TypeId,
    block: BlockId,
    name: &str,
) -> (InstId, Operand) {
    let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), Some(name));
    let at =
        function.block(block).instructions().first().map_or(Position::End(block), |&first| Position::Before(first));
    function.insert(phi, at).expect("a placed phi");
    (phi, Operand::Value(function.instruction(phi).result.expect("a phi's value")))
}

/// `found` split: an outer loop over windows around the loop on far pointers,
/// or, for one window, the start normalized before it.
fn _split(
    unit: &mut Unit,
    space: u32,
    found: &Found,
) {
    let (context, declared, function, layout) =
        (&mut *unit.context, &mut *unit.declared, &mut *unit.function, unit.layout);
    let far = context.types.ptr(space);
    let far_index = layout.pointer(space).index_bits;
    let entering = function.terminator(found.preheader).expect("a preheader's branch");
    if found.one {
        // One window: each walk from its start, the loop's own test kept.
        let count = found.proof.count.clone().expect("one window has a count");
        for walk in &found.walks {
            let huge = function.value(walk.recurrence.value).ty;
            let Type::Pointer(huge_space) = *context.types.get(huge) else { unreachable!("a pointer recurrence") };
            let trips = counting::constant(context, &count, layout.pointer(huge_space).index_bits);
            let origin = _windowed(
                context,
                declared,
                function,
                layout,
                far,
                walk,
                walk.recurrence.start,
                trips,
                Position::Before(entering),
            );
            let _ = _far(context, function, walk, origin, found.preheader, found.header, far, far_index);
        }
        _erased(function, found);
        return;
    }
    let width = found.proof.width();
    let count_ty = context.types.int(width);
    let word = context.types.int(16);
    // The trips, before the loop: none skips it.
    let mut seeds = Seeds { context: &mut *context, function: &mut *function, at: entering, width };
    let trips = match &found.proof.count {
        Some(count) => induction::AffineOperand::constant(count.clone(), width),
        None => {
            induction::trips(&found.proof, &mut |kind, args| seeds.computed(kind, args)).expect("trips _found placed")
        }
    };
    let trips = seeds.operand(&trips);
    let skip = if found.proof.count.is_none() && !found.proof.entry_guarded {
        counting::skip_guard(&mut seeds, &found.proof)
    } else {
        None
    };
    // Outer header, and its latch.
    let windows = function.create_block(Some("windows"));
    function.insert_block(windows, Some(found.preheader)).expect("a new block");
    let next_window = function.create_block(Some("window.next"));
    function.insert_block(next_window, Some(found.latch)).expect("a new block");
    // Every header phi is carried across windows; a walk's as its huge pointer.
    let header_phis: Vec<InstId> = function
        .block(found.header)
        .instructions()
        .iter()
        .copied()
        .take_while(|&one| function.instruction(one).opcode == Opcode::Phi)
        .collect();
    let walked: Vec<InstId> = found.walks.iter().map(|walk| walk.recurrence.phi).collect();
    let mut carried = Vec::new();
    for &phi in &header_phis {
        let ty = function.value(function.instruction(phi).result.expect("a phi's value")).ty;
        let (outer, value) = _phi(function, ty, windows, "carried");
        carried.push((phi, outer, value));
    }
    let (remaining_phi, remaining) = _phi(function, count_ty, windows, "remaining");
    let at =
        |function: &Function, block: BlockId| Position::Before(function.terminator(block).expect("a terminated block"));
    // This window's trips: those left, or the most a window holds.
    let most = counting::constant(context, &BigInt::from(found.most), width);
    let bit = context.types.int(1);
    function.set_operands(entering, vec![Operand::Block(windows)]);
    let jump = function.create_instruction(
        Opcode::Br,
        context.types.void(),
        vec![Operand::Block(found.header)],
        Flags::default(),
        None,
    );
    function.insert(jump, Position::End(windows)).expect("a new block");
    let fewer = _placed(function, Opcode::ICmp(IntPredicate::Ult), bit, vec![remaining, most], Position::Before(jump));
    let trips_here = _placed(function, Opcode::Select, count_ty, vec![fewer, remaining, most], Position::Before(jump));
    let counted16 = _resized(context, function, trips_here, width, 16, false, Position::Before(jump));
    let counted32 = _resized(context, function, trips_here, width, 32, false, Position::Before(jump));
    // Each walk normalized where this window starts.
    let mut stepped_on = Vec::new();
    for (walk, &(_, _, outer_value)) in found
        .walks
        .iter()
        .map(|walk| (walk, carried.iter().find(|(phi, ..)| *phi == walk.recurrence.phi).expect("a carried walk")))
    {
        let origin =
            _windowed(context, declared, function, layout, far, walk, outer_value, counted32, Position::Before(jump));
        let last = _far(context, function, walk, origin, windows, found.header, far, far_index);
        // The next window's start: the far recurrence past the last trip,
        // which the window holds, less the accesses' lowest byte. The huge
        // pointer is not held across the window.
        let huge = function.value(walk.recurrence.value).ty;
        let reached =
            _placed(function, Opcode::Cast(CastOp::AddrSpaceCast), huge, vec![last], Position::End(next_window));
        let next = if walk.low == 0 {
            reached
        } else {
            let back = counting::constant(context, &BigInt::from(-walk.low), 32);
            let byte = context.types.int(8);
            _placed(
                function,
                Opcode::GetElementPtr { source: byte },
                huge,
                vec![reached, back],
                Position::End(next_window),
            )
        };
        stepped_on.push((walk.recurrence.phi, next));
    }
    // The inner loop counts this window's trips, in place of its own test:
    // a window runs one at least, so it tests after each.
    let (count_phi, count) = _phi(function, word, found.header, "window.trips");
    let one = counting::constant(context, &BigInt::from(1), 16);
    let counted_down =
        _placed(function, Opcode::Binary(BinaryOp::Sub), word, vec![count, one], at(function, found.latch));
    function
        .set_operands(count_phi, vec![counted16, Operand::Block(windows), counted_down, Operand::Block(found.latch)]);
    let zero16 = counting::constant(context, &BigInt::from(0), 16);
    let goes_on =
        _placed(function, Opcode::ICmp(IntPredicate::Ne), bit, vec![counted_down, zero16], at(function, found.latch));
    let latch_branch = function.terminator(found.latch).expect("a latch's branch");
    function.set_operands(latch_branch, vec![goes_on, Operand::Block(found.header), Operand::Block(next_window)]);
    // What the branches are known to do, for the blocks' frequencies.
    let expected = _expected(found);
    let windows_run = ((expected + found.most - 1) / found.most).max(1);
    _weighed(context, declared, function, latch_branch, (expected + windows_run - 1) / windows_run - 1);
    if !found.proof.posttested {
        let branch = function.terminator(found.header).expect("a header's branch");
        function.set_operands(branch, vec![Operand::Block(found.body)]);
    }
    // The outer latch: the trips left, and the next window or the exit.
    // Taken before the window runs, so its trips need not outlive it.
    let left =
        _placed(function, Opcode::Binary(BinaryOp::Sub), count_ty, vec![remaining, trips_here], Position::Before(jump));
    let zero = counting::constant(context, &BigInt::from(0), width);
    let more = _placed(function, Opcode::ICmp(IntPredicate::Ne), bit, vec![left, zero], Position::End(next_window));
    let back = function.create_instruction(
        Opcode::Br,
        context.types.void(),
        vec![more, Operand::Block(windows), Operand::Block(found.exit)],
        Flags::default(),
        None,
    );
    function.insert(back, Position::End(next_window)).expect("a new block");
    _weighed(context, declared, function, back, windows_run - 1);
    function
        .set_operands(remaining_phi, vec![trips, Operand::Block(found.preheader), left, Operand::Block(next_window)]);
    // The carried values: a header phi's start, then its value as a window
    // ends, its latch's; a walk's, its huge pointer stepped on.
    let arm_of = |function: &Function, phi: InstId, block: BlockId| {
        function.instruction(phi).operands.chunks(2).find(|pair| pair[1] == Operand::Block(block)).expect("an arm")[0]
    };
    let mut endings = Vec::new();
    for &(phi, _, _) in &carried {
        let ending = stepped_on
            .iter()
            .find(|(one, _)| *one == phi)
            .map_or_else(|| arm_of(function, phi, found.latch), |(_, next)| *next);
        endings.push((Operand::Value(function.instruction(phi).result.expect("a value")), ending));
    }
    for (&(phi, outer, outer_value), &(_, ending)) in carried.iter().zip(&endings) {
        let start = arm_of(function, phi, found.preheader);
        function.set_operands(outer, vec![start, Operand::Block(found.preheader), ending, Operand::Block(next_window)]);
        if !walked.contains(&phi) {
            let arms = function.instruction(phi).operands.clone();
            let arms = arms
                .chunks(2)
                .flat_map(|pair| {
                    if pair[1] == Operand::Block(found.preheader) {
                        [outer_value, Operand::Block(windows)]
                    } else {
                        [pair[0], pair[1]]
                    }
                })
                .collect();
            function.set_operands(phi, arms);
        }
    }
    // The exit is left from the outer latch, with what the header held then;
    // a walk's huge pointer, past its last window. A skipped loop leaves with
    // the starts.
    let test = if found.proof.posttested { found.latch } else { found.header };
    let exit_phis: Vec<InstId> = function
        .block(found.exit)
        .instructions()
        .iter()
        .copied()
        .take_while(|&one| function.instruction(one).opcode == Opcode::Phi)
        .collect();
    for phi in exit_phis {
        let mut arms = function.instruction(phi).operands.clone();
        let mut skipped = None;
        for pair in arms.chunks_mut(2) {
            if pair[1] != Operand::Block(test) {
                continue;
            }
            skipped = Some(_entry_value(function, pair[0], &carried, found.preheader));
            pair[1] = Operand::Block(next_window);
            // Tested before each trip, the header held the next trip's values:
            // the latch's.
            if let Some(&(_, ending)) =
                endings.iter().find(|(value, _)| *value == pair[0]).filter(|_| !found.proof.posttested)
            {
                pair[0] = ending;
            }
        }
        if let (Some(_), Some(value)) = (skip, skipped) {
            arms.extend([value, Operand::Block(found.preheader)]);
        }
        function.set_operands(phi, arms);
    }
    if let Some(skip) = skip {
        function
            .set_operands(entering, vec![Operand::Value(skip), Operand::Block(found.exit), Operand::Block(windows)]);
    }
    _erased(function, found);
}

/// What an exit value is where the loop runs no trip: a header phi's
/// start, or itself where it is not the loop's.
fn _entry_value(
    function: &Function,
    value: Operand,
    carried: &[(InstId, InstId, Operand)],
    preheader: BlockId,
) -> Operand {
    let start_of = |phi: InstId| {
        function
            .instruction(phi)
            .operands
            .chunks(2)
            .find(|pair| pair[1] == Operand::Block(preheader))
            .map(|pair| pair[0])
    };
    let Operand::Value(id) = value else { return value };
    let ValueDef::Instruction(inst) = function.value(id).def else { return value };
    carried.iter().find(|(phi, ..)| *phi == inst).and_then(|&(_, outer, _)| start_of(outer)).unwrap_or(value)
}

/// `value`, `from` bits wide, as `to` bits.
fn _resized(
    context: &mut Context,
    function: &mut Function,
    value: Operand,
    from: u32,
    to: u32,
    signed: bool,
    at: Position,
) -> Operand {
    let ty = context.types.int(to);
    match from.cmp(&to) {
        std::cmp::Ordering::Equal => value,
        std::cmp::Ordering::Greater => _placed(function, Opcode::Cast(CastOp::Trunc), ty, vec![value], at),
        std::cmp::Ordering::Less => {
            _placed(function, Opcode::Cast(if signed { CastOp::SExt } else { CastOp::ZExt }), ty, vec![value], at)
        }
    }
}

/// Where `walk` starts a window from its huge pointer `start`: the lowest
/// byte its `trips` (of its huge index's width) reach, normalized, then on
/// to the first trip's lowest byte, far. A walk up reaches its lowest on its
/// first trip; a walk down, on its last.
#[allow(clippy::too_many_arguments)]
fn _windowed(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    layout: &llrm_mir::datalayout::DataLayout,
    far: TypeId,
    walk: &Walk,
    start: Operand,
    trips: Operand,
    at: Position,
) -> Operand {
    let huge = function.value(walk.recurrence.value).ty;
    let Type::Pointer(huge_space) = *context.types.get(huge) else { unreachable!("a pointer recurrence") };
    let Type::Pointer(far_space) = *context.types.get(far) else { unreachable!("a far pointer") };
    let (huge_index, far_index) = (layout.pointer(huge_space).index_bits, layout.pointer(far_space).index_bits);
    let byte = context.types.int(8);
    let gep = Opcode::GetElementPtr { source: byte };
    let step = walk.step();
    // The lowest byte reached, from `start`: `low`, and for a backward walk its
    // trips less one on. Walking down, the far recurrence passes below the
    // last trip's bytes: one step more.
    let lowest = if step < 0 {
        let dword = context.types.int(huge_index);
        let by = counting::constant(context, &BigInt::from(step), huge_index);
        let bytes = _placed(function, Opcode::Binary(BinaryOp::Mul), dword, vec![trips, by], at);
        let low = counting::constant(context, &BigInt::from(walk.low), huge_index);
        Some(_placed(function, Opcode::Binary(BinaryOp::Add), dword, vec![bytes, low], at))
    } else {
        (walk.low != 0).then(|| counting::constant(context, &BigInt::from(walk.low), huge_index))
    };
    let base = lowest.map_or(start, |lowest| _placed(function, gep.clone(), huge, vec![start, lowest], at));
    let (callee, function_type) = _declared(context, declared, far, huge);
    let ty = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Global(callee) }));
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new()],
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let origin = _placed(function, Opcode::Call(Box::new(info)), far, vec![base, callee], at);
    // The far recurrence starts at the first trip's lowest byte: the origin
    // itself walking up, its trips on walking down.
    if step > 0 {
        return origin;
    }
    let lowest = _resized(context, function, lowest.expect("a walk down's lowest"), huge_index, far_index, true, at);
    let index = context.types.int(far_index);
    let low = counting::constant(context, &BigInt::from(walk.low), far_index);
    let back = _placed(function, Opcode::Binary(BinaryOp::Sub), index, vec![low, lowest], at);
    _placed(function, gep, far, vec![origin, back], at)
}

/// `walk`'s accesses on a far recurrence from `origin` that follows their
/// lowest byte, entered from `entry`, stepped where the huge one was; its
/// stepped value.
#[allow(clippy::too_many_arguments)]
fn _far(
    context: &mut Context,
    function: &mut Function,
    walk: &Walk,
    origin: Operand,
    entry: BlockId,
    header: BlockId,
    far: TypeId,
    far_index: u32,
) -> Operand {
    let recurrence = &walk.recurrence;
    let byte = context.types.int(8);
    let gep = Opcode::GetElementPtr { source: byte };
    let (phi, value) = _phi(function, far, header, "window");
    let step = counting::constant(context, &recurrence.step, far_index);
    let next = _placed(function, gep.clone(), far, vec![value, step], Position::Before(recurrence.stepping));
    let arms = function.instruction(recurrence.phi).operands.clone();
    let operands = arms
        .chunks(2)
        .flat_map(|pair| {
            if function.parent(recurrence.stepping).is_some_and(|_| {
                pair[0] == Operand::Value(function.instruction(recurrence.stepping).result.expect("a step"))
            }) {
                [next, pair[1]]
            } else {
                [origin, Operand::Block(entry)]
            }
        })
        .collect();
    function.set_operands(phi, operands);
    for &(user, index, by) in &walk.reads {
        let by = by - walk.low;
        let address = if by == 0 {
            value
        } else {
            let by = counting::constant(context, &BigInt::from(by), far_index);
            _placed(function, gep.clone(), far, vec![value, by], Position::Before(user))
        };
        function.set_operand(user, index, address);
    }
    next
}

/// Each walk's huge recurrence, read by nothing now.
fn _erased(
    function: &mut Function,
    found: &Found,
) {
    for walk in &found.walks {
        // The constant steps its accesses read through, left unread.
        let value = function.instruction(walk.recurrence.phi).result.expect("a phi's value");
        let steps: Vec<InstId> =
            function.users(value).iter().map(|one| one.user).filter(|&user| user != walk.recurrence.stepping).collect();
        for step in steps {
            function.erase(step).expect("an unread step");
        }
        function.set_operands(walk.recurrence.phi, Vec::new());
        function.erase(walk.recurrence.stepping).expect("its phi was emptied");
        function.erase(walk.recurrence.phi).expect("its readers were replaced");
    }
}

/// `Intrinsic::Window` from `huge` pointers to `far` ones, declared where the
/// module has none.
fn _declared(
    context: &mut Context,
    declared: &mut Declared,
    far: TypeId,
    huge: TypeId,
) -> (GlobalId, TypeId) {
    let ty = context.types.intern(Type::Function { returns: far, parameters: vec![huge], variadic: false });
    (declared.declare(&llrm_mir::intrinsics::window_name(&context.types, far, huge), ty), ty)
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
