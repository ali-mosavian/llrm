//! LLVM's GlobalOpt gives an internal function whose every caller it sees a calling convention
//! of its own (`fastcc`). Here that is the callee popping its arguments: `ret N` (3 bytes
//! against `ret`'s 1) once, and no `add sp,N` (3 bytes, `pop cx` for one word) at each call.
//! It is faster too, a clock per call. The convention is assigned here and nowhere else: isel
//! takes both sides' cleanup from it (`passing`), so a call and the function it names agree.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::globalsaa::embedded;
use llrm_mir::context::{ConstantKind, GlobalId};
use llrm_mir::module::{GlobalKind, InstId, Linkage, MetadataOperand, Module, Operand};
use llrm_mir::opcode::{FAST, Opcode};
use llrm_mir::passes::{ModuleAnalyses, ModulePass};

pub struct CalleePop;

impl ModulePass for CalleePop {
    fn name(&self) -> &'static str {
        "calleepop"
    }

    fn run(&mut self, module: &mut Module, _: &mut ModuleAnalyses) -> Vec<GlobalId> {
        let (chosen, sites) = decided(module);
        let mut changed = BTreeSet::new();
        for &id in &chosen {
            if let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind {
                function.calling_convention = FAST;
                changed.insert(id);
            }
        }
        for (caller, inst) in sites {
            if let GlobalKind::Function(function) = &mut module.globals[caller.0 as usize].kind {
                function.set_call_convention(inst, FAST);
                changed.insert(caller);
            }
        }
        changed.into_iter().collect()
    }
}

/// The bytes a call saves of its cleanup, from at least `words` words of arguments: `pop cx`
/// for one, two pops for two, `add sp,N` otherwise.
fn cleanup_bytes(words: usize) -> usize {
    match words {
        1 => 1,
        2 => 2,
        _ => 3,
    }
}

/// The functions to give the convention, and the calls of them (caller, instruction) that
/// take it: internal, not variadic, with arguments, called directly and never otherwise named
/// (stored, passed, listed in a table or in a call's `callees`), and called often enough that
/// the `ret N` it costs each return is paid for.
fn decided(module: &Module) -> (BTreeSet<GlobalId>, Vec<(GlobalId, InstId)>) {
    let candidate = |id: GlobalId| {
        let global = module.global(id);
        let Some(function) = global.function() else { return false };
        let (_, parameters, variadic) = module.signature(function.ty);
        !function.is_declaration()
            && matches!(global.linkage, Linkage::Internal | Linkage::Private)
            && function.calling_convention == 0
            && !variadic
            && !parameters.is_empty()
    };
    let mut escaped: BTreeSet<GlobalId> = BTreeSet::new();
    let mut calls: BTreeMap<GlobalId, Vec<(GlobalId, InstId)>> = BTreeMap::new();
    for global in &module.globals {
        if let GlobalKind::Variable(variable) = &global.kind {
            variable.initializer.iter().for_each(|&one| embedded(&module.context, one, &mut escaped));
        }
    }
    for node in &module.metadata {
        for operand in &node.operands {
            if let MetadataOperand::Constant(one) = operand {
                embedded(&module.context, *one, &mut escaped);
            }
        }
    }
    for (caller, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
        function.personality.iter().for_each(|&one| embedded(&module.context, one, &mut escaped));
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            let direct = match &instruction.opcode {
                Opcode::Call(info) | Opcode::Invoke(info) => match instruction.operands.last() {
                    Some(Operand::Constant(id)) => match module.context.get(*id).kind {
                        ConstantKind::Global(callee) if info.calling_convention == 0 => Some(callee),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            };
            let named = instruction.operands.len() - usize::from(direct.is_some());
            for operand in &instruction.operands[..named] {
                if let Operand::Constant(one) = operand {
                    embedded(&module.context, *one, &mut escaped);
                }
            }
            if let Some(callee) = direct {
                calls.entry(callee).or_default().push((caller, inst));
            }
        }
    }
    let chosen: BTreeSet<GlobalId> = calls
        .iter()
        .filter(|(callee, sites)| {
            let callee = **callee;
            let function = module.global(callee).function().expect("a candidate is a function");
            let (_, parameters, _) = module.signature(function.ty);
            let returns = function.walk().filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Ret)).count();
            candidate(callee) && !escaped.contains(&callee) && sites.len() * cleanup_bytes(parameters.len()) > 2 * returns
        })
        .map(|(&callee, _)| callee)
        .collect();
    let sites = chosen.iter().flat_map(|callee| calls[callee].iter().copied()).collect();
    (chosen, sites)
}
