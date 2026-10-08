//! Which globals no code outside the program reaches but by name: LLVM's
//! GlobalsAA, GCC's ipa-reference, and in llrm-core the BC raise's private
//! segments and `spared`.
//!
//! A global is tracked when no module lets its address escape -- alias's
//! escape says so of every body, and no initializer holds it -- and either
//! the program does not export it, or the program's runtime module lists
//! it in `!llrm.named`: the runtime names it but never hands its address
//! out.
//! A tracked global is uncaptured (`memory::global_object`), so no
//! nonlocal reach meets it. What each body does to one, callees included,
//! is in alias's summaries; a callee no summary describes reaches the
//! tracked globals as `alias` asks here:
//!
//! - a body of the module: all of them;
//! - otherwise the named ones it writes -- those its `!llrm.writes` node
//!   in the runtime module lists, or all without one -- and, unless it is
//!   `nocallback`, what the module's entries do, as it may call back into
//!   them.
//!
//! `!llrm.named = !{!0}` with `!0 = !{ptr @g, ...}`; `!llrm.writes = !{!1,
//! ...}` with `!1 = !{ptr @routine, ptr @g, ...}`: of the named globals,
//! `@routine` writes by name only those listed. The runtime module's
//! globals stand for each module's of the same names.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::{ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
use std::rc::Rc;

use llrm_mir::module::{GlobalKind, GlobalValue, InstId, Linkage, MetadataOperand, Module, Operand};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::passes::ModuleAnalyses;
use llrm_mir::program::{Program, ProgramAnalyses, ProgramAnalysis, ProgramProxy, defines};

use crate::alias;
use crate::cfg::Shape;
use crate::memory::{Identity, MemoryKind, Unit};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Globals {
    tracked: BTreeSet<GlobalId>,
    /// The `!llrm.named` globals.
    named: BTreeSet<GlobalId>,
    /// Each routine's `!llrm.writes` list.
    writes: BTreeMap<GlobalId, BTreeSet<GlobalId>>,
    /// The functions with a body here.
    bodies: BTreeSet<GlobalId>,
    /// The bodies outside code may call: not internal, or address-taken.
    entries: BTreeSet<String>,
}

impl Globals {
    pub fn tracked(&self, global: GlobalId) -> bool {
        self.tracked.contains(&global)
    }

    pub fn tracked_globals(&self) -> &BTreeSet<GlobalId> {
        &self.tracked
    }

    /// The tracked globals the call of `callee`, which no summary
    /// describes, may read and may write, but for callbacks.
    pub fn unsummarized(&self, callee: Option<GlobalId>) -> (BTreeSet<GlobalId>, BTreeSet<GlobalId>) {
        if callee.is_some_and(|one| self.bodies.contains(&one)) {
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
}

/// The globals `constant` holds the address of, through aggregates and
/// constant expressions.
pub fn embedded(context: &Context, constant: ConstantId, out: &mut BTreeSet<GlobalId>) {
    match &context.get(constant).kind {
        ConstantKind::Global(global) => {
            out.insert(*global);
        }
        ConstantKind::Aggregate(members) => members.iter().for_each(|&one| embedded(context, one, out)),
        ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => operands.iter().for_each(|&one| embedded(context, one, out)),
        ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => embedded(context, *value, out),
        _ => {}
    }
}

/// Whether the call `at` may call back into the module: LLVM's
/// `nocallback`, at the site or on the callee, says it may not.
pub fn calls_back(unit: &Unit, at: InstId) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &unit.function.instruction(at).opcode else { return false };
    let flagged = |attrs: &[Attribute]| llrm_mir::facts::Facts::of(attrs).no_callback();
    // A callee a pass declared after the outer facts were taken is not among them.
    let callee = llrm_mir::memory::callee(unit.context, unit.function, at).and_then(|one| unit.globals.get(one.0 as usize)?.function());
    !(flagged(&info.attrs) || callee.is_some_and(|one| flagged(&one.attrs)))
}

/// The first operand and the rest of each `!name` node, as globals.
fn listed(module: &Module, name: &str) -> Vec<(Option<GlobalId>, BTreeSet<GlobalId>)> {
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
fn promised(module: &Module, runtime: &Module) -> (BTreeSet<GlobalId>, BTreeMap<GlobalId, BTreeSet<GlobalId>>) {
    let here = |id: GlobalId| runtime.global(id).name.as_deref().and_then(|name| module.named(name));
    let named = listed(runtime, "llrm.named").into_iter().flat_map(|(first, rest)| first.into_iter().chain(rest)).filter_map(here).collect();
    let writes = listed(runtime, "llrm.writes").into_iter().filter_map(|(routine, cells)| Some((here(routine?)?, cells.into_iter().filter_map(here).collect()))).collect();
    (named, writes)
}

/// The globals an instruction names other than as a call's callee; with
/// `retained`, not those it passes as an argument the callee `noretain`s.
fn named_by_bodies(module: &Module, retained: bool) -> BTreeSet<GlobalId> {
    let mut out = BTreeSet::new();
    for global in &module.globals {
        if let GlobalKind::Function(function) = &global.kind {
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
        }
    }
    out
}

/// The globals an initializer or an instruction names other than as a
/// call's callee.
fn referenced(module: &Module) -> BTreeSet<GlobalId> {
    let mut out = named_by_bodies(module, false);
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

/// `module`'s, each body's shape from its manager in `analyses` and the
/// other modules as the cached `ProgramGlobals` finds them.
pub fn analysis(module: &Module, analyses: &mut ModuleAnalyses) -> Result<Globals, String> {
    let program = Rc::clone(analyses.program());
    let elsewhere = match program.cached::<ProgramGlobals>() {
        Some(all) => Result::as_ref(&*all).map_err(String::clone)?[program.module].clone(),
        None => Elsewhere::default(),
    };
    found(module, &program, &elsewhere, &mut |id| analyses.function::<Shape>(module, id))
}

/// `module`'s under `program`, the other modules doing `elsewhere`, each
/// body's shape as `shape` gives it.
pub fn found(module: &Module, program: &ProgramProxy, elsewhere: &Elsewhere, shape: &mut dyn FnMut(GlobalId) -> Rc<Shape>) -> Result<Globals, String> {
    let layout = &program.layout;
    let (named, writes) = promised(module, &program.runtime);
    // Outside code reaches a global by name only where the program exports
    // it, or the runtime names it.
    let unexported = |id: GlobalId, global: &GlobalValue| !program.exports.exported(global) && (defines(global) || elsewhere.defined.contains(&id));
    let mut tracked = module
        .globals
        .iter()
        .enumerate()
        .map(|(at, global)| (GlobalId(at as u32), global))
        .filter(|&(id, global)| matches!(global.kind, GlobalKind::Variable(_)) && (named.contains(&id) || unexported(id, global)))
        .map(|(id, _)| id)
        .filter(|id| !elsewhere.escaped.contains(id))
        .collect::<BTreeSet<_>>();
    let bodies = module.functions().filter(|(_, _, function)| !function.is_declaration()).collect::<Vec<_>>();
    for &(id, _, function) in &bodies {
        let shape = shape(id);
        let unit = Unit { program: Some(program), ..Unit::of(module, layout, function) };
        // Each access asks whether its frame object's address is exposed: found once for the body.
        let exposed = crate::memory::exposed_frames(&unit);
        let facts = alias::points_to(&unit.with_shape(&shape).with_exposed(&exposed), None, None)?;
        for object in facts.escaped.iter().filter(|one| one.kind == MemoryKind::Global) {
            if let Some(Identity::Global(global)) = object.identity {
                tracked.remove(&GlobalId(global));
            }
        }
    }
    tracked.retain(|one| !held(module).contains(one));
    let mut taken = referenced(module);
    taken.extend(&elsewhere.taken);
    let entries = bodies
        .iter()
        .filter(|(id, global, _)| program.exports.exported(global) || taken.contains(id))
        .filter_map(|(_, global, _)| global.name.clone())
        .collect();
    let bodies = bodies.iter().map(|(id, _, _)| *id).chain(elsewhere.defined.iter().copied().filter(|&one| module.global(one).function().is_some())).collect();
    Ok(Globals { tracked, named, writes, bodies, entries })
}

/// The globals `module`'s initializers hold the address of. An initializer
/// is read only through its own global, so a private one nothing names,
/// directly or through another live initializer, holds nothing; naming it
/// only as an argument its callee `noretain`s is not naming it.
fn held(module: &Module) -> BTreeSet<GlobalId> {
    let mut live = named_by_bodies(module, true);
    live.extend(module.globals.iter().enumerate().filter(|(_, global)| !matches!(global.linkage, Linkage::Internal | Linkage::Private)).map(|(at, _)| GlobalId(at as u32)));
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

fn symbol(program: &Program, at: usize, id: GlobalId) -> Option<Symbol> {
    match program.definition(at, id) {
        Some((module, global)) => Some(Symbol::Defined(module, global)),
        None => program.modules[at].global(id).name.clone().map(Symbol::Named),
    }
}

/// What each module's globals undergo in the program's other modules.
pub struct ProgramGlobals;

impl ProgramAnalysis for ProgramGlobals {
    type Result = Result<Vec<Elsewhere>, String>;
    const NAME: &'static str = "program-globals";
    fn run(program: &Program, analyses: &mut ProgramAnalyses) -> Self::Result {
        let count = program.modules.len();
        if count < 2 {
            return Ok(vec![Elsewhere::default(); count]);
        }
        let mut escaped = Vec::new();
        let mut taken = Vec::new();
        for (at, module) in program.modules.iter().enumerate() {
            let proxy = analyses.proxy(program, at);
            let mut ids = held(module);
            for (_, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
                let shape = Shape::of(function);
                let unit = Unit { program: Some(&proxy), ..Unit::of(module, &program.layout, function) };
                let exposed = crate::memory::exposed_frames(&unit);
                let facts = alias::points_to(&unit.with_shape(&shape).with_exposed(&exposed), None, None)?;
                ids.extend(facts.escaped.iter().filter(|one| one.kind == MemoryKind::Global).filter_map(|one| match one.identity {
                    Some(Identity::Global(global)) => Some(GlobalId(global)),
                    _ => None,
                }));
            }
            let symbols = |ids: BTreeSet<GlobalId>| ids.into_iter().filter_map(|id| symbol(program, at, id)).collect::<BTreeSet<_>>();
            escaped.push(symbols(ids));
            taken.push(symbols(referenced(module)));
        }
        let elsewhere = |sets: &[BTreeSet<Symbol>], at: usize, one: &Symbol| sets.iter().enumerate().any(|(other, set)| other != at && set.contains(one));
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
