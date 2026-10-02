//! Which defined function calls which, as LLVM's CallGraph: the order the
//! inliner and function-attrs visit functions in, callees first. An indirect
//! call reaches what its `!callees` lists; with none, any function, which
//! `calls_unknown` says of the caller.
//! A module's names its functions by id; a program's by module and id, a
//! declaration standing for the definition it resolves to.

use std::collections::{BTreeMap, BTreeSet};

use crate::context::{ConstantKind, GlobalId};
use crate::memory;
use crate::module::{Function, InstId, MetadataOperand, Module};
use crate::opcode::Opcode;
use crate::passes::{ModuleAnalyses, ModuleAnalysis};
use crate::program::{Program, ProgramAnalyses, ProgramAnalysis};

#[derive(Debug, PartialEq)]
pub struct CallGraph<N = GlobalId> {
    callees: BTreeMap<N, BTreeSet<N>>,
    /// The functions with a call whose callee is neither named nor listed.
    unknown: BTreeSet<N>,
}

/// A function of a program: its module's index and its id there.
pub type Defined = (usize, GlobalId);

/// What a call of `inst` may reach: the global it names, or the functions
/// `!callees` lists of an indirect call; `None` where it may reach any.
fn reached(module: &Module, function: &Function, inst: InstId) -> Option<Vec<GlobalId>> {
    if let Some(callee) = memory::callee(&module.context, function, inst) {
        return Some(vec![callee]);
    }
    let node = function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "callees").map(|(_, node)| node)?;
    module
        .metadata
        .get(node.0 as usize)?
        .operands
        .iter()
        .map(|one| match one {
            MetadataOperand::Constant(at) => match module.context.get(*at).kind {
                ConstantKind::Global(global) => Some(global),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Whether `inst` is a call.
fn calls(function: &Function, inst: InstId) -> bool {
    matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_))
}

impl CallGraph {
    pub fn new(module: &Module) -> Self {
        let mut callees = BTreeMap::new();
        let mut unknown = BTreeSet::new();
        for (id, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
            let mut called = BTreeSet::new();
            for (_, inst) in function.walk().filter(|&(_, inst)| calls(function, inst)) {
                match reached(module, function, inst) {
                    Some(found) => called.extend(found),
                    None => {
                        unknown.insert(id);
                    }
                }
            }
            callees.insert(id, called);
        }
        Self { callees, unknown }
    }
}

impl CallGraph<Defined> {
    /// `program`'s, each call to a declaration an edge to its definition.
    pub fn of(program: &Program) -> Self {
        let mut callees = BTreeMap::new();
        let mut unknown = BTreeSet::new();
        for (at, module) in program.modules.iter().enumerate() {
            for (id, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
                let mut called = BTreeSet::new();
                for (_, inst) in function.walk().filter(|&(_, inst)| calls(function, inst)) {
                    match reached(module, function, inst) {
                        Some(found) => called.extend(found.into_iter().map(|callee| program.definition(at, callee).unwrap_or((at, callee)))),
                        None => {
                            unknown.insert((at, id));
                        }
                    }
                }
                callees.insert((at, id), called);
            }
        }
        Self { callees, unknown }
    }
}

impl<N: Copy + Ord> CallGraph<N> {
    /// Whether `function` makes a call that may reach any function: through
    /// a pointer no `!callees` bounds.
    pub fn calls_unknown(&self, function: N) -> bool {
        self.unknown.contains(&function)
    }

    /// Every defined function, callees before their callers.
    pub fn bottom_up(&self) -> Vec<N> {
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        let mut roots: Vec<N> = self.callees.keys().copied().collect();
        roots.sort();
        for root in roots {
            let mut stack = vec![(root, false)];
            while let Some((at, done)) = stack.pop() {
                if done {
                    order.push(at);
                    continue;
                }
                if !seen.insert(at) {
                    continue;
                }
                stack.push((at, true));
                for &next in self.callees.get(&at).into_iter().flatten().rev() {
                    if self.callees.contains_key(&next) && !seen.contains(&next) {
                        stack.push((next, false));
                    }
                }
            }
        }
        order
    }

    /// Whether `from` calls `to`, directly or not.
    pub fn reaches(&self, from: N, to: N) -> bool {
        let mut seen = BTreeSet::new();
        let mut work: Vec<N> = self.callees.get(&from).into_iter().flatten().copied().collect();
        while let Some(at) = work.pop() {
            if at == to {
                return true;
            }
            if seen.insert(at) {
                work.extend(self.callees.get(&at).into_iter().flatten().copied());
            }
        }
        false
    }
}

/// LLVM's `CallGraphAnalysis`.
pub struct CallGraphAnalysis;

impl ModuleAnalysis for CallGraphAnalysis {
    type Result = CallGraph;
    const NAME: &'static str = "call-graph";
    fn run(module: &Module, _: &mut ModuleAnalyses) -> CallGraph {
        CallGraph::new(module)
    }
}

/// The program's call graph, as a program analysis.
pub struct ProgramCallGraph;

impl ProgramAnalysis for ProgramCallGraph {
    type Result = CallGraph<Defined>;
    const NAME: &'static str = "program-call-graph";
    fn run(program: &Program, _: &mut ProgramAnalyses) -> CallGraph<Defined> {
        CallGraph::of(program)
    }
}

#[cfg(test)]
#[path = "callgraph_tests.rs"]
mod tests;
