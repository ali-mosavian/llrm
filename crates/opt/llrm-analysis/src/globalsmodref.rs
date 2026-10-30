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
//! if every callee of the SCC has one. A call to a declaration that may call
//! back into the module (no `nocallback`), an indirect call or a function
//! whose definition is not exact leaves the caller without. `get_mod_ref_info`
//! answers a call against a location that is a tracked global; any other
//! location is left to the caller's other analyses.
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
//! - G6: `escaping_among` (GlobalsAA's use) does not count a global named in an
//!   initializer as escaped; GlobalsAA reads an initializer only through its
//!   holder (`globalsaa::held`). QB's `REDIM` descriptor held every array's
//!   address in a constant nothing read: deedlines +128 memory operands without
//!   it
//!   (test
//!   `a_private_initializer_nothing_names_does_not_leak_the_address_it_holds`).
//!   Exit: the pin's rule, once a rows pair shows no row worse.
//! - G7: `llrm.ia16.window` and `llrm.ia16.restrict` return their operand's
//!   bytes, so the scan follows the call's result as it follows a `gep`, as the
//!   pin follows `threadlocal.address`.
//! - G6: a declaration needs `nocallback` only, not `nosync` as well
//!   (`MaySyncOrCallIntoModule`): MIR has no `nosync` and the targets no
//!   threads.
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
use llrm_mir::context::{ConstantExpr, ConstantId, ConstantKind, Context};
use llrm_mir::facts::Facts;
use llrm_mir::module::{Function, GlobalKind, GlobalValue, InstId, Linkage, Module, Operand, ValueId};
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
    /// The variables no outside code reaches, taken as local (DEVIATE G8).
    unexported: BTreeSet<GlobalId>,
    functions: BTreeMap<GlobalId, FunctionInfo>,
    /// The local variables the use scan calls escaped (it has no `phi` or
    /// `select` case) that escape by no use but being joined: `covers` takes
    /// one for what it was.
    joined: BTreeSet<GlobalId>,
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

thread_local! {
    static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has scanned a module for the sites of its
/// globals.
#[cfg(test)]
fn built() -> usize {
    BUILT.with(std::cell::Cell::get)
}

/// Every instruction operand that names a global, and the globals some
/// initializer names (a constant user: the global's address escapes).
struct Sites {
    /// An operand that is the global, or a constant `gep` or pointer cast of
    /// it: the pin's scan recurses into those operators, so the use is the
    /// instruction's.
    named: BTreeMap<GlobalId, Vec<(GlobalId, InstId, u32)>>,
    in_initializer: BTreeSet<GlobalId>,
    /// Globals in any other constant operand (`ptrtoint`, an aggregate, a
    /// `gep` index): a live constant user, so escaped.
    in_operand: BTreeSet<GlobalId>,
}

/// The global `c` is, or is a constant `gep` or pointer cast of.
fn derived_from(
    context: &Context,
    c: ConstantId,
) -> Option<GlobalId> {
    match &context.get(c).kind {
        ConstantKind::Global(global) => Some(*global),
        ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => derived_from(context, *operands.first()?),
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::BitCast | CastOp::AddrSpaceCast, value }) => {
            derived_from(context, *value)
        }
        _ => None,
    }
}

/// Every global named anywhere inside `c`.
fn named_inside(
    context: &Context,
    c: ConstantId,
    into: &mut BTreeSet<GlobalId>,
) {
    let mut work = vec![c];
    while let Some(c) = work.pop() {
        match &context.get(c).kind {
            ConstantKind::Global(g) => {
                into.insert(*g);
            }
            ConstantKind::Aggregate(members) => work.extend(members.iter().copied()),
            ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => work.extend(operands.iter().copied()),
            ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => work.push(*value),
            _ => {}
        }
    }
}

impl Sites {
    fn of(module: &Module) -> Self {
        BUILT.with(|count| count.set(count.get() + 1));
        let mut named: BTreeMap<GlobalId, Vec<(GlobalId, InstId, u32)>> = BTreeMap::new();
        let (mut in_initializer, mut in_operand) = (BTreeSet::new(), BTreeSet::new());
        for (id, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
            for (_, inst) in function.walk() {
                for (at, operand) in function.instruction(inst).operands.iter().enumerate() {
                    let Operand::Constant(c) = operand else { continue };
                    // A number or a string names no global; most operands are.
                    if !matches!(
                        module.context.get(*c).kind,
                        ConstantKind::Global(_) | ConstantKind::Aggregate(_) | ConstantKind::Expr(_)
                    ) {
                        continue;
                    }
                    match derived_from(&module.context, *c) {
                        Some(global) => named.entry(global).or_default().push((id, inst, at as u32)),
                        None => named_inside(&module.context, *c, &mut in_operand),
                    }
                }
            }
        }
        for global in &module.globals {
            if let GlobalKind::Variable(variable) = &global.kind
                && let Some(initializer) = variable.initializer
            {
                named_inside(&module.context, initializer, &mut in_initializer);
            }
        }
        Self { named, in_initializer, in_operand }
    }
}

struct Analysis<'a> {
    module: &'a Module,
    sites: std::rc::Rc<Sites>,
    /// A global in an initializer escapes (the pin's `isConstantUsed` rule).
    /// GlobalsAA turns it off and reads initializers through their holders.
    initializers_escape: bool,
    /// A `phi` or `select` of the pointer is followed as a `getelementptr` is,
    /// each once. The pin's `AnalyzeUsesOfPointer` has no such case: any other
    /// user is an escape. Asked only to tell a global that escapes by a join
    /// from one that escapes by what the join leads to.
    through_joins: bool,
    seen: std::cell::RefCell<BTreeSet<(GlobalId, ValueId)>>,
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
            && self.initializers_escape
            && self.sites.in_initializer.contains(&g)
        {
            return true;
        }
        if let Root::Global(g) = root
            && self.sites.in_operand.contains(&g)
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
                Opcode::Phi | Opcode::Select if self.through_joins => {
                    let Some(result) = op.result else { return true };
                    // A select's condition is no pointer, and a compare of it
                    // is no use of one; a join already followed is not again.
                    if at == 0 && matches!(op.opcode, Opcode::Select) {
                        return true;
                    }
                    if self.seen.borrow_mut().insert((f, result))
                        && self.escapes(Root::Value(f, result), readers, writers)
                    {
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
                    // What these return is the operand's bytes: followed as
                    // the pin follows `threadlocal.address`.
                    if at == 0
                        && matches!(
                            self.module.global(callee).name.as_deref().and_then(llrm_mir::intrinsics::Intrinsic::named),
                            Some(llrm_mir::intrinsics::Intrinsic::Window | llrm_mir::intrinsics::Intrinsic::Restrict)
                        )
                    {
                        let Some(result) = op.result else { return true };
                        if self.escapes(Root::Value(f, result), readers, writers) {
                            return true;
                        }
                        continue;
                    }
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

/// The globals among `candidates` whose address escapes (`AnalyzeUsesOfPointer`
/// at each, true meaning escaped): the use scan of `AnalyzeGlobals`, asked of
/// any global and not only the internal ones, with one pass over the module's
/// uses.
pub fn escaping_among(
    module: &Module,
    candidates: &BTreeSet<GlobalId>,
) -> BTreeSet<GlobalId> {
    let analysis = Analysis {
        module,
        sites: std::rc::Rc::new(Sites::of(module)),
        initializers_escape: false,
        through_joins: false,
        seen: Default::default(),
    };
    candidates.iter().copied().filter(|&id| analysis.escapes(Root::Global(id), &mut None, &mut None)).collect()
}

/// [`escaping_among`], a `phi` or `select` of a global's address followed like
/// a `getelementptr` (each join once): the globals that escape by what they are
/// used for, not by being joined.
pub fn escaping_among_joins(
    module: &Module,
    candidates: &BTreeSet<GlobalId>,
) -> BTreeSet<GlobalId> {
    let analysis = Analysis {
        module,
        sites: std::rc::Rc::new(Sites::of(module)),
        initializers_escape: false,
        through_joins: true,
        seen: Default::default(),
    };
    // The joins seen are one candidate's: another reaching the same join must
    // follow it too.
    candidates
        .iter()
        .copied()
        .filter(|&id| {
            analysis.seen.borrow_mut().clear();
            analysis.escapes(Root::Global(id), &mut None, &mut None)
        })
        .collect()
}

/// `GlobalsAAResult::analyzeModule`.
/// What a call to the declaration `function` does, from its attributes alone
/// (the `F->isDeclaration()` arm of `AnalyzeCallGraph`); None where it may
/// call back into the module, so nothing is known of its callers.
///
/// DEVIATE G6: `MaySyncOrCallIntoModule` asks `nosync` as well as
/// `nocallback`; MIR has no `nosync` and the targets have no threads, so
/// `nocallback` is enough.
fn declaration_info(function: &Function) -> Option<FunctionInfo> {
    let effects = MemoryEffects::of_attrs(&function.attrs);
    let mut info = FunctionInfo::default();
    if effects.does_not_access_memory() {
        return Some(info);
    }
    let may_call_into_module = !Facts::of(&function.attrs).no_callback();
    if effects.only_reads_memory() {
        info.add_mod_ref(ModRefInfo::Ref);
        if !effects.only_accesses_arg_memory() && may_call_into_module {
            info.may_read_any_global = true;
        }
        return Some(info);
    }
    info.add_mod_ref(ModRefInfo::ModRef);
    if !effects.only_accesses_arg_memory() {
        info.may_read_any_global = true;
    }
    (!may_call_into_module).then_some(info)
}

pub fn compute(module: &Module) -> GlobalsModRef {
    compute_with(module, BTreeSet::new())
}

/// `compute`, the variables `unexported` taken as local too. DEVIATE G8: the
/// pin's `hasLocalLinkage`
/// ([GlobalsModRef.cpp#L276](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/GlobalsModRef.cpp#L276))
/// sees what LTO's internalization has made internal; here the program says
/// which globals no outside code reaches (`Exports`, the runtime's
/// `!llrm.named`).
pub fn compute_with(
    module: &Module,
    unexported: BTreeSet<GlobalId>,
) -> GlobalsModRef {
    COMPUTES.with(|made| made.set(made.get() + 1));
    let sites = std::rc::Rc::new(Sites::of(module));
    let analysis = Analysis {
        module,
        sites: std::rc::Rc::clone(&sites),
        initializers_escape: true,
        through_joins: false,
        seen: Default::default(),
    };
    let mut result = GlobalsModRef { unexported, ..GlobalsModRef::default() };
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
    let mut joins: Option<Analysis> = None;
    for (at, global) in module.globals.iter().enumerate() {
        let (id, GlobalKind::Variable(variable)) = (GlobalId(at as u32), &global.kind) else { continue };
        if !local(global.linkage) && !result.unexported.contains(&id) {
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
            let joins = joins.get_or_insert_with(|| Analysis {
                module,
                sites: std::rc::Rc::clone(&sites),
                initializers_escape: true,
                through_joins: true,
                seen: Default::default(),
            });
            joins.seen.borrow_mut().clear();
            if !joins.escapes(Root::Global(id), &mut None, &mut None) {
                result.joined.insert(id);
            }
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
                if let Some(declared) = module.global(callee).function().filter(|one| one.is_declaration()) {
                    match declaration_info(declared) {
                        Some(declared) => info.add_function_info(&declared),
                        None => {
                            know_nothing = true;
                            break;
                        }
                    }
                } else if let Some(callee_info) = infos.get(&callee) {
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
    // A declaration is a node of the call graph with the info its attributes
    // give (the `isDeclaration()` arm of `AnalyzeCallGraph`), where it may not
    // call back into the module.
    for (id, _, function) in module.functions().filter(|(_, _, function)| function.is_declaration()) {
        if let Some(info) = declaration_info(function) {
            infos.insert(id, info);
        }
    }
    result.functions = infos;
    result
}

impl GlobalsModRef {
    /// Whether `self`, held past edits that add no memory operation, still
    /// bounds what `fresh` finds: it tracks no global `fresh` does not, and
    /// each function it states something of is stated by `fresh` as no more,
    /// of the globals it tracks (it answers `ModRef` for the rest).
    pub fn covers(
        &self,
        fresh: &Self,
    ) -> bool {
        let within = |stale: ModRefInfo, fresh: ModRefInfo| stale.union(fresh) == stale;
        self.non_address_taken.iter().all(|one| fresh.non_address_taken.contains(one) || fresh.joined.contains(one))
            && (self.unknown_functions_with_local_linkage || !fresh.unknown_functions_with_local_linkage)
            && self.functions.iter().all(|(id, stale)| {
                fresh.functions.get(id).is_some_and(|now| {
                    within(stale.mod_ref(), now.mod_ref())
                        && (stale.may_read_any_global || !now.may_read_any_global)
                        && self
                            .non_address_taken
                            .iter()
                            .all(|&global| within(stale.mod_ref_for_global(global), now.mod_ref_for_global(global)))
                })
            })
    }

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

    /// `getModRefInfo(Call, Loc)`; `global` is the global that
    /// `getUnderlyingObject(Loc.Ptr)` reaches, and `argument_may_reach` is
    /// `getModRefInfoForArgument`, run only when every check passed. `ModRef`
    /// where nothing is known of `callee`, the direct callee.
    pub fn get_mod_ref_info_for_global(
        &self,
        globals: &[GlobalValue],
        callee: Option<GlobalId>,
        global: GlobalId,
        argument_may_reach: impl FnOnce() -> ModRefInfo,
    ) -> ModRefInfo {
        let global_value: &GlobalValue = &globals[global.0 as usize];
        if !(local(global_value.linkage) || self.unexported.contains(&global))
            || self.unknown_functions_with_local_linkage
        {
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

/// `GlobalsAAResult` as one of a query's providers: what a call may do to a
/// tracked global (`getModRefInfo(Call, Loc)`), the top of the lattice for
/// everything else.
pub struct GlobalsAAResult<'a> {
    unit: &'a crate::memory::Unit<'a>,
    info: &'a GlobalsModRef,
}

impl<'a> GlobalsAAResult<'a> {
    pub fn new(
        unit: &'a crate::memory::Unit<'a>,
        info: &'a GlobalsModRef,
    ) -> Self {
        Self { unit, info }
    }

    /// `getModRefInfoForArgument`: what the call may do to `global` through
    /// its arguments. DEVIATE G7: `getUnderlyingObjects` is one object, through
    /// geps and casts (no phi or select), and the "all identified or not
    /// aliasing" test asks identified-ness only.
    fn for_argument(
        &self,
        call: InstId,
        global: GlobalId,
    ) -> ModRefInfo {
        let function = self.unit.function;
        let effects = crate::basicaa::call_memory_effects(self.unit, call);
        if effects.does_not_access_memory() {
            return ModRefInfo::NoModRef;
        }
        let conservative = if effects.only_reads_memory() { ModRefInfo::Ref } else { ModRefInfo::ModRef };
        let op = function.instruction(call);
        let arguments = op.operands.len().saturating_sub(1);
        for &argument in &op.operands[..arguments] {
            if self.unit.space(argument).is_none() {
                continue;
            }
            let object = crate::basicaa::underlying_object(self.unit, argument);
            if !crate::basicaa::is_identified_object(self.unit, object) {
                return conservative;
            }
            if let Operand::Constant(id) = object
                && matches!(
                    self.unit.context.get(id).kind,
                    ConstantKind::Global(one) if one == global
                )
            {
                return conservative;
            }
        }
        ModRefInfo::NoModRef
    }
}

impl GlobalsAAResult<'_> {
    /// The global an underlying object is.
    fn global_of(
        &self,
        object: Operand,
    ) -> Option<GlobalId> {
        match object {
            Operand::Constant(id) => match self.unit.context.get(id).kind {
                ConstantKind::Global(global) => Some(global),
                _ => None,
            },
            _ => None,
        }
    }

    /// `isNonEscapingGlobalNoAlias` (L715): an object that is an argument, a
    /// call's result, or loaded from somewhere an escape would have been, is
    /// not the non-address-taken `global`. DEVIATE G9: another global ends the
    /// walk with "may" (the pin goes on for two sized variables), and so does a
    /// null pointer (it asks `NullPointerIsDefined`).
    fn non_escaping_no_alias(
        &self,
        global: GlobalId,
        object: Operand,
    ) -> bool {
        let mut visited = vec![object];
        let mut inputs = vec![object];
        let mut depth = 0;
        while let Some(input) = inputs.pop() {
            if let Some(other) = self.global_of(input) {
                let _ = other;
                return false;
            }
            let Some(defined) = self.unit.defining(input).map(|(_, one)| one) else {
                // An argument escapes already.
                if matches!(input, Operand::Value(_)) {
                    continue;
                }
                return false;
            };
            if matches!(defined.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
                continue;
            }
            depth += 1;
            if depth > 4 {
                return false;
            }
            let mut push = |one: Operand, inputs: &mut Vec<Operand>| {
                let one = crate::basicaa::underlying_object(self.unit, one);
                if !visited.contains(&one) {
                    visited.push(one);
                    inputs.push(one);
                }
            };
            match defined.opcode {
                Opcode::Load { .. } => {
                    let pointer = crate::basicaa::underlying_object(self.unit, defined.operands[0]);
                    if self.loaded_no_alias(pointer, &mut depth) {
                        continue;
                    }
                    return false;
                }
                Opcode::Select => {
                    push(defined.operands[1], &mut inputs);
                    push(defined.operands[2], &mut inputs);
                }
                Opcode::Phi => {
                    for &incoming in defined.operands.iter().step_by(2) {
                        push(incoming, &mut inputs);
                    }
                }
                _ => return false,
            }
        }
        let _ = global;
        true
    }

    /// `isNonEscapingGlobalNoAliasWithLoad` (L633): a pointer loaded from
    /// `object` is no tracked global's, if what `object` is loaded from is
    /// itself an argument, a call's result or a global.
    fn loaded_no_alias(
        &self,
        object: Operand,
        depth: &mut u32,
    ) -> bool {
        let mut visited = vec![object];
        let mut inputs = vec![object];
        while let Some(input) = inputs.pop() {
            if self.global_of(input).is_some() {
                continue;
            }
            let Some(defined) = self.unit.defining(input).map(|(_, one)| one) else {
                if matches!(input, Operand::Value(_)) {
                    continue;
                }
                return false;
            };
            if matches!(defined.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
                continue;
            }
            *depth += 1;
            if *depth > 4 {
                return false;
            }
            let mut add = |one: Operand, inputs: &mut Vec<Operand>| {
                let one = crate::basicaa::underlying_object(self.unit, one);
                if !visited.contains(&one) {
                    visited.push(one);
                    inputs.push(one);
                }
            };
            match defined.opcode {
                Opcode::Load { .. } => add(defined.operands[0], &mut inputs),
                Opcode::Select => {
                    add(defined.operands[1], &mut inputs);
                    add(defined.operands[2], &mut inputs);
                }
                Opcode::Phi => {
                    for &incoming in defined.operands.iter().step_by(2) {
                        add(incoming, &mut inputs);
                    }
                }
                _ => return false,
            }
        }
        true
    }
}

impl crate::aa::AAResults for GlobalsAAResult<'_> {
    /// `GlobalsAAResult::alias` (L836), the non-address-taken globals' part
    /// (L845 to L876); the indirect globals' (L882 on) are not translated.
    fn alias(
        &mut self,
        a: &crate::aa::MemoryLocation,
        b: &crate::aa::MemoryLocation,
        _: &mut crate::aa::AAQueryInfo,
        _: Option<InstId>,
    ) -> crate::aa::AliasResult {
        let (first, second) =
            (crate::basicaa::underlying_object(self.unit, a.ptr), crate::basicaa::underlying_object(self.unit, b.ptr));
        // The global's address taken: pretend it is not known to be a pointer
        // to it.
        // DEVIATE G10: a global the unit states unaddressed (`-oa`, INIT `sw`)
        // is reached by its name alone, as one the use scan finds
        // non-address-taken is.
        let tracked = |object| {
            self.global_of(object).filter(|&global| {
                self.info.is_non_address_taken(global) || self.unit.globals_aa.is_some_and(|aa| aa.tracked(global))
            })
        };
        let (one, two) = (tracked(first), tracked(second));
        if one.is_some() && two.is_some() && one != two {
            return crate::aa::AliasResult::NoAlias;
        }
        if (one.is_some() || two.is_some()) && one != two {
            let (global, other) = match one {
                Some(global) => (global, second),
                None => (two.expect("one of the two"), first),
            };
            if self.non_escaping_no_alias(global, other) {
                return crate::aa::AliasResult::NoAlias;
            }
        }
        crate::aa::AliasResult::MayAlias
    }

    fn get_mod_ref_info_mask(
        &mut self,
        _: &crate::aa::MemoryLocation,
        _: &mut crate::aa::AAQueryInfo,
        _: bool,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }

    fn get_arg_mod_ref_info(
        &mut self,
        _: InstId,
        _: u32,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }

    fn get_memory_effects(
        &mut self,
        _: InstId,
        _: &mut crate::aa::AAQueryInfo,
    ) -> MemoryEffects {
        MemoryEffects::unknown()
    }

    fn get_mod_ref_info(
        &mut self,
        inst: InstId,
        loc: Option<&crate::aa::MemoryLocation>,
        _: &mut crate::aa::AAQueryInfo,
    ) -> ModRefInfo {
        let (Some(loc), Opcode::Call(_) | Opcode::Invoke(_)) = (loc, &self.unit.function.instruction(inst).opcode)
        else {
            return ModRefInfo::ModRef;
        };
        let object = crate::basicaa::underlying_object(self.unit, loc.ptr);
        let Operand::Constant(id) = object else { return ModRefInfo::ModRef };
        let ConstantKind::Global(global) = self.unit.context.get(id).kind else { return ModRefInfo::ModRef };
        let callee = llrm_mir::memory::callee(self.unit.context, self.unit.function, inst);
        self.info.get_mod_ref_info_for_global(self.unit.globals, callee, global, || self.for_argument(inst, global))
    }

    fn get_mod_ref_info_call(
        &mut self,
        _: InstId,
        _: InstId,
        _: &mut crate::aa::AAQueryInfo,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }

    fn call_captures_before(
        &mut self,
        _: InstId,
        _: &crate::aa::MemoryLocation,
        _: &mut crate::aa::AAQueryInfo,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }
}

#[cfg(test)]
#[path = "globalsmodref_tests.rs"]
mod tests;

thread_local! {
    static COMPUTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has worked out `GlobalsModRef` of a module
/// (`compute_with`), for a test that one analysis result serves its readers.
pub fn computes_made() -> usize {
    COMPUTES.with(std::cell::Cell::get)
}
