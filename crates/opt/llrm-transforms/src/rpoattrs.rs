//! LLVM's `ReversePostOrderFunctionAttrsPass`: `norecurse` for an internal
//! function whose every use is a call from a `norecurse` function.
//!
//! # How it works
//! The one-member call-graph components are walked callers first. An internal
//! defined function with a use is marked `norecurse` when every use is a direct
//! call of it, and each caller is `norecurse` already: no path from it can come
//! back without passing a function that does not recurse. A caller marked
//! earlier in the walk counts.
//!
//! DEVIATE: the pin also infers `nofpclass` on the parameters and result from
//! the calls (`addNoFPClassAttrsTopDown`); MIR has no `nofpclass` attribute.
//!
//! # Example
//! ```text
//! define void @top() norecurse { call @leaf() }    define internal void @leaf() norecurse
//! define internal void @leaf() { ... }        ->   (called only from @top)
//! ```
//!
//! # Runs
//! After `available-externally` in the module pipeline
//! (PassBuilderPipelines.cpp:1542).
//!
//! # See also
//! `deduceFunctionAttributeInRPO`
//! ([FunctionAttrs.cpp:2394](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L2394)),
//! `addNoRecurseAttrsTopDown`
//! ([:2323](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L2323)).

use llrm_mir::callgraph::{CallGraph, direct_calls, direct_only};
use llrm_mir::context::GlobalId;
use llrm_mir::facts::{Fact, Facts};
use llrm_mir::module::{GlobalKind, Module};
use llrm_mir::passes::{ModuleAnalyses, ModulePass};

pub struct ReversePostOrderFunctionAttrs;

impl ModulePass for ReversePostOrderFunctionAttrs {
    fn name(&self) -> &'static str {
        "rpo-function-attrs"
    }

    fn run(
        &mut self,
        module: &mut Module,
        _analyses: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        let internal = direct_only(module);
        let calls = direct_calls(module);
        let components = CallGraph::new(module).bottom_up_components();
        let mut changed = Vec::new();
        for (members, cyclic) in components.into_iter().rev() {
            let [id] = members[..] else { continue };
            if cyclic || !internal.contains(&id) || calls.refused.contains(&id) {
                continue;
            }
            let Some(sites) = calls.sites.get(&id) else { continue };
            let has = |module: &Module, id: GlobalId| {
                module.global(id).function().is_some_and(|one| Facts::of(&one.attrs).no_recurse())
            };
            if has(module, id) || !sites.iter().all(|&(caller, _)| has(module, caller)) {
                continue;
            }
            if let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind {
                function.attrs.extend(Fact::NoRecurse.attribute());
                changed.push(id);
            }
        }
        changed
    }
}

#[cfg(test)]
#[path = "rpoattrs_tests.rs"]
mod tests;
