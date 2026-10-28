//! Tail calls are marked, and a function's calls to itself in tail position
//! become a loop, as LLVM's `TailCallElim`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `TailRecursionElimination.cpp`, each
//! function linking the lines it translates.
//!
//! `mark_tails` sets `tail` on every call that cannot reach a frame object of
//! the function: one made before any alloca (or `byval` parameter) escapes, or
//! whose arguments are all constants and parameters of a function that reads
//! no memory. `TailRecursionEliminator` then looks, in each block that ends in
//! a `ret` (or in a `br` to a block that is one `ret`, which it duplicates
//! first), for a tail call to the function itself with only instructions that
//! can move above the call after it, and turns it into a branch to a new
//! header block that holds a phi for every parameter.
//!
//! A single add, mul, and, or, xor or shift by a constant on the call's result
//! on the way out is an accumulator: a phi in the header starts at the
//! operation's identity (or the base-case constant, for a shift) and takes the
//! operation's other operand on each trip. A return value that is known on one
//! trip but not on the others is carried by `ret.tr` and `ret.known.tr`.
//!
//! ```text
//!   before                        after
//!   b0: ...                       b0:  br tailrecurse
//!       %v = call f(%n-1)         tailrecurse:
//!       %s = add %v, %x             %n.tr = phi [%n, b0], [%n-1, tailrecurse]
//!       ret %s                      %acc  = phi [0, b0], [%acc+%x, ...]
//!                                   ...
//!                                   br tailrecurse     ; was the call
//!                                 other exits: ret (%acc + value)
//! ```
//!
//! DEVIATE (anything MIR or Rust lacks):
//! 24. MIR has no operand bundles, token types, `stackrestore` or pseudo
//!     probes, so the checks that refuse those have nothing to refuse.
//! 25. No `OptimizationRemarkEmitter`, `BlockFrequencyInfo` or
//!     `ProfileSummaryInfo`: `shouldDisableTailCallsForCold` is false with its
//!     default option, and the entry count is only updated under instrumented
//!     PGO (`UpdateFunctionEntryCount` is false in the default pipeline).
//! 26. The dominator trees are not updated in place (`DomTreeUpdater` is not
//!     passed): the pass reports what it changed and the manager drops the
//!     analyses a new entry block and edges leave stale, as `gvn` does.
//! 27. `isSafeToLoadUnconditionally` is `dereferenceable` and the load's
//!     alignment: no "a load of it dominates" proof.
//! 28. `isAssociative` is the integer operations only (`combine_shared`'s
//!     DEVIATE 2), so a float accumulator is not recognized.
//!
//! # Example
//! `sumto(n, x) = n == 0 ? x : sumto(n - 1, x + n)` becomes a loop that
//! carries `n` and `x`; see `tailcallelim_tests.rs`.
//!
//! # Runs
//! As `tailrec`, the name its option (`sibcalls`) and slot (`tailrec@688`)
//! keep: in the function pipeline after `gvn::EqualCalls`, so two equal calls
//! are one first, and before `loopsimplify`, so the loop it makes goes to the
//! loop passes. Once per round of the fixed point; at -O2 and above, off at -O0
//! and -O1 and by `-fno-optimize-sibling-calls`.
//!
//! # Needs
//! `BasicAA` for the mod/ref of a call on a load that moves above it.
//!
//! # Keeps
//! The dominators, loops and shape where only tail flags changed; nothing
//! where a loop was made.

use llrm_analysis::aa::{BatchAAResults, MemoryLocation};
use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::memory::Unit as AaUnit;
use llrm_mir::context::{Constant, ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::facts::Fact;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::memory;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Attribute, BinaryOp, CallInfo, CastOp, Flags, Opcode, Tail};
use llrm_mir::passes::{Analyses, Declared, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};
use llrm_mir::types::{Type, TypeId};

use crate::lcssa::{arms, from_arms};

/// The pass: `TailCallElimPass::run`
/// ([TailRecursionElimination.cpp:1172](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L1172)).
pub struct TailCallElim;

/// The flag of a function whose tail-call elimination waits (D10: a tree
/// recursion is copied into itself first, `walk::SelfInline`).
pub const DEFERRED: &str = "tre-deferred";

impl FunctionPass for TailCallElim {
    fn name(&self) -> &'static str {
        "tailrec"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("tailcallelim")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if unit.function.is_declaration()
            || unit.function.attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == DEFERRED))
        {
            return PreservedAnalyses::all();
        }
        let outer = analyses.outer();
        match eliminate(unit.context, unit.layout, unit.function, unit.declared, outer, unit.id) {
            Outcome::Unchanged => PreservedAnalyses::all(),
            Outcome::Marked => PreservedAnalyses::none()
                .preserve::<Dominators>()
                .preserve::<Loops>()
                .preserve::<llrm_analysis::cfg::Shape>(),
            Outcome::Looped => PreservedAnalyses::none(),
        }
    }
}

/// What `eliminate` did to the function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    Unchanged,
    /// Only `tail` flags were set.
    Marked,
    /// A recursion became a loop.
    Looped,
}

/// `TailRecursionEliminator::eliminate`
/// ([TailRecursionElimination.cpp:1090](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L1090)).
fn eliminate(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    id: Option<GlobalId>,
) -> Outcome {
    if function.attrs.iter().any(|attr| {
        matches!(
            attr,
            Attribute::Str(name, value) if name == "disable-tail-calls" && value.as_deref() == Some("true")
        )
    }) {
        return Outcome::Unchanged;
    }
    let marked = mark_tails(context, layout, function, outer);
    // If this function is a varargs function, we won't be able to PHI the args
    // right, so don't even try to convert it...
    if function_parts(context, function.ty).1 || !can_tre(function) {
        return if marked { Outcome::Marked } else { Outcome::Unchanged };
    }
    let mut eliminator = TailRecursionEliminator::new(context, layout, function, declared, outer, id);
    let blocks = eliminator.function.layout().to_vec();
    let mut changed = false;
    for block in blocks {
        changed |= eliminator.process_block(block);
    }
    eliminator.cleanup_and_finalize();
    match (changed, marked) {
        (true, _) => Outcome::Looped,
        (false, true) => Outcome::Marked,
        (false, false) => Outcome::Unchanged,
    }
}

/// `canTRE`
/// ([TailRecursionElimination.cpp:146](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L146)):
/// every alloca is static.
fn can_tre(function: &Function) -> bool {
    function.walk().all(|(block, inst)| {
        !matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }) || is_static_alloca(function, block, inst)
    })
}

/// `AllocaInst::isStaticAlloca`: a constant size, in the entry block.
fn is_static_alloca(
    function: &Function,
    block: BlockId,
    inst: InstId,
) -> bool {
    Some(block) == function.entry()
        && function.instruction(inst).operands.iter().all(|one| matches!(one, Operand::Constant(_)))
}

/// `AllocaDerivedValueTracker`
/// ([TailRecursionElimination.cpp:158](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L158)).
#[derive(Default)]
struct AllocaDerivedValueTracker {
    alloca_users: IdSet<InstId>,
    escape_points: IdSet<InstId>,
}

impl AllocaDerivedValueTracker {
    /// `walk`: start at a root value and walk its use-def chain to mark calls
    /// that use the value or a derived value in `alloca_users`, and places
    /// where it may escape in `escape_points`.
    fn walk(
        &mut self,
        context: &Context,
        outer: &Outer,
        function: &Function,
        root: ValueId,
    ) {
        let mut worklist: Vec<(InstId, u32)> = Vec::new();
        let mut visited: IdSet<ValueId> = IdSet::new();
        let add_uses = |value: ValueId, worklist: &mut Vec<(InstId, u32)>, visited: &mut IdSet<ValueId>| {
            if visited.insert(value) {
                worklist.extend(function.users(value).iter().map(|one| (one.user, one.index)));
            }
        };
        add_uses(root, &mut worklist, &mut visited);
        while let Some((user, index)) = worklist.pop() {
            let instruction = function.instruction(user);
            match &instruction.opcode {
                Opcode::Call(info) | Opcode::Invoke(info) => {
                    // If the alloca-derived argument is passed byval it is not
                    // an escape point, or a use of an alloca. Calling with
                    // byval copies the contents of the
                    // alloca into argument registers
                    // or stack slots, which exist beyond the lifetime of the
                    // current frame.
                    let argument = ((index as usize) + 1 < instruction.operands.len()).then_some(index as usize);
                    if argument.is_some_and(|at| has_type_attr(info.argument_attrs.get(at), "byval")) {
                        continue;
                    }
                    let nocapture =
                        argument.is_some_and(|at| memory::nocapture(context, outer.callees(), function, user, at));
                    self.call_uses_local_stack(context, outer, function, user, nocapture);
                    if nocapture {
                        // If the alloca-derived argument is passed in as
                        // nocapture, then it can't propagate to the call's
                        // return. That would be capturing.
                        continue;
                    }
                }
                // The result of a load is not alloca-derived (unless an alloca
                // has otherwise escaped, but this is a local analysis).
                Opcode::Load { .. } => continue,
                Opcode::Store { .. } => {
                    if index == 0 {
                        self.escape_points.insert(user);
                    }
                    // Stores have no users to analyze.
                    continue;
                }
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast)
                | Opcode::GetElementPtr { .. }
                | Opcode::Phi
                | Opcode::Select => {}
                _ => {
                    self.escape_points.insert(user);
                }
            }
            if let Some(result) = instruction.result {
                add_uses(result, &mut worklist, &mut visited);
            }
        }
    }

    /// `callUsesLocalStack`
    fn call_uses_local_stack(
        &mut self,
        context: &Context,
        outer: &Outer,
        function: &Function,
        call: InstId,
        nocapture: bool,
    ) {
        // Add it to the list of alloca users.
        self.alloca_users.insert(call);
        // If it's nocapture then it can't capture this alloca.
        if nocapture {
            return;
        }
        // If it can write to memory, it can leak the alloca value.
        if memory::of(context, outer.callees(), function, call).writes {
            self.escape_points.insert(call);
        }
    }
}

/// Whether `attrs` carry the type attribute `name`.
fn has_type_attr(
    attrs: Option<&Vec<Attribute>>,
    name: &str,
) -> bool {
    attrs.is_some_and(|attrs| attrs.iter().any(|attr| matches!(attr, Attribute::Type(one, _) if one == name)))
}

/// `Function::callsFunctionThatReturnsTwice`: a call whose site or callee says
/// `returns_twice`.
fn calls_function_that_returns_twice(
    context: &Context,
    outer: &Outer,
    function: &Function,
) -> bool {
    function
        .walk()
        .any(
            |(_, inst)| {
                let Opcode::Call(info) = &function.instruction(inst).opcode else { return false };
                memory::has(&info.attrs, "returns_twice")
                    || memory::callee(context, function, inst)
                        .and_then(|one| outer.globals.get(one.0 as usize)?.function())
                        .is_some_and(|one| memory::has(&one.attrs, "returns_twice"))
            },
        )
}

/// `markTails`
/// ([TailRecursionElimination.cpp:243](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L243)).
fn mark_tails(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    if calls_function_that_returns_twice(context, outer, function) {
        return false;
    }
    // The local stack holds all alloca instructions and all byval arguments.
    let mut tracker = AllocaDerivedValueTracker::default();
    for (at, parameter) in function.parameters().to_vec().into_iter().enumerate() {
        if has_type_attr(function.parameter_attrs.get(at), "byval") {
            tracker.walk(context, outer, function, parameter);
        }
    }
    for (_, inst) in function.walk().collect::<Vec<_>>() {
        if let (Opcode::Alloca { .. }, Some(result)) =
            (&function.instruction(inst).opcode, function.instruction(inst).result)
        {
            tracker.walk(context, outer, function, result);
        }
    }

    // Track whether a block is reachable after an alloca has escaped. Blocks
    // that contain the escaping instruction will be marked as being visited
    // without an escaped alloca, since that is how the block began.
    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    enum VisitType {
        Unvisited,
        Unescaped,
        Escaped,
    }
    let mut visited: IdMap<BlockId, VisitType> = IdMap::new();
    // We propagate the fact that an alloca has escaped from block to successor.
    // Visit the blocks that are propagating the escapedness first. To do this,
    // we maintain two worklists.
    let mut worklist_unescaped: Vec<BlockId> = Vec::new();
    let mut worklist_escaped: Vec<BlockId> = Vec::new();
    // We may enter a block and visit it thinking that no alloca has escaped
    // yet, then see an escape point and go back around a loop edge and come
    // back to the same block twice. Because of this, we defer setting tail on
    // calls when we first encounter them in a block.
    let mut deferred_tails: Vec<InstId> = Vec::new();
    let mut readnone_tails: Vec<InstId> = Vec::new();

    let unit = AaUnit::within(&*context, layout, &*function, outer);
    let mut aa = BasicAA::new(&unit);
    let mut batch = BatchAAResults::new(&mut aa);

    let mut block = function.entry();
    let mut escaped = VisitType::Unescaped;
    while let Some(bb) = block {
        for inst in function.iter_block(bb).collect::<Vec<_>>() {
            if tracker.escape_points.contains(&inst) {
                escaped = VisitType::Escaped;
            }
            let instruction = function.instruction(inst);
            let Opcode::Call(info) = &instruction.opcode else { continue };
            if matches!(info.tail, Tail::Tail | Tail::MustTail) {
                continue;
            }
            let is_no_tail = info.tail == Tail::NoTail;

            if !is_no_tail && batch.does_not_access_memory(inst) {
                // A call to a readnone function whose arguments are all things
                // computed outside this function can be marked tail. Even if
                // you stored the alloca address into a global, a readnone
                // function can't load the global anyhow.
                //
                // Note that this runs whether we know an alloca has escaped or
                // not. If it has, then we can't trust `alloca_users` to be
                // accurate.
                let safe_to_tail =
                    instruction.operands[..instruction.operands.len() - 1].iter().all(|argument| match argument {
                        Operand::Constant(_) => true,
                        Operand::Value(value) => {
                            matches!(
                                function.value(*value).def,
                                ValueDef::Argument(index) if !has_type_attr(function.parameter_attrs.get(index as usize), "byval")
                            )
                        }
                        Operand::Block(_) => false,
                    });
                if safe_to_tail {
                    readnone_tails.push(inst);
                    continue;
                }
            }

            if !is_no_tail && escaped == VisitType::Unescaped && !tracker.alloca_users.contains(&inst) {
                deferred_tails.push(inst);
            }
        }

        for succ in function.successors(bb).iter().copied().collect::<Vec<_>>() {
            let state = visited.get_or_insert_with(succ, || VisitType::Unvisited);
            if *state < escaped {
                *state = escaped;
                if *state == VisitType::Escaped {
                    worklist_escaped.push(succ);
                } else {
                    worklist_unescaped.push(succ);
                }
            }
        }

        if let Some(next) = worklist_escaped.pop() {
            block = Some(next);
            escaped = VisitType::Escaped;
        } else {
            block = None;
            while let Some(next) = worklist_unescaped.pop() {
                if visited.get(&next).copied() == Some(VisitType::Unescaped) {
                    block = Some(next);
                    escaped = VisitType::Unescaped;
                    break;
                }
            }
        }
    }
    drop(batch);

    let mut modified = false;
    for call in readnone_tails {
        set_tail_call(function, call);
        modified = true;
    }
    for call in deferred_tails {
        let parent = function.parent(call).expect("a placed call");
        if visited.get(&parent).copied() != Some(VisitType::Escaped) {
            // If the escape point was part way through the block, calls after
            // the escape point wouldn't have been put into `deferred_tails`.
            set_tail_call(function, call);
            modified = true;
        }
    }
    modified
}

/// `CallInst::setTailCall`.
fn set_tail_call(
    function: &mut Function,
    call: InstId,
) {
    let Opcode::Call(info) = function.instruction(call).opcode.clone() else { return };
    let mut info = info;
    info.tail = Tail::Tail;
    function.set_opcode(call, Opcode::Call(info));
}

/// `canMoveAboveCall`
/// ([TailRecursionElimination.cpp:394](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L394)):
/// whether it is safe to move `inst` from after `call` to before it, assuming
/// that all instructions between the call and this instruction are movable.
fn can_move_above_call(
    context: &Context,
    layout: &DataLayout,
    outer: &Outer,
    function: &Function,
    inst: InstId,
    call: InstId,
) -> bool {
    let intrinsic =
        memory::callee(context, function, inst).and_then(|one| outer.globals.get(one.0 as usize)?.intrinsic());
    if intrinsic == Some(Intrinsic::LifetimeEnd) {
        return true;
    }
    // FIXME: We can move load/store/call/free instructions above the call if
    // the call does not mod/ref the memory location being processed.
    if may_have_side_effects(context, outer, function, inst) {
        // This also handles volatile loads.
        return false;
    }
    if let Opcode::Load { align, .. } = function.instruction(inst).opcode {
        // Loads may always be moved above calls without side effects.
        if may_have_side_effects(context, outer, function, call) {
            // Non-volatile loads may be moved above a call with side effects if
            // it does not write to memory and the load provably won't trap.
            // Writes to memory only matter if they may alias the pointer being
            // loaded from.
            let unit = AaUnit::within(context, layout, function, outer);
            let mut aa = BasicAA::new(&unit);
            let mut batch = BatchAAResults::new(&mut aa);
            let location = MemoryLocation::get(&unit, inst);
            if batch.get_mod_ref_info(call, location.as_ref()).is_mod_set()
                || !is_safe_to_load_unconditionally(context, layout, outer, function, inst, align)
            {
                return false;
            }
        }
    }
    // Otherwise, if this is a side-effect free instruction, check to make sure
    // that it does not use the return value of the call. If it doesn't use the
    // return value of the call, it must only use things that are defined before
    // the call, or movable instructions between the call and the instruction
    // itself.
    let result = function.instruction(call).result.map(Operand::Value);
    !function.instruction(inst).operands.iter().any(|one| Some(*one) == result)
}

/// `Instruction::mayHaveSideEffects`.
fn may_have_side_effects(
    context: &Context,
    outer: &Outer,
    function: &Function,
    inst: InstId,
) -> bool {
    crate::local::may_have_side_effects(context, outer.callees(), &outer.globals, function, inst)
}

/// `isSafeToLoadUnconditionally`, DEVIATE 27.
fn is_safe_to_load_unconditionally(
    context: &Context,
    layout: &DataLayout,
    outer: &Outer,
    function: &Function,
    load: InstId,
    align: Option<u64>,
) -> bool {
    let instruction = function.instruction(load);
    let pointer = instruction.operands[0];
    let bytes = layout.store_size(&context.types, instruction.ty) as u64;
    let aligned =
        align.unwrap_or(1) <= llrm_mir::valuetracking::alignment(context, layout, &outer.globals, function, pointer);
    aligned
        && llrm_mir::valuetracking::dereferenceable(
            context,
            layout,
            &llrm_mir::valuetracking::Sizes::default(),
            function,
            pointer,
            bytes,
        )
}

/// `isUnaryAccumulatorRecurrence`
/// ([TailRecursionElimination.cpp:437](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L437)):
/// a shift by a constant amount, whose chain `(Base << C) << C ...` is one
/// shift by the sum of the amounts.
fn is_unary_accumulator_recurrence(
    function: &Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    matches!(
        instruction.opcode,
        Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)
    )
        && matches!(instruction.operands.get(1), Some(Operand::Constant(_)))
}

/// `Instruction::isAssociative && isCommutative`, DEVIATE 28.
fn is_associative_and_commutative(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
    )
}

/// `TailRecursionEliminator`
/// ([TailRecursionElimination.cpp:450](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L450)).
struct TailRecursionEliminator<'a> {
    context: &'a mut Context,
    layout: &'a DataLayout,
    function: &'a mut Function,
    declared: &'a mut Declared,
    outer: &'a Outer,
    id: Option<GlobalId>,
    tti: std::rc::Rc<dyn llrm_mir::tti::Tti>,

    // The below are shared state we want to have available when eliminating
    // any calls in the function. Their values are populated by
    // `create_tail_recurse_loop_header` the first time we find a call we can
    // eliminate.
    header_bb: Option<BlockId>,
    argument_phis: Vec<InstId>,
    /// PHI node to store our return value.
    ret_pn: Option<InstId>,
    /// i1 PHI node to track if we have a valid return value stored in `ret_pn`.
    ret_known_pn: Option<InstId>,
    /// Select instructions we inserted. These selects use `ret_known_pn` to
    /// either propagate `ret_pn` or select a new return value.
    ret_selects: Vec<InstId>,

    // The below are shared state needed when performing accumulator recursion.
    // Their values are populated by `insert_accumulator` the first time we find
    // an elimination that requires an accumulator.
    /// PHI node to store our current accumulated value.
    acc_pn: Option<InstId>,
    /// The instruction doing the accumulating.
    accumulator_recursion_instr: Option<InstId>,
    accumulator_initial_value: Option<Operand>,
    /// The operands of the phis made here, set once each when the pass is
    /// done: adding an arm to a phi costs its length in MIR, where LLVM's
    /// `addIncoming` is constant, so one call per site would be quadratic.
    incoming: IdMap<InstId, Vec<Operand>>,
}

impl<'a> TailRecursionEliminator<'a> {
    fn new(
        context: &'a mut Context,
        layout: &'a DataLayout,
        function: &'a mut Function,
        declared: &'a mut Declared,
        outer: &'a Outer,
        id: Option<GlobalId>,
    ) -> Self {
        let tti = crate::looprotate::target_tti(outer.target());
        Self {
            context,
            layout,
            function,
            declared,
            outer,
            id,
            tti,
            header_bb: None,
            argument_phis: Vec::new(),
            ret_pn: None,
            ret_known_pn: None,
            ret_selects: Vec::new(),
            acc_pn: None,
            accumulator_recursion_instr: None,
            accumulator_initial_value: None,
            incoming: IdMap::new(),
        }
    }

    fn result_of(
        &self,
        inst: InstId,
    ) -> Operand {
        Operand::Value(self.function.instruction(inst).result.expect("an instruction with a value"))
    }

    /// `findBaseCaseRetConstant`
    /// ([TailRecursionElimination.cpp:558](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L558)):
    /// the base-case return value for the function, given the accumulator
    /// recursion instruction `acc_rec_instr` that is about to be eliminated.
    /// Every return other than the one fed by it survives the transformation
    /// and will be rewritten to return the accumulator, so all of them have to
    /// yield the same base-case constant.
    fn find_base_case_ret_constant(
        &self,
        acc_rec_instr: InstId,
    ) -> Option<Operand> {
        let mut base_case_val: Option<Operand> = None;
        // Records C as the base-case constant the first time it's seen, and
        // otherwise checks that it agrees with the one already on record.
        let mut set_or_match = |c: Operand| {
            if base_case_val.is_none() {
                base_case_val = Some(c);
            }
            base_case_val == Some(c)
        };
        let function = &*self.function;
        for &block in function.layout() {
            let Some(ret) = function.terminator(block).filter(|&one| function.instruction(one).opcode == Opcode::Ret)
            else {
                continue;
            };
            let Some(&rv) = function.instruction(ret).operands.first() else { continue };
            // This is the recursive case being turned into a loop: the return
            // goes away along with `acc_rec_instr`.
            if Some(rv) == function.instruction(acc_rec_instr).result.map(Operand::Value) {
                continue;
            }
            // Anything else has to be the base case. In particular a return
            // still computing from a recursive call (e.g. a second recursion
            // site that is not eliminated) must be rejected: returning the
            // accumulator in its place would drop that computation.
            if !matches!(rv, Operand::Constant(_)) || !set_or_match(rv) {
                return None;
            }
        }
        for &select in &self.ret_selects {
            let false_value = function.instruction(select).operands[2];
            if !matches!(false_value, Operand::Constant(_)) || !set_or_match(false_value) {
                return None;
            }
        }
        base_case_val
    }

    /// `canTransformAccumulatorRecursion`
    /// ([TailRecursionElimination.cpp:603](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L603)):
    /// whether the instruction `inst` can be used to perform accumulator
    /// recursion elimination for the call `call`; the accumulator's initial
    /// value.
    fn can_transform_accumulator_recursion(
        &self,
        inst: InstId,
        call: InstId,
    ) -> Option<Operand> {
        let function = &*self.function;
        let instruction = function.instruction(inst);
        let is_unary = is_unary_accumulator_recurrence(function, inst);
        let Opcode::Binary(op) = instruction.opcode else { return None };
        if !is_associative_and_commutative(op) && !is_unary {
            return None;
        }
        let call_result = function.instruction(call).result.map(Operand::Value);
        let init = if is_unary {
            // For unary accumulator recurrences, we require that the recursive
            // call is always on the first operand.
            if Some(instruction.operands[0]) != call_result {
                return None;
            }
            // `find_tre_candidate` guarantees `call` is a recursive call to its
            // own function, so scan the enclosing function for the base-case
            // return.
            self.find_base_case_ret_constant(inst)?
        } else {
            let init = identity(self.context, op, instruction.ty);
            // Exactly one operand should be the result of the call instruction.
            let [left, right] = instruction.operands[..] else { return None };
            if (Some(left) == call_result) == (Some(right) == call_result) {
                return None;
            }
            init
        };
        // The only user of this instruction we allow is a single return
        // instruction.
        let result = instruction.result?;
        let only = function.users(result).only()?;
        (function.instruction(only.user).opcode == Opcode::Ret).then_some(init)
    }

    /// `findTRECandidate`
    /// ([TailRecursionElimination.cpp:643](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L643)).
    fn find_tre_candidate(
        &self,
        block: BlockId,
    ) -> Option<InstId> {
        let function = &*self.function;
        let terminator = function.terminator(block)?;
        // Make sure there is something before the terminator.
        if function.iter_block(block).next() == Some(terminator) {
            return None;
        }
        // Scan backwards from the return, checking to see if there is a tail
        // call in this block. If so, set `call` to it.
        let call = function
            .iter_block_rev(block)
            .find(
                |&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_))
                    && self.id.is_some()
                    && memory::callee(self.context, function, inst) == self.id,
            )?;
        let Opcode::Call(info) = &function.instruction(call).opcode else { return None };
        if !matches!(info.tail, Tail::Tail | Tail::MustTail) {
            return None;
        }
        // As a special case, detect code like this:
        //   double fabs(double f) { return __builtin_fabs(f); } // a 'fabs'
        // call and disable this xform in this case, because the code
        // generator will lower the call to fabs into inline code.
        let first = function.iter_block(block).next();
        let second = function.iter_block(block).nth(1);
        if Some(block) == function.entry() && first == Some(call) && second == Some(terminator) {
            let callee = self.id.and_then(|one| self.outer.globals.get(one.0 as usize));
            if let Some(callee) = callee {
                let lowered = self
                    .tti
                    .is_lowered_to_call(
                        callee.name.as_deref(),
                        matches!(
                            callee.linkage,
                            llrm_mir::module::Linkage::Internal | llrm_mir::module::Linkage::Private
                        ),
                    );
                // A single-block function with just a call and a return. Check
                // that the arguments match.
                if !lowered
                    && function.instruction(call).operands[..function.instruction(call).operands.len() - 1]
                        == function.parameters().iter().map(|&one| Operand::Value(one)).collect::<Vec<_>>()[..]
                {
                    return None;
                }
            }
        }
        Some(call)
    }

    /// `createTailRecurseLoopHeader`
    /// ([TailRecursionElimination.cpp:688](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L688)).
    fn create_tail_recurse_loop_header(&mut self) {
        let header = self.function.entry().expect("a body");
        self.header_bb = Some(header);
        let new_entry = self.function.create_block(None);
        self.function.insert_block(new_entry, Some(header)).expect("a new block");
        self.function.move_block_after(header, new_entry).expect("a placed block");
        self.function.take_block_name(header, new_entry);
        self.function.set_block_name(header, "tailrecurse");
        let void = self.context.types.void();
        let jump =
            self.function.create_instruction(Opcode::Br, void, vec![Operand::Block(header)], Flags::default(), None);
        self.function.insert(jump, Position::End(new_entry)).expect("a placed block");

        // Move all fixed sized allocas from `header` to `new_entry`.
        for inst in self.function.iter_block(header).collect::<Vec<_>>() {
            if matches!(self.function.instruction(inst).opcode, Opcode::Alloca { .. })
                && self.function.instruction(inst).operands.iter().all(|one| matches!(one, Operand::Constant(_)))
            {
                self.function.move_to(inst, Position::Before(jump)).expect("a placed instruction");
            }
        }

        // Now that we have created a new block, which jumps to the entry
        // block, insert a PHI node for each argument of the function. For now,
        // we initialize each PHI to only have the real arguments which are
        // passed in.
        let at = self.function.iter_block(header).next().expect("the header holds the body");
        for parameter in self.function.parameters().to_vec() {
            let ty = self.function.value(parameter).ty;
            let name = self.function.value(parameter).name.clone().map(|one| format!("{one}.tr"));
            let phi = self.function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), name.as_deref());
            self.function.insert(phi, Position::Before(at)).expect("a placed instruction");
            // Everyone use the PHI node now!
            let value = self.result_of(phi);
            self.function.replace_all_uses_with(parameter, value);
            self.add_incoming(phi, Operand::Value(parameter), new_entry);
            self.argument_phis.push(phi);
        }

        // If the function doesn't return void, create the `ret_pn` and
        // `ret_known_pn` PHI nodes to track our return value. We initialize
        // `ret_pn` with poison and `ret_known_pn` with false since we can't
        // know our return value at function entry.
        let ret_type = function_parts(self.context, self.function.ty).0;
        if ret_type != void {
            let bool_type = self.context.types.int(1);
            let ret_pn =
                self.function.create_instruction(Opcode::Phi, ret_type, Vec::new(), Flags::default(), Some("ret.tr"));
            self.function.insert(ret_pn, Position::Before(at)).expect("a placed instruction");
            let ret_known = self
                .function
                .create_instruction(Opcode::Phi, bool_type, Vec::new(), Flags::default(), Some("ret.known.tr"));
            self.function.insert(ret_known, Position::Before(at)).expect("a placed instruction");
            let poison =
                Operand::Constant(self.context.constant(Constant { ty: ret_type, kind: ConstantKind::Poison }));
            let no = Operand::Constant(self.context.int(bool_type, 0));
            self.add_incoming(ret_pn, poison, new_entry);
            self.add_incoming(ret_known, no, new_entry);
            self.ret_pn = Some(ret_pn);
            self.ret_known_pn = Some(ret_known);
        }
    }

    /// `PHINode::addIncoming`: the arm is kept until `flush_incoming`.
    fn add_incoming(
        &mut self,
        phi: InstId,
        value: Operand,
        block: BlockId,
    ) {
        self.incoming.get_or_insert_with(phi, Vec::new).extend([value, Operand::Block(block)]);
    }

    /// The arms added so far become the phis' operands.
    fn flush_incoming(&mut self) {
        for (phi, operands) in std::mem::take(&mut self.incoming).iter() {
            self.function.set_operands(phi, operands.clone());
        }
    }

    /// `insertAccumulator`
    /// ([TailRecursionElimination.cpp:742](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L742)).
    fn insert_accumulator(
        &mut self,
        acc_rec_instr: InstId,
    ) {
        assert!(self.acc_pn.is_none(), "Trying to insert multiple accumulators");
        self.accumulator_recursion_instr = Some(acc_rec_instr);
        let header = self.header_bb.expect("the header exists");
        // Start by inserting a new PHI node for the accumulator.
        let ret_type = function_parts(self.context, self.function.ty).0;
        let acc = self
            .function
            .create_instruction(Opcode::Phi, ret_type, Vec::new(), Flags::default(), Some("accumulator.tr"));
        let first = self.function.iter_block(header).next().expect("the header is not empty");
        self.function.insert(acc, Position::Before(first)).expect("a placed instruction");
        self.acc_pn = Some(acc);
        // Loop over all of the predecessors of the tail recursion block. For
        // the real entry into the function we seed the PHI with the identity
        // constant for the accumulation operation. For any other existing
        // branches to this block (due to other tail recursions eliminated) the
        // accumulator is not modified. Because we haven't added the branch in
        // the current block to the header yet, it will not show up as a
        // predecessor.
        let entry = self.function.entry().expect("a body");
        let acc_value = self.result_of(acc);
        let initial = self.accumulator_initial_value.expect("an accumulator has an initial value");
        for pred in self.function.predecessors(header).to_vec() {
            let value = if pred == entry { initial } else { acc_value };
            self.add_incoming(acc, value, pred);
        }
    }

    /// `IRBuilder::CreateMemCpy` of the size of `aggregate`, at `at`: an
    /// `llvm.memcpy` of `i64` bytes, `alignment` on both pointers.
    fn build_memcpy(
        &mut self,
        at: Position,
        destination: Operand,
        source: Operand,
        alignment: u64,
        aggregate: TypeId,
    ) {
        let (callee, function_type) =
            crate::loopidioms::_copy(self.context, self.declared, crate::loopidioms::How::Apart, 0, 0, 64);
        let size = self.layout.alloc_size(&self.context.types, aggregate) as i128;
        let (i64_type, i1_type, pointer_type) =
            (self.context.types.int(64), self.context.types.int(1), self.context.types.ptr(0));
        let size = Operand::Constant(self.context.int(i64_type, size));
        let not_volatile = Operand::Constant(self.context.int(i1_type, 0));
        let callee =
            Operand::Constant(self.context.constant(Constant { ty: pointer_type, kind: ConstantKind::Global(callee) }));
        let aligned = vec![Attribute::Int("align".to_owned(), alignment)];
        let info = CallInfo {
            function_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![aligned.clone(), aligned, Vec::new(), Vec::new()],
            attrs: Vec::new(),
            tail: Tail::None,
        };
        let void = self.context.types.void();
        let call = self
            .function
            .create_instruction(
                Opcode::Call(Box::new(info)),
                void,
                vec![destination, source, size, not_volatile, callee],
                Flags::default(),
                None,
            );
        self.function.insert(call, at).expect("a placed position");
    }

    /// `copyByValueOperandIntoLocalTemp`
    /// ([TailRecursionElimination.cpp:773](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L773)):
    /// a copy of the contents of the byval operand `index` of `call` in a new
    /// temporary variable.
    fn copy_by_value_operand_into_local_temp(
        &mut self,
        call: InstId,
        index: usize,
    ) {
        let (aggregate, alignment) = by_value_type(self.function, call, index);
        let entry = self.function.entry().expect("a body");
        let pointer = self.function.instruction(call).operands[index];
        let name = match pointer {
            Operand::Value(value) => self.function.value(value).name.clone(),
            _ => None,
        };
        let ptr_ty = self.function.operand_type(self.context, pointer).expect("a pointer operand");
        let one = Operand::Constant(self.context.int(self.context.types.int(32), 1));
        let alloca = self
            .function
            .create_instruction(
                Opcode::Alloca { allocated: aggregate, align: Some(alignment), address_space: 0 },
                ptr_ty,
                vec![one],
                Flags::default(),
                name.as_deref(),
            );
        let first = self.function.iter_block(entry).next().expect("the entry is not empty");
        self.function.insert(alloca, Position::Before(first)).expect("a placed instruction");
        self.build_memcpy(Position::Before(call), self.result_of(alloca), pointer, alignment, aggregate);
        let temp = self.result_of(alloca);
        self.function.set_operand(call, index, temp);
    }

    /// `copyLocalTempOfByValueOperandIntoArguments`
    /// ([TailRecursionElimination.cpp:800](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L800)):
    /// a copy from the temporary variable holding the byval argument into the
    /// corresponding function argument location.
    fn copy_local_temp_of_by_value_operand_into_arguments(
        &mut self,
        call: InstId,
        index: usize,
    ) {
        let (aggregate, alignment) = by_value_type(self.function, call, index);
        let source = self.function.instruction(call).operands[index];
        let destination = Operand::Value(self.function.parameters()[index]);
        self.build_memcpy(Position::Before(call), destination, source, alignment, aggregate);
    }

    /// `eliminateCall`
    /// ([TailRecursionElimination.cpp:819](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L819)).
    fn eliminate_call(
        &mut self,
        call: InstId,
    ) -> bool {
        let block = self.function.parent(call).expect("a placed call");
        let ret = self.function.terminator(block).expect("a return");
        // Ok, we found a potential tail call. We can currently only transform
        // the tail call if all of the instructions between the call and the
        // return are movable to above the call itself, leaving the call next to
        // the return. Check that this is the case now.
        let mut acc_rec_instr: Option<InstId> = None;
        let between: Vec<InstId> = {
            let mut after = self.function.iter_block(block).skip_while(|&one| one != call);
            after.next();
            after.take_while(|&one| one != ret).collect()
        };
        for inst in between {
            if can_move_above_call(self.context, self.layout, self.outer, self.function, inst, call) {
                continue;
            }
            // If we can't move the instruction above the call, it might be
            // because it is an (associative and commutative) or unary
            // accumulator recurrence arithmetic operation that could be
            // transformed using accumulator recursion elimination. Check to see
            // if this is the case, and if so, remember which instruction
            // accumulates for later.
            let acc_init_val = self.can_transform_accumulator_recursion(inst, call);
            let Some(acc_init_val) = acc_init_val.filter(|_| self.acc_pn.is_none()) else {
                return false; // We cannot eliminate the tail recursion!
            };
            // Yes, this is accumulator recursion. Remember which instruction
            // accumulates.
            acc_rec_instr = Some(inst);
            // Keep track of the base case (i.e., initial value) of the
            // accumulator return value if any.
            self.accumulator_initial_value = Some(acc_init_val);
        }

        // OK! We can transform this tail call. If this is the first one found,
        // create the new entry block, allowing us to branch back to the old
        // entry.
        if self.header_bb.is_none() {
            self.create_tail_recurse_loop_header();
        }

        let arguments = self.function.instruction(call).operands.len() - 1;
        // Copy values of ByVal operands into local temporarily variables.
        for index in 0..arguments {
            if is_by_value_argument(self.function, call, index) {
                self.copy_by_value_operand_into_local_temp(call, index);
            }
        }

        // Ok, now that we know we have a pseudo-entry block WITH all of the
        // required PHI nodes, add entries into the PHI node for the actual
        // parameters passed into the tail-recursive call.
        for index in 0..arguments {
            let phi = self.argument_phis[index];
            if is_by_value_argument(self.function, call, index) {
                self.copy_local_temp_of_by_value_operand_into_arguments(call, index);
                // When eliminating a tail call, we modify the values of the
                // arguments. Therefore, if the byval parameter has a readonly
                // attribute, we have to remove it. It is safe because, from the
                // perspective of a caller, the byval parameter is always
                // treated as "readonly," even if the readonly attribute is
                // removed.
                self.function.parameter_attrs[index].retain(|attr| Fact::of_attribute(attr) != Some(Fact::ReadOnly));
                let parameter = Operand::Value(self.function.parameters()[index]);
                self.add_incoming(phi, parameter, block);
            } else {
                let argument = self.function.instruction(call).operands[index];
                self.add_incoming(phi, argument, block);
            }
        }

        if let Some(acc_rec_instr) = acc_rec_instr {
            self.insert_accumulator(acc_rec_instr);
            let acc = self.result_of(self.acc_pn.expect("just inserted"));
            // Rewrite the accumulator recursion instruction so that it does not
            // use the result of the call anymore, instead, use the PHI node we
            // just inserted.
            let call_result = self.function.instruction(call).result.map(Operand::Value);
            let at = usize::from(
                self.function.instruction(acc_rec_instr).operands[0] != call_result.expect("a call result"),
            );
            self.function.set_operand(acc_rec_instr, at, acc);
            // Reassociating into the loop reorders the operands, so flags from
            // the original order (nsw/nuw/exact/...) may no longer hold.
            crate::local::drop_poison_generating_flags(self.context, self.function, acc_rec_instr);
        }

        // Update our return value tracking
        if let (Some(ret_pn), Some(ret_known)) = (self.ret_pn, self.ret_known_pn) {
            let returned = self.function.instruction(ret).operands.first().copied();
            let call_result = self.function.instruction(call).result.map(Operand::Value);
            if returned == call_result || acc_rec_instr.is_some() {
                // Defer selecting a return value
                let (ret_value, known_value) = (self.result_of(ret_pn), self.result_of(ret_known));
                self.add_incoming(ret_pn, ret_value, block);
                self.add_incoming(ret_known, known_value, block);
            } else {
                // We found a return value we want to use, insert a select
                // instruction to select it if we don't already know what our
                // return value will be and store the result in our return value
                // PHI node.
                let returned = returned.expect("a returned value");
                let ret_type = function_parts(self.context, self.function.ty).0;
                let select = self
                    .function
                    .create_instruction(
                        Opcode::Select,
                        ret_type,
                        vec![self.result_of(ret_known), self.result_of(ret_pn), returned],
                        Flags::default(),
                        Some("current.ret.tr"),
                    );
                self.function.insert(select, Position::Before(ret)).expect("a placed return");
                self.ret_selects.push(select);
                let selected = self.result_of(select);
                self.add_incoming(ret_pn, selected, block);
                let yes = Operand::Constant(self.context.int(self.context.types.int(1), 1));
                self.add_incoming(ret_known, yes, block);
            }
            if let Some(acc_pn) = self.acc_pn {
                let value = match acc_rec_instr {
                    Some(inst) => self.result_of(inst),
                    None => self.result_of(acc_pn),
                };
                self.add_incoming(acc_pn, value, block);
            }
        }

        // Now that all of the PHI nodes are in place, remove the call and ret
        // instructions, replacing them with an unconditional branch.
        let header = self.header_bb.expect("the header exists");
        let void = self.context.types.void();
        let branch =
            self.function.create_instruction(Opcode::Br, void, vec![Operand::Block(header)], Flags::default(), None);
        self.function.insert(branch, Position::Before(ret)).expect("a placed return");
        self.function.erase(ret).expect("a return defines nothing");
        self.function.erase(call).expect("the call's users are gone");
        true
    }

    /// `cleanupAndFinalize`
    /// ([TailRecursionElimination.cpp:959](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L959)).
    fn cleanup_and_finalize(&mut self) {
        self.flush_incoming();
        // If we eliminated any tail recursions, it's possible that we inserted
        // some silly PHI nodes which just merge an initial value (the incoming
        // operand) with themselves. Check to see if we did and clean up our
        // mess if so. This occurs when a function passes an argument straight
        // through to its tail call.
        for phi in std::mem::take(&mut self.argument_phis) {
            if let Some(value) = llrm_mir::simplify::simplify_instruction(self.context, self.layout, self.function, phi)
            {
                let result = self.function.instruction(phi).result.expect("a phi has a value");
                self.function.replace_all_uses_with(result, value);
                self.function.erase(phi).expect("its users are gone");
            }
        }

        let (Some(ret_pn), Some(ret_known)) = (self.ret_pn, self.ret_known_pn) else { return };
        let acc_rec_instr = self.accumulator_recursion_instr;
        let acc_pn = self.acc_pn;
        let returns: Vec<InstId> = self
            .function
            .layout()
            .to_vec()
            .into_iter()
            .filter_map(|block| self.function.terminator(block))
            .filter(|&inst| self.function.instruction(inst).opcode == Opcode::Ret)
            .collect();

        if self.ret_selects.is_empty() {
            // If we didn't insert any select instructions, then we know we
            // didn't store a return value and we can remove the PHI nodes we
            // inserted.
            for phi in [ret_pn, ret_known] {
                self.function.set_operands(phi, Vec::new());
            }
            for phi in [ret_pn, ret_known] {
                self.function.erase(phi).expect("its users are gone");
            }
            if let (Some(acc_pn), Some(acc_rec_instr)) = (acc_pn, acc_rec_instr) {
                // We need to insert a copy of our accumulator instruction
                // before any return in the function, and return its result
                // instead.
                for ret in returns {
                    if is_unary_accumulator_recurrence(self.function, acc_rec_instr) {
                        // Base-case initialization: the accumulator PHI already
                        // holds the final result, so return it directly.
                        let value = self.result_of(acc_pn);
                        self.function.set_operand(ret, 0, value);
                    } else {
                        // Since the accumulator starts with the identity value,
                        // before the return we need to apply the accumulation
                        // instruction one more time to combine the last value
                        // with the result of the recursive call.
                        let other = self.function.instruction(ret).operands[0];
                        let new = self.materialize_accumulator(acc_rec_instr, acc_pn, other, Position::Before(ret));
                        self.function.set_operand(ret, 0, new);
                    }
                }
            }
        } else {
            // We need to insert a select instruction before any return left in
            // the function to select our stored return value if we have one.
            let ret_type = function_parts(self.context, self.function.ty).0;
            for ret in returns {
                let value = self.function.instruction(ret).operands[0];
                let select = self
                    .function
                    .create_instruction(
                        Opcode::Select,
                        ret_type,
                        vec![self.result_of(ret_known), self.result_of(ret_pn), value],
                        Flags::default(),
                        Some("current.ret.tr"),
                    );
                self.function.insert(select, Position::Before(ret)).expect("a placed return");
                self.ret_selects.push(select);
                let selected = self.result_of(select);
                self.function.set_operand(ret, 0, selected);
            }
            if let (Some(acc_pn), Some(acc_rec_instr)) = (acc_pn, acc_rec_instr) {
                // We need to insert a copy of our accumulator instruction
                // before any of the selects we inserted, and select its result
                // instead.
                for select in self.ret_selects.clone() {
                    if is_unary_accumulator_recurrence(self.function, acc_rec_instr) {
                        let value = self.result_of(acc_pn);
                        self.function.set_operand(select, 2, value);
                    } else {
                        let other = self.function.instruction(select).operands[2];
                        let new = self.materialize_accumulator(acc_rec_instr, acc_pn, other, Position::Before(select));
                        self.function.set_operand(select, 2, new);
                    }
                }
            }
        }
    }

    /// `MaterializeAccumulator`: a clone of the accumulating instruction with
    /// `other` in place of the accumulator, at `at`.
    fn materialize_accumulator(
        &mut self,
        acc_rec_instr: InstId,
        acc_pn: InstId,
        other: Operand,
        at: Position,
    ) -> Operand {
        let new = self.function.clone_instruction(acc_rec_instr);
        let acc = self.result_of(acc_pn);
        let slot = usize::from(self.function.instruction(acc_rec_instr).operands[0] == acc);
        self.function.set_operand(new, slot, other);
        self.function.insert(new, at).expect("a placed position");
        let result = self.function.instruction(new).result.expect("a value");
        self.function.set_name(result, "accumulator.ret.tr");
        Operand::Value(result)
    }

    /// `processBlock`
    /// ([TailRecursionElimination.cpp:1048](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/TailRecursionElimination.cpp#L1048)).
    fn process_block(
        &mut self,
        block: BlockId,
    ) -> bool {
        let Some(terminator) = self.function.terminator(block) else { return false };
        let instruction = self.function.instruction(terminator);
        if instruction.opcode == Opcode::Br && instruction.operands.len() == 1 {
            let Operand::Block(successor) = instruction.operands[0] else { return false };
            let Some(ret) = first_non_phi_or_dbg(self.function, successor)
                .filter(|&one| self.function.instruction(one).opcode == Opcode::Ret)
            else {
                return false;
            };
            let Some(call) = self.find_tre_candidate(block) else { return false };
            fold_return_into_uncond_branch(self.function, ret, successor, block);
            // If all predecessors of the successor have been eliminated by
            // `fold_return_into_uncond_branch`, delete it. It is important to
            // empty it, because the ret instruction in there is still using a
            // value which `eliminate_call` will attempt to remove. This block
            // can only contain instructions that can't have uses, therefore it
            // is safe to remove.
            if self.function.predecessors(successor).is_empty() {
                for inst in self.function.iter_block_rev(successor).collect::<Vec<_>>() {
                    self.function.erase(inst).expect("only the block's own instructions use them");
                }
                self.function.erase_block(successor).expect("an empty block nothing names");
            }
            self.eliminate_call(call);
            return true;
        }
        if instruction.opcode == Opcode::Ret {
            if let Some(call) = self.find_tre_candidate(block) {
                return self.eliminate_call(call);
            }
        }
        false
    }
}

/// The return type of the function type `ty`, and whether it takes variadic
/// arguments.
fn function_parts(
    context: &Context,
    ty: TypeId,
) -> (TypeId, bool) {
    match context.types.get(ty) {
        Type::Function { returns, variadic, .. } => (*returns, *variadic),
        other => unreachable!("a function's type is a function type, not {other:?}"),
    }
}

/// `BasicBlock::getFirstNonPHIOrDbg`.
fn first_non_phi_or_dbg(
    function: &Function,
    block: BlockId,
) -> Option<InstId> {
    function.iter_block(block).find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
}

/// Whether argument `index` of `call` is passed byval.
fn is_by_value_argument(
    function: &Function,
    call: InstId,
    index: usize,
) -> bool {
    matches!(
        &function.instruction(call).opcode,
        Opcode::Call(info) if has_type_attr(info.argument_attrs.get(index), "byval")
    )
}

/// `getParamByValType` and `getParamAlign().valueOrOne()` of argument `index`.
fn by_value_type(
    function: &Function,
    call: InstId,
    index: usize,
) -> (TypeId, u64) {
    let Opcode::Call(info) = &function.instruction(call).opcode else { unreachable!("a call") };
    let attrs = &info.argument_attrs[index];
    let aggregate = attrs
        .iter()
        .find_map(|attr| match attr {
            Attribute::Type(name, ty) if name == "byval" => Some(*ty),
            _ => None,
        })
        .expect("a byval argument has a type");
    let alignment = attrs
        .iter()
        .find_map(|attr| match attr {
            Attribute::Int(name, value) if name == "align" => Some(*value),
            _ => None,
        })
        .unwrap_or(1);
    (aggregate, alignment)
}

/// `ConstantExpr::getIdentity` of the commutative binary operation `op`
/// ([Constants.cpp:2921](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Constants.cpp#L2921)).
fn identity(
    context: &Context,
    op: BinaryOp,
    ty: TypeId,
) -> Operand {
    let value = match op {
        BinaryOp::Mul => 1,
        BinaryOp::And => -1,
        _ => 0,
    };
    Operand::Constant(context.int(ty, value))
}

/// `FoldReturnIntoUncondBranch`
/// ([BasicBlockUtils.cpp:1532](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L1532)):
/// the return `ret` of `block` copied to the end of `pred`, whose unconditional
/// branch to `block` is erased.
pub(crate) fn fold_return_into_uncond_branch(
    function: &mut Function,
    ret: InstId,
    block: BlockId,
    pred: BlockId,
) -> InstId {
    let branch = function.terminator(pred).expect("a terminator");
    // Clone the return and add it to the end of the predecessor.
    let new_ret = function.clone_instruction(ret);
    function.insert(new_ret, Position::End(pred)).expect("a placed block");
    // If the return instruction returns a value, and if the value was a PHI
    // node in `block`, propagate the right value into the return.
    for slot in 0..function.instruction(new_ret).operands.len() {
        let mut value = function.instruction(new_ret).operands[slot];
        let mut new_cast: Option<InstId> = None;
        if let Some(cast) = defining(function, value)
            .filter(|&one| matches!(
                function.instruction(one).opcode,
                Opcode::Cast(CastOp::BitCast)
            ))
        {
            // Return value might be bitcasted. Clone and insert it before the
            // return instruction.
            value = function.instruction(cast).operands[0];
            let copy = function.clone_instruction(cast);
            function.insert(copy, Position::Before(new_ret)).expect("a placed return");
            function.set_operand(new_ret, slot, Operand::Value(function.instruction(copy).result.expect("a value")));
            new_cast = Some(copy);
        }
        let mut new_extract: Option<InstId> = None;
        if let Some(extract) =
            defining(function, value).filter(|&one| matches!(function.instruction(one).opcode, Opcode::ExtractValue(_)))
        {
            value = function.instruction(extract).operands[0];
            let copy = function.clone_instruction(extract);
            let result = Operand::Value(function.instruction(copy).result.expect("a value"));
            if let Some(cast) = new_cast {
                function.set_operand(cast, 0, result);
                function.insert(copy, Position::Before(cast)).expect("a placed cast");
            } else {
                function.insert(copy, Position::Before(new_ret)).expect("a placed return");
                function.set_operand(new_ret, slot, result);
            }
            new_extract = Some(copy);
        }
        if let Some(phi) = defining(function, value)
            .filter(|&one| function.instruction(one).opcode == Opcode::Phi && function.parent(one) == Some(block))
        {
            let incoming = arms(function, phi)
                .into_iter()
                .find(|&(_, from)| from == pred)
                .map(|(value, _)| value)
                .expect("an arm for the predecessor");
            if let Some(extract) = new_extract {
                function.set_operand(extract, 0, incoming);
            } else if let Some(cast) = new_cast {
                function.set_operand(cast, 0, incoming);
            } else {
                function.set_operand(new_ret, slot, incoming);
            }
        }
    }
    // Update any PHI nodes in the returning block to realize that we no longer
    // branch to them.
    for phi in crate::edges::phis(function, block) {
        let kept: Vec<_> = arms(function, phi).into_iter().filter(|&(_, from)| from != pred).collect();
        function.set_operands(phi, from_arms(&kept));
    }
    function.erase(branch).expect("a branch defines nothing");
    new_ret
}

/// The instruction that defines `operand`, if one does.
fn defining(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        },
        _ => None,
    }
}

#[cfg(test)]
#[path = "tailcallelim_tests.rs"]
mod tests;
