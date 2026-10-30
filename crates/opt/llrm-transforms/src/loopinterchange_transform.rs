//! `LoopInterchangeTransform` of `LoopInterchange.cpp` at `d1106deb71cc`: the
//! rewrite that swaps the roles of two adjacent loops of a perfect nest. See
//! [`crate::loopinterchange`] for the pass.
//!
//! # How it works
//! `transform` splits the inner latch and clones the exit compare and the
//! induction updates into the new latch, splits the inner header so it has one
//! successor, and moves the code of the inner preheader into the outer header.
//! `adjust_loop_branches` points the outer predecessor at the inner preheader,
//! the inner header at the outer preheader and the latches at each other's
//! exits, then moves the LCSSA and reduction phis to the block that is now
//! their loop's, swaps the contents of the two preheaders and makes LCSSA
//! again for the values defined in the old outer header.
//!
//! # Deviations from LLVM
//! - D1: `restructureLoops` and the dominator tree updates are replaced by
//!   `LoopInfo::analyze` and `DomTree::of` after the branches are rewired (see
//!   [`crate::loopinterchange`]).
//!
//! # Example
//! Before, `i` outer and `j` inner: `i.header -> j.preheader -> j.header ->
//! j.body -> j.latch -> i.latch`. After, `j.preheader -> j.header ->
//! i.preheader -> i.header -> j.body -> i.latch -> j.latch`: the old inner
//! header and latch bound the new outer loop.

use llrm_analysis::scev::Cx;
use llrm_analysis::scev::lcssa_form::form_lcssa_for_instructions;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::loopinfo::{Loop, LoopInfo};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::types::TypeId;

use crate::edges::{split_block, split_block_at, split_block_predecessors};

/// `LoopInterchangeLegality::InnerReduction`: a reduction of the inner loop
/// stored into memory by the outer one.
#[derive(Clone, Debug)]
pub struct InnerReduction {
    /// The reduction phi itself.
    pub reduction: InstId,
    pub init: Operand,
    pub next: Operand,
    /// The LCSSA phi.
    pub lcssa_phi: InstId,
    /// The store of the reduction's result.
    pub lcssa_store: InstId,
    /// The memory location.
    pub mem_ref: Operand,
    pub elem_ty: TypeId,
}

/// What the transform needs of a pair of loops and of what legality found,
/// read before anything changes.
pub struct Plan {
    pub outer_header: BlockId,
    pub outer_latch: BlockId,
    pub outer_preheader: BlockId,
    pub inner_header: BlockId,
    pub inner_latch: BlockId,
    pub inner_preheader: BlockId,
    /// Every block of the inner loop.
    pub inner_blocks: Vec<BlockId>,
    /// The blocks whose innermost loop is the inner one.
    pub inner_direct: Vec<BlockId>,
    pub outer_inner_reductions: Vec<InstId>,
    pub inductions: Vec<InstId>,
    pub drop_no_wrap: Vec<InstId>,
    pub drop_no_inf: Vec<InstId>,
    pub inner_reductions: Vec<InnerReduction>,
}

impl Plan {
    #[allow(clippy::too_many_arguments)]
    pub fn of(
        cx: &Cx,
        outer: Loop,
        inner: Loop,
        outer_inner_reductions: Vec<InstId>,
        inductions: Vec<InstId>,
        drop_no_wrap: Vec<InstId>,
        drop_no_inf: Vec<InstId>,
        inner_reductions: Vec<InnerReduction>,
    ) -> Self {
        let function = cx.function;
        Self {
            outer_header: outer.get_header(),
            outer_latch: outer.get_loop_latch(function).expect("an outer latch"),
            outer_preheader: outer.get_loop_preheader(function).expect("an outer preheader"),
            inner_header: inner.get_header(),
            inner_latch: inner.get_loop_latch(function).expect("an inner latch"),
            inner_preheader: inner.get_loop_preheader(function).expect("an inner preheader"),
            inner_blocks: inner.get_blocks().to_vec(),
            inner_direct: inner
                .get_blocks()
                .iter()
                .copied()
                .filter(|&block| cx.loops.get_loop_for(block) == Some(inner))
                .collect(),
            outer_inner_reductions,
            inductions,
            drop_no_wrap,
            drop_no_inf,
            inner_reductions,
        }
    }
}

fn instruction_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(v) = operand else { return None };
    match function.value(v).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

fn phis(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect()
}

fn first_non_phi(
    function: &Function,
    block: BlockId,
) -> InstId {
    function
        .iter_block(block)
        .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
        .expect("a terminated block")
}

fn value_of(
    function: &Function,
    inst: InstId,
) -> Operand {
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

/// `BasicBlock::replacePhiUsesWith`: the phis of `block` that name `old` as an
/// incoming block name `new`.
fn replace_phi_uses_with(
    function: &mut Function,
    block: BlockId,
    old: BlockId,
    new: BlockId,
) {
    for phi in phis(function, block) {
        let operands = function.instruction(phi).operands.clone();
        for (at, operand) in operands.into_iter().enumerate() {
            if operand == Operand::Block(old) {
                function.set_operand(phi, at, Operand::Block(new));
            }
        }
    }
}

/// `updateSuccessor`: `term` jumps to `new` where it jumped to `old`.
fn update_successor(
    function: &mut Function,
    term: InstId,
    old: BlockId,
    new: BlockId,
) {
    let operands = function.instruction(term).operands.clone();
    let mut changed = false;
    for (at, operand) in operands.into_iter().enumerate() {
        if operand == Operand::Block(old) {
            function.set_operand(term, at, Operand::Block(new));
            changed = true;
        }
    }
    assert!(changed, "Expected a successor to be updated");
}

/// The unique successor of `block`: all its branches go to one block.
fn unique_successor(
    function: &Function,
    block: BlockId,
) -> Option<BlockId> {
    let successors = function.successors(block);
    (successors.len() == 1).then(|| successors[0])
}

/// The unique predecessor of `block`.
fn unique_predecessor(
    function: &Function,
    block: BlockId,
) -> Option<BlockId> {
    let predecessors = function.predecessors(block);
    (predecessors.len() == 1).then(|| predecessors[0])
}

/// `followLCSSA`.
fn follow_lcssa(
    function: &Function,
    mut value: Operand,
) -> Operand {
    loop {
        let Some(inst) = instruction_of(function, value) else { return value };
        let instruction = function.instruction(inst);
        if instruction.opcode != Opcode::Phi || instruction.operands.len() != 2 {
            return value;
        }
        value = instruction.operands[0];
    }
}

/// `reduction2Memory`: the inner reduction goes through memory, so the loops
/// can swap.
///
/// ```text
/// loop:                       loop:
///   re = phi<0.0, next>         tmp = MEM_REF[idx]
///   next = re op ...            new_var = first_iter ? 0.0 : tmp
/// endloop                       next = new_var op ...
/// reduc = phi<next>             MEM_REF[idx] = next
/// MEM_REF[idx] = reduc        endloop
/// ```
fn reduction_to_memory(
    function: &mut Function,
    context: &Context,
    layout: &DataLayout,
    plan: &Plan,
) {
    let reduction = &plan.inner_reductions[0];
    let header = plan.inner_header;
    let i1 = context.types.int(1);
    let at = first_non_phi(function, header);

    // Whether it is the first iteration.
    let one = Operand::Constant(context.constant(Constant { ty: i1, kind: ConstantKind::Int(1) }));
    let zero = Operand::Constant(context.constant(Constant { ty: i1, kind: ConstantKind::Int(0) }));
    let first_iter = function.create_instruction(
        Opcode::Phi,
        i1,
        vec![one, Operand::Block(plan.inner_preheader), zero, Operand::Block(plan.inner_latch)],
        Flags::default(),
        Some("first.iter"),
    );
    function.insert(first_iter, Position::Before(at)).expect("a placed header");

    // The reduction starts from a constant: the first iteration reads the
    // memory the previous outer iteration stored to.
    let align = layout.align(&context.types, reduction.elem_ty);
    let load = function.create_instruction(
        Opcode::Load { align: Some(align), volatile: false },
        reduction.elem_ty,
        vec![reduction.mem_ref],
        Flags::default(),
        None,
    );
    function.insert(load, Position::Before(at)).expect("a placed header");

    // `new_var` is the constant at first, the memory after.
    let new_var = function.create_instruction(
        Opcode::Select,
        reduction.elem_ty,
        vec![value_of(function, first_iter), reduction.init, value_of(function, load)],
        Flags::default(),
        Some("new.var"),
    );
    function.insert(new_var, Position::Before(at)).expect("a placed header");

    // Every use of the reduction reads the new variable.
    let phi_value = function.instruction(reduction.reduction).result.expect("a phi");
    function.replace_all_uses_with(phi_value, value_of(function, new_var));

    // The store moves into the inner loop, after the definition of `next`.
    function.set_operand(reduction.lcssa_store, 0, reduction.next);
    let next = instruction_of(function, reduction.next).expect("next is an instruction");
    function.move_to(reduction.lcssa_store, Position::After(next)).expect("a placed store");
}

/// `LoopInterchangeTransform::transform`.
pub fn transform(
    function: &mut Function,
    context: &mut Context,
    layout: &DataLayout,
    plan: &Plan,
) {
    if plan.inner_reductions.len() == 1 {
        reduction_to_memory(function, context, layout, plan);
    }

    // The values the induction phis take from the latch, other than phis.
    let mut inner_index_vars: Vec<InstId> = Vec::new();
    for &induction in &plan.inductions {
        let incoming = function
            .instruction(induction)
            .operands
            .chunks(2)
            .find(|pair| pair[1] == Operand::Block(plan.inner_latch))
            .map(|pair| pair[0])
            .expect("an incoming value from the inner latch");
        let inst = instruction_of(function, incoming).expect("Incoming value from loop latch isn't an instruction");
        if plan.inductions.contains(&inst) {
            continue;
        }
        inner_index_vars.push(inst);
    }

    // A new latch for the inner loop: split at the terminator, and the
    // condition and the operands that are not loop-invariant or the induction
    // phi are cloned into it.
    let terminator = function.terminator(plan.inner_latch).expect("a terminated latch");
    let new_latch = split_block_at(function, plan.inner_latch, terminator);
    let in_inner_loop = |block: BlockId| block == new_latch || plan.inner_blocks.contains(&block);
    let is_direct = |block: BlockId| block == new_latch || plan.inner_direct.contains(&block);

    let mut worklist: Vec<InstId> = Vec::new();
    let mut at = 0;
    let move_instructions = |function: &mut Function, worklist: &mut Vec<InstId>, at: &mut usize| {
        while *at < worklist.len() {
            let current = worklist[*at];
            // Phis cannot be cloned and moved: legality (the latch phi check)
            // ensures none reach the worklist.
            assert!(function.instruction(current).opcode != Opcode::Phi, "MoveInstructions does not support PHI nodes");
            // Duplicate the instruction and move it to the new latch. The uses
            // that have been moved read the clone.
            let clone = function.clone_instruction(current);
            let first = function.block(new_latch).first().expect("a terminated latch");
            function.insert(clone, Position::Before(first)).expect("a placed latch");
            if let (Some(original), Some(copy)) =
                (function.instruction(current).result, function.instruction(clone).result)
            {
                let uses: Vec<(InstId, u32)> =
                    function.users(original).iter().map(|one| (one.user, one.index)).collect();
                for (user, index) in uses {
                    let user_block = function.parent(user).expect("a placed user");
                    if !in_inner_loop(user_block) || user_block == new_latch || plan.inductions.contains(&user) {
                        function.set_operand(user, index as usize, Operand::Value(copy));
                    }
                }
            }
            // The operands of the moved instruction join the worklist, unless
            // they are outside the inner loop or the induction phi.
            let operands = function.instruction(current).operands.clone();
            for operand in operands {
                let Some(inst) = instruction_of(function, operand) else { continue };
                let block = function.parent(inst).expect("a placed operand");
                if !is_direct(block) || plan.inductions.contains(&inst) {
                    continue;
                }
                if !worklist.contains(&inst) {
                    worklist.push(inst);
                }
            }
            *at += 1;
        }
    };

    // FIXME (LLVM): should it interchange when the condition is a constant?
    let latch_terminator = function.terminator(new_latch).expect("a terminated latch");
    if let Some(condition) = instruction_of(function, function.instruction(latch_terminator).operands[0]) {
        worklist.push(condition);
    }
    move_instructions(function, &mut worklist, &mut at);
    for var in inner_index_vars {
        if !worklist.contains(&var) {
            worklist.push(var);
        }
    }
    move_instructions(function, &mut worklist, &mut at);

    // Split the inner header so that it has a unique successor.
    split_block(function, plan.inner_header);

    // The inner preheader's instructions may depend on values of the outer
    // header: they move there, as the preheader becomes the entry of the
    // interchanged nest. LICM moves the invariant ones out of the nest.
    let mut outer_header = plan.outer_header;
    if plan.inner_preheader != outer_header {
        // The phis of the inner preheader.
        for phi in phis(function, plan.inner_preheader) {
            let operands = function.instruction(phi).operands.clone();
            assert!(
                operands.chunks(2).all(|pair| pair[0] == operands[0]),
                "Expected equivalent incoming values in inner loop preheader"
            );
            let phi_value = function.instruction(phi).result.expect("a phi");
            function.replace_all_uses_with(phi_value, operands[0]);
            function.erase(phi).expect("an unused phi");
        }
        let body: Vec<InstId> = {
            let mut all: Vec<InstId> = function.iter_block(plan.inner_preheader).collect();
            all.pop();
            all
        };
        let before = function.terminator(outer_header).expect("a terminated outer header");
        for inst in body {
            function.move_to(inst, Position::Before(before)).expect("a placed instruction");
        }
    }

    adjust_loop_branches(function, context, layout, plan, new_latch, &mut outer_header);

    // Reordering the reduction's operations drops the flags that promise no
    // overflow or no infinity.
    for &inst in &plan.drop_no_wrap {
        let mut flags = function.instruction(inst).flags;
        flags.remove(Flags::NSW);
        flags.remove(Flags::NUW);
        function.set_flags(inst, flags);
    }
    for &inst in &plan.drop_no_inf {
        let mut flags = function.instruction(inst).flags;
        flags.remove(Flags::NINF);
        function.set_flags(inst, flags);
    }
}

/// `moveBBContents` and `swapBBContents`: the instructions of the two blocks
/// but their terminators change places.
fn swap_bb_contents(
    function: &mut Function,
    bb1: BlockId,
    bb2: BlockId,
) {
    let body = |function: &Function, block: BlockId| -> Vec<InstId> {
        let mut all: Vec<InstId> = function.iter_block(block).collect();
        all.pop();
        all
    };
    let (first, second) = (body(function, bb1), body(function, bb2));
    let term1 = function.terminator(bb1).expect("a terminated block");
    let term2 = function.terminator(bb2).expect("a terminated block");
    if !second.is_empty() {
        function.splice(&second, Position::Before(term1)).expect("placed instructions");
    }
    if !first.is_empty() {
        function.splice(&first, Position::Before(term2)).expect("placed instructions");
    }
}

/// `simplifyLCSSAPhis`: an LCSSA phi in the outer latch, when it is not the
/// inner exit, is removed.
fn simplify_lcssa_phis(
    function: &mut Function,
    outer_latch: BlockId,
    inner_exit: BlockId,
) {
    // Do not modify the phis where they belong, in exit blocks.
    if outer_latch == inner_exit {
        return;
    }
    for phi in phis(function, outer_latch) {
        let operands = function.instruction(phi).operands.clone();
        assert!(operands.len() == 2, "Single input phi expected");
        let value = function.instruction(phi).result.expect("a phi");
        function.replace_all_uses_with(value, operands[0]);
        function.erase(phi).expect("an unused phi");
    }
}

/// `moveLCSSAPhis`.
#[allow(clippy::too_many_arguments)]
fn move_lcssa_phis(
    function: &mut Function,
    loops: &LoopInfo,
    inner_header_loop: BlockId,
    inner_exit: BlockId,
    inner_header: BlockId,
    inner_latch: BlockId,
    outer_header: BlockId,
    outer_latch: BlockId,
    outer_exit: Option<BlockId>,
) {
    let inner_loop = loops.get_loop_for(inner_header_loop).expect("the inner loop");
    // LCSSA phis of the inner exit, defined in the header or the latch: those
    // blocks become the header and latch of the new outer loop, and the only
    // users are phis of the nest exit or the outer header (reductions, the
    // incoming value defined in the inner header). The user takes the
    // incoming value and the phi goes.
    for p in phis(function, inner_exit) {
        let operands = function.instruction(p).operands.clone();
        assert!(operands.len() == 2, "Only loops with a single exit are supported!");
        let Some(incoming) = operands.chunks(2).find(|pair| pair[1] == Operand::Block(inner_latch)).map(|pair| pair[0])
        else {
            continue;
        };
        let p_value = function.instruction(p).result.expect("a phi");
        let Some(inc_i) = instruction_of(function, incoming) else {
            // Not an instruction: it is loop invariant, and replaces the phi.
            function.replace_all_uses_with(p_value, incoming);
            function.erase(p).expect("an unused phi");
            continue;
        };
        // In a multi-level nest, follow LCSSA to the value defined by the
        // innermost loop.
        let Some(innermost) = instruction_of(function, follow_lcssa(function, value_of(function, inc_i))) else {
            continue;
        };
        let block = function.parent(innermost).expect("a placed instruction");
        // Incoming values from the inner loop body, not the header or latch,
        // stay.
        if block != inner_latch && block != inner_header {
            continue;
        }
        function.replace_all_uses_with(p_value, value_of(function, inc_i));
        function.erase(p).expect("an unused phi");
    }

    let lcssa_inner_exit = phis(function, inner_exit);
    let lcssa_inner_latch = phis(function, inner_latch);

    // The phis for values used outside the inner loop are in the inner exit.
    // A user outside the exit block is a use outside the interchanged nest, so
    // they stay, in the inner latch, the new exit of the innermost loop.
    for &p in &lcssa_inner_exit {
        let at = first_non_phi(function, inner_latch);
        function.move_to(p, Position::Before(at)).expect("a placed phi");
    }
    // The inner latch's LCSSA phis come from a child loop: they move to the new
    // inner latch.
    for &p in &lcssa_inner_latch {
        let at = first_non_phi(function, inner_exit);
        function.move_to(p, Position::Before(at)).expect("a placed phi");
    }

    // The LCSSA phis of the nest exit with values defined in the outer loop get
    // a new phi in the inner latch, now the exit of the outer loop.
    if let Some(outer_exit) = outer_exit {
        for p in phis(function, outer_exit) {
            let operands = function.instruction(p).operands.clone();
            if operands.len() != 2 {
                continue;
            }
            // Values defined in the inner loop were already updated.
            let Some(inst) = instruction_of(function, operands[0]) else { continue };
            let parent = function.parent(inst).expect("a placed instruction");
            if loops.get_loop_for(parent) == Some(inner_loop) {
                continue;
            }
            let new_phi = function.clone_instruction(p);
            let mut new_operands = vec![operands[0], Operand::Block(outer_latch)];
            // There may be incoming edges from other blocks: the original outer
            // header.
            for pred in function.edge_predecessors(inner_latch) {
                if pred == outer_latch {
                    continue;
                }
                new_operands.push(operands[0]);
                new_operands.push(Operand::Block(pred));
            }
            function.set_operands(new_phi, new_operands);
            let at = first_non_phi(function, inner_latch);
            function.insert(new_phi, Position::Before(at)).expect("a placed latch");
            function.set_operand(p, 0, value_of(function, new_phi));
        }
    }

    // The moved phis name the new latch where they named the old.
    replace_phi_uses_with(function, inner_latch, inner_latch, outer_latch);
    let _ = (outer_header, loops);
}

/// `adjustLoopBranches`.
fn adjust_loop_branches(
    function: &mut Function,
    context: &mut Context,
    layout: &DataLayout,
    plan: &Plan,
    new_latch: BlockId,
    outer_header: &mut BlockId,
) {
    let mut outer_preheader = plan.outer_preheader;
    let mut inner_preheader = plan.inner_preheader;
    let inner_header = plan.inner_header;
    let inner_latch = new_latch;
    let outer_latch = plan.outer_latch;
    let inner_exit = unique_successor_of_latch(function, inner_latch, inner_header);

    simplify_lcssa_phis(function, outer_latch, inner_exit);

    // Both preheaders hold no phi and have a single predecessor, so they move
    // easily. An extra preheader is made where they do not.
    let has_phi = |function: &Function, block: BlockId| {
        function.iter_block(block).next().is_some_and(|first| function.instruction(first).opcode == Opcode::Phi)
    };
    if has_phi(function, outer_preheader) || unique_predecessor(function, outer_preheader).is_none() {
        outer_preheader = split_block_predecessors(function, *outer_header, &[outer_preheader]);
    }
    if inner_preheader == *outer_header {
        inner_preheader = split_block_predecessors(function, inner_header, &[inner_preheader]);
    }

    let outer_predecessor = unique_predecessor(function, outer_preheader).expect("a unique predecessor");
    let inner_latch_predecessor = unique_predecessor(function, inner_latch).expect("a unique predecessor");
    let outer_latch_bi = function.terminator(outer_latch).expect("a terminated latch");
    let inner_latch_bi = function.terminator(inner_latch).expect("a terminated latch");
    let outer_header_bi = function.terminator(*outer_header).expect("a terminated header");
    let inner_header_bi = function.terminator(inner_header).expect("a terminated header");
    assert!(
        function.instruction(outer_latch_bi).operands.len() == 3
            && function.instruction(inner_latch_bi).operands.len() == 3,
        "Failed to find a conditional branch"
    );
    let inner_latch_predecessor_bi = function.terminator(inner_latch_predecessor).expect("a terminated block");
    let outer_predecessor_bi = function.terminator(outer_predecessor).expect("a terminated block");
    let inner_header_successor =
        unique_successor(function, inner_header).expect("a unique successor for the inner loop header");

    // The loop preheader and headers. The branches in the outer predecessor
    // and the outer header can be unconditional, or conditional with
    // duplicates.
    update_successor(function, outer_predecessor_bi, outer_preheader, inner_preheader);
    // The outer header might or might not branch to the outer latch; it does
    // to the inner preheader.
    if function.successors(*outer_header).contains(&outer_latch) {
        // The outer header branches to the inner latch.
        update_successor(function, outer_header_bi, outer_latch, inner_latch);
    }
    update_successor(function, outer_header_bi, inner_preheader, inner_header_successor);

    // The reduction phis' incoming block has changed.
    replace_phi_uses_with(function, inner_header_successor, inner_header, *outer_header);

    update_successor(function, inner_header_bi, inner_header_successor, outer_preheader);

    // The loop latches.
    let operands = function.instruction(inner_latch_bi).operands.clone();
    let inner_latch_successor = match (operands[1], operands[2]) {
        (Operand::Block(first), Operand::Block(second)) => {
            if first == inner_header {
                second
            } else {
                first
            }
        }
        _ => unreachable!("a conditional branch names blocks"),
    };
    update_successor(function, inner_latch_predecessor_bi, inner_latch, inner_latch_successor);

    let operands = function.instruction(outer_latch_bi).operands.clone();
    let outer_latch_successor = match (operands[1], operands[2]) {
        (Operand::Block(first), Operand::Block(second)) => {
            if first == *outer_header {
                second
            } else {
                first
            }
        }
        _ => unreachable!("a conditional branch names blocks"),
    };
    update_successor(function, inner_latch_bi, inner_latch_successor, outer_latch_successor);
    update_successor(function, outer_latch_bi, outer_latch_successor, inner_latch);

    // D1: the loops, as `restructureLoops` leaves them.
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let outer_exit = loops.get_loop_for(inner_header).and_then(|one| one.get_exit_block(&*function));

    move_lcssa_phis(
        function,
        &loops,
        inner_header,
        inner_latch_successor,
        inner_header,
        inner_latch,
        *outer_header,
        outer_latch,
        outer_exit,
    );
    // The nest exit's phis: the outer latch has been replaced by the inner's.
    replace_phi_uses_with(function, outer_latch_successor, outer_latch, inner_latch);

    // The reduction phis of the inner and outer headers.
    let inner_loop_phis: Vec<InstId> =
        phis(function, inner_header).into_iter().filter(|phi| plan.outer_inner_reductions.contains(phi)).collect();
    let outer_loop_phis: Vec<InstId> =
        phis(function, *outer_header).into_iter().filter(|phi| plan.outer_inner_reductions.contains(phi)).collect();

    // The remaining reduction phis move from the outer to the inner header and
    // back: they are part of a reduction across both loops, and what is left
    // is the incoming blocks.
    for &phi in &outer_loop_phis {
        let at = first_non_phi(function, inner_header);
        function.move_to(phi, Position::Before(at)).expect("a placed phi");
    }
    for &phi in &inner_loop_phis {
        let at = first_non_phi(function, *outer_header);
        function.move_to(phi, Position::Before(at)).expect("a placed phi");
    }

    // The incoming blocks of the moved phis.
    replace_phi_uses_with(function, *outer_header, inner_preheader, outer_preheader);
    replace_phi_uses_with(function, *outer_header, inner_latch, outer_latch);
    replace_phi_uses_with(function, inner_header, outer_preheader, inner_preheader);
    replace_phi_uses_with(function, inner_header, outer_latch, inner_latch);

    // The preheaders swap their contents so each definition sits in the
    // preheader of the loop it now belongs to, before the LCSSA phis are made.
    debug_assert_eq!(
        loops.get_loop_for(*outer_header).and_then(|one| one.get_loop_preheader(&*function)),
        Some(outer_preheader)
    );
    debug_assert_eq!(
        loops.get_loop_for(inner_header).and_then(|one| one.get_loop_preheader(&*function)),
        Some(inner_preheader)
    );
    swap_bb_contents(function, outer_preheader, inner_preheader);

    // Values defined in the outer loop header may be used in the inner loop
    // latch: after the interchange they are defined in the new inner loop and
    // used in the new outer one, so they need LCSSA phis.
    let may_need_lcssa_phis: Vec<InstId> = {
        let mut all: Vec<InstId> = function.iter_block(*outer_header).collect();
        all.pop();
        all
    };
    form_lcssa_for_instructions(context, layout, function, &loops, &tree, may_need_lcssa_phis);
}

/// The inner exit block: the successor of the latch that is not the header.
fn unique_successor_of_latch(
    function: &Function,
    latch: BlockId,
    header: BlockId,
) -> BlockId {
    function.successors(latch).into_iter().find(|&block| block != header).expect("the latch leaves the loop")
}

/// `formLCSSARecursively` of the loop headed by `header`: its sub-loops first,
/// then itself (`formLCSSA`).
pub fn form_lcssa_recursively(
    function: &mut Function,
    context: &mut Context,
    layout: &DataLayout,
    header: BlockId,
) {
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let Some(top) = loops.get_loop_for(header) else { return };

    /// The loops in the order `formLCSSARecursively` closes them.
    fn post_order<'a>(
        l: Loop<'a>,
        out: &mut Vec<Loop<'a>>,
    ) {
        for sub in l.get_sub_loops() {
            post_order(sub, out);
        }
        out.push(l);
    }
    let mut order = Vec::new();
    post_order(top, &mut order);
    for l in order {
        let exit_blocks = l.get_exit_blocks(&*function);
        if exit_blocks.is_empty() {
            continue;
        }
        // `computeBlocksDominatingExits`: from the exits up the dominator tree,
        // to the header; a block that dominates no exit defines nothing used
        // outside.
        let mut dominating: Vec<BlockId> = Vec::new();
        let mut blocks = exit_blocks.clone();
        while let Some(block) = blocks.pop() {
            if l.get_header() == block {
                continue;
            }
            let Some(idom) = tree.immediate_dominator(block) else { continue };
            if !l.contains(idom) {
                continue;
            }
            if !dominating.contains(&idom) {
                dominating.push(idom);
                blocks.push(idom);
            }
        }
        let mut worklist: Vec<InstId> = Vec::new();
        for block in dominating {
            // The blocks of sub-loops are in LCSSA already.
            if loops.get_loop_for(block) != Some(l) {
                continue;
            }
            for inst in function.iter_block(block) {
                let Some(result) = function.instruction(inst).result else { continue };
                let users = function.users(result);
                if users.is_empty() {
                    continue;
                }
                if users.has_one_use() {
                    let user = users.iter().next().expect("a use").user;
                    if function.parent(user) == Some(block) && function.instruction(user).opcode != Opcode::Phi {
                        continue;
                    }
                }
                worklist.push(inst);
            }
        }
        form_lcssa_for_instructions(context, layout, function, &loops, &tree, worklist);
    }
}
