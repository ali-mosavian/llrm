//! `recognizeShiftUntilZero` of `LoopIdiomRecognize.cpp` at `d1106deb71cc`: a
//! loop that shifts one value by a growing amount until nothing is left gets
//! a trip count computed from `ctlz` or `cttz`.
//!
//! # How it works
//! [`LoopIdiomRecognize::recognize_shift_until_zero`] takes a one-block loop
//! `iv = phi [start, ph], [iv + 1, loop]` that leaves when `val >> (iv + off)`
//! (or `<<`, `>>` arithmetic) is zero, `val` loop-invariant. The shifted value
//! is zero from `iv = bitwidth - ctlz(val) - off` on, so the last `iv` is
//! `smax(that, start)`. The loop becomes a counted one from zero, the
//! original `iv` is `civ + start`, and the values it leaves are the final one.
//!
//! # Deviations
//! - The loop's `!prof` branch weights are not carried: the module holds none.
//! - The cost gate reads [`Tti::tcc_basic`] where the pin has `TCC_Basic`, and
//!   a compile without a `Tti` forms none.
//!
//! # Example
//! ```text
//! loop:  %iv = phi [%start, ph], [%iv.next, loop]        ph:   %lz = ctlz %val, false
//!        %n = add nsw %iv, %off                                ...  %iv.final, %tripcount
//!        %s = lshr %val, %n                              loop: %civ = phi [0, ph], [%civ.next, loop]
//!        %z = icmp eq %s, 0                                    %civ.next = add nuw %civ, 1
//!        %iv.next = add %iv, 1                                 %ivcheck = icmp eq %civ.next, %tripcount
//!        br %z, exit, loop                                     %iv = add nsw %civ, %start
//! ```

use llrm_analysis::scev::expander::Expander;
use llrm_analysis::scev::expr::Scev;
use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::tti::{CostKind, Ty};

use super::LoopIdiomRecognize;
use super::common::{defining_instruction, emit, int, uses_outside};
use super::ffs::incoming_value;
use super::process::{Run, cx};

/// What `detectShiftUntilZeroIdiom` (the one over `ScalarEvolution`) found.
struct ShiftUntilZero {
    /// `val >> nbits`, compared with zero.
    val_shifted_is_zero: InstId,
    /// `ctlz` for a right shift, `cttz` for a left one.
    intrinsic: &'static str,
    iv: InstId,
    start: Operand,
    val: Operand,
    /// `-off`, the offset the shift amount adds to `iv`, negated.
    extra_offset: Scev,
    inverted_cond: bool,
}

/// `nuw` and `nsw` as asked.
fn wrap(
    nuw: bool,
    nsw: bool,
) -> Flags {
    let mut flags = Flags::default();
    if nuw {
        flags = flags.union(Flags::NUW);
    }
    if nsw {
        flags = flags.union(Flags::NSW);
    }
    flags
}

impl LoopIdiomRecognize {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L3727
    /// `recognizeShiftUntilZero`.
    pub(super) fn recognize_shift_until_zero(
        &mut self,
        run: &mut Run,
    ) -> bool {
        let Some(found) = detect_shift_until_zero_idiom(run) else { return false };

        // Ok, it is the idiom we were looking for, we *could* transform this
        // loop, but is it profitable to transform?
        let header = run.cur_loop.get_header();
        let preheader = run.cur_loop.get_loop_preheader(run.function).expect("a preheader");
        let successor = run.cur_loop.get_exit_block(run.function).expect("a single successor");
        let ty = run.function.operand_type(run.context, found.val).expect("a typed value");
        let bitwidth = run.context.types.int_bits(ty).expect("an integer");

        // The rewrite is considered to be unprofitable iff and only iff the
        // intrinsic we'll use are not cheap. Note that we are okay with *just*
        // making the loop countable, even if nothing else changes.
        let Some(tti) = run.tti else { return false };
        let kind = CostKind::SizeAndLatency;
        let cost = tti.intrinsic_cost(
            &format!("llvm.{}.i{bitwidth}", found.intrinsic),
            Some(Ty::Int(bitwidth)),
            &[Ty::Int(bitwidth), Ty::Int(1)],
            kind,
        );
        if !cost.valid || cost.value > tti.tcc_basic(kind).value {
            return false;
        }

        // Ok, transform appears worthwhile.
        let offset_is_zero = run.se.is_zero_scev(found.extra_offset);
        let at = run.function.terminator(preheader).expect("a preheader ends");
        let i1 = run.context.types.int(1);
        let one = int(run.context, ty, 1);

        // Step 1: Compute the loop's final IV value / trip count.
        let false_flag = int(run.context, i1, 0);
        let num_leading_zeros = crate::switchopt::intrinsic_call(
            run.context,
            run.declared,
            run.function,
            at,
            found.intrinsic,
            ty,
            vec![found.val, false_flag],
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

        let mut expander = Expander::new("loop-idiom");
        expander.set_insert_point(at);
        let extra_offset = expander.expand_code_for(
            run.env,
            run.context,
            run.declared,
            run.function,
            run.se,
            found.extra_offset,
            None,
        );

        let num_active_bits_offset = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Add),
            ty,
            vec![num_active_bits, extra_offset],
            wrap(offset_is_zero, true),
            "",
        );
        let iv_final = crate::switchopt::intrinsic_call(
            run.context,
            run.declared,
            run.function,
            at,
            "smax",
            ty,
            vec![num_active_bits_offset, found.start],
        );
        let backedge_taken_count = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Sub),
            ty,
            vec![iv_final, found.start],
            wrap(offset_is_zero, true),
            "",
        );
        // FIXME (LLVM): or when the offset was `add nuw`

        // We know loop's backedge-taken count, but what's loop's trip count?
        let trip_count = emit(
            run.function,
            at,
            Opcode::Binary(BinaryOp::Add),
            ty,
            vec![backedge_taken_count, one],
            wrap(true, bitwidth != 2),
            "",
        );

        // Step 2: Adjust the successor basic block to receive the original
        //         induction variable's final value instead of the orig. IV
        //         itself.
        let iv_value = run.function.instruction(found.iv).result.expect("a phi value");
        for (user, index) in uses_outside(run.function, iv_value, header) {
            run.function.set_operand(user, index, iv_final);
        }

        // Step 3: Rewrite the loop into a countable form, with canonical IV.

        // The new canonical induction variable.
        let first = run.function.iter_block(header).next().expect("a loop body");
        let civ = run
            .function
            .create_instruction(
                Opcode::Phi,
                ty,
                vec![int(run.context, ty, 0), Operand::Block(preheader)],
                Flags::default(),
                Some("iv"),
            );
        run.function.insert(civ, Position::Before(first)).expect("a placed phi");
        let civ_value = Operand::Value(run.function.instruction(civ).result.expect("a phi value"));

        // The induction itself.
        let first_non_phi = run
            .function
            .iter_block(header)
            .find(|&inst| run.function.instruction(inst).opcode != Opcode::Phi)
            .expect("a non-phi");
        let civ_next = emit(
            run.function,
            first_non_phi,
            Opcode::Binary(BinaryOp::Add),
            ty,
            vec![civ_value, one],
            wrap(true, bitwidth != 2),
            "iv.next",
        );

        // The loop trip count check.
        let civ_check = emit(
            run.function,
            first_non_phi,
            Opcode::ICmp(IntPredicate::Eq),
            i1,
            vec![civ_next, trip_count],
            Flags::default(),
            "ivcheck",
        );
        let new_iv_check = if found.inverted_cond {
            emit(
                run.function,
                first_non_phi,
                Opcode::Binary(BinaryOp::Xor),
                i1,
                vec![civ_check, int(run.context, i1, 1)],
                Flags::default(),
                "",
            )
        } else {
            civ_check
        };

        // The original IV, but rebased to be an offset to the CIV.
        let iv_de_phid = emit(
            run.function,
            first_non_phi,
            Opcode::Binary(BinaryOp::Add),
            ty,
            vec![civ_value, found.start],
            wrap(false, true),
            "",
        ); // FIXME (LLVM): what about NUW?

        // The loop terminator.
        let terminator = run.function.terminator(header).expect("a loop branch");
        let branch = run
            .function
            .create_instruction(
                Opcode::Br,
                run.context.types.void(),
                vec![civ_check, Operand::Block(successor), Operand::Block(header)],
                Flags::default(),
                None,
            );
        run.function.insert(branch, Position::Before(terminator)).expect("a placed branch");
        run.function.erase(terminator).expect("the old branch");

        // Populate the IV PHI.
        run.function.set_operands(
            civ,
            vec![int(run.context, ty, 0), Operand::Block(preheader), civ_next, Operand::Block(header)],
        );

        // Step 4: Forget the "non-computable" trip-count SCEV associated with
        //   the loop. The loop would otherwise not be deleted even if it
        //   becomes empty.
        let cx = cx!(run);
        run.se.forget_loop(&cx, header);

        // Step 5: Try to cleanup the loop's body somewhat.
        run.function.replace_all_uses_with(iv_value, iv_de_phid);
        run.function.set_operands(found.iv, Vec::new());
        run.function.erase(found.iv).expect("an unused phi");

        let is_zero = run.function.instruction(found.val_shifted_is_zero).result.expect("a compare value");
        run.function.replace_all_uses_with(is_zero, new_iv_check);
        run.function.erase(found.val_shifted_is_zero).expect("an unused compare");

        // Other passes will take care of actually deleting the loop if
        // possible.
        true
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L3559
/// `detectShiftUntilZeroIdiom` (the one over `ScalarEvolution`).
fn detect_shift_until_zero_idiom(run: &mut Run) -> Option<ShiftUntilZero> {
    // Give up if the loop has multiple blocks or multiple backedges.
    if run.cur_loop.get_num_blocks() != 1 || run.cur_loop.get_num_back_edges(&*run.function) != 1 {
        return None;
    }
    let header = run.cur_loop.get_header();
    let preheader = run.cur_loop.get_loop_preheader(run.function)?;

    // Step 1: Check if the loop backedge, condition is in desirable form.
    let terminator = run.function.terminator(header)?;
    let [condition, Operand::Block(mut true_block), Operand::Block(mut false_block)] =
        run.function.instruction(terminator).operands[..]
    else {
        return None;
    };
    let val_shifted_is_zero = defining_instruction(run.function, condition)?;
    let Opcode::ICmp(predicate) = run.function.instruction(val_shifted_is_zero).opcode else { return None };
    let [shifted, Operand::Constant(zero)] = run.function.instruction(val_shifted_is_zero).operands[..] else {
        return None;
    };
    if !matches!(
        run.context.get(zero).kind,
        ConstantKind::Int(0) | ConstantKind::Zero
    )
        || !matches!(predicate, IntPredicate::Eq | IntPredicate::Ne)
    {
        return None;
    }
    let val_shifted = defining_instruction(run.function, shifted)?;

    // Step 2: Check if the comparison's operand is in desirable form.
    // FIXME (LLVM): Val could be a one-input PHI node, which we should look
    // past.
    let Opcode::Binary(shift @ (BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)) =
        run.function.instruction(val_shifted).opcode
    else {
        return None;
    };
    let [val, nbits] = run.function.instruction(val_shifted).operands[..] else { return None };
    if !run.cur_loop.is_loop_invariant(run.function, val) {
        return None;
    }
    let nbits = defining_instruction(run.function, nbits)?;
    let intrinsic = if shift == BinaryOp::Shl { "cttz" } else { "ctlz" };

    // Step 3: Check if the shift amount is in desirable form.
    let nbits_flags = run.function.instruction(nbits).flags;
    let nbits_opcode = run.function.instruction(nbits).opcode.clone();
    let nbits_operands = run.function.instruction(nbits).operands.clone();
    let as_add = (nbits_opcode == Opcode::Binary(BinaryOp::Add))
        .then(|| {
            // `m_c_Add(m_Instruction(IV),
            // m_LoopInvariant(m_Value(ExtraOffset)))`.
            let [first, second] = nbits_operands[..] else { return None };
            [(first, second), (second, first)]
                .into_iter()
                .find_map(
                    |(iv, offset)| {
                        let iv = defining_instruction(run.function, iv)?;
                        run.cur_loop.is_loop_invariant(run.function, offset).then_some((iv, offset))
                    },
                )
        })
        .flatten()
        .filter(|_| nbits_flags.contains(Flags::NSW) || nbits_flags.contains(Flags::NUW));
    let as_sub = (nbits_opcode == Opcode::Binary(BinaryOp::Sub))
        .then(|| {
            let [first, second] = nbits_operands[..] else { return None };
            let iv = defining_instruction(run.function, first)?;
            run.cur_loop.is_loop_invariant(run.function, second).then_some((iv, second))
        })
        .flatten()
        .filter(|_| nbits_flags.contains(Flags::NSW));
    let (iv, extra_offset) = if let Some((iv, offset)) = as_add {
        let cx = cx!(run);
        let offset = run.se.get_scev(&cx, offset);
        (iv, run.se.get_negative_scev(&cx, offset, 0))
    } else if let Some((iv, offset)) = as_sub {
        let cx = cx!(run);
        (iv, run.se.get_scev(&cx, offset))
    } else {
        let cx = cx!(run);
        let nbits_value = Operand::Value(run.function.instruction(nbits).result?);
        let ev = run.se.get_scev(&cx, nbits_value);
        let ty = run.se.type_of(ev);
        (nbits, run.se.get_zero(&cx, ty))
    };

    // Step 4: Check if the recurrence is in desirable form.
    if run.function.instruction(iv).opcode != Opcode::Phi || run.function.parent(iv) != Some(header) {
        return None;
    }
    let start = incoming_value(run.function, iv, preheader)?;
    let iv_next = defining_instruction(run.function, incoming_value(run.function, iv, header)?)?;
    let iv_value = Operand::Value(run.function.instruction(iv).result?);
    let step = run.function.instruction(iv_next);
    if step.opcode != Opcode::Binary(BinaryOp::Add)
        || step.operands[0] != iv_value
        || !matches!(
            step.operands[1],
            Operand::Constant(c) if run.context.get(c).kind == ConstantKind::Int(1)
        )
    {
        return None;
    }

    // Step 4: Check if the backedge's destinations are in desirable form.

    // cmp-br is commutative, so canonicalize to a single variant.
    let inverted_cond = predicate != IntPredicate::Eq;
    if inverted_cond {
        std::mem::swap(&mut true_block, &mut false_block);
    }

    // We expect to exit loop when comparison yields true, so when it yields
    // false we should branch back to loop header.
    let _ = true_block;
    if false_block != header {
        return None;
    }

    // The new, countable, loop will certainly only run a known number of
    // iterations, It won't be infinite. But the old loop might be infinite
    // under certain conditions. For logical shifts, the value will become zero
    // after at most bitwidth(%Val) loop iterations. However, for arithmetic
    // right-shift, iff the sign bit was set, the value will never become zero,
    // and the loop may never finish.
    if shift == BinaryOp::AShr {
        let must_progress = llrm_mir::facts::Facts::of(&run.function.attrs).must_progress()
            || llrm_mir::loopinfo::marked(run.env.metadata, run.function, header);
        let cx = cx!(run);
        let val_scev = run.se.get_scev(&cx, val);
        if !must_progress && !run.se.is_known_non_negative(&cx, val_scev) {
            return None;
        }
    }

    // Okay, idiom checks out.
    Some(ShiftUntilZero { val_shifted_is_zero, intrinsic, iv, start, val, extra_offset, inverted_cond })
}
