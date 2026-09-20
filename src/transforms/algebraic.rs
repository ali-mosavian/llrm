//! Exact algebraic identities for portable integer IR.
//!
//! This pass deliberately recognizes only identities which are valid for the
//! declared fixed-width integer type.  It does not turn potentially trapping
//! instructions into constants and leaves malformed typed IR for verification
//! to diagnose.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::ir::{
    BinaryOp, Constant, Function, Instruction, InstructionKind, Module, Operand, TypeId, TypeKind,
    TypedConstant, ValueId,
};

use super::rewrite::replace_value_uses;
use super::{FunctionPass, PassFailure, PassOutcome, PreservedAnalyses};

/// A construction error for [`AlgebraicSimplify`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AlgebraicError {
    /// The module declares one type identity more than once.
    DuplicateType(TypeId),
    /// An integer type cannot be represented by the portable integer oracle.
    InvalidIntegerWidth { type_id: TypeId, bits: u16 },
}

impl fmt::Display for AlgebraicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateType(type_id) => write!(formatter, "duplicate type id {type_id}"),
            Self::InvalidIntegerWidth { type_id, bits } => {
                write!(
                    formatter,
                    "integer type {type_id} has unsupported width {bits}"
                )
            }
        }
    }
}

impl std::error::Error for AlgebraicError {}

/// Simplifies safe fixed-width integer identities.
///
/// The pass removes only the binary operation itself.  Its operands are SSA
/// values or constants and have therefore already been evaluated.  It never
/// removes division except by the nonzero constant `+1`, nor does it use
/// identities such as `x * 0` or `x / x` that could suppress a trap.
#[derive(Clone, Debug)]
pub struct AlgebraicSimplify {
    integer_widths: BTreeMap<TypeId, u16>,
}

impl AlgebraicSimplify {
    /// Builds an algebraic simplifier from the module's integer declarations.
    pub fn new(module: &Module) -> Result<Self, AlgebraicError> {
        let mut integer_widths = BTreeMap::new();
        let mut declared = BTreeSet::new();

        for type_ in &module.types {
            if !declared.insert(type_.id) {
                return Err(AlgebraicError::DuplicateType(type_.id));
            }
            if let TypeKind::Integer { bits } = type_.kind {
                if !(1..=128).contains(&bits) {
                    return Err(AlgebraicError::InvalidIntegerWidth {
                        type_id: type_.id,
                        bits,
                    });
                }
                integer_widths.insert(type_.id, bits);
            }
        }

        Ok(Self { integer_widths })
    }

    fn integer_constant(&self, operand: &Operand) -> Option<IntegerConstant> {
        let Operand::Constant(constant) = operand else {
            return None;
        };
        let Constant::Integer(value) = &constant.value else {
            return None;
        };
        let bits = self.integer_widths.get(&constant.type_id).copied()?;
        Some(IntegerConstant {
            raw: (*value as u128) & integer_mask(bits),
        })
    }

    fn operand_type(
        &self,
        operand: &Operand,
        value_types: &BTreeMap<ValueId, TypeId>,
        duplicate_values: &BTreeSet<ValueId>,
    ) -> Option<TypeId> {
        match operand {
            Operand::Value(value) if !duplicate_values.contains(value) => {
                value_types.get(value).copied()
            }
            Operand::Value(_) => None,
            Operand::Constant(constant) => Some(constant.type_id),
        }
    }

    fn simplify_instruction(
        &self,
        instruction: &Instruction,
        value_types: &BTreeMap<ValueId, TypeId>,
        duplicate_values: &BTreeSet<ValueId>,
    ) -> Option<(ValueId, Operand)> {
        let [result] = instruction.results.as_slice() else {
            return None;
        };
        if duplicate_values.contains(&result.id) {
            return None;
        }
        let InstructionKind::Binary { op, left, right } = &instruction.kind else {
            return None;
        };
        let bits = self.integer_widths.get(&result.type_id).copied()?;
        if self.operand_type(left, value_types, duplicate_values) != Some(result.type_id)
            || self.operand_type(right, value_types, duplicate_values) != Some(result.type_id)
            || matches!(left, Operand::Value(value) if *value == result.id)
            || matches!(right, Operand::Value(value) if *value == result.id)
        {
            return None;
        }

        let left_constant = self.integer_constant(left);
        let right_constant = self.integer_constant(right);
        let zero = |constant: Option<IntegerConstant>| constant.is_some_and(|value| value.raw == 0);
        let one = |constant: Option<IntegerConstant>| constant.is_some_and(|value| value.raw == 1);
        let all_ones = |constant: Option<IntegerConstant>| {
            constant.is_some_and(|value| value.raw == integer_mask(bits))
        };
        let same = left == right;

        let replacement = match op {
            BinaryOp::Add if zero(right_constant) => Some(left.clone()),
            BinaryOp::Add if zero(left_constant) => Some(right.clone()),
            BinaryOp::Subtract if zero(right_constant) => Some(left.clone()),
            BinaryOp::Subtract if same => Some(integer_value(result.type_id, 0)),
            BinaryOp::Multiply if one(right_constant) => Some(left.clone()),
            BinaryOp::Multiply if one(left_constant) => Some(right.clone()),
            BinaryOp::And if all_ones(right_constant) => Some(left.clone()),
            BinaryOp::And if all_ones(left_constant) => Some(right.clone()),
            BinaryOp::And if same => Some(left.clone()),
            BinaryOp::Or if zero(right_constant) => Some(left.clone()),
            BinaryOp::Or if zero(left_constant) => Some(right.clone()),
            BinaryOp::Or if same => Some(left.clone()),
            BinaryOp::Xor if zero(right_constant) => Some(left.clone()),
            BinaryOp::Xor if zero(left_constant) => Some(right.clone()),
            BinaryOp::Xor if same => Some(integer_value(result.type_id, 0)),
            BinaryOp::ShiftLeft | BinaryOp::LogicalShiftRight | BinaryOp::ArithmeticShiftRight
                if zero(right_constant) =>
            {
                Some(left.clone())
            }
            BinaryOp::UnsignedDivide | BinaryOp::SignedDivide if one(right_constant) => {
                Some(left.clone())
            }
            BinaryOp::UnsignedRemainder
            | BinaryOp::SignedRemainder
            | BinaryOp::FloatAdd
            | BinaryOp::FloatSubtract
            | BinaryOp::FloatMultiply
            | BinaryOp::FloatDivide
            | BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::SignedDivide
            | BinaryOp::UnsignedDivide
            | BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::Xor
            | BinaryOp::ShiftLeft
            | BinaryOp::LogicalShiftRight
            | BinaryOp::ArithmeticShiftRight => None,
        }?;

        match replacement {
            Operand::Value(value) if value == result.id => None,
            replacement => Some((result.id, replacement)),
        }
    }
}

impl FunctionPass for AlgebraicSimplify {
    fn name(&self) -> &'static str {
        "algebraic-simplify"
    }

    fn run(&mut self, function: &mut Function) -> Result<PassOutcome, PassFailure> {
        let (value_types, duplicate_values) = value_types(function);
        let mut replacements = BTreeMap::new();
        let mut changed = false;

        loop {
            let normalized_replacements = normalize_replacements(&mut replacements);
            let rewrote_uses = replace_value_uses(function, &replacements);
            let mut simplified_instruction = false;

            for block in &mut function.blocks {
                let instructions = std::mem::take(&mut block.instructions);
                let mut retained = Vec::with_capacity(instructions.len());
                for instruction in instructions {
                    let Some((result, replacement)) =
                        self.simplify_instruction(&instruction, &value_types, &duplicate_values)
                    else {
                        retained.push(instruction);
                        continue;
                    };
                    let Some(replacement) = resolve_replacement(&replacement, &replacements) else {
                        retained.push(instruction);
                        continue;
                    };
                    if matches!(replacement, Operand::Value(value) if value == result) {
                        retained.push(instruction);
                        continue;
                    }
                    replacements.insert(result, replacement);
                    simplified_instruction = true;
                }
                block.instructions = retained;
            }

            changed |= normalized_replacements || rewrote_uses || simplified_instruction;
            if !simplified_instruction {
                break;
            }
        }

        Ok(if changed {
            PassOutcome::changed(PreservedAnalyses::None)
        } else {
            PassOutcome::unchanged()
        })
    }
}

#[derive(Clone, Copy)]
struct IntegerConstant {
    raw: u128,
}

fn integer_mask(bits: u16) -> u128 {
    match bits {
        1..=127 => (1u128 << bits) - 1,
        128 => u128::MAX,
        _ => unreachable!("AlgebraicSimplify validates integer widths at construction"),
    }
}

fn integer_value(type_id: TypeId, value: i128) -> Operand {
    Operand::Constant(TypedConstant {
        type_id,
        value: Constant::Integer(value),
    })
}

fn value_types(function: &Function) -> (BTreeMap<ValueId, TypeId>, BTreeSet<ValueId>) {
    let mut types = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    for value in function.parameters.iter().chain(
        function
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .flat_map(|instruction| instruction.results.iter()),
    ) {
        if types.insert(value.id, value.type_id).is_some() {
            duplicates.insert(value.id);
        }
    }
    (types, duplicates)
}

fn resolve_replacement(
    replacement: &Operand,
    replacements: &BTreeMap<ValueId, Operand>,
) -> Option<Operand> {
    let mut resolved = replacement.clone();
    let mut visited = BTreeSet::new();
    while let Operand::Value(value) = resolved {
        if !visited.insert(value) {
            return None;
        }
        let Some(next) = replacements.get(&value) else {
            return Some(Operand::Value(value));
        };
        resolved = next.clone();
    }
    Some(resolved)
}

fn normalize_replacements(replacements: &mut BTreeMap<ValueId, Operand>) -> bool {
    let updates = replacements
        .iter()
        .filter_map(|(value, replacement)| {
            let resolved = resolve_replacement(replacement, replacements)?;
            (resolved != *replacement).then_some((*value, resolved))
        })
        .collect::<Vec<_>>();
    for (value, replacement) in &updates {
        replacements.insert(*value, replacement.clone());
    }
    !updates.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        Block, BlockId, CallingConvention, FloatKind, FunctionId, InstructionId, Linkage,
        Signature, Terminator, Type, Value,
    };

    const I8: TypeId = TypeId::new(0);
    const F32: TypeId = TypeId::new(1);

    fn integer(value: i128) -> Operand {
        integer_value(I8, value)
    }

    fn value(id: u32, type_id: TypeId) -> Value {
        Value {
            id: ValueId::new(id),
            type_id,
        }
    }

    fn instruction(
        id: u32,
        result: Value,
        op: BinaryOp,
        left: Operand,
        right: Operand,
    ) -> Instruction {
        Instruction {
            id: InstructionId::new(id),
            results: vec![result],
            kind: InstructionKind::Binary { op, left, right },
        }
    }

    fn module(blocks: Vec<Block>, parameters: Vec<Value>) -> Module {
        Module {
            name: "algebraic-test".into(),
            types: vec![
                Type {
                    id: I8,
                    kind: TypeKind::Integer { bits: 8 },
                },
                Type {
                    id: F32,
                    kind: TypeKind::Float(FloatKind::Binary32),
                },
            ],
            globals: Vec::new(),
            functions: vec![Function {
                id: FunctionId::new(0),
                name: "main".into(),
                signature: Signature {
                    result: I8,
                    parameters: parameters
                        .iter()
                        .map(|parameter| parameter.type_id)
                        .collect(),
                    variadic: false,
                    calling_convention: CallingConvention::C,
                },
                linkage: Linkage::Internal,
                attributes: Vec::new(),
                parameters,
                blocks,
            }],
        }
    }

    #[test]
    fn reaches_a_terminator_through_chained_identities() {
        let parameter = value(9, I8);
        let first = value(0, I8);
        let second = value(1, I8);
        let mut module = module(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    instruction(
                        0,
                        first.clone(),
                        BinaryOp::Add,
                        Operand::Value(parameter.id),
                        integer(0),
                    ),
                    instruction(
                        1,
                        second.clone(),
                        BinaryOp::Xor,
                        Operand::Value(first.id),
                        integer(0),
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Value(second.id))),
            }],
            vec![parameter.clone()],
        );

        let outcome = AlgebraicSimplify::new(&module)
            .unwrap()
            .run(&mut module.functions[0])
            .unwrap();

        assert!(outcome.changed_ir());
        assert!(module.functions[0].blocks[0].instructions.is_empty());
        assert_eq!(
            module.functions[0].blocks[0].terminator,
            Terminator::Return(Some(Operand::Value(parameter.id)))
        );
    }

    #[test]
    fn normalizes_narrow_all_ones_before_simplifying_and() {
        let parameter = value(9, I8);
        let result = value(0, I8);
        let mut module = module(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![instruction(
                    0,
                    result.clone(),
                    BinaryOp::And,
                    integer(-1),
                    Operand::Value(parameter.id),
                )],
                terminator: Terminator::Return(Some(Operand::Value(result.id))),
            }],
            vec![parameter.clone()],
        );

        AlgebraicSimplify::new(&module)
            .unwrap()
            .run(&mut module.functions[0])
            .unwrap();

        assert!(module.functions[0].blocks[0].instructions.is_empty());
        assert_eq!(
            module.functions[0].blocks[0].terminator,
            Terminator::Return(Some(Operand::Value(parameter.id)))
        );
    }

    #[test]
    fn preserves_trap_or_float_sensitive_non_identities() {
        let parameter = value(9, I8);
        let product = value(0, I8);
        let quotient = value(1, I8);
        let float_result = value(2, F32);
        let float_zero = Operand::Constant(TypedConstant {
            type_id: F32,
            value: Constant::Float("0".into()),
        });
        let mut module = module(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    instruction(
                        0,
                        product,
                        BinaryOp::Multiply,
                        Operand::Value(parameter.id),
                        integer(0),
                    ),
                    instruction(
                        1,
                        quotient,
                        BinaryOp::SignedDivide,
                        Operand::Value(parameter.id),
                        integer(-1),
                    ),
                    instruction(
                        2,
                        float_result,
                        BinaryOp::FloatAdd,
                        float_zero.clone(),
                        float_zero,
                    ),
                ],
                terminator: Terminator::Return(Some(integer(0))),
            }],
            vec![parameter],
        );

        let outcome = AlgebraicSimplify::new(&module)
            .unwrap()
            .run(&mut module.functions[0])
            .unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(module.functions[0].blocks[0].instructions.len(), 3);
    }
}
