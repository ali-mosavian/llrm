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
//! An operand of the select that only the select uses, is safe to execute
//! speculatively and costs `TCC_Expensive` or more (a division) moves into the
//! block of its own side, so the other path does not run it: CodeGenPrepare's
//! `sinkSelectOperand`. A cheap one stays, as in LLVM. Where the target states
//! no cost model every instruction costs one, and nothing moves.
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
use llrm_mir::passes::Outer;
use llrm_mir::tti::{Cost, CostKind, Tti};

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

/// What the sinking of a select's operands asks of the target.
pub struct Pricing<'a> {
    pub target: Option<std::rc::Rc<dyn llrm_mir::target::Machine>>,
    pub layout: &'a llrm_mir::datalayout::DataLayout,
    pub tti: &'a dyn Tti,
}

/// Every select of `module`'s functions made a branch and a phi.
pub fn lowered(
    module: &mut Module,
    pricing: &Pricing,
) -> Result<(), String> {
    let outer = Outer::of(module, pricing.target.clone());
    let context = &module.context;
    for global in &mut module.globals {
        let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind else { continue };
        loop {
            let found =
                function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).opcode == Opcode::Select);
            let Some(select) = found else { break };
            let sink =
                |function: &Function, operand: Operand| sunk(context, pricing, &outer, function, select, operand);
            let (yes, no) = (
                sink(function, function.instruction(select).operands[1]),
                sink(function, function.instruction(select).operands[2]),
            );
            _branched(function, select, yes, no)?;
        }
    }
    Ok(())
}

/// `sinkSelectOperand`: `operand` is an instruction only `select` uses, safe
/// to run speculatively and expensive to.
fn sunk(
    context: &llrm_mir::context::Context,
    pricing: &Pricing,
    outer: &Outer,
    function: &Function,
    select: InstId,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    let llrm_mir::module::ValueDef::Instruction(inst) = function.value(value).def else { return None };
    if function.users(value).only().is_none_or(|only| only.user != select) {
        return None;
    }
    if !llrm_transforms::licm::hoist::is_safe_to_speculatively_execute_in(
        context,
        pricing.layout,
        function,
        outer,
        inst,
    ) {
        return None;
    }
    let cost = llrm_mir::instructioncost::instruction_cost(
        pricing.tti,
        context,
        pricing.layout,
        &outer.globals,
        function,
        inst,
        CostKind::SizeAndLatency,
    );
    (!cost.valid || cost.value >= Cost::EXPENSIVE.value).then_some(inst)
}

/// `select` made a branch around an empty block into a phi.
fn _branched(
    function: &mut Function,
    select: InstId,
    sink_yes: Option<InstId>,
    sink_no: Option<InstId>,
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
    if let Some(inst) = sink_yes {
        function.splice(&[inst], Position::Before(jump))?;
    }
    // The other side is a block of its own only where it has an instruction to
    // run.
    let (otherwise, from) = match sink_no {
        Some(inst) => {
            let untaken = function.create_block(None);
            function.insert_block(untaken, Some(taken))?;
            let past =
                function.create_instruction(Opcode::Br, void, vec![Operand::Block(join)], Flags::default(), None);
            function.insert(past, Position::End(untaken))?;
            function.splice(&[inst], Position::Before(past))?;
            (untaken, untaken)
        }
        None => (join, block),
    };
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![condition, Operand::Block(taken), Operand::Block(otherwise)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::End(block))?;
    let phi = function.create_instruction(
        Opcode::Phi,
        op.ty,
        vec![yes, Operand::Block(taken), no, Operand::Block(from)],
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
    use llrm_mir::module::Module;
    use llrm_mir::opcode::Opcode;
    use llrm_mir::tti::{ArithOp, Basic, CastKind, CmpSel, Cost, CostKind, ImmUser, MemoryOp, OperandInfo, Tti, Ty};

    /// `Basic`, where a division costs `Cost::EXPENSIVE`.
    struct Pricey;

    impl Tti for Pricey {
        fn cmp_sel_cost(
            &self,
            a: CmpSel,
            b: Ty,
            c: CostKind,
        ) -> Cost {
            Basic.cmp_sel_cost(a, b, c)
        }
        fn memory_op_cost(
            &self,
            a: MemoryOp,
            b: Ty,
            c: CostKind,
            d: bool,
        ) -> Cost {
            Basic.memory_op_cost(a, b, c, d)
        }
        fn cf_instr_cost(
            &self,
            a: bool,
            b: CostKind,
        ) -> Cost {
            Basic.cf_instr_cost(a, b)
        }
        fn int_imm_cost(
            &self,
            a: i128,
            b: u32,
        ) -> Cost {
            Basic.int_imm_cost(a, b)
        }
        fn int_imm_cost_inst(
            &self,
            a: ImmUser,
            b: u32,
            c: i128,
            d: u32,
        ) -> Cost {
            Basic.int_imm_cost_inst(a, b, c, d)
        }
        fn is_truncate_free(
            &self,
            a: Ty,
            b: Ty,
        ) -> bool {
            Basic.is_truncate_free(a, b)
        }
        fn is_legal_addressing_mode(
            &self,
            a: bool,
            b: i64,
            c: bool,
            d: i64,
        ) -> bool {
            Basic.is_legal_addressing_mode(a, b, c, d)
        }
        fn cast_cost(
            &self,
            a: CastKind,
            b: Ty,
            c: Ty,
            d: CostKind,
        ) -> Cost {
            Basic.cast_cost(a, b, c, d)
        }
        fn arithmetic_cost(
            &self,
            op: ArithOp,
            ty: Ty,
            kind: CostKind,
            a: OperandInfo,
            b: OperandInfo,
        ) -> Cost {
            if matches!(op, ArithOp::UDiv | ArithOp::SDiv) {
                Cost::EXPENSIVE
            } else {
                Basic.arithmetic_cost(op, ty, kind, a, b)
            }
        }
    }

    fn lowered(
        module: &mut Module,
        tti: &dyn Tti,
    ) {
        let layout = module.datalayout.as_deref().map(|text| llrm_mir::datalayout::DataLayout::parse(text).unwrap());
        let layout = layout.unwrap_or_default();
        let pricing = super::Pricing { target: None, layout: &layout, tti };
        super::lowered(module, &pricing).unwrap();
    }

    /// A select between a division and a sum: the division is run on its own
    /// side only, so the other path does not pay it (CodeGenPrepare's
    /// `sinkSelectOperand`). Hoisted by SimplifyCFG's two-entry phi, it ran on
    /// both paths once the select became a branch.
    #[test]
    fn test_an_expensive_operand_only_the_select_uses_runs_on_its_own_side() {
        let text = "define i16 @f(i16 %a, i16 %b) {
entry:
  %q = udiv i16 %a, 3
  %s = add i16 %a, %b
  %c = icmp ult i16 %a, %b
  %r = select i1 %c, i16 %q, i16 %s
  ret i16 %r
}
";
        let before = llrm_mir::parse::module(text).unwrap();
        let mut after = before.clone();
        lowered(&mut after, &Pricey);
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
        let printed = llrm_mir::print::module(&after);
        let function = after.named("f").and_then(|id| after.global(id).function()).expect("f");
        let first = function.walk().next().map(|(block, _)| block).expect("a block");
        let divides = |block| {
            function
                .block(block)
                .instructions()
                .iter()
                .any(
                    |&inst| matches!(
                        function.instruction(inst).opcode,
                        Opcode::Binary(llrm_mir::opcode::BinaryOp::UDiv)
                    ),
                )
        };
        assert!(!divides(first), "{printed}");
        assert!(
            function.walk().any(|(block, inst)| block != first
                && function.instruction(inst).opcode == Opcode::Binary(llrm_mir::opcode::BinaryOp::UDiv)),
            "{printed}"
        );
        for (a, b) in [(3, 5), (9, 2), (4, 4)] {
            let arguments = vec![Val::Int { bits: a, width: 16 }, Val::Int { bits: b, width: 16 }];
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
