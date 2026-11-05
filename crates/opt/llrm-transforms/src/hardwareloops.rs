//! A counted loop whose target counts in hardware: LLVM's `HardwareLoops`
//! (`hardware-loops`; HardwareLoops.cpp and `HardwareLoopInfo` at
//! `d1106deb71cc`).
//!
//! # How it works
//! The loops are visited innermost first. For a loop of the target's choice
//! ([`Tti::is_hardware_loop_profitable`], which also sets the counter's width
//! and the decrement) that has a preheader and no irreducible control flow,
//! an exiting block is looked for that every trip passes through, ends in a
//! conditional branch, and has a loop-invariant, non-zero exit count that fits
//! the counter (`isHardwareLoopCandidate`). The trip count `exit count + 1` is
//! written in the counter's type before the preheader's branch and handed to
//! `llvm.set.loop.iterations`; the exit branch's condition becomes
//! `llvm.loop.decrement(decrement)`, true where the loop goes on. The old
//! condition, and the induction variable only it read, are deleted.
//!
//! A loop inside another stops the search at the parent when a conversion was
//! made and the target does not allow nesting.
//!
//! # Example
//! `for (i = 0; i != 10; i++) body` has exit count 9: `set.loop.iterations(10)`
//! in the preheader, and `br i1 (loop.decrement(1)), loop, exit` where
//! `br i1 (icmp ne i.next, 10), loop, exit` was.
//!
//! # Runs
//! In the strength slot, after `lsr`, only for a target whose
//! [`Tti::is_hardware_loop_profitable`] says yes: none does yet, so the pass
//! changes nothing anywhere until a target lowers the intrinsics.
//!
//! # Limits
//! Only the form without a phi counter and without an entry test is made
//! (`llvm.loop.decrement`, `llvm.set.loop.iterations`).
//!
//! DEVIATE 1: `isHardwareLoopCandidate` rejects an exit count whose type is
//! wider than the counter. Here the count is taken when its unsigned range
//! fits the counter and is cut to it: an `int` loop of 54 trips has an `i16`
//! exit count, and a one-byte counter holds it. The trip count wraps to zero
//! at the counter's largest value plus one, which is the full count of a
//! counter that runs from zero. Exit: none; a target that wants the pin's
//! test sets a counter as wide as its types.
//!
//! DEVIATE 2: swapping the exit branch's successors drops its `prof` branch
//! weights; the pin swaps them. Exit: swap the weights.
//!
//! DEVIATE 3: the target's hook sees no loop (the pin passes the loop, SCEV,
//! the assumption cache and library info), and the pass asks it once, not per
//! loop. Exit: the first target whose answer depends on the loop.
//!
//! DEVIATE 4: `noduplicate`, which `llvm.set.loop.iterations`,
//! `llvm.test.set.loop.iterations` and `llvm.loop.decrement` carry, is not an
//! attribute MIR holds. Nothing after this pass copies a loop or a call
//! (the strength slot is followed by the code generator). Exit: MIR's
//! `noduplicate`.
//!
//! DEVIATE 5: a loop is eligible only in LoopSimplify form (the pin inserts a
//! preheader where there is none). Exit: `InsertPreheaderForLoop`.
//!
//! # See also
//! `HardwareLoopsImpl::TryConvertLoop`, `HardwareLoop::Create`,
//! `HardwareLoopInfo::isHardwareLoopCandidate`.

use std::rc::Rc;

use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::analysis::{ScevSession, Scevs};
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expander::Expander;
use llrm_analysis::scev::expr::{Kind, Scev, Ty};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{CallInfo, Flags, Opcode, Tail};
use llrm_mir::passes::{self, Analyses, Declared, FunctionPass, Outer, PreservedAnalyses};
use llrm_mir::tti::{HardwareLoopTarget, Tti};
use llrm_mir::types::Type;

use crate::lftr::branch_of;
use crate::loopdeletion::{contains_irreducible_cfg, loop_blocks_rpo};

/// `HardwareLoopsPass`.
pub struct HardwareLoops;

impl FunctionPass for HardwareLoops {
    fn name(&self) -> &'static str {
        "hardwareloops"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("hardware-loops")
    }

    /// False: the calls it makes are the three counter intrinsics, which touch
    /// no memory a function could name.
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = Rc::clone(analyses.outer());
        let Some(tti) = outer.target().tti() else { return PreservedAnalyses::all() };
        // DEVIATE 3: asked once, for a target that wants no loop converted
        // costs nothing here.
        if !tti.is_hardware_loop_profitable(&mut HardwareLoopTarget::default()) {
            return PreservedAnalyses::all();
        }
        let mut session = ScevSession::of(unit.context, unit.function, &outer);
        let held = analyses.get::<Scevs>(unit.context, unit.layout, unit.function);
        session.adopt(held.take());
        let changed =
            converted_in(&mut session, unit.context, unit.layout, unit.function, unit.declared, &outer, &*tti);
        held.put(session.into_scev());
        if changed {
            // The blocks and edges are as they were.
            PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// The loops of `function` converted, innermost first; whether any was.
pub fn converted(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    tti: &dyn Tti,
) -> bool {
    let mut session = ScevSession::of(context, function, outer);
    converted_in(&mut session, context, layout, function, declared, outer, tti)
}

/// [`converted`] over `session`, whose expressions are valid for `function`.
pub fn converted_in(
    session: &mut ScevSession,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    tti: &dyn Tti,
) -> bool {
    let forest: Vec<Tree> = {
        let (_, env) = session.split(layout, &outer.globals, &outer.metadata);
        env.loops.top_level_loops().map(|top| Tree::of(&top)).collect()
    };
    let mut run = Run {
        session,
        context,
        layout,
        function,
        declared,
        outer,
        tti,
        rewriter: Expander::new("loopcnt"),
        made_change: false,
    };
    for top in &forest {
        run.try_convert_loop_tree(top);
    }
    run.made_change
}

/// A loop and the loops directly inside it, by header.
struct Tree {
    header: BlockId,
    inner: Vec<Tree>,
}

impl Tree {
    fn of(lp: &llrm_mir::loopinfo::Loop<'_, BlockId>) -> Tree {
        Tree { header: lp.get_header(), inner: lp.get_sub_loops().map(|sub| Tree::of(&sub)).collect() }
    }
}

/// `HardwareLoopInfo`: the target's part, and what the candidate search finds
/// (`ExitBlock` is the branch's block: nothing reads it apart).
struct Info {
    target: HardwareLoopTarget,
    /// `ExitBranch`.
    exit_branch: InstId,
    /// `ExitCount`.
    exit_count: Scev,
}

/// `HardwareLoopsImpl`.
struct Run<'a> {
    session: &'a mut ScevSession,
    context: &'a mut Context,
    layout: &'a DataLayout,
    function: &'a mut Function,
    declared: &'a mut Declared,
    outer: &'a Outer,
    tti: &'a dyn Tti,
    rewriter: Expander,
    /// `MadeChange`.
    made_change: bool,
}

impl Run<'_> {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/HardwareLoops.cpp#L286
    /// `TryConvertLoop(Loop *L, LLVMContext &Ctx)`: whether the search should
    /// stop, which is when a conversion was made and the target does not allow
    /// a hardware loop inside another.
    fn try_convert_loop_tree(
        &mut self,
        node: &Tree,
    ) -> bool {
        // Process nested loops first.
        let mut any_changed = false;
        for inner in &node.inner {
            any_changed |= self.try_convert_loop_tree(inner);
        }
        if any_changed {
            return true; // Stop search.
        }
        let mut target = HardwareLoopTarget::default();
        if !self.tti.is_hardware_loop_profitable(&mut target) {
            return false;
        }
        self.made_change |= self.try_convert_loop(node.header, target);
        self.made_change && !target.is_nesting_legal
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/HardwareLoops.cpp#L329
    /// `TryConvertLoop(HardwareLoopInfo &)`.
    fn try_convert_loop(
        &mut self,
        header: BlockId,
        target: HardwareLoopTarget,
    ) -> bool {
        let Some(info) = self.is_hardware_loop_candidate(header, target) else { return false };
        self.create(header, info)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/TargetTransformInfo.cpp#L126
    /// `HardwareLoopInfo::isHardwareLoopCandidate` (and `canAnalyze`, and the
    /// preheader and simplify form the pass needs: DEVIATE 5).
    fn is_hardware_loop_candidate(
        &mut self,
        header: BlockId,
        target: HardwareLoopTarget,
    ) -> Option<Info> {
        let (se, env) = self.session.split(self.layout, &self.outer.globals, &self.outer.metadata);
        let cx = env.cx(&*self.context, &*self.function);
        let lp = cx.loops.get_loop_for(header)?;
        if !lp.is_loop_simplify_form(self.function) {
            return None;
        }
        // `canAnalyze`: a loop with irreducible control flow is not converted.
        let rpo = loop_blocks_rpo(self.function, lp.clone());
        if contains_irreducible_cfg(self.function, env.loops, &rpo) {
            return None;
        }
        if contains_hardware_loop_intrinsic(&cx, &lp) {
            return None;
        }
        let exiting = lp.get_exiting_blocks(self.function);
        for bb in exiting {
            // If we pass the updated counter back through a phi, we need to
            // know which latch the updated value will be coming from.
            if !lp.is_loop_latch(self.function, bb) && target.counter_in_reg {
                continue;
            }
            let ec = se.get_exit_count(&cx, header, bb, ExitCountKind::Exact);
            if se.is_could_not_compute(ec) {
                continue;
            }
            if let Kind::Constant(value) = se.node(ec).kind {
                if value.is_zero() {
                    continue;
                }
            } else if !se.is_loop_invariant(&cx, ec, Some(header)) {
                continue;
            }
            // DEVIATE 1: the count fits the counter; the pin asks the same of
            // its type.
            if !fits(se, &cx, ec, target.count_bits) {
                continue;
            }
            // If this exiting block is contained in a nested loop, it is not
            // eligible for insertion of the branch-and-decrement since the
            // inner loop would end up messing up the value in the CTR.
            if !target.is_nesting_legal && cx.loops.get_loop_for(bb).map(|inner| inner.get_header()) != Some(header) {
                continue;
            }
            // We now have a loop-invariant count of loop iterations (which is
            // not the constant zero) for which we know that this loop will not
            // exit via this existing block.
            //
            // We need to make sure that this block will run on every loop
            // iteration. For this to be true, we must dominate all blocks with
            // backedges. Such blocks are in-loop predecessors to the header
            // block.
            let not_always = self
                .function
                .predecessors(header)
                .into_iter()
                .filter(|&pred| lp.contains(pred))
                .any(|pred| !env.tree.dominates(bb, pred));
            if not_always {
                continue;
            }
            // Make sure this blocks ends with a conditional branch. Note that
            // this block may not be the loop latch block, even if the loop has
            // a latch block.
            let Some((exit_branch, _)) = branch_of(self.function, bb) else { continue };
            return Some(Info { target, exit_branch, exit_count: ec });
        }
        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/HardwareLoops.cpp#L360
    /// `HardwareLoop::Create` without a phi counter or an entry test.
    fn create(
        &mut self,
        header: BlockId,
        info: Info,
    ) -> bool {
        let Some((preheader, count)) = self.init_loop_count(header, &info) else { return false };
        // InsertIterationSetup
        let count_ty = self.context.types.int(info.target.count_bits);
        let void = self.context.types.void();
        let terminator = self.function.terminator(preheader).expect("a terminator");
        self.insert_intrinsic_call(terminator, "llvm.set.loop.iterations", count_ty, void, &[count]);
        // InsertLoopDec
        let decrement = Operand::Constant(
            self.context
                .constant(Constant { ty: count_ty, kind: ConstantKind::Int(u128::from(info.target.loop_decrement)) }),
        );
        let i1 = self.context.types.int(1);
        let new_cond = self.insert_intrinsic_call(info.exit_branch, "llvm.loop.decrement", count_ty, i1, &[decrement]);
        let old_cond = self.function.instruction(info.exit_branch).operands[0];
        self.function.set_operand(info.exit_branch, 0, new_cond);
        // The false branch must exit the loop.
        let taken = self.taken(info.exit_branch);
        let inside = {
            let (_, env) = self.session.split(self.layout, &self.outer.globals, &self.outer.metadata);
            env.loops.get_loop_for(header).expect("a loop").contains(taken)
        };
        if !inside {
            let operands = &self.function.instruction(info.exit_branch).operands;
            let (taken, other) = (operands[1], operands[2]);
            self.function.set_operand(info.exit_branch, 1, other);
            self.function.set_operand(info.exit_branch, 2, taken);
            // DEVIATE 2.
            self.function.set_metadata(info.exit_branch, "prof", None);
        }
        // The old condition may be dead now, and may have even created a dead
        // PHI (the original induction variable).
        self.delete_trivially_dead(old_cond);
        // Run through the basic blocks of the loop and see if any of them have
        // dead PHIs that can be removed.
        let blocks: Vec<BlockId> = {
            let (_, env) = self.session.split(self.layout, &self.outer.globals, &self.outer.metadata);
            env.loops.get_loop_for(header).expect("a loop").get_blocks().to_vec()
        };
        for block in blocks {
            self.delete_dead_phis(block);
        }
        // The exit branch's condition changed.
        let (se, env) = self.session.split(self.layout, &self.outer.globals, &self.outer.metadata);
        se.forget_loop(&env.cx(&*self.context, &*self.function), header);
        true
    }

    fn taken(
        &self,
        branch: InstId,
    ) -> BlockId {
        match self.function.instruction(branch).operands[1] {
            Operand::Block(block) => block,
            other => unreachable!("a branch names a block, not {other:?}"),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/HardwareLoops.cpp#L419
    /// `HardwareLoop::InitLoopCount` without the entry test: the preheader and
    /// the trip count `exit count + 1` written there, in the counter's type.
    fn init_loop_count(
        &mut self,
        header: BlockId,
        info: &Info,
    ) -> Option<(BlockId, Operand)> {
        let count_ty = Ty::Int(info.target.count_bits);
        let (se, env) = self.session.split(self.layout, &self.outer.globals, &self.outer.metadata);
        let (preheader, trip) = {
            let cx = env.cx(&*self.context, &*self.function);
            let preheader = cx.loops.get_loop_for(header)?.get_loop_preheader(self.function)?;
            let count = se.get_truncate_or_zero_extend(&cx, info.exit_count, count_ty, 0);
            let one = se.get_one(&cx, count_ty);
            let trip = se
                .get_add_expr(
                    &cx,
                    vec![llrm_analysis::scev::expr::Use::of(count), llrm_analysis::scev::expr::Use::of(one)],
                    Default::default(),
                    0,
                )
                .scev;
            (preheader, trip)
        };
        let at = self.function.terminator(preheader)?;
        {
            let cx = env.cx(&*self.context, &*self.function);
            if !self.rewriter.is_safe_to_expand_at(&cx, se, trip, at) {
                return None;
            }
        }
        self.rewriter.set_insert_point(at);
        let count =
            self.rewriter.expand_code_for(&env, self.context, self.declared, self.function, se, trip, Some(count_ty));
        Some((preheader, count))
    }

    /// `Builder.CreateIntrinsic(ID, Ty, args)` before `at`: a call of the
    /// declaration `<name>.i<width>`, its result of type `returns`.
    fn insert_intrinsic_call(
        &mut self,
        at: InstId,
        name: &str,
        overload: llrm_mir::types::TypeId,
        returns: llrm_mir::types::TypeId,
        args: &[Operand],
    ) -> Operand {
        let width = self.context.types.int_bits(overload).expect("an integer");
        let parameters = vec![overload; args.len()];
        let function_type = self.context.types.intern(Type::Function { returns, parameters, variadic: false });
        let callee = self.declared.declare(&format!("{name}.i{width}"), function_type);
        let pointer = self.context.types.ptr(0);
        let callee =
            Operand::Constant(self.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
        let info = CallInfo {
            function_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); args.len()],
            attrs: Vec::new(),
            tail: Tail::default(),
        };
        let mut operands = args.to_vec();
        operands.push(callee);
        let call =
            self.function.create_instruction(Opcode::Call(Box::new(info)), returns, operands, Flags::default(), None);
        self.function.insert(call, Position::Before(at)).expect("a placed instruction");
        match self.function.instruction(call).result {
            Some(result) => Operand::Value(result),
            None => Operand::Block(BlockId(0)),
        }
    }

    /// `RecursivelyDeleteTriviallyDeadInstructions(V)`.
    fn delete_trivially_dead(
        &mut self,
        root: Operand,
    ) {
        let mut work: Vec<InstId> = crate::lftr::def_of(self.function, root).into_iter().collect();
        while let Some(inst) = work.pop() {
            if self.function.is_erased(inst)
                || !crate::local::is_instruction_trivially_dead(
                    self.context,
                    self.outer.callees(),
                    &self.outer.globals,
                    self.function,
                    inst,
                )
            {
                continue;
            }
            let operands = self.function.instruction(inst).operands.clone();
            self.function.set_operands(inst, Vec::new());
            self.function.erase(inst).expect("a dead instruction");
            work.extend(operands.into_iter().filter_map(|one| crate::lftr::def_of(self.function, one)));
        }
    }

    /// `DeleteDeadPHIs(BB)`: the phis at the top of `block` nothing reads.
    fn delete_dead_phis(
        &mut self,
        block: BlockId,
    ) {
        let phis: Vec<InstId> = crate::edges::phis(self.function, block).into_iter().collect();
        for phi in phis {
            if !self.function.is_erased(phi)
                && crate::local::is_instruction_trivially_dead(
                    self.context,
                    self.outer.callees(),
                    &self.outer.globals,
                    self.function,
                    phi,
                )
            {
                let operands = self.function.instruction(phi).operands.clone();
                self.function.set_operands(phi, Vec::new());
                self.function.erase(phi).expect("a dead phi");
                let _ = operands;
            }
        }
    }
}

/// DEVIATE 1: whether `ec`, taken as an unsigned number, is held by `bits`.
fn fits(
    se: &mut ScalarEvolution,
    cx: &llrm_analysis::scev::Cx,
    ec: Scev,
    bits: u32,
) -> bool {
    let width = se.get_type_size_in_bits(cx, se.type_of(ec));
    width <= bits || se.get_unsigned_range_max(cx, ec).active_bits() <= bits
}

/// Whether the loop already holds a counter intrinsic.
fn contains_hardware_loop_intrinsic(
    cx: &llrm_analysis::scev::Cx,
    lp: &llrm_mir::loopinfo::Loop<'_, BlockId>,
) -> bool {
    use llrm_mir::intrinsics::Intrinsic;
    lp.get_blocks()
        .iter()
        .any(
            |&block| cx.function
                .iter_block(block)
                .any(
                    |inst| matches!(
                        crate::local::intrinsic_of(cx.context, cx.globals, cx.function, inst),
                        Some(Intrinsic::SetLoopIterations | Intrinsic::TestSetLoopIterations | Intrinsic::LoopDecrement)
                    ),
                ),
        )
}

#[cfg(test)]
#[path = "hardwareloops_tests.rs"]
mod tests;
