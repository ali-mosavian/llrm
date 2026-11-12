//! `LoopRotate`: a loop with its exit test in the header becomes a loop with
//! its exit test in the latch, behind a copy of the test that guards the entry.
//!
//! # How it works
//! `process_loop` first folds the latch into the block that exits before it, if
//! that is a few speculated instructions (`simplify_loop_latch`), then
//! `rotate_loop` rotates a loop whose header ends in the exit test.
//!
//! The header's instructions are copied, in order, into the preheader: one
//! whose operands are all loop-invariant and that touches no memory is moved
//! instead of copied; a copy that simplifies (its phi operands now the values
//! from the preheader) is replaced by what it simplifies to. The preheader then
//! ends in a copy of the header's branch, so it is a guard: the loop is entered
//! only if the test passes. The header's phis lose their preheader input, and
//! every use of a header value the loop no longer dominates is rewritten with
//! `SsaUpdater` to the preheader's copy or the header's. The header's
//! successor in the loop is the new header; the old header is merged into the
//! latch if they are joined by a jump.
//!
//! ```text
//!   pre -> H(test) -> F -> L -> H            pre(test') -> F' -> L(H, test) -> F'
//!             \-> X                                  \-> X              \-> X
//! ```
//!
//! # Deviations
//! - R1 No `DominatorTree`, `LoopInfo`, `ScalarEvolution`, `MemorySSA` or
//!   `AssumptionCache` to update (D8 of the plan page): they are derived from
//!   the function. `process_loop` reads one `LoopInfo` made before the
//!   rotation; the blocks the rotation adds belong to the loops outside the
//!   rotated one, which the driver derives again before it reaches them.
//! - R2 `check-exit-count` is the late slot's option
//!   (PassBuilderPipelines.cpp#L1621); R7 below prices it.
//! - R7 `profitableToRotateLoopExitingLatch` under that option makes a
//!   ScalarEvolution session of its own for the question.
//! - R3 New blocks go at the end of the layout: `Function::insert_block` after
//!   a block scans the layout. LLVM puts them next to their neighbours.
//! - R4 No `llvm.experimental.noalias.scope.decl` scope cloning and no
//!   `llvm.coro` check: MIR has no local noalias scope declarations, and no
//!   coroutines. Debug records stand before an instruction and move with it, so
//!   only the preheader's old terminator needs `flushTerminatorDbgRecords`.
//! - R5 The dominator tree and assumptions `simplifyInstruction` is given are
//!   built from the function when first asked and dropped when a loop rotates
//!   (the pin keeps its own up to date).
//! - R6 A target with no cost model (`Machine::tti`) is priced by the generic
//!   profile, `llrm_mir::tti::Basic`: a pass names no target crate.
//!
//! # Example
//! Before:
//! ```mir
//! b0:
//!   br label %b1
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
//!   %go = icmp slt i16 %i, %n
//!   br i1 %go, label %b2, label %b3
//! b2:
//!   %next = add i16 %i, 1
//!   br label %b1
//! b3:
//!   ret i16 %i
//! ```
//! After, the test copied as the guard and the old header merged into the
//! latch:
//! ```mir
//! b0:
//!   %g = icmp slt i16 0, %n
//!   br i1 %g, label %ph, label %b3
//! ph:
//!   br label %b2
//! b2:
//!   %i = phi i16 [ 0, %ph ], [ %next, %b2 ]
//!   %next = add i16 %i, 1
//!   %go = icmp slt i16 %next, %n
//!   br i1 %go, label %b2, label %exit
//! exit:
//!   %i.lcssa = phi i16 [ %next, %b2 ]
//!   br label %b3
//! b3:
//!   %r = phi i16 [ %i.lcssa, %exit ], [ 0, %b0 ]
//!   ret i16 %r
//! ```
//!
//! # Used by
//! The `loop-rotate` slot of the pipeline.
//!
//! # See also
//! [`LoopRotate::rotateLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopRotationUtils.cpp#L348),
//! [`simplifyLoopLatch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopRotationUtils.cpp#L908),
//! [`processLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopRotationUtils.cpp#L946).

use llrm_analysis::ssaupdater::SsaUpdater;
use llrm_mir::codemetrics::{CodeMetrics, Convergence, Query, collect_ephemeral_values};
use llrm_mir::context::{ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::loopinfo::{Loop, LoopInfo};
use llrm_mir::memory::Callees;
use llrm_mir::module::{
    BlockId, Function, GlobalValue, InstId, MetadataId, MetadataNode, MetadataOperand, Operand, ValueDef, ValueId,
};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::tti::Tti;

use crate::lcssa::{arms, from_arms};
use crate::{cfg, edges, local, trial};

/// `ZeroTripCountWeights`: the probability that a rotated loop has zero trip
/// count.
const ZERO_TRIP_COUNT_WEIGHTS: [u32; 2] = [1, 127];

/// What `LoopRotation` is asked for.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// `MaxHeaderSize`.
    pub max_header_size: u32,
    /// DEVIATE (size builds, target opt-in): the header is also held to the
    /// `CodeSize` of the branches the rotation removes, the loop-closing jump
    /// and the header's exit test.
    pub priced_at_size: bool,
    /// DEVIATE RS1 (size builds, target opt-in, owners m64 and arm64, exit 2026-11-20):
    /// a loop is rotated only if the copied exit test folds in the
    /// preheader. gcc's `ch_base::copy_headers` copies at `-Os` only when the
    /// code does not grow; an unfolded guard is a compare and a branch
    /// against the one jump the rotation removes.
    pub guard_must_fold: bool,
    /// DEVIATE ROTATION-TRIAL (size builds, target opt-in, owner greedy-os-causes, exit
    /// 2026-12-05): a counted loop whose copied guard does not fold, which RS1 still
    /// rotates, is left as it is in the driver's second compile and the smaller object
    /// is kept (`trial`). gcc's `ch_base::copy_headers` leaves such a loop unrotated
    /// at -Os; whether that pays depends on what the later passes do with the loop,
    /// which only the finished code shows (x_prefix -12 B, x_transpose +34 B).
    pub rotation_trial: bool,
    /// DEVIATE Z80-ROT (-Oz, target opt-in, owner z80, exit 2026-11-21): the pin's
    /// minsize limit of no copied instruction is waived for a loop whose copied
    /// header folds away in the preheader, guard and all: the copy costs no byte
    /// and the loop-closing jump goes (gcc's `ch_base::copy_headers` at -Os).
    pub free_guard: bool,
    /// `RotationOnly`: no `simplifyLoopLatch`.
    pub rotation_only: bool,
    /// `IsUtilMode`: rotate even a loop whose latch already exits.
    pub is_util_mode: bool,
    pub prepare_for_lto: bool,
    /// `CheckExitCount`: a loop whose latch exit count rotating makes
    /// computable is rotated (the late slot, PassBuilderPipelines.cpp#L1621).
    pub check_exit_count: bool,
}

/// What the rotation reads and changes.
pub struct Run<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub globals: &'a [GlobalValue],
    pub callees: &'a Callees,
    pub this: Option<GlobalId>,
    pub tti: &'a dyn Tti,
    pub function: &'a mut Function,
    /// The module's metadata nodes.
    pub metadata: &'a [MetadataNode],
    pub declared: &'a mut Declared,
    pub options: Options,
    /// `Function::hasOneLiveUse`.
    pub has_one_live_use: &'a dyn Fn(GlobalId) -> bool,
    /// Blocks emptied by a merge, erased together by `finish` (R3's twin:
    /// erasing one scans the layout).
    pub dead: IdSet<BlockId>,
    /// The expressions `profitable_to_rotate_loop_exiting_latch` asks about,
    /// kept while no loop has changed (R7); a rotation drops them.
    scev: std::cell::RefCell<Option<llrm_analysis::scev::analysis::ScevSession>>,
    /// `SQ`'s dominator tree and assumptions (R5), kept like `scev`.
    dom: std::cell::RefCell<Option<llrm_mir::valuetracking::FunctionAnalyses>>,
}

impl Run<'_> {
    /// Erases the blocks the rotations emptied.
    pub fn finish(&mut self) {
        let dead = std::mem::take(&mut self.dead).iter().collect::<Vec<_>>();
        self.function.erase_blocks(&dead).expect("emptied blocks nothing names");
    }

    fn node(
        &self,
        id: MetadataId,
    ) -> Option<&MetadataNode> {
        match (id.0 as usize).checked_sub(self.metadata.len()) {
            None => self.metadata.get(id.0 as usize),
            Some(made) => self.declared.nodes().get(made),
        }
    }

    /// `getBranchWeightMDNode` with its weights.
    fn branch_weights(
        &self,
        branch: InstId,
    ) -> Option<(MetadataId, Vec<u32>)> {
        let (_, id) = self.function.instruction(branch).metadata.iter().find(|(kind, _)| kind == "prof")?;
        let node = self.node(*id)?;
        let mut operands = node.operands.iter();
        let Some(MetadataOperand::String(name)) = operands.next() else { return None };
        if name != "branch_weights" {
            return None;
        }
        let weights = operands
            .map(|one| match one {
                MetadataOperand::Constant(at) => match self.context.get(*at).kind {
                    ConstantKind::Int(bits) => Some(bits as u32),
                    ConstantKind::Zero => Some(0),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some((*id, weights))
    }

    /// `setBranchWeights`.
    fn set_branch_weights(
        &mut self,
        branch: InstId,
        weights: [u32; 2],
    ) {
        let word = self.context.types.int(32);
        let mut operands = vec![MetadataOperand::String("branch_weights".to_owned())];
        for weight in weights {
            operands.push(MetadataOperand::Constant(self.context.int(word, i128::from(weight))));
        }
        let node = self.declared.node(MetadataNode { distinct: false, operands });
        self.function.set_metadata(branch, "prof", Some(node));
    }
}

/// `isa<ConstantInt>`: the value of an integer constant.
fn constant_int(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        ConstantKind::Zero => Some(0),
        _ => None,
    }
}

/// `updateBranchWeights`: the weights of the preheader's guard and of the loop
/// branch that was cloned from it.
fn update_branch_weights(
    run: &mut Run,
    pre_header_br: InstId,
    loop_br: InstId,
    has_conditional_pre_header: bool,
    succs_swapped: bool,
) {
    let Some((weight_md, weights)) = run.branch_weights(pre_header_br) else { return };
    // The loop branch should be a clone of the guard with the same metadata; a
    // degenerate case where instsimplify changed the instructions is left.
    if run.branch_weights(loop_br).map(|(id, _)| id) != Some(weight_md) {
        return;
    }
    let [mut orig_loop_exit_weight, mut orig_loop_backedge_weight] = weights[..] else { return };
    if succs_swapped {
        std::mem::swap(&mut orig_loop_exit_weight, &mut orig_loop_backedge_weight);
    }

    // x == x0 + x1 (counts to the exit stay the same), y0 == x - x0 == x1
    // (how often the loop was entered at all), y1 == y - y0 (how often it was
    // repeated after the first iteration). How often the loop had a zero trip
    // count cannot be deduced, so x is split by a guess.
    let (exit_weight0, exit_weight1, enter_weight, loop_back_weight);
    if orig_loop_exit_weight > 0 && orig_loop_backedge_weight > 0 {
        let mut x0 = 0;
        if has_conditional_pre_header {
            if orig_loop_backedge_weight >= orig_loop_exit_weight {
                // The loop count is bigger than the exit count: a zero trip
                // count nearly never happens. Scale up the counts to match
                // `ZeroTripCountWeights` for the x0:x1 ratio, without
                // overflow.
                x0 = ZERO_TRIP_COUNT_WEIGHTS[0];
                while orig_loop_exit_weight < ZERO_TRIP_COUNT_WEIGHTS[1] + x0 {
                    let high_bit = 1u32 << 31;
                    if orig_loop_backedge_weight & high_bit != 0 || orig_loop_exit_weight & high_bit != 0 {
                        break;
                    }
                    orig_loop_backedge_weight <<= 1;
                    orig_loop_exit_weight <<= 1;
                }
            } else {
                // A higher exit count than backedge count: only zero-trip and
                // one-trip cases.
                x0 = orig_loop_exit_weight - orig_loop_backedge_weight;
            }
        } else if orig_loop_exit_weight > orig_loop_backedge_weight {
            // The body runs at least once, so the backedge count is not less
            // than the exit count; sampled profiles can disagree.
            orig_loop_backedge_weight = orig_loop_exit_weight;
        }
        debug_assert!(orig_loop_exit_weight >= x0, "Bad branch weight");
        exit_weight0 = x0;
        exit_weight1 = orig_loop_exit_weight - x0;
        enter_weight = exit_weight1;
        debug_assert!(orig_loop_backedge_weight >= enter_weight, "Bad branch weight");
        loop_back_weight = orig_loop_backedge_weight - enter_weight;
    } else if orig_loop_exit_weight == 0 {
        if orig_loop_backedge_weight == 0 {
            // Degenerate: everything stays zero.
            (exit_weight0, exit_weight1, enter_weight, loop_back_weight) = (0, 0, 0, 0);
        } else {
            // An endless loop: the entry is not the same as the exit.
            (exit_weight0, exit_weight1, enter_weight, loop_back_weight) = (0, 0, 1, orig_loop_backedge_weight);
        }
    } else {
        // The loop is never entered.
        debug_assert!(orig_loop_backedge_weight == 0, "remaining case is backedge zero");
        (exit_weight0, exit_weight1, enter_weight, loop_back_weight) = (1, 1, 0, 0);
    }

    let order = |taken: u32, other: u32| if succs_swapped { [other, taken] } else { [taken, other] };
    // `{ SuccsSwapped ? LoopBackWeight : ExitWeight1, SuccsSwapped ?
    // ExitWeight1 : LoopBackWeight }`
    run.set_branch_weights(
        loop_br,
        if succs_swapped { [loop_back_weight, exit_weight1] } else { [exit_weight1, loop_back_weight] },
    );
    if has_conditional_pre_header {
        let _ = order;
        run.set_branch_weights(
            pre_header_br,
            if succs_swapped { [enter_weight, exit_weight0] } else { [exit_weight0, enter_weight] },
        );
    }
}

/// `isSafeToSpeculativelyExecute`, for the instructions `shouldSpeculateInstrs`
/// lets through (an arithmetic or bitwise operation, a cast, a GEP): none
/// traps or reads memory, a division being refused by the opcode switch.
fn is_safe_to_speculatively_execute(
    function: &Function,
    inst: InstId,
) -> bool {
    !matches!(
        function.instruction(inst).opcode,
        Opcode::Binary(BinaryOp::SDiv | BinaryOp::UDiv | BinaryOp::SRem | BinaryOp::URem)
            | Opcode::Load { .. }
            | Opcode::Store { .. }
            | Opcode::Call(_)
            | Opcode::Invoke(_)
            | Opcode::Alloca { .. }
            | Opcode::LandingPad { .. }
    ) && !function.instruction(inst).opcode.is_terminator()
}

/// `shouldSpeculateInstrs`: whether the instructions of `latch` before its
/// jump are safe and cheap to speculate: a single arithmetic instruction along
/// with any type conversions.
fn should_speculate_instrs(
    run: &Run,
    lp: Loop,
    latch: BlockId,
) -> bool {
    let function = &*run.function;
    let jump = function.terminator(latch).expect("a terminated latch");
    let mut seen_increment = false;
    let multi_exit_loop = lp.get_exiting_block(function).is_none();
    for inst in function.iter_block(latch) {
        if inst == jump {
            break;
        }
        if !is_safe_to_speculatively_execute(function, inst) {
            return false;
        }
        let operation = function.instruction(inst);
        let all_constant_indices = |operation: &llrm_mir::module::Instruction| {
            operation.operands[1..].iter().all(|one| matches!(one, Operand::Constant(_)))
        };
        match operation.opcode {
            Opcode::GetElementPtr { .. }
            | Opcode::Binary(
                BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::And
                | BinaryOp::Or
                | BinaryOp::Xor
                | BinaryOp::Shl
                | BinaryOp::LShr
                | BinaryOp::AShr,
            ) => {
                // GEPs are cheap if all indices are constant.
                if matches!(operation.opcode, Opcode::GetElementPtr { .. }) && !all_constant_indices(operation) {
                    return false;
                }
                let is_constant = |operand: Operand| matches!(operand, Operand::Constant(_));
                let iv_operand = if !is_constant(operation.operands[0]) {
                    Some(operation.operands[0])
                } else {
                    operation.operands.get(1).copied().filter(|&one| !is_constant(one))
                };
                let Some(iv_operand) = iv_operand else { return false };
                // If the increment operand is used outside of the loop, the
                // speculation could cause extra live range interference.
                if multi_exit_loop
                    && let Operand::Value(value) = iv_operand
                    && function.users(value).iter().any(|one| !lp.contains_instruction(function, one.user))
                {
                    return false;
                }
                if seen_increment {
                    return false;
                }
                seen_increment = true;
            }
            // Type conversions are ignored.
            Opcode::Cast(CastOp::Trunc | CastOp::ZExt | CastOp::SExt) => {}
            _ => return false,
        }
    }
    true
}

/// `simplifyLoopLatch`: folds the loop tail into the loop exit by speculating
/// the tail instructions, typically a single post-increment.
fn simplify_loop_latch(
    run: &mut Run,
    lp: Loop,
) -> Option<BlockId> {
    let latch = lp.get_loop_latch(run.function)?;
    let jump = run.function.terminator(latch)?;
    let operation = run.function.instruction(jump);
    if operation.opcode != Opcode::Br || operation.operands.len() != 1 {
        return None;
    }
    let predecessors = run.function.predecessors(latch);
    let [last_exit] = predecessors[..] else { return None };
    if !lp.contains(last_exit) || !lp.is_loop_exiting(run.function, last_exit) {
        return None;
    }
    let end = run.function.terminator(last_exit)?;
    if run.function.instruction(end).opcode != Opcode::Br {
        return None;
    }
    if !should_speculate_instrs(run, lp, latch) {
        return None;
    }
    if cfg::merge_block_into_predecessor(run.context, run.function, latch, true) {
        run.dead.insert(latch);
        return Some(last_exit);
    }
    None
}

/// `RewriteUsesOfClonedInstructions`: the header's values now exist twice, the
/// first in the preheader and the next in the old header; the uses outside
/// the header read whichever reaches them.
fn rewrite_uses_of_cloned_instructions(
    run: &mut Run,
    orig_header: BlockId,
    orig_preheader: BlockId,
    value_map: &IdMap<ValueId, Operand>,
) {
    // Remove phi entries that are no longer live.
    let header_instructions = run.function.iter_block(orig_header).collect::<Vec<_>>();
    for &inst in &header_instructions {
        if run.function.instruction(inst).opcode != Opcode::Phi {
            break;
        }
        let kept = arms(run.function, inst).into_iter().filter(|&(_, from)| from != orig_preheader).collect::<Vec<_>>();
        run.function.set_operands(inst, from_arms(&kept));
    }

    // Now fix up users of the instructions in the old header, inserting phis as
    // necessary.
    for &inst in &header_instructions {
        let Some(result) = run.function.instruction(inst).result else { continue };
        let uses = run.function.users(result).iter().copied().collect::<Vec<_>>();
        // If there are no uses of the value there is nothing to rewrite.
        if uses.is_empty() {
            continue;
        }
        let Some(&orig_pre_header_val) = value_map.get(&result) else { continue };
        // The value now exists in two versions: the initial value in the
        // preheader and the loop's next value in the old header.
        let ty = run.function.value(result).ty;
        let mut ssa = SsaUpdater::new(ty, None);
        ssa.add_available_value(orig_header, Operand::Value(result));
        ssa.add_available_value(orig_preheader, orig_pre_header_val);
        for one in uses {
            // The updater can't handle a non-phi use in the same block as an
            // earlier definition; those are handled here.
            if run.function.instruction(one.user).opcode != Opcode::Phi {
                let user_block = run.function.parent(one.user);
                // The original users in the old header already use the
                // original definitions.
                if user_block == Some(orig_header) {
                    continue;
                }
                // Users in the preheader use the value the original definitions
                // are mapped to.
                if user_block == Some(orig_preheader) {
                    run.function.set_operand(one.user, one.index as usize, orig_pre_header_val);
                    continue;
                }
            }
            // Anything else can be handled by the updater.
            ssa.rewrite_use(run.context, run.layout, run.function, one);
        }
    }
}

/// `profitableToRotateLoopExitingLatch`: whether a header phi has no user but
/// in the header's exit, so rotating makes it dead, or (`CheckExitCount`)
/// rotating makes the latch's exit count computable.
fn profitable_to_rotate_loop_exiting_latch(
    run: &Run,
    lp: Loop,
) -> bool {
    let function = &*run.function;
    let header = lp.get_header();
    let branch = function.terminator(header).expect("a terminated header");
    let operands = &function.instruction(branch).operands;
    let (Operand::Block(first), Operand::Block(second)) = (operands[1], operands[2]) else { return false };
    let header_exit = if lp.contains(first) { second } else { first };
    for phi in edges::phis(function, header) {
        let result = function.instruction(phi).result.expect("a phi's value");
        // Look for uses of this phi in the loop or via exits other than the
        // header's.
        if function.users(result).iter().any(|one| function.parent(one.user) != Some(header_exit)) {
            continue;
        }
        return true;
    }
    if !run.options.check_exit_count {
        return false;
    }
    // DEVIATE R7: expressions made for this one question (the pass keeps no
    // ScalarEvolution), one session for the loops until a rotation changes the
    // function; exit 2026-11-20 with a session the driver holds.
    let Some(latch) = lp.get_loop_latch(run.function) else { return false };
    let mut held = run.scev.borrow_mut();
    let session = held.get_or_insert_with(|| {
        llrm_analysis::scev::analysis::ScevSession::within(run.context, run.globals, run.metadata, run.function)
    });
    let (se, env) = session.split(run.layout, run.globals, run.metadata);
    let cx = env.cx(&*run.context, &*run.function);
    use llrm_analysis::scev::backedge::ExitCountKind::Exact;
    let latch_count = se.get_exit_count(&cx, header, latch, Exact);
    let header_count = se.get_exit_count(&cx, header, header, Exact);
    se.is_could_not_compute(latch_count) && !se.is_could_not_compute(header_count)
}

/// `simplifyInstruction(copy, SQ)`: with the dominator tree, which finds the
/// guard a dominating test implies (R5).
fn simplified_copy(
    run: &mut Run,
    copy: InstId,
) -> Option<Operand> {
    let mut held = run.dom.borrow_mut();
    let analyses = held.get_or_insert_with(|| {
        llrm_mir::valuetracking::FunctionAnalyses::of(run.context, run.globals, run.metadata, run.layout, run.function)
    });
    llrm_mir::simplify::simplify_instruction_in(
        run.context,
        run.globals,
        run.layout,
        Some(&*analyses),
        run.function,
        copy,
    )
}

/// `LoopInfo::replacementPreservesLCSSAForm`.
pub(crate) fn replacement_preserves_lcssa_form(
    info: &LoopInfo,
    function: &Function,
    from: InstId,
    to: Operand,
) -> bool {
    // Only a replacing instruction can break LCSSA form.
    let Operand::Value(value) = to else { return true };
    let ValueDef::Instruction(replacement) = function.value(value).def else { return true };
    let (Some(to_block), Some(from_block)) = (function.parent(replacement), function.parent(from)) else {
        return true;
    };
    // Defined in the same block: replacing cannot break LCSSA form.
    if to_block == from_block {
        return true;
    }
    // Not defined in a loop: it can replace anything.
    let Some(to_loop) = info.get_loop_for(to_block) else { return true };
    // Defined in a loop: it can only replace instructions defined in the same
    // loop or in loops nested inside it.
    match info.get_loop_for(from_block) {
        Some(from_loop) => to_loop.contains_loop(Some(from_loop)),
        None => false,
    }
}

/// `createPHIsForSplitLoopExit` for the one predecessor `pred`: a phi in the
/// new exit block `split` for each phi of `dest`, which then reads it.
fn create_phis_for_split_loop_exit(
    run: &mut Run,
    pred: BlockId,
    split: BlockId,
    dest: BlockId,
) {
    for phi in edges::phis(run.function, dest) {
        let incoming = arms(run.function, phi);
        let Some(at) = incoming.iter().position(|&(_, from)| from == split) else { continue };
        let value = incoming[at].0;
        // A phi that already satisfies LCSSA needs no new one.
        if let Operand::Value(one) = value
            && let ValueDef::Instruction(def) = run.function.value(one).def
            && run.function.instruction(def).opcode == Opcode::Phi
            && run.function.parent(def) == Some(split)
        {
            continue;
        }
        let ty = run.function.instruction(phi).ty;
        let new = run
            .function
            .create_instruction(Opcode::Phi, ty, vec![value, Operand::Block(pred)], Flags::default(), Some("split"));
        let terminator = run.function.terminator(split).expect("a terminated split block");
        run.function.insert(new, Position::Before(terminator)).expect("a placed block");
        let result = Operand::Value(run.function.instruction(new).result.expect("a phi's value"));
        let mut updated = incoming;
        updated[at].0 = result;
        run.function.set_operands(phi, from_arms(&updated));
    }
}

/// `SplitCriticalEdge(pred, dest)` with the loop and LCSSA options, for an
/// edge `pred` is a block of the loop `lp` (if any) leaves by.
fn split_critical_edge(
    run: &mut Run,
    info: &LoopInfo,
    pred: BlockId,
    dest: BlockId,
) -> Option<BlockId> {
    let split = edges::split_critical_edge(run.function, pred, dest)?;
    // If `pred` is in a loop and `dest` is outside it, `split` is a new exit
    // block: its LCSSA form is made here.
    if let Some(pred_loop) = info.get_loop_for(pred)
        && !pred_loop.contains(dest)
    {
        create_phis_for_split_loop_exit(run, pred, split, dest);
    }
    Some(split)
}

/// Whether the header's exit count is computable (RS1): a counted loop keeps
/// its rotated form, which the induction variable passes after it read.
fn header_exit_count_known(
    run: &Run,
    lp: Loop,
    header: BlockId,
) -> bool {
    let mut held = run.scev.borrow_mut();
    let session = held.get_or_insert_with(|| {
        llrm_analysis::scev::analysis::ScevSession::within(run.context, run.globals, run.metadata, run.function)
    });
    let (se, env) = session.split(run.layout, run.globals, run.metadata);
    let cx = env.cx(&*run.context, &*run.function);
    let _ = lp;
    use llrm_analysis::scev::backedge::ExitCountKind::Exact;
    let count = se.get_exit_count(&cx, header, header, Exact);
    !se.is_could_not_compute(count)
}

/// Whether the header's exit test is a constant once its phis read the
/// preheader's values: the copy the rotation would put in the preheader
/// (RS1), simplified as `rotate_loop` does and removed again.
fn guard_folds(
    run: &mut Run,
    orig_header: BlockId,
    orig_preheader: BlockId,
    header_end: InstId,
) -> Guard {
    let entry = run.function.terminator(orig_preheader).expect("a terminated preheader");
    let mut map = IdMap::<ValueId, Operand>::new();
    let mut copies: Vec<InstId> = Vec::new();
    let mut remains = false;
    for inst in run.function.iter_block(orig_header).collect::<Vec<_>>() {
        if inst == header_end {
            break;
        }
        let operation = run.function.instruction(inst);
        let Some(result) = operation.result else { continue };
        if operation.opcode == Opcode::Phi {
            if let Some((value, _)) = arms(run.function, inst).into_iter().find(|&(_, from)| from == orig_preheader) {
                map.insert(result, value);
            }
            continue;
        }
        let copy = run.function.clone_instruction(inst);
        run.function.insert(copy, Position::Before(entry)).expect("a placed preheader");
        let operands = run.function.instruction(copy).operands.clone();
        for (index, operand) in operands.into_iter().enumerate() {
            if let Operand::Value(one) = operand
                && let Some(&mapped) = map.get(&one)
            {
                run.function.set_operand(copy, index, mapped);
            }
        }
        match simplified_copy(run, copy) {
            Some(value) => {
                map.insert(result, value);
            }
            None => {
                remains = true;
                if let Some(made) = run.function.instruction(copy).result {
                    map.insert(result, Operand::Value(made));
                }
            }
        }
        copies.push(copy);
    }
    let mut condition = run.function.instruction(header_end).operands[0];
    if let Operand::Value(one) = condition
        && let Some(&mapped) = map.get(&one)
    {
        condition = mapped;
    }
    let folds = constant_int(run.context, condition).is_some();
    for copy in copies.into_iter().rev() {
        run.function.erase(copy).expect("a temporary copy");
    }
    Guard { folds, remains }
}

/// Whether no header phi is wider than the target's widest native integer: rotating keeps a
/// counter and its successor live together at the loop's end, which costs a register
/// pair a word of the counter (z80 -Oz, 32-bit counters: div_constants +535 B).
fn phis_fit_a_register(
    run: &Run,
    orig_header: BlockId,
) -> bool {
    let widest = run.layout.largest_legal_integer();
    run.function.iter_block(orig_header).all(|inst| {
        let operation = run.function.instruction(inst);
        operation.opcode != Opcode::Phi
            || !matches!(run.context.types.get(operation.ty), llrm_mir::types::Type::Int(width) if *width > widest)
    })
}

/// What [`guard_folds`] found.
struct Guard {
    /// The exit test is a constant in the preheader.
    folds: bool,
    /// Some copied header instruction does not simplify away: the copy leaves code.
    remains: bool,
}

impl Guard {
    /// The whole copied header folds away.
    fn free(&self) -> bool {
        self.folds && !self.remains
    }
}

/// `rotateLoop`: whether the loop was rotated. `simplified_latch`: the latch
/// was just folded into the final loop exit; the new latch is an exiting
/// branch, and the rotation would have happened had it not been folded.
fn rotate_loop(
    run: &mut Run,
    info: &LoopInfo,
    lp: Loop,
    simplified_latch: bool,
) -> Option<BlockId> {
    // If the loop has only one block there is not much to rotate: a block
    // merged away by `simplify_loop_latch` is no longer one of them.
    if lp.get_blocks().iter().filter(|&&block| !run.dead.contains(&block)).count() == 1 {
        return None;
    }
    let orig_header = lp.get_header();
    let Some(orig_latch) = lp.get_loop_latch(run.function) else { return None };
    // If the loop header does not end in a conditional branch, the loop is not
    // suitable for rotation.
    let Some(header_end) = run.function.terminator(orig_header) else { return None };
    {
        let end = run.function.instruction(header_end);
        if end.opcode != Opcode::Br || end.operands.len() != 3 {
            return None;
        }
    }
    // If the header is not one of the loop's exiting blocks, either the loop is
    // already rotated or it is not suitable.
    if !lp.is_loop_exiting(run.function, orig_header) {
        return None;
    }
    // Rotate if the latch was just simplified, or if it is profitable.
    if lp.is_loop_exiting(run.function, orig_latch)
        && !simplified_latch
        && !run.options.is_util_mode
        && !profitable_to_rotate_loop_exiting_latch(run, lp)
    {
        return None;
    }

    // Check the size of the original header and reject the loop if it is very
    // big or we can't duplicate blocks inside it.
    let mut too_big = false;
    {
        let function = &*run.function;
        let assumes = assume_calls(run, lp);
        let effects_free = |inst: InstId| llrm_mir::memory::only_value(run.context, run.callees, function, inst);
        let eph = collect_ephemeral_values(function, &effects_free, &assumes, &|block| lp.contains(block));
        let query = Query {
            tti: run.tti,
            context: run.context,
            layout: run.layout,
            globals: run.globals,
            this: run.this,
            has_one_live_use: run.has_one_live_use,
        };
        let mut metrics = CodeMetrics::default();
        metrics.analyze_basic_block(&query, function, orig_header, &eph, run.options.prepare_for_lto);
        if metrics.not_duplicatable || metrics.convergence != Convergence::None || !metrics.num_insts.valid {
            return None;
        }
        too_big = metrics.num_insts.value > i64::from(run.options.max_header_size);
        if too_big && !run.options.free_guard {
            return None;
        }
        if run.options.priced_at_size {
            let size = |inst: InstId| {
                llrm_mir::instructioncost::instruction_cost(
                    run.tti,
                    run.context,
                    run.layout,
                    run.globals,
                    function,
                    inst,
                    llrm_mir::tti::CostKind::CodeSize,
                )
                .value
            };
            let latch_end = function.terminator(orig_latch).expect("a latch ends in a terminator");
            if metrics.num_insts.value > size(header_end) + size(latch_end) {
                return None;
            }
        }
        // When preparing for LTO, avoid rotating loops with calls that could be
        // inlined during the LTO stage.
        if run.options.prepare_for_lto && metrics.num_inline_candidates > 0 {
            return None;
        }
    }

    // Now this loop is suitable for rotation.
    let Some(orig_preheader) = lp.get_loop_preheader(run.function) else { return None };
    // If the loop could not be converted to canonical form, it must have an
    // indirectbr in it: give up.
    if !lp.has_dedicated_exits(run.function) {
        return None;
    }
    if too_big
        && !(guard_folds(run, orig_header, orig_preheader, header_end).free() && phis_fit_a_register(run, orig_header))
    {
        return None;
    }
    if run.options.guard_must_fold && !guard_folds(run, orig_header, orig_preheader, header_end).folds {
        // A big function is rotated in both trial compiles.
        let priced =
            run.options.rotation_trial && run.function.instruction_count() <= trial::ROTATION_TRIAL_MAX_INSTRUCTIONS;
        let this = run.this.and_then(|this| run.globals[this.0 as usize].name.as_deref());
        if !header_exit_count_known(run, lp, orig_header) {
            // Uncounted: left, unless the ROTATION-HOT compile rotates it.
            if !(priced && trial::rotates_more()) {
                if let (true, Some(name)) = (priced, this) {
                    trial::note_left(name);
                }
                return None;
            }
        } else if priced {
            // Counted: rotated, unless the ROTATION-TRIAL compile leaves it.
            if trial::unrotated() {
                return None;
            }
            if let Some(name) = this {
                if llrm_support::debug::enabled("trial") {
                    eprintln!("trial noted {name}: {} instructions", run.function.instruction_count());
                }
                trial::note_rotated(name);
            }
        }
    }

    // Find the new loop header: the header's one successor inside the loop. The
    // header's other successor is outside the loop.
    let (mut exit, mut new_header) = {
        let operands = &run.function.instruction(header_end).operands;
        let (Operand::Block(first), Operand::Block(second)) = (operands[1], operands[2]) else { return None };
        (first, second)
    };
    let bi_succs_swapped = lp.contains(exit);
    if bi_succs_swapped {
        std::mem::swap(&mut exit, &mut new_header);
    }
    debug_assert!(lp.contains(new_header) && !lp.contains(exit), "Unable to determine loop header and exit blocks");

    // This code assumes that the new header has exactly one predecessor. Remove
    // any single-entry phis in it.
    debug_assert!(run.function.predecessors(new_header)[..] == [orig_header], "New header doesn't have one pred!");
    local::fold_single_entry_phi_nodes(run.context, run.function, new_header);

    // Begin by walking the old header and populating the value map with an
    // entry for each instruction. For phis, the value available in the old
    // preheader is just the incoming value from the old preheader.
    let mut value_map = IdMap::<ValueId, Operand>::new();
    let header_instructions = run.function.iter_block(orig_header).collect::<Vec<_>>();
    let mut rest = 0;
    for &inst in &header_instructions {
        if run.function.instruction(inst).opcode != Opcode::Phi {
            break;
        }
        let value = arms(run.function, inst)
            .into_iter()
            .find(|&(_, from)| from == orig_preheader)
            .map(|(value, _)| value)
            .expect("a header phi with an input from the preheader");
        value_map.insert(run.function.instruction(inst).result.expect("a phi's value"), value);
        rest += 1;
    }

    // For the rest of the instructions, either hoist to the old preheader if
    // possible or create a clone in the old preheader if not.
    let loop_entry_branch = run.function.terminator(orig_preheader).expect("a terminated preheader");
    // The local scope declarations of the header: after the rotation they are
    // duplicated and their scopes cloned (LoopRotationUtils.cpp:500).
    let declarations: Vec<InstId> = header_instructions
        .iter()
        .copied()
        .filter(|&inst| crate::noaliasscopes::declares(run.function, inst))
        .collect();
    let mut cloned_declarations: Vec<(InstId, InstId)> = Vec::new();
    for &inst in &header_instructions[rest..] {
        let operation = run.function.instruction(inst);
        // If the instruction's operands are invariant and it doesn't read or
        // write memory, then it is safe to hoist. Doing this doesn't change the
        // order of execution in the preheader, but does prevent the instruction
        // from executing in each iteration of the loop. This means it is safe
        // to hoist something that might trap, but isn't safe to hoist
        // something that reads memory (without proving that the loop
        // doesn't write).
        let invariant = operation.operands.iter().all(|&one| lp.is_loop_invariant(run.function, one));
        let effects = llrm_mir::memory::of(run.context, run.callees, run.function, inst);
        if invariant
            && !effects.reads
            && !effects.writes
            && !operation.opcode.is_terminator()
            && !matches!(operation.opcode, Opcode::Alloca { .. })
        {
            run.function.move_to(inst, Position::Before(loop_entry_branch)).expect("a placed preheader");
            continue;
        }

        // Otherwise, create a duplicate of the instruction.
        let copy = run.function.clone_instruction(inst);
        run.function.insert(copy, Position::Before(loop_entry_branch)).expect("a placed preheader");

        // Eagerly remap the operands of the instruction.
        let operands = run.function.instruction(copy).operands.clone();
        for (index, operand) in operands.into_iter().enumerate() {
            if let Operand::Value(one) = operand
                && let Some(&mapped) = value_map.get(&one)
            {
                run.function.set_operand(copy, index, mapped);
            }
        }

        // With the operands remapped, see if the instruction constant folds or
        // is otherwise simplifiable. This commonly occurs because the entry
        // from phis allows compares and other instructions to fold.
        let simplified = simplified_copy(run, copy);
        let original = run.function.instruction(inst).result;
        match simplified {
            Some(value) if replacement_preserves_lcssa_form(info, run.function, copy, value) => {
                // Delete the temporary instruction and put the folded value in
                // the map; keep it if it has side effects.
                if let Some(original) = original {
                    value_map.insert(original, value);
                }
                let effects = llrm_mir::memory::of(run.context, run.callees, run.function, copy);
                let side_effects =
                    effects.writes || !llrm_mir::memory::only_value(run.context, run.callees, run.function, copy);
                if !side_effects {
                    if let Some(result) = run.function.instruction(copy).result {
                        run.function.replace_value(result, value);
                    }
                    run.function.erase(copy).expect("a replaced copy");
                }
            }
            _ => {
                if let (Some(original), Some(result)) = (original, run.function.instruction(copy).result) {
                    value_map.insert(original, Operand::Value(result));
                }
                if declarations.contains(&inst) {
                    cloned_declarations.push((inst, copy));
                }
            }
        }
    }

    // `ValueMap[*NoAliasDeclInstructions.begin()]` and what follows
    // (LoopRotationUtils.cpp:625): the declarations again at the top of the new
    // header, then the scopes of the header and of the preheader's copy cloned.
    if let Some(&(_, first_copy)) = cloned_declarations.first() {
        let scopes = crate::noaliasscopes::declared_scopes(run.function, declarations.iter().copied());
        let at = run
            .function
            .iter_block(new_header)
            .find(|&inst| run.function.instruction(inst).opcode != Opcode::Phi)
            .expect("a terminator");
        for &declaration in &declarations {
            let again = run.function.clone_instruction(declaration);
            run.function.insert(again, Position::Before(at)).expect("a placed new header");
        }
        let header_now = run.function.iter_block(orig_header).collect::<Vec<_>>();
        crate::noaliasscopes::cloned_and_adapted(
            run.function,
            &scopes,
            &header_now,
            "h.rot",
            run.metadata,
            run.declared,
        );
        let tail: Vec<InstId> =
            run.function.iter_block(orig_preheader).skip_while(|&inst| inst != first_copy).collect();
        crate::noaliasscopes::cloned_and_adapted(
            run.function,
            &scopes,
            &tail,
            "pre.rot",
            run.metadata,
            run.declared,
        );
    }

    // Along with all the other instructions, the header's terminator was cloned
    // into the preheader. Fix up the phis in each of the header's successors by
    // duplicating their incoming values for the header.
    for successor in run.function.successors(orig_header) {
        for phi in edges::phis(run.function, successor) {
            let mut incoming = arms(run.function, phi);
            let value = incoming.iter().find(|&&(_, from)| from == orig_header).map(|&(value, _)| value);
            if let Some(value) = value {
                incoming.push((value, orig_preheader));
                run.function.set_operands(phi, from_arms(&incoming));
            }
        }
    }

    // Now that the preheader has a clone of the header's terminator, remove the
    // preheader's old terminator (the original branch into the loop), the
    // debug records before it kept before the new one
    // (`flushTerminatorDbgRecords`).
    let preheader_end = run.function.iter_block(orig_preheader).collect::<Vec<_>>();
    if let [.., new_terminator, _] = preheader_end[..] {
        run.function.move_records_before(loop_entry_branch, new_terminator);
    }
    run.function.erase(loop_entry_branch).expect("a jump defines nothing");

    // If there were any uses of instructions in the duplicated block outside
    // the loop, update them, inserting phis as required.
    rewrite_uses_of_cloned_instructions(run, orig_header, orig_preheader, &value_map);

    // At this point the major CFG changes are finished. The duplicated branch
    // may now branch on a constant; if that means the loop is entered, the
    // conditional branch is folded to an unconditional one. This simplifies
    // the loop in cases important for nested loops, and means fewer edges are
    // split.
    let guard = run.function.terminator(orig_preheader).expect("a terminated preheader");
    let condition = run.function.instruction(guard).operands[0];
    let has_conditional_pre_header = match constant_int(run.context, condition) {
        None => true,
        Some(bits) => {
            // `getSuccessor(isZero)`.
            let taken = run.function.instruction(guard).operands[1 + usize::from(bits == 0)];
            taken != Operand::Block(new_header)
        }
    };

    update_branch_weights(run, guard, header_end, has_conditional_pre_header, bi_succs_swapped);

    if has_conditional_pre_header {
        // The conditional branch can't be folded, handle the general case.
        // Split edges as necessary to preserve loop-simplify form.
        //
        // The preheader has two successors now, the new header and the exit,
        // and is not a preheader anymore: split the edge to form a real one.
        split_critical_edge(run, info, orig_preheader, new_header);

        // Preserve canonical loop form, which means that the exit should have
        // only one predecessor. The exit could be an exit block for multiple
        // nested loops, causing both of the edges to be critical and to need
        // splitting.
        let exit_preds = run.function.predecessors(exit);
        for exit_pred in exit_preds {
            // Only loop exit edges need splitting.
            let Some(pred_loop) = info.get_loop_for(exit_pred) else { continue };
            if pred_loop.contains(exit) {
                continue;
            }
            if let Some(exit_split) = split_critical_edge(run, info, exit_pred, exit) {
                // `ExitSplit->moveBefore(Exit)` (LoopRotationUtils.cpp:788).
                run.function.move_block_before(exit_split, exit).expect("placed blocks");
            }
        }
    } else {
        // The conditional branch can be folded, which makes things simpler.
        // The first step is to remove the extra edge to the exit block.
        run.function.predecessor_removed(&*run.context, exit, orig_preheader, true);
        let void = run.function.instruction(guard).ty;
        let jump =
            run.function.create_instruction(Opcode::Br, void, vec![Operand::Block(new_header)], Flags::default(), None);
        run.function.insert(jump, Position::End(orig_preheader)).expect("a placed preheader");
        run.function.move_records_before(guard, jump);
        run.function.erase(guard).expect("a branch defines nothing");
    }

    // Now that the CFG is consistent again, try to merge the old header into
    // the old latch. This succeeds if they are connected by an
    // unconditional branch. This is just a cleanup so the emitted code
    // isn't too gross in this common case.
    let latch = if cfg::merge_block_into_predecessor(run.context, run.function, orig_header, false) {
        run.dead.insert(orig_header);
        orig_latch
    } else {
        orig_header
    };
    Some(latch)
}

/// The `llvm.assume` calls of the loop (`AssumptionCache::assumptions` less
/// those outside it, R1).
fn assume_calls(
    run: &Run,
    lp: Loop,
) -> Vec<InstId> {
    let function = &*run.function;
    lp.get_blocks()
        .iter()
        .flat_map(|&block| function.iter_block(block))
        .filter(|&inst| {
            llrm_mir::memory::callee(run.context, function, inst)
                // A callee made after the pass manager's snapshot of the
                // globals is not the `llvm.assume` declared with the module.
                .and_then(|callee| run.globals.get(callee.0 as usize))
                .is_some_and(|callee| callee.name.as_deref() == Some("llvm.assume"))
        })
        .collect()
}

/// `LoopRotate::processLoop`: whether the loop changed.
pub fn process_loop(
    run: &mut Run,
    info: &LoopInfo,
    lp: Loop,
) -> bool {
    // Save the loop metadata.
    let loop_md = lp
        .get_loop_latch(run.function)
        .and_then(|latch| run.function.terminator(latch))
        .and_then(|term| {
            run.function.instruction(term).metadata.iter().find(|(kind, _)| kind == "llvm.loop").map(|(_, id)| *id)
        })
        // `Loop::getLoopID`: a loop ID is a node whose first operand is itself.
        .filter(|&id| {
            run.node(id).is_some_and(|node| {
                matches!(
                    node.operands.first(),
                    Some(MetadataOperand::Node(first)) if *first == id
                )
            })
        });

    // Simplify the loop latch before attempting to rotate the header upward:
    // rotation may not be needed if the loop tail can be folded into the loop
    // exit.
    let simplified_latch = if run.options.rotation_only { None } else { simplify_loop_latch(run, lp) };

    let rotated_latch = rotate_loop(run, info, lp, simplified_latch.is_some());

    // Restore the loop metadata onto the new latch's branch.
    if let Some(latch) = rotated_latch.or(simplified_latch)
        && let Some(md) = loop_md
        && let Some(term) = run.function.terminator(latch)
    {
        run.function.set_metadata(term, "llvm.loop", Some(md));
    }
    let changed = rotated_latch.is_some() || simplified_latch.is_some();
    if changed {
        run.scev.take();
        run.dom.take();
    }
    changed
}

/// `LoopRotatePass` over a function: every loop, each after the loops it holds,
/// as the loop pass manager visits them.
///
/// `LoopInfo` is made once. A rotation adds blocks to the loops outside the
/// rotated one and the driver derives it again before it reaches one of them;
/// any other loop is read from the first.
pub fn rotate_loops(run: &mut Run) -> bool {
    let analyze = |run: &Run| LoopInfo::analyze(run.function, || llrm_mir::domtree::DomTree::of(run.function));
    let mut info = analyze(run);
    // The loop pass manager's worklist is popped from the back: a loop's
    // children before it, siblings in program order.
    let mut headers: Vec<BlockId> =
        info.get_loops_in_reverse_sibling_preorder().iter().rev().map(|one| one.get_header()).collect();
    crate::loopfilter::retain(&mut headers);
    let mut stale = IdSet::<BlockId>::new();
    let mut changed = false;
    for header in headers {
        if stale.contains(&header) {
            run.finish();
            info = analyze(run);
            stale.clear();
        }
        if !info.is_loop_header(header) {
            continue;
        }
        let lp = info.get_loop_for(header).expect("a loop headed by this block");
        if process_loop(run, &info, lp) {
            changed = true;
            let mut outer = lp.get_parent_loop();
            while let Some(one) = outer {
                stale.insert(one.get_header());
                outer = one.get_parent_loop();
            }
        }
    }
    run.finish();
    changed
}

/// The header instructions a loop may copy: none without header duplication,
/// else the pin's 16, or the target's own limit for a size build
/// (`Machine::size_rotation_header_limit`; the pin lowers it for `minsize`
/// only).
pub fn header_limit(
    duplicate_headers: bool,
    size: bool,
    target: &dyn llrm_mir::target::Machine,
    tti: &dyn llrm_mir::tti::Tti,
) -> u32 {
    if !duplicate_headers {
        0
    } else if size {
        target.size_rotation_header_limit().or_else(|| tti.size_rotation_header_limit()).unwrap_or(16)
    } else {
        16
    }
}

/// `LoopRotatePass`: every loop of the function rotated with header
/// duplication on (`EnableHeaderDuplication`), whose limit is the pass's
/// default of 16 header instructions (the target's at -Os), or none where
/// the function is optimized for size or `-fno-tree-ch` says so.
pub struct LoopRotate {
    pub duplicate_headers: bool,
    /// Code bytes outrank speed (`-Os`): the header is copied only when small.
    pub size: bool,
    /// `CheckExitCount`.
    pub check_exit_count: bool,
    /// The slot that follows the other loop passes (the unroll refuses a rotated loop, LSR
    /// picks worse counters for one: switch_empty_cases -Oz 45 B became 76 B rotated before
    /// it): at -Oz only, the loops whose copied header folds away (`Options::free_guard`).
    pub free_guard: bool,
}

impl FunctionPass for LoopRotate {
    fn name(&self) -> &'static str {
        "looprotate"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("loop-rotate")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let tti = target_tti(outer.target());
        let options = Options {
            max_header_size: header_limit(self.duplicate_headers, self.size, outer.target(), &*tti),
            priced_at_size: self.size && tti.rotation_header_priced_at_size(),
            guard_must_fold: self.size && tti.rotation_guard_must_fold_at_size(),
            rotation_trial: self.size && tti.rotation_guard_must_fold_at_size() && tti.rotation_trial_at_size(),
            free_guard: self.free_guard && self.size && !self.duplicate_headers && tti.rotation_free_guard_at_minsize(),
            rotation_only: false,
            is_util_mode: false,
            prepare_for_lto: false,
            check_exit_count: self.check_exit_count,
        };
        let mut run = Run {
            context: unit.context,
            layout: unit.layout,
            globals: &outer.globals,
            callees: outer.callees(),
            this: unit.id,
            tti: &*tti,
            function: unit.function,
            metadata: unit.metadata,
            declared: unit.declared,
            options,
            has_one_live_use: &|_| false,
            dead: IdSet::new(),
            scev: Default::default(),
            dom: Default::default(),
        };
        if rotate_loops(&mut run) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// The cost model the pass prices headers with: the target's, or the generic
/// profile where it has none (R6).
pub fn target_tti(target: &dyn llrm_mir::target::Machine) -> std::rc::Rc<dyn Tti> {
    target.tti().unwrap_or_else(|| std::rc::Rc::new(llrm_mir::tti::Basic))
}

/// `rotate_loops` over the function `name` of `module`; whether any loop
/// rotated.
pub fn rotate_function(
    module: &mut llrm_mir::module::Module,
    name: &str,
    tti: &dyn Tti,
    options: Options,
) -> Result<bool, String> {
    rotate_function_knowing(module, name, tti, options, module.globals.len())
}

/// `rotate_function` with the globals the pass manager's snapshot holds: the
/// first `known`. A global made after the snapshot is a callee it cannot name.
pub fn rotate_function_knowing(
    module: &mut llrm_mir::module::Module,
    name: &str,
    tti: &dyn Tti,
    options: Options,
    known: usize,
) -> Result<bool, String> {
    let layout =
        DataLayout::parse(module.datalayout.as_deref().unwrap_or("e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"))?;
    let callees = llrm_mir::memory::callees(module);
    let mut declared = Declared::of(module);
    let mut globals = module.globals.clone();
    globals.truncate(known);
    let metadata = module.metadata.clone();
    let id = module
        .functions()
        .find(|(_, global, _)| global.name.as_deref() == Some(name))
        .map(|(id, _, _)| id)
        .ok_or_else(|| format!("no @{name}"))?;
    let changed = {
        let context = &mut module.context;
        let llrm_mir::module::GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else {
            return Err(format!("@{name} is not a function"));
        };
        let mut run = Run {
            context,
            layout: &layout,
            globals: &globals,
            callees: &callees,
            this: Some(id),
            tti,
            function,
            metadata: &metadata,
            declared: &mut declared,
            options,
            has_one_live_use: &|_| false,
            dead: IdSet::new(),
            scev: Default::default(),
            dom: Default::default(),
        };
        rotate_loops(&mut run)
    };
    declared.place(module)?;
    Ok(changed)
}

#[cfg(test)]
#[path = "looprotate_tests.rs"]
mod tests;
