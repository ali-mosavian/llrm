//! Exact constant folding for the portable integer IR.
//!
//! The pass deliberately folds only operations whose integer semantics are
//! fully specified by the current IR interpreter.  It never invents a value
//! for a trapping operation or for an operation outside that subset.

use std::collections::BTreeMap;
use std::fmt;

use crate::ir::{
    BinaryOp, Callee, CastOp, ComparePredicate, Constant, Function, Instruction, InstructionKind,
    Module, Operand, Terminator, TypeId, TypeKind, TypedConstant, UnaryOp, ValueId,
};

use super::{FunctionPass, PassFailure, PassOutcome, PreservedAnalyses};

/// A construction error for [`ConstantFold`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FoldError {
    /// The module declares one type identity more than once.
    DuplicateType(TypeId),
    /// An integer type cannot be represented by the portable integer oracle.
    InvalidIntegerWidth { type_id: TypeId, bits: u16 },
}

impl fmt::Display for FoldError {
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

impl std::error::Error for FoldError {}

/// Folds exact integer expressions and propagates their typed constants.
///
/// The pass owns the integer declarations it needs rather than retaining a
/// borrow of the module.  That makes it a normal [`FunctionPass`] while
/// keeping all type decisions explicit and deterministic.
#[derive(Clone, Debug)]
pub struct ConstantFold {
    integer_widths: BTreeMap<TypeId, u16>,
}

impl ConstantFold {
    /// Builds a folder from the module's declared integer types.
    pub fn new(module: &Module) -> Result<Self, FoldError> {
        let mut integer_widths = BTreeMap::new();
        let mut declared = BTreeMap::new();

        for type_ in &module.types {
            if declared.insert(type_.id, ()).is_some() {
                return Err(FoldError::DuplicateType(type_.id));
            }
            if let TypeKind::Integer { bits } = &type_.kind {
                if !(1..=128).contains(bits) {
                    return Err(FoldError::InvalidIntegerWidth {
                        type_id: type_.id,
                        bits: *bits,
                    });
                }
                integer_widths.insert(type_.id, *bits);
            }
        }

        Ok(Self { integer_widths })
    }

    fn integer_width(&self, type_id: TypeId) -> Option<u16> {
        self.integer_widths.get(&type_id).copied()
    }

    fn fold_instruction(&self, instruction: &Instruction) -> Option<(ValueId, TypedConstant)> {
        let [result] = instruction.results.as_slice() else {
            return None;
        };
        let value = match &instruction.kind {
            InstructionKind::Unary { op, operand } => self.fold_unary(*op, operand, result.type_id),
            InstructionKind::Binary { op, left, right } => {
                self.fold_binary(*op, left, right, result.type_id)
            }
            InstructionKind::Compare {
                predicate,
                left,
                right,
            } => self.fold_compare(*predicate, left, right, result.type_id),
            InstructionKind::Cast { op, operand, to } => {
                if *to == result.type_id {
                    self.fold_cast(*op, operand, *to)
                } else {
                    None
                }
            }
            InstructionKind::Select {
                condition,
                then_value,
                else_value,
            } => self.fold_select(condition, then_value, else_value, result.type_id),
            InstructionKind::Phi { .. }
            | InstructionKind::Load { .. }
            | InstructionKind::Store { .. }
            | InstructionKind::GetElementPointer { .. }
            | InstructionKind::Call { .. }
            | InstructionKind::Intrinsic { .. } => None,
        }?;

        Some((result.id, value))
    }

    fn fold_unary(
        &self,
        op: UnaryOp,
        operand: &Operand,
        result_type: TypeId,
    ) -> Option<TypedConstant> {
        let operand = self.integer_constant(operand)?;
        if operand.type_id != result_type {
            return None;
        }
        let raw = match op {
            UnaryOp::Negate => 0u128.wrapping_sub(operand.raw),
            UnaryOp::Not => !operand.raw,
            UnaryOp::FloatNegate | UnaryOp::FloatAbsolute => return None,
        };
        Some(integer_constant_value(result_type, operand.bits, raw))
    }

    fn fold_binary(
        &self,
        op: BinaryOp,
        left: &Operand,
        right: &Operand,
        result_type: TypeId,
    ) -> Option<TypedConstant> {
        let left = self.integer_constant(left)?;
        let right = self.integer_constant(right)?;
        if left.type_id != result_type || right.type_id != result_type || left.bits != right.bits {
            return None;
        }

        let raw = match op {
            BinaryOp::Add => left.raw.wrapping_add(right.raw),
            BinaryOp::Subtract => left.raw.wrapping_sub(right.raw),
            BinaryOp::Multiply => left.raw.wrapping_mul(right.raw),
            BinaryOp::And => left.raw & right.raw,
            BinaryOp::Or => left.raw | right.raw,
            BinaryOp::Xor => left.raw ^ right.raw,
            BinaryOp::ShiftLeft | BinaryOp::LogicalShiftRight | BinaryOp::ArithmeticShiftRight => {
                if right.raw >= u128::from(left.bits) {
                    return None;
                }
                let count = right.raw as u32;
                match op {
                    BinaryOp::ShiftLeft => left.raw << count,
                    BinaryOp::LogicalShiftRight => left.raw >> count,
                    BinaryOp::ArithmeticShiftRight => (left.as_signed() >> count) as u128,
                    _ => return None,
                }
            }
            BinaryOp::UnsignedDivide | BinaryOp::UnsignedRemainder => {
                if right.raw == 0 {
                    return None;
                }
                if matches!(op, BinaryOp::UnsignedDivide) {
                    left.raw / right.raw
                } else {
                    left.raw % right.raw
                }
            }
            BinaryOp::SignedDivide | BinaryOp::SignedRemainder => {
                let dividend = left.as_signed();
                let divisor = right.as_signed();
                if divisor == 0 || (dividend == signed_minimum(left.bits) && divisor == -1) {
                    return None;
                }
                if matches!(op, BinaryOp::SignedDivide) {
                    (dividend / divisor) as u128
                } else {
                    (dividend % divisor) as u128
                }
            }
            BinaryOp::FloatAdd
            | BinaryOp::FloatSubtract
            | BinaryOp::FloatMultiply
            | BinaryOp::FloatDivide => return None,
        };
        Some(integer_constant_value(result_type, left.bits, raw))
    }

    fn fold_compare(
        &self,
        predicate: ComparePredicate,
        left: &Operand,
        right: &Operand,
        result_type: TypeId,
    ) -> Option<TypedConstant> {
        let left = self.integer_constant(left)?;
        let right = self.integer_constant(right)?;
        if left.type_id != right.type_id || self.integer_width(result_type) != Some(1) {
            return None;
        }
        let value = match predicate {
            ComparePredicate::Equal => left.raw == right.raw,
            ComparePredicate::NotEqual => left.raw != right.raw,
            ComparePredicate::SignedLessThan => left.as_signed() < right.as_signed(),
            ComparePredicate::SignedLessEqual => left.as_signed() <= right.as_signed(),
            ComparePredicate::SignedGreaterThan => left.as_signed() > right.as_signed(),
            ComparePredicate::SignedGreaterEqual => left.as_signed() >= right.as_signed(),
            ComparePredicate::UnsignedLessThan => left.raw < right.raw,
            ComparePredicate::UnsignedLessEqual => left.raw <= right.raw,
            ComparePredicate::UnsignedGreaterThan => left.raw > right.raw,
            ComparePredicate::UnsignedGreaterEqual => left.raw >= right.raw,
            ComparePredicate::OrderedEqual
            | ComparePredicate::OrderedNotEqual
            | ComparePredicate::OrderedLessThan
            | ComparePredicate::OrderedLessEqual
            | ComparePredicate::OrderedGreaterThan
            | ComparePredicate::OrderedGreaterEqual => return None,
        };
        Some(integer_constant_value(result_type, 1, u128::from(value)))
    }

    fn fold_cast(
        &self,
        op: CastOp,
        operand: &Operand,
        result_type: TypeId,
    ) -> Option<TypedConstant> {
        let operand = self.integer_constant(operand)?;
        let result_bits = self.integer_width(result_type)?;
        let raw = match op {
            CastOp::Truncate if result_bits < operand.bits => operand.raw,
            CastOp::SignExtend if result_bits > operand.bits => operand.as_signed() as u128,
            CastOp::ZeroExtend if result_bits > operand.bits => operand.raw,
            CastOp::Bitcast if result_bits == operand.bits => operand.raw,
            CastOp::Truncate | CastOp::SignExtend | CastOp::ZeroExtend | CastOp::Bitcast => {
                return None;
            }
            CastOp::IntegerToFloat
            | CastOp::FloatToInteger
            | CastOp::FloatExtend
            | CastOp::FloatTruncate
            | CastOp::PointerToInteger
            | CastOp::IntegerToPointer => return None,
        };
        Some(integer_constant_value(result_type, result_bits, raw))
    }

    fn fold_select(
        &self,
        condition: &Operand,
        then_value: &Operand,
        else_value: &Operand,
        result_type: TypeId,
    ) -> Option<TypedConstant> {
        let condition = self.integer_constant(condition)?;
        if condition.bits != 1 {
            return None;
        }
        let then_value = self.integer_constant(then_value)?;
        let else_value = self.integer_constant(else_value)?;
        if then_value.type_id != result_type || else_value.type_id != result_type {
            return None;
        }
        Some(if condition.raw == 0 {
            integer_constant_value(result_type, else_value.bits, else_value.raw)
        } else {
            integer_constant_value(result_type, then_value.bits, then_value.raw)
        })
    }

    fn integer_constant(&self, operand: &Operand) -> Option<IntegerConstant> {
        let Operand::Constant(constant) = operand else {
            return None;
        };
        let Constant::Integer(value) = &constant.value else {
            return None;
        };
        let bits = self.integer_width(constant.type_id)?;
        Some(IntegerConstant {
            type_id: constant.type_id,
            bits,
            raw: (*value as u128) & integer_mask(bits),
        })
    }
}

impl FunctionPass for ConstantFold {
    fn name(&self) -> &'static str {
        "constant-fold"
    }

    fn run(&mut self, function: &mut Function) -> Result<PassOutcome, PassFailure> {
        let mut replacements = BTreeMap::new();
        let mut changed = false;

        loop {
            let rewrote_uses = rewrite_function_operands(function, &replacements);
            let mut folded_instruction = false;

            for block in &mut function.blocks {
                let instructions = std::mem::take(&mut block.instructions);
                let mut retained = Vec::with_capacity(instructions.len());
                for instruction in instructions {
                    if let Some((value, constant)) = self.fold_instruction(&instruction) {
                        replacements.insert(value, constant);
                        folded_instruction = true;
                    } else {
                        retained.push(instruction);
                    }
                }
                block.instructions = retained;
            }

            changed |= rewrote_uses || folded_instruction;
            if !folded_instruction {
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
    type_id: TypeId,
    bits: u16,
    raw: u128,
}

fn integer_mask(bits: u16) -> u128 {
    match bits {
        1..=127 => (1u128 << bits) - 1,
        128 => u128::MAX,
        _ => unreachable!("ConstantFold validates integer widths at construction"),
    }
}

fn signed_minimum(bits: u16) -> i128 {
    match bits {
        1..=127 => -(1i128 << (bits - 1)),
        128 => i128::MIN,
        _ => unreachable!("ConstantFold validates integer widths at construction"),
    }
}

fn signed_value(bits: u16, raw: u128) -> i128 {
    let raw = raw & integer_mask(bits);
    if bits == 128 {
        raw as i128
    } else if raw & (1u128 << (bits - 1)) == 0 {
        raw as i128
    } else {
        (raw | !integer_mask(bits)) as i128
    }
}

impl IntegerConstant {
    fn as_signed(self) -> i128 {
        signed_value(self.bits, self.raw)
    }
}

fn integer_constant_value(type_id: TypeId, bits: u16, raw: u128) -> TypedConstant {
    TypedConstant {
        type_id,
        value: Constant::Integer(signed_value(bits, raw)),
    }
}

fn rewrite_function_operands(
    function: &mut Function,
    replacements: &BTreeMap<ValueId, TypedConstant>,
) -> bool {
    let mut changed = false;
    for block in &mut function.blocks {
        for instruction in &mut block.instructions {
            changed |= rewrite_instruction_operands(instruction, replacements);
        }
        changed |= rewrite_terminator_operands(&mut block.terminator, replacements);
    }
    changed
}

fn rewrite_instruction_operands(
    instruction: &mut Instruction,
    replacements: &BTreeMap<ValueId, TypedConstant>,
) -> bool {
    let mut changed = false;
    match &mut instruction.kind {
        InstructionKind::Phi { incoming } => {
            for incoming in incoming {
                changed |= rewrite_operand(&mut incoming.value, replacements);
            }
        }
        InstructionKind::Unary { operand, .. } | InstructionKind::Cast { operand, .. } => {
            changed |= rewrite_operand(operand, replacements);
        }
        InstructionKind::Binary { left, right, .. }
        | InstructionKind::Compare { left, right, .. } => {
            changed |= rewrite_operand(left, replacements);
            changed |= rewrite_operand(right, replacements);
        }
        InstructionKind::Load { address, .. } => {
            changed |= rewrite_operand(address, replacements);
        }
        InstructionKind::Store { address, value, .. } => {
            changed |= rewrite_operand(address, replacements);
            changed |= rewrite_operand(value, replacements);
        }
        InstructionKind::GetElementPointer { base, indices } => {
            changed |= rewrite_operand(base, replacements);
            for index in indices {
                changed |= rewrite_operand(index, replacements);
            }
        }
        InstructionKind::Select {
            condition,
            then_value,
            else_value,
        } => {
            changed |= rewrite_operand(condition, replacements);
            changed |= rewrite_operand(then_value, replacements);
            changed |= rewrite_operand(else_value, replacements);
        }
        InstructionKind::Call {
            callee, arguments, ..
        } => {
            if let Callee::Indirect(operand) = callee {
                changed |= rewrite_operand(operand, replacements);
            }
            for argument in arguments {
                changed |= rewrite_operand(argument, replacements);
            }
        }
        InstructionKind::Intrinsic { arguments, .. } => {
            for argument in arguments {
                changed |= rewrite_operand(argument, replacements);
            }
        }
    }
    changed
}

fn rewrite_terminator_operands(
    terminator: &mut Terminator,
    replacements: &BTreeMap<ValueId, TypedConstant>,
) -> bool {
    match terminator {
        Terminator::Jump(_) | Terminator::Unreachable => false,
        Terminator::Branch { condition, .. } => rewrite_operand(condition, replacements),
        Terminator::Switch { selector, .. } => rewrite_operand(selector, replacements),
        Terminator::Return(value) => value
            .as_mut()
            .is_some_and(|operand| rewrite_operand(operand, replacements)),
    }
}

fn rewrite_operand(operand: &mut Operand, replacements: &BTreeMap<ValueId, TypedConstant>) -> bool {
    let Operand::Value(value) = operand else {
        return false;
    };
    let Some(constant) = replacements.get(value) else {
        return false;
    };
    *operand = Operand::Constant(constant.clone());
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        Block, BlockId, CallingConvention, Function, FunctionId, Linkage, Signature, Type, Value,
    };

    const VOID: TypeId = TypeId::new(0);
    const I1: TypeId = TypeId::new(1);
    const I8: TypeId = TypeId::new(2);

    fn integer(type_id: TypeId, value: i128) -> Operand {
        Operand::Constant(TypedConstant {
            type_id,
            value: Constant::Integer(value),
        })
    }

    fn module(blocks: Vec<Block>) -> Module {
        Module {
            name: "fold-test".into(),
            types: vec![
                Type {
                    id: VOID,
                    kind: TypeKind::Void,
                },
                Type {
                    id: I1,
                    kind: TypeKind::Integer { bits: 1 },
                },
                Type {
                    id: I8,
                    kind: TypeKind::Integer { bits: 8 },
                },
            ],
            globals: Vec::new(),
            functions: vec![Function {
                id: FunctionId::new(0),
                name: "main".into(),
                signature: Signature {
                    result: I8,
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: CallingConvention::C,
                },
                linkage: Linkage::Internal,
                attributes: Vec::new(),
                parameters: Vec::new(),
                blocks,
            }],
        }
    }

    fn value(id: u32, type_id: TypeId) -> Value {
        Value {
            id: ValueId::new(id),
            type_id,
        }
    }

    #[test]
    fn folds_a_constant_chain_to_the_return() {
        let first = value(0, I8);
        let second = value(1, I8);
        let mut module = module(vec![Block {
            id: BlockId::new(0),
            instructions: vec![
                Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![first.clone()],
                    kind: InstructionKind::Binary {
                        op: BinaryOp::Add,
                        left: integer(I8, 120),
                        right: integer(I8, 8),
                    },
                },
                Instruction {
                    id: crate::ir::InstructionId::new(1),
                    results: vec![second.clone()],
                    kind: InstructionKind::Unary {
                        op: UnaryOp::Negate,
                        operand: Operand::Value(first.id),
                    },
                },
            ],
            terminator: Terminator::Return(Some(Operand::Value(second.id))),
        }]);

        let mut pass = ConstantFold::new(&module).unwrap();
        let outcome = pass.run(&mut module.functions[0]).unwrap();

        assert!(outcome.changed_ir());
        assert_eq!(module.functions[0].blocks[0].instructions, Vec::new());
        assert_eq!(
            module.functions[0].blocks[0].terminator,
            Terminator::Return(Some(integer(I8, -128)))
        );
    }

    #[test]
    fn wraps_fixed_width_integer_results() {
        let result = value(0, I8);
        let mut module = module(vec![Block {
            id: BlockId::new(0),
            instructions: vec![Instruction {
                id: crate::ir::InstructionId::new(0),
                results: vec![result.clone()],
                kind: InstructionKind::Binary {
                    op: BinaryOp::Add,
                    left: integer(I8, 127),
                    right: integer(I8, 1),
                },
            }],
            terminator: Terminator::Return(Some(Operand::Value(result.id))),
        }]);

        ConstantFold::new(&module)
            .unwrap()
            .run(&mut module.functions[0])
            .unwrap();

        assert_eq!(
            module.functions[0].blocks[0].terminator,
            Terminator::Return(Some(integer(I8, -128)))
        );
    }

    #[test]
    fn rewrites_compare_branch_and_select_uses() {
        let comparison = value(0, I1);
        let selected = value(1, I8);
        let mut module = module(vec![
            Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![comparison.clone()],
                        kind: InstructionKind::Compare {
                            predicate: ComparePredicate::UnsignedGreaterThan,
                            left: integer(I8, 9),
                            right: integer(I8, 4),
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(1),
                        results: vec![selected.clone()],
                        kind: InstructionKind::Select {
                            condition: Operand::Value(comparison.id),
                            then_value: integer(I8, 7),
                            else_value: integer(I8, 3),
                        },
                    },
                ],
                terminator: Terminator::Branch {
                    condition: Operand::Value(comparison.id),
                    then_block: BlockId::new(1),
                    else_block: BlockId::new(2),
                },
            },
            Block {
                id: BlockId::new(1),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(Operand::Value(selected.id))),
            },
            Block {
                id: BlockId::new(2),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(integer(I8, 0))),
            },
        ]);

        ConstantFold::new(&module)
            .unwrap()
            .run(&mut module.functions[0])
            .unwrap();

        assert_eq!(module.functions[0].blocks[0].instructions, Vec::new());
        assert_eq!(
            module.functions[0].blocks[0].terminator,
            Terminator::Branch {
                condition: integer(I1, 1),
                then_block: BlockId::new(1),
                else_block: BlockId::new(2),
            }
        );
        assert_eq!(
            module.functions[0].blocks[1].terminator,
            Terminator::Return(Some(integer(I8, 7)))
        );
    }

    #[test]
    fn preserves_trapping_signed_division() {
        let quotient = value(0, I8);
        let mut module = module(vec![Block {
            id: BlockId::new(0),
            instructions: vec![Instruction {
                id: crate::ir::InstructionId::new(0),
                results: vec![quotient.clone()],
                kind: InstructionKind::Binary {
                    op: BinaryOp::SignedDivide,
                    left: integer(I8, -128),
                    right: integer(I8, -1),
                },
            }],
            terminator: Terminator::Return(Some(Operand::Value(quotient.id))),
        }]);

        let outcome = ConstantFold::new(&module)
            .unwrap()
            .run(&mut module.functions[0])
            .unwrap();

        assert!(!outcome.changed_ir());
        assert_eq!(module.functions[0].blocks[0].instructions.len(), 1);
        assert_eq!(
            module.functions[0].blocks[0].terminator,
            Terminator::Return(Some(Operand::Value(quotient.id)))
        );
    }
}
