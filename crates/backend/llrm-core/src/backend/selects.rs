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
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};

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
pub fn lowered(module: &mut Module) -> Result<(), String> {
    for global in &mut module.globals {
        let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind else { continue };
        loop {
            let found =
                function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).opcode == Opcode::Select);
            let Some(select) = found else { break };
            _branched(function, select)?;
        }
    }
    Ok(())
}

/// `select` made a branch around an empty block into a phi.
fn _branched(
    function: &mut Function,
    select: InstId,
) -> Result<(), String> {
    let block = function.parent(select).ok_or("a placed select")?;
    let op = function.instruction(select).clone();
    let [condition, yes, no] = op.operands[..] else { return Err("a select of three operands".into()) };
    let result = op.result.ok_or("a select's value")?;
    // What follows the select moves to the join, the block's way out with it.
    let body = function.block(block).instructions().to_vec();
    let after = body.iter().position(|&inst| inst == select).expect("its own block") + 1;
    let join = function.create_block(None);
    function.insert_block(join, Some(block))?;
    let moved = body[after..].to_vec();
    function.move_run(&moved, join)?;
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
    let phi = function.create_instruction(
        Opcode::Phi,
        op.ty,
        vec![yes, Operand::Block(taken), no, Operand::Block(block)],
        Flags::default(),
        None,
    );
    let first = function.block(join).instructions()[0];
    function.insert(phi, Position::Before(first))?;
    function.replace_all_uses_with(result, Operand::Value(function.instruction(phi).result.expect("a phi's value")));
    function.erase(select)
}

/// `next`'s phis name `to` where they named `from`.
fn _renamed_edges(
    function: &mut Function,
    next: BlockId,
    from: BlockId,
    to: BlockId,
) {
    let phis = function
        .block(next)
        .instructions()
        .iter()
        .copied()
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
        super::lowered(&mut after).unwrap();
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
