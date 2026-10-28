//! LLVM's `CalledValuePropagationPass`: `!callees` on an indirect call whose
//! targets are few and known.
//!
//! # How it works
//! A sparse solver over a lattice of function sets (at most four functions,
//! then overdefined) in three groups: SSA registers, function returns and the
//! memory of a global variable only loaded and stored whole. Arguments of a
//! local function whose address is not taken are tracked through its direct
//! calls; returns of a function with an exact definition through its callers.
//! Control flow is followed as `SparseSolver` does: only executable blocks and
//! feasible edges are visited. After it settles, each indirect call whose
//! called value is a non-empty function set gets `!callees` listing them, by
//! name.
//!
//! # Example
//! ```text
//! @slot = internal global ptr @only        %f = load ptr, ptr @slot
//! %f = load ptr, ptr @slot                 call void %f(), !callees !{ptr @only}
//! call void %f()
//! ```
//!
//! # Runs
//! After IPSCCP (PassBuilderPipelines.cpp:1232), inside the early
//! interprocedural step.
//!
//! # See also
//! [CalledValuePropagation.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/CalledValuePropagation.cpp),
//! [SparsePropagation.h](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Analysis/SparsePropagation.h),
//! [ValueLatticeUtils.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/ValueLatticeUtils.cpp).

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::callgraph::addressed;
use llrm_mir::context::{Constant, ConstantExpr, ConstantId, ConstantKind, GlobalId};
use llrm_mir::module::{
    BlockId, GlobalKind, InstId, Linkage, MetadataId, MetadataNode, MetadataOperand, Module, Operand, ValueDef, ValueId,
};
use llrm_mir::opcode::{CastOp, Opcode};

use crate::sccp::{can_track_returns, has_exact_definition};

/// `cvp-max-functions-per-value`.
const MAX_FUNCTIONS_PER_VALUE: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Lattice {
    Undefined,
    /// The functions, sorted by name; none is a null pointer.
    Functions(Vec<GlobalId>),
    Overdefined,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Group {
    Register,
    Return,
    Memory,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Subject {
    Value(GlobalId, ValueId),
    Constant(ConstantId),
    Global(GlobalId),
}

type Key = (Group, Subject);

/// A value whose users the solver revisits when its state changes.
enum Work {
    Value(GlobalId, ValueId),
    Global(GlobalId),
}

struct Solver<'a> {
    module: &'a Module,
    addressed: BTreeSet<GlobalId>,
    /// The instructions naming each global directly as an operand.
    users: BTreeMap<GlobalId, Vec<(GlobalId, InstId)>>,
    /// The globals a constant expression or an initializer names.
    indirectly_used: BTreeSet<GlobalId>,
    state: BTreeMap<Key, Lattice>,
    executable: BTreeSet<(GlobalId, BlockId)>,
    edges: BTreeSet<(GlobalId, BlockId, BlockId)>,
    blocks: Vec<(GlobalId, BlockId)>,
    values: Vec<Work>,
    indirect: Vec<(GlobalId, InstId)>,
}

impl<'a> Solver<'a> {
    fn new(module: &'a Module) -> Self {
        let mut users: BTreeMap<GlobalId, Vec<(GlobalId, InstId)>> = BTreeMap::new();
        let mut indirectly_used = BTreeSet::new();
        let mut work: Vec<ConstantId> = Vec::new();
        for (id, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
            for (_, inst) in function.walk() {
                for &operand in &function.instruction(inst).operands {
                    let Operand::Constant(constant) = operand else { continue };
                    match module.context.get(constant).kind {
                        ConstantKind::Global(global) => users.entry(global).or_default().push((id, inst)),
                        _ => work.push(constant),
                    }
                }
            }
        }
        for global in &module.globals {
            if let GlobalKind::Variable(variable) = &global.kind {
                work.extend(variable.initializer);
            }
        }
        while let Some(constant) = work.pop() {
            match &module.context.get(constant).kind {
                ConstantKind::Global(global) => {
                    indirectly_used.insert(*global);
                }
                ConstantKind::Aggregate(members) => work.extend(members),
                ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => work.extend(operands),
                ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => work.push(*value),
                _ => {}
            }
        }
        Self {
            module,
            addressed: addressed(module),
            users,
            indirectly_used,
            state: BTreeMap::new(),
            executable: BTreeSet::new(),
            edges: BTreeSet::new(),
            blocks: Vec::new(),
            values: Vec::new(),
            indirect: Vec::new(),
        }
    }

    fn function(
        &self,
        id: GlobalId,
    ) -> &'a llrm_mir::module::Function {
        self.module.global(id).function().expect("a function")
    }

    fn name(
        &self,
        id: GlobalId,
    ) -> &str {
        self.module.global(id).name.as_deref().unwrap_or("")
    }

    /// `canTrackArgumentsInterprocedurally`.
    fn tracks_arguments(
        &self,
        id: GlobalId,
    ) -> bool {
        let global = self.module.global(id);
        let function = self.function(id);
        has_exact_definition(global.linkage, function)
            && matches!(global.linkage, Linkage::Internal | Linkage::Private)
            && !self.addressed.contains(&id)
    }

    /// `canTrackReturnsInterprocedurally`.
    fn tracks_returns(
        &self,
        id: GlobalId,
    ) -> bool {
        can_track_returns(self.module.global(id).linkage, self.function(id))
    }

    /// `canTrackGlobalVariableInterprocedurally`.
    fn tracks_variable(
        &self,
        id: GlobalId,
    ) -> bool {
        let global = self.module.global(id);
        let GlobalKind::Variable(variable) = &global.kind else { return false };
        if variable.constant
            || !matches!(global.linkage, Linkage::Internal | Linkage::Private)
            || variable.initializer.is_none()
            || variable.externally_initialized
            || self.indirectly_used.contains(&id)
        {
            return false;
        }
        self.users
            .get(&id)
            .into_iter()
            .flatten()
            .all(
                |&(caller, inst)| {
                    let function = self.function(caller);
                    let instruction = function.instruction(inst);
                    let ty_of = |operand: Operand| function.operand_type(&self.module.context, operand);
                    let named = |operand: Operand| {
                        matches!(
                            operand,
                            Operand::Constant(c) if self.module.context.get(c).kind == ConstantKind::Global(id)
                        )
                    };
                    match instruction.opcode {
                        Opcode::Store { volatile, .. } => {
                            !volatile
                                && named(instruction.operands[1])
                                && !named(instruction.operands[0])
                                && ty_of(instruction.operands[0]) == Some(variable.ty)
                        }
                        Opcode::Load { volatile, .. } => !volatile && instruction.ty == variable.ty,
                        _ => false,
                    }
                },
            )
    }

    fn key(
        &self,
        function: GlobalId,
        operand: Operand,
    ) -> Key {
        match operand {
            Operand::Value(value) => (Group::Register, Subject::Value(function, value)),
            Operand::Constant(constant) => (Group::Register, Subject::Constant(constant)),
            Operand::Block(_) => unreachable!("a block is no value"),
        }
    }

    /// `computeConstant`: a null pointer is no function, a function (through
    /// pointer casts) is itself.
    fn constant(
        &self,
        constant: ConstantId,
    ) -> Lattice {
        let mut at = constant;
        loop {
            match &self.module.context.get(at).kind {
                ConstantKind::Null => return Lattice::Functions(Vec::new()),
                ConstantKind::Global(global) if self.module.global(*global).function().is_some() => {
                    return Lattice::Functions(vec![*global]);
                }
                ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::BitCast | CastOp::AddrSpaceCast, value }) => {
                    at = *value
                }
                _ => return Lattice::Overdefined,
            }
        }
    }

    /// `ComputeLatticeVal`.
    fn computed(
        &self,
        key: Key,
    ) -> Lattice {
        match key {
            (Group::Register, Subject::Value(function, value)) => match self.function(function).value(value).def {
                ValueDef::Instruction(_) => Lattice::Undefined,
                ValueDef::Argument(_) if self.tracks_arguments(function) => Lattice::Undefined,
                ValueDef::Argument(_) => Lattice::Overdefined,
            },
            (Group::Register, Subject::Constant(constant)) => self.constant(constant),
            (Group::Memory | Group::Return, Subject::Global(global)) => match &self.module.global(global).kind {
                GlobalKind::Variable(variable) if self.tracks_variable(global) => {
                    variable.initializer.map_or(Lattice::Overdefined, |init| self.constant(init))
                }
                GlobalKind::Function(_) if self.tracks_returns(global) => Lattice::Undefined,
                _ => Lattice::Overdefined,
            },
            _ => Lattice::Overdefined,
        }
    }

    /// `getValueState`.
    fn get(
        &mut self,
        key: Key,
    ) -> Lattice {
        if let Some(found) = self.state.get(&key) {
            return found.clone();
        }
        let made = self.computed(key);
        self.state.insert(key, made.clone());
        made
    }

    /// `MergeValues`.
    fn merge(
        &self,
        x: &Lattice,
        y: &Lattice,
    ) -> Lattice {
        match (x, y) {
            (Lattice::Overdefined, _) | (_, Lattice::Overdefined) => Lattice::Overdefined,
            (Lattice::Undefined, Lattice::Undefined) => Lattice::Undefined,
            _ => {
                let empty = Vec::new();
                let list = |one: &'_ Lattice| match one {
                    Lattice::Functions(found) => found.clone(),
                    _ => empty.clone(),
                };
                let mut union = list(x);
                union.extend(list(y));
                union.sort_by(|a, b| self.name(*a).cmp(self.name(*b)));
                union.dedup();
                if union.len() > MAX_FUNCTIONS_PER_VALUE { Lattice::Overdefined } else { Lattice::Functions(union) }
            }
        }
    }

    /// `UpdateState`.
    fn update(
        &mut self,
        key: Key,
        value: Lattice,
    ) {
        if self.state.get(&key) == Some(&value) {
            return;
        }
        self.state.insert(key, value);
        match key.1 {
            Subject::Value(function, value) => self.values.push(Work::Value(function, value)),
            Subject::Global(global) => self.values.push(Work::Global(global)),
            Subject::Constant(_) => {}
        }
    }

    fn mark_block(
        &mut self,
        function: GlobalId,
        block: BlockId,
    ) {
        if self.executable.insert((function, block)) {
            self.blocks.push((function, block));
        }
    }

    fn mark_edge(
        &mut self,
        function: GlobalId,
        from: BlockId,
        to: BlockId,
    ) {
        if !self.edges.insert((function, from, to)) {
            return;
        }
        if self.executable.contains(&(function, to)) {
            let body = self.function(function);
            let phis: Vec<InstId> =
                body.iter_block(to).take_while(|&inst| body.instruction(inst).opcode == Opcode::Phi).collect();
            for phi in phis {
                self.visit_phi(function, phi);
            }
        } else {
            self.mark_block(function, to);
        }
    }

    /// `getFeasibleSuccessors`, `AggressiveUndef`: the blocks the terminator
    /// `term` may go to, once per slot.
    fn feasible(
        &mut self,
        function: GlobalId,
        term: InstId,
    ) -> Vec<BlockId> {
        let instruction = self.function(function).instruction(term);
        let targets: Vec<BlockId> = instruction
            .operands
            .iter()
            .filter_map(|operand| if let Operand::Block(block) = operand { Some(*block) } else { None })
            .collect();
        let conditional = match instruction.opcode {
            Opcode::Br => targets.len() == 2,
            Opcode::Switch => true,
            _ => false,
        };
        if conditional {
            let key = self.key(function, instruction.operands[0]);
            if self.get(key) == Lattice::Undefined {
                return Vec::new();
            }
        }
        targets
    }

    /// `visitPHINode`.
    fn visit_phi(
        &mut self,
        function: GlobalId,
        phi: InstId,
    ) {
        let body = self.function(function);
        let instruction = body.instruction(phi);
        let Some(result) = instruction.result else { return };
        let key = (Group::Register, Subject::Value(function, result));
        let mut value = self.get(key);
        if matches!(value, Lattice::Overdefined) {
            return;
        }
        if instruction.operands.len() / 2 > 64 {
            self.update(key, Lattice::Overdefined);
            return;
        }
        let parent = body.parent(phi).expect("a placed phi");
        for pair in instruction.operands.chunks(2) {
            let [incoming, Operand::Block(from)] = pair else { continue };
            let Some(term) = body.terminator(*from) else { continue };
            if !self.feasible(function, term).contains(&parent) {
                continue;
            }
            let other = self.get(self.key(function, *incoming));
            if other != value {
                value = self.merge(&value, &other);
            }
            if matches!(value, Lattice::Overdefined) {
                break;
            }
        }
        self.update(key, value);
    }

    /// `visitInst` and `ComputeInstructionState`.
    fn visit(
        &mut self,
        function: GlobalId,
        inst: InstId,
    ) {
        let body = self.function(function);
        let instruction = body.instruction(inst);
        if instruction.opcode == Opcode::Phi {
            return self.visit_phi(function, inst);
        }
        let mut changed: Vec<(Key, Lattice)> = Vec::new();
        let register = instruction.result.map(|result| (Group::Register, Subject::Value(function, result)));
        match &instruction.opcode {
            Opcode::Call(_) | Opcode::Invoke(_) => self.visit_call(function, inst, &mut changed),
            Opcode::Load { .. } => {
                if let Some(register) = register {
                    let memory = match instruction.operands[0] {
                        Operand::Constant(c) => match self.module.context.get(c).kind {
                            ConstantKind::Global(global) if self.module.global(global).function().is_none() => {
                                Some((Group::Memory, Subject::Global(global)))
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    let value = match memory {
                        Some(memory) => {
                            let (a, b) = (self.get(register), self.get(memory));
                            self.merge(&a, &b)
                        }
                        None => Lattice::Overdefined,
                    };
                    changed.push((register, value));
                }
            }
            Opcode::Ret => {
                if let Some(&value) = instruction.operands.first() {
                    let ret = (Group::Return, Subject::Global(function));
                    let (a, b) = (self.get(self.key(function, value)), self.get(ret));
                    changed.push((ret, self.merge(&a, &b)));
                }
            }
            Opcode::Select => {
                if let Some(register) = register {
                    let (a, b) = (
                        self.get(self.key(function, instruction.operands[1])),
                        self.get(self.key(function, instruction.operands[2])),
                    );
                    changed.push((register, self.merge(&a, &b)));
                }
            }
            Opcode::Store { .. } => {
                if let Operand::Constant(c) = instruction.operands[1]
                    && let ConstantKind::Global(global) = self.module.context.get(c).kind
                    && self.module.global(global).function().is_none()
                {
                    let memory = (Group::Memory, Subject::Global(global));
                    let (a, b) = (self.get(self.key(function, instruction.operands[0])), self.get(memory));
                    changed.push((memory, self.merge(&a, &b)));
                }
            }
            _ => {
                if let Some(register) = register
                    && body.users(instruction.result.expect("a result")).into_iter().next().is_some()
                {
                    changed.push((register, Lattice::Overdefined));
                }
            }
        }
        for (key, value) in changed {
            self.update(key, value);
        }
        if instruction.opcode.is_terminator() {
            let from = body.parent(inst).expect("a placed terminator");
            for to in self.feasible(function, inst) {
                self.mark_edge(function, from, to);
            }
        }
    }

    /// `visitCallBase`.
    fn visit_call(
        &mut self,
        function: GlobalId,
        inst: InstId,
        changed: &mut Vec<(Key, Lattice)>,
    ) {
        let body = self.function(function);
        let instruction = body.instruction(inst);
        let register = instruction.result.map(|result| (Group::Register, Subject::Value(function, result)));
        let callee = llrm_mir::memory::callee(&self.module.context, body, inst);
        if callee.is_none() && !self.indirect.contains(&(function, inst)) {
            self.indirect.push((function, inst));
        }
        let Some(callee) = callee.filter(|&callee| self.tracks_returns(callee)) else {
            if let Some(register) = register {
                changed.push((register, Lattice::Overdefined));
            }
            return;
        };
        let target = self.function(callee);
        if let Some(entry) = target.entry() {
            self.mark_block(callee, entry);
        }
        for (at, &formal) in target.parameters().iter().enumerate() {
            let Some(&actual) = instruction.operands.get(at) else { continue };
            let formal = (Group::Register, Subject::Value(callee, formal));
            let (a, b) = (self.get(formal), self.get(self.key(function, actual)));
            changed.push((formal, self.merge(&a, &b)));
        }
        if let Some(register) = register {
            let (a, b) = (self.get(register), self.get((Group::Return, Subject::Global(callee))));
            changed.push((register, self.merge(&a, &b)));
        }
    }

    /// `Solve`.
    fn solve(&mut self) {
        while !self.blocks.is_empty() || !self.values.is_empty() {
            while let Some(work) = self.values.pop() {
                let users: Vec<(GlobalId, InstId)> = match work {
                    Work::Value(function, value) => {
                        self.function(function).users(value).into_iter().map(|one| (function, one.user)).collect()
                    }
                    Work::Global(global) => self.users.get(&global).cloned().unwrap_or_default(),
                };
                for (function, inst) in users {
                    let body = self.function(function);
                    if body.is_erased(inst) {
                        continue;
                    }
                    if body.parent(inst).is_some_and(|block| self.executable.contains(&(function, block))) {
                        self.visit(function, inst);
                    }
                }
            }
            while let Some((function, block)) = self.blocks.pop() {
                let body = self.function(function);
                for inst in body.iter_block(block).collect::<Vec<_>>() {
                    self.visit(function, inst);
                }
            }
        }
    }
}

/// `runCVP`: the functions whose indirect calls got `!callees`.
pub fn propagated(module: &mut Module) -> Vec<GlobalId> {
    let attach: Vec<(GlobalId, InstId, Vec<GlobalId>)> = {
        let mut solver = Solver::new(module);
        let tracked: Vec<GlobalId> = module
            .functions()
            .filter(|(id, _, function)| !function.is_declaration() && !solver.tracks_arguments(*id))
            .map(|(id, _, _)| id)
            .collect();
        for id in tracked {
            if let Some(entry) = solver.function(id).entry() {
                solver.mark_block(id, entry);
            }
        }
        solver.solve();
        let mut found = Vec::new();
        for &(function, inst) in &solver.indirect.clone() {
            let body = solver.function(function);
            let Some(&called) = body.instruction(inst).operands.last() else { continue };
            let key = solver.key(function, called);
            if let Some(Lattice::Functions(list)) = solver.state.get(&key)
                && !list.is_empty()
            {
                found.push((function, inst, list.clone()));
            }
        }
        found
    };
    let mut changed = BTreeSet::new();
    for (function, inst, list) in attach {
        let operands: Vec<MetadataOperand> = list
            .iter()
            .map(|&global| {
                let ty = module.context.types.ptr(module.global(global).address_space);
                MetadataOperand::Constant(module.context.constant(Constant { ty, kind: ConstantKind::Global(global) }))
            })
            .collect();
        let node = match module.metadata.iter().position(|one| !one.distinct && one.operands == operands) {
            Some(at) => MetadataId(at as u32),
            None => {
                module.metadata.push(MetadataNode { distinct: false, operands });
                MetadataId((module.metadata.len() - 1) as u32)
            }
        };
        if let GlobalKind::Function(body) = &mut module.globals[function.0 as usize].kind {
            body.set_metadata(inst, "callees", Some(node));
            changed.insert(function);
        }
    }
    changed.into_iter().collect()
}

#[cfg(test)]
#[path = "calledvaluepropagation_tests.rs"]
mod tests;
