//! Which defined function calls which directly, as LLVM's CallGraph: the
//! order the inliner and function-attrs visit functions in, callees first.

use std::collections::{BTreeSet, HashMap};

use crate::context::GlobalId;
use crate::memory;
use crate::module::Module;

pub struct CallGraph {
    callees: HashMap<GlobalId, BTreeSet<GlobalId>>,
}

impl CallGraph {
    pub fn new(module: &Module) -> Self {
        let callees = module
            .functions()
            .filter(|(_, _, function)| !function.is_declaration())
            .map(|(id, _, function)| (id, function.walk().filter_map(|(_, inst)| memory::callee(&module.context, function, inst)).collect()))
            .collect();
        Self { callees }
    }

    /// Every defined function, callees before their callers.
    pub fn bottom_up(&self) -> Vec<GlobalId> {
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        let mut roots: Vec<GlobalId> = self.callees.keys().copied().collect();
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
    pub fn reaches(&self, from: GlobalId, to: GlobalId) -> bool {
        let mut seen = BTreeSet::new();
        let mut work: Vec<GlobalId> = self.callees.get(&from).into_iter().flatten().copied().collect();
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
