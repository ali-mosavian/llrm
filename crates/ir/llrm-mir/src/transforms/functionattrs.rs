//! Function attributes inferred, as LLVM's function-attrs pass infers
//! them callees first: `memory(...)` from what a function reads and
//! writes, its own stack aside, and `willreturn` where every loop counts
//! to its bound and every callee comes back.

use crate::callgraph::CallGraphAnalysis;
use crate::context::{Context, GlobalId};
use crate::dominators::DominatorTree;
use crate::loops::LoopInfo;
use crate::memory::{self, Callees, Effects};
use crate::module::{Function, GlobalKind, Module, Operand, ValueDef};
use crate::opcode::{Attribute, Opcode};
use crate::passes::{CalleeEffects, ModuleAnalyses, ModulePass};
use crate::scalarevolution::Evolution;
use crate::valuetracking;

pub struct FunctionAttrs;

impl ModulePass for FunctionAttrs {
    fn name(&self) -> &'static str {
        "function-attrs"
    }

    fn run(&mut self, module: &mut Module, analyses: &mut ModuleAnalyses) -> Vec<GlobalId> {
        let graph = analyses.get::<CallGraphAnalysis>(module);
        let layout = analyses.program().layout.clone();
        let mut callees = (*analyses.get::<CalleeEffects>(module)).clone();
        let mut changed = Vec::new();
        for id in graph.bottom_up() {
            // A cycle of calls proves nothing about itself.
            if graph.reaches(id, id) {
                continue;
            }
            let quiet = recurses_never(module, &graph, id);
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
            if !memory::returns(&function.attrs) && returns(context, &callees, function, quiet) {
                added.push(Attribute::Flag("willreturn".to_owned()));
            }
            if quiet && !function.attrs.iter().any(|one| matches!(one, Attribute::Flag(name) if name == "norecurse")) {
                added.push(Attribute::Flag("norecurse".to_owned()));
            }
            if added.is_empty() {
                continue;
            }
            // A stated `memory(...)` stays: it was never wider than the truth.
            function.attrs.retain(|attr| !matches!(attr, Attribute::Memory(_)) || !added.iter().any(|one| matches!(one, Attribute::Memory(_))));
            function.attrs.extend(added);
            let memset = callees.get(&id).is_some_and(|one| one.memset);
            callees.insert(id, memory::Summary { memset, ..memory::summary(function) });
            changed.push(id);
        }
        changed
    }
}

/// Whether `id` can never be entered again while it runs: it is in no cycle of
/// calls, and neither it nor what it reaches calls a function the graph does
/// not bound, or one outside the module that may call back (a declaration not
/// `nocallback`), as LLVM's `addNoRecurseAttrs`.
fn recurses_never(module: &Module, graph: &crate::callgraph::CallGraph, id: GlobalId) -> bool {
    if graph.reaches(id, id) {
        return false;
    }
    let mut over = graph.reachable(id);
    over.insert(id);
    over.into_iter().all(|at| {
        let GlobalKind::Function(function) = &module.globals[at.0 as usize].kind else { return true };
        if function.is_declaration() {
            return function.attrs.iter().any(|one| matches!(one, Attribute::Flag(name) if name == "nocallback")) || module.globals[at.0 as usize].name.as_deref().and_then(crate::intrinsics::Intrinsic::named).is_some_and(|one| !matches!(one, crate::intrinsics::Intrinsic::Code | crate::intrinsics::Intrinsic::Asm));
        }
        !graph.calls_unknown(at)
    })
}

/// Whether `function` does nothing a loop that never ended could be seen by:
/// no volatile access and no call that writes.
fn unobserved(context: &Context, callees: &Callees, function: &Function) -> bool {
    function.walk().all(|(_, inst)| match &function.instruction(inst).opcode {
        Opcode::Load { volatile, .. } | Opcode::Store { volatile, .. } => !volatile,
        Opcode::Call(_) | Opcode::Invoke(_) => !memory::of(context, callees, function, inst).writes,
        _ => true,
    })
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
/// leaves once a counter stepping by one reaches its bound, or the language
/// promises it (`mustprogress`) of a function that cannot re-enter itself
/// and does nothing observable: a loop of such a function that never ended
/// would be undefined.
fn returns(context: &Context, callees: &Callees, function: &Function, norecurse: bool) -> bool {
    let calls_return = function.walk().all(|(_, inst)| match &function.instruction(inst).opcode {
        Opcode::Call(info) => memory::returns(&info.attrs) || memory::callee(context, function, inst).and_then(|one| callees.get(&one)).is_some_and(|one| one.returns),
        Opcode::Invoke(_) => false,
        _ => true,
    });
    if !calls_return {
        return false;
    }
    if norecurse && crate::facts::Facts::of(&function.attrs).must_progress() && unobserved(context, callees, function) {
        return true;
    }
    let tree = DominatorTree::new(function);
    let loops = LoopInfo::new(function, &tree);
    let evolution = Evolution::new(context, function, &loops);
    loops.loops.iter().all(|one| crate::scalarevolution::counted(context, function, &tree, &evolution, one).is_some())
}

