//! LLVM's GlobalOpt gives an internal function whose every caller it sees a calling convention
//! of its own (`fastcc`). Here that is the callee popping its arguments: `ret N` once, and no
//! cleanup of the arguments at each call. The convention is assigned here and nowhere else:
//! isel takes both sides' cleanup from it (`passing`), so a call and the function it names
//! agree. Priced by the target for the level being built, in clocks or in bytes.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::callgraph::direct_only;
use llrm_mir::context::{ConstantKind, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{GlobalKind, InstId, Module, Operand};
use llrm_mir::opcode::{FAST, Opcode};
use llrm_mir::passes::{ModuleAnalyses, ModulePass};
use llrm_mir::target::OperationCosts;

pub struct CalleePop {
    /// Code size outranks speed: the target's byte costs, not its clocks.
    pub size: bool,
}

impl ModulePass for CalleePop {
    fn name(&self) -> &'static str {
        "calleepop"
    }

    fn run(&mut self, module: &mut Module, analyses: &mut ModuleAnalyses) -> Vec<GlobalId> {
        let target = &analyses.program().target;
        let costs = if self.size { target.size_costs() } else { target.costs() };
        let (chosen, sites) = decided(module, &costs, &analyses.program().layout);
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

/// The functions to give the convention, and the calls of them (caller, instruction) that
/// take it: internal, not variadic, with arguments, never named but as a callee, called only
/// with C's convention, and called often enough that what each return costs is paid for by
/// what each call saves, a word of arguments at least.
fn decided(module: &Module, costs: &OperationCosts, layout: &DataLayout) -> (BTreeSet<GlobalId>, Vec<(GlobalId, InstId)>) {
    let internal = direct_only(module);
    let mut calls: BTreeMap<GlobalId, Vec<(GlobalId, InstId)>> = BTreeMap::new();
    let mut other: BTreeSet<GlobalId> = BTreeSet::new();
    for (caller, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            let (Opcode::Call(info) | Opcode::Invoke(info)) = &instruction.opcode else { continue };
            let Some(Operand::Constant(id)) = instruction.operands.last() else { continue };
            let ConstantKind::Global(callee) = module.context.get(*id).kind else { continue };
            if info.calling_convention == 0 {
                calls.entry(callee).or_default().push((caller, inst));
            } else {
                other.insert(callee);
            }
        }
    }
    let chosen: BTreeSet<GlobalId> = calls
        .iter()
        .filter(|(callee, sites)| {
            let callee = **callee;
            let Some(function) = module.global(callee).function() else { return false };
            let (_, parameters, variadic) = module.signature(function.ty);
            // The stack words its arguments take: each a word at least, a dword or a far pointer two.
            let words: i64 = parameters.iter().map(|&ty| (layout.alloc_size(&module.context.types, ty).max(2) as i64 + 1) / 2).sum();
            let returns = function.walk().filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Ret)).count() as i64;
            !function.is_declaration()
                && internal.contains(&callee)
                && function.calling_convention == 0
                && !variadic
                && !parameters.is_empty()
                && !other.contains(&callee)
                // A pop's worth of bytes saved is not worth an instruction more at every call.
                && sites.len() as i64 * costs.cleanup(words) + costs.pop >= returns * costs.return_pops
        })
        .map(|(&callee, _)| callee)
        .collect();
    let sites = chosen.iter().flat_map(|callee| calls[callee].iter().copied()).collect();
    (chosen, sites)
}
