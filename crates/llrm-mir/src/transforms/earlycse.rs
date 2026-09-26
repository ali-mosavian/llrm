//! Redundancy removed down the dominator tree: LLVM's EarlyCSE. A pure
//! instruction equal to one that dominates it is that one; a load of what a
//! dominating load read or store wrote, with no write since, is that value;
//! and a branch's condition is known on each edge it takes into a block
//! with no other way in.

use std::collections::HashMap;

use crate::context::{Constant, ConstantKind};
use crate::memory;
use crate::module::{BlockId, InstId, Operand, ValueId};
use crate::opcode::{BinaryOp, Flags, Opcode};
use crate::passes::{Analyses, Dominators, FunctionPass, PreservedAnalyses, Unit};
use crate::types::TypeId;

pub struct EarlyCse;

impl FunctionPass for EarlyCse {
    fn name(&self) -> &'static str {
        "earlycse"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let mut children: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
        for &block in unit.function.layout() {
            if let Some(parent) = tree.immediate_dominator(block).filter(|_| tree.is_reachable(block)) {
                children.entry(parent).or_default().push(block);
            }
        }
        let entry = unit.function.entry().expect("a defined function");
        let mut cse = Cse { unit, changed: false, generation: 0 };
        let mut stack = vec![(entry, Scope::default())];
        while let Some((block, scope)) = stack.pop() {
            let scope = cse.block(block, scope);
            for &child in children.get(&block).into_iter().flatten() {
                stack.push((child, scope.clone()));
            }
        }
        // Blocks and edges are as they were.
        if cse.changed { PreservedAnalyses::none().preserve::<Dominators>() } else { PreservedAnalyses::all() }
    }
}

/// A pure instruction's identity: what it computes, from what.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Key {
    opcode: Opcode,
    ty: TypeId,
    flags: Flags,
    operands: Vec<Operand>,
}

/// What a dominating block left known.
#[derive(Clone, Default)]
struct Scope {
    pure: HashMap<Key, ValueId>,
    /// A pointer's value, loaded or stored as a type, and the memory
    /// generation it was current in.
    memory: HashMap<(Operand, TypeId), (Operand, u64)>,
    conditions: HashMap<ValueId, bool>,
    generation: u64,
}

struct Cse<'u, 'a> {
    unit: &'u mut Unit<'a>,
    changed: bool,
    /// Bumped at every possible write, and at every block with more than
    /// one way in: what was current before is current nowhere after.
    generation: u64,
}

impl Cse<'_, '_> {
    fn fresh_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    fn block(&mut self, block: BlockId, mut scope: Scope) -> Scope {
        let function = &*self.unit.function;
        let predecessors = function.predecessors(block);
        match predecessors[..] {
            // Memory is as the only predecessor, the dominator, left it.
            [single] => {
                let branch = function.instruction(function.terminator(single).expect("a terminator"));
                if let [Operand::Value(condition), Operand::Block(taken), Operand::Block(otherwise)] = branch.operands[..]
                    && branch.opcode == Opcode::Br
                    && taken != otherwise
                {
                    scope.conditions.insert(condition, block == taken);
                }
            }
            _ => scope.generation = self.fresh_generation(),
        }
        for inst in self.unit.function.block(block).instructions().to_vec() {
            self.known_conditions(inst, &scope);
            self.instruction(inst, &mut scope);
        }
        scope
    }

    /// Operands whose truth an edge into here settled, made constants.
    fn known_conditions(&mut self, inst: InstId, scope: &Scope) {
        if self.unit.function.instruction(inst).opcode == Opcode::Phi {
            return;
        }
        for (at, operand) in self.unit.function.instruction(inst).operands.clone().into_iter().enumerate() {
            let Operand::Value(value) = operand else { continue };
            let Some(&truth) = scope.conditions.get(&value) else { continue };
            let ty = self.unit.function.value(value).ty;
            let known = self.unit.context.constant(Constant { ty, kind: ConstantKind::Int(u128::from(truth)) });
            self.unit.function.set_operand(inst, at, Operand::Constant(known));
            self.changed = true;
        }
    }

    fn replace(&mut self, inst: InstId, with: Operand) {
        let function = &mut *self.unit.function;
        let result = function.instruction(inst).result.expect("a value");
        function.replace_all_uses_with(result, with);
        function.erase(inst).expect("its uses were replaced");
        self.changed = true;
    }

    fn instruction(&mut self, inst: InstId, scope: &mut Scope) {
        let function = &*self.unit.function;
        let instruction = function.instruction(inst);
        match &instruction.opcode {
            Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::GetElementPtr { .. } | Opcode::Select
            | Opcode::FNeg | Opcode::ExtractValue(_) => {
                let mut operands = instruction.operands.clone();
                if let Opcode::Binary(BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) = instruction.opcode {
                    operands.sort_by_key(|one| format!("{one:?}"));
                }
                let key = Key { opcode: instruction.opcode.clone(), ty: instruction.ty, flags: instruction.flags, operands };
                let result = instruction.result.expect("a pure instruction's value");
                match scope.pure.get(&key) {
                    Some(&earlier) => self.replace(inst, Operand::Value(earlier)),
                    None => {
                        scope.pure.insert(key, result);
                    }
                }
            }
            Opcode::Load { volatile: false, .. } => {
                let (pointer, ty, result) = (instruction.operands[0], instruction.ty, instruction.result.expect("a load's value"));
                match scope.memory.get(&(pointer, ty)) {
                    Some(&(value, generation)) if generation == scope.generation => self.replace(inst, value),
                    _ => {
                        scope.memory.insert((pointer, ty), (Operand::Value(result), scope.generation));
                    }
                }
            }
            Opcode::Store { volatile: false, .. } => {
                let (value, pointer) = (instruction.operands[0], instruction.operands[1]);
                let ty = function.operand_type(self.unit.context, value).expect("a stored value's type");
                scope.generation = self.fresh_generation();
                scope.memory.insert((pointer, ty), (value, scope.generation));
            }
            _ => {
                if memory::of(self.unit.context, self.unit.callees, function, inst).writes {
                    scope.generation = self.fresh_generation();
                }
            }
        }
    }
}
