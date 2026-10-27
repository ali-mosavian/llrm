//! Which globals no code outside the program reaches but by name: LLVM's
//! GlobalsAA, GCC's ipa-reference.
//!
//! # How it works
//! A global variable is tracked when three things hold. No body in any module
//! lets its address escape: each body's pointer solve (`alias::points_to`)
//! lists the globals that escape. No live initializer holds its address (an
//! initializer is read only through its own global). And the program does not
//! export it, or the program's runtime module lists it in `!llrm.named`: the
//! runtime names it but never hands its address out.
//!
//! ```text
//!  module A  @g = internal global ...      tracked: no body leaks &@g
//!            f(): store 1, @g              reached only by its name,
//!            h(): call @ext()              so a declared @ext cannot touch @g
//! ```
//!
//! A tracked global is uncaptured (`memory::global_object`), so no nonlocal
//! reach meets it. What each body does to one, callees included, is in
//! [`crate::alias`]'s summaries. A callee no summary describes reaches the
//! tracked globals as `Globals::unsummarized` says:
//!
//! - a body of the module (or of another module that defines this declaration):
//!   all of them;
//! - otherwise the named ones it writes (those its `!llrm.writes` node in the
//!   runtime module lists, or all without one) and, unless it is `nocallback`
//!   (`calls_back`), what the module's entry points do, as it may call back
//!   into them (`entries`: bodies not internal, or address-taken).
//!
//! `!llrm.named = !{!0}` with `!0 = !{ptr @g, ...}`; `!llrm.writes = !{!1,
//! ...}` with `!1 = !{ptr @routine, ptr @g, ...}`: of the named globals,
//! `@routine` writes by name only those listed. The runtime module's globals
//! stand for each module's of the same names.
//!
//! Across modules, `ProgramGlobals` finds, per module, the globals another
//! module lets escape or holds in an initializer (`Elsewhere.escaped`), its
//! declarations another module defines and the functions another takes the
//! address of, comparing globals by the definition they resolve to. A global
//! passed to a callee that is `noretain` is not named for this purpose. Each
//! body's contribution is kept while its history is unchanged, and `covers`
//! says whether a held result still claims no more than a fresh one would.
//!
//! # Example
//! `@g` is internal and no body leaks its address, so a call that cannot name
//! it keeps the 7:
//!
//! ```mir
//! @g = internal global i16 0
//!
//! declare void @outside(ptr) nocallback
//!
//! define i16 @f() {
//! b0:
//!   store i16 7, ptr @g
//!   call void @outside(ptr null)
//!   %r = load i16, ptr @g
//!   ret i16 %r
//! }
//! ```
//!
//! `%r` is `Known { n: 7, width: 16 }`. Passing `@g` to `@outside`, storing
//! `@g` anywhere, or holding it in another global's initializer makes `%r`
//! unknown.
//!
//! From unit tests `a_private_global_is_kept_across_a_call_that_cannot_name_it`
//! and `a_private_global_whose_address_leaves_is_forgotten` in
//! `globalsaa_tests.rs`.
//!
//! # Provides
//! `Globals` (`tracked`, `unsummarized`, `entries`, `covers`), `analysis`,
//! `ProgramGlobals`, and the helpers `embedded` and `calls_back`. The manager
//! entry is [`crate::manager::GlobalsAA`]. Consumers: [`crate::alias`],
//! [`crate::memory`], and `globalopt` and `globaldce` in `llrm-transforms`
//! (`embedded`).
//!
//! # Needs
//! [`crate::alias`] (`points_to`), [`crate::cfg::Shape`], `llrm_mir`'s program
//! and module analyses.
//!
//! # See also
//! LLVM `GlobalsAA`; GCC `ipa-reference.cc`.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_mir::context::{ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
use llrm_mir::module::{GlobalKind, GlobalValue, InstId, Linkage, Mark, MetadataOperand, Module, Operand};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::passes::{Declarations, ModuleAnalyses};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramAnalysis, ProgramProxy, defines};
use llrm_support::hash::IndexMap;

use crate::memory::Unit;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Globals {
    tracked: BTreeSet<GlobalId>,
    /// Globals the use scan calls escaped (the pin's `AnalyzeUsesOfPointer`,
    /// GlobalsModRef.cpp#L312, has no `phi` or `select` case, so one is an
    /// escape) that escape by no use but being joined: reached by name only, to
    /// a body that follows a pointer through its joins. `covers` takes one for
    /// what it was.
    joined: BTreeSet<GlobalId>,
    /// The module forges a pointer into the data segment (`forging`): the
    /// selector casts of its far pointers may name any object.
    forged: bool,
    /// The `!llrm.named` globals.
    named: BTreeSet<GlobalId>,
    /// Each routine's `!llrm.writes` list.
    writes: BTreeMap<GlobalId, BTreeSet<GlobalId>>,
    /// The functions with a body here.
    bodies: BTreeSet<GlobalId>,
    /// The bodies outside code may call: not internal, or address-taken.
    entries: BTreeSet<String>,
    /// How many globals the module had: a function at or past it was added
    /// since (an ipa-cp copy), a body as LLVM's absent `FunctionInfo` is.
    known: u32,
}

impl Globals {
    /// Whether `self`, held past passes that add no memory operation, claims no
    /// more than `fresh` does: a global it takes to be reached by name only
    /// is still so, and the rest of what it states is as it was.
    pub fn covers(
        &self,
        fresh: &Globals,
    ) -> bool {
        self.tracked.iter().all(|one| fresh.tracked.contains(one) || fresh.joined.contains(one))
            && (self.forged || !fresh.forged)
            && self.named == fresh.named
            && self.writes == fresh.writes
            && fresh.bodies.iter().all(|one| self.bodies.contains(one) || one.0 >= self.known)
            && self.entries == fresh.entries
    }

    /// Whether the module forges a pointer into the data segment.
    pub fn forged(&self) -> bool {
        self.forged
    }

    #[cfg(test)]
    pub(crate) fn set_forged(
        &mut self,
        forged: bool,
    ) {
        self.forged = forged;
    }

    pub fn tracked(
        &self,
        global: GlobalId,
    ) -> bool {
        self.tracked.contains(&global)
    }

    pub fn tracked_globals(&self) -> &BTreeSet<GlobalId> {
        &self.tracked
    }

    /// The tracked globals the call of `callee`, which no summary
    /// describes, may read and may write, but for callbacks.
    pub fn unsummarized(
        &self,
        callee: Option<GlobalId>,
    ) -> (BTreeSet<GlobalId>, BTreeSet<GlobalId>) {
        if callee.is_some_and(|one| self.bodies.contains(&one) || one.0 >= self.known) {
            return (self.tracked.clone(), self.tracked.clone());
        }
        let named = self.named.intersection(&self.tracked).copied().collect::<BTreeSet<_>>();
        let writes = match callee.and_then(|one| self.writes.get(&one)) {
            Some(listed) => listed.intersection(&named).copied().collect(),
            None => named.clone(),
        };
        (named, writes)
    }

    /// The bodies outside code may call.
    pub fn entries(&self) -> &BTreeSet<String> {
        &self.entries
    }

    /// A number that is the same for two results where a body that names only
    /// the globals `ids` reads the same of both: what is tracked, named and
    /// entered at all, and what each of `ids` is as a body here and as a
    /// writer.
    pub fn read_by(
        &self,
        ids: &BTreeSet<GlobalId>,
    ) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = llrm_mir::hash::Fx::default();
        (&self.tracked, &self.named, &self.entries).hash(&mut hasher);
        // By what each is, not by its id: a copy of a body is the body.
        let mut each: Vec<(bool, Option<&BTreeSet<GlobalId>>)> =
            ids.iter().map(|id| (self.bodies.contains(id), self.writes.get(id))).collect();
        each.sort_unstable();
        each.dedup();
        each.hash(&mut hasher);
        hasher.finish()
    }
}

/// The globals `constant` holds the address of, through aggregates and
/// constant expressions.
pub fn embedded(
    context: &Context,
    constant: ConstantId,
    out: &mut BTreeSet<GlobalId>,
) {
    match &context.get(constant).kind {
        ConstantKind::Global(global) => {
            out.insert(*global);
        }
        ConstantKind::Aggregate(members) => members.iter().for_each(|&one| embedded(context, one, out)),
        ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => {
            operands.iter().for_each(|&one| embedded(context, one, out))
        }
        ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => embedded(context, *value, out),
        _ => {}
    }
}

/// Whether the call `at` may call back into the module: LLVM's
/// `nocallback`, at the site or on the callee, says it may not.
pub fn calls_back(
    unit: &Unit,
    at: InstId,
) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &unit.function.instruction(at).opcode else { return false };
    let flagged = |attrs: &[Attribute]| llrm_mir::facts::Facts::of(attrs).no_callback();
    // A callee a pass declared after the outer facts were taken is not among
    // them.
    let callee = llrm_mir::memory::callee(unit.context, unit.function, at)
        .and_then(|one| unit.globals.get(one.0 as usize)?.function());
    !(flagged(&info.attrs) || callee.is_some_and(|one| flagged(&one.attrs)))
}

/// The first operand and the rest of each `!name` node, as globals.
fn listed(
    module: &Module,
    name: &str,
) -> Vec<(Option<GlobalId>, BTreeSet<GlobalId>)> {
    let global = |operand: &MetadataOperand| match operand {
        MetadataOperand::Constant(id) => match module.context.get(*id).kind {
            ConstantKind::Global(global) => Some(global),
            _ => None,
        },
        _ => None,
    };
    let nodes = module.named_metadata.iter().filter(|(one, _)| one == name).flat_map(|(_, nodes)| nodes);
    nodes
        .map(|node| {
            let operands = &module.metadata[node.0 as usize].operands;
            (operands.first().and_then(global), operands.iter().skip(1).filter_map(global).collect())
        })
        .collect()
}

/// `runtime`'s `!llrm.named` cells and `!llrm.writes` lists, as
/// `module`'s globals of the same names.
fn promised(
    module: &Module,
    runtime: &Module,
) -> (BTreeSet<GlobalId>, BTreeMap<GlobalId, BTreeSet<GlobalId>>) {
    let here = |id: GlobalId| runtime.global(id).name.as_deref().and_then(|name| module.named(name));
    let named = listed(runtime, "llrm.named")
        .into_iter()
        .flat_map(|(first, rest)| first.into_iter().chain(rest))
        .filter_map(here)
        .collect();
    let writes = listed(runtime, "llrm.writes")
        .into_iter()
        .filter_map(|(routine, cells)| Some((here(routine?)?, cells.into_iter().filter_map(here).collect())))
        .collect();
    (named, writes)
}

/// The globals one body's instructions name other than as a call's callee; with
/// `retained`, not those it passes as an argument the callee `noretain`s.
fn named_in(
    module: &Module,
    function: &llrm_mir::module::Function,
    retained: bool,
) -> BTreeSet<GlobalId> {
    let mut out = BTreeSet::new();
    for (_, inst) in function.walk() {
        let op = function.instruction(inst);
        let callee = matches!(op.opcode, Opcode::Call(_) | Opcode::Invoke(_)).then(|| op.operands.len() - 1);
        let named = op.operands.iter().enumerate().filter(|(at, _)| Some(*at) != callee);
        for (at, operand) in named {
            if let Operand::Constant(id) = operand
                && !(retained && llrm_mir::memory::noretain(&module.context, &module.globals, function, inst, at))
            {
                embedded(&module.context, *id, &mut out);
            }
        }
    }
    out
}

/// The globals an instruction names other than as a call's callee; with
/// `retained`, not those it passes as an argument the callee `noretain`s.
fn named_by_bodies(
    module: &Module,
    retained: bool,
) -> BTreeSet<GlobalId> {
    let mut out = BTreeSet::new();
    for global in &module.globals {
        if let GlobalKind::Function(function) = &global.kind {
            out.extend(named_in(module, function, retained));
        }
    }
    out
}

/// The globals an initializer or an instruction names other than as a
/// call's callee.
fn referenced(module: &Module) -> BTreeSet<GlobalId> {
    referenced_from(module, named_by_bodies(module, false))
}

/// `referenced`, the bodies' part found.
fn referenced_from(
    module: &Module,
    mut out: BTreeSet<GlobalId>,
) -> BTreeSet<GlobalId> {
    for global in &module.globals {
        if let GlobalKind::Variable(variable) = &global.kind {
            variable.initializer.iter().for_each(|&one| embedded(&module.context, one, &mut out));
        }
    }
    out
}

/// What the program's other modules do to one module's globals.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Elsewhere {
    /// Its globals whose address another module lets escape or holds in
    /// an initializer.
    pub escaped: BTreeSet<GlobalId>,
    /// Its declarations another module defines.
    pub defined: BTreeSet<GlobalId>,
    /// Its functions another module takes the address of.
    pub taken: BTreeSet<GlobalId>,
}

/// What one body contributes: the globals whose address it lets escape, and
/// those it names (plainly, and but as an argument its callee `noretain`s). Of
/// the body and the declarations alone.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Contribution {
    named: BTreeSet<GlobalId>,
    retained: BTreeSet<GlobalId>,
}

/// What `GlobalsAA` keeps for its next run: each body's contribution as of the
/// history it had, good while the declarations and the other modules' doing are
/// those it was made under.
#[derive(Default)]
pub struct Bodies {
    declarations: Option<Rc<Vec<GlobalValue>>>,
    elsewhere: Option<Elsewhere>,
    per: IndexMap<GlobalId, (Mark, Rc<Contribution>)>,
}

/// The variables of `module` no outside code reaches by name: not exported by
/// the program, defined in it and not let escape by another module. The
/// runtime module's `!llrm.named` globals (b$seg) are such: it names them and
/// never hands their address out.
pub fn unexported(
    module: &Module,
    analyses: &mut ModuleAnalyses,
) -> BTreeSet<GlobalId> {
    let program = Rc::clone(analyses.program());
    let elsewhere = match program.cached::<ProgramGlobals>() {
        Some(all) => Result::as_ref(&*all).map(|all| all[program.module].clone()).unwrap_or_default(),
        None => Elsewhere::default(),
    };
    module
        .globals
        .iter()
        .enumerate()
        .filter(|(at, global)| {
            matches!(global.kind, GlobalKind::Variable(_))
                && !program.exports.exported(global)
                && (defines(global) || elsewhere.defined.contains(&GlobalId(*at as u32)))
                && !elsewhere.escaped.contains(&GlobalId(*at as u32))
        })
        .map(|(at, _)| GlobalId(at as u32))
        .collect()
}

/// `module`'s, each body's shape from its manager in `analyses` and the
/// other modules as the cached `ProgramGlobals` finds them.
pub fn analysis(
    module: &Module,
    analyses: &mut ModuleAnalyses,
) -> Result<Globals, String> {
    let program = Rc::clone(analyses.program());
    let elsewhere = match program.cached::<ProgramGlobals>() {
        Some(all) => Result::as_ref(&*all).map_err(String::clone)?[program.module].clone(),
        None => Elsewhere::default(),
    };
    let declarations = analyses.get::<Declarations>(module);
    let mut memo = std::mem::take(analyses.memo::<Bodies>());
    if analyses.from_scratch()
        || !memo.declarations.as_ref().is_some_and(|then| Rc::ptr_eq(then, &declarations))
        || memo.elsewhere.as_ref() != Some(&elsewhere)
    {
        memo.per.clear();
    }
    memo.declarations = Some(declarations);
    memo.elsewhere = Some(elsewhere.clone());
    let forged = *analyses.get::<crate::manager::ForgedSelectors>(module);
    let found = found_with(module, &program, &elsewhere, forged, Some(&mut memo.per));
    *analyses.memo::<Bodies>() = memo;
    found
}

/// `module`'s under `program`, the other modules doing `elsewhere`, each
/// body's shape as `shape` gives it.
pub fn found(
    module: &Module,
    program: &ProgramProxy,
    elsewhere: &Elsewhere,
) -> Result<Globals, String> {
    found_with(module, program, elsewhere, crate::forging::forges_selector(module, program.spaces), None)
}

fn found_with(
    module: &Module,
    program: &ProgramProxy,
    elsewhere: &Elsewhere,
    forged: bool,
    mut kept: Option<&mut IndexMap<GlobalId, (Mark, Rc<Contribution>)>>,
) -> Result<Globals, String> {
    let (named, writes) = promised(module, &program.runtime);
    // Outside code reaches a global by name only where the program exports
    // it, or the runtime names it.
    let unexported = |id: GlobalId, global: &GlobalValue| {
        !program.exports.exported(global) && (defines(global) || elsewhere.defined.contains(&id))
    };
    let mut tracked = module
        .globals
        .iter()
        .enumerate()
        .map(|(at, global)| (GlobalId(at as u32), global))
        .filter(|&(id, global)| {
            matches!(global.kind, GlobalKind::Variable(_)) && (named.contains(&id) || unexported(id, global))
        })
        .map(|(id, _)| id)
        .filter(|id| !elsewhere.escaped.contains(id))
        .collect::<BTreeSet<_>>();
    let bodies = module.functions().filter(|(_, _, function)| !function.is_declaration()).collect::<Vec<_>>();
    let (mut plain, mut retained) = (BTreeSet::new(), BTreeSet::new());
    for &(id, _, function) in &bodies {
        let mark = function.mark();
        let reused = kept
            .as_ref()
            .and_then(|kept| kept.get(&id))
            .filter(|(then, _)| *then == mark)
            .map(|(_, one)| Rc::clone(one));
        let contribution = match reused {
            Some(one) => one,
            None => {
                let made = Rc::new(Contribution {
                    named: named_in(module, function, false),
                    retained: named_in(module, function, true),
                });
                if let Some(kept) = kept.as_mut() {
                    kept.insert(id, (mark, Rc::clone(&made)));
                }
                made
            }
        };
        plain.extend(contribution.named.iter().copied());
        retained.extend(contribution.retained.iter().copied());
    }
    // The use scan of `AnalyzeGlobals`, once for the module: a global whose
    // address escapes is not reached by name alone.
    let candidates = tracked.clone();
    for escaped in crate::globalsmodref::escaping_among(module, &tracked) {
        tracked.remove(&escaped);
    }
    // Of those, the ones a pointer phi or select, and no other use, lets
    // escape.
    let mut joined = candidates.difference(&tracked).copied().collect::<BTreeSet<_>>();
    for escaped in crate::globalsmodref::escaping_among_joins(module, &joined) {
        joined.remove(&escaped);
    }
    if let Some(kept) = kept.as_mut() {
        let live: BTreeSet<GlobalId> = bodies.iter().map(|(id, _, _)| *id).collect();
        kept.retain(|id, _| live.contains(id));
    }
    let held = held_from(module, retained);
    tracked.retain(|one| !held.contains(one));
    joined.retain(|one| !held.contains(one));
    // A pointer forged into the data segment reaches any global, one whose
    // address nobody computed among them (`forging`).
    if forged {
        tracked.clear();
        joined.clear();
    }
    let mut taken = referenced_from(module, plain);
    taken.extend(&elsewhere.taken);
    let entries = bodies
        .iter()
        .filter(|(id, global, _)| program.exports.exported(global) || taken.contains(id))
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    let bodies = bodies
        .iter()
        .map(|(id, _, _)| *id)
        .chain(elsewhere.defined.iter().copied().filter(|&one| module.global(one).function().is_some()))
        .collect();
    Ok(Globals { tracked, joined, forged, named, writes, bodies, entries, known: module.globals.len() as u32 })
}

/// The globals `module`'s initializers hold the address of. An initializer
/// is read only through its own global, so a private one nothing names,
/// directly or through another live initializer, holds nothing; naming it
/// only as an argument its callee `noretain`s is not naming it.
fn held(module: &Module) -> BTreeSet<GlobalId> {
    held_from(module, named_by_bodies(module, true))
}

/// `held`, the bodies' part found.
fn held_from(
    module: &Module,
    mut live: BTreeSet<GlobalId>,
) -> BTreeSet<GlobalId> {
    live.extend(
        module
            .globals
            .iter()
            .enumerate()
            .filter(|(_, global)| !matches!(global.linkage, Linkage::Internal | Linkage::Private))
            .map(|(at, _)| GlobalId(at as u32)),
    );
    let mut out = BTreeSet::new();
    let mut pending = live.iter().copied().collect::<Vec<_>>();
    while let Some(at) = pending.pop() {
        let GlobalKind::Variable(variable) = &module.global(at).kind else { continue };
        let mut inside = BTreeSet::new();
        variable.initializer.iter().for_each(|&one| embedded(&module.context, one, &mut inside));
        pending.extend(inside.iter().copied().filter(|one| live.insert(*one)));
        out.extend(inside);
    }
    out
}

/// A global as every module names it: the definition it resolves to, or
/// the name of one the program leaves undefined.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Symbol {
    Defined(usize, GlobalId),
    Named(String),
}

fn symbol(
    program: &Program,
    at: usize,
    id: GlobalId,
) -> Option<Symbol> {
    match program.definition(at, id) {
        Some((module, global)) => Some(Symbol::Defined(module, global)),
        None => program.modules[at].global(id).name.owned().map(Symbol::Named),
    }
}

/// What each module's globals undergo in the program's other modules.
pub struct ProgramGlobals;

impl ProgramAnalysis for ProgramGlobals {
    type Result = Result<Vec<Elsewhere>, String>;
    const NAME: &'static str = "program-globals";
    fn run(
        program: &Program,
        _: &mut ProgramAnalyses,
    ) -> Self::Result {
        let count = program.modules.len();
        if count < 2 {
            return Ok(vec![Elsewhere::default(); count]);
        }
        let mut escaped = Vec::new();
        let mut taken = Vec::new();
        for (at, module) in program.modules.iter().enumerate() {
            let mut ids = held(module);
            // The globals this module lets escape: the use scan of
            // `AnalyzeGlobals` over every global variable it names.
            let variables = module
                .globals
                .iter()
                .enumerate()
                .filter(|(_, global)| matches!(global.kind, GlobalKind::Variable(_)))
                .map(|(index, _)| GlobalId(index as u32))
                .collect::<BTreeSet<_>>();
            ids.extend(crate::globalsmodref::escaping_among(module, &variables));
            let symbols = |ids: BTreeSet<GlobalId>| {
                ids.into_iter().filter_map(|id| symbol(program, at, id)).collect::<BTreeSet<_>>()
            };
            escaped.push(symbols(ids));
            taken.push(symbols(referenced(module)));
        }
        let elsewhere = |sets: &[BTreeSet<Symbol>], at: usize, one: &Symbol| {
            sets.iter().enumerate().any(|(other, set)| other != at && set.contains(one))
        };
        Ok(program
            .modules
            .iter()
            .enumerate()
            .map(|(at, module)| {
                let mut out = Elsewhere::default();
                for (index, global) in module.globals.iter().enumerate() {
                    let id = GlobalId(index as u32);
                    if !defines(global) && program.definition(at, id).is_some() {
                        out.defined.insert(id);
                    }
                    let Some(one) = symbol(program, at, id) else { continue };
                    if elsewhere(&escaped, at, &one) {
                        out.escaped.insert(id);
                    }
                    if elsewhere(&taken, at, &one) {
                        out.taken.insert(id);
                    }
                }
                out
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "globalsaa_tests.rs"]
mod tests;
