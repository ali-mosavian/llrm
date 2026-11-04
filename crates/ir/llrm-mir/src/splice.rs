//! The copy an inlining makes: a call replaced by the body it calls, its
//! returns branching to what followed. Where to inline is
//! `llrm_transforms::inline`'s policy; this is only how.

use crate::context::Context;
use crate::edit::Position;
use crate::hash::HashMap;
use crate::module::{BlockId, Function, InstId, Operand, ValueId};
use crate::opcode::{Flags, Opcode};

/// Whether a copy of `body` can stand in another function: no unwind edge.
/// Stack allocated on entry and of a constant size sits on the caller's entry;
/// any other (`is_static` is false) stays where it is, and the caller brackets
/// the copy with `stacksave`/`stackrestore` (`has_dynamic_alloca`).
pub fn carries(body: &Function) -> bool {
    body.walk().all(|(_, inst)| {
        !matches!(
            body.instruction(inst).opcode,
            Opcode::Invoke(_) | Opcode::LandingPad { .. } | Opcode::Resume
        )
    })
}

/// Whether the alloca `inst` of `body` is on its entry and of a constant size.
fn is_static(
    body: &Function,
    block: BlockId,
    inst: InstId,
) -> bool {
    Some(block) == body.entry() && body.instruction(inst).operands.iter().all(|one| matches!(one, Operand::Constant(_)))
}

/// The address space of the first alloca of `body` that is not static, if any.
pub fn has_dynamic_alloca(body: &Function) -> Option<u32> {
    body.walk().find_map(|(block, inst)| match body.instruction(inst).opcode {
        Opcode::Alloca { address_space, .. } if !is_static(body, block, inst) => Some(address_space),
        _ => None,
    })
}

fn phis(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect()
}

/// `call`, a call to `callee`, replaced by a copy of its body, which
/// `carries`. The instructions of the copy, in the order of the body's.
pub fn splice(
    context: &Context,
    function: &mut Function,
    call: InstId,
    callee: &Function,
) -> Vec<InstId> {
    splice_before(context, function, call, callee, None)
}

/// `splice` where the copy's static allocas go before `anchor`, an instruction
/// of the caller's entry, not before the entry's first: copies made out of
/// order stand as they would made in it.
pub fn splice_before(
    context: &Context,
    function: &mut Function,
    call: InstId,
    callee: &Function,
    anchor: Option<InstId>,
) -> Vec<InstId> {
    spliced(context, function, call, callee, anchor).copies
}

/// A copy `splice_before` made.
pub struct Spliced {
    /// The instructions of the copy, in the order of the body's.
    pub copies: Vec<InstId>,
    /// The copy of the body's entry block.
    pub entry: BlockId,
    /// What stands for each of the body's returns: the branch back, or for a
    /// single return the first instruction after the call.
    pub returns: Vec<InstId>,
}

/// `splice_before`, with where the copy begins and returns.
pub fn spliced(
    context: &Context,
    function: &mut Function,
    call: InstId,
    callee: &Function,
    anchor: Option<InstId>,
) -> Spliced {
    let void = context.types.void();
    let block = function.parent(call).expect("a placed call");
    let instructions = function.iter_block(block).collect::<Vec<_>>();
    let at = instructions.iter().position(|&one| one == call).expect("in its block");
    // What followed the call, and the phis that named its block.
    let rest = function.create_block(None);
    function.insert_block(rest, Some(block)).expect("a placed block");
    function.move_run(&instructions[at + 1..], rest).expect("a placed instruction");
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
    let mut values: HashMap<ValueId, Operand> =
        callee.parameters().iter().copied().zip(arguments.iter().copied()).collect();
    let mut blocks: HashMap<BlockId, BlockId> = HashMap::default();
    let mut last = block;
    for &one in callee.layout() {
        let copy = function.create_block(None);
        function.insert_block(copy, Some(last)).expect("a placed block");
        blocks.insert(one, copy);
        last = copy;
    }
    let first = anchor.unwrap_or_else(|| {
        function.block(function.entry().expect("a defined caller")).first().expect("a block with an instruction")
    });
    let entry = callee.entry().expect("a defined callee");
    let mut made = Vec::new();
    let mut returns = Vec::new();
    let mut returned_from = Vec::new();
    for &one in callee.layout() {
        for inst in callee.iter_block(one) {
            let instruction = callee.instruction(inst);
            if instruction.opcode == Opcode::Ret {
                returns.push((instruction.operands.first().copied(), blocks[&one]));
                let back =
                    function.create_instruction(Opcode::Br, void, vec![Operand::Block(rest)], Flags::default(), None);
                returned_from.push(back);
                function.insert(back, Position::End(blocks[&one])).expect("a placed block");
                continue;
            }
            let name = instruction.result.and_then(|value| callee.value(value).name.clone());
            let copy = function.create_instruction(
                instruction.opcode.clone(),
                instruction.ty,
                Vec::new(),
                instruction.flags,
                name.as_deref(),
            );
            for (kind, node) in &instruction.metadata {
                function.annotate(copy, kind, *node);
            }
            // A static alloca stays static: on the caller's entry.
            let position = match instruction.opcode {
                Opcode::Alloca { .. } if is_static(callee, one, inst) => Position::Before(first),
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
    let copies: Vec<InstId> = made.iter().map(|&(copy, _)| copy).collect();
    for (copy, inst) in made {
        let operands = callee.instruction(inst).operands.iter().map(|&operand| mapped(operand)).collect();
        function.set_operands(copy, operands);
    }
    let enter =
        function.create_instruction(Opcode::Br, void, vec![Operand::Block(blocks[&entry])], Flags::default(), None);
    function.insert(enter, Position::End(block)).expect("a placed block");
    if let Some(result) = function.instruction(call).result {
        let returned: Vec<(Operand, BlockId)> =
            returns.into_iter().map(|(value, from)| (mapped(value.expect("a value returned")), from)).collect();
        let with = match returned[..] {
            [(value, _)] => value,
            _ => {
                let ty = function.value(result).ty;
                let operands = returned.iter().flat_map(|&(value, from)| [value, Operand::Block(from)]).collect();
                let phi = function.create_instruction(Opcode::Phi, ty, operands, Flags::default(), None);
                let head = function.block(rest).first().expect("a block with an instruction");
                function.insert(phi, Position::Before(head)).expect("a placed block");
                Operand::Value(function.instruction(phi).result.expect("a phi's value"))
            }
        };
        function.replace_all_uses_with(result, with);
    }
    function.erase(call).expect("its uses were replaced");
    let mut entry = blocks[&entry];
    let mut returned_from = returned_from;
    // One return: its block's code goes to the head of what followed the call
    // and the block goes, as `InlineFunction` splices ReturnBB into AfterCallBB
    // (L3524); the join keeps AfterCallBB's place in the layout.
    if let [back] = returned_from[..] {
        let from = function.parent(back).expect("a placed return");
        let head = function.block(rest).first().expect("the call's block had a terminator");
        function.erase(back).expect("a branch has no value");
        let run: Vec<InstId> = function.iter_block(from).collect();
        function.splice(&run, Position::Before(head)).expect("placed instructions");
        function.replace_block_uses_with(from, rest);
        function.erase_block(from).expect("an emptied block nothing names");
        if entry == from {
            entry = rest;
        }
        returned_from = vec![head];
    }
    Spliced { copies, entry, returns: returned_from }
}
