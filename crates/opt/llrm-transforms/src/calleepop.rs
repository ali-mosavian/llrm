//! Callee-pops calling convention (LLVM's GlobalOpt giving an internal function
//! a `fastcc`): an internal function all of whose callers are seen pops its own
//! stack arguments.
//!
//! # How it works
//! The convention is assigned here and nowhere else: isel takes the cleanup of
//! both sides from it, so a call and the function it names agree. A function is
//! a candidate when `direct_only` accepts it (internal, never named but as a
//! callee), it is not variadic, and every call of it is a plain call in the
//! function's own convention.
//!
//! ```text
//!  f:  ...  ret             (cdecl)         f:  ...  ret 4     (callee pops)
//!  c1: call f; add sp, 4                ==> c1: call f
//!  c2: call f; add sp, 4                    c2: call f
//! ```
//!
//! Two cases, by what the target's description says.
//!
//! By default a function takes the target's callee-pops twin of its convention
//! (`Machine::callee_pop`), if it has one and takes stack arguments. It is
//! priced by the target for the level being built, in clocks or, at -Os, in
//! bytes: the cleanup each call no longer makes, summed over the call sites
//! (`costs.cleanup(words)`, plus `pop`), must pay for what each `ret N` costs
//! (`return_pops`).
//!
//! Where the description gives a private function a convention of its own
//! (`private`, as the x86 m32 description does with `fastcc32`), every
//! internal, non-variadic function whose convention may be replaced takes that
//! one, with the calls of it, under any ABI the program has. No price is asked:
//! registers cost no more than the stack at either end.
//!
//! A function some call of which is not a plain call in its convention keeps
//! its convention.
//!
//! # Translation
//! | LLVM | Rust | Kind |
//! |---|---|---|
//! | [`OptimizeFunctions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/GlobalOpt.cpp#L2019): `fastcc` for a function with a changeable convention | `CalleePop::run` | MATCH |
//! | [`hasChangeableCCImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/GlobalOpt.cpp#L1700) | `direct_only` and the plain-call test | MATCH |
//! | [`ChangeCalleesToFastCall`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/GlobalOpt.cpp#L1673) | the call-site loop in `run` | MATCH |
//! | [`useFastCCForInternalCall`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L8196): always true here | none | MATCH |
//! | what `fastcc` is: two integer arguments in ECX, EDX, the rest on the stack, the caller pops | `fastcc32` at m32 (the pin's); the callee-pop twin at m16 | MATCH |
//!
//! CC1 is closed: the private m32 convention is the pin's `fastcc`
//! (`fastcc32`). `LLRM_PRIVATE_WATCALL=1` gives `watcall32` back, to compare;
//! it is no switch for production. Priced on the 71 m32 rows (486 model) and
//! QCport (62 modules, `qcport.py`), dda83cd1c5, the same binary on both arms;
//! new against the old `watcall32` default:
//!
//! | level | clocks | code bytes | QCport bytes |
//! |---|---|---|---|
//! | -O1 | -0.9% | +0.05% | +0.18% |
//! | -O2 | -0.2% | +0.6% | +0.03% |
//! | -O3 | -0.2% | +0.4% | +0.08% |
//! | -Os | +0.2% | -0.1% | +0.06% |
//!
//! The exit ("adopt the pin's `fastcc` if no worse") is met on clocks at
//! -O1..-O3, by `rectwo` (-9.6%) and `fib` (-5.0%); d_faces compile Ir -0.28%.
//! Rows above the old default are numbered, owner funcattrs-port, exit "tune
//! after the allocator work" (the cost is frame traffic around calls), review
//! 2026-11-13:
//! - CC2, -Os clocks +0.2%: `x_hanoi2` +32.2%, `queens` +4.9%, `bintree` +2.0%.
//! - CC3, -O1..-O3 rows: `x_hanoi2` +9.4% at -O1 and +3.8% at -O2 and -O3,
//!   `queens` +8.2% at -O2.
//! - CC4, code bytes +0.05%, +0.6%, +0.4% at -O1..-O3 (`quicksort` and
//!   `bintree` +64 and +48 B at -O2), QCport +0.03% to +0.18%.
//!
//! # Example
//! Before:
//!
//! ```mir
//! define internal i16 @work(i16 %a, i16 %b, i16 %c) {
//! b0:
//!   %x = add i16 %a, %b
//!   %y = add i16 %x, %c
//!   ret i16 %y
//! }
//!
//! define i16 @f(i16 %x) {
//! b0:
//!   %p = call i16 @work(i16 %x, i16 2, i16 3)
//!   %q = call i16 @work(i16 3, i16 %x, i16 1)
//!   %s = add i16 %p, %q
//!   ret i16 %s
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define internal fastcc i16 @work(i16 %a, i16 %b, i16 %c) {
//! ; ...
//! }
//!
//! define i16 @f(i16 %x) {
//! b0:
//!   %p = call fastcc i16 @work(i16 %x, i16 2, i16 3)
//!   %q = call fastcc i16 @work(i16 3, i16 %x, i16 1)
//!   %s = add i16 %p, %q
//!   ret i16 %s
//! }
//! ```
//!
//! The function and both calls take the `fastcc` convention, so isel emits `ret
//! N` once and no cleanup after either call.
//!
//! From unit test
//! `an_internal_function_called_directly_twice_pops_its_own_arguments` in
//! `calleepop_tests.rs`.
//!
//! # Runs
//! A module pass, once, after the interprocedural step, `globaldce` and
//! `available-externally`, so the callers that remain are the ones that stay.
//! Skipped at -O0.

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

    fn run(
        &mut self,
        module: &mut Module,
        analyses: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
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

/// The functions to give a convention, each with the one it takes, and the
/// calls of them (caller, instruction, convention) that take it: internal, not
/// variadic, with arguments, never named but as a callee, called only in its
/// own convention, one the target has a twin of that removes the stack
/// arguments in the callee, and called often enough that what each return costs
/// is paid for by what each call saves, a word of arguments at least.
fn decided(
    module: &Module,
    costs: &OperationCosts,
    layout: &DataLayout,
    target: &dyn Machine,
) -> (BTreeMap<GlobalId, u32>, Vec<(GlobalId, InstId, u32)>) {
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
            // The stack words its arguments take: each a word at least, a dword
            // or a far pointer two; the convention's registers take
            // the ones it puts there.
            let arguments: Vec<Argument> =
                parameters.iter().map(|&ty| argument_of(&module.context.types, layout, ty)).collect();
            let bytes = target
                .stack_argument_bytes(function.calling_convention, &arguments)
                .unwrap_or_else(|| arguments.iter().map(|one| one.bytes.max(2)).sum());
            let words = (bytes + 1) / 2;
            let returns =
                function.walk().filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Ret)).count()
                    as i64;
            (!function.is_declaration()
                && internal.contains(&callee)
                && !variadic
                && !parameters.is_empty()
                && !other.contains(&callee)
                // A pop's worth of bytes saved is not worth an instruction more
                // at every call.
                && words > 0
                && sites.len() as i64 * costs.cleanup(words) + costs.pop >= returns * costs.return_pops)
                .then_some((callee, twin))
        })
        .collect();
    let sites = chosen
        .iter()
        .flat_map(|(callee, &twin)| calls[callee].iter().map(move |&(caller, inst)| (caller, inst, twin)))
        .collect();
    (chosen, sites)
}

/// What an argument of type `ty` is, for where a convention puts it.
fn argument_of(
    types: &llrm_mir::types::Types,
    layout: &DataLayout,
    ty: llrm_mir::types::TypeId,
) -> Argument {
    use llrm_mir::types::Type;
    Argument {
        bytes: layout.alloc_size(types, ty) as i64,
        integer: types.int_bits(ty).is_some(),
        pointer: matches!(types.get(ty), Type::Pointer(_)),
        floating: matches!(types.get(ty), Type::Float(_)),
        memory: false,
    }
}

/// The functions nothing outside the module reaches (`direct_only`) that have a
/// convention the target lets `private` replace, and the calls of them. A
/// function is left as it is where a call of it is not a plain call in its own
/// convention, or it takes a variable number of arguments.
fn privately(
    module: &Module,
    private: &PrivateConvention,
) -> (BTreeMap<GlobalId, u32>, Vec<(GlobalId, InstId, u32)>) {
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
                Opcode::Call(info) if Some(info.calling_convention) == callee_convention => {
                    calls.entry(callee).or_default().push((caller, inst))
                }
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
            !variadic
                && !refused.contains(callee)
                && function.calling_convention != private.to
                && private.from.contains(&function.calling_convention)
        })
        .map(|callee| (callee, private.to))
        .collect();
    let sites = chosen
        .iter()
        .flat_map(|(callee, &to)| {
            calls.get(callee).into_iter().flatten().map(move |&(caller, inst)| (caller, inst, to))
        })
        .collect();
    (chosen, sites)
}
