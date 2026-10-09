//! Selects as branches: the 486 has no conditional move, so each `select
//! c, a, b` becomes a branch on `c` around an empty block into a phi of
//! `a` and `b`, before instruction selection. LLVM's CodeGenPrepare does
//! the same where the target prices a branch below a conditional move.

use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Module, Operand};
use llrm_mir::opcode::{Flags, Opcode};

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
