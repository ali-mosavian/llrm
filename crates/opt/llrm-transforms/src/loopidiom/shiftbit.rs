//! `recognizeShiftUntilBitTest` of `LoopIdiomRecognize.cpp` at `d1106deb71cc`:
//! a loop that shifts a value left until a bit of it is set gets a trip count
//! computed from `ctlz`.
//!
//! # How it works
//! [`LoopIdiomRecognize::recognize_shift_until_bit_test`] takes a one-block
//! loop `x.curr = phi [x, ph], [x.next, loop]`, `x.next = x.curr << 1`, that
//! leaves when `x.curr & (1 << bitpos)` is set (the mask a loop-invariant
//! `shl 1, bitpos`, or any single-bit test the compare decomposes into). The
//! last trip is the first where the bit is set: the highest set bit of `x`
//! below `bitpos`, so the backedge-taken count is `bitpos - leadingonepos(x &
//! (lowbits | bit))`. The loop becomes a counted one, and the values it leaves
//! (`x.curr`, `x.next`) are shifts of `x` by that count.
//!
//! # Deviations
//! - The loop's `!prof` branch weights are not carried: the module holds none.
//! - The cost gate reads [`Tti::tcc_basic`] where the pin has `TCC_Basic`, and
//!   a compile without a `Tti` forms none.
//!
//! # Example
//! ```text
//! loop:  %x.curr = phi [%x, ph], [%x.next, loop]      ph:   %lz = ctlz (x & mask), true
//!        %m = and %x.curr, %bitmask                         ...  %tripcount, %x.curr', %x.next'
//!        %set = icmp ne %m, 0                         loop: %iv = phi [0, ph], [%iv.next, loop]
//!        %x.next = shl %x.curr, 1                           %iv.next = add nuw %iv, 1
//!        br %set, exit, loop                                %ivcheck = icmp eq %iv.next, %tripcount
//! ```

use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::tti::{ArithOp, CostKind, OperandInfo, Ty};
use llrm_mir::valuetracking::bittest::{BitTestOptions, decompose_bit_test};
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison;

use super::LoopIdiomRecognize;
use super::common::{defining_instruction, emit, int, uses_outside};
use super::countdown::wrap;
use super::ffs::{incoming_value, query};
use super::process::{Run, cx};

/// What `detectShiftUntilBitTestIdiom` found.
struct ShiftUntilBitTest {
    /// `x`, the value the loop starts from.
    base_x: Operand,
    bit_mask: Operand,
    bit_pos: Operand,
    /// `x.curr`.
    curr_x: Operand,
    /// `x.next = x.curr << 1`.
    next_x: InstId,
}

impl LoopIdiomRecognize {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L3345
    /// `recognizeShiftUntilBitTest`.
    pub(super) fn recognize_shift_until_bit_test(
        &mut self,
        run: &mut Run,
    ) -> bool {
        let Some(found) = detect_shift_until_bit_test_idiom(run) else { return false };

        // Ok, it is the idiom we were looking for, we *could* transform this
        // loop, but is it profitable to transform?
        let header = run.cur_loop.get_header();
        let preheader = run.cur_loop.get_loop_preheader(run.function).expect("a preheader");
        let successor = run.cur_loop.get_exit_block(run.function).expect("a single successor");
        let ty = run.function.operand_type(run.context, found.base_x).expect("a typed value");
        let bitwidth = run.context.types.int_bits(ty).expect("an integer");

        // The rewrite is considered to be unprofitable iff and only iff the
        // intrinsic/shift we'll use are not cheap. Note that we are okay with
        // *just* making the loop countable, even if nothing else changes.
        let Some(tti) = run.tti else { return false };
        let kind = CostKind::SizeAndLatency;
        let basic = tti.tcc_basic(kind);
        let cost = tti.zero_poison_intrinsic_cost(
            &format!("llvm.ctlz.i{bitwidth}"),
            Some(Ty::Int(bitwidth)),
            &[Ty::Int(bitwidth), Ty::Int(1)],
            kind,
        );
        if !cost.valid || cost.value > basic.value {
            return false;
        }
        let shift = tti.arithmetic_cost(ArithOp::Shl, Ty::Int(bitwidth), kind, OperandInfo::ANY, OperandInfo::ANY);
        if !shift.valid || shift.value > basic.value {
            return false;
        }

        // Ok, transform appears worthwhile.
        let mut bit_pos = found.bit_pos;
        let at_end = run.function.terminator(preheader).expect("a preheader ends");
        let guaranteed = {
            let mut q = query(run, at_end);
            q.dt = None;
            q.assumptions = None;
            q.cxt = None;
            is_guaranteed_not_to_be_undef_or_poison(&q, bit_pos, 0)
        };
        if !guaranteed {
            // BitMask may be computed from BitPos, Freeze BitPos so we can
            // increase it's use count.
            let position = match defining_instruction(run.function, bit_pos) {
                Some(def) => Position::After(def),
                None => {
                    let root = run.env.tree.root();
                    let first = run
                        .function
                        .iter_block(root)
                        .find(|&inst| run.function.instruction(inst).opcode != Opcode::Phi);
                    match first {
                        Some(first) => Position::Before(first),
                        None => return false,
                    }
                }
            };
            let pos_ty = run.function.operand_type(run.context, bit_pos).expect("a typed value");
            let freeze = run.function.create_instruction(Opcode::Freeze, pos_ty, vec![bit_pos], Flags::default(), None);
            run.function.insert(freeze, position).expect("a placed freeze");
            let frozen = Operand::Value(run.function.instruction(freeze).result.expect("a value"));
            if let Operand::Value(old) = bit_pos {
                let users: Vec<(InstId, usize)> =
                    run.function.users(old).into_iter().map(|one| (one.user, one.index as usize)).collect();
                for (user, index) in users {
                    if user != freeze {
                        run.function.set_operand(user, index, frozen);
                    }
                }
            }
            bit_pos = frozen;
        }

        // Step 1: Compute the loop trip count.
        let at = run.function.terminator(preheader).expect("a preheader ends");
        let all_ones = int(run.context, ty, llrm_mir::context::mask(bitwidth));
        let one = int(run.context, ty, 1);
        let low_bit_mask = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Add),
            ty,
            vec![found.bit_mask, all_ones],
            Flags::default(),
            "",
        );
        let mask = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Or),
            ty,
            vec![low_bit_mask, found.bit_mask],
            Flags::default(),
            "",
        );
        let x_masked =
            emit(run.function, at, Opcode::Binary(BinaryOp::And), ty, vec![found.base_x, mask], Flags::default(), "");
        let i1 = run.context.types.int(1);
        let true_flag = int(run.context, i1, 1);
        let num_leading_zeros = crate::switchopt::intrinsic_call(
            run.context,
            run.declared,
            run.function,
            at,
            "ctlz",
            ty,
            vec![x_masked, true_flag],
        );
        let num_active_bits = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Sub),
            ty,
            vec![int(run.context, ty, u128::from(bitwidth)), num_leading_zeros],
            wrap(true, bitwidth != 2),
            "",
        );
        let leading_one_pos = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Add),
            ty,
            vec![num_active_bits, all_ones],
            wrap(false, bitwidth > 2),
            "",
        );
        let backedge_taken_count = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Sub),
            ty,
            vec![bit_pos, leading_one_pos],
            wrap(true, true),
            "",
        );
        // We know loop's backedge-taken count, but what's loop's trip count?
        // Note that while NUW is always safe, while NSW is only for bitwidths
        // != 2.
        let trip_count = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Add),
            ty,
            vec![backedge_taken_count, one],
            wrap(true, bitwidth != 2),
            "",
        );

        // Step 2: Compute the recurrence's final value without a loop.

        // NewX is always safe to compute, because `LoopBackedgeTakenCount`
        // will always be smaller than `bitwidth(X)`, i.e. we never get poison.
        let next_flags = run.function.instruction(found.next_x).flags;
        let new_x = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Shl),
            ty,
            vec![found.base_x, backedge_taken_count],
            next_flags,
            "",
        );

        // Rewriting XNext is more complicated, however, because `X <<
        // LoopTripCount` will be poison iff `LoopTripCount == bitwidth(X)`
        // (which will happen iff `BitPos` is `bitwidth(x) - 1` and `X` is
        // `1`). So unless we know that isn't the case, we'll need to emit an
        // alternative, safe IR.
        let wraps = next_flags.contains(Flags::NSW) || next_flags.contains(Flags::NUW);
        let pos_not_top = matches!(
            bit_pos,
            Operand::Constant(c) if matches!(run.context.get(c).kind, ConstantKind::Int(v) if v != u128::from(bitwidth - 1))
        );
        let new_x_next = if wraps || pos_not_top {
            emit(run.function, at, Opcode::Binary(BinaryOp::Shl), ty, vec![found.base_x, trip_count], next_flags, "")
        } else {
            // Otherwise, just additionally shift by one. It's the smallest
            // solution, alternatively, we could check that NewX is INT_MIN (or
            // BitPos is ) and select 0 instead.
            emit(run.function, at, Opcode::Binary(BinaryOp::Shl), ty, vec![new_x, one], next_flags, "")
        };

        // Step 3: Adjust the successor basic block to receive the computed
        //         recurrence's final value instead of the recurrence itself.
        let replace_outside = |run: &mut Run, old: Operand, new: Operand| {
            if let Operand::Value(old) = old {
                for (user, index) in uses_outside(run.function, old, header) {
                    run.function.set_operand(user, index, new);
                }
            }
        };
        replace_outside(run, found.curr_x, new_x);
        let next_value = Operand::Value(run.function.instruction(found.next_x).result.expect("a value"));
        replace_outside(run, next_value, new_x_next);

        // Step 4: Rewrite the loop into a countable form, with canonical IV.

        // The new canonical induction variable.
        let first = run.function.iter_block(header).next().expect("a loop body");
        let iv = run
            .function
            .create_instruction(
                Opcode::Phi,
                ty,
                vec![int(run.context, ty, 0), Operand::Block(preheader)],
                Flags::default(),
                Some("iv"),
            );
        run.function.insert(iv, Position::Before(first)).expect("a placed phi");
        let iv_value = Operand::Value(run.function.instruction(iv).result.expect("a phi value"));

        // The induction itself. Note that while NUW is always safe, while NSW
        // is only for bitwidths != 2.
        let terminator = run.function.terminator(header).expect("a loop branch");
        let iv_next = emit(
            run.function,
            terminator,
            Opcode::Binary(BinaryOp::Add),
            ty,
            vec![iv_value, one],
            wrap(true, bitwidth != 2),
            "iv.next",
        );

        // The loop trip count check.
        let iv_check = emit(
            run.function,
            terminator,
            Opcode::ICmp(IntPredicate::Eq),
            i1,
            vec![iv_next, trip_count],
            Flags::default(),
            "ivcheck",
        );
        let branch = run
            .function
            .create_instruction(
                Opcode::Br,
                run.context.types.void(),
                vec![iv_check, Operand::Block(successor), Operand::Block(header)],
                Flags::default(),
                None,
            );
        run.function.insert(branch, Position::Before(terminator)).expect("a placed branch");
        run.function.erase(terminator).expect("the old branch");

        // Populate the IV PHI.
        run.function.set_operands(
            iv,
            vec![int(run.context, ty, 0), Operand::Block(preheader), iv_next, Operand::Block(header)],
        );

        // Step 5: Forget the "non-computable" trip-count SCEV associated with
        //   the loop. The loop would otherwise not be deleted even if it
        //   becomes empty.
        let cx = cx!(run);
        run.se.forget_loop(&cx, header);

        // Other passes will take care of actually deleting the loop if
        // possible.
        true
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L3191
/// `detectShiftUntilBitTestIdiom`.
fn detect_shift_until_bit_test_idiom(run: &Run) -> Option<ShiftUntilBitTest> {
    let function = &*run.function;
    let context = &*run.context;

    // Give up if the loop has multiple blocks or multiple backedges.
    if run.cur_loop.get_num_blocks() != 1 || run.cur_loop.get_num_back_edges(function) != 1 {
        return None;
    }
    let header = run.cur_loop.get_header();
    let preheader = run.cur_loop.get_loop_preheader(function)?;

    // Step 1: Check if the loop backedge is in desirable form.
    let terminator = function.terminator(header)?;
    let [condition, Operand::Block(mut true_block), Operand::Block(mut false_block)] =
        function.instruction(terminator).operands[..]
    else {
        return None;
    };
    let compare = defining_instruction(function, condition)?;
    let Opcode::ICmp(mut predicate) = function.instruction(compare).opcode else { return None };
    let [cmp_lhs, cmp_rhs] = function.instruction(compare).operands[..] else { return None };

    // Step 2: Check if the backedge's condition is in desirable form.
    let is_zero = |operand: Operand| {
        matches!(
            operand,
            Operand::Constant(c) if matches!(context.get(c).kind, ConstantKind::Int(0) | ConstantKind::Zero)
        )
    };
    let is_one = |operand: Operand| {
        matches!(
            operand,
            Operand::Constant(c) if context.get(c).kind == ConstantKind::Int(1)
        )
    };
    // `m_c_And(m_Value(CurrX), m_CombineAnd(m_Value(BitMask),
    // m_LoopInvariant(m_Shl(m_One(), m_Value(BitPos)), CurLoop)))`.
    let variable_bit_mask = || -> Option<(Operand, Operand, Operand)> {
        if !matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) || !is_zero(cmp_rhs) {
            return None;
        }
        let and = defining_instruction(function, cmp_lhs)
            .filter(|&inst| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::And))?;
        let [first, second] = function.instruction(and).operands[..] else { return None };
        [(first, second), (second, first)]
            .into_iter()
            .find_map(
                |(curr, mask)| {
                    let shl = defining_instruction(function, mask)
                        .filter(|&inst| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Shl))?;
                    let [base, bit_pos] = function.instruction(shl).operands[..] else { return None };
                    (is_one(base) && run.cur_loop.is_loop_invariant(function, mask)).then_some((curr, mask, bit_pos))
                },
            )
    };
    let (curr_x, bit_mask, bit_pos) = match variable_bit_mask() {
        Some(found) => found,
        None => {
            let q = query(run, terminator);
            let options = BitTestOptions { look_through_trunc: true, allow_non_zero_c: false, decompose_and: true };
            let decomposed = decompose_bit_test(&q, condition, options)?;
            if !decomposed.mask.is_power_of_two() {
                return None;
            }
            debug_assert!(matches!(decomposed.pred, IntPredicate::Eq | IntPredicate::Ne));
            predicate = decomposed.pred;
            let x_ty = function.operand_type(context, decomposed.x)?;
            (
                decomposed.x,
                int(context, x_ty, decomposed.mask),
                int(context, x_ty, u128::from(decomposed.mask.trailing_zeros())),
            )
        }
    };

    // Step 3: Check if the recurrence is in desirable form.
    let curr_x_phi = defining_instruction(function, curr_x)
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi && function.parent(inst) == Some(header))?;
    let base_x = incoming_value(function, curr_x_phi, preheader)?;
    let next_x = defining_instruction(function, incoming_value(function, curr_x_phi, header)?)?;

    // FIXME (LLVM): support right-shift?
    if function.instruction(next_x).opcode != Opcode::Binary(BinaryOp::Shl)
        || function.instruction(next_x).operands[0] != curr_x
        || !is_one(function.instruction(next_x).operands[1])
    {
        return None;
    }

    // Step 4: Check if the backedge's destinations are in desirable form.

    // cmp-br is commutative, so canonicalize to a single variant.
    if predicate != IntPredicate::Eq {
        std::mem::swap(&mut true_block, &mut false_block);
    }

    // We expect to exit loop when comparison yields false, so when it yields
    // true we should branch back to loop header.
    let _ = false_block;
    if true_block != header {
        return None;
    }

    // Okay, idiom checks out.
    Some(ShiftUntilBitTest { base_x, bit_mask, bit_pos, curr_x, next_x })
}
