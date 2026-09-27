//! Which globals no code outside the module reaches but by name: LLVM's
//! GlobalsAA, GCC's ipa-reference, and in llrm-core the BC raise's private
//! segments and `spared`.
//!
//! A global is tracked when its address never leaves the module -- alias's
//! escape says so of every body, and no initializer holds it -- and either
//! its linkage is internal or private, or the frontend lists it in
//! `!llrm.named`: outside code names it but never hands its address out.
//! A tracked global is uncaptured (`memory::global_object`), so no
//! nonlocal reach meets it. What each body does to one, callees included,
//! is in alias's summaries; a callee no summary describes reaches the
//! tracked globals as `alias` asks here:
//!
//! - a body of the module: all of them;
//! - otherwise the named ones it writes -- those its `!llrm.writes` node
//!   lists, or all without one -- and, unless it is `nocallback`, what the
//!   module's entries do, as it may call back into them.
//!
//! `!llrm.named = !{!0}` with `!0 = !{ptr @g, ...}`; `!llrm.writes = !{!1,
//! ...}` with `!1 = !{ptr @routine, ptr @g, ...}`: of the named globals,
//! `@routine` writes by name only those listed.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::{ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{GlobalKind, InstId, Linkage, MetadataOperand, Module, Operand};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::target::Machine;

use crate::alias;
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
    let flagged = |attrs: &[Attribute]| attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == "nocallback"));
    let callee = llrm_mir::memory::callee(unit.context, unit.function, at).and_then(|one| unit.globals[one.0 as usize].function());
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

/// The globals an initializer or an instruction names other than as a
/// call's callee.
fn referenced(module: &Module) -> BTreeSet<GlobalId> {
    let mut out = BTreeSet::new();
    for global in &module.globals {
        match &global.kind {
            GlobalKind::Variable(variable) => variable.initializer.iter().for_each(|&one| embedded(&module.context, one, &mut out)),
            GlobalKind::Function(function) => {
                for (_, inst) in function.walk() {
                    let op = function.instruction(inst);
                    let callee = matches!(op.opcode, Opcode::Call(_) | Opcode::Invoke(_)).then(|| op.operands.len() - 1);
                    let named = op.operands.iter().enumerate().filter(|(at, _)| Some(*at) != callee);
                    for (_, operand) in named {
                        if let Operand::Constant(id) = operand {
                            embedded(&module.context, *id, &mut out);
                        }
                    }
                }
            }
        }
    }
    out
}

pub fn analysis(module: &Module, layout: &DataLayout, target: Option<&dyn Machine>) -> Result<Globals, String> {
    let named = listed(module, "llrm.named").into_iter().flat_map(|(first, rest)| first.into_iter().chain(rest)).collect::<BTreeSet<_>>();
    let writes = listed(module, "llrm.writes").into_iter().filter_map(|(routine, cells)| Some((routine?, cells))).collect();
    let mut tracked = module
        .globals
        .iter()
        .enumerate()
        .filter(|(at, global)| {
            matches!(global.kind, GlobalKind::Variable(_)) && (matches!(global.linkage, Linkage::Internal | Linkage::Private) || named.contains(&GlobalId(*at as u32)))
        })
        .map(|(at, _)| GlobalId(at as u32))
        .collect::<BTreeSet<_>>();
    let bodies = module.functions().filter(|(_, _, function)| !function.is_declaration()).collect::<Vec<_>>();
    for (_, _, function) in &bodies {
        let facts = alias::points_to(&Unit { machine: target, ..Unit::of(module, layout, function) }, None, None)?;
        for object in facts.escaped.iter().filter(|one| one.kind == MemoryKind::Global) {
            if let Some(Identity::Global(global)) = object.identity {
                tracked.remove(&GlobalId(global));
            }
        }
    }
    for global in &module.globals {
        if let GlobalKind::Variable(variable) = &global.kind {
            let mut held = BTreeSet::new();
            variable.initializer.iter().for_each(|&one| embedded(&module.context, one, &mut held));
            tracked.retain(|one| !held.contains(one));
        }
    }
    let taken = referenced(module);
    let entries = bodies
        .iter()
        .filter(|(id, global, _)| !matches!(global.linkage, Linkage::Internal | Linkage::Private) || taken.contains(id))
        .filter_map(|(_, global, _)| global.name.clone())
        .collect();
    Ok(Globals { tracked, named, writes, bodies: bodies.iter().map(|(id, _, _)| *id).collect(), entries })
}

#[cfg(test)]
#[path = "globalsaa_tests.rs"]
mod tests;
