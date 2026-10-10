//! Which defined function calls which, as LLVM's CallGraph: the order the
//! inliner and function-attrs visit functions in, callees first. An indirect
//! call reaches what its `!callees` lists; with none, any function, which
//! `calls_unknown` says of the caller.
//! A module's names its functions by id; a program's by module and id, a
//! declaration standing for the definition it resolves to.

use std::collections::{BTreeMap, BTreeSet};

use crate::context::{ConstantExpr, ConstantKind, GlobalId};
use crate::facts::Facts;
use crate::intrinsics::Intrinsic;
use crate::memory;
use crate::module::{Function, GlobalKind, InstId, Linkage, MetadataOperand, Module, Operand};
use crate::opcode::Opcode;
use crate::passes::{ModuleAnalyses, ModuleAnalysis};
use crate::program::{Program, ProgramAnalyses, ProgramAnalysis};

#[derive(Debug, PartialEq)]
pub struct CallGraph<N = GlobalId> {
    callees: BTreeMap<N, BTreeSet<N>>,
    /// The functions with a call whose callee is neither named nor listed.
    unknown: BTreeSet<N>,
    /// Each function's strongly connected component, found once.
    components: std::cell::OnceCell<BTreeMap<N, (usize, bool)>>,
}

/// A function of a program: its module's index and its id there.
pub type Defined = (usize, GlobalId);

/// What a call of `inst` may reach: the global it names, or the functions
/// `!callees` lists of an indirect call; `None` where it may reach any.
fn reached(
    module: &Module,
    function: &Function,
    inst: InstId,
) -> Option<Vec<GlobalId>> {
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
fn calls(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(
        function.instruction(inst).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    )
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
        Self { callees, unknown, components: Default::default() }
    }
}

impl CallGraph {
    /// The functions that can never be entered again while they run: in no
    /// cycle of calls, and neither they nor what they reach calls an unbounded
    /// pointer or a declaration that may call back (not `nocallback`, not
    /// an intrinsic): LLVM's `addNoRecurseAttrs`. Found for all at once, from
    /// the functions that do reach one and the callers of those, not by
    /// walking what each reaches (a chain of N reached N(N-1)/2).
    pub fn cannot_reenter(
        &self,
        module: &Module,
    ) -> BTreeSet<GlobalId> {
        let mut callers: BTreeMap<GlobalId, Vec<GlobalId>> = BTreeMap::new();
        let mut nodes: BTreeSet<GlobalId> = self.callees.keys().copied().collect();
        for (&caller, called) in &self.callees {
            for &callee in called {
                callers.entry(callee).or_default().push(caller);
                nodes.insert(callee);
            }
        }
        let mut reaching: BTreeSet<GlobalId> = nodes
            .iter()
            .copied()
            .filter(|&at| {
                let global = module.global(at);
                let Some(function) = global.function() else { return false };
                if function.is_declaration() {
                    return !(Facts::of(&function.attrs).no_callback()
                        || global
                            .name
                            .as_deref()
                            .and_then(Intrinsic::named)
                            .is_some_and(|one| !matches!(one, Intrinsic::Code | Intrinsic::Asm)));
                }
                self.calls_unknown(at)
            })
            .collect();
        let mut work: Vec<GlobalId> = reaching.iter().copied().collect();
        while let Some(at) = work.pop() {
            for &caller in callers.get(&at).into_iter().flatten() {
                if reaching.insert(caller) {
                    work.push(caller);
                }
            }
        }
        self.callees.keys().copied().filter(|&id| !self.recursive(id) && !reaching.contains(&id)).collect()
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
                        Some(found) => called.extend(
                            found.into_iter().map(|callee| program.definition(at, callee).unwrap_or((at, callee))),
                        ),
                        None => {
                            unknown.insert((at, id));
                        }
                    }
                }
                callees.insert((at, id), called);
            }
        }
        Self { callees, unknown, components: Default::default() }
    }
}

impl<N: Copy + Ord> CallGraph<N> {
    /// What `function` calls directly.
    pub fn callees_of(
        &self,
        function: N,
    ) -> Vec<N> {
        self.callees.get(&function).into_iter().flatten().copied().collect()
    }

    /// Every function `from` reaches, through its calls and theirs.
    pub fn reachable(
        &self,
        from: N,
    ) -> BTreeSet<N> {
        let mut seen = BTreeSet::new();
        let mut work: Vec<N> = self.callees.get(&from).into_iter().flatten().copied().collect();
        while let Some(at) = work.pop() {
            if seen.insert(at) {
                work.extend(self.callees.get(&at).into_iter().flatten().copied());
            }
        }
        seen
    }

    /// Whether `function` makes a call that may reach any function: through
    /// a pointer no `!callees` bounds.
    pub fn calls_unknown(
        &self,
        function: N,
    ) -> bool {
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

    /// A graph of these calls alone, as `new` makes of a module's.
    pub fn from_edges(callees: BTreeMap<N, BTreeSet<N>>) -> Self {
        Self { callees, unknown: BTreeSet::new(), components: Default::default() }
    }

    /// Each node's component and whether it is a cycle: more than one node,
    /// or a call to itself. Tarjan's, iteratively: the one answer to "does
    /// this call itself, and through whom".
    fn components(&self) -> &BTreeMap<N, (usize, bool)> {
        self.components.get_or_init(|| {
            // Over dense indices of the nodes, in order: no map is asked in
            // the walk.
            let nodes: BTreeSet<N> =
                self.callees.iter().flat_map(|(from, to)| std::iter::once(*from).chain(to.iter().copied())).collect();
            let nodes: Vec<N> = nodes.into_iter().collect();
            let at = |node: &N| nodes.binary_search(node).expect("a node of the graph");
            let mut out: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
            for (from, to) in &self.callees {
                out[at(from)] = to.iter().map(at).collect();
            }
            let (component, cyclic) = strong_components(&out);
            nodes.iter().enumerate().map(|(one, node)| (*node, (component[one], cyclic[component[one]]))).collect()
        })
    }

    /// The strongly connected components, callees before their callers, each
    /// with whether it is a cycle: what a bottom-up solver visits,
    /// iterating only within a cycle.
    pub fn bottom_up_components(&self) -> Vec<(Vec<N>, bool)> {
        let mut out: BTreeMap<usize, (Vec<N>, bool)> = BTreeMap::new();
        for (&node, &(component, cyclic)) in self.components() {
            if self.callees.contains_key(&node) {
                out.entry(component).or_insert_with(|| (Vec::new(), cyclic)).0.push(node);
            }
        }
        out.into_values().collect()
    }

    /// Whether `function` can call itself: it is in a cycle of calls.
    pub fn recursive(
        &self,
        function: N,
    ) -> bool {
        self.components().get(&function).is_some_and(|one| one.1)
    }

    /// Whether `one` and `other` are in one cycle of calls.
    pub fn together(
        &self,
        one: N,
        other: N,
    ) -> bool {
        let components = self.components();
        matches!(
            (components.get(&one), components.get(&other)),
            (Some(a), Some(b)) if a.1 && a.0 == b.0
        )
    }

    /// Whether `from` calls `to`, directly or not.
    pub fn reaches(
        &self,
        from: N,
        to: N,
    ) -> bool {
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
    fn run(
        module: &Module,
        _: &mut ModuleAnalyses,
    ) -> CallGraph {
        CallGraph::new(module)
    }
}

/// The program's call graph, as a program analysis.
pub struct ProgramCallGraph;

impl ProgramAnalysis for ProgramCallGraph {
    type Result = CallGraph<Defined>;
    const NAME: &'static str = "program-call-graph";
    fn run(
        program: &Program,
        _: &mut ProgramAnalyses,
    ) -> CallGraph<Defined> {
        CallGraph::of(program)
    }
}

/// Tarjan's, iteratively, over the nodes `0..callees.len()` and their callees
/// in order: each node's component, numbered callees first, and whether that
/// component is a cycle (more than one node, or a call to itself).
pub fn strong_components(callees: &[Vec<usize>]) -> (Vec<usize>, Vec<bool>) {
    const NONE: usize = usize::MAX;
    let count = callees.len();
    let (mut index, mut low) = (vec![NONE; count], vec![0; count]);
    let (mut on, mut stack, mut next) = (vec![false; count], Vec::<usize>::new(), 0);
    let (mut component, mut cyclic) = (vec![NONE; count], Vec::<bool>::new());
    for root in 0..count {
        if index[root] != NONE {
            continue;
        }
        let mut work = vec![(root, 0_usize)];
        while let Some((node, done)) = work.pop() {
            if done == 0 {
                index[node] = next;
                low[node] = next;
                next += 1;
                stack.push(node);
                on[node] = true;
            }
            if let Some(&to) = callees[node].get(done) {
                work.push((node, done + 1));
                if index[to] == NONE {
                    work.push((to, 0));
                } else if on[to] {
                    low[node] = low[node].min(index[to]);
                }
                continue;
            }
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == index[node] {
                let first = stack.iter().rposition(|one| *one == node).expect("on the stack");
                let members = stack.split_off(first);
                let this = cyclic.len();
                cyclic.push(members.len() > 1 || callees[node].contains(&node));
                for one in members {
                    on[one] = false;
                    component[one] = this;
                }
            }
        }
    }
    (component, cyclic)
}

#[cfg(test)]
#[path = "callgraph_tests.rs"]
mod tests;

/// Functions whose address is taken: named anywhere but as a callee, in a
/// global's initializer, a personality, or a metadata list (a call's `callees`,
/// which an indirect call may reach).
pub fn addressed(module: &Module) -> BTreeSet<GlobalId> {
    let context = &module.context;
    let mut out = BTreeSet::new();
    let mut work = Vec::new();
    for (_, _, function) in module.functions() {
        work.extend(function.personality);
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            let skip = usize::from(memory::callee(context, function, inst).is_some());
            let kept = instruction.operands.len() - skip;
            work.extend(
                instruction.operands[..kept]
                    .iter()
                    .filter_map(|&operand| if let Operand::Constant(id) = operand { Some(id) } else { None }),
            );
        }
    }
    work.extend(module.globals.iter().filter_map(|global| {
        if let GlobalKind::Variable(variable) = &global.kind { variable.initializer } else { None }
    }));
    for node in &module.metadata {
        work.extend(
            node.operands
                .iter()
                .filter_map(|operand| if let MetadataOperand::Constant(id) = operand { Some(*id) } else { None }),
        );
    }
    while let Some(id) = work.pop() {
        match &context.get(id).kind {
            ConstantKind::Global(global) => {
                out.insert(*global);
            }
            ConstantKind::Aggregate(members) => work.extend(members),
            ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => work.extend(operands),
            ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => work.push(*value),
            _ => {}
        }
    }
    out
}

/// A function attribute a frontend states: nothing outside the module's code
/// enters this function by a far call, so where every call of it is direct, it
/// may be entered near.
pub const NEAR_CODE: &str = "nearcode";

/// Defined functions only their own module's calls reach: internal or private,
/// not interrupt handlers, and never named but as a callee. What a call to one
/// may assume of its callers (where they are, what they pass) holds for every
/// call there is.
pub fn direct_only(module: &Module) -> BTreeSet<GlobalId> {
    let named = addressed(module);
    module
        .functions()
        .filter(|(id, global, function)| {
            !function.is_declaration()
                && matches!(global.linkage, Linkage::Internal | Linkage::Private)
                && function.calling_convention != crate::opcode::X86_INTR
                && !named.contains(id)
        })
        .map(|(id, _, _)| id)
        .collect()
}

/// Every plain direct call of each function, and the functions some call of
/// which is not one: an `invoke`, or a call with more or fewer arguments than
/// parameters.
pub struct DirectCalls {
    pub sites: BTreeMap<GlobalId, Vec<(GlobalId, InstId)>>,
    pub refused: BTreeSet<GlobalId>,
}

pub fn direct_calls(module: &Module) -> DirectCalls {
    let mut found = DirectCalls { sites: BTreeMap::new(), refused: BTreeSet::new() };
    for (caller, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            let Some(callee) = memory::callee(&module.context, function, inst) else { continue };
            match &instruction.opcode {
                Opcode::Call(_)
                    if instruction.operands.len()
                        == module.global(callee).function().map_or(0, |one| one.parameters().len()) + 1 =>
                {
                    found.sites.entry(callee).or_default().push((caller, inst));
                }
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    found.refused.insert(callee);
                }
                _ => {}
            }
        }
    }
    found
}
