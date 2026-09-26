//! Redundancy removed down the dominator tree: LLVM's EarlyCSE. A pure
//! instruction equal to one that dominates it is that one; a load of what a
//! dominating load read or store wrote, with no write since or none
//! possible, is that value;
//! and a branch's condition is known on each edge it takes into a block
//! with no other way in.

use std::collections::HashMap;

use crate::context::{Constant, ConstantKind};
use crate::alias::{alias, captured, contains, object, Alias, Location, Object};
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
        let mut cse = Cse { unit, changed: false };
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

/// What memory is known to hold: a value loaded or stored as a type, or
/// bytes a `memset` zeroed.
#[derive(Clone)]
enum Known {
    Value { at: Location, ty: TypeId, value: Operand },
    /// Zeroed, but for the byte ranges since written, as offsets from the
    /// object `at` lies in.
    Zero { at: Location, holes: Vec<(i64, i64)> },
}

impl Known {
    fn at(&self) -> Location {
        match *self {
            Known::Value { at, .. } | Known::Zero { at, .. } => at,
        }
    }
}

/// What a dominating block left known.
#[derive(Clone, Default)]
struct Scope {
    pure: HashMap<Key, ValueId>,
    memory: Vec<Known>,
    conditions: HashMap<ValueId, bool>,
}

struct Cse<'u, 'a> {
    unit: &'u mut Unit<'a>,
    changed: bool,
}

impl Cse<'_, '_> {
    /// What survives a write to `at`: what it cannot overlap, and a
    /// zeroed range with a hole where it lands.
    fn written(&self, scope: &mut Scope, at: Location) {
        let unit = &*self.unit;
        let (base, offset) = crate::valuetracking::underlying(unit.context, unit.layout, unit.function, at.pointer);
        scope.memory.retain_mut(|known| {
            if memory::invariant(unit.context, unit.layout, unit.function, known.at().pointer)
                || alias(unit.context, unit.layout, unit.callees, unit.function, known.at(), at) == Alias::No
            {
                return true;
            }
            let Known::Zero { at: zeroed, holes } = known else { return false };
            match (crate::valuetracking::underlying(unit.context, unit.layout, unit.function, zeroed.pointer), offset) {
                ((there, Some(_)), Some(offset)) if there == base => {
                    holes.push((offset, offset + at.bytes as i64));
                    true
                }
                _ => false,
            }
        });
    }

    /// What survives a write to memory no location names: the invariant,
    /// and slots whose address never escapes.
    fn clobbered(&self, scope: &mut Scope) {
        let unit = &*self.unit;
        scope.memory.retain(|known| {
            let pointer = known.at().pointer;
            let (base, _) = crate::valuetracking::underlying(unit.context, unit.layout, unit.function, pointer);
            memory::invariant(unit.context, unit.layout, unit.function, pointer)
                || matches!(object(unit.context, unit.function, base), Some(Object::Slot(slot)) if !captured(unit.context, unit.callees, unit.function, slot))
        });
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
            _ => {
                let unit = &*self.unit;
                scope.memory.retain(|known| memory::invariant(unit.context, unit.layout, unit.function, known.at().pointer));
            }
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
                let at = Location { pointer, bytes: self.unit.layout.store_size(&self.unit.context.types, ty) };
                match self.known(scope, at, ty) {
                    Some(value) => self.replace(inst, value),
                    None => scope.memory.push(Known::Value { at, ty, value: Operand::Value(result) }),
                }
            }
            Opcode::Store { volatile: false, .. } => {
                let (value, pointer) = (instruction.operands[0], instruction.operands[1]);
                let ty = function.operand_type(self.unit.context, value).expect("a stored value's type");
                let at = Location { pointer, bytes: self.unit.layout.store_size(&self.unit.context.types, ty) };
                self.written(scope, at);
                scope.memory.push(Known::Value { at, ty, value });
            }
            _ => {
                if !memory::of(self.unit.context, self.unit.callees, function, inst).writes {
                    return;
                }
                match memory::memset(self.unit.context, self.unit.callees, function, inst) {
                    Some((pointer, byte, length)) => {
                        let bytes = match length {
                            Operand::Constant(id) => match self.unit.context.get(id).kind {
                                ConstantKind::Int(bits) => Some(bits as u64),
                                _ => None,
                            },
                            _ => None,
                        };
                        let zero = matches!(byte, Operand::Constant(id) if self.unit.context.get(id).kind == ConstantKind::Int(0));
                        let at = Location { pointer, bytes: bytes.unwrap_or(u64::MAX / 2) };
                        self.written(scope, at);
                        if zero && bytes.is_some() {
                            scope.memory.push(Known::Zero { at, holes: Vec::new() });
                        }
                    }
                    None => self.clobbered(scope),
                }
            }
        }
    }

    /// What a load of `ty` at `at` reads, if memory is known to hold it.
    fn known(&mut self, scope: &Scope, at: Location, ty: TypeId) -> Option<Operand> {
        let unit = &*self.unit;
        for known in scope.memory.iter().rev() {
            match known {
                Known::Value { at: there, ty: stored, value } if *stored == ty && alias(unit.context, unit.layout, unit.callees, unit.function, *there, at) == Alias::Must => {
                    return Some(*value);
                }
                Known::Zero { at: there, holes } if contains(unit.context, unit.layout, unit.function, *there, at) => {
                    let (_, Some(offset)) = crate::valuetracking::underlying(unit.context, unit.layout, unit.function, at.pointer) else { return None };
                    let end = offset + at.bytes as i64;
                    if holes.iter().any(|&(start, stop)| start < end && offset < stop) {
                        return None;
                    }
                    let kind = match unit.context.types.get(ty) {
                        crate::types::Type::Int(_) => ConstantKind::Int(0),
                        crate::types::Type::Pointer(_) => ConstantKind::Null,
                        crate::types::Type::Float(_) => ConstantKind::Float(0),
                        _ => return None,
                    };
                    return Some(Operand::Constant(self.unit.context.constant(Constant { ty, kind })));
                }
                _ => {}
            }
        }
        None
    }
}
