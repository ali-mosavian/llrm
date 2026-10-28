//! Identical constants of a module are one, as LLVM's `ConstantMerge`.
//!
//! # How it works
//! A translation of LLVM
//! [`ConstantMerge.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/ConstantMerge.cpp)
//! at d1106deb71cc. A constant variable with an initializer, in the default
//! address space, in no section, not named by `llvm.used` or
//! `llvm.compiler.used` and not weak for the linker, is keyed by its
//! initializer. Of the variables with one key, the best is canonical: external
//! over local, then `unnamed_addr` over not; of equals the last wins
//! (`IsBetterCanonical`). Every other one with local linkage goes to the
//! canonical one when at least one of the two is `unnamed_addr` (the merged
//! variable loses `unnamed_addr` if the one it absorbed had none), and the
//! canonical variable takes the larger alignment. The step repeats while it
//! merges anything: variables whose initializers named two merged ones now
//! hold the same value.
//!
//! # Example
//! ```text
//!  before                                    after
//!  @a = private unnamed_addr constant        @b = private unnamed_addr constant
//!         [2 x i32] [i32 1, i32 2]                  [2 x i32] [i32 1, i32 2]
//!  @b = private unnamed_addr constant        ; a load of @a loads @b
//!         [2 x i32] [i32 1, i32 2]
//! ```
//!
//! DEVIATE 1: a local variable nothing uses is not deleted here (the pin's pass
//! does); `globaldce` ran before and ours has no stage after this.
//! DEVIATE 2: the alignment of an unstated one is the type's preferred
//! alignment, without the 16-byte raise of a large initialized global.
//! MIR keeps no comdat, thread-local flag or global metadata, so the pin's
//! refusals on those have nothing to refuse.
//!
//! # Parity slot
//! `constmerge`, the last module pass of the pin's pipeline
//! (PassBuilderPipelines.cpp#L1714), after the function passes.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::globalsaa::embedded;
use llrm_mir::context::{ConstantId, ConstantKind, Context, GlobalId};
use llrm_mir::module::{GlobalKind, GlobalValue, Linkage, Module, UnnamedAddr};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};

pub struct ConstantMerge;

/// `isWeakForLinker`.
fn weak_for_linker(linkage: Linkage) -> bool {
    matches!(
        linkage,
        Linkage::Weak
            | Linkage::WeakOdr
            | Linkage::LinkOnce
            | Linkage::LinkOnceOdr
            | Linkage::Common
            | Linkage::ExternWeak
    )
}

fn local(global: &GlobalValue) -> bool {
    matches!(global.linkage, Linkage::Internal | Linkage::Private)
}

/// `FindUsedValues`: the globals `llvm.used` and `llvm.compiler.used` name.
fn used(module: &Module) -> BTreeSet<GlobalId> {
    let mut out = BTreeSet::new();
    for global in &module.globals {
        if matches!(
            global.name.as_deref(),
            Some("llvm.used" | "llvm.compiler.used")
        )
            && let GlobalKind::Variable(variable) = &global.kind
            && let Some(initializer) = variable.initializer
        {
            embedded(&module.context, initializer, &mut out);
        }
    }
    out
}

/// `isUnmergeableGlobal`, and the pass's own refusals of weak and external-only
/// variables: the initializer of a variable the pass may touch.
fn mergeable(
    module: &Module,
    id: GlobalId,
    used: &BTreeSet<GlobalId>,
) -> Option<ConstantId> {
    let global = module.global(id);
    let GlobalKind::Variable(variable) = &global.kind else { return None };
    let initializer = variable.initializer?;
    let in_section = global.name.as_deref().is_some_and(|name| module.sections.contains_key(name));
    (variable.constant
        && global.address_space == 0
        && !in_section
        && !used.contains(&id)
        && !weak_for_linker(global.linkage)
        && global.linkage != Linkage::AvailableExternally)
        .then_some(initializer)
}

/// What `constant` is, with each global it names given as the one `merged`
/// says stands for it: two initializers that differ only in which of two merged
/// variables they name have one key.
fn key(
    context: &Context,
    merged: &BTreeMap<GlobalId, GlobalId>,
    constant: ConstantId,
    out: &mut String,
) {
    use std::fmt::Write;
    let one = context.get(constant);
    let _ = write!(out, "{:?}:", one.ty);
    match &one.kind {
        ConstantKind::Global(global) => {
            let _ = write!(out, "@{}", merged.get(global).unwrap_or(global).0);
        }
        ConstantKind::Aggregate(members) => {
            out.push('[');
            for &member in members {
                key(context, merged, member, out);
                out.push(',');
            }
            out.push(']');
        }
        ConstantKind::Expr(llrm_mir::context::ConstantExpr::GetElementPtr {
            source,
            inbounds,
            nusw,
            nuw,
            operands,
        }) => {
            let _ = write!(out, "gep {source:?} {inbounds} {nusw} {nuw} (");
            for &operand in operands {
                key(context, merged, operand, out);
                out.push(',');
            }
            out.push(')');
        }
        ConstantKind::Expr(llrm_mir::context::ConstantExpr::Cast { op, value }) => {
            let _ = write!(out, "cast {op:?} (");
            key(context, merged, *value, out);
            out.push(')');
        }
        other => {
            let _ = write!(out, "{other:?}");
        }
    }
}

/// `IsBetterCanonical(A, B)`.
fn better(
    a: &GlobalValue,
    b: &GlobalValue,
) -> bool {
    if !local(a) && local(b) {
        return true;
    }
    if local(a) && !local(b) {
        return false;
    }
    let (a_unnamed, b_unnamed) = (a.unnamed_addr == UnnamedAddr::Global, b.unnamed_addr == UnnamedAddr::Global);
    if a_unnamed != b_unnamed {
        return a_unnamed;
    }
    true
}

/// `mergeConstants(M)`: whether any variable was merged.
fn merge(
    module: &mut Module,
    layout: &llrm_mir::datalayout::DataLayout,
) -> bool {
    let used = used(module);
    let mut merged: BTreeMap<GlobalId, GlobalId> = BTreeMap::new();
    loop {
        let mut slots: BTreeMap<String, GlobalId> = BTreeMap::new();
        for index in 0..module.globals.len() as u32 {
            let id = GlobalId(index);
            if merged.contains_key(&id) {
                continue;
            }
            let Some(initializer) = mergeable(module, id, &used) else { continue };
            let mut text = String::new();
            key(&module.context, &merged, initializer, &mut text);
            match slots.get(&text) {
                Some(&held) if !better(module.global(id), module.global(held)) => {}
                _ => {
                    slots.insert(text, id);
                }
            }
        }
        let mut replacements: Vec<(GlobalId, GlobalId)> = Vec::new();
        for index in 0..module.globals.len() as u32 {
            let id = GlobalId(index);
            if merged.contains_key(&id) || !local(module.global(id)) {
                continue;
            }
            let Some(initializer) = mergeable(module, id, &used) else { continue };
            let mut text = String::new();
            key(&module.context, &merged, initializer, &mut text);
            let Some(&slot) = slots.get(&text) else { continue };
            if slot == id {
                continue;
            }
            // `makeMergeable`.
            let (old_unnamed, new_unnamed) = (
                module.global(id).unnamed_addr == UnnamedAddr::Global,
                module.global(slot).unnamed_addr == UnnamedAddr::Global,
            );
            if !old_unnamed && !new_unnamed {
                continue;
            }
            if !old_unnamed {
                module.globals[slot.0 as usize].unnamed_addr = UnnamedAddr::None;
            }
            replacements.push((id, slot));
        }
        if replacements.is_empty() {
            break;
        }
        for (old, new) in replacements {
            let aligned = |module: &Module, id: GlobalId| {
                let GlobalKind::Variable(variable) = &module.global(id).kind else { unreachable!("a variable") };
                variable
                    .align
                    .unwrap_or_else(
                        || layout.global_align(&module.context.types, variable.ty, None, false, true, false),
                    )
            };
            let stated = |module: &Module, id: GlobalId| match &module.global(id).kind {
                GlobalKind::Variable(variable) => variable.align.is_some(),
                GlobalKind::Function(_) => false,
            };
            if stated(module, old) || stated(module, new) {
                let larger = aligned(module, old).max(aligned(module, new));
                if let GlobalKind::Variable(variable) = &mut module.globals[new.0 as usize].kind {
                    variable.align = Some(larger);
                }
            }
            merged.insert(old, new);
        }
    }
    if merged.is_empty() {
        return false;
    }
    // Every name of a merged variable is the one it went to, then the merged
    // variables go.
    module.context.renumber_globals(&|global| Some(merged.get(&global).copied().unwrap_or(global)));
    module.retain_globals(&|global| !merged.contains_key(&global));
    true
}

impl ProgramPass for ConstantMerge {
    fn name(&self) -> &'static str {
        "constmerge"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        let layout = program.layout.clone();
        let mut changed = false;
        for module in &mut program.modules {
            changed |= merge(module, &layout);
        }
        if changed {
            analyses.invalidate();
            analyses.abandon::<llrm_analysis::manager::GlobalsAA>();
            analyses.abandon::<llrm_analysis::manager::GlobalsModRefAnalysis>();
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "constmerge_tests.rs"]
mod tests;
