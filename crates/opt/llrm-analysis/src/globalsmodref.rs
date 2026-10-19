//! What each function does to the globals no code reaches but by name: LLVM's
//! `GlobalsModRef.cpp` (`GlobalsAAResult`), translated at the pin
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/GlobalsModRef.cpp>).
//!
//! # How it works
//! `AnalyzeGlobals` finds the internal globals whose address is only loaded
//! from, stored through, compared with null or passed to a `nocallback`
//! declaration (`AnalyzeUsesOfPointer`), with the functions that read and
//! write each. `AnalyzeCallGraph` then walks the call graph callees first, one
//! SCC at a time: a function gets a `FunctionInfo` (one `ModRefInfo` for all
//! memory, a `ModRefInfo` per tracked global, and "may read any global") only
//! if every callee of the SCC has one. A call to a declaration, an indirect
//! call or a function whose definition is not exact leaves the caller
//! without. `get_mod_ref_info` answers a call against a location that is a
//! tracked global; any other location is left to the caller's other analyses.
//!
//! ```text
//!  @g = internal global i16 0      f reads @g          g() calls f, writes @g
//!  define i16 @f() { load @g }     f: {@g: Ref}        g: {@g: ModRef}
//!  define void @g() { call f; store @g }
//! ```
//!
//! # Example
//! A call of `@f` can read `@g` and not write it:
//!
//! ```mir
//! @g = internal global i16 0
//!
//! define i16 @f() {
//! b0:
//!   %v = load i16, ptr @g
//!   ret i16 %v
//! }
//! ```
//!
//! `info(@f).mod_ref_for_global(@g)` is `Ref`.
//!
//! From unit test `a_reader_of_a_private_global_has_ref_on_it` in
//! `globalsmodref_tests.rs`.
//!
//! # Deviations
//! - G1: `IndirectGlobals` and `AllocsForIndirectGlobals` (the `alias` rule for
//!   a global that owns heap memory) are not built; it only adds `NoAlias`
//!   answers, so leaving it out is conservative.
//! - G2: an indirect call with a `!callees` list is the edges it lists (llrm's
//!   `CallGraph`); LLVM's call graph calls external.
//! - G3: `NullPointerIsDefined` is taken as true: a null pointer is never
//!   `NoAlias` with a tracked global.
//! - G4: no `TargetLibraryInfo`: a call to `free` is not recognised, so a
//!   global passed to it is not tracked.
//! - G5: `hasOptNone` is `noipa` (`llrm_mir::facts::opaque`).
//!
//! # Provides
//! `GlobalsModRef`, `FunctionInfo`, `compute`.
//!
//! # Needs
//! [`llrm_mir::callgraph::CallGraph`], [`crate::aa`] (`ModRefInfo`,
//! `MemoryEffects`).
//!
//! # See also
//! [`crate::globalsaa`], which finds the same globals from `alias`'s
//! summaries and is what the passes read today.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::GlobalId;
use llrm_mir::callgraph::CallGraph;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::facts::Facts;
use llrm_mir::module::{GlobalKind, GlobalValue, InstId, Linkage, Module, Operand, ValueId};
use llrm_mir::opcode::{CastOp, Opcode};

use crate::aa::{MemoryEffects, ModRefInfo};

/// `GlobalsAAResult::FunctionInfo`: what a function does to memory, in
/// general and to each tracked global.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FunctionInfo {
    mod_ref: Option<ModRefInfo>,
    may_read_any_global: bool,
    globals: BTreeMap<GlobalId, ModRefInfo>,
}

impl FunctionInfo {
    /// `getModRefInfo()`: all memory.
    pub fn mod_ref(&self) -> ModRefInfo {
        self.mod_ref.unwrap_or(ModRefInfo::NoModRef)
    }

    /// `mayReadAnyGlobal()`.
    pub fn may_read_any_global(&self) -> bool {
        self.may_read_any_global
    }

    /// `getModRefInfoForGlobal`.
    pub fn mod_ref_for_global(
        &self,
        global: GlobalId,
    ) -> ModRefInfo {
        let known = if self.may_read_any_global { ModRefInfo::Ref } else { ModRefInfo::NoModRef };
        known.union(self.globals.get(&global).copied().unwrap_or(ModRefInfo::NoModRef))
    }

    fn add_mod_ref(
        &mut self,
        info: ModRefInfo,
    ) {
        self.mod_ref = Some(self.mod_ref().union(info));
    }

    fn add_for_global(
        &mut self,
        global: GlobalId,
        info: ModRefInfo,
    ) {
        let held = self.globals.entry(global).or_insert(ModRefInfo::NoModRef);
        *held = held.union(info);
    }

    /// `addFunctionInfo`: `other` into `self`, saturating towards `ModRef`.
    fn add_function_info(
        &mut self,
        other: &FunctionInfo,
    ) {
        self.add_mod_ref(other.mod_ref());
        self.may_read_any_global |= other.may_read_any_global;
        for (&global, &info) in &other.globals {
            self.add_for_global(global, info);
        }
    }
}

/// `GlobalsAAResult`: the tracked globals and each function's `FunctionInfo`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GlobalsModRef {
    non_address_taken: BTreeSet<GlobalId>,
    unknown_functions_with_local_linkage: bool,
    functions: BTreeMap<GlobalId, FunctionInfo>,
}

/// `isDefinitionExact`.
fn exact(linkage: Linkage) -> bool {
    matches!(
        linkage,
        Linkage::External | Linkage::Internal | Linkage::Private
    )
}

fn local(linkage: Linkage) -> bool {
    matches!(linkage, Linkage::Internal | Linkage::Private)
}

/// Where a pointer's uses are found: a global, or a value of a function.
#[derive(Clone, Copy)]
enum Root {
    Global(GlobalId),
    Value(GlobalId, ValueId),
}

/// Every instruction operand that names a global, and the globals some
/// initializer names (a constant user: the global's address escapes).
struct Sites {
    named: BTreeMap<GlobalId, Vec<(GlobalId, InstId, u32)>>,
    in_initializer: BTreeSet<GlobalId>,
}

impl Sites {
    fn of(module: &Module) -> Self {
        let mut named: BTreeMap<GlobalId, Vec<(GlobalId, InstId, u32)>> = BTreeMap::new();
        let mut in_initializer = BTreeSet::new();
        for (id, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
            for (_, inst) in function.walk() {
                for (at, operand) in function.instruction(inst).operands.iter().enumerate() {
                    if let Operand::Constant(c) = operand
                        && let ConstantKind::Global(global) = module.context.get(*c).kind
                    {
                        named.entry(global).or_default().push((id, inst, at as u32));
                    }
                }
            }
        }
        for global in &module.globals {
            if let GlobalKind::Variable(variable) = &global.kind
                && let Some(initializer) = variable.initializer
            {
                let mut work = vec![initializer];
                while let Some(c) = work.pop() {
                    match &module.context.get(c).kind {
                        ConstantKind::Global(g) => {
                            in_initializer.insert(*g);
                        }
                        ConstantKind::Aggregate(members) => work.extend(members.iter().copied()),
                        ConstantKind::Expr(llrm_mir::context::ConstantExpr::GetElementPtr { operands, .. }) => {
                            work.extend(operands.iter().copied());
                        }
                        ConstantKind::Expr(llrm_mir::context::ConstantExpr::Cast { value, .. }) => work.push(*value),
                        _ => {}
                    }
                }
            }
        }
        Self { named, in_initializer }
    }
}

struct Analysis<'a> {
    module: &'a Module,
    sites: Sites,
}

impl Analysis<'_> {
    fn context(&self) -> &Context {
        &self.module.context
    }

    fn uses(
        &self,
        root: Root,
    ) -> Vec<(GlobalId, InstId, u32)> {
        match root {
            Root::Global(g) => self.sites.named.get(&g).cloned().unwrap_or_default(),
            Root::Value(f, v) => {
                let function = self.module.global(f).function().expect("a procedure");
                function.users(v).into_iter().map(|one| (f, one.user, one.index)).collect()
            }
        }
    }

    /// `AnalyzeUsesOfPointer`: whether the address escapes, noting the
    /// functions that read and write it.
    fn escapes(
        &self,
        root: Root,
        readers: &mut Option<&mut BTreeSet<GlobalId>>,
        writers: &mut Option<&mut BTreeSet<GlobalId>>,
    ) -> bool {
        if let Root::Global(g) = root
            && self.sites.in_initializer.contains(&g)
        {
            return true;
        }
        for (f, inst, at) in self.uses(root) {
            let function = self.module.global(f).function().expect("a procedure");
            let op = function.instruction(inst);
            match &op.opcode {
                Opcode::Load { .. } => {
                    if let Some(set) = readers {
                        set.insert(f);
                    }
                }
                Opcode::Store { .. } => {
                    if at == 1 {
                        if let Some(set) = writers {
                            set.insert(f);
                        }
                    } else {
                        return true;
                    }
                }
                Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => {
                    let Some(result) = op.result else { return true };
                    if self.escapes(Root::Value(f, result), readers, writers) {
                        return true;
                    }
                }
                Opcode::Call(info) | Opcode::Invoke(info) => {
                    let tail = if matches!(op.opcode, Opcode::Invoke(_)) { 3 } else { 1 };
                    let arguments = op.operands.len() - tail;
                    if (at as usize) >= arguments {
                        // The function being called, not passed.
                        continue;
                    }
                    let callee = llrm_mir::memory::callee(self.context(), function, inst);
                    let Some(callee) = callee else { return true };
                    let callee_function = self.module.global(callee).function();
                    let Some(callee_function) = callee_function.filter(|one| one.is_declaration()) else {
                        return true;
                    };
                    let no_callback =
                        Facts::of(&info.attrs).no_callback() || Facts::of(&callee_function.attrs).no_callback();
                    let argument = at as usize;
                    let no_capture = info.argument_attrs.get(argument).is_some_and(|a| Facts::of(a).no_capture())
                        || callee_function.parameter_attrs.get(argument).is_some_and(|a| Facts::of(a).no_capture());
                    if !no_callback || !no_capture {
                        return true;
                    }
                    // Conservatively, the call reads and writes the global.
                    if let Some(set) = readers {
                        set.insert(f);
                    }
                    if let Some(set) = writers {
                        set.insert(f);
                    }
                }
                Opcode::ICmp(_) => {
                    let null = matches!(
                        op.operands.get(1),
                        Some(Operand::Constant(c)) if matches!(self.context().get(*c).kind, ConstantKind::Null)
                    );
                    if !null {
                        return true;
                    }
                }
                _ => return true,
            }
        }
        false
    }
}

/// `GlobalsAAResult::analyzeModule`.
pub fn compute(module: &Module) -> GlobalsModRef {
    let analysis = Analysis { module, sites: Sites::of(module) };
    let mut result = GlobalsModRef::default();
    let mut tracked_functions: BTreeSet<GlobalId> = BTreeSet::new();
    let mut infos: BTreeMap<GlobalId, FunctionInfo> = BTreeMap::new();
    // AnalyzeGlobals
    for (id, global, _) in module.functions() {
        if local(global.linkage) {
            if !analysis.escapes(Root::Global(id), &mut None, &mut None) {
                result.non_address_taken.insert(id);
                tracked_functions.insert(id);
            } else {
                result.unknown_functions_with_local_linkage = true;
            }
        }
    }
    for (at, global) in module.globals.iter().enumerate() {
        let (id, GlobalKind::Variable(variable)) = (GlobalId(at as u32), &global.kind) else { continue };
        if !local(global.linkage) {
            continue;
        }
        let (mut readers, mut writers) = (BTreeSet::new(), BTreeSet::new());
        let writes = !variable.constant;
        let escaped = if writes {
            analysis.escapes(Root::Global(id), &mut Some(&mut readers), &mut Some(&mut writers))
        } else {
            analysis.escapes(Root::Global(id), &mut Some(&mut readers), &mut None)
        };
        if escaped {
            continue;
        }
        result.non_address_taken.insert(id);
        for reader in readers {
            tracked_functions.insert(reader);
            infos.entry(reader).or_default().add_for_global(id, ModRefInfo::Ref);
        }
        if writes {
            for writer in writers {
                tracked_functions.insert(writer);
                infos.entry(writer).or_default().add_for_global(id, ModRefInfo::Mod);
            }
        }
    }
    // AnalyzeCallGraph
    let graph = CallGraph::new(module);
    for (scc, _) in graph.bottom_up_components() {
        let first = scc[0];
        let exact_definition = |id: GlobalId| exact(module.global(id).linkage);
        if !exact_definition(first) {
            for id in &scc {
                infos.remove(id);
            }
            continue;
        }
        let mut info = infos.remove(&first).unwrap_or_default();
        let mut know_nothing = false;
        for &id in &scc {
            let function = module.global(id).function().expect("a procedure");
            if llrm_mir::facts::opaque(function) {
                // hasOptNone: the attributes are the best approximation.
                let effects = MemoryEffects::of_attrs(&function.attrs);
                if effects.does_not_access_memory() {
                } else if effects.only_reads_memory() {
                    info.add_mod_ref(ModRefInfo::Ref);
                    if !effects.only_accesses_arg_memory() {
                        info.may_read_any_global = true;
                    }
                } else {
                    info.add_mod_ref(ModRefInfo::ModRef);
                    if !effects.only_accesses_arg_memory() {
                        info.may_read_any_global = true;
                    }
                    know_nothing = true;
                    break;
                }
                continue;
            }
            if graph.calls_unknown(id) {
                know_nothing = true;
                break;
            }
            for callee in graph.callees_of(id) {
                if let Some(callee_info) = infos.get(&callee) {
                    info.add_function_info(&callee_info.clone());
                } else if !scc.contains(&callee) {
                    know_nothing = true;
                    break;
                }
            }
            if know_nothing {
                break;
            }
        }
        if know_nothing {
            for id in &scc {
                infos.remove(id);
            }
            continue;
        }
        for &id in &scc {
            if info.mod_ref() == ModRefInfo::ModRef {
                break;
            }
            let function = module.global(id).function().expect("a procedure");
            if llrm_mir::facts::opaque(function) {
                continue;
            }
            for (_, inst) in function.walk() {
                if info.mod_ref() == ModRefInfo::ModRef {
                    break;
                }
                let op = function.instruction(inst);
                if matches!(op.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
                    continue;
                }
                // MIR has no atomics: a load reads, and writes if volatile.
                match op.opcode {
                    Opcode::Load { volatile, .. } => {
                        info.add_mod_ref(ModRefInfo::Ref);
                        if volatile {
                            info.add_mod_ref(ModRefInfo::Mod);
                        }
                    }
                    Opcode::Store { .. } => info.add_mod_ref(ModRefInfo::Mod),
                    _ => {}
                }
            }
        }
        for &id in &scc {
            infos.insert(id, info.clone());
        }
    }
    result.functions = infos;
    result
}

impl GlobalsModRef {
    /// `getFunctionInfo`: none where nothing useful is known.
    pub fn function_info(
        &self,
        function: GlobalId,
    ) -> Option<&FunctionInfo> {
        self.functions.get(&function)
    }

    /// Whether `global` is an internal global whose address is only loaded,
    /// stored through or compared.
    pub fn is_non_address_taken(
        &self,
        global: GlobalId,
    ) -> bool {
        self.non_address_taken.contains(&global)
    }

    /// `getMemoryEffects(const Function *)`.
    pub fn get_memory_effects(
        &self,
        function: GlobalId,
    ) -> MemoryEffects {
        self.function_info(function).map_or_else(MemoryEffects::unknown, |one| MemoryEffects::everywhere(one.mod_ref()))
    }

    /// `getModRefInfo(Call, Loc)` where `Loc` is the global `global`: what the
    /// direct callee `callee` does to it, with the call's arguments. `Known`
    /// is `ModRef` where nothing is known.
    pub fn get_mod_ref_info_for_global(
        &self,
        module: &Module,
        callee: Option<GlobalId>,
        global: GlobalId,
        argument_may_reach: impl FnOnce() -> ModRefInfo,
    ) -> ModRefInfo {
        let global_value: &GlobalValue = module.global(global);
        if !local(global_value.linkage) || self.unknown_functions_with_local_linkage {
            return ModRefInfo::ModRef;
        }
        let Some(callee) = callee else { return ModRefInfo::ModRef };
        if !self.non_address_taken.contains(&global) {
            return ModRefInfo::ModRef;
        }
        match self.function_info(callee) {
            Some(info) => info.mod_ref_for_global(global).union(argument_may_reach()),
            None => ModRefInfo::ModRef,
        }
    }
}

#[cfg(test)]
#[path = "globalsmodref_tests.rs"]
mod tests;
