//! Turns each MIR `select` into a branch around an empty block and a phi, where
//! the target has no conditional move.
//!
//! # How it works
//! `select c, a, b` splits its block at the select. The instructions after it
//! move to a new join block, which takes over the block's way out (the phis of
//! its successors are renamed to name the join). A new block `taken` holds only
//! a jump to the join. The original block ends in a branch on `c` to `taken` or
//! the join, and the join starts with `phi [a, taken], [b, block]`, which
//! replaces the select's uses:
//!
//! ```text
//!  block:  ...                   block:  ...
//!          %r = select c, a, b           br c, taken, join
//!          rest                  taken:  br join
//!                                join:   %r = phi [a, taken], [b, block]
//!                                        rest
//! ```
//!
//! DEVIATE S2 (no number): the pin keeps the select to instruction selection.
//! CodeGenPrepare's `optimizeSelectInst` forms no branch where
//! `isPredictableSelectExpensive` is false (in-order i486, `CodeGenPrepare.cpp`
//! `isFormingBranchFromSelectProfitable`), and `X86TargetLowering::
//! EmitLoweredSelect` then expands the select to a diamond on machine IR, with
//! both operands computed before it. Ours expands in MIR. The operand sinking
//! of `sinkSelectOperand`, which the pin does not do here, is deleted: it
//! moved nothing over 834 objects. Exit: the expansion at selection, the
//! registry slot `emit-lowered-select` (lir-cleanup-port, PR by 2026-11-13;
//! #3052).
//!
//! The loop is repeated until no select is left in any function, so two selects
//! in one block each get their own split. The result is checked by an
//! interpreter test that runs a function before and after.
//!
//! # Example
//! Before:
//!
//! ```mir
//! define i16 @f(i16 %a, i16 %b) {
//! entry:
//!   %less = icmp ult i16 %a, %b
//!   %least = select i1 %less, i16 %a, i16 %b
//!   %twice = add i16 %least, %least
//!   br label %done
//! done:
//!   %r = phi i16 [ %twice, %entry ]
//!   ret i16 %r
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define i16 @f(i16 %a, i16 %b) {
//! entry:
//!   %less = icmp ult i16 %a, %b
//!   br i1 %less, label %0, label %1
//! 0:
//!   br label %1
//! 1:
//!   %2 = phi i16 [ %a, %0 ], [ %b, %entry ]
//!   %twice = add i16 %2, %2
//!   br label %done
//! done:
//!   %r = phi i16 [ %twice, %1 ]
//!   ret i16 %r
//! }
//! ```
//!
//! The select became a branch to the empty block `0` and a phi in `1`; the phi
//! of `done` now names `1` as its predecessor.
//!
//! From unit test `test_a_select_becomes_a_branch_and_a_phi` in `selects.rs`.
//!
//! # Runs
//! Once per program in `driver::optimized`, after `fpconvert` and before the
//! spill-model dump, only where the target's `datalayout.toml` lists `select`
//! under `expand` (m16 and m32: no conditional move before the Pentium Pro).
//! There is no flag; it runs at every -O level.
//!
//! # See also
//! LLVM's CodeGenPrepare (`optimizeSelectInst`) makes the same change where the
//! target prices a branch below a conditional move.

use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, Function, GlobalKind, InstId, Module, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::tti::Tti;

/// Every `llvm.{s,u}{min,max}` call of `module` made a compare and a select,
/// as LLVM expands ISD::SMAX where the target has no such instruction.
pub fn minmax_expanded(module: &mut Module) -> Result<(), String> {
    let names: Vec<Option<String>> = module.globals.iter().map(|global| global.name.clone()).collect();
    let context = &mut module.context;
    for global in &mut module.globals {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        let found: Vec<(InstId, bool, bool)> = function
            .walk()
            .filter_map(|(_, inst)| {
                let instruction = function.instruction(inst);
                let Some(&Operand::Constant(callee)) = instruction.operands.last() else { return None };
                let ConstantKind::Global(callee) = context.get(callee).kind else { return None };
                let name = names[callee.0 as usize].as_deref()?;
                match (matches!(instruction.opcode, Opcode::Call(_)), Intrinsic::named(name)) {
                    (true, Some(Intrinsic::MinMax { signed, max })) => Some((inst, signed, max)),
                    _ => None,
                }
            })
            .collect();
        for (inst, signed, max) in found {
            let call = function.instruction(inst).clone();
            let [a, b, _] = call.operands[..] else { return Err("a min or max of two operands".into()) };
            let bit = context.types.int(1);
            let predicate = match (signed, max) {
                (true, true) => IntPredicate::Sgt,
                (true, false) => IntPredicate::Slt,
                (false, true) => IntPredicate::Ugt,
                (false, false) => IntPredicate::Ult,
            };
            let before = |function: &mut Function, opcode, ty, operands: Vec<Operand>| -> Result<Operand, String> {
                let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
                function.insert(made, Position::Before(inst))?;
                Ok(Operand::Value(function.instruction(made).result.expect("a value")))
            };
            let beats = before(function, Opcode::ICmp(predicate), bit, vec![a, b])?;
            let chosen = before(function, Opcode::Select, call.ty, vec![beats, a, b])?;
            function.replace_all_uses_with(call.result.ok_or("a min or max value")?, chosen);
            function.erase(inst)?;
        }
    }
    Ok(())
}

/// Every select of `module`'s functions made a branch and a phi.
pub fn lowered(
    module: &mut Module,
    tti: &dyn Tti,
    wanted: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    let context = &module.context;
    for global in &mut module.globals {
        let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind else { continue };
        // Arithmetic on the condition where the constants allow it, as the
        // DAG combiner makes it before any conditional move.
        let all: Vec<InstId> = function
            .walk()
            .map(|(_, inst)| inst)
            .filter(|&inst| function.instruction(inst).opcode == Opcode::Select)
            .collect();
        for select in all {
            if wanted(&context.types.display(function.instruction(select).ty).to_string()) {
                if !crate::backend::selectconstants::boolean(context, function, select)? {
                    crate::backend::selectconstants::folded(
                        context,
                        function,
                        select,
                        tti.convert_select_of_constants_to_math(),
                    )?;
                }
            }
        }
        loop {
            let found = function
                .walk()
                .map(|(_, inst)| inst)
                .find(
                    |&inst| function.instruction(inst).opcode == Opcode::Select
                        && wanted(&context.types.display(function.instruction(inst).ty).to_string()),
                );
            let Some(select) = found else { break };
            // `EmitLoweredSelect` case 1: the selects on one condition are
            // lowered together, by one branch. The pin's scheduler puts them in
            // a row; here an instruction between that reads none of their
            // results stays above the branch, and the rest follows the join.
            let (run, join_side) = _run(context, function, select, wanted);
            _branched(function, &run, &join_side)?;
        }
    }
    Ok(())
}

/// The selects that `EmitLoweredSelect` lowers with `select` (same condition,
/// operands not made after it) and the instructions of the block that follow
/// the join, terminator last.
fn _run(
    context: &llrm_mir::context::Context,
    function: &Function,
    select: InstId,
    wanted: &dyn Fn(&str) -> bool,
) -> (Vec<InstId>, Vec<InstId>) {
    let block = function.parent(select).expect("a placed select");
    let condition = function.instruction(select).operands[0];
    let following: Vec<InstId> = function.iter_block(block).skip_while(|&inst| inst != select).skip(1).collect();
    let mut run = vec![select];
    // What is only known after the branch: the selects' results and what reads them.
    let mut group: Vec<Operand> = function.instruction(select).result.map(Operand::Value).into_iter().collect();
    let mut after: Vec<Operand> = Vec::new();
    let mut effects = false;
    // The independent, pure instructions between: kept above the branch only
    // where a select of the run reads them, else they follow the join with
    // the code that reads them (a compare feeding the block's branch fuses
    // with it there).
    let mut kept: Vec<InstId> = Vec::new();
    for (index, &inst) in following.iter().enumerate() {
        let one = function.instruction(inst);
        if index + 1 == following.len() {
            break;
        }
        let reads = |among: &[Operand]| one.operands.iter().any(|operand| among.contains(operand));
        if one.opcode == Opcode::Select
            && one.operands[0] == condition
            && wanted(&context.types.display(one.ty).to_string())
            && !one.operands[1..].iter().any(|operand| after.contains(operand))
        {
            run.push(inst);
            group.extend(one.result.map(Operand::Value));
            continue;
        }
        let pure = matches!(
            one.opcode,
            Opcode::ICmp(_) | Opcode::Select | Opcode::Cast(_) | Opcode::GetElementPtr { .. }
        ) || matches!(
            one.opcode,
            Opcode::Binary(
                BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Shl
                    | BinaryOp::LShr
                    | BinaryOp::AShr
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Xor
            )
        );
        if reads(&group) || reads(&after) || effects || !pure {
            effects |= !pure;
            after.extend(one.result.map(Operand::Value));
        } else {
            kept.push(inst);
        }
    }
    let mut needed: Vec<Operand> = run.iter().flat_map(|&inst| function.instruction(inst).operands[1..].to_vec()).collect();
    let mut above: Vec<InstId> = Vec::new();
    for &inst in kept.iter().rev() {
        let one = function.instruction(inst);
        if one.result.is_some_and(|result| needed.contains(&Operand::Value(result))) {
            needed.extend(one.operands.iter().copied());
            above.push(inst);
        }
    }
    let join_side: Vec<InstId> =
        following.iter().copied().filter(|inst| !run.contains(inst) && !above.contains(inst)).collect();
    (run, join_side)
}

/// The selects of `run` (in a row, on one condition) made a branch around an
/// empty block and a phi each (`createPHIsForCMOVsInSinkBB`).
fn _branched(
    function: &mut Function,
    run: &[InstId],
    moved: &[InstId],
) -> Result<(), String> {
    let select = run[0];
    let block = function.parent(select).ok_or("a placed select")?;
    let first = function.instruction(select).clone();
    let condition = first.operands[0];
    // What follows the run moves to the join, the block's way out with it.
    let join = function.create_block(None);
    function.insert_block(join, Some(block))?;
    function.move_run(moved, join)?;
    for next in function.successors(join) {
        _renamed_edges(function, next, block, join);
    }
    let taken = function.create_block(None);
    function.insert_block(taken, Some(block))?;
    let void = function.instruction(*moved.last().expect("a terminator")).ty;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(join)], Flags::default(), None);
    function.insert(jump, Position::End(taken))?;
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![condition, Operand::Block(taken), Operand::Block(join)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::End(block))?;
    // What each result is on each edge: a phi may not read an earlier phi of its
    // own block, so a select reading an earlier one reads what that was on the
    // edge.
    let mut edges: Vec<(Operand, Operand, Operand)> = Vec::new();
    let at_top = function.block(join).first().expect("a block with instructions");
    for &inst in run {
        let op = function.instruction(inst).clone();
        let [_, yes, no] = op.operands[..] else { return Err("a select of three operands".into()) };
        let result = op.result.ok_or("a select's value")?;
        let renamed = |value: Operand, taken_edge: bool| {
            edges
                .iter()
                .find(|(earlier, _, _)| *earlier == value)
                .map_or(value, |&(_, yes, no)| if taken_edge { yes } else { no })
        };
        let (yes, no) = (renamed(yes, true), renamed(no, false));
        let phi = function.create_instruction(
            Opcode::Phi,
            op.ty,
            vec![yes, Operand::Block(taken), no, Operand::Block(block)],
            Flags::default(),
            None,
        );
        function.insert(phi, Position::Before(at_top))?;
        let made = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        edges.push((Operand::Value(result), yes, no));
        function.replace_all_uses_with(result, made);
    }
    for &inst in run {
        function.erase(inst)?;
    }
    Ok(())
}

/// `next`'s phis name `to` where they named `from`.
fn _renamed_edges(
    function: &mut Function,
    next: BlockId,
    from: BlockId,
    to: BlockId,
) {
    let phis = function
        .iter_block(next)
        .take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .collect::<Vec<_>>();
    for phi in phis {
        let operands = function
            .instruction(phi)
            .operands
            .iter()
            .map(|&one| if one == Operand::Block(from) { Operand::Block(to) } else { one })
            .collect();
        function.set_operands(phi, operands);
    }
}

#[cfg(test)]
mod tests {
    use llrm_mir::interpret::{Val, run};
    use llrm_mir::module::Module;
    use llrm_mir::tti::Tti;

    fn lowered(
        module: &mut Module,
        tti: &dyn Tti,
    ) {
        super::lowered(module, tti, &|_| true).unwrap();
    }

    fn same_answers(
        before: &Module,
        after: &Module,
        inputs: &[(i128, i128)],
    ) {
        for &(a, b) in inputs {
            let arguments = |one: i128, other: i128| {
                vec![Val::Int { bits: one as u128 & 0xFFFF, width: 16 }, Val::Int { bits: other as u128 & 0xFFFF, width: 16 }]
            };
            assert_eq!(
                run(after, "f", arguments(a, b), 100).unwrap(),
                run(before, "f", arguments(a, b), 100).unwrap(),
                "{a}, {b}"
            );
        }
    }

    fn conditional_branches(module: &Module) -> usize {
        llrm_mir::print::module(module).lines().filter(|line| line.trim_start().starts_with("br i1")).count()
    }

    /// The two selects of isqrt's step test one condition, with a subtract
    /// between them. Each made its own branch, and the second tested a flag
    /// the first block had materialised (`setae al; test al, al`), where the
    /// pin's `EmitLoweredSelect` lowers the pair with one branch: x_isqrt
    /// executed 2x the instructions.
    #[test]
    fn test_selects_on_one_condition_share_one_branch() {
        let text = "define i16 @f(i16 %a, i16 %b) {
entry:
  %c = icmp uge i16 %a, %b
  %p = select i1 %c, i16 %b, i16 0
  %d = sub i16 %a, %p
  %q = select i1 %c, i16 %a, i16 0
  %r = add i16 %q, %d
  ret i16 %r
}
";
        let before = llrm_mir::parse::module(text).unwrap();
        let mut after = before.clone();
        lowered(&mut after, &llrm_mir::tti::Basic);
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
        assert!(!llrm_mir::print::module(&after).contains("select"));
        assert_eq!(conditional_branches(&after), 1, "{}", llrm_mir::print::module(&after));
        same_answers(&before, &after, &[(3, 5), (9, 2), (4, 4), (0, 0), (65535, 1)]);
    }

    /// `select x, true, y` is `x | y` (`foldBoolSelectToLogic`): queens' `||`
    /// chain made a branch per select and a setcc-and-test between them.
    #[test]
    fn test_a_boolean_select_is_a_logic_operation() {
        let text = "define i1 @f(i16 %a, i16 %b) {
entry:
  %x = icmp eq i16 %a, 0
  %y = icmp eq i16 %a, %b
  %o = select i1 %x, i1 true, i1 %y
  %z = icmp ult i16 %a, %b
  %p = select i1 %o, i1 true, i1 %z
  ret i1 %p
}
";
        let before = llrm_mir::parse::module(text).unwrap();
        let mut after = before.clone();
        lowered(&mut after, &llrm_mir::tti::Basic);
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
        assert_eq!(conditional_branches(&after), 0, "{}", llrm_mir::print::module(&after));
        for (a, b) in [(0, 5), (3, 3), (4, 9), (9, 4)] {
            let arguments =
                vec![Val::Int { bits: a, width: 16 }, Val::Int { bits: b, width: 16 }];
            assert_eq!(run(&after, "f", arguments.clone(), 100).unwrap(), run(&before, "f", arguments, 100).unwrap());
        }
    }

    /// `llvm.smax.i16` and its kin, which SCEV's exit values and InstCombine
    /// now emit, were refused by isel (item.c, cmd.c, mdl_ai.c): a compare and
    /// a select, with the same answers.
    #[test]
    fn test_a_min_or_max_becomes_a_compare_and_a_select() {
        let text = "declare i16 @llvm.smax.i16(i16, i16)
declare i16 @llvm.smin.i16(i16, i16)
declare i16 @llvm.umax.i16(i16, i16)
declare i16 @llvm.umin.i16(i16, i16)
define i16 @f(i16 %a, i16 %b) {
entry:
  %v1 = call i16 @llvm.smax.i16(i16 %a, i16 %b)
  %v2 = call i16 @llvm.smin.i16(i16 %a, i16 %b)
  %v3 = call i16 @llvm.umax.i16(i16 %a, i16 %b)
  %v4 = call i16 @llvm.umin.i16(i16 %a, i16 %b)
  %v5 = mul i16 %v1, 3
  %v6 = mul i16 %v2, 5
  %v7 = mul i16 %v3, 7
  %v8 = mul i16 %v4, 11
  %v9 = add i16 %v5, %v6
  %v10 = add i16 %v9, %v7
  %v11 = add i16 %v10, %v8
  ret i16 %v11
}
";
        let before = llrm_mir::parse::module(text).unwrap();
        let mut after = before.clone();
        super::minmax_expanded(&mut after).unwrap();
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
        assert!(!llrm_mir::print::module(&after).contains("= call"));
        for (a, b) in [(3, 5), (-9, 2), (4, 4), (-1, -7), (0x8000, 1)] {
            let arguments = |one: i128, other: i128| {
                vec![
                    Val::Int { bits: one as u128 & 0xFFFF, width: 16 },
                    Val::Int { bits: other as u128 & 0xFFFF, width: 16 },
                ]
            };
            assert_eq!(
                run(&after, "f", arguments(a, b), 100).unwrap(),
                run(&before, "f", arguments(a, b), 100).unwrap()
            );
        }
    }

    /// A select of the lesser of two, then a loop on it: the same answers
    /// as a branch and a phi, and no select left. Instruction selection
    /// refused Nib's `zip` once its exits became one on their least count.
    #[test]
    fn test_a_select_becomes_a_branch_and_a_phi() {
        let text = "define i16 @f(i16 %a, i16 %b) {
entry:
  %less = icmp ult i16 %a, %b
  %least = select i1 %less, i16 %a, i16 %b
  %twice = add i16 %least, %least
  br label %done
done:
  %r = phi i16 [ %twice, %entry ]
  ret i16 %r
}
";
        let before = llrm_mir::parse::module(text).unwrap();
        let mut after = before.clone();
        lowered(&mut after, &llrm_mir::tti::Basic);
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
        assert!(!llrm_mir::print::module(&after).contains("select"));
        for (a, b) in [(3, 5), (9, 2), (4, 4)] {
            let arguments = |one: i128, other: i128| {
                vec![Val::Int { bits: one as u128, width: 16 }, Val::Int { bits: other as u128, width: 16 }]
            };
            assert_eq!(
                run(&after, "f", arguments(a, b), 100).unwrap(),
                run(&before, "f", arguments(a, b), 100).unwrap()
            );
        }
    }
}
