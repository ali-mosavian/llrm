//! `recognizePopcount` of `LoopIdiomRecognize.cpp` at `d1106deb71cc`: a loop
//! that clears the lowest set bit until none is left becomes `ctpop`.
//!
//! # How it works
//! [`LoopIdiomRecognize::recognize_popcount`] takes a one-block loop under a
//! precondition `if (x != 0)`, whose body holds `x2 = x1 & (x1 - 1)` and a
//! counter `cnt2 = cnt1 + 1` that is read after the loop. It puts
//! `ctpop(x)` in the precondition block, makes the precondition test the count,
//! and counts the loop down from it, so the loop has a known trip count and
//! goes dead where it counts nothing else. The counter's uses after the loop
//! read the count.
//!
//! # Deviations
//! - The pin asks `TargetTransformInfo::getPopcntSupport(32)`: here it is
//!   [`Tti::popcnt_support_fast`], and a compile without a `Tti` forms none.
//!
//! # Example
//! ```text
//! pre:   br (x != 0), ph, exit            pre:   %pop = ctpop x
//! ph:    br loop                                 br (%pop != 0), ph, exit
//! loop:  %x1 = phi [x, ph], [%x2, loop]   loop:  %tc = phi [%pop, ph], [%dec, loop]
//!        %c1 = phi [0, ph], [%c2, loop]          ...
//!        %c2 = add %c1, 1                        %dec = sub nsw %tc, 1
//!        %m = add %x1, -1                        br (%dec u> 0), loop, exit
//!        %x2 = and %x1, %m                exit:  (uses of %c2 read %pop)
//!        br (%x2 != 0), loop, exit
//! ```

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};

use super::LoopIdiomRecognize;
use super::common::{
    defining_instruction, delete_dead_cascade, emit, get_recurrence_var, int, match_condition, uses_outside,
    zext_or_trunc,
};
use super::countdown::count_down_loop;
use super::process::{Run, cx};

/// What `detectPopcountIdiom` found.
struct Popcount {
    /// The `cnt2 = cnt1 + 1` that is read after the loop.
    count_inst: InstId,
    /// Its phi, `cnt1`.
    count_phi: InstId,
    /// The value whose bits are counted, `x0`.
    var: Operand,
}

impl LoopIdiomRecognize {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2855
    /// `recognizePopcount`.
    pub(super) fn recognize_popcount(
        &mut self,
        run: &mut Run,
    ) -> bool {
        if !run.tti.is_some_and(|tti| tti.popcnt_support_fast(32)) {
            return false;
        }

        // Counting population are usually conducted by few arithmetic
        // instructions. Such instructions can be easily "absorbed" by vacant
        // slots in a non-compact loop. Therefore, recognizing popcount idiom
        // only makes sense in a compact loop.

        // Give up if the loop has multiple blocks or multiple backedges.
        if run.cur_loop.get_num_back_edges(&*run.function) != 1 || run.cur_loop.get_num_blocks() != 1 {
            return false;
        }

        let body = run.cur_loop.get_blocks()[0];
        if run.function.iter_block(body).count() >= 20 {
            // The loop is too big, bail out.
            return false;
        }

        let Some(pre_cond_block) = precondition_block(run) else { return false };
        let Some(found) = detect_popcount_idiom(run, pre_cond_block) else { return false };
        transform_loop_to_popcount(run, pre_cond_block, &found);
        true
    }
}

/// The block before the loop, where `ctpop` goes: the preheader holds only an
/// unconditional branch, and its one predecessor ends in a conditional one.
fn precondition_block(run: &Run) -> Option<llrm_mir::module::BlockId> {
    // It should have a preheader containing nothing but an unconditional
    // branch.
    let preheader = run.cur_loop.get_loop_preheader(run.function)?;
    let mut entry = run.function.iter_block(preheader);
    let (Some(first), None) = (entry.next(), entry.next()) else { return None };
    let entry_branch = run.function.instruction(first);
    if entry_branch.opcode != Opcode::Br || entry_branch.operands.len() != 1 {
        return None;
    }

    // It should have a precondition block where the generated popcount
    // intrinsic function can be inserted.
    let mut predecessors = run
        .function
        .block_users(preheader)
        .into_iter()
        .filter(|one| run.function.instruction(one.user).opcode.is_terminator());
    let (Some(only), None) = (predecessors.next(), predecessors.next()) else { return None };
    let pre_cond_block = run.function.parent(only.user)?;
    let pre_cond_branch = run.function.instruction(only.user);
    (pre_cond_branch.opcode == Opcode::Br && pre_cond_branch.operands.len() == 3).then_some(pre_cond_block)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2477
/// `detectPopcountIdiom`.
fn detect_popcount_idiom(
    run: &Run,
    pre_cond_block: llrm_mir::module::BlockId,
) -> Option<Popcount> {
    let function = &*run.function;
    let context = &*run.context;
    let loop_entry = run.cur_loop.get_blocks()[0];

    // step 1: Check if the loop-back branch is in desirable form.
    let loop_term = function.terminator(loop_entry)?;
    if function.instruction(loop_term).operands.len() != 3 {
        return None;
    }
    let def_x2 = defining_instruction(function, match_condition(context, function, loop_term, loop_entry, false)?)?;

    // step 2: detect instructions corresponding to "x2 = x1 & (x1 - 1)"
    if function.instruction(def_x2).opcode != Opcode::Binary(BinaryOp::And) {
        return None;
    }
    let binary = |operand: Operand| {
        defining_instruction(function, operand)
            .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Binary(_)))
    };
    let [and_0, and_1] = function.instruction(def_x2).operands[..] else { return None };
    let (var_x1, sub_one) = match binary(and_0) {
        Some(sub_one) => (and_1, sub_one),
        None => (and_0, binary(and_1)?),
    };
    let [base, Operand::Constant(dec)] = function.instruction(sub_one).operands[..] else { return None };
    if base != var_x1 {
        return None;
    }
    let ConstantKind::Int(dec_bits) = context.get(dec).kind else { return None };
    let width = context.types.int_bits(context.get(dec).ty)?;
    let minus_one = llrm_mir::context::mask(width);
    match function.instruction(sub_one).opcode {
        Opcode::Binary(BinaryOp::Sub) if dec_bits == 1 => {}
        Opcode::Binary(BinaryOp::Add) if dec_bits == minus_one => {}
        _ => return None,
    }

    // step 3: Check the recurrence of variable X
    let phi_x = get_recurrence_var(function, var_x1, def_x2, loop_entry)?;

    // step 4: Find the instruction which count the population: cnt2 = cnt1 + 1
    let one = |operand: Operand| match operand {
        Operand::Constant(constant) => context.get(constant).kind == ConstantKind::Int(1),
        _ => false,
    };
    let (count_inst, count_phi) = function.iter_block(loop_entry).find_map(|inst| {
        let instruction = function.instruction(inst);
        if instruction.opcode != Opcode::Binary(BinaryOp::Add) || !one(instruction.operands[1]) {
            return None;
        }
        let phi = get_recurrence_var(function, instruction.operands[0], inst, loop_entry)?;
        // Check if the result of the instruction is live of the loop.
        let result = instruction.result?;
        let live_out_loop = function.users(result).into_iter().any(|one| function.parent(one.user) != Some(loop_entry));
        live_out_loop.then_some((inst, phi))
    })?;

    // step 5: check if the precondition is in this form:
    //   "if (x != 0) goto loop-head ; else goto somewhere-we-don't-care;"
    let preheader = run.cur_loop.get_loop_preheader(function)?;
    let pre_cond_term = function.terminator(pre_cond_block)?;
    let var = match_condition(context, function, pre_cond_term, preheader, false)?;
    let phi_x = &function.instruction(phi_x).operands;
    if var != phi_x[0] && var != phi_x[2] {
        return None;
    }
    Some(Popcount { count_inst, count_phi, var })
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L3043
/// `transformLoopToPopcount`: `ctpop` in the precondition block, which tests
/// the count, and the loop counting down from it.
fn transform_loop_to_popcount(
    run: &mut Run,
    pre_cond_block: llrm_mir::module::BlockId,
    found: &Popcount,
) {
    let preheader = run.cur_loop.get_loop_preheader(run.function).expect("detected with a preheader");
    let body = run.cur_loop.get_blocks()[0];
    let pre_cond_br = run.function.terminator(pre_cond_block).expect("a precondition branch");
    let count_ty = run.function.value(run.function.instruction(found.count_phi).result.expect("a phi value")).ty;
    let var_ty = run.function.operand_type(run.context, found.var).expect("a typed value");
    let i1 = run.context.types.int(1);

    // Assuming before transformation, the loop is following:
    //  if (x) // the precondition
    //     do { cnt++; x &= x - 1; } while(x);

    // Step 1: Insert the ctpop instruction at the end of the precondition
    // block
    let pop_cnt = crate::switchopt::intrinsic_call(
        run.context,
        run.declared,
        run.function,
        pre_cond_br,
        "ctpop",
        var_ty,
        vec![found.var],
    );
    let pop_cnt_zext = zext_or_trunc(run.context, run.function, pre_cond_br, pop_cnt, count_ty, "");
    // TripCnt is exactly the number of iterations the loop has
    let trip_cnt = pop_cnt_zext;
    let mut new_count = pop_cnt_zext;

    // If the population counter's initial value is not zero, insert Add Inst.
    let count_phi = &run.function.instruction(found.count_phi).operands;
    let init = count_phi.chunks(2).find(|pair| pair[1] == Operand::Block(preheader)).expect("a preheader input")[0];
    let init_is_zero = matches!(
        init,
        Operand::Constant(constant) if run.context.get(constant).kind == ConstantKind::Int(0)
    );
    if !init_is_zero {
        new_count = emit(
            run.function,
            pre_cond_br,
            Opcode::Binary(BinaryOp::Add),
            count_ty,
            vec![new_count, init],
            Flags::default(),
            "",
        );
    }

    // Step 2: Replace the precondition from "if (x == 0) goto loop-exit" to
    //   "if (NewCount == 0) loop-exit". Without this change, the intrinsic
    //   function would be partial dead code, and downstream passes will drag
    //   it back from the precondition block to the preheader.
    let pre_cond = defining_instruction(run.function, run.function.instruction(pre_cond_br).operands[0])
        .expect("matched as a compare");
    let Opcode::ICmp(predicate) = run.function.instruction(pre_cond).opcode else { unreachable!("a compare") };
    // `matchCondition` reads the variable from the first operand.
    debug_assert_eq!(run.function.instruction(pre_cond).operands[0], found.var);
    let zero = int(run.context, count_ty, 0);
    let new_pre_cond =
        emit(run.function, pre_cond_br, Opcode::ICmp(predicate), i1, vec![pop_cnt_zext, zero], Flags::default(), "");
    run.function.set_operand(pre_cond_br, 0, new_pre_cond);
    delete_dead_cascade(run.function, pre_cond);

    // Step 3: Note that the population count is exactly the trip count of the
    // loop in question, which enable us to convert the loop from noncountable
    // loop into a countable one. The benefit is twofold:
    //
    //  - If the loop only counts population, the entire loop becomes dead after
    //    the transformation. It is a lot easier to prove a countable loop dead
    //    than to prove a noncountable one. (In some C dialects, an infinite
    //    loop isn't dead even if it computes nothing useful. In general, DCE
    //    needs to prove a noncountable loop finite before safely delete it.)
    //
    //  - If the loop also performs something else, it remains alive. Since it
    //    is transformed to countable form, it can be aggressively optimized by
    //    some optimizations which are in general not applicable to a
    //    noncountable loop.
    //
    // After this step, this loop (conceptually) would look like following:
    //   newcnt = __builtin_ctpop(x);
    //   t = newcnt;
    //   if (x)
    //     do { cnt++; x &= x-1; t--) } while (t > 0);
    count_down_loop(run, preheader, body, trip_cnt, IntPredicate::Ugt, IntPredicate::Sle);

    // Step 4: All the references to the original population counter outside
    //  the loop are replaced with the NewCount -- the value returned from
    //  __builtin_ctpop().
    let count = run.function.instruction(found.count_inst).result.expect("an add value");
    for (user, index) in uses_outside(run.function, count, body) {
        run.function.set_operand(user, index, new_count);
    }

    // step 5: Forget the "non-computable" trip-count SCEV associated with the
    //   loop. The loop would otherwise not be deleted even if it becomes empty.
    let cx = cx!(run);
    run.se.forget_loop(&cx, run.cur_loop.get_header());
}

#[cfg(test)]
mod tests {
    use crate::loopidiom::pass::LoopIdiom;
    use crate::testing::{parsed, printed};

    /// `for (; x; x &= x - 1) n++` after rotation.
    const ONES: &str = "define i32 @ones(i32 %x) {
entry:
  %zero = icmp eq i32 %x, 0
  br i1 %zero, label %end, label %ph

ph:
  br label %loop

loop:
  %x1 = phi i32 [ %x, %ph ], [ %x2, %loop ]
  %n1 = phi i32 [ 0, %ph ], [ %n2, %loop ]
  %n2 = add nuw nsw i32 %n1, 1
  %m = add i32 %x1, -1
  %x2 = and i32 %x1, %m
  %done = icmp eq i32 %x2, 0
  br i1 %done, label %exit, label %loop

exit:
  br label %end

end:
  %n = phi i32 [ 0, %entry ], [ %n2, %exit ]
  ret i32 %n
}
";

    /// A target whose `Tti` does not say `ctpop` is fast (the base class:
    /// software) keeps the loop.
    #[test]
    fn a_target_without_a_fast_popcnt_keeps_the_loop() {
        let mut module = parsed(ONES);
        let mut passes = llrm_mir::passes::PassManager::default();
        passes.add(LoopIdiom::default());
        passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
        let after = printed(&module);
        assert!(!after.contains("ctpop"), "{after}");
        assert!(after.contains("%x2 = and i32 %x1, %m"), "{after}");
    }
}
