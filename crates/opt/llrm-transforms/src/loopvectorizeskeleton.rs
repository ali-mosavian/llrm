//! The control flow around a widened loop: the scalar remainder
//! (`InnerLoopVectorizer::createVectorLoopSkeleton`).
//!
//! # How it works
//! The pin keeps the original loop as the scalar remainder and runs a vector
//! loop of `VF x IC` lanes a step in front of it. Here the vector loop is the
//! original loop widened in place and the remainder is a copy of the loop made
//! first, so [`add_remainder`] runs before the widening. The vector trip count
//! is the trip count less its remainder, `n.vec`; the remainder starts where
//! the vector loop ends.
//!
//! ```text
//! preheader:                              preheader:
//!   br loop                                 %n = sub %bound, %start
//!                                           %min.iters.check = icmp ult %n, VF*IC
//! loop:                      =>             br %min.iters.check, scalar.ph, vector.ph
//!   %i = phi [%start, ..], ..             vector.ph:
//!   ..                                      %n.mod.vf = and %n, VF*IC-1
//!   %c = icmp eq %i.next, %bound            %n.vec = sub %n, %n.mod.vf
//!   br %c, exit, loop                       %ind.end = add %start, %n.vec
//!                                         loop:  (widened, compares to %ind.end)
//!                                         middle.block:
//!                                           %cmp.n = icmp eq %n, %n.vec
//!                                           br %cmp.n, exit, scalar.ph
//!                                         scalar.ph:
//!                                           %bc.resume.val = phi [%ind.end, middle.block], [%start, preheader]
//!                                         loop.scalar:  (the original loop, from %bc.resume.val)
//! ```
//!
//! With both bounds constant the checks fold, as the pin's builder folds them:
//! the preheader falls into the vector loop, which leaves into the remainder.
//!
//! # DEVIATE rows
//! - LVS1 the trip count is `bound - start` written as an instruction, not the
//!   SCEV expansion of the backedge-taken count plus one (the two are one value
//!   for the loops accepted: an induction stepping by one against an `eq`/`ne`
//!   exit test). Exit: the vector trip count from the SCEV expander when loops
//!   with a computed trip count are accepted.
//! - LVS2 the remainder is the scalar loop only (VP3).
//! - LVS3 the exit block's phis take invariant values only, but the final value
//!   of a reduction (the interleaved loops' `bin.rdx`, LVI2), which the middle
//!   block computes; any other loop-defined value used outside is refused by
//!   the recipe extraction, LVP2.

use llrm_analysis::runtimecheck::{RuntimePointerChecking, VectorizerParams};
use llrm_analysis::scev::expander::Expander;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::module::{BlockId, Function, InstId, MetadataId, MetadataNode, MetadataOperand, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::types::{Type, TypeId};

use crate::edges;
use crate::lcssa::{arms, from_arms};
use crate::runtimechecks::Rewriter;

/// The loop the remainder is made for.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shape {
    pub header: BlockId,
    pub preheader: BlockId,
    /// The one block the loop leaves to.
    pub exit: BlockId,
    /// The induction phi, stepping by one (by minus one if `reverse`).
    pub primary: InstId,
    /// The exit test and the operand of it that is the loop-invariant bound.
    pub compare: InstId,
    pub bound_operand: usize,
    pub index_ty: TypeId,
    /// The induction counts down.
    pub reverse: bool,
    /// The exit test reads the phi, before the step, not the stepped value
    /// (`for (i = n; i >= 0; --i)` leaves when `i == 0`): [`stepped_exit_test`]
    /// moves it.
    pub tests_phi: bool,
}

/// The exit test of a loop that tests the phi, on the stepped value instead:
/// `phi == bound` is `stepped == bound + step` (the loop leaves on the same
/// iteration), the form the trip count and the vector loop's end are written
/// for. The bound is folded where it is a constant, else made in the preheader.
pub(crate) fn stepped_exit_test(
    context: &mut Context,
    function: &mut Function,
    shape: Shape,
    before_checks: Option<InstId>,
) -> Shape {
    if !shape.tests_phi {
        return shape;
    }
    let Shape { header, preheader, primary, compare, bound_operand, index_ty, reverse, .. } = shape;
    let bits = bits_of(context, index_ty);
    let stepped = arms(function, primary).into_iter().find(|&(_, from)| from == header).expect("a back edge").0;
    let bound = function.instruction(compare).operands[bound_operand];
    let step: u128 = if reverse { mask(bits) } else { 1 };
    let moved = match constant_int(context, bound) {
        Some(value) => {
            Operand::Constant(context.constant(Constant {
                ty: index_ty,
                kind: ConstantKind::Int(value.wrapping_add(step) & mask(bits)),
            }))
        }
        None => {
            let by = Operand::Constant(context.constant(Constant { ty: index_ty, kind: ConstantKind::Int(step) }));
            // Before the memory checks, which move to their own block.
            let before = Position::Before(
                before_checks.unwrap_or_else(|| function.terminator(preheader).expect("a terminated preheader")),
            );
            emit(function, before, Opcode::Binary(BinaryOp::Add), index_ty, vec![bound, by], "bound.next")
        }
    };
    function.set_operand(compare, bound_operand, moved);
    function.set_operand(compare, 1 - bound_operand, stepped);
    Shape { tests_phi: false, ..shape }
}

/// The memory checks, made at the end of the preheader: the first instruction
/// of them, and the `i1` that is true when some pair of pointer groups may
/// overlap (`GeneratedRTChecks::MemRuntimeCheckCond`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct MemChecks {
    pub first: InstId,
    pub conflict: Operand,
}

/// `GeneratedRTChecks::create`'s memory part (L1636): the checks that the
/// groups of `rtc` do not overlap, made at the end of the preheader of the loop
/// at `header`, for `vf` x `ic` lanes. `Ok(None)` where they fold away (no
/// conflict); `Err` where one always conflicts, so the vector loop never runs.
pub(crate) fn add_memory_checks(
    rw: &mut Rewriter,
    rtc: &RuntimePointerChecking,
    header: BlockId,
    preheader: BlockId,
    vf: u32,
    ic: u32,
) -> Result<Option<MemChecks>, &'static str> {
    let term = rw.function.terminator(preheader).expect("a terminated preheader");
    // The checks are made before the terminator, after the existing
    // instructions.
    let existing = rw.function.iter_block(preheader).count() - 1;
    let mut exp = Expander::new("scev.check");
    let conflict = match rtc.diff_checks() {
        Some(diff) => rw.add_diff_runtime_checks(&mut exp, term, diff, vf, ic),
        None => rw.add_runtime_checks(
            &mut exp,
            term,
            header,
            rtc,
            &rtc.checks,
            VectorizerParams::default().hoist_runtime_checks,
        ),
    };
    let Some(conflict) = conflict else { return Ok(None) };
    if let Some(value) = constant_int(rw.context, conflict) {
        return if value == 0 { Ok(None) } else { Err("the memory checks always conflict") };
    }
    let first = rw.function.iter_block(preheader).nth(existing).unwrap_or(term);
    Ok(Some(MemChecks { first, conflict }))
}

/// The blocks around a widened loop.
pub(crate) struct Skeleton {
    /// Where the loop's invariants are made and the loop is entered from
    /// (`vector.ph`, or the preheader where the checks folded).
    pub vector_ph: BlockId,
    /// `middle.block`, between the loop and what follows it: made where the
    /// loop has reductions to combine.
    pub middle: Option<BlockId>,
    /// `scalar.ph`, where the remainder resumes (a run-time trip count).
    pub scalar_ph: Option<BlockId>,
    /// The remainder loop and the copy of each instruction of the original.
    pub remainder: Option<(BlockId, Vec<(InstId, InstId)>)>,
}

/// An integer constant's value, masked to its type.
fn constant_int(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(c) => match context.get(c).kind {
            ConstantKind::Int(value) => Some(value),
            ConstantKind::Zero => Some(0),
            _ => None,
        },
        _ => None,
    }
}

/// The iterations between a constant `start` and the `bound` the stepped
/// induction meets (the bound less the start, the start less the bound counting
/// down), not yet masked to the type.
fn iterations(
    reverse: bool,
    start: u128,
    bound: u128,
) -> u128 {
    if reverse { start.wrapping_sub(bound) } else { bound.wrapping_sub(start) }
}

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

fn bits_of(
    context: &Context,
    ty: TypeId,
) -> u32 {
    match context.types.get(ty) {
        Type::Int(bits) => *bits,
        other => unreachable!("an induction of {other:?}"),
    }
}

/// An instruction made at `at`, and its value.
fn emit(
    function: &mut Function,
    at: Position,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
    name: &str,
) -> Operand {
    let inst = function.create_instruction(opcode, ty, operands, Flags::default(), Some(name));
    function.insert(inst, at).expect("a placed block");
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

/// A terminator at the end of `block`.
fn end_with(
    function: &mut Function,
    block: BlockId,
    void: TypeId,
    operands: Vec<Operand>,
) {
    let br = function.create_instruction(Opcode::Br, void, operands, Flags::default(), None);
    function.insert(br, Position::End(block)).expect("a placed block");
}

/// The scalar remainder after the loop `shape`, which is about to be widened by
/// `vf_ic` lanes an iteration (a power of two). Returns the block the
/// vector loop's invariants are made in (`vector.ph`, or the preheader where
/// the checks folded).
///
/// `None`, with the loop untouched, where a constant trip count is under
/// `vf_ic`: the vector loop would not run. `with_middle`: the loop has
/// reductions, so a middle block is made even where the checks fold. `memory`:
/// the checks made in the preheader; they move to `vector.memcheck`, between
/// the iteration check and `vector.ph`, and a conflict goes to `scalar.ph`.
pub(crate) fn add_remainder(
    context: &mut Context,
    function: &mut Function,
    declared: &mut Declared,
    metadata: &[MetadataNode],
    shape: Shape,
    vf_ic: u64,
    with_middle: bool,
    memory: Option<MemChecks>,
) -> Option<Skeleton> {
    let Shape { header, preheader, exit, primary, compare, bound_operand, index_ty, reverse, .. } = shape;
    debug_assert!(!shape.tests_phi, "the exit test is on the stepped value");
    let bits = bits_of(context, index_ty);
    let preheader_term = function.terminator(preheader).expect("a terminated preheader");
    let void = function.instruction(preheader_term).ty;
    let header_name = function.block(header).name.clone().unwrap_or_default();
    let incoming = arms(function, primary);
    let start = incoming.iter().find(|&&(_, from)| from == preheader).expect("the start arm").0;
    let bound = function.instruction(compare).operands[bound_operand];
    let (start_value, bound_value) = (constant_int(context, start), constant_int(context, bound));
    if let (Some(start_value), Some(bound_value)) = (start_value, bound_value)
        && (iterations(reverse, start_value, bound_value) & mask(bits)) < u128::from(vf_ic)
    {
        return None;
    }

    // The remainder: the loop as it is, before the widening.
    let mut vmap = ValueMap::default();
    let remainder = function.clone_block(header, &mut vmap);
    for (from, to) in function
        .iter_block(header)
        .collect::<Vec<_>>()
        .into_iter()
        .zip(function.iter_block(remainder).collect::<Vec<_>>())
    {
        function.remap_instruction(to, &vmap);
        if let (Some(from), Some(to)) = (function.instruction(from).result, function.instruction(to).result)
            && let Some(name) = function.value(from).name.clone()
        {
            function.set_name(to, &format!("{name}.scalar"));
        }
    }
    function.set_block_name(remainder, &format!("{header_name}.scalar"));
    function.insert_block(remainder, Some(header)).expect("a new block");
    let copies: Vec<(InstId, InstId)> = function
        .iter_block(header)
        .collect::<Vec<_>>()
        .into_iter()
        .zip(function.iter_block(remainder).collect::<Vec<_>>())
        .collect();

    // The copy of the induction phi stands where the original does.
    let at = function.iter_block(header).position(|inst| inst == primary).expect("a phi of the loop");
    let remainder_phi = function.iter_block(remainder).nth(at).expect("the copy of the phi");
    let stepped =
        arms(function, remainder_phi).into_iter().find(|&(_, from)| from == remainder).expect("a back edge").0;
    let exit_term = function.terminator(header).expect("a terminated loop");
    let remainder_term = function.terminator(remainder).expect("a terminated copy");

    let constant = |context: &mut Context, value: u128| {
        Operand::Constant(context.constant(Constant { ty: index_ty, kind: ConstantKind::Int(value & mask(bits)) }))
    };

    let vector_ph;
    let mut middle_block = None;
    let mut scalar_block = None;
    // A loop with memory checks keeps its remainder: a conflict runs it whole.
    if let (Some(start_value), Some(bound_value)) = (start_value, bound_value)
        && memory.is_none()
    {
        // Both ends known: the checks fold (the pin's constant folder).
        let trip = iterations(reverse, start_value, bound_value) & mask(bits);
        let moved = trip - trip % u128::from(vf_ic);
        let end = constant(context, if reverse { start_value.wrapping_sub(moved) } else { start_value + moved });
        function.set_operand(compare, bound_operand, end);
        let from = if with_middle {
            // The reductions are combined between the loop and its remainder.
            let middle = function.create_block(Some("middle.block"));
            function.insert_block(middle, Some(header)).expect("a new block");
            edges::retarget(function, exit_term, exit, middle);
            end_with(function, middle, void, vec![Operand::Block(remainder)]);
            middle_block = Some(middle);
            middle
        } else {
            edges::retarget(function, exit_term, exit, remainder);
            header
        };
        function.set_operands(remainder_phi, from_arms(&[(end, from), (stepped, remainder)]));
        retarget_exit_phis(function, exit, header, &[remainder]);
        vector_ph = preheader;
    } else {
        let bool_ty = context.types.intern(Type::Int(1));
        // The iteration check comes before the memory checks.
        let before = Position::Before(memory.map_or(preheader_term, |checks| checks.first));
        // n = bound - start (start - bound counting down), the iterations of
        // the loop.
        let trip = if reverse {
            emit(function, before, Opcode::Binary(BinaryOp::Sub), index_ty, vec![start, bound], "n.trip")
        } else if start_value == Some(0) {
            bound
        } else {
            emit(function, before, Opcode::Binary(BinaryOp::Sub), index_ty, vec![bound, start], "n.trip")
        };
        let vector_lanes = constant(context, u128::from(vf_ic));
        let min_iters = emit(
            function,
            before,
            Opcode::ICmp(IntPredicate::Ult),
            bool_ty,
            vec![trip, vector_lanes],
            "min.iters.check",
        );

        let vector = function.create_block(Some("vector.ph"));
        let middle = function.create_block(Some("middle.block"));
        let scalar_ph = function.create_block(Some("scalar.ph"));
        function.insert_block_before(vector, header).expect("a new block");
        function.insert_block(middle, Some(header)).expect("a new block");
        function.insert_block(scalar_ph, Some(middle)).expect("a new block");

        // vector.ph: the vector trip count, the iterations the vector loop
        // runs.
        let lane_mask = constant(context, u128::from(vf_ic - 1));
        let at_vector = Position::End(vector);
        let remainder_lanes =
            emit(function, at_vector, Opcode::Binary(BinaryOp::And), index_ty, vec![trip, lane_mask], "n.mod.vf");
        let vector_trip =
            emit(function, at_vector, Opcode::Binary(BinaryOp::Sub), index_ty, vec![trip, remainder_lanes], "n.vec");
        let end = if reverse {
            emit(function, at_vector, Opcode::Binary(BinaryOp::Sub), index_ty, vec![start, vector_trip], "ind.end")
        } else if start_value == Some(0) {
            vector_trip
        } else {
            emit(function, at_vector, Opcode::Binary(BinaryOp::Add), index_ty, vec![start, vector_trip], "ind.end")
        };
        end_with(function, vector, void, vec![Operand::Block(header)]);

        // The preheader branches around the vector loop for a short trip; the
        // memory checks, if any, are the block between.
        let memcheck = memory.map(|checks| {
            let block = edges::split_block_at(function, preheader, checks.first);
            function.set_block_name(block, "vector.memcheck");
            block
        });
        let guard_at = function.terminator(preheader).expect("a terminated preheader");
        let guard = function.create_instruction(
            Opcode::Br,
            void,
            vec![min_iters, Operand::Block(scalar_ph), Operand::Block(memcheck.unwrap_or(vector))],
            Flags::default(),
            None,
        );
        function.insert(guard, Position::Before(guard_at)).expect("a placed terminator");
        function.set_operands(guard_at, Vec::new());
        function.erase(guard_at).expect("the old jump");
        if let Some(checks) = memory {
            let conflict = function.create_instruction(
                Opcode::Br,
                void,
                vec![checks.conflict, Operand::Block(scalar_ph), Operand::Block(vector)],
                Flags::default(),
                None,
            );
            function.insert(conflict, Position::Before(preheader_term)).expect("a placed terminator");
            function.set_operands(preheader_term, Vec::new());
            function.erase(preheader_term).expect("the old jump");
        }
        let entry = memcheck.unwrap_or(preheader);

        // The vector loop enters from vector.ph, compares to the end and leaves
        // to the middle block.
        let entering: Vec<_> =
            incoming.iter().map(|&(value, from)| (value, if from == preheader { vector } else { from })).collect();
        function.set_operands(primary, from_arms(&entering));
        // The reductions' phis are entered from there too.
        for phi in edges::phis(function, header) {
            if phi == primary {
                continue;
            }
            let moved: Vec<_> = arms(function, phi)
                .into_iter()
                .map(|(value, from)| (value, if from == entry { vector } else { from }))
                .collect();
            function.set_operands(phi, from_arms(&moved));
        }
        function.set_operand(compare, bound_operand, end);
        edges::retarget(function, exit_term, exit, middle);

        // middle.block: done if the vector loop ran every iteration.
        let all = emit(
            function,
            Position::End(middle),
            Opcode::ICmp(IntPredicate::Eq),
            bool_ty,
            vec![trip, vector_trip],
            "cmp.n",
        );
        end_with(function, middle, void, vec![all, Operand::Block(exit), Operand::Block(scalar_ph)]);

        // scalar.ph: where the remainder resumes.
        let resume = emit(function, Position::End(scalar_ph), Opcode::Phi, index_ty, Vec::new(), "bc.resume.val");
        let Operand::Value(resume_value) = resume else { unreachable!("a phi") };
        let resume_phi = match function.value(resume_value).def {
            llrm_mir::module::ValueDef::Instruction(def) => def,
            _ => unreachable!("a phi"),
        };
        let mut resumed = vec![(end, middle), (start, preheader)];
        if let Some(block) = memcheck {
            resumed.push((start, block));
        }
        function.set_operands(resume_phi, from_arms(&resumed));
        end_with(function, scalar_ph, void, vec![Operand::Block(remainder)]);
        function.set_operands(remainder_phi, from_arms(&[(resume, scalar_ph), (stepped, remainder)]));
        retarget_exit_phis(function, exit, header, &[middle, remainder]);
        vector_ph = vector;
        middle_block = Some(middle);
        scalar_block = Some(scalar_ph);
    }
    mark_vectorized(context, declared, metadata, function, exit_term);
    mark_vectorized(context, declared, metadata, function, remainder_term);
    Some(Skeleton { vector_ph, middle: middle_block, scalar_ph: scalar_block, remainder: Some((remainder, copies)) })
}

/// The middle block of a loop with no remainder (the trip count is a multiple
/// of the lanes): between the loop and the block it leaves to, where the
/// reductions are combined.
pub(crate) fn add_middle(
    function: &mut Function,
    shape: Shape,
) -> BlockId {
    let Shape { header, exit, .. } = shape;
    let exit_term = function.terminator(header).expect("a terminated loop");
    let void = function.instruction(exit_term).ty;
    let middle = function.create_block(Some("middle.block"));
    function.insert_block(middle, Some(header)).expect("a new block");
    edges::retarget(function, exit_term, exit, middle);
    end_with(function, middle, void, vec![Operand::Block(exit)]);
    retarget_exit_phis(function, exit, header, &[middle]);
    middle
}

/// The exit block's phis: the arm from the loop `old` now comes from each of
/// `new` (the loop's own value is an invariant, LVS3).
fn retarget_exit_phis(
    function: &mut Function,
    exit: BlockId,
    old: BlockId,
    new: &[BlockId],
) {
    for phi in edges::phis(function, exit) {
        let mut incoming = arms(function, phi);
        let Some(at) = incoming.iter().position(|&(_, from)| from == old) else { continue };
        let (value, _) = incoming.remove(at);
        for (offset, &from) in new.iter().enumerate() {
            incoming.insert(at + offset, (value, from));
        }
        function.set_operands(phi, from_arms(&incoming));
    }
}

/// The metadata node `at`: the module's, or one this pass made.
fn node_at(
    declared: &Declared,
    metadata: &[MetadataNode],
    at: MetadataId,
) -> Option<MetadataNode> {
    metadata
        .get(at.0 as usize)
        .or_else(|| at.0.checked_sub(metadata.len() as u32).and_then(|offset| declared.nodes().get(offset as usize)))
        .cloned()
}

/// Whether `option`, an operand of a loop ID, is the node of the option `name`.
fn is_option(
    declared: &Declared,
    metadata: &[MetadataNode],
    option: &MetadataOperand,
    name: &str,
) -> bool {
    match option {
        MetadataOperand::Node(one) => node_at(declared, metadata, *one).is_some_and(|node| {
            matches!(
                node.operands.first(),
                Some(MetadataOperand::String(first)) if first == name
            )
        }),
        _ => false,
    }
}

/// The options of the loop ID on `branch` (the operands after the ID itself).
fn loop_options(
    declared: &Declared,
    metadata: &[MetadataNode],
    function: &Function,
    branch: InstId,
) -> Vec<MetadataOperand> {
    function
        .instruction(branch)
        .metadata
        .iter()
        .find(|(kind, _)| kind == "llvm.loop")
        .and_then(|(_, id)| node_at(declared, metadata, *id))
        .map(|id| id.operands.into_iter().skip(1).collect())
        .unwrap_or_default()
}

/// `llvm.loop.isvectorized` on the loop whose latch ends in `branch`: the
/// pin's own mark that the loop is the product of vectorization, which a later
/// run leaves alone (`LoopVectorizeHints::getIsVectorized`).
pub(crate) fn already_vectorized(
    metadata: &[MetadataNode],
    function: &Function,
    branch: InstId,
) -> bool {
    let declared = Declared::default();
    loop_options(&declared, metadata, function, branch)
        .iter()
        .any(|one| is_option(&declared, metadata, one, "llvm.loop.isvectorized"))
}

/// `llvm.loop.isvectorized` and `llvm.loop.unroll.runtime.disable` on the loop
/// whose latch ends in `branch`: the pin marks the vector loop and the
/// remainder alike, so the remainder is not vectorized again or unrolled at
/// run time.
pub(crate) fn mark_vectorized(
    context: &mut Context,
    declared: &mut Declared,
    metadata: &[MetadataNode],
    function: &mut Function,
    branch: InstId,
) {
    let mut options = loop_options(declared, metadata, function, branch);
    if !options.iter().any(|one| is_option(declared, metadata, one, "llvm.loop.isvectorized")) {
        let i32_ty = context.types.intern(Type::Int(32));
        let one = context.int(i32_ty, 1);
        let node = declared.node(MetadataNode {
            distinct: false,
            operands: vec![
                MetadataOperand::String("llvm.loop.isvectorized".to_owned()),
                MetadataOperand::Constant(one),
            ],
        });
        options.push(MetadataOperand::Node(node));
    }
    if !options.iter().any(|one| is_option(declared, metadata, one, "llvm.loop.unroll.runtime.disable")) {
        let node = declared.node(MetadataNode {
            distinct: false,
            operands: vec![MetadataOperand::String("llvm.loop.unroll.runtime.disable".to_owned())],
        });
        options.push(MetadataOperand::Node(node));
    }
    let id = declared.next_node();
    let operands = std::iter::once(MetadataOperand::Node(id)).chain(options).collect();
    declared.node(MetadataNode { distinct: true, operands });
    function.set_metadata(branch, "llvm.loop", Some(id));
}
