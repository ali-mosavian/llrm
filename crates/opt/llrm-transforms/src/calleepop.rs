//! LLVM's GlobalOpt gives an internal function whose every caller it sees a calling convention
//! of its own (`fastcc`). Here that is the callee popping its arguments: `ret N` once, and no
//! cleanup of the arguments at each call. The convention is assigned here and nowhere else:
//! isel takes both sides' cleanup from it (`passing`), so a call and the function it names
//! agree. Priced by the target for the level being built, in clocks or in bytes.
//!
//! Where the target's description gives a private function a convention of its own (`private`),
//! every such function takes that one, with the calls of it: no one outside sees it, so the
//! target's best serves, whatever ABI the program has. No price is asked: registers cost no more
//! than the stack at either end. LLVM's `fastcc` here is x86's ECX/EDX; gcc's `regparm(3)` for a
//! `local` function, EAX/EDX/ECX; this is the description's.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::callgraph::direct_only;
use llrm_mir::context::{ConstantKind, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{GlobalKind, InstId, Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{ModuleAnalyses, ModulePass};
use llrm_mir::target::{Argument, Machine, OperationCosts, PrivateConvention};

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
        let private = target.private_convention();
        let (chosen, sites) = match &private {
            Some(private) => privately(module, private),
            None => decided(module, &costs, &analyses.program().layout, &**target),
        };
        let mut changed = BTreeSet::new();
        for (&id, &convention) in &chosen {
            if let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind {
                function.calling_convention = convention;
                changed.insert(id);
            }
        }
        for (caller, inst, convention) in sites {
            if let GlobalKind::Function(function) = &mut module.globals[caller.0 as usize].kind {
                function.set_call_convention(inst, convention);
                changed.insert(caller);
            }
        }
        changed.into_iter().collect()
    }
}

/// The functions to give a convention, each with the one it takes, and the calls of them (caller, instruction, convention) that
/// take it: internal, not variadic, with arguments, never named but as a callee, called only in its own convention, one the target
/// has a twin of that removes the stack arguments in the callee, and called often enough that what each return costs is paid for
/// by what each call saves, a word of arguments at least.
fn decided(module: &Module, costs: &OperationCosts, layout: &DataLayout, target: &dyn Machine) -> (BTreeMap<GlobalId, u32>, Vec<(GlobalId, InstId, u32)>) {
    let internal = direct_only(module);
    let mut calls: BTreeMap<GlobalId, Vec<(GlobalId, InstId)>> = BTreeMap::new();
    let mut other: BTreeSet<GlobalId> = BTreeSet::new();
    for (caller, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            let (Opcode::Call(info) | Opcode::Invoke(info)) = &instruction.opcode else { continue };
            let Some(Operand::Constant(id)) = instruction.operands.last() else { continue };
            let ConstantKind::Global(callee) = module.context.get(*id).kind else { continue };
            if module.global(callee).function().is_some_and(|one| one.calling_convention == info.calling_convention) {
                calls.entry(callee).or_default().push((caller, inst));
            } else {
                other.insert(callee);
            }
        }
    }
    let chosen: BTreeMap<GlobalId, u32> = calls
        .iter()
        .filter_map(|(&callee, sites)| {
            let function = module.global(callee).function()?;
            let (_, parameters, variadic) = module.signature(function.ty);
            let twin = target.callee_pop(function.calling_convention)?;
            // The stack words its arguments take: each a word at least, a dword or a far pointer two; the convention's registers take
            // the ones it puts there.
            let arguments: Vec<Argument> = parameters.iter().map(|&ty| argument_of(&module.context.types, layout, ty)).collect();
            let bytes = target.stack_argument_bytes(function.calling_convention, &arguments).unwrap_or_else(|| arguments.iter().map(|one| one.bytes.max(2)).sum());
            let words = (bytes + 1) / 2;
            let returns = function.walk().filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Ret)).count() as i64;
            (!function.is_declaration()
                && internal.contains(&callee)
                && !variadic
                && !parameters.is_empty()
                && !other.contains(&callee)
                // A pop's worth of bytes saved is not worth an instruction more at every call.
                && words > 0
                && sites.len() as i64 * costs.cleanup(words) + costs.pop >= returns * costs.return_pops)
                .then_some((callee, twin))
        })
        .collect();
    let sites = chosen.iter().flat_map(|(callee, &twin)| calls[callee].iter().map(move |&(caller, inst)| (caller, inst, twin))).collect();
    (chosen, sites)
}

/// What an argument of type `ty` is, for where a convention puts it.
fn argument_of(types: &llrm_mir::types::Types, layout: &DataLayout, ty: llrm_mir::types::TypeId) -> Argument {
    use llrm_mir::types::Type;
    Argument { bytes: layout.alloc_size(types, ty) as i64, integer: types.int_bits(ty).is_some(), pointer: matches!(types.get(ty), Type::Pointer(_)), floating: matches!(types.get(ty), Type::Float(_)), memory: false }
}

/// The functions nothing outside the module reaches (`direct_only`) that have a convention the
/// target lets `private` replace, and the calls of them. A function is left as it is where a call
/// of it is not a plain call in its own convention, or it takes a variable number of arguments.
fn privately(module: &Module, private: &PrivateConvention) -> (BTreeMap<GlobalId, u32>, Vec<(GlobalId, InstId, u32)>) {
    let internal = direct_only(module);
    let mut calls: BTreeMap<GlobalId, Vec<(GlobalId, InstId)>> = BTreeMap::new();
    let mut refused: BTreeSet<GlobalId> = BTreeSet::new();
    for (caller, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            let Some(Operand::Constant(id)) = instruction.operands.last() else { continue };
            let ConstantKind::Global(callee) = module.context.get(*id).kind else { continue };
            let callee_convention = module.global(callee).function().map(|one| one.calling_convention);
            match &instruction.opcode {
                Opcode::Call(info) if Some(info.calling_convention) == callee_convention => calls.entry(callee).or_default().push((caller, inst)),
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    refused.insert(callee);
                }
                _ => {}
            }
        }
    }
    let chosen: BTreeMap<GlobalId, u32> = internal
        .into_iter()
        .filter(|callee| {
            let Some(function) = module.global(*callee).function() else { return false };
            let (_, _, variadic) = module.signature(function.ty);
            !variadic && !refused.contains(callee) && function.calling_convention != private.to && private.from.contains(&function.calling_convention)
        })
        .map(|callee| (callee, private.to))
        .collect();
    let sites = chosen.iter().flat_map(|(callee, &to)| calls.get(callee).into_iter().flatten().map(move |&(caller, inst)| (caller, inst, to))).collect();
    (chosen, sites)
}
