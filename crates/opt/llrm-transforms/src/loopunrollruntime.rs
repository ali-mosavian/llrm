//! The epilog remainder of `UnrollRuntimeLoopRemainder`
//! (`LoopUnrollRuntime.cpp` at `d1106deb71cc`): before a loop of a run-time
//! trip count is unrolled by `count`, a copy of the loop runs the `trip count %
//! count` iterations the unrolled loop leaves.
//!
//! # How it works
//! The preheader computes the trip count `TC = BECount + 1` (SCEV expanded),
//! `xtraiter = TC % count`, and branches to the epilog when `BECount <u count -
//! 1` (fewer iterations than one unrolled body runs), else into the loop, which
//! now counts `niter` up to `TC - xtraiter`. The loop's exit leaves for a
//! guard that skips the epilog when `xtraiter` is zero. The epilog is a copy of
//! the loop's blocks (`.epil`) that counts `epil.iter` up to `xtraiter`, and
//! its header and exit phis take the values the unrolled loop left.
//!
//! The caller then unrolls the loop's body by `count` itself (the jam does).
//! With `count` 2 the epilog runs once: its backedge is broken and its latch
//! merged into its exit.
//!
//! # Deviations from LLVM
//! - D1: only the epilog form: the prolog is for `-unroll-allow-remainder`
//!   loops the unroller's runtime mode makes, which llrm has no pass for.
//! - D2: only a loop with one exiting block and one exit block. The pin's
//!   multi-exit mode needs `UnrollRuntimeMultiExit` or a target that says it
//!   pays; the jam refuses such loops before it gets here.
//! - D3: no branch weights (`!prof` of the guards): the pass manager keeps no
//!   weights for the loops this runs on, and the jam's tests have none.
//! - D4: no `llvm.loop.unroll.disable` on the epilog (`setLoopAlreadyUnrolled`)
//!   and no loop estimated trip count: a function pass cannot add metadata
//!   nodes; llrm has no partial unroller to run on the epilog.
//! - D5: `LoopInfo`, the dominator tree and SCEV are made again by the caller
//!   after the call, as in [`crate::loopinterchange`] D1.
//!
//! # Example
//! A loop `for (i = 0; i < n; i++) body` unrolled by 4 becomes, in the
//! preheader, `xtraiter = (n) & 3`; the unrolled loop runs `n - xtraiter`
//! iterations; the epilog `body.epil` runs `xtraiter` times.
//!
//! # See also
//! `UnrollRuntimeLoopRemainder` (L672), `ConnectEpilog` (L255),
//! `CloneLoopBlocks` (L404), `CreateTripRemainder` (L607).

use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expand_cost::ExpansionTti;
use llrm_analysis::scev::expander::Expander;
use llrm_analysis::scev::expr::{Scev, Ty, Use};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Declared, Outer};
use llrm_mir::types::TypeId;

use crate::edges::{phis, retarget, split_block_at, split_block_predecessors_of};

/// `SCEVCheapExpansionBudget`: its `cl::opt` default.
const CHEAP_EXPANSION_BUDGET: u32 = 4;

/// What the call reads and changes.
pub struct Env<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub function: &'a mut Function,
    pub declared: &'a mut Declared,
    pub outer: &'a Outer,
    pub tti: &'a dyn ExpansionTti,
}

/// What the call found out about the loop before it changes anything.
struct Facts {
    preheader: BlockId,
    latch: BlockId,
    latch_exit: BlockId,
    be_count: Scev,
    trip_count: Scev,
    abnormal_exits: bool,
    blocks: Vec<BlockId>,
    trip_ty: Ty,
}

fn int_constant(
    context: &Context,
    ty: TypeId,
    value: u128,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }))
}

fn type_of(
    env: &Env,
    operand: Operand,
) -> TypeId {
    env.function.operand_type(env.context, operand).expect("a typed operand")
}

fn value_of(
    function: &Function,
    inst: InstId,
) -> Operand {
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

fn is_constant(operand: Operand) -> bool {
    matches!(operand, Operand::Constant(_))
}

/// `Builder.CreateBinOp(op, a, b, name)` before `at`: folded where both
/// operands are constants.
fn create_binary(
    env: &mut Env,
    at: InstId,
    op: BinaryOp,
    a: Operand,
    b: Operand,
    name: &str,
) -> Operand {
    if is_constant(a)
        && is_constant(b)
        && let Some(folded) = llrm_mir::simplify::simplify_binop_operands(
            env.context,
            env.layout,
            env.function,
            op,
            Flags::default(),
            a,
            b,
        )
        && is_constant(folded)
    {
        return folded;
    }
    let ty = type_of(env, a);
    let inst = env
        .function
        .create_instruction(Opcode::Binary(op), ty, vec![a, b], Flags::default(), (!name.is_empty()).then_some(name));
    env.function.insert(inst, Position::Before(at)).expect("a placed instruction");
    value_of(env.function, inst)
}

/// `Builder.CreateICmp(pred, a, b, name)` before `at`.
fn create_icmp(
    env: &mut Env,
    at: InstId,
    pred: IntPredicate,
    a: Operand,
    b: Operand,
    name: &str,
) -> Operand {
    if is_constant(a)
        && is_constant(b)
        && let Some(folded) =
            llrm_mir::simplify::simplify_icmp_operands(env.context, env.layout, env.function, pred, a, b)
        && is_constant(folded)
    {
        return folded;
    }
    let ty = env.context.types.int(1);
    let inst = env
        .function
        .create_instruction(Opcode::ICmp(pred), ty, vec![a, b], Flags::default(), (!name.is_empty()).then_some(name));
    env.function.insert(inst, Position::Before(at)).expect("a placed instruction");
    value_of(env.function, inst)
}

/// `CreateTripRemainder`: `(BECount + 1) % Count` allowing the add to wrap.
fn create_trip_remainder(
    env: &mut Env,
    at: InstId,
    be_count: Operand,
    trip_count: Operand,
    count: u32,
) -> Operand {
    let ty = type_of(env, be_count);
    if count.is_power_of_two() {
        // Zero means either no iterations left for the epilog, or the add
        // wrapped: then the count is a multiple of `Count` too.
        let mask = int_constant(env.context, ty, u128::from(count - 1));
        return create_binary(env, at, BinaryOp::And, trip_count, mask, "xtraiter");
    }
    // `(BECount % Count) + 1` cannot wrap; it may be `Count`, hence the second
    // remainder.
    let count_c = int_constant(env.context, ty, u128::from(count));
    let rem = create_binary(env, at, BinaryOp::URem, be_count, count_c, "");
    let one = int_constant(env.context, ty, 1);
    let add = create_binary(env, at, BinaryOp::Add, rem, one, "");
    create_binary(env, at, BinaryOp::URem, add, count_c, "xtraiter")
}

/// `LoopBlocksDFS::perform`: the blocks of the loop in reverse post order.
pub(crate) fn reverse_post_order(
    function: &Function,
    header: BlockId,
    in_loop: &dyn Fn(BlockId) -> bool,
) -> Vec<BlockId> {
    let mut post: Vec<BlockId> = Vec::new();
    let mut seen: Vec<BlockId> = vec![header];
    let kids_of = |block: BlockId| -> Vec<BlockId> {
        let mut kids: Vec<BlockId> = function.successors(block).into_iter().collect();
        kids.reverse();
        kids
    };
    let mut stack: Vec<(BlockId, Vec<BlockId>)> = vec![(header, kids_of(header))];
    while let Some((block, pending)) = stack.last_mut() {
        if let Some(next) = pending.pop() {
            if in_loop(next) && !seen.contains(&next) {
                seen.push(next);
                stack.push((next, kids_of(next)));
            }
        } else {
            post.push(*block);
            stack.pop();
        }
    }
    post.reverse();
    post
}

/// `UnrollRuntimeLoopRemainder(L, Count, AllowExpensiveTripCount,
/// UseEpilogRemainder = true, ...)` of the loop headed by `header`: `None`
/// where it declines; else the header of the epilog loop, `None` there if its
/// backedge was broken (`Count == 2`). The caller makes its analyses again.
pub fn unroll_runtime_loop_remainder(
    env: &mut Env,
    session: &mut ScevSession,
    header: BlockId,
    count: u32,
    allow_expensive_trip_count: bool,
) -> Option<Option<BlockId>> {
    let outer = env.outer;
    // What is true of the loop before anything changes.
    let facts = {
        let (se, senv) = session.split(env.layout, &outer.globals, &outer.metadata);
        let cx = senv.cx(&*env.context, &*env.function);
        let l = cx.loops.get_loop_for(header)?;
        // Make sure the loop is in canonical form.
        if !l.is_loop_simplify_form(cx.function) {
            llrm_support::debug!("unroll-runtime", "not in simplify form");
            return None;
        }
        let latch = l.get_loop_latch(cx.function)?;
        let term = cx.function.terminator(latch)?;
        let branch = cx.function.instruction(term);
        if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
            llrm_support::debug!("unroll-runtime", "the latch is not a conditional branch");
            return None;
        }
        let exit_index = if branch.operands[1] == Operand::Block(header) { 2 } else { 1 };
        let Operand::Block(latch_exit) = branch.operands[exit_index] else { return None };
        if l.contains(latch_exit) {
            return None;
        }
        // D2: one exit, one exiting block.
        if l.get_exiting_block(cx.function).is_none() || !l.get_unique_non_latch_exit_blocks(cx.function).is_empty() {
            llrm_support::debug!("unroll-runtime", "multiple exits");
            return None;
        }
        // The trip count: only a loop with a computable one.
        let be_count = se.get_exit_count(&cx, header, latch, ExitCountKind::Exact);
        if se.is_could_not_compute(be_count) {
            llrm_support::debug!("unroll-runtime", "could not compute the exit block SCEV");
            return None;
        }
        let Ty::Int(be_width) = se.type_of(be_count) else { return None };
        let one = se.get_constant_of_type(&cx, se.type_of(be_count), 1);
        // Add 1: the backedge count leaves out the first iteration (it may
        // wrap; handled below).
        let trip_count = se.get_add_expr(&cx, vec![Use::of(be_count), Use::of(one)], Default::default(), 0).scev;
        if se.is_could_not_compute(trip_count) {
            return None;
        }
        let preheader = l.get_loop_preheader(cx.function)?;
        let preheader_br = cx.function.terminator(preheader)?;
        if !allow_expensive_trip_count {
            let expander = Expander::new("loop-unroll");
            if expander.is_high_cost_expansion(
                &cx,
                se,
                &[trip_count],
                header,
                CHEAP_EXPANSION_BUDGET,
                env.tti,
                preheader_br,
            ) {
                llrm_support::debug!("unroll-runtime", "high cost for expanding the trip count");
                return None;
            }
        }
        // `Log2(Count)` must not exceed the width, so an overflowing trip count
        // is easy to handle.
        if count.ilog2() > be_width {
            return None;
        }
        let abnormal_exits = !se.loop_has_no_abnormal_exits(&cx, header);
        let in_loop = |block: BlockId| l.contains(block);
        let blocks = reverse_post_order(cx.function, header, &in_loop);
        Facts {
            preheader,
            latch,
            latch_exit,
            be_count,
            trip_count,
            abnormal_exits,
            blocks,
            trip_ty: se.type_of(trip_count),
        }
    };
    let (preheader, latch, latch_exit) = (facts.preheader, facts.latch, facts.latch_exit);
    let loop_blocks = facts.blocks.clone();
    let in_loop_set = |block: BlockId| loop_blocks.contains(&block);

    // The loop is: PreHeader, Header ... Latch, LatchExit.
    // Split PreHeader to insert a branch around the loop for unrolling.
    let preheader_term = env.function.terminator(preheader).expect("a terminated preheader");
    let new_preheader = split_block_at(env.function, preheader, preheader_term);
    let new_name = format!("{}.new", env.function.block(preheader).name.clone().unwrap_or_default());
    env.function.set_block_name(new_preheader, &new_name);
    // Split LatchExit to create the phis of the branch above.
    let new_exit = split_block_predecessors_of(env.function, latch_exit, &[latch], true);
    env.function.set_block_name(
        new_exit,
        &format!("{}.unr-lcssa", env.function.block(latch_exit).name.clone().unwrap_or_default()),
    );
    // Split NewExit to insert the epilog remainder loop.
    let new_exit_term = env.function.terminator(new_exit).expect("a terminated block");
    let epilog_preheader = split_block_at(env.function, new_exit, new_exit_term);
    let header_name = env.function.block(header).name.clone().unwrap_or_default();
    env.function.set_block_name(epilog_preheader, &format!("{header_name}.epil.preheader"));

    let pre_br = env.function.terminator(preheader).expect("a terminated preheader");
    let (trip_count, be_count) = expanded(env, session, &facts, pre_br);

    let mod_val = create_trip_remainder(env, pre_br, be_count, trip_count, count);
    let be_ty = type_of(env, be_count);
    let limit = int_constant(env.context, be_ty, u128::from(count - 1));
    let branch_val = create_icmp(env, pre_br, IntPredicate::Ult, be_count, limit, "");
    // Branch to the epilog (too few iterations for the unrolled loop) or the
    // unrolled loop.
    let void = env.function.instruction(pre_br).ty;
    let guard = env
        .function
        .create_instruction(
            Opcode::Br,
            void,
            vec![branch_val, Operand::Block(epilog_preheader), Operand::Block(new_preheader)],
            Flags::default(),
            None,
        );
    env.function.insert(guard, Position::Before(pre_br)).expect("a placed branch");
    env.function.erase(pre_br).expect("the old branch has no users");

    // Clone the loop's blocks for the epilog: they run `xtraiter` times.
    let mut map = ValueMap::default();
    let mut new_blocks: Vec<BlockId> = Vec::new();
    let suffix = "epil";
    let latch_clone;
    {
        let mut latch_copy = None;
        for &block in &loop_blocks {
            let copy = env.function.clone_block(block, &mut map);
            let name = format!("{}.{suffix}", env.function.block(block).name.clone().unwrap_or_default());
            env.function.set_block_name(copy, &name);
            for inst in env.function.iter_block(block).collect::<Vec<_>>() {
                if let Some(from) = env.function.instruction(inst).result
                    && let Some(&Operand::Value(to)) = map.values.get(&from)
                    && let Some(original) = env.function.value(from).name.clone()
                {
                    env.function.set_name(to, &format!("{original}.{suffix}"));
                }
            }
            new_blocks.push(copy);
            if block == latch {
                latch_copy = Some(copy);
            }
        }
        latch_clone = latch_copy.expect("the latch is a loop block");
    }
    let header_clone = *map.blocks.get(&header).expect("the header was cloned");

    // Connect the top of the epilog to the cloned header.
    let epilog_top = env.function.terminator(epilog_preheader).expect("a terminated block");
    env.function.set_operand(epilog_top, 0, Operand::Block(header_clone));

    // The cloned latch counts `epil.iter` up to `ModVal`.
    {
        let latch_br = env.function.terminator(latch_clone).expect("a terminated latch");
        let iter_ty = type_of(env, mod_val);
        let zero = int_constant(env.context, iter_ty, 0);
        let one = int_constant(env.context, iter_ty, 1);
        let first = env.function.iter_block(header_clone).find(|&i| env.function.instruction(i).opcode != Opcode::Phi);
        let idx =
            env.function.create_instruction(Opcode::Phi, iter_ty, Vec::new(), Flags::default(), Some("epil.iter"));
        env.function.insert(idx, Position::Before(first.expect("a terminated header"))).expect("a placed block");
        let idx_value = value_of(env.function, idx);
        let next = create_binary(env, latch_br, BinaryOp::Add, idx_value, one, "epil.iter.next");
        let cmp = create_icmp(env, latch_br, IntPredicate::Ne, next, mod_val, "epil.iter.cmp");
        let latch_exit_operand = Operand::Block(latch_exit);
        let cond = env
            .function
            .create_instruction(
                Opcode::Br,
                void,
                vec![cmp, Operand::Block(header_clone), latch_exit_operand],
                Flags::default(),
                None,
            );
        env.function.insert(cond, Position::Before(latch_br)).expect("a placed branch");
        env.function.erase(latch_br).expect("the old latch branch has no users");
        env.function.set_operands(idx, vec![zero, Operand::Block(epilog_preheader), next, Operand::Block(latch_clone)]);
    }

    // The cloned header phis read the preheader's and the cloned latch's
    // values.
    for phi in phis(env.function, header_clone) {
        let operands = env.function.instruction(phi).operands.clone();
        if env.function.value(env.function.instruction(phi).result.expect("a phi")).name.as_deref() == Some("epil.iter")
        {
            continue;
        }
        let mut updated = operands.clone();
        for pair in 0..operands.len() / 2 {
            let (value, from) = (operands[2 * pair], operands[2 * pair + 1]);
            if from == Operand::Block(preheader_or_new(preheader, new_preheader))
                || from == Operand::Block(new_preheader)
            {
                updated[2 * pair + 1] = Operand::Block(epilog_preheader);
            } else if from == Operand::Block(latch) {
                updated[2 * pair + 1] = Operand::Block(latch_clone);
                if let Operand::Value(v) = value
                    && let Some(&Operand::Value(mapped)) = map.values.get(&v)
                {
                    updated[2 * pair] = Operand::Value(mapped);
                }
            }
        }
        env.function.set_operands(phi, updated);
    }

    // Insert the cloned blocks into the function before the exit.
    for &block in &new_blocks {
        env.function.insert_block_before(block, latch_exit).expect("a new block");
    }

    // Rewrite the cloned instruction operands to use the values created when
    // the clone was.
    for &block in &new_blocks {
        for inst in env.function.iter_block(block).collect::<Vec<_>>() {
            env.function.remap_instruction(inst, &map);
        }
    }

    // Connect the epilog code to the original loop and update the phis.
    connect_epilog(
        env,
        &map,
        in_loop_set_ref(&in_loop_set),
        &ConnectArgs {
            header,
            latch,
            latch_clone,
            mod_val,
            new_exit,
            exit: latch_exit,
            preheader,
            epilog_preheader,
            new_preheader,
        },
    );

    // The counter of the loop for unrolling: `niter` counts up to
    // `TripCount - ModVal`.
    {
        let new_pre_term = env.function.terminator(new_preheader).expect("a terminated block");
        let test_val = create_binary(env, new_pre_term, BinaryOp::Sub, trip_count, mod_val, "unroll_iter");
        let latch_br = env.function.terminator(latch).expect("a terminated latch");
        let iter_ty = type_of(env, test_val);
        let zero = int_constant(env.context, iter_ty, 0);
        let one = int_constant(env.context, iter_ty, 1);
        let first = env.function.iter_block(header).find(|&i| env.function.instruction(i).opcode != Opcode::Phi);
        let idx = env.function.create_instruction(Opcode::Phi, iter_ty, Vec::new(), Flags::default(), Some("niter"));
        env.function.insert(idx, Position::Before(first.expect("a terminated header"))).expect("a placed block");
        let idx_value = value_of(env.function, idx);
        let next = create_binary(env, latch_br, BinaryOp::Add, idx_value, one, "niter.next");
        let continues_on_true = env.function.instruction(latch_br).operands[1] == Operand::Block(header);
        let pred = if continues_on_true { IntPredicate::Ne } else { IntPredicate::Eq };
        let cmp = create_icmp(env, latch_br, pred, next, test_val, "niter.ncmp");
        env.function.set_operands(idx, vec![zero, Operand::Block(new_preheader), next, Operand::Block(latch)]);
        env.function.set_operand(latch_br, 0, cmp);
    }

    // For unroll factor 2 the epilog loop has one iteration.
    let mut epilog_header = Some(header_clone);
    if count == 2 {
        epilog_header = None;
        break_epilog_backedge(env, &new_blocks, header_clone, latch_clone);
    }
    Some(epilog_header)
}

/// The trip count and the backedge count as values, before the preheader's
/// branch `pre_br`.
fn expanded(
    env: &mut Env,
    session: &mut ScevSession,
    facts: &Facts,
    pre_br: InstId,
) -> (Operand, Operand) {
    let outer = env.outer;
    let (se, senv) = session.split(env.layout, &outer.globals, &outer.metadata);
    let mut expander = Expander::new("loop-unroll");
    expander.set_insert_point(pre_br);
    let mut trip_count = expander.expand_code_for(
        &senv,
        env.context,
        env.declared,
        env.function,
        se,
        facts.trip_count,
        Some(facts.trip_ty),
    );
    // Another exit may keep the latch exit from ever being taken: the count may
    // be poison, and two branches use it. Freeze it once.
    let poison = facts.abnormal_exits && {
        let cx = senv.cx(&*env.context, &*env.function);
        let query = cx.query(Some(pre_br));
        !is_constant(trip_count)
            && !llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison(&query, trip_count, 0)
    };
    let be_count = if poison {
        let ty = env.function.operand_type(env.context, trip_count).expect("a typed value");
        let freeze = env.function.create_instruction(Opcode::Freeze, ty, vec![trip_count], Flags::default(), None);
        env.function.insert(freeze, Position::Before(pre_br)).expect("a placed branch");
        trip_count = value_of(env.function, freeze);
        let all_ones = int_constant(env.context, ty, u128::MAX);
        let inst = env
            .function
            .create_instruction(Opcode::Binary(BinaryOp::Add), ty, vec![trip_count, all_ones], Flags::default(), None);
        env.function.insert(inst, Position::Before(pre_br)).expect("a placed branch");
        value_of(env.function, inst)
    } else {
        // Otherwise SCEV expands the backedge count too, for better reuse.
        expander.expand_code_for(
            &senv,
            env.context,
            env.declared,
            env.function,
            se,
            facts.be_count,
            Some(facts.trip_ty),
        )
    };
    (trip_count, be_count)
}

fn preheader_or_new(
    preheader: BlockId,
    _new: BlockId,
) -> BlockId {
    preheader
}

fn in_loop_set_ref<'a>(f: &'a dyn Fn(BlockId) -> bool) -> &'a dyn Fn(BlockId) -> bool {
    f
}

/// What `ConnectEpilog` reads.
struct ConnectArgs {
    header: BlockId,
    latch: BlockId,
    latch_clone: BlockId,
    mod_val: Operand,
    new_exit: BlockId,
    exit: BlockId,
    preheader: BlockId,
    epilog_preheader: BlockId,
    new_preheader: BlockId,
}

/// `ConnectEpilog`: the phis at the exit and in the epilog take the values the
/// unrolled loop leaves, and the unrolled loop's exit branches around the
/// epilog when no iterations are left over.
fn connect_epilog(
    env: &mut Env,
    map: &ValueMap,
    in_loop: &dyn Fn(BlockId) -> bool,
    args: &ConnectArgs,
) {
    let ConnectArgs { header, latch, latch_clone, mod_val, new_exit, exit, preheader, epilog_preheader, new_preheader } =
        *args;
    let mapped = |env: &Env, value: Operand| -> Operand {
        // A value from an instruction in the loop is its copy's.
        if let Operand::Value(v) = value
            && let ValueDef::Instruction(def) = env.function.value(v).def
            && env.function.parent(def).is_some_and(|block| in_loop(block))
            && let Some(&copy) = map.values.get(&v)
        {
            return copy;
        }
        value
    };

    // The phis at the exit: each of NewExit's is read by one in Exit.
    for pn in phis(env.function, new_exit) {
        let users = env.function.users(env.function.instruction(pn).result.expect("a phi"));
        let epilog_pn = users.iter().next().expect("The phi should have 1 use").user;
        debug_assert_eq!(env.function.parent(epilog_pn), Some(exit));
        let incoming = env
            .function
            .instruction(pn)
            .operands
            .chunks(2)
            .find(|pair| pair[1] == Operand::Block(latch))
            .map(|pair| pair[0])
            .expect("an input from the latch");
        let value = mapped(env, incoming);
        // The epilog's value, from the epilog latch.
        let mut operands = env.function.instruction(epilog_pn).operands.clone();
        operands.push(value);
        operands.push(Operand::Block(latch_clone));
        // The EpilogPreHeader input is NewExit's.
        for pair in 0..operands.len() / 2 {
            if operands[2 * pair + 1] == Operand::Block(epilog_preheader) {
                operands[2 * pair + 1] = Operand::Block(new_exit);
            }
        }
        env.function.set_operands(epilog_pn, operands);
    }

    // Phis at NewExit (from the unrolled loop's latch) and at EpilogPreHeader
    // (from PreHeader and NewExit), and the epilog loop's header phis.
    let header_phis = phis(env.function, header);
    for pn in header_phis {
        if env.function.value(env.function.instruction(pn).result.expect("a phi")).name.as_deref() == Some("niter") {
            continue;
        }
        let ty = env.function.instruction(pn).ty;
        let name =
            env.function.value(env.function.instruction(pn).result.expect("a phi")).name.clone().unwrap_or_default();
        let from_latch = env
            .function
            .instruction(pn)
            .operands
            .chunks(2)
            .find(|pair| pair[1] == Operand::Block(latch))
            .map(|pair| pair[0])
            .expect("a latch input");
        let from_pre = env
            .function
            .instruction(pn)
            .operands
            .chunks(2)
            .find(|pair| pair[1] == Operand::Block(new_preheader))
            .map(|pair| pair[0])
            .expect("a preheader input");
        // Add to the loop exit block (NewExit).
        let new_pn0 = env
            .function
            .create_instruction(
                Opcode::Phi,
                ty,
                vec![from_latch, Operand::Block(latch)],
                Flags::default(),
                Some(&format!("{name}.unr")),
            );
        let at0 = env
            .function
            .iter_block(new_exit)
            .find(|&i| env.function.instruction(i).opcode != Opcode::Phi)
            .expect("a terminated block");
        env.function.insert(new_pn0, Position::Before(at0)).expect("a placed block");
        // And to EpilogPreHeader.
        let new_pn1 = env
            .function
            .create_instruction(
                Opcode::Phi,
                ty,
                vec![from_pre, Operand::Block(preheader), value_of(env.function, new_pn0), Operand::Block(new_exit)],
                Flags::default(),
                Some(&format!("{name}.epil.init")),
            );
        let at1 = env
            .function
            .iter_block(epilog_preheader)
            .find(|&i| env.function.instruction(i).opcode != Opcode::Phi)
            .expect("a terminated block");
        env.function.insert(new_pn1, Position::Before(at1)).expect("a placed block");
        // The epilog's copy of the phi reads it from EpilogPreHeader.
        let original = env.function.instruction(pn).result.expect("a phi");
        let Some(&Operand::Value(copy)) = map.values.get(&original) else { continue };
        let ValueDef::Instruction(vpn) = env.function.value(copy).def else { continue };
        let mut operands = env.function.instruction(vpn).operands.clone();
        for pair in 0..operands.len() / 2 {
            if operands[2 * pair + 1] == Operand::Block(epilog_preheader) {
                operands[2 * pair] = value_of(env.function, new_pn1);
            }
        }
        env.function.set_operands(vpn, operands);
    }

    // In NewExit, branch around the epilog loop if there are no extra
    // iterations.
    let new_exit_term = env.function.terminator(new_exit).expect("a terminated block");
    let mod_ty = type_of(env, mod_val);
    let zero = int_constant(env.context, mod_ty, 0);
    let not_null = create_icmp(env, new_exit_term, IntPredicate::Ne, mod_val, zero, "lcmp.mod");
    // Split the epilog exit to keep loop canonical form.
    let preds: Vec<BlockId> = env.function.edge_predecessors(exit);
    let epilog_lcssa = split_block_predecessors_of(env.function, exit, &preds, true);
    env.function.set_block_name(
        epilog_lcssa,
        &format!("{}.epilog-lcssa", env.function.block(exit).name.clone().unwrap_or_default()),
    );
    let void = env.function.instruction(new_exit_term).ty;
    let guard = env
        .function
        .create_instruction(
            Opcode::Br,
            void,
            vec![not_null, Operand::Block(epilog_preheader), Operand::Block(exit)],
            Flags::default(),
            None,
        );
    env.function.insert(guard, Position::Before(new_exit_term)).expect("a placed branch");
    env.function.erase(new_exit_term).expect("the old branch has no users");
    // In EpilogPreHeader, assume the extra iterations are not zero: the
    // assumption lets SCEV bound the epilog's trip count.
    let at = env
        .function
        .iter_block(epilog_preheader)
        .find(|&i| env.function.instruction(i).opcode != Opcode::Phi)
        .expect("a terminated block");
    let not_null2 = create_icmp(env, at, IntPredicate::Ne, mod_val, zero, "lcmp.mod");
    // `Builder.CreateAssumption(cond)`: a call of `llvm.assume`.
    let void_ty = env.context.types.void();
    let bool_ty = env.context.types.int(1);
    let function_type = env
        .context
        .types
        .intern(
            llrm_mir::types::Type::Function {
                returns: void_ty,
                parameters: vec![bool_ty],
                variadic: false,
            },
        );
    let callee = env.declared.declare("llvm.assume", function_type);
    let pointer = env.context.types.ptr(0);
    let target = Operand::Constant(env.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
    let info = llrm_mir::opcode::CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new()],
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let call = env
        .function
        .create_instruction(Opcode::Call(Box::new(info)), void_ty, vec![not_null2, target], Flags::default(), None);
    env.function.insert(call, Position::Before(at)).expect("a placed call");
}

/// The epilog of a count of two runs once: its backedge goes, its values
/// simplify, and its latch joins its exit.
fn break_epilog_backedge(
    env: &mut Env,
    blocks: &[BlockId],
    header: BlockId,
    latch: BlockId,
) {
    // `breakLoopBackedge`: the conditional latch becomes a jump to the exit.
    let term = env.function.terminator(latch).expect("a terminated latch");
    let operands = env.function.instruction(term).operands.clone();
    let (Operand::Block(first), Operand::Block(second)) = (operands[1], operands[2]) else {
        unreachable!("a conditional branch names blocks")
    };
    let exit = if first == header { second } else { first };
    env.function.predecessor_removed(&*env.context, header, latch, true);
    let void = env.function.instruction(term).ty;
    let jump = env.function.create_instruction(Opcode::Br, void, vec![Operand::Block(exit)], Flags::default(), None);
    env.function.insert(jump, Position::Before(term)).expect("a placed branch");
    env.function.erase(term).expect("the branch has no users");

    // Simplify the values after breaking the backedge.
    let tree = llrm_mir::domtree::DomTree::of(env.function);
    let loops = llrm_mir::loopinfo::LoopInfo::analyze(env.function, || &tree);
    for &block in blocks {
        let mut dead: Vec<InstId> = Vec::new();
        for inst in env.function.iter_block(block).collect::<Vec<_>>() {
            if env.function.is_erased(inst) {
                continue;
            }
            if let Some(value) = llrm_mir::simplify::simplify_instruction_in(
                env.context,
                &env.outer.globals,
                env.layout,
                None,
                env.function,
                inst,
            ) && crate::looprotate::replacement_preserves_lcssa_form(&loops, env.function, inst, value)
                && let Some(result) = env.function.instruction(inst).result
            {
                env.function.replace_all_uses_with(result, value);
            }
            let instruction = env.function.instruction(inst);
            if !instruction.opcode.is_terminator()
                && instruction.result.is_none_or(|result| env.function.users(result).is_empty())
                && !matches!(
                    instruction.opcode,
                    Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_) | Opcode::LandingPad { .. }
                )
            {
                dead.push(inst);
            }
        }
        // Deleted when the block is done: a phi may use a later instruction.
        while let Some(inst) = dead.pop() {
            if env.function.is_erased(inst) {
                continue;
            }
            let instruction = env.function.instruction(inst);
            if instruction.result.is_some_and(|result| !env.function.users(result).is_empty()) {
                continue;
            }
            let operands = instruction.operands.clone();
            env.function.erase(inst).expect("a dead instruction");
            for operand in operands {
                if let Operand::Value(v) = operand
                    && let ValueDef::Instruction(def) = env.function.value(v).def
                    && !env.function.instruction(def).opcode.is_terminator()
                    && !matches!(
                        env.function.instruction(def).opcode,
                        Opcode::Store { .. }
                            | Opcode::Call(_)
                            | Opcode::Invoke(_)
                            | Opcode::LandingPad { .. }
                            | Opcode::Phi
                    )
                {
                    dead.push(def);
                }
            }
        }
    }

    // Merge the latch into its exit.
    let successors = env.function.successors(latch);
    if let [exit_block] = successors[..] {
        let _ = crate::local::merge_block_into_predecessor(&*env.context, env.function, exit_block, false);
    }
    let _ = retarget;
}
