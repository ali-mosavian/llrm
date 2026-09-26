//! Inlining, as LLVM's inliner does it bottom-up over the call graph: a
//! direct call to a defined function no bigger than the threshold becomes
//! a copy of its body, its returns branching to what followed the call.
//! A call within a cycle of calls stays.

use std::collections::HashMap;

use crate::callgraph::CallGraph;
use crate::context::{Context, GlobalId};
use crate::memory::callee;
use crate::edit::Position;
use crate::module::{BlockId, Function, GlobalKind, InstId, Module, Operand, ValueId};
use crate::opcode::{Attribute, Flags, Opcode};
use crate::passes::ModulePass;
use crate::types::Type;

/// LLVM's default threshold of 225, at its 5 per instruction.
const THRESHOLD: usize = 45;

pub struct Inline;

impl ModulePass for Inline {
    fn name(&self) -> &'static str {
        "inline"
    }

    fn run(&mut self, module: &mut Module) -> Vec<GlobalId> {
        let graph = CallGraph::new(module);
        let mut changed = Vec::new();
        for caller in graph.bottom_up() {
            let mut inlined = false;
            while let Some((call, callee)) = site(module, &graph, caller) {
                let GlobalKind::Function(body) = &module.globals[callee.0 as usize].kind else { unreachable!("a function") };
                let body = (**body).clone();
                let Module { context, globals, .. } = &mut *module;
                let GlobalKind::Function(function) = &mut globals[caller.0 as usize].kind else { unreachable!("a function") };
                inline(context, function, call, &body);
                inlined = true;
            }
            if inlined {
                changed.push(caller);
            }
        }
        changed
    }
}

/// A call in `caller` worth inlining, and its callee.
fn site(module: &Module, graph: &CallGraph, caller: GlobalId) -> Option<(InstId, GlobalId)> {
    let function = module.global(caller).function()?;
    function.walk().find_map(|(_, inst)| {
        let callee = callee(&module.context, function, inst)?;
        let body = module.global(callee).function().filter(|one| !one.is_declaration())?;
        let Opcode::Call(info) = &function.instruction(inst).opcode else { return None };
        let fits = info.function_type == body.ty
            && !matches!(module.context.types.get(body.ty), Type::Function { variadic: true, .. })
            && !body.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(flag) if flag == "noinline" || flag == "optnone"))
            && callee != caller
            && !graph.reaches(callee, caller)
            && inlinable(body);
        fits.then_some((inst, callee))
    })
}

/// Small enough, and nothing a copy cannot carry: an unwind edge, or
/// stack allocated anywhere but on entry.
fn inlinable(body: &Function) -> bool {
    let entry = body.entry();
    let mut count = 0;
    for (block, inst) in body.walk() {
        count += 1;
        match body.instruction(inst).opcode {
            Opcode::Invoke(_) | Opcode::LandingPad { .. } | Opcode::Resume => return false,
            Opcode::Alloca { .. } if Some(block) != entry => return false,
            _ => {}
        }
    }
    count <= THRESHOLD
}

fn phis(function: &Function, block: BlockId) -> Vec<InstId> {
    function.block(block).instructions().iter().copied().take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect()
}

/// `call`, a call to `callee`, replaced by a copy of its body.
fn inline(context: &mut Context, function: &mut Function, call: InstId, callee: &Function) {
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
    let mut blocks: HashMap<BlockId, BlockId> = HashMap::new();
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
            let copy = function.create_instruction(instruction.opcode.clone(), instruction.ty, Vec::new(), instruction.flags, None);
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
