//! The copy an inlining makes: a call replaced by the body it calls, its returns
//! branching to what followed. Where to inline is `llrm_transforms::inline`'s
//! policy; this is only how.

use crate::hash::HashMap;

use crate::context::Context;
use crate::edit::Position;
use crate::module::{BlockId, Function, InstId, Operand, ValueId};
use crate::opcode::{Flags, Opcode};

/// Whether a copy of `body` can stand in another function: no unwind edge,
/// and stack allocated only on entry and of a constant size, so the copy
/// can sit on the caller's entry, above anything it could be counted by.
pub fn carries(body: &Function) -> bool {
    let entry = body.entry();
    body.walk().all(|(block, inst)| match &body.instruction(inst).opcode {
        Opcode::Invoke(_) | Opcode::LandingPad { .. } | Opcode::Resume => false,
        Opcode::Alloca { .. } => Some(block) == entry && body.instruction(inst).operands.iter().all(|one| matches!(one, Operand::Constant(_))),
        _ => true,
    })
}

fn phis(function: &Function, block: BlockId) -> Vec<InstId> {
    function.block(block).instructions().iter().copied().take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect()
}

/// `call`, a call to `callee`, replaced by a copy of its body, which
/// `carries`.
pub fn splice(context: &mut Context, function: &mut Function, call: InstId, callee: &Function) {
    let void = context.types.void();
    let block = function.parent(call).expect("a placed call");
    let instructions = function.block(block).instructions().to_vec();
    let at = instructions.iter().position(|&one| one == call).expect("in its block");
    // What followed the call, and the phis that named its block.
    let rest = function.create_block(None);
    function.insert_block(rest, Some(block)).expect("a placed block");
    for &inst in &instructions[at + 1..] {
        function.move_to(inst, Position::End(rest)).expect("a placed instruction");
    }
    for successor in function.successors(rest) {
        for phi in phis(function, successor) {
            for (index, operand) in function.instruction(phi).operands.clone().into_iter().enumerate() {
                if operand == Operand::Block(block) {
                    function.set_operand(phi, index, Operand::Block(rest));
                }
            }
        }
    }
    let arguments = &function.instruction(call).operands;
    let mut values: HashMap<ValueId, Operand> = callee.parameters().iter().copied().zip(arguments.iter().copied()).collect();
    let mut blocks: HashMap<BlockId, BlockId> = HashMap::default();
    let mut last = block;
    for &one in callee.layout() {
        let copy = function.create_block(None);
        function.insert_block(copy, Some(last)).expect("a placed block");
        blocks.insert(one, copy);
        last = copy;
    }
    let first = function.block(function.entry().expect("a defined caller")).instructions()[0];
    let entry = callee.entry().expect("a defined callee");
    let mut made = Vec::new();
    let mut returns = Vec::new();
    for &one in callee.layout() {
        for &inst in callee.block(one).instructions() {
            let instruction = callee.instruction(inst);
            if instruction.opcode == Opcode::Ret {
                returns.push((instruction.operands.first().copied(), blocks[&one]));
                let back = function.create_instruction(Opcode::Br, void, vec![Operand::Block(rest)], Flags::default(), None);
                function.insert(back, Position::End(blocks[&one])).expect("a placed block");
                continue;
            }
            let name = instruction.result.and_then(|value| callee.value(value).name.clone());
            let copy = function.create_instruction(instruction.opcode.clone(), instruction.ty, Vec::new(), instruction.flags, name.as_deref());
            for (kind, node) in &instruction.metadata {
                function.annotate(copy, kind, *node);
            }
            // A static alloca stays static: on the caller's entry.
            let position = match instruction.opcode {
                Opcode::Alloca { .. } if one == entry => Position::Before(first),
                _ => Position::End(blocks[&one]),
            };
            function.insert(copy, position).expect("a placed block");
            if let (Some(old), Some(new)) = (instruction.result, function.instruction(copy).result) {
                values.insert(old, Operand::Value(new));
            }
            made.push((copy, inst));
        }
    }
    let mapped = |operand: Operand| match operand {
        Operand::Value(value) => values[&value],
        Operand::Block(one) => Operand::Block(blocks[&one]),
        constant => constant,
    };
    for (copy, inst) in made {
        let operands = callee.instruction(inst).operands.iter().map(|&operand| mapped(operand)).collect();
        function.set_operands(copy, operands);
    }
    let enter = function.create_instruction(Opcode::Br, void, vec![Operand::Block(blocks[&entry])], Flags::default(), None);
    function.insert(enter, Position::End(block)).expect("a placed block");
    if let Some(result) = function.instruction(call).result {
        let returned: Vec<(Operand, BlockId)> = returns.into_iter().map(|(value, from)| (mapped(value.expect("a value returned")), from)).collect();
        let with = match returned[..] {
            [(value, _)] => value,
            _ => {
                let ty = function.value(result).ty;
                let operands = returned.iter().flat_map(|&(value, from)| [value, Operand::Block(from)]).collect();
                let phi = function.create_instruction(Opcode::Phi, ty, operands, Flags::default(), None);
                let head = function.block(rest).instructions()[0];
                function.insert(phi, Position::Before(head)).expect("a placed block");
                Operand::Value(function.instruction(phi).result.expect("a phi's value"))
            }
        };
        function.replace_all_uses_with(result, with);
    }
    function.erase(call).expect("its uses were replaced");
}
