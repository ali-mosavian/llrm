//! `runOnNoncountableLoop` and `recognizeAndInsertStrLen` of LLVM's
//! `LoopIdiomRecognize.cpp` at `d1106deb71cc`: a loop that walks a string to
//! its terminator becomes a call of `strlen`, and the values it leaves are
//! `base + step * strlen`.
//!
//! # How it works
//! [`LoopIdiomRecognize::recognize_and_insert_str_len`] checks the loop with
//! `StrlenVerifier::isValidStrlenIdiom` (one block, one back edge, one exit; a
//! byte loaded through an affine pointer and compared with zero; nothing in the
//! body with a side effect; every value the exit phis hold affine with a
//! constant step), then calls `strlen` on the base in the preheader and
//! rewrites each exit phi from it. The loop's own branch is made to leave at
//! once, as LoopDeletion deletes only what it can count.
//!
//! # Deviations
//! - D11: `strlen` is formed only where the module declares or defines it (a
//!   name that is `strlen`, or `strlen` with the one decoration underscore),
//!   and the call copies that declaration's type and calling convention. The
//!   pin forms it on the triple's `TargetLibraryInfo` alone; no target here
//!   implements `Target::library`, and the decoration and the convention are
//!   frontend facts. llrm-c declares it for the targets whose C link resolves
//!   it; llrm-qb and llrm-nib declare nothing and never form it. Exit: a
//!   `Target::library` with the name and convention for the C targets. Owner
//!   w-6, review 2026-11-30. A function with `"no-builtins"` forms none
//!   (`LoopIdiomRecognize::libcall`, the one availability every idiom asks).
//! - D12: `wcslen` is not formed: there is no `wchar_t` width in the target's
//!   library facts (`getWCharSize`). Exit with D11.
//! - Calls count as side effects in the body scan, where LLVM lets a call that
//!   does not write, always returns and does not unwind through.
//! - `runOnNoncountableLoop`'s other idioms (`recognizePopcount`,
//!   `recognizeAndInsertFFS`, `recognizeShiftUntilBitTest`,
//!   `recognizeShiftUntilZero`, `recognizeShiftUntilLessThan`) are owed.
//!
//! # Example
//! ```text
//! b2:                                  preheader:
//!   %i = phi [0, %b1], [%n, %b2]         %len = call strlen(%s)
//!   %p = gep %s, %i                    exit:
//!   %c = load i8, %p                     (the phi of %i reads %len)
//!   %z = icmp ne i8 %c, 0
//!   %n = add %i, 1
//!   br %z, %b2, %exit
//! ```

use llrm_analysis::scev::Flags as ScevFlags;
use llrm_analysis::scev::expander::Expander;
use llrm_analysis::scev::expr::Use;
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{Attribute, CallInfo, Flags, IntPredicate, Opcode};
use llrm_mir::types::Type;

use super::LoopIdiomRecognize;
use super::process::{Run, cx};

/// `matchCondition`: the value the loop's branch tests against zero, when the
/// branch stays in the loop on a non-zero value.
fn match_condition(
    run: &Run,
    terminator: InstId,
    loop_entry: llrm_mir::module::BlockId,
) -> Option<Operand> {
    let branch = run.function.instruction(terminator);
    let [condition, Operand::Block(true_successor), Operand::Block(false_successor)] = branch.operands[..] else {
        return None;
    };
    let Operand::Value(condition) = condition else { return None };
    let llrm_mir::module::ValueDef::Instruction(compare) = run.function.value(condition).def else { return None };
    let Opcode::ICmp(predicate) = run.function.instruction(compare).opcode else { return None };
    let [tested, Operand::Constant(zero)] = run.function.instruction(compare).operands[..] else { return None };
    if !matches!(
        run.context.get(zero).kind,
        ConstantKind::Int(0) | ConstantKind::Zero
    ) {
        return None;
    }
    ((predicate == IntPredicate::Ne && true_successor == loop_entry)
        || (predicate == IntPredicate::Eq && false_successor == loop_entry))
        .then_some(tested)
}

impl LoopIdiomRecognize {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2052
    /// `runOnNoncountableLoop`.
    pub fn run_on_noncountable_loop(
        &mut self,
        run: &mut Run,
    ) -> bool {
        // OWED: recognizePopcount, recognizeAndInsertFFS,
        // recognizeShiftUntilBitTest, recognizeShiftUntilZero and
        // recognizeShiftUntilLessThan, in this order, before the string length.
        self.recognize_and_insert_str_len(run)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2219
    /// `recognizeAndInsertStrLen`.
    fn recognize_and_insert_str_len(
        &mut self,
        run: &mut Run,
    ) -> bool {
        let no_builtins =
            run.function.attrs.iter().any(|attr| matches!(attr, Attribute::Str(key, _) if key == "no-builtins"));
        if self.options.disable_all || no_builtins {
            return false;
        }
        let Some(verified) = self.verify_strlen_idiom(run) else { return false };
        // D11: the compile says the program links `strlen`.
        let stated = self.routines.iter().find(|one| one.standard == "strlen").cloned();
        let Some(routine) = stated.filter(|_| self.libcall(true)) else { return false };

        let preheader = run.cur_loop.get_loop_preheader(run.function).expect("verified to have one");
        let loop_body = run.cur_loop.get_blocks()[0];
        let exit = run.cur_loop.get_exit_block(run.function).expect("verified to have one exit");
        let loop_term = run.function.terminator(loop_body).expect("a terminator");
        let insert_at = run.function.terminator(preheader).expect("a preheader ends");

        let mut expander = Expander::new("strlen_idiom");
        expander.set_insert_point(insert_at);
        let base = expander.expand_code_for(
            run.env,
            run.context,
            run.declared,
            run.function,
            run.se,
            verified.load_base,
            None,
        );

        // `emitStrLen`: `getOrInsertFunction` of `size_t strlen(const char *)`
        // and the call, placed before the preheader's terminator.
        let pointer = run.context.types.ptr(0);
        let returns = run.context.types.int(run.env.layout.pointer(0).index_bits);
        let ty = run.context.types.intern(Type::Function { returns, parameters: vec![pointer], variadic: false });
        let callee_id = run
            .declared
            .routine(
                &routine.symbol,
                ty,
                routine.calling_convention,
                vec![
                    llrm_mir::opcode::Attribute::Memory(vec![(Some("argmem".to_owned()), "read".to_owned())]),
                    llrm_mir::facts::Fact::NoUnwind.carrier(),
                    llrm_mir::facts::Fact::WillReturn.carrier(),
                ],
                vec![llrm_mir::facts::Fact::NoCapture.carrier()],
            );
        let callee =
            Operand::Constant(run.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee_id) }));
        let info = CallInfo {
            function_type: ty,
            calling_convention: routine.calling_convention,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new()],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        let call = run
            .function
            .create_instruction(Opcode::Call(Box::new(info)), returns, vec![base, callee], Flags::default(), None);
        run.function.insert(call, Position::Before(insert_at)).expect("a placed call");
        let length = Operand::Value(run.function.instruction(call).result.expect("strlen returns"));

        let strlen_ev = {
            let cx = cx!(run);
            run.se.get_scev(&cx, length)
        };
        let mut cleanup = Vec::new();
        let phis: Vec<InstId> = run
            .function
            .iter_block(exit)
            .take_while(|&inst| run.function.instruction(inst).opcode == Opcode::Phi)
            .collect();
        for phi in phis {
            // We can now materialize the loop output as all phi have scev
            // {base,+,a}: %phi.new = base expression + step * %strlen.
            let result = Operand::Value(run.function.instruction(phi).result.expect("a phi has a result"));
            let cx = cx!(run);
            let ev = run.se.get_scev(&cx, result);
            let (start, step, _) = run.se.match_affine_add_rec(ev).expect("verified affine");
            // It is safe to truncate to base since if base is narrower than
            // size_t the equivalent user code will have to truncate anyways.
            let base_ty = run.se.type_of(start);
            let adjusted = run.se.get_truncate_or_sign_extend(&cx, strlen_ev, base_ty, 0);
            let scaled = run.se.get_mul_expr(&cx, vec![Use::of(step), Use::of(adjusted)], ScevFlags::default(), 0).scev;
            let new_ev = run.se.get_add_expr(&cx, vec![Use::of(start), Use::of(scaled)], ScevFlags::default(), 0).scev;
            let ty = run.se.type_of(new_ev);
            let materialized =
                expander.expand_code_for(run.env, run.context, run.declared, run.function, run.se, new_ev, Some(ty));
            let Operand::Value(old) = result else { unreachable!("a result") };
            run.function.replace_all_uses_with(old, materialized);
            cleanup.push(phi);
        }

        // All LCSSA loop phi are dead, the left over dead loop body can be
        // cleaned up by later passes.
        for phi in cleanup {
            if let Some(result) = run.function.instruction(phi).result
                && run.function.users(result).into_iter().next().is_none()
            {
                run.function.erase(phi).expect("an unused phi");
            }
        }

        // LoopDeletion only deletes invariant loops with a known trip count:
        // make the loop leave at once so it can.
        let Operand::Block(first) = run.function.instruction(loop_term).operands[1] else {
            unreachable!("a conditional branch")
        };
        let leaves_on = if first == loop_body { 0 } else { 1 };
        let condition = Operand::Constant(run.context.int(run.context.types.int(1), leaves_on));
        run.function.set_operand(loop_term, 0, condition);
        let cx = cx!(run);
        run.se.forget_loop(&cx, run.cur_loop.get_header());
        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2131
    /// `StrlenVerifier::isValidStrlenIdiom`: the base of the string where the
    /// loop is one.
    fn verify_strlen_idiom(
        &mut self,
        run: &mut Run,
    ) -> Option<Verified> {
        // Give up if the loop has multiple blocks, multiple backedges, or
        // multiple exit blocks.
        if run.cur_loop.get_num_back_edges(&*run.function) != 1
            || run.cur_loop.get_num_blocks() != 1
            || run.cur_loop.get_unique_exit_block(&*run.function).is_none()
        {
            return None;
        }

        // It should have a preheader and a branch instruction.
        let preheader = run.cur_loop.get_loop_preheader(run.function)?;
        let ends = run.function.terminator(preheader)?;
        if run.function.instruction(ends).opcode != Opcode::Br {
            return None;
        }

        // The loop exit must be conditioned on an icmp with 0, the null
        // terminator; the icmp operand has to be a load on some SSA register
        // that increments by 1 in the loop.
        let loop_body = run.cur_loop.get_blocks()[0];

        // Skip if the body is too big as it most likely is not a strlen idiom.
        if run.function.iter_block(loop_body).count() >= 15 {
            return None;
        }

        let loop_term = run.function.terminator(loop_body)?;
        if run.function.instruction(loop_term).opcode != Opcode::Br
            || run.function.instruction(loop_term).operands.len() != 3
        {
            return None;
        }
        let Operand::Value(loop_cond) = match_condition(run, loop_term, loop_body)? else { return None };
        let llrm_mir::module::ValueDef::Instruction(load) = run.function.value(loop_cond).def else { return None };
        let Opcode::Load { .. } = run.function.instruction(load).opcode else { return None };
        let pointer = run.function.instruction(load).operands[0];
        let pointer_ty = run.function.operand_type(run.context, pointer)?;
        if !matches!(run.context.types.get(pointer_ty), Type::Pointer(0)) {
            return None;
        }
        let operand_ty = run.function.instruction(load).ty;
        let op_width = run.context.types.int_bits(operand_ty)?;

        // See if the pointer expression is an AddRec with constant step a of
        // form ({n,+,a}) where a is the width of the char type.
        let (load_base, step_size) = {
            let cx = cx!(run);
            let load_ev = run.se.get_scev(&cx, pointer);
            let (base, step, _) = run.se.match_affine_add_rec(load_ev)?;
            (base, run.se.as_constant(step)?.zext_value())
        };

        // Verify that the step is consistent with the platform's char width.
        // D12: only the byte string.
        if op_width != 8 || step_size != 1 {
            return None;
        }

        // Scan every instruction in the loop to ensure there are no side
        // effects.
        for inst in run.function.iter_block(loop_body) {
            if matches!(
                run.function.instruction(inst).opcode,
                Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Load { volatile: true, .. }
            ) {
                return None;
            }
        }

        let exit = run.cur_loop.get_exit_block(run.function)?;
        for phi in run
            .function
            .iter_block(exit)
            .take_while(|&inst| run.function.instruction(inst).opcode == Opcode::Phi)
            .collect::<Vec<_>>()
        {
            let result = Operand::Value(run.function.instruction(phi).result?);
            let cx = cx!(run);
            if !run.se.is_value_scevable(&cx, result) {
                return None;
            }
            // Since we verified that the loop trip count will be a valid
            // strlen idiom, we can expand all lcssa phi with {n,+,1} as
            // (n + strlen) and materialize the loop output with the SCEV
            // expander. Only affine recurrences with a constant step.
            let ev = run.se.get_scev(&cx, result);
            let (_, step, _) = run.se.match_affine_add_rec(ev)?;
            run.se.as_constant(step)?;
        }
        Some(Verified { load_base })
    }
}

/// What the verifier found.
struct Verified {
    load_base: llrm_analysis::scev::expr::Scev,
}

#[cfg(test)]
mod tests {
    use crate::loopidiom::pass::LoopIdiom;
    use crate::testing::{parsed, printed};

    /// jpeg_single's `message`: a loop to the terminator and the length it
    /// leaves. Without the strlen idiom the callee kept its loop, priced 30
    /// against the pin's 45 and 225 against its 337, and five cold call
    /// sites were inlined that the pin refuses.
    const MESSAGE: &str = "define i32 @len(ptr %s) {
entry:
  br label %b2

b2:
  %i = phi i32 [ 0, %entry ], [ %n, %b2 ]
  %p = getelementptr inbounds i8, ptr %s, i32 %i
  %c = load i8, ptr %p
  %n = add nsw i32 %i, 1
  %z = icmp ne i8 %c, 0
  br i1 %z, label %b2, label %exit

exit:
  %r = phi i32 [ %i, %b2 ]
  ret i32 %r
}
";

    fn run(text: &str) -> String {
        run_with(text, true)
    }

    fn run_with(
        text: &str,
        stated: bool,
    ) -> String {
        let mut module = parsed(text);
        let mut passes = llrm_mir::passes::PassManager::default();
        (passes.verify_each, passes.verify_invalidation) = (true, true);
        let routine =
            llrm_mir::libfunc::Routine { standard: "strlen", symbol: "_strlen".to_owned(), calling_convention: 1003 };
        passes.add(LoopIdiom::with_routines(if stated { vec![routine] } else { Vec::new() }));
        passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
        printed(&module)
    }

    #[test]
    fn a_loop_to_the_terminator_is_a_strlen_where_the_compile_states_the_library() {
        let after = run(MESSAGE);
        assert!(after.contains("call cc 1003 i64 @_strlen(ptr %s)"), "{after}");
        assert!(
            after.contains("declare cc 1003 i64 @_strlen(ptr nocapture) memory(argmem: read) nounwind willreturn"),
            "{after}"
        );
    }

    /// tests/run c/builtins_table.c at -m32 -mabi=sysv: the program's own
    /// `strlen` (symbol `_strlen`) was a call of itself, and ran forever. The
    /// pin skips a function named for the idiom; the name here is decorated.
    #[test]
    fn the_loop_of_the_functions_own_strlen_stays_a_loop() {
        let own = MESSAGE.replace("@len", "@_strlen");
        let after = run(&own);
        assert!(!after.contains("call cc 1003"), "{after}");
    }

    /// The pin's x_strlen.ll (functions carry `"no-builtins"`): the pin's
    /// asm had no `strlen` call and ours had two, since only memset and
    /// memcpy read the attribute.
    #[test]
    fn a_function_with_no_builtins_keeps_its_strlen_loop() {
        let text = MESSAGE.replace("define i32 @len(ptr %s) {", "define i32 @len(ptr %s) \"no-builtins\" {");
        let after = run(&text);
        assert!(!after.contains("strlen"), "{after}");
        // The same function without the attribute still forms the call.
        assert!(run(MESSAGE).contains("call cc 1003 i64 @_strlen(ptr %s)"));
    }

    #[test]
    fn a_compile_that_states_no_strlen_keeps_its_loop() {
        let after = run_with(MESSAGE, false);
        assert!(!after.contains("strlen"), "{after}");
    }

    /// x_strlen (Open Watcom, `-fno-builtin`) got a `call _strlen` in a
    /// `"no-builtins"` function, which the pin's TLI forbids (+15
    /// instructions).
    #[test]
    fn a_no_builtins_function_keeps_its_strlen_loop() {
        let text = MESSAGE.replace("@len(ptr %s) {", "@len(ptr %s) \"no-builtins\" {");
        let after = run(&text);
        assert!(!after.contains("strlen"), "{after}");
        assert!(run(MESSAGE).contains("strlen"));
    }

    #[test]
    fn a_loop_over_wider_characters_is_left() {
        let wide = MESSAGE.replace("i8", "i16").replace("i32 %i\n  %c", "i32 %i\n  %c");
        let after = run(&wide);
        assert!(!after.contains("call i32 @strlen"), "{after}");
    }
}
