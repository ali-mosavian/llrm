//! Type consistency checks layered on the structural IR verifier.
//!
//! Structural verification reports unknown or duplicate identities.  This
//! module deliberately skips checks that depend on those identities, so one
//! malformed declaration does not produce a cascade of speculative errors.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::diagnostic::{Diagnostic, Severity};

use super::{
    BinaryOp, Callee, CastOp, ComparePredicate, Constant, FloatKind, Function, FunctionId,
    Instruction, InstructionKind, Intrinsic, Module, Operand, Terminator, TypeId, TypeKind,
    TypedConstant, UnaryOp, ValueId,
};

/// Verifies portable-IR type consistency after structural verification.
///
/// The caller owns ordering this after the structural verifier.  This function
/// consequently does not repeat identity, CFG, or dominance checks.
pub(super) fn verify_types(module: &Module) -> Vec<Diagnostic> {
    let types = TypeTable::new(module);
    let functions = FunctionTable::new(module);
    let mut verifier = TypeVerifier {
        types,
        functions,
        diagnostics: Vec::new(),
    };
    verifier.verify(module);
    verifier.diagnostics
}

struct TypeVerifier<'module> {
    types: TypeTable<'module>,
    functions: FunctionTable<'module>,
    diagnostics: Vec<Diagnostic>,
}

impl<'module> TypeVerifier<'module> {
    fn verify(&mut self, module: &Module) {
        for global in &module.globals {
            self.require_non_void(global.type_id, format!("global {}", global.id));
            if let Some(initializer) = &global.initializer {
                self.verify_constant(
                    initializer,
                    global.type_id,
                    format!("global {} initializer", global.id),
                );
            }
        }

        for function in &module.functions {
            self.verify_function(function);
        }
    }

    fn verify_function(&mut self, function: &Function) {
        let values = ValueTypes::new(function);
        for (index, type_id) in function.signature.parameters.iter().enumerate() {
            self.require_non_void(
                *type_id,
                format!("function {} signature parameter {}", function.id, index),
            );
        }
        for (index, parameter) in function.parameters.iter().enumerate() {
            self.require_non_void(
                parameter.type_id,
                format!("function {} parameter {}", function.id, index),
            );
        }
        for block in &function.blocks {
            for instruction in &block.instructions {
                for result in &instruction.results {
                    self.require_non_void(
                        result.type_id,
                        format!(
                            "function {} instruction {} result {}",
                            function.id, instruction.id, result.id
                        ),
                    );
                }
                self.verify_instruction_constants(instruction);
                self.verify_instruction(function, instruction, &values);
            }
            self.verify_terminator_constants(&block.terminator);
            self.verify_terminator(function, &block.terminator, &values);
        }
    }

    fn verify_instruction_constants(&mut self, instruction: &Instruction) {
        match &instruction.kind {
            InstructionKind::Phi { incoming } => {
                for incoming in incoming {
                    self.verify_operand_constant(&incoming.value);
                }
            }
            InstructionKind::Unary { operand, .. } | InstructionKind::Cast { operand, .. } => {
                self.verify_operand_constant(operand);
            }
            InstructionKind::Binary { left, right, .. }
            | InstructionKind::Compare { left, right, .. } => {
                self.verify_operand_constant(left);
                self.verify_operand_constant(right);
            }
            InstructionKind::Load { address, .. } => self.verify_operand_constant(address),
            InstructionKind::Store { address, value, .. } => {
                self.verify_operand_constant(address);
                self.verify_operand_constant(value);
            }
            InstructionKind::GetElementPointer { base, indices } => {
                self.verify_operand_constant(base);
                for index in indices {
                    self.verify_operand_constant(index);
                }
            }
            InstructionKind::Select {
                condition,
                then_value,
                else_value,
            } => {
                self.verify_operand_constant(condition);
                self.verify_operand_constant(then_value);
                self.verify_operand_constant(else_value);
            }
            InstructionKind::Call {
                callee, arguments, ..
            } => {
                if let Callee::Indirect(callee) = callee {
                    self.verify_operand_constant(callee);
                }
                for argument in arguments {
                    self.verify_operand_constant(argument);
                }
            }
            InstructionKind::Intrinsic { arguments, .. } => {
                for argument in arguments {
                    self.verify_operand_constant(argument);
                }
            }
        }
    }

    fn verify_terminator_constants(&mut self, terminator: &Terminator) {
        match terminator {
            Terminator::Jump(_) | Terminator::Unreachable => {}
            Terminator::Branch { condition, .. } => self.verify_operand_constant(condition),
            Terminator::Switch { selector, .. } => self.verify_operand_constant(selector),
            Terminator::Return(value) => {
                if let Some(value) = value {
                    self.verify_operand_constant(value);
                }
            }
        }
    }

    fn verify_operand_constant(&mut self, operand: &Operand) {
        if let Operand::Constant(constant) = operand {
            self.verify_typed_constant(constant, None, "constant".into());
        }
    }

    fn verify_instruction(
        &mut self,
        function: &Function,
        instruction: &Instruction,
        values: &ValueTypes,
    ) {
        match &instruction.kind {
            InstructionKind::Phi { incoming } => {
                let Some(result) = self.single_result_type(instruction) else {
                    return;
                };
                for (index, incoming) in incoming.iter().enumerate() {
                    self.require_operand_type(
                        &incoming.value,
                        result,
                        values,
                        format!(
                            "function {} instruction {} phi incoming {}",
                            function.id, instruction.id, index
                        ),
                    );
                }
            }
            InstructionKind::Unary { op, operand } => {
                let Some(result) = self.single_result_type(instruction) else {
                    return;
                };
                self.require_operand_type(
                    operand,
                    result,
                    values,
                    format!(
                        "function {} instruction {} unary operand",
                        function.id, instruction.id
                    ),
                );
                match op {
                    UnaryOp::Negate | UnaryOp::Not => self.require_integer(
                        result,
                        format!(
                            "function {} instruction {} unary result",
                            function.id, instruction.id
                        ),
                    ),
                    UnaryOp::FloatNegate | UnaryOp::FloatAbsolute => self.require_float(
                        result,
                        format!(
                            "function {} instruction {} unary result",
                            function.id, instruction.id
                        ),
                    ),
                }
            }
            InstructionKind::Binary { op, left, right } => {
                let Some(result) = self.single_result_type(instruction) else {
                    return;
                };
                let context = format!(
                    "function {} instruction {} binary",
                    function.id, instruction.id
                );
                self.require_operand_type(left, result, values, format!("{context} left operand"));
                self.require_operand_type(
                    right,
                    result,
                    values,
                    format!("{context} right operand"),
                );
                if matches!(
                    op,
                    BinaryOp::FloatAdd
                        | BinaryOp::FloatSubtract
                        | BinaryOp::FloatMultiply
                        | BinaryOp::FloatDivide
                ) {
                    self.require_float(result, format!("{context} result"));
                } else {
                    self.require_integer(result, format!("{context} result"));
                }
            }
            InstructionKind::Compare {
                predicate,
                left,
                right,
            } => {
                let Some(result) = self.single_result_type(instruction) else {
                    return;
                };
                self.require_i1(
                    result,
                    format!(
                        "function {} instruction {} compare result",
                        function.id, instruction.id
                    ),
                );
                self.require_same_operand_types(
                    left,
                    right,
                    values,
                    format!(
                        "function {} instruction {} compare operands",
                        function.id, instruction.id
                    ),
                );
                let Some(operand_type) = self.operand_type(left, values) else {
                    return;
                };
                let context = format!(
                    "function {} instruction {} compare operands",
                    function.id, instruction.id
                );
                match predicate {
                    ComparePredicate::Equal | ComparePredicate::NotEqual => {
                        self.require_integer_or_pointer(operand_type, context)
                    }
                    ComparePredicate::SignedLessThan
                    | ComparePredicate::SignedLessEqual
                    | ComparePredicate::SignedGreaterThan
                    | ComparePredicate::SignedGreaterEqual
                    | ComparePredicate::UnsignedLessThan
                    | ComparePredicate::UnsignedLessEqual
                    | ComparePredicate::UnsignedGreaterThan
                    | ComparePredicate::UnsignedGreaterEqual => {
                        self.require_integer(operand_type, context)
                    }
                    ComparePredicate::OrderedEqual
                    | ComparePredicate::OrderedNotEqual
                    | ComparePredicate::OrderedLessThan
                    | ComparePredicate::OrderedLessEqual
                    | ComparePredicate::OrderedGreaterThan
                    | ComparePredicate::OrderedGreaterEqual => {
                        self.require_float(operand_type, context)
                    }
                }
            }
            InstructionKind::Cast { op, operand, to } => {
                let Some(result) = self.single_result_type(instruction) else {
                    return;
                };
                if self.known_type(result) && self.known_type(*to) && result != *to {
                    self.error(format!(
                        "function {} instruction {} cast result has type {}, expected target type {}",
                        function.id, instruction.id, result, to
                    ));
                }
                self.verify_cast(
                    *op,
                    operand,
                    *to,
                    values,
                    format!(
                        "function {} instruction {} cast",
                        function.id, instruction.id
                    ),
                );
            }
            InstructionKind::Load { address, .. } => {
                let Some(result) = self.single_result_type(instruction) else {
                    return;
                };
                self.require_operand_pointer(
                    address,
                    values,
                    format!(
                        "function {} instruction {} load address",
                        function.id, instruction.id
                    ),
                );
                self.require_non_void(
                    result,
                    format!(
                        "function {} instruction {} load result",
                        function.id, instruction.id
                    ),
                );
            }
            InstructionKind::Store { address, value, .. } => {
                self.require_operand_pointer(
                    address,
                    values,
                    format!(
                        "function {} instruction {} store address",
                        function.id, instruction.id
                    ),
                );
                self.require_operand_non_void(
                    value,
                    values,
                    format!(
                        "function {} instruction {} store value",
                        function.id, instruction.id
                    ),
                );
            }
            InstructionKind::GetElementPointer { base, indices } => {
                let Some(result) = self.single_result_type(instruction) else {
                    return;
                };
                self.require_operand_pointer(
                    base,
                    values,
                    format!(
                        "function {} instruction {} gep base",
                        function.id, instruction.id
                    ),
                );
                self.require_pointer(
                    result,
                    format!(
                        "function {} instruction {} gep result",
                        function.id, instruction.id
                    ),
                );
                for (index, operand) in indices.iter().enumerate() {
                    self.require_operand_integer(
                        operand,
                        values,
                        format!(
                            "function {} instruction {} gep index {}",
                            function.id, instruction.id, index
                        ),
                    );
                }
            }
            InstructionKind::Select {
                condition,
                then_value,
                else_value,
            } => {
                let Some(result) = self.single_result_type(instruction) else {
                    return;
                };
                self.require_operand_i1(
                    condition,
                    values,
                    format!(
                        "function {} instruction {} select condition",
                        function.id, instruction.id
                    ),
                );
                self.require_operand_type(
                    then_value,
                    result,
                    values,
                    format!(
                        "function {} instruction {} select then value",
                        function.id, instruction.id
                    ),
                );
                self.require_operand_type(
                    else_value,
                    result,
                    values,
                    format!(
                        "function {} instruction {} select else value",
                        function.id, instruction.id
                    ),
                );
            }
            InstructionKind::Call {
                callee, arguments, ..
            } => self.verify_call(function, instruction, callee, arguments, values),
            InstructionKind::Intrinsic {
                intrinsic,
                arguments,
            } => self.verify_intrinsic(function, instruction, *intrinsic, arguments, values),
        }
    }

    fn verify_cast(
        &mut self,
        op: CastOp,
        operand: &Operand,
        target: TypeId,
        values: &ValueTypes,
        context: String,
    ) {
        let Some(source) = self.operand_type(operand, values) else {
            return;
        };
        if !self.known_type(target) {
            return;
        }

        match op {
            CastOp::Truncate => {
                self.require_integer_pair(source, target, &context, |source, target| {
                    target < source
                })
            }
            CastOp::SignExtend | CastOp::ZeroExtend => {
                self.require_integer_pair(source, target, &context, |source, target| {
                    target > source
                })
            }
            CastOp::IntegerToFloat => {
                self.require_kinds(source, target, &context, is_integer, is_float)
            }
            CastOp::FloatToInteger => {
                self.require_kinds(source, target, &context, is_float, is_integer)
            }
            CastOp::FloatExtend => {
                self.require_float_pair(source, target, &context, |source, target| target > source)
            }
            CastOp::FloatTruncate => {
                self.require_float_pair(source, target, &context, |source, target| target < source)
            }
            CastOp::PointerToInteger => {
                self.require_kinds(source, target, &context, is_pointer, is_integer)
            }
            CastOp::IntegerToPointer => {
                self.require_kinds(source, target, &context, is_integer, is_pointer)
            }
            CastOp::Bitcast => self.require_non_void_pair(source, target, &context),
        }
    }

    fn verify_call(
        &mut self,
        function: &Function,
        instruction: &Instruction,
        callee: &Callee,
        arguments: &[Operand],
        values: &ValueTypes,
    ) {
        match callee {
            Callee::Indirect(callee) => self.require_operand_pointer(
                callee,
                values,
                format!(
                    "function {} instruction {} indirect callee",
                    function.id, instruction.id
                ),
            ),
            Callee::Direct(id) => {
                let Some(signature) = self
                    .functions
                    .get(*id)
                    .map(|callee| callee.signature.clone())
                else {
                    return;
                };
                let fixed_count = signature.parameters.len();
                if (!signature.variadic && arguments.len() != fixed_count)
                    || (signature.variadic && arguments.len() < fixed_count)
                {
                    self.error(format!(
                        "function {} instruction {} call has {} arguments, expected {}{}",
                        function.id,
                        instruction.id,
                        arguments.len(),
                        fixed_count,
                        if signature.variadic { "+" } else { "" }
                    ));
                }
                for (index, (argument, expected)) in
                    arguments.iter().zip(&signature.parameters).enumerate()
                {
                    self.require_operand_type(
                        argument,
                        *expected,
                        values,
                        format!(
                            "function {} instruction {} call argument {}",
                            function.id, instruction.id, index
                        ),
                    );
                }
                self.verify_call_result(function, instruction, signature.result);
            }
        }
    }

    fn verify_call_result(
        &mut self,
        function: &Function,
        instruction: &Instruction,
        result: TypeId,
    ) {
        let Some(is_void) = self
            .types
            .get(result)
            .map(|kind| matches!(kind, TypeKind::Void))
        else {
            return;
        };
        if is_void {
            if !instruction.results.is_empty() {
                self.error(format!(
                    "function {} instruction {} void call has a result",
                    function.id, instruction.id
                ));
            }
        } else if let Some(actual) = self.single_result_type(instruction) {
            if self.known_type(actual) && actual != result {
                self.error(format!(
                    "function {} instruction {} call result has type {}, expected {}",
                    function.id, instruction.id, actual, result
                ));
            }
        }
    }

    fn verify_intrinsic(
        &mut self,
        function: &Function,
        instruction: &Instruction,
        intrinsic: Intrinsic,
        arguments: &[Operand],
        values: &ValueTypes,
    ) {
        match intrinsic {
            Intrinsic::Trap => {
                if !arguments.is_empty() || !instruction.results.is_empty() {
                    self.error(format!(
                        "function {} instruction {} trap intrinsic takes no arguments and has no result",
                        function.id, instruction.id
                    ));
                }
            }
            Intrinsic::SquareRoot
            | Intrinsic::Sine
            | Intrinsic::Cosine
            | Intrinsic::Arctangent
            | Intrinsic::Log2
            | Intrinsic::Exp2 => {
                if instruction.results.len() != 1 {
                    self.error(format!(
                        "function {} instruction {} floating intrinsic has {} results, expected 1",
                        function.id,
                        instruction.id,
                        instruction.results.len()
                    ));
                    return;
                }
                let Some(result) = self.single_result_type(instruction) else {
                    return;
                };
                self.require_float(
                    result,
                    format!(
                        "function {} instruction {} intrinsic result",
                        function.id, instruction.id
                    ),
                );
                if let Some(argument) = arguments.first() {
                    self.require_operand_type(
                        argument,
                        result,
                        values,
                        format!(
                            "function {} instruction {} intrinsic argument",
                            function.id, instruction.id
                        ),
                    );
                }
                if arguments.len() != 1 {
                    self.error(format!(
                        "function {} instruction {} floating intrinsic has {} arguments, expected 1",
                        function.id, instruction.id, arguments.len()
                    ));
                }
            }
            Intrinsic::MemoryCopy | Intrinsic::MemoryMove | Intrinsic::MemorySet => {
                // Their exact operand contracts are intentionally not encoded in
                // the IR model yet. Structural verification still validates each
                // operand reference, and opaque pointers prevent pointee checks.
            }
        }
    }

    fn verify_terminator(
        &mut self,
        function: &Function,
        terminator: &Terminator,
        values: &ValueTypes,
    ) {
        match terminator {
            Terminator::Jump(_) | Terminator::Unreachable => {}
            Terminator::Branch { condition, .. } => self.require_operand_i1(
                condition,
                values,
                format!("function {} branch condition", function.id),
            ),
            Terminator::Switch { selector, .. } => self.require_operand_integer(
                selector,
                values,
                format!("function {} switch selector", function.id),
            ),
            Terminator::Return(value) => {
                let Some(is_void) = self
                    .types
                    .get(function.signature.result)
                    .map(|kind| matches!(kind, TypeKind::Void))
                else {
                    return;
                };
                match (is_void, value) {
                    (true, _) => {}
                    (false, Some(value)) => self.require_operand_type(
                        value,
                        function.signature.result,
                        values,
                        format!("function {} return value", function.id),
                    ),
                    (false, None) => {}
                }
            }
        }
    }

    fn verify_constant(&mut self, constant: &Constant, type_id: TypeId, context: String) {
        let Some(kind) = self.constant_type(type_id) else {
            return;
        };
        match (constant, kind) {
            (Constant::Integer(_), ConstantType::Integer)
            | (Constant::Float(_), ConstantType::Float)
            | (Constant::Null, ConstantType::Pointer)
            | (Constant::GlobalAddress { .. }, ConstantType::Pointer)
            | (Constant::FunctionAddress(_), ConstantType::Pointer) => {}
            (Constant::Undefined, ConstantType::Void) => {
                self.error(format!("{context} cannot be undefined void"))
            }
            (Constant::Undefined, _) => {}
            (
                Constant::Bytes(bytes) | Constant::RelocatableBytes { bytes, .. },
                ConstantType::Array { element, length },
            ) => {
                if length != bytes.len() as u64 {
                    self.error(format!(
                        "{context} byte length does not match its array type"
                    ));
                }
                let is_byte =
                    matches!(self.types.get(element), Some(TypeKind::Integer { bits: 8 }));
                if self.known_type(element) && !is_byte {
                    self.error(format!("{context} bytes require an array of i8"));
                }
            }
            (Constant::Aggregate(values), ConstantType::Array { element, length }) => {
                if length != values.len() as u64 {
                    self.error(format!(
                        "{context} aggregate length does not match its array type"
                    ));
                }
                for (index, value) in values.iter().enumerate() {
                    self.verify_typed_constant(
                        value,
                        Some(element),
                        format!("{context} aggregate element {index}"),
                    );
                }
            }
            (Constant::Aggregate(values), ConstantType::Structure(fields)) => {
                if fields.len() != values.len() {
                    self.error(format!(
                        "{context} aggregate length does not match its structure type"
                    ));
                }
                for (index, (value, field)) in values.iter().zip(&fields).enumerate() {
                    self.verify_typed_constant(
                        value,
                        Some(*field),
                        format!("{context} aggregate field {index}"),
                    );
                }
            }
            (Constant::Aggregate(_), _)
            | (Constant::Bytes(_), _)
            | (Constant::RelocatableBytes { .. }, _) => {
                self.error(format!(
                    "{context} has a constant kind incompatible with type {type_id}"
                ));
            }
            _ => self.error(format!(
                "{context} has a constant kind incompatible with type {type_id}"
            )),
        }
    }

    fn verify_typed_constant(
        &mut self,
        constant: &TypedConstant,
        expected: Option<TypeId>,
        context: String,
    ) {
        if let Some(expected) = expected {
            if self.known_type(constant.type_id)
                && self.known_type(expected)
                && constant.type_id != expected
            {
                self.error(format!(
                    "{context} has type {}, expected {}",
                    constant.type_id, expected
                ));
            }
        }
        self.verify_constant(&constant.value, constant.type_id, context);
    }

    fn operand_type(&self, operand: &Operand, values: &ValueTypes) -> Option<TypeId> {
        match operand {
            Operand::Value(id) => values.get(*id).filter(|type_id| self.known_type(*type_id)),
            Operand::Constant(constant) => self
                .known_type(constant.type_id)
                .then_some(constant.type_id),
        }
    }

    fn require_operand_type(
        &mut self,
        operand: &Operand,
        expected: TypeId,
        values: &ValueTypes,
        context: String,
    ) {
        let Some(actual) = self.operand_type(operand, values) else {
            return;
        };
        if self.known_type(expected) && actual != expected {
            self.error(format!("{context} has type {actual}, expected {expected}"));
        }
    }

    fn require_same_operand_types(
        &mut self,
        left: &Operand,
        right: &Operand,
        values: &ValueTypes,
        context: String,
    ) {
        let Some(left) = self.operand_type(left, values) else {
            return;
        };
        let Some(right) = self.operand_type(right, values) else {
            return;
        };
        if left != right {
            self.error(format!("{context} have types {left} and {right}"));
        }
    }

    fn require_operand_i1(&mut self, operand: &Operand, values: &ValueTypes, context: String) {
        if let Some(type_id) = self.operand_type(operand, values) {
            self.require_i1(type_id, context);
        }
    }

    fn require_operand_integer(&mut self, operand: &Operand, values: &ValueTypes, context: String) {
        if let Some(type_id) = self.operand_type(operand, values) {
            self.require_integer(type_id, context);
        }
    }

    fn require_operand_pointer(&mut self, operand: &Operand, values: &ValueTypes, context: String) {
        if let Some(type_id) = self.operand_type(operand, values) {
            self.require_pointer(type_id, context);
        }
    }

    fn require_operand_non_void(
        &mut self,
        operand: &Operand,
        values: &ValueTypes,
        context: String,
    ) {
        if let Some(type_id) = self.operand_type(operand, values) {
            self.require_non_void(type_id, context);
        }
    }

    fn single_result_type(&self, instruction: &Instruction) -> Option<TypeId> {
        (instruction.results.len() == 1)
            .then(|| instruction.results[0].type_id)
            .filter(|type_id| self.known_type(*type_id))
    }

    fn require_i1(&mut self, type_id: TypeId, context: String) {
        if self.known_type(type_id)
            && !matches!(self.types.get(type_id), Some(TypeKind::Integer { bits: 1 }))
        {
            self.error(format!("{context} has type {type_id}, expected i1"));
        }
    }

    fn require_integer(&mut self, type_id: TypeId, context: String) {
        if self.known_type(type_id)
            && !matches!(self.types.get(type_id), Some(TypeKind::Integer { .. }))
        {
            self.error(format!("{context} has type {type_id}, expected integer"));
        }
    }

    fn require_integer_or_pointer(&mut self, type_id: TypeId, context: String) {
        if self.known_type(type_id)
            && !matches!(
                self.types.get(type_id),
                Some(TypeKind::Integer { .. } | TypeKind::Pointer { .. })
            )
        {
            self.error(format!(
                "{context} has type {type_id}, expected integer or pointer"
            ));
        }
    }

    fn require_float(&mut self, type_id: TypeId, context: String) {
        if self.known_type(type_id) && !matches!(self.types.get(type_id), Some(TypeKind::Float(_)))
        {
            self.error(format!("{context} has type {type_id}, expected float"));
        }
    }

    fn require_pointer(&mut self, type_id: TypeId, context: String) {
        if self.known_type(type_id)
            && !matches!(self.types.get(type_id), Some(TypeKind::Pointer { .. }))
        {
            self.error(format!("{context} has type {type_id}, expected pointer"));
        }
    }

    fn require_non_void(&mut self, type_id: TypeId, context: String) {
        if matches!(self.types.get(type_id), Some(TypeKind::Void)) {
            self.error(format!("{context} has void type"));
        }
    }

    fn require_integer_pair(
        &mut self,
        source: TypeId,
        target: TypeId,
        context: &str,
        relation: impl FnOnce(u16, u16) -> bool,
    ) {
        let pair = match (self.types.get(source), self.types.get(target)) {
            (
                Some(TypeKind::Integer { bits: source }),
                Some(TypeKind::Integer { bits: target }),
            ) => Some((*source, *target)),
            _ => None,
        };
        let Some((source, target)) = pair else {
            self.error(format!(
                "{context} requires integer source and target types"
            ));
            return;
        };
        if !relation(source, target) {
            self.error(format!("{context} has incompatible integer widths"));
        }
    }

    fn require_float_pair(
        &mut self,
        source: TypeId,
        target: TypeId,
        context: &str,
        relation: impl FnOnce(u8, u8) -> bool,
    ) {
        let pair = match (self.types.get(source), self.types.get(target)) {
            (Some(TypeKind::Float(source)), Some(TypeKind::Float(target))) => {
                Some((*source, *target))
            }
            _ => None,
        };
        let Some((source, target)) = pair else {
            self.error(format!("{context} requires float source and target types"));
            return;
        };
        if !relation(float_rank(source), float_rank(target)) {
            self.error(format!("{context} has incompatible float widths"));
        }
    }

    fn require_kinds(
        &mut self,
        source: TypeId,
        target: TypeId,
        context: &str,
        source_predicate: fn(&TypeKind) -> bool,
        target_predicate: fn(&TypeKind) -> bool,
    ) {
        let compatible = match (self.types.get(source), self.types.get(target)) {
            (Some(source), Some(target)) => source_predicate(source) && target_predicate(target),
            _ => return,
        };
        if !compatible {
            self.error(format!(
                "{context} has incompatible source and target types"
            ));
        }
    }

    fn require_non_void_pair(&mut self, source: TypeId, target: TypeId, context: &str) {
        let has_void = match (self.types.get(source), self.types.get(target)) {
            (Some(source), Some(target)) => {
                matches!(source, TypeKind::Void) || matches!(target, TypeKind::Void)
            }
            _ => return,
        };
        if has_void {
            self.error(format!("{context} cannot bitcast void"));
        }
    }

    fn known_type(&self, type_id: TypeId) -> bool {
        self.types.get(type_id).is_some()
    }

    fn constant_type(&self, type_id: TypeId) -> Option<ConstantType> {
        self.types.get(type_id).map(|kind| match kind {
            TypeKind::Void => ConstantType::Void,
            TypeKind::Integer { .. } => ConstantType::Integer,
            TypeKind::Float(_) => ConstantType::Float,
            TypeKind::Pointer { .. } => ConstantType::Pointer,
            TypeKind::Array { element, length } => ConstantType::Array {
                element: *element,
                length: *length,
            },
            TypeKind::Structure { fields, .. } => ConstantType::Structure(fields.clone()),
        })
    }

    fn error(&mut self, message: String) {
        self.diagnostics
            .push(Diagnostic::new(Severity::Error, message));
    }
}

enum ConstantType {
    Void,
    Integer,
    Float,
    Pointer,
    Array { element: TypeId, length: u64 },
    Structure(Vec<TypeId>),
}

struct TypeTable<'module> {
    kinds: BTreeMap<TypeId, &'module TypeKind>,
    duplicates: BTreeSet<TypeId>,
}

impl<'module> TypeTable<'module> {
    fn new(module: &'module Module) -> Self {
        let mut kinds = BTreeMap::new();
        let mut duplicates = BTreeSet::new();
        for type_ in &module.types {
            if kinds.insert(type_.id, &type_.kind).is_some() {
                duplicates.insert(type_.id);
            }
        }
        Self { kinds, duplicates }
    }

    fn get(&self, type_id: TypeId) -> Option<&'module TypeKind> {
        (!self.duplicates.contains(&type_id))
            .then(|| self.kinds.get(&type_id).copied())
            .flatten()
    }
}

struct FunctionTable<'module> {
    functions: BTreeMap<FunctionId, &'module Function>,
    duplicates: BTreeSet<FunctionId>,
}

impl<'module> FunctionTable<'module> {
    fn new(module: &'module Module) -> Self {
        let mut functions = BTreeMap::new();
        let mut duplicates = BTreeSet::new();
        for function in &module.functions {
            if functions.insert(function.id, function).is_some() {
                duplicates.insert(function.id);
            }
        }
        Self {
            functions,
            duplicates,
        }
    }

    fn get(&self, id: FunctionId) -> Option<&'module Function> {
        (!self.duplicates.contains(&id))
            .then(|| self.functions.get(&id).copied())
            .flatten()
    }
}

struct ValueTypes {
    types: BTreeMap<ValueId, TypeId>,
    duplicates: BTreeSet<ValueId>,
}

impl ValueTypes {
    fn new(function: &Function) -> Self {
        let mut types = BTreeMap::new();
        let mut duplicates = BTreeSet::new();
        for value in function
            .parameters
            .iter()
            .chain(function.blocks.iter().flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .flat_map(|instruction| instruction.results.iter())
            }))
        {
            if types.insert(value.id, value.type_id).is_some() {
                duplicates.insert(value.id);
            }
        }
        Self { types, duplicates }
    }

    fn get(&self, value: ValueId) -> Option<TypeId> {
        (!self.duplicates.contains(&value))
            .then(|| self.types.get(&value).copied())
            .flatten()
    }
}

fn is_integer(kind: &TypeKind) -> bool {
    matches!(kind, TypeKind::Integer { .. })
}

fn is_float(kind: &TypeKind) -> bool {
    matches!(kind, TypeKind::Float(_))
}

fn is_pointer(kind: &TypeKind) -> bool {
    matches!(kind, TypeKind::Pointer { .. })
}

fn float_rank(kind: FloatKind) -> u8 {
    match kind {
        FloatKind::Binary32 => 32,
        FloatKind::Binary64 => 64,
        FloatKind::Extended80 => 80,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BinaryOp, Block, BlockId, CallingConvention, Constant, FunctionId, InstructionId, Linkage,
        Signature, Type, TypedConstant, Value,
    };

    const VOID: TypeId = TypeId::new(0);
    const I1: TypeId = TypeId::new(1);
    const I8: TypeId = TypeId::new(2);
    const I16: TypeId = TypeId::new(3);

    fn integer(type_id: TypeId, value: i128) -> Operand {
        Operand::Constant(TypedConstant {
            type_id,
            value: Constant::Integer(value),
        })
    }

    fn value(id: u32, type_id: TypeId) -> Value {
        Value {
            id: ValueId::new(id),
            type_id,
        }
    }

    fn module(result: TypeId, instructions: Vec<Instruction>, terminator: Terminator) -> Module {
        Module {
            name: "type-verify-test".into(),
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
                Type {
                    id: I16,
                    kind: TypeKind::Integer { bits: 16 },
                },
            ],
            globals: Vec::new(),
            functions: vec![Function {
                id: FunctionId::new(0),
                name: "main".into(),
                signature: Signature {
                    result,
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: CallingConvention::C,
                },
                linkage: Linkage::Internal,
                attributes: Vec::new(),
                parameters: Vec::new(),
                blocks: vec![Block {
                    id: BlockId::new(0),
                    instructions,
                    terminator,
                }],
            }],
        }
    }

    #[test]
    fn rejects_mismatched_binary_operand_types() {
        let module = module(
            I8,
            vec![Instruction {
                id: InstructionId::new(0),
                results: vec![value(0, I8)],
                kind: InstructionKind::Binary {
                    op: BinaryOp::Add,
                    left: integer(I8, 1),
                    right: integer(I16, 2),
                },
            }],
            Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
        );

        assert!(
            verify_types(&module)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("binary right operand"))
        );
    }

    #[test]
    fn rejects_non_i1_branch_conditions() {
        let module = module(
            I8,
            Vec::new(),
            Terminator::Branch {
                condition: integer(I8, 1),
                then_block: BlockId::new(0),
                else_block: BlockId::new(0),
            },
        );

        assert!(
            verify_types(&module)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expected i1"))
        );
    }

    #[test]
    fn rejects_return_values_with_the_wrong_type() {
        let module = module(I8, Vec::new(), Terminator::Return(Some(integer(I16, 7))));

        assert!(verify_types(&module).iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("return value has type 3, expected 2")
        }));
    }

    #[test]
    fn accepts_a_valid_typed_function() {
        let module = module(
            I8,
            vec![Instruction {
                id: InstructionId::new(0),
                results: vec![value(0, I8)],
                kind: InstructionKind::Binary {
                    op: BinaryOp::Add,
                    left: integer(I8, 1),
                    right: integer(I8, 2),
                },
            }],
            Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
        );

        assert!(verify_types(&module).is_empty());
    }

    #[test]
    fn rejects_ordered_float_comparison_on_integers() {
        let module = module(
            I1,
            vec![Instruction {
                id: InstructionId::new(0),
                results: vec![value(0, I1)],
                kind: InstructionKind::Compare {
                    predicate: ComparePredicate::OrderedEqual,
                    left: integer(I8, 1),
                    right: integer(I8, 1),
                },
            }],
            Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
        );

        assert!(verify_types(&module).iter().any(|diagnostic| {
            diagnostic.message.contains("compare operands")
                && diagnostic.message.contains("expected float")
        }));
    }

    #[test]
    fn rejects_void_ssa_results() {
        let module = module(
            VOID,
            vec![Instruction {
                id: InstructionId::new(0),
                results: vec![value(0, VOID)],
                kind: InstructionKind::Phi {
                    incoming: Vec::new(),
                },
            }],
            Terminator::Return(None),
        );

        assert!(verify_types(&module).iter().any(|diagnostic| {
            diagnostic.message.contains("instruction 0 result 0")
                && diagnostic.message.contains("void type")
        }));
    }
}
