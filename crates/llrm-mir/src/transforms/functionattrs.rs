//! Function attributes inferred, as LLVM's function-attrs pass infers
//! them callees first: `memory(...)` from what a function reads and
//! writes, its own stack aside, and `willreturn` where every loop counts
//! to its bound and every callee comes back.

use crate::callgraph::CallGraph;
use crate::context::{Context, GlobalId};
use crate::dominators::DominatorTree;
use crate::loops::LoopInfo;
use crate::memory::{self, Callees, Effects};
use crate::module::{Function, GlobalKind, Module, Operand, ValueDef};
use crate::opcode::{Attribute, Opcode};
use crate::passes::ModulePass;
use crate::scalarevolution::Evolution;
use crate::valuetracking;

pub struct FunctionAttrs;

impl ModulePass for FunctionAttrs {
    fn name(&self) -> &'static str {
        "function-attrs"
    }

    fn run(&mut self, module: &mut Module) -> Vec<GlobalId> {
        let graph = CallGraph::new(module);
        let layout = match &module.datalayout {
            Some(text) => crate::datalayout::DataLayout::parse(text).unwrap_or_default(),
            None => Default::default(),
        };
        let mut callees = memory::callees(module);
        let mut changed = Vec::new();
        for id in graph.bottom_up() {
            // A cycle of calls proves nothing about itself.
            if graph.reaches(id, id) {
                continue;
            }
            let Module { context, globals, .. } = &mut *module;
            let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { continue };
            let (arguments, other) = accesses(context, &layout, &callees, function);
            let mut added = Vec::new();
            if !memory::argument_memory_only(&function.attrs) && memory::stated(&function.attrs) != Effects::NONE {
                let access = |effects: Effects| match (effects.reads, effects.writes) {
                    (true, true) => "readwrite",
                    (true, false) => "read",
                    (false, true) => "write",
                    (false, false) => "none",
                };
                if other == Effects::NONE && arguments == Effects::NONE {
                    added.push(Attribute::Memory(vec![(None, "none".to_owned())]));
                } else if other == Effects::NONE {
                    added.push(Attribute::Memory(vec![(Some("argmem".to_owned()), access(arguments).to_owned())]));
                }
            }
            if !memory::summary(&function.attrs).returns && returns(context, &callees, function) {
                added.push(Attribute::Flag("willreturn".to_owned()));
            }
            if added.is_empty() {
                continue;
            }
            // A stated `memory(...)` stays: it was never wider than the truth.
            function.attrs.retain(|attr| !matches!(attr, Attribute::Memory(_)) || !added.iter().any(|one| matches!(one, Attribute::Memory(_))));
            function.attrs.extend(added);
            callees.insert(id, memory::summary(&function.attrs));
            changed.push(id);
        }
        changed
    }
}

/// What `function` does to memory its pointer arguments reach, and to any
/// other; its own allocas, gone once it returns, are neither.
fn accesses(context: &Context, layout: &crate::datalayout::DataLayout, callees: &Callees, function: &Function) -> (Effects, Effects) {
    let (mut arguments, mut other) = (Effects::NONE, Effects::NONE);
    // An access through `pointer`: to an argument's memory, another's, or the frame's.
    let reached = |pointer: Operand| -> Option<bool> {
        let (base, _) = valuetracking::underlying(context, layout, function, pointer);
        match base {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Instruction(inst) if matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }) => None,
                ValueDef::Argument(_) => Some(true),
                ValueDef::Instruction(_) => Some(false),
            },
            _ => Some(false),
        }
    };
    let touch = |pointer: Operand, effects: Effects, arguments: &mut Effects, other: &mut Effects| {
        let Some(argument) = reached(pointer) else { return };
        let into = if argument { arguments } else { other };
        into.reads |= effects.reads;
        into.writes |= effects.writes;
    };
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        match &instruction.opcode {
            Opcode::Load { volatile: false, .. } => touch(instruction.operands[0], Effects { reads: true, writes: false }, &mut arguments, &mut other),
            Opcode::Store { volatile: false, .. } => touch(instruction.operands[1], Effects { reads: false, writes: true }, &mut arguments, &mut other),
            Opcode::Call(info) | Opcode::Invoke(info) => {
                let effects = memory::of(context, callees, function, inst);
                let confined = memory::argument_memory_only(&info.attrs)
                    || memory::callee(context, function, inst).and_then(|one| callees.get(&one)).is_some_and(|one| one.arguments_only);
                if confined {
                    let count = instruction.operands.len() - 1;
                    for &operand in &instruction.operands[..count] {
                        if function.operand_type(context, operand).is_some_and(|ty| matches!(context.types.get(ty), crate::types::Type::Pointer(_))) {
                            touch(operand, effects, &mut arguments, &mut other);
                        }
                    }
                } else {
                    other.reads |= effects.reads;
                    other.writes |= effects.writes;
                }
            }
            _ => {
                let effects = memory::of(context, callees, function, inst);
                other.reads |= effects.reads;
                other.writes |= effects.writes;
            }
        }
    }
    (arguments, other)
}

/// Whether `function` always comes back: every callee does, and every loop
/// leaves once a counter stepping by one reaches its bound.
fn returns(context: &Context, callees: &Callees, function: &Function) -> bool {
    let calls_return = function.walk().all(|(_, inst)| match &function.instruction(inst).opcode {
        Opcode::Call(info) => memory::summary(&info.attrs).returns || memory::callee(context, function, inst).and_then(|one| callees.get(&one)).is_some_and(|one| one.returns),
        Opcode::Invoke(_) => false,
        _ => true,
    });
    if !calls_return {
        return false;
    }
    let tree = DominatorTree::new(function);
    let loops = LoopInfo::new(function, &tree);
    let evolution = Evolution::new(context, function, &loops);
    loops.loops.iter().all(|one| crate::scalarevolution::counted(context, function, &tree, &evolution, one).is_some())
}

