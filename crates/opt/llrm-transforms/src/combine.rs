//! One worklist over a function's instructions: each is visited, simplified to
//! a value it equals when its operands say so, and what read it is visited
//! again.
//!
//! A translation of LLVM's InstCombine driver,
//! [`InstructionCombining.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp)
//! at d1106deb71cc: `prepareWorklist` (:6031) seeds it, `run` (:5780) drains
//! it, `eraseInstFromFunction` (InstCombineInternal.h:504) re-queues what lost
//! a user, over
//! [`InstructionWorklist`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Transforms/Utils/InstructionWorklist.h).
//! `visit` is [`llrm_mir::simplify::simplify_instruction_in`], then the
//! translated families' `visit*` of the opcode (`visitors`).
//!
//! MIRROR: the seed in reverse reverse-post-order after folding and deleting
//! the dead as it walks; the live-successor handling of a constant branch (the
//! phi inputs of a dead edge become poison); the pop, delete-if-dead, visit,
//! replace-all-uses, `pushUsersToWorkList`, erase sequence.
//!
//! DEVIATE, with the reason: blocks no live edge reaches keep their
//! instructions (`decide` removes them; LLVM's
//! `removeAllNonTerminatorAndEHPadInstructions` is a CFG edit this pass does
//! not make); a call is never trivially dead here (`dead` knows what a call
//! does not write); `visit` runs only the families translated so
//! far (`visitors`: add, mul, and, or, xor, div, rem, shifts,
//! trunc, zext, sext, icmp, getelementptr, select, store): the other `visit*`
//! rewrites are `algebraic`'s, to move in as they are translated.
//!
//! # Used by
//! `combine_pass::Combine`, the `combine@N` slots of LLVM's order.
//!
//! # Example
//! An instruction that simplifies is replaced, and what read it is visited
//! again:
//!
//! ```text
//! %a = add i32 %x, 0
//! %b = mul i32 %a, 1   ; %a becomes %x, then %b becomes %x
//! ```

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, MetadataNode, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::simplify::simplify_instruction_in;
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::FunctionAnalyses;

use crate::combine_addsub::visit_add;
use crate::combine_and::visit_and;
use crate::combine_casts::{visit_sext, visit_trunc, visit_zext};
use crate::combine_compares::visit_icmp;
use crate::combine_muldivrem::{visit_mul, visit_sdiv, visit_srem, visit_udiv, visit_urem};
use crate::combine_or::visit_or;
use crate::combine_shifts::{visit_ashr, visit_lshr, visit_shl};
use crate::combine_sink::{droppable_uses, sink, sink_block, sinkable};
use crate::combine_sub::visit_sub;
use crate::combine_xor::visit_xor;
use crate::instworklist::InstWorklist;

/// `handleUseCountDecrement`: an operand that lost a user, and the one user it
/// has left, are looked at again.
pub(crate) fn handle_use_count_decrement(
    worklist: &mut InstWorklist,
    function: &Function,
    operand: Operand,
) {
    let Some(inst) = defining(function, operand) else { return };
    worklist.add(inst);
    if let Some(result) = function.instruction(inst).result
        && let Some(only) = function.users(result).only()
    {
        worklist.add(only.user);
    }
}

/// The instruction in the body that defines `operand`.
pub(crate) fn defining(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    function.parent(inst).map(|_| inst)
}

/// `isInstructionTriviallyDead` (Local.cpp:402): nothing reads it and running
/// it does nothing else, a call with no side effects included.
fn is_trivially_dead(
    env: &Env,
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    crate::local::is_instruction_trivially_dead(context, env.callees, env.globals, function, inst)
}

/// `eraseInstFromFunction`: nothing reads `inst`; its operands are looked at
/// again.
fn erase(
    function: &mut Function,
    worklist: &mut InstWorklist,
    inst: InstId,
) {
    let operands = function.instruction(inst).operands.to_vec();
    worklist.remove(inst);
    function.erase(inst).expect("nothing reads it");
    for operand in operands {
        handle_use_count_decrement(worklist, function, operand);
    }
}

/// The sink step of `run` (:5865): `inst` moves to the block of its only
/// user when `tryToSinkInstruction` allows it; its operands' definitions and
/// the `llvm.assume`s that read it are looked at again. Whether it moved.
fn try_to_sink(
    c: &mut Combiner,
    inst: InstId,
) -> bool {
    let (dest, dropped) = {
        let q = crate::combine_shared::query(c, Some(inst));
        let Some(dest) = sink_block(&q, inst) else { return false };
        if !sinkable(&q, inst, dest) {
            return false;
        }
        (dest, droppable_uses(&q, inst, dest))
    };
    sink(c.context, c.function, inst, dest, &dropped);
    for one in &dropped {
        c.worklist.push(one.user);
    }
    let operands = c.function.instruction(inst).operands.to_vec();
    for operand in operands {
        if let Some(def) = defining(c.function, operand) {
            c.worklist.push(def);
        }
    }
    true
}

/// `InstCombinerImpl::prepareWorklist` (:6031): the blocks in reverse
/// post-order, each live one's instructions folded or deleted where that is
/// trivial, a constant branch's dead edge cut from the phis it feeds, then the
/// rest pushed so the first is popped first. Whether anything changed.
fn prepare(
    env: &Env,
    context: &mut Context,
    function: &mut Function,
    worklist: &mut InstWorklist,
) -> bool {
    let mut changed = false;
    let Some(entry) = function.entry() else { return false };
    let tree = env.analyses.dominator_tree();
    let graph = llrm_analysis::cfg::graph(function);
    let order = llrm_support::graph::reverse_postorder(&graph, llrm_analysis::cfg::id(entry));
    let mut dead_edges = std::collections::BTreeSet::<(BlockId, BlockId)>::new();
    let mut live = std::collections::BTreeSet::<BlockId>::new();
    let mut seeded: Vec<InstId> = Vec::new();
    for at in order {
        let block = llrm_analysis::cfg::block(at);
        if !tree.is_reachable(block) {
            continue;
        }
        // A block no live edge enters (every way in is dead, or comes from a
        // block it dominates) is skipped, its successors' edges from it
        // dead.
        if block != entry {
            let predecessors = function.predecessors(block);
            if predecessors.iter().all(|&pred| dead_edges.contains(&(pred, block)) || tree.dominates(block, pred)) {
                for succ in function.successors(block) {
                    handle_only_live_successor(context, function, &mut dead_edges, &mut changed, block, None, succ);
                }
                continue;
            }
        }
        live.insert(block);
        for inst in function.iter_block(block).collect::<Vec<_>>() {
            if function.parent(inst).is_none() {
                continue;
            }
            // ConstantProp an instruction that is trivially constant.
            let instruction = function.instruction(inst);
            let first_constant =
                instruction.operands.is_empty() || matches!(instruction.operands.first(), Some(Operand::Constant(_)));
            if let Some(result) = instruction.result
                && !function.users(result).is_empty()
                && first_constant
                && let Some(Operand::Constant(found)) =
                    simplify_instruction_in(context, env.globals, env.layout, Some(&env.analyses), function, inst)
            {
                function.replace_value(result, Operand::Constant(found));
                if is_trivially_dead(env, context, function, inst) {
                    let _ = function.erase(inst);
                }
                changed = true;
                continue;
            }
            seeded.push(inst);
        }
        // A branch or switch on undef or poison is UB: no successor is live.
        if let Some(last) = function.terminator(block)
            && matches!(function.instruction(last).opcode, Opcode::Br | Opcode::Switch)
            && let Some(&Operand::Constant(condition)) = function.instruction(last).operands.first()
            && matches!(
                context.get(condition).kind,
                llrm_mir::context::ConstantKind::Undef | llrm_mir::context::ConstantKind::Poison
            )
        {
            for succ in function.successors(block) {
                handle_only_live_successor(context, function, &mut dead_edges, &mut changed, block, None, succ);
            }
            continue;
        }
        // A branch on a number has one live successor.
        if let Some(branch) = function.terminator(block)
            && function.instruction(branch).opcode == Opcode::Br
            && let [Operand::Constant(condition), Operand::Block(taken), Operand::Block(other)] =
                function.instruction(branch).operands[..]
            && let Some(bits) = number(context, condition)
        {
            let (live, dead) = if bits & 1 == 1 { (taken, other) } else { (other, taken) };
            handle_only_live_successor(context, function, &mut dead_edges, &mut changed, block, Some(live), dead);
        }
    }
    // Remove instructions inside unreachable blocks
    // (`removeAllNonTerminatorAndEHPadInstructions`): this keeps the combine
    // from the bad special cases, and reduces use counts.
    for block in function.layout().to_vec() {
        if live.contains(&block) {
            continue;
        }
        let last = function.terminator(block);
        for inst in function.iter_block(block).collect::<Vec<_>>().into_iter().rev() {
            if Some(inst) == last || matches!(function.instruction(inst).opcode, Opcode::LandingPad { .. }) {
                continue;
            }
            if let Some(result) = function.instruction(inst).result
                && !function.users(result).is_empty()
            {
                let ty = function.instruction(inst).ty;
                let poison = Operand::Constant(
                    context.constant(llrm_mir::context::Constant { ty, kind: llrm_mir::context::ConstantKind::Poison }),
                );
                function.replace_value(result, poison);
            }
            let _ = function.erase(inst);
            changed = true;
        }
    }
    // Pushed in reverse so the first instruction is the first popped, the dead
    // ones deleted as they are met.
    for inst in seeded.into_iter().rev() {
        if function.parent(inst).is_none() {
            continue;
        }
        if is_trivially_dead(env, context, function, inst) {
            let _ = function.erase(inst);
            changed = true;
            continue;
        }
        llrm_mir::counts::bump(llrm_mir::counts::Event::CombineInitial);
        worklist.push(inst);
    }
    changed
}

/// A constant integer condition's value.
fn number(
    context: &Context,
    id: llrm_mir::context::ConstantId,
) -> Option<u128> {
    match context.get(id).kind {
        llrm_mir::context::ConstantKind::Int(bits) => Some(bits),
        llrm_mir::context::ConstantKind::Null | llrm_mir::context::ConstantKind::Zero => Some(0),
        _ => None,
    }
}

/// `HandleOnlyLiveSuccessor` (:6046): `block` goes to `live` only; the edge to
/// `dead` is dead, and the inputs of `dead`'s phis that come over it are
/// poison.
fn handle_only_live_successor(
    context: &mut Context,
    function: &mut Function,
    dead_edges: &mut std::collections::BTreeSet<(BlockId, BlockId)>,
    changed: &mut bool,
    block: BlockId,
    live: Option<BlockId>,
    dead: BlockId,
) {
    if Some(dead) == live || !dead_edges.insert((block, dead)) {
        return;
    }
    for phi in function.iter_block(dead).collect::<Vec<_>>() {
        if function.instruction(phi).opcode != Opcode::Phi {
            break;
        }
        let ty = function.instruction(phi).ty;
        let poison = Operand::Constant(
            context.constant(llrm_mir::context::Constant { ty, kind: llrm_mir::context::ConstantKind::Poison }),
        );
        let arms: Vec<Operand> = function.instruction(phi).operands.to_vec();
        for (at, pair) in arms.chunks(2).enumerate() {
            if let [value, Operand::Block(from)] = *pair
                && from == block
                && !matches!(
                    value,
                    Operand::Constant(id) if context.get(id).kind == llrm_mir::context::ConstantKind::Poison
                )
            {
                function.set_operand(phi, at * 2, poison);
                *changed = true;
            }
        }
    }
}

/// What `visit` returns, as LLVM's `Instruction *`: null, the instruction
/// itself (changed in place) or another value that replaces it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visit {
    None,
    Changed,
    /// `eraseInstFromFunction(I)` already done by the visit; counts as a
    /// change. The driver does not touch the instruction again.
    Erased,
    Replace(Operand),
}

/// What `visit` reads besides the function.
/// What the target's `Tti` lets the combiner do.
#[derive(Clone, Copy, Default)]
pub struct TargetHooks {
    /// `Tti::variable_shift_count_in_fixed_register`.
    pub fixed_shift_count: bool,
    /// `Tti::folds_select_to_cmp`.
    pub select_to_cmp: bool,
    /// `Tti::folds_icmp_of_constant_phi`.
    pub icmp_of_constant_phi: bool,
    /// DEVIATE PHICONST (`combine_phi.rs`): fold a phi of constants into all
    /// its users. Off where bytes are strict: a phi of constants costs a move
    /// per edge.
    pub phi_constant_users: bool,
    /// DEVIATE ICMPDIV-1 (`Tti::keeps_icmp_of_shared_division`).
    pub keeps_shared_division_compare: bool,
    /// DEVIATE PHICONST-2 (`Tti::folds_phi_constant_chains`).
    pub phi_constant_chains: bool,
    /// DEVIATE SYMPAIR-1 (`Tti::keeps_icmp_of_swapped_selects`).
    pub keeps_swapped_select_compare: bool,
}

pub struct Env<'a> {
    pub globals: &'a [GlobalValue],
    /// The module's metadata nodes, which ValueTracking reads (`!range`).
    pub metadata: &'a [MetadataNode],
    /// What each callee does to memory, for `isInstructionTriviallyDead`.
    pub callees: &'a Callees,
    pub layout: &'a DataLayout,
    pub analyses: FunctionAnalyses,
    /// `instcombine-code-sinking` (default true): the driver sinks an
    /// instruction to its only user's block after popping it.
    pub sinking: bool,
    pub hooks: TargetHooks,
    /// `BackEdges`, computed on the first ask (`computeBackEdges`).
    pub back_edges: std::cell::OnceCell<std::collections::BTreeSet<(BlockId, BlockId)>>,
}

/// `InstCombinerImpl`: the function being combined, the worklist, and the
/// edits a `visit*` makes, each of which queues what it touches.
pub struct Combiner<'a> {
    pub context: &'a mut Context,
    pub function: &'a mut Function,
    pub env: &'a Env<'a>,
    pub worklist: InstWorklist,
    /// What the pass declares in its module
    /// (`Intrinsic::getOrInsertDeclaration`).
    pub declared: &'a mut Declared,
    /// `SQ.getWithoutDomCondCache()`: ValueTracking queries drop the
    /// dominating conditions while set (`foldICmpUsingKnownBits`).
    pub without_dom_conditions: bool,
    /// The intrinsics this run declared that `env.globals` lacks, which a pattern must still read as intrinsics (`ValueTracking`'s
    /// `Query::pending_intrinsics`).
    pub pending_intrinsics: Vec<(u32, llrm_mir::valuetracking::pattern::IntrinsicId)>,
}

impl Combiner<'_> {
    /// `isBackEdge` (InstCombiner.h:393).
    pub fn is_back_edge(
        &self,
        from: BlockId,
        to: BlockId,
    ) -> bool {
        self.env
            .back_edges
            .get_or_init(|| llrm_analysis::cfgwalk::find_function_backedges(self.function).into_iter().collect())
            .contains(&(from, to))
    }

    /// `Worklist.add`.
    pub fn add_to_worklist(
        &mut self,
        inst: InstId,
    ) {
        self.worklist.add(inst);
    }

    /// `Builder.Insert*` through the `InstCombineWorklist` inserter: a new
    /// instruction before `at`, queued. Its value, if it has one.
    pub fn insert_before(
        &mut self,
        at: InstId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: Flags,
    ) -> InstId {
        let new = self.function.create_instruction(opcode, ty, operands, flags, None);
        self.function.insert(new, Position::Before(at)).expect("a placed instruction");
        // The builder's insertion point is `at`: `Builder.SetInsertPoint(&I)`
        // takes its debug location.
        let place =
            self.function.instruction(at).metadata.iter().find(|(kind, _)| kind == "dbg").map(|&(_, node)| node);
        if place.is_some() {
            self.function.set_metadata(new, "dbg", place);
        }
        self.worklist.add(new);
        new
    }

    /// `replaceInstUsesWith`: every use of `inst` reads `with`, and the users
    /// are queued (`pushUsersToWorkList`). `inst` stays, unread.
    pub fn replace_inst_with(
        &mut self,
        inst: InstId,
        with: Operand,
    ) {
        let result = self.function.instruction(inst).result.expect("a value");
        let users: Vec<InstId> = self.function.users(result).iter().map(|one| one.user).collect();
        self.function.replace_value(result, with);
        for user in users {
            self.worklist.push(user);
        }
        if let Some(new) = defining(self.function, with) {
            self.worklist.push(new);
        }
    }

    /// `eraseInstFromFunction`.
    pub fn erase(
        &mut self,
        inst: InstId,
    ) {
        erase(self.function, &mut self.worklist, inst);
    }
}

/// One family's `visit*` of an opcode.
type Visitor = fn(&mut Combiner, InstId) -> Visit;

/// `InstCombinerImpl::visit`'s switch (InstCombineInternal.h): the `visit*`
/// of the translated families, per opcode, tried in order until one acts. A
/// family that lands adds its function here and nowhere else.
fn visitors(opcode: &Opcode) -> &'static [Visitor] {
    match opcode {
        Opcode::Binary(BinaryOp::Add) => &[visit_add],
        Opcode::Binary(BinaryOp::Mul) => &[visit_mul],
        Opcode::Binary(BinaryOp::And) => &[visit_and],
        Opcode::Binary(BinaryOp::Or) => &[visit_or],
        Opcode::Binary(BinaryOp::Xor) => &[visit_xor],
        Opcode::Binary(BinaryOp::Sub) => &[visit_sub],
        Opcode::FNeg => &[crate::combine_fneg::visit_fneg],
        Opcode::Binary(BinaryOp::FDiv) => &[crate::combine_fp::visit_fdiv],
        Opcode::Binary(BinaryOp::FSub) => &[crate::combine_fp::visit_fsub],
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FMul) => &[crate::combine_fp::visit_fadd_or_fmul],
        Opcode::Binary(BinaryOp::UDiv) => &[visit_udiv],
        Opcode::Binary(BinaryOp::SDiv) => &[visit_sdiv],
        Opcode::Binary(BinaryOp::URem) => &[visit_urem],
        Opcode::Binary(BinaryOp::SRem) => &[visit_srem],
        Opcode::Binary(BinaryOp::Shl) => &[visit_shl],
        Opcode::Binary(BinaryOp::LShr) => &[visit_lshr],
        Opcode::Binary(BinaryOp::AShr) => &[visit_ashr],
        Opcode::Cast(CastOp::Trunc) => &[visit_trunc],
        Opcode::Cast(CastOp::ZExt) => &[visit_zext],
        Opcode::Cast(CastOp::SExt) => &[visit_sext],
        Opcode::Cast(CastOp::PtrToInt) => &[|c, inst| c.visit_ptr_to_int(inst)],
        Opcode::Cast(CastOp::IntToPtr) => &[|c, inst| c.visit_int_to_ptr(inst)],
        Opcode::Cast(CastOp::UIToFP) => &[crate::combine_fpcast::visit_uitofp],
        Opcode::Cast(CastOp::SIToFP) => &[crate::combine_fpcast::visit_sitofp],
        Opcode::Cast(CastOp::BitCast) => &[crate::combine_bitcast::visit_bitcast],
        Opcode::Cast(CastOp::FPTrunc) => &[crate::combine_fptrunc::visit_fptrunc],
        Opcode::Cast(CastOp::FPExt) => &[crate::combine_fpcast::visit_fpext],
        Opcode::Cast(CastOp::FPToUI | CastOp::FPToSI) => &[crate::combine_fptoint::visit_fptoi],
        Opcode::ICmp(_) => &[visit_icmp],
        Opcode::FCmp(_) => &[crate::combine_fcmp::visit_fcmp],
        Opcode::GetElementPtr { .. } => &[|c, inst| c.visit_get_element_ptr(inst)],
        Opcode::Alloca { .. } => &[|c, inst| c.visit_alloca(inst)],
        Opcode::Store { .. } => &[|c, inst| c.visit_store(inst)],
        Opcode::Load { .. } => &[|c, inst| c.visit_load(inst)],
        Opcode::Select => &[|c, inst| c.visit_select(inst)],
        Opcode::Br => &[|c, inst| c.visit_cond_br(inst), |c, inst| c.visit_uncond_br(inst)],
        Opcode::Switch => &[|c, inst| c.visit_switch(inst)],
        Opcode::Call(_) => &[|c, inst| c.visit_call(inst)],
        Opcode::Phi => &[|c, inst| c.visit_phi(inst)],
        Opcode::Freeze => &[|c, inst| c.visit_freeze(inst)],
        _ => &[],
    }
}

#[cfg(test)]
thread_local! {
    /// The instructions `visit` asked the generic `simplify_instruction_in`
    /// about.
    pub(crate) static GENERIC_SIMPLIFIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Whether the visitor of `opcode` opens with its own `simplify*Inst`
/// (`simplify_visited`, `simplify_first`, the shifts' `simplified` and the
/// compare's), unconditionally and before it changes anything.
fn simplifies_itself(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::ICmp(_)
            | Opcode::Binary(
                BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Xor
                    | BinaryOp::Mul
                    | BinaryOp::UDiv
                    | BinaryOp::SDiv
                    | BinaryOp::URem
                    | BinaryOp::SRem
                    | BinaryOp::Shl
                    | BinaryOp::LShr
                    | BinaryOp::AShr
            )
    )
}

/// `InstCombinerImpl::run`'s per-instruction step: what the instruction
/// simplifies to, else the first `visit*` of its opcode that acts.
pub fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    let env = c.env;
    // The visitors below open with their own `simplify*Inst`, as the pin's do;
    // a pass of the generic one before them asked the same question twice
    // (d_faces -O2: 9,931 asks, 18.6 M Ir, 9.6 M of it on compares).
    if !simplifies_itself(&c.function.instruction(inst).opcode)
        && let Some(result) = c.function.instruction(inst).result
        && let Some(found) = {
            #[cfg(test)]
            GENERIC_SIMPLIFIES.with(|one| one.set(one.get() + 1));
            simplify_instruction_in(c.context, env.globals, env.layout, Some(&env.analyses), c.function, inst)
        }
        && found != Operand::Value(result)
    {
        return Visit::Replace(found);
    }
    // A flags result is read by a comparison and nothing else: neither the
    // comparison nor what reads it (`sext (icmp slt v, 0)`) is rewritten into
    // a user of `v` that is not a comparison.
    let compares_flags = |inst| llrm_mir::facts::compares_flags_result(c.context, env.globals, c.function, inst);
    // An `i1` add, subtract or multiply is a xor or an and of the compare
    // (never a user of the flags): the selector refuses the arithmetic, so
    // it is always rewritten.
    let boolean_arithmetic =
        matches!(
            c.function.instruction(inst).opcode,
            Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul)
        )
            && c.context.types.int_bits(c.function.instruction(inst).ty) == Some(1);
    if !boolean_arithmetic
        && (compares_flags(inst)
            || c.function.instruction(inst).operands.iter().any(|operand| {
            matches!(
                operand,
                Operand::Value(value) if matches!(c.function.value(*value).def, ValueDef::Instruction(one) if compares_flags(one))
            )
        }))
    {
        return Visit::None;
    }
    // DEVIATE COMBINE-VEC: the arithmetic visitors read integer widths (MIR had
    // no vectors), so an operation on vectors, which the loop vectorizer makes,
    // is left as it is. Exit: vector-aware folds (owner arm64-vec, with LV-X3).
    if matches!(c.function.instruction(inst).opcode, Opcode::Binary(_) | Opcode::Cast(_))
        && matches!(c.context.types.get(c.function.instruction(inst).ty), llrm_mir::types::Type::Vector { .. })
    {
        return Visit::None;
    }
    // The table is 'static: the opcode (a clone per visit, a Box for calls) is
    // not held.
    let table = visitors(&c.function.instruction(inst).opcode);
    table.iter().map(|visitor| visitor(c, inst)).find(|found| *found != Visit::None).unwrap_or(Visit::None)
}

/// `InstCombinerImpl::run` (:5780): drain the worklist. Whether anything
/// changed.
fn drain(
    c: &mut Combiner,
    visit: fn(&mut Combiner, InstId) -> Visit,
) -> bool {
    let mut changed = false;
    llrm_mir::counts::bump(llrm_mir::counts::Event::CombineDrains);
    while !c.worklist.is_empty() {
        // Walk deferred instructions, deleting the dead among them (and so,
        // chain by chain, what only they read), and push the rest.
        while let Some(inst) = c.worklist.pop_deferred() {
            if c.function.parent(inst).is_none() {
                continue;
            }
            if is_trivially_dead(c.env, c.context, c.function, inst) {
                c.erase(inst);
                changed = true;
                continue;
            }
            c.worklist.push(inst);
        }
        let Some(inst) = c.worklist.remove_one() else { continue };
        if c.function.parent(inst).is_none() {
            continue;
        }
        // Check to see if we can DCE the instruction.
        if is_trivially_dead(c.env, c.context, c.function, inst) {
            c.erase(inst);
            changed = true;
            continue;
        }
        if c.env.sinking && try_to_sink(c, inst) {
            changed = true;
        }
        llrm_mir::counts::bump(llrm_mir::counts::Event::CombineVisits);
        match visit(c, inst) {
            Visit::None => {}
            Visit::Erased => changed = true,
            // Changed in place: it and its users are looked at again.
            Visit::Changed => {
                let result = c.function.instruction(inst).result;
                if let Some(result) = result {
                    let users: Vec<InstId> = c.function.users(result).iter().map(|one| one.user).collect();
                    for user in users {
                        c.worklist.push(user);
                    }
                }
                if is_trivially_dead(c.env, c.context, c.function, inst) {
                    c.erase(inst);
                } else {
                    c.worklist.push(inst);
                }
                changed = true;
            }
            Visit::Replace(found) => {
                c.replace_inst_with(inst, found);
                c.erase(inst);
                changed = true;
            }
        }
    }
    changed
}

/// One combine of `function`: the worklist seeded and drained. Whether anything
/// changed.
pub fn combine(
    context: &mut Context,
    globals: &[GlobalValue],
    callees: &Callees,
    metadata: &[MetadataNode],
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
) -> bool {
    combine_on_target(context, globals, callees, metadata, layout, function, declared, TargetHooks::default())
}

/// `combine` on a target that says whether a variable shift count must sit in
/// one register (`Tti::variable_shift_count_in_fixed_register`) and whether
/// it makes `scmp`/`ucmp` (`Tti::folds_select_to_cmp`).
pub fn combine_on_target(
    context: &mut Context,
    globals: &[GlobalValue],
    callees: &Callees,
    metadata: &[MetadataNode],
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    hooks: TargetHooks,
) -> bool {
    combine_for(context, globals, callees, metadata, layout, function, visit, true, declared, hooks)
}

/// `combine` with the `visit*` of a test, and no sinking: a test of one
/// family sees what that family does.
#[cfg(test)]
pub(crate) fn combine_with(
    context: &mut Context,
    globals: &[GlobalValue],
    metadata: &[MetadataNode],
    layout: &DataLayout,
    function: &mut Function,
    visit: fn(&mut Combiner, InstId) -> Visit,
) -> bool {
    let mut declared = Declared::over(std::rc::Rc::new(globals.to_vec()), metadata.len());
    combine_with_declared(context, globals, metadata, layout, function, visit, &mut declared)
}

/// `combine_with`, the declarations it makes kept in `declared` for the test
/// to place in the module.
#[cfg(test)]
pub(crate) fn combine_with_declared(
    context: &mut Context,
    globals: &[GlobalValue],
    metadata: &[MetadataNode],
    layout: &DataLayout,
    function: &mut Function,
    visit: fn(&mut Combiner, InstId) -> Visit,
    declared: &mut Declared,
) -> bool {
    combine_with_sinking(context, globals, &Callees::default(), metadata, layout, function, visit, false, declared)
}

pub(crate) fn combine_with_sinking(
    context: &mut Context,
    globals: &[GlobalValue],
    callees: &Callees,
    metadata: &[MetadataNode],
    layout: &DataLayout,
    function: &mut Function,
    visit: fn(&mut Combiner, InstId) -> Visit,
    sinking: bool,
    declared: &mut Declared,
) -> bool {
    combine_for(context, globals, callees, metadata, layout, function, visit, sinking, declared, TargetHooks::default())
}

pub(crate) fn combine_for(
    context: &mut Context,
    globals: &[GlobalValue],
    callees: &Callees,
    metadata: &[MetadataNode],
    layout: &DataLayout,
    function: &mut Function,
    visit: fn(&mut Combiner, InstId) -> Visit,
    sinking: bool,
    declared: &mut Declared,
    hooks: TargetHooks,
) -> bool {
    // InstCombine's `SQ.DC`: what the branches dominating an instruction say,
    // those visited so far.
    let analyses =
        FunctionAnalyses::of_declared(context, globals, metadata, layout, function, declared.assume_declared(globals))
            .with_lazy_dom_conditions();
    let env = Env { globals, metadata, callees, layout, analyses, sinking, hooks, back_edges: Default::default() };
    let mut c = Combiner {
        context,
        function,
        env: &env,
        worklist: InstWorklist::default(),
        declared,
        without_dom_conditions: false,
        pending_intrinsics: Vec::new(),
    };
    let mut changed = prepare(&env, c.context, c.function, &mut c.worklist);
    changed |= drain(&mut c, visit);
    changed
}

#[cfg(test)]
#[path = "combine_tests.rs"]
mod tests;
