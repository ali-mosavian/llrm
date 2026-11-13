//! `recognizeAndInsertFFS` of `LoopIdiomRecognize.cpp` at `d1106deb71cc`: a
//! loop that shifts a value by one until it is zero becomes a count down from
//! `ctlz` (a right shift) or `cttz` (a left shift) of the start value.
//!
//! # How it works
//! [`LoopIdiomRecognize::recognize_and_insert_ffs`] takes a one-block loop
//! that ends on `x.next != 0` for `x.next = x >> 1` (or `x << 1`) and has a
//! counter stepping by one. The trip count is `BitWidth - ctlz(x)`; the loop
//! counts down from it, so it goes dead where it does nothing else, and the
//! counter's uses after the loop read the count. Where the counter's phi is
//! read after the loop the count is that of `x >> 1`, plus one; where it is
//! not, `x` must be known non-zero (a precondition tests it) so that a zero
//! start does not run once.
//!
//! # Deviations
//! - D18: formed only where the target's `Tti` says `shift_loop_counts` (see
//!   `run_on_noncountable_loop`); the pin forms it everywhere.
//! - The pin prices the intrinsic with `getIntrinsicInstrCost` at
//!   `TCK_SizeAndLatency` against `TCC_Basic`; here that is [`Tti::tcc_basic`]
//!   (the pin's constant 1 on a target that says nothing), and a compile
//!   without a `Tti` forms none.
//!
//! # Example
//! ```text
//! loop:  %x = phi [%x0, ph], [%x.next, loop]    ph:   %lz = ctlz %x0, true
//!        %n = phi [%n0, ph], [%n.next, loop]          %c = sub 32, %lz
//!        %n.next = add %n, 1                   loop: %tc = phi [%c, ph], [%dec, loop]
//!        %x.next = lshr %x, 1                        ...
//!        br (%x.next != 0), loop, exit               %dec = sub nsw %tc, 1
//!                                                    br (%dec != 0), loop, exit
//! ```

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::tti::CostKind;
use llrm_mir::valuetracking::{Query, known, nonzero};

use super::LoopIdiomRecognize;
use super::common::{
    defining_instruction, emit, get_recurrence_var, int, match_condition, uses_outside, zext_or_trunc,
};
use super::countdown::count_down_loop;
use super::process::{Run, cx};

/// `ctlz` for a right shift, `cttz` for a left one.
#[derive(Clone, Copy, PartialEq)]
enum Direction {
    Ctlz,
    Cttz,
}

impl Direction {
    fn name(self) -> &'static str {
        match self {
            Direction::Ctlz => "ctlz",
            Direction::Cttz => "cttz",
        }
    }
}

/// What `detectShiftUntilZeroIdiom` found.
struct ShiftUntilZero {
    direction: Direction,
    /// `x0`, the value the shift starts from.
    init_x: Operand,
    /// `cnt.next = cnt + 1` (or `- 1`).
    count_inst: InstId,
    /// Its phi.
    count_phi: InstId,
    /// `x.next = x >> 1` (or `x << 1`).
    def_x: InstId,
}

impl LoopIdiomRecognize {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2767
    /// `recognizeAndInsertFFS`.
    pub(super) fn recognize_and_insert_ffs(
        &mut self,
        run: &mut Run,
    ) -> bool {
        // Give up if the loop has multiple blocks or multiple backedges.
        if run.cur_loop.get_num_back_edges(&*run.function) != 1 || run.cur_loop.get_num_blocks() != 1 {
            return false;
        }
        let Some(found) = detect_shift_until_zero_idiom(run) else { return false };
        insert_ffs_if_profitable(run, &found)
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2785
impl LoopIdiomRecognize {
    /// `recognizeShiftUntilLessThan`.
    pub(super) fn recognize_shift_until_less_than(
        &mut self,
        run: &mut Run,
    ) -> bool {
        // Give up if the loop has multiple blocks or multiple backedges.
        if run.cur_loop.get_num_back_edges(&*run.function) != 1 || run.cur_loop.get_num_blocks() != 1 {
            return false;
        }
        let Some((found, threshold)) = detect_shift_until_less_than_idiom(run) else { return false };

        if threshold == 2 {
            // Treat as regular FFS.
            return insert_ffs_if_profitable(run, &found);
        }

        // Look for Floor Log2 Idiom.
        if threshold != 4 {
            return false;
        }

        // Abort if CntPhi is used outside of the loop.
        let body = run.cur_loop.get_blocks()[0];
        let phi_result = run.function.instruction(found.count_phi).result.expect("a phi value");
        if run.function.users(phi_result).into_iter().any(|one| run.function.parent(one.user) != Some(body)) {
            return false;
        }

        // It is safe to assume Preheader exist as it was checked in parent
        // function RunOnLoop.
        let preheader = run.cur_loop.get_loop_preheader(run.function).expect("a preheader");
        let Some(pre_cond) = single_predecessor(run, preheader) else { return false };
        let Some(pre_cond_br) = run.function.terminator(pre_cond).filter(|&term| {
            run.function.instruction(term).opcode == Opcode::Br && run.function.instruction(term).operands.len() == 3
        }) else {
            return false;
        };
        let Some((tested, pre_loop_threshold)) = match_shift_ult_condition(run, pre_cond_br, preheader) else {
            return false;
        };
        if tested != found.init_x || pre_loop_threshold != 2 {
            return false;
        }

        let zero_check = true;

        // the loop has only 6 instructions:
        //  %n.addr.0 = phi [ %n, %entry ], [ %shr, %while.cond ]
        //  %i.0 = phi [ %i0, %entry ], [ %inc, %while.cond ]
        //  %shr = ashr %n.addr.0, 1
        //  %tobool = icmp ult %n.addr.0, C
        //  %inc = add nsw %i.0, 1
        //  br i1 %tobool
        const IDIOM_CANONICAL_SIZE: usize = 6;
        if !is_profitable_to_insert_ffs(run, found.direction, found.init_x, zero_check, IDIOM_CANONICAL_SIZE) {
            return false;
        }

        // log2(x) = w - 1 - clz(x)
        transform_loop_to_countable(run, preheader, &found, zero_check, false, true);
        true
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2319
/// `matchShiftULTCondition`: the variable `terminator` compares `u<` a
/// constant when the comparison being false yields to `loop_entry`, and the
/// constant.
fn match_shift_ult_condition(
    run: &Run,
    terminator: InstId,
    loop_entry: BlockId,
) -> Option<(Operand, u128)> {
    let function = &*run.function;
    let [condition, _, Operand::Block(false_successor)] = function.instruction(terminator).operands[..] else {
        return None;
    };
    let compare = defining_instruction(function, condition)?;
    let Opcode::ICmp(predicate) = function.instruction(compare).opcode else { return None };
    let [tested, Operand::Constant(threshold)] = function.instruction(compare).operands[..] else { return None };
    let ConstantKind::Int(threshold) = run.context.get(threshold).kind else { return None };
    (predicate == IntPredicate::Ult && false_successor == loop_entry).then_some((tested, threshold))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2378
/// `detectShiftUntilLessThanIdiom`: the loop and the threshold it compares
/// against.
fn detect_shift_until_less_than_idiom(run: &Run) -> Option<(ShiftUntilZero, u128)> {
    let function = &*run.function;
    let context = &*run.context;
    let loop_entry = run.cur_loop.get_blocks()[0];

    // step 1: Check if the loop-back branch is in desirable form.
    let entry_br = function.terminator(loop_entry)?;
    if function.instruction(entry_br).operands.len() != 3 {
        return None;
    }
    let (tested, threshold) = match_shift_ult_condition(run, entry_br, loop_entry)?;

    // step 2: Check the recurrence of variable X
    let var_phi =
        defining_instruction(function, tested).filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)?;
    let back_edge = incoming_value(function, var_phi, loop_entry)?;
    let def_x = defining_instruction(function, back_edge)?;
    let var_phi_value = Operand::Value(function.instruction(var_phi).result?);
    if function.instruction(def_x).operands.first() != Some(&var_phi_value) {
        return None;
    }

    // step 3: detect instructions corresponding to "x.next = x >> 1"
    if function.instruction(def_x).opcode != Opcode::Binary(BinaryOp::LShr) {
        return None;
    }
    let [_, Operand::Constant(amount)] = function.instruction(def_x).operands[..] else { return None };
    if context.get(amount).kind != ConstantKind::Int(1) {
        return None;
    }
    let preheader = run.cur_loop.get_loop_preheader(function)?;
    let init_x = incoming_value(function, var_phi, preheader)?;

    // step 4: Find the instruction which count the CTLZ.
    let (count_inst, count_phi) = find_counter(function, context, loop_entry)?;
    Some((ShiftUntilZero { direction: Direction::Ctlz, init_x, count_inst, count_phi, def_x }, threshold))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2611
/// `detectShiftUntilZeroIdiom`.
fn detect_shift_until_zero_idiom(run: &Run) -> Option<ShiftUntilZero> {
    let function = &*run.function;
    let context = &*run.context;
    let loop_entry = run.cur_loop.get_blocks()[0];

    // step 1: Check if the loop-back branch is in desirable form.
    let loop_term = function.terminator(loop_entry)?;
    if function.instruction(loop_term).operands.len() != 3 {
        return None;
    }
    let def_x = defining_instruction(function, match_condition(context, function, loop_term, loop_entry, false)?)?;

    // step 2: detect instructions corresponding to "x.next = x >> 1 or x << 1"
    let Opcode::Binary(shift @ (BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)) = function.instruction(def_x).opcode
    else {
        return None;
    };
    let direction = if shift == BinaryOp::Shl { Direction::Cttz } else { Direction::Ctlz };
    let [var_x, Operand::Constant(amount)] = function.instruction(def_x).operands[..] else { return None };
    if context.get(amount).kind != ConstantKind::Int(1) {
        return None;
    }

    // step 3: Check the recurrence of variable X
    let phi_x = get_recurrence_var(function, var_x, def_x, loop_entry)?;
    let preheader = run.cur_loop.get_loop_preheader(function)?;
    let init_x = incoming_value(function, phi_x, preheader)?;

    // Make sure the initial value can't be negative otherwise the ashr in the
    // loop might never reach zero which would make the loop infinite.
    if shift == BinaryOp::AShr && !known::is_known_non_negative(&query(run, loop_term), init_x, 0) {
        return None;
    }

    let (count_inst, count_phi) = find_counter(function, context, loop_entry)?;
    Some(ShiftUntilZero { direction, init_x, count_inst, count_phi, def_x })
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2425
/// Step 4 of the shift detectors: find the instruction which count the CTLZ:
/// cnt.next = cnt + 1 or cnt.next = cnt + -1.
// TODO (LLVM): We can skip the step. If loop trip count is known (CTLZ), then
// all uses of "cnt.next" could be optimized to the trip count plus "cnt0".
// Currently it is not optimized. This step could be used to detect POPCNT
// instruction: cnt.next = cnt + (x.next & 1)
fn find_counter(
    function: &llrm_mir::module::Function,
    context: &llrm_mir::context::Context,
    loop_entry: BlockId,
) -> Option<(InstId, InstId)> {
    function
        .iter_block(loop_entry)
        .find_map(
            |inst| {
                let instruction = function.instruction(inst);
                if instruction.opcode != Opcode::Binary(BinaryOp::Add) {
                    return None;
                }
                let Operand::Constant(step) = instruction.operands[1] else { return None };
                let ConstantKind::Int(bits) = context.get(step).kind else { return None };
                let width = context.types.int_bits(context.get(step).ty)?;
                if bits != 1 && bits != llrm_mir::context::mask(width) {
                    return None;
                }
                Some((inst, get_recurrence_var(function, instruction.operands[0], inst, loop_entry)?))
            },
        )
}

/// `getIncomingValueForBlock`.
pub(super) fn incoming_value(
    function: &llrm_mir::module::Function,
    phi: InstId,
    block: BlockId,
) -> Option<Operand> {
    function.instruction(phi).operands.chunks(2).find(|pair| pair[1] == Operand::Block(block)).map(|pair| pair[0])
}

/// A `Query` asked at `at`, with the dominator tree and assumptions.
pub(super) fn query<'a>(
    run: &'a Run,
    at: InstId,
) -> Query<'a> {
    Query {
        context: &*run.context,
        globals: run.env.globals,
        metadata: run.env.metadata,
        function: &*run.function,
        layout: run.env.layout,
        dt: Some(run.env.tree),
        assumptions: Some(run.env.assumptions),
        dom_conditions: None,
        cond_context: None,
        cxt: Some(at),
        pending_intrinsics: &[],
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2683
/// `isProfitableToInsertFFS`: check if CTLZ / CTTZ intrinsic is profitable.
/// Assume it is always profitable if we delete the loop.
fn is_profitable_to_insert_ffs(
    run: &Run,
    direction: Direction,
    init_x: Operand,
    zero_check: bool,
    canonical_size: usize,
) -> bool {
    let Some(tti) = run.tti else { return false };
    let ty = run.function.operand_type(run.context, init_x).expect("a typed value");
    let width = run.context.types.int_bits(ty).expect("an integer");
    let header_size = run.function.iter_block(run.cur_loop.get_header()).count();
    let name = format!("llvm.{}.i{width}", direction.name());
    let types = [llrm_mir::tti::Ty::Int(width), llrm_mir::tti::Ty::Int(1)];
    let ret = Some(llrm_mir::tti::Ty::Int(width));
    let cost = match zero_check {
        true => tti.zero_poison_intrinsic_cost(&name, ret, &types, CostKind::SizeAndLatency),
        false => tti.intrinsic_cost(&name, ret, &types, CostKind::SizeAndLatency),
    };
    let basic = tti.tcc_basic(CostKind::SizeAndLatency);
    !(header_size != canonical_size && (!cost.valid || cost.value > basic.value))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2703
/// `insertFFSIfProfitable`: convert CTLZ / CTTZ idiom loop into countable
/// loop. Whether CTLZ / CTTZ was inserted as a new trip count.
fn insert_ffs_if_profitable(
    run: &mut Run,
    found: &ShiftUntilZero,
) -> bool {
    let body = run.cur_loop.get_blocks()[0];
    let used_outside = |run: &Run, inst: InstId| {
        let result = run.function.instruction(inst).result.expect("a value");
        run.function.users(result).into_iter().any(|one| run.function.parent(one.user) != Some(body))
    };
    let is_cnt_phi_used_outside_loop = used_outside(run, found.count_phi);
    let is_cnt_inst_used_outside_loop = used_outside(run, found.count_inst);
    // If both CntInst and CntPhi are used outside the loop the profitability
    // is questionable.
    if is_cnt_inst_used_outside_loop && is_cnt_phi_used_outside_loop {
        return false;
    }

    // For some CPUs result of CTLZ(X) intrinsic is undefined when X is 0. If
    // we can not guarantee X != 0, we need to check this when expand.
    let mut zero_check = false;
    // It is safe to assume Preheader exist as it was checked in parent
    // function RunOnLoop.
    let preheader = run.cur_loop.get_loop_preheader(run.function).expect("a preheader");

    // If we are using the count instruction outside the loop, make sure we
    // have a zero check as a precondition. Without the check the loop would
    // run one iteration for before any check of the input value. This means 0
    // and 1 would have identical behavior in the original loop and thus
    if !is_cnt_phi_used_outside_loop {
        let tested = single_predecessor(run, preheader)
            .and_then(|pre_cond| run.function.terminator(pre_cond))
            .filter(|&term| {
                run.function.instruction(term).opcode == Opcode::Br
                    && run.function.instruction(term).operands.len() == 3
            })
            .and_then(|term| match_condition(run.context, run.function, term, preheader, false));
        let at = run.function.terminator(preheader).expect("a preheader ends");
        if tested != Some(found.init_x) && !nonzero::is_known_non_zero(&query(run, at), found.init_x, 0) {
            return false;
        }
        zero_check = true;
    }

    // FFS idiom loop has only 6 instructions:
    //  %n.addr.0 = phi [ %n, %entry ], [ %shr, %while.cond ]
    //  %i.0 = phi [ %i0, %entry ], [ %inc, %while.cond ]
    //  %shr = ashr %n.addr.0, 1
    //  %tobool = icmp eq %shr, 0
    //  %inc = add nsw %i.0, 1
    //  br i1 %tobool
    const IDIOM_CANONICAL_SIZE: usize = 6;
    if !is_profitable_to_insert_ffs(run, found.direction, found.init_x, zero_check, IDIOM_CANONICAL_SIZE) {
        return false;
    }

    transform_loop_to_countable(run, preheader, found, zero_check, is_cnt_phi_used_outside_loop, false);
    true
}

/// `getSinglePredecessor`: the one block whose terminator names `block`.
fn single_predecessor(
    run: &Run,
    block: BlockId,
) -> Option<BlockId> {
    let mut predecessors = run
        .function
        .block_users(block)
        .into_iter()
        .filter(|one| run.function.instruction(one.user).opcode.is_terminator());
    let (Some(only), None) = (predecessors.next(), predecessors.next()) else { return None };
    run.function.parent(only.user)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2951
/// `transformLoopToCountable`: the count of `ctlz` / `cttz` in the preheader,
/// the loop counting down from it, and the counter's uses after the loop
/// reading the count.
fn transform_loop_to_countable(
    run: &mut Run,
    preheader: BlockId,
    found: &ShiftUntilZero,
    zero_check: bool,
    is_cnt_phi_used_outside_loop: bool,
    insert_sub: bool,
) {
    let body = run.cur_loop.get_blocks()[0];
    // Step 1: Insert the CTLZ/CTTZ instruction at the end of the preheader
    // block
    let at = run.function.terminator(preheader).expect("a preheader ends");
    let ty = run.function.operand_type(run.context, found.init_x).expect("a typed value");
    let width = run.context.types.int_bits(ty).expect("an integer");

    // If there are no uses of CntPhi crate:
    //   Count = BitWidth - CTLZ(InitX);
    //   NewCount = Count;
    // If there are uses of CntPhi create:
    //   NewCount = BitWidth - CTLZ(InitX >> 1);
    //   Count = NewCount + 1;
    let init_x_next = if is_cnt_phi_used_outside_loop {
        let Opcode::Binary(shift) = run.function.instruction(found.def_x).opcode else { unreachable!("a shift") };
        emit(
            run.function,
            at,
            Opcode::Binary(shift),
            ty,
            vec![found.init_x, int(run.context, ty, 1)],
            Flags::default(),
            "",
        )
    } else {
        found.init_x
    };
    let i1 = run.context.types.int(1);
    let zero_check_flag = int(run.context, i1, u128::from(zero_check));
    let mut count = crate::switchopt::intrinsic_call(
        run.context,
        run.declared,
        run.function,
        at,
        found.direction.name(),
        ty,
        vec![init_x_next, zero_check_flag],
    );
    count = emit(
        run.function,
        at,
        Opcode::Binary(BinaryOp::Sub),
        ty,
        vec![int(run.context, ty, u128::from(width)), count],
        Flags::default(),
        "",
    );
    if insert_sub {
        count = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Sub),
            ty,
            vec![count, int(run.context, ty, 1)],
            Flags::default(),
            "",
        );
    }
    let mut new_count = count;
    if is_cnt_phi_used_outside_loop {
        count = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Add),
            ty,
            vec![count, int(run.context, ty, 1)],
            Flags::default(),
            "",
        );
    }

    let count_ty = run.function.value(run.function.instruction(found.count_inst).result.expect("a value")).ty;
    new_count = zext_or_trunc(run.context, run.function, at, new_count, count_ty, "");

    let cnt_init = incoming_value(run.function, found.count_phi, preheader).expect("a preheader input");
    let step = run.function.instruction(found.count_inst).operands[1];
    let incremented = matches!(
        step,
        Operand::Constant(c) if run.context.get(c).kind == ConstantKind::Int(1)
    );
    if incremented {
        // If the counter was being incremented in the loop, add NewCount to
        // the counter's initial value, but only if the initial value is not
        // zero.
        let init_is_zero = matches!(
            cnt_init,
            Operand::Constant(c) if run.context.get(c).kind == ConstantKind::Int(0)
        );
        if !init_is_zero {
            new_count = emit(
                run.function,
                at,
                Opcode::Binary(BinaryOp::Add),
                count_ty,
                vec![new_count, cnt_init],
                Flags::default(),
                "",
            );
        }
    } else {
        // If the count was being decremented in the loop, subtract NewCount
        // from the counter's initial value.
        new_count = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Sub),
            count_ty,
            vec![cnt_init, new_count],
            Flags::default(),
            "",
        );
    }

    // Step 2: Insert new IV and loop condition:
    // loop:
    //   ...
    //   PhiCount = PHI [Count, Dec]
    //   ...
    //   Dec = PhiCount - 1
    //   ...
    //   Br: loop if (Dec != 0)
    count_down_loop(run, preheader, body, count, IntPredicate::Ne, IntPredicate::Eq);

    // Step 3: All the references to the original counter outside the loop are
    //  replaced with the NewCount
    let replaced = if is_cnt_phi_used_outside_loop { found.count_phi } else { found.count_inst };
    let replaced = run.function.instruction(replaced).result.expect("a value");
    for (user, index) in uses_outside(run.function, replaced, body) {
        run.function.set_operand(user, index, new_count);
    }

    // step 4: Forget the "non-computable" trip-count SCEV associated with the
    //   loop. The loop would otherwise not be deleted even if it becomes empty.
    let cx = cx!(run);
    run.se.forget_loop(&cx, run.cur_loop.get_header());
}

#[cfg(test)]
mod tests {
    use crate::loopidiom::pass::LoopIdiom;
    use crate::testing::{parsed, printed};

    /// `while (x >>= 1) n++`, counting from the shifted value.
    const LOG2: &str = "define i32 @log2(i32 %x) {
entry:
  br label %loop

loop:
  %x1 = phi i32 [ %x, %entry ], [ %x2, %loop ]
  %n1 = phi i32 [ 0, %entry ], [ %n2, %loop ]
  %x2 = lshr i32 %x1, 1
  %done = icmp eq i32 %x2, 0
  %n2 = add nuw nsw i32 %n1, 1
  br i1 %done, label %exit, label %loop

exit:
  %n = phi i32 [ %n1, %loop ]
  ret i32 %n
}
";

    /// A target whose `Tti` does not opt in (z80: the 32-bit `ctlz` expands to
    /// 89 more lines than the loop) keeps its shift loop. The identity A/B had
    /// every target but wasm form the count.
    #[test]
    fn a_target_that_does_not_count_shift_loops_keeps_the_loop() {
        let mut module = parsed(LOG2);
        let mut passes = llrm_mir::passes::PassManager::default();
        passes.add(LoopIdiom::default());
        passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
        let after = printed(&module);
        assert!(!after.contains("ctlz"), "{after}");
        assert!(after.contains("%x2 = lshr i32 %x1, 1"), "{after}");
    }
}
