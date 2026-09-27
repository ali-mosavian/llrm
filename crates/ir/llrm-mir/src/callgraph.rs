//! Which defined function calls which directly, as LLVM's CallGraph: the
//! order the inliner and function-attrs visit functions in, callees first.
//! A module's names its functions by id; a program's by module and id, a
//! declaration standing for the definition it resolves to.

use std::collections::{BTreeMap, BTreeSet};

use crate::context::GlobalId;
use crate::memory;
use crate::module::Module;
use crate::passes::{ModuleAnalyses, ModuleAnalysis};
use crate::program::{Program, ProgramAnalyses, ProgramAnalysis};

#[derive(Debug, PartialEq)]
pub struct CallGraph<N = GlobalId> {
    callees: BTreeMap<N, BTreeSet<N>>,
}

/// A function of a program: its module's index and its id there.
pub type Defined = (usize, GlobalId);

impl CallGraph {
    pub fn new(module: &Module) -> Self {
        let callees = module
            .functions()
            .filter(|(_, _, function)| !function.is_declaration())
            .map(|(id, _, function)| (id, function.walk().filter_map(|(_, inst)| memory::callee(&module.context, function, inst)).collect()))
            .collect();
        Self { callees }
    }
}

impl CallGraph<Defined> {
    /// `program`'s, each call to a declaration an edge to its definition.
    pub fn of(program: &Program) -> Self {
        let mut callees = BTreeMap::new();
        for (at, module) in program.modules.iter().enumerate() {
            for (id, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
                let called = function
                    .walk()
                    .filter_map(|(_, inst)| memory::callee(&module.context, function, inst))
                    .map(|callee| program.definition(at, callee).unwrap_or((at, callee)))
                    .collect();
                callees.insert((at, id), called);
            }
        }
        Self { callees }
    }
}

impl<N: Copy + Ord> CallGraph<N> {
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
