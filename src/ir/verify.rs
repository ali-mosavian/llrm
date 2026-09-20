//! Structural verification for portable SSA IR.
//!
//! This pass deliberately establishes only representation invariants.  It
//! does not prove dominance, infer full instruction types, or judge
//! optimization legality; those require analyses layered above this module.

use std::collections::BTreeSet;

use crate::support::diagnostic::{Diagnostic, Severity};

use super::{
    Block, BlockId, Callee, Constant, Function, FunctionAttribute, FunctionId, GlobalId,
    GlobalRelocation, Instruction, InstructionKind, Module, Operand, Terminator, TypeId, TypeKind,
    TypedConstant, Value, ValueId,
};

/// Validates the representation-level invariants of a portable IR module.
///
/// All discoverable violations are returned together in deterministic source
/// order.  This lets a parser or transformation report a useful complete set
/// of structural errors without performing I/O.
pub fn verify(module: &Module) -> Result<(), Vec<Diagnostic>> {
    let mut verifier = Verifier::new(module);
    verifier.verify();
    verifier
        .diagnostics
        .extend(super::type_verify::verify_types(module));
    if verifier.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(verifier.diagnostics)
    }
}

impl Module {
    /// Validates this module's structural SSA invariants.
    pub fn verify(&self) -> Result<(), Vec<Diagnostic>> {
        verify(self)
    }
}

struct Verifier<'module> {
    module: &'module Module,
    type_ids: BTreeSet<TypeId>,
    global_ids: BTreeSet<GlobalId>,
    function_ids: BTreeSet<FunctionId>,
    diagnostics: Vec<Diagnostic>,
}

impl<'module> Verifier<'module> {
    fn new(module: &'module Module) -> Self {
        Self {
            module,
            type_ids: BTreeSet::new(),
            global_ids: BTreeSet::new(),
            function_ids: BTreeSet::new(),
            diagnostics: Vec::new(),
        }
    }

    fn verify(&mut self) {
        for ty in &self.module.types {
            if !self.type_ids.insert(ty.id) {
                self.error(format!("duplicate type id {}", ty.id));
            }
        }
        for global in &self.module.globals {
            if !self.global_ids.insert(global.id) {
                self.error(format!("duplicate global id {}", global.id));
            }
        }
        for function in &self.module.functions {
            if !self.function_ids.insert(function.id) {
                self.error(format!("duplicate function id {}", function.id));
            }
        }

        for ty in &self.module.types {
            self.verify_type(ty.id, &ty.kind);
        }
        for global in &self.module.globals {
            self.require_type(global.type_id, format!("global {} type", global.id));
            if let Some(initializer) = &global.initializer {
                self.verify_constant_value(
                    initializer,
                    format!("global {} initializer", global.id),
                );
            }
        }
        for function in &self.module.functions {
            self.verify_function(function);
        }
    }

    fn verify_type(&mut self, id: TypeId, kind: &TypeKind) {
        match kind {
            TypeKind::Integer { bits: 0 } => {
                self.error(format!("integer type {} has zero width", id));
            }
            TypeKind::Array { element, .. } => {
                self.require_type(*element, format!("array type {} element", id));
            }
            TypeKind::Structure { fields, .. } => {
                for (index, field) in fields.iter().enumerate() {
                    self.require_type(*field, format!("structure type {} field {}", id, index));
                }
            }
            TypeKind::Void
            | TypeKind::Integer { .. }
            | TypeKind::Float(_)
            | TypeKind::Pointer { .. } => {}
        }
    }

    fn verify_function(&mut self, function: &Function) {
        self.require_type(
            function.signature.result,
            format!("function {} result type", function.id),
        );
        for (index, type_id) in function.signature.parameters.iter().enumerate() {
            self.require_type(
                *type_id,
                format!("function {} parameter {} type", function.id, index),
            );
        }
        self.verify_attributes(function.id, &function.attributes);

        if function.blocks.is_empty() {
            if !matches!(function.linkage, super::Linkage::External) {
                self.error(format!(
                    "internal function {} is a declaration without blocks",
                    function.id
                ));
            }
        }

        if function.parameters.len() != function.signature.parameters.len() {
            self.error(format!(
                "function {} has {} parameters but signature declares {}",
                function.id,
                function.parameters.len(),
                function.signature.parameters.len()
            ));
        }
        for (index, parameter) in function.parameters.iter().enumerate() {
            self.verify_value_type(
                parameter,
                format!("function {} parameter {}", function.id, index),
            );
            if let Some(expected) = function.signature.parameters.get(index) {
                if parameter.type_id != *expected {
                    self.error(format!(
                        "function {} parameter {} has type {}, expected {}",
                        function.id, index, parameter.type_id, expected
                    ));
                }
            }
        }

        let mut block_ids = BTreeSet::new();
        let mut instruction_ids = BTreeSet::new();
        let mut value_ids = BTreeSet::new();
        for (index, parameter) in function.parameters.iter().enumerate() {
            if !value_ids.insert(parameter.id) {
                self.error(format!(
                    "function {} parameter {} redefines value {}",
                    function.id, index, parameter.id
                ));
            }
        }
        for block in &function.blocks {
            if !block_ids.insert(block.id) {
                self.error(format!(
                    "function {} has duplicate block id {}",
                    function.id, block.id
                ));
            }
            for instruction in &block.instructions {
                if !instruction_ids.insert(instruction.id) {
                    self.error(format!(
                        "function {} has duplicate instruction id {}",
                        function.id, instruction.id
                    ));
                }
                for result in &instruction.results {
                    if !value_ids.insert(result.id) {
                        self.error(format!(
                            "function {} instruction {} redefines value {}",
                            function.id, instruction.id, result.id
                        ));
                    }
                }
            }
        }

        let predecessors = self.predecessors(function, &block_ids);
        for block in &function.blocks {
            self.verify_block(function, block, &value_ids, &predecessors);
        }
    }

    fn verify_attributes(&mut self, function: FunctionId, attributes: &[FunctionAttribute]) {
        for pair in attributes.windows(2) {
            if pair[0] >= pair[1] {
                self.error(format!(
                    "function {} attributes must be unique and sorted",
                    function
                ));
                break;
            }
        }
    }

    fn predecessors(
        &mut self,
        function: &Function,
        blocks: &BTreeSet<BlockId>,
    ) -> Vec<(BlockId, BTreeSet<BlockId>)> {
        let mut predecessors = function
            .blocks
            .iter()
            .map(|block| (block.id, BTreeSet::new()))
            .collect::<Vec<_>>();

        for block in &function.blocks {
            for target in terminator_targets(&block.terminator) {
                if !blocks.contains(&target) {
                    self.error(format!(
                        "function {} block {} targets unknown block {}",
                        function.id, block.id, target
                    ));
                    continue;
                }
                if let Some((_, incoming)) = predecessors.iter_mut().find(|(id, _)| *id == target) {
                    incoming.insert(block.id);
                }
            }
        }
        predecessors
    }

    fn verify_block(
        &mut self,
        function: &Function,
        block: &Block,
        values: &BTreeSet<ValueId>,
        predecessors: &[(BlockId, BTreeSet<BlockId>)],
    ) {
        let mut saw_non_phi = false;
        for instruction in &block.instructions {
            let is_phi = matches!(instruction.kind, InstructionKind::Phi { .. });
            if saw_non_phi && is_phi {
                self.error(format!(
                    "function {} block {} has phi instruction {} after a non-phi instruction",
                    function.id, block.id, instruction.id
                ));
            }
            saw_non_phi |= !is_phi;
            self.verify_instruction(function, block, instruction, values, predecessors);
        }
        self.verify_terminator(function, block, values);
    }

    fn verify_instruction(
        &mut self,
        function: &Function,
        block: &Block,
        instruction: &Instruction,
        values: &BTreeSet<ValueId>,
        predecessors: &[(BlockId, BTreeSet<BlockId>)],
    ) {
        let expected_results = match &instruction.kind {
            InstructionKind::Store { .. } => Some(0),
            InstructionKind::Phi { .. }
            | InstructionKind::StackAlloc { .. }
            | InstructionKind::Unary { .. }
            | InstructionKind::Binary { .. }
            | InstructionKind::Compare { .. }
            | InstructionKind::Cast { .. }
            | InstructionKind::Load { .. }
            | InstructionKind::GetElementPointer { .. }
            | InstructionKind::Select { .. } => Some(1),
            InstructionKind::Call { .. } | InstructionKind::Intrinsic { .. } => None,
        };
        if let Some(expected) = expected_results {
            if instruction.results.len() != expected {
                self.error(format!(
                    "function {} block {} instruction {} has {} results, expected {}",
                    function.id,
                    block.id,
                    instruction.id,
                    instruction.results.len(),
                    expected
                ));
            }
        }
        if expected_results.is_none() && instruction.results.len() > 1 {
            self.error(format!(
                "function {} block {} instruction {} has {} results, expected at most 1",
                function.id,
                block.id,
                instruction.id,
                instruction.results.len()
            ));
        }
        for result in &instruction.results {
            self.verify_value_type(
                result,
                format!(
                    "function {} instruction {} result {}",
                    function.id, instruction.id, result.id
                ),
            );
        }

        match &instruction.kind {
            InstructionKind::Phi { incoming } => {
                let Some(expected) = predecessors
                    .iter()
                    .find(|(id, _)| *id == block.id)
                    .map(|(_, incoming)| incoming)
                else {
                    self.error(format!(
                        "function {} block {} has no CFG predecessor entry",
                        function.id, block.id
                    ));
                    return;
                };
                let mut incoming_blocks = BTreeSet::new();
                for (index, source) in incoming.iter().enumerate() {
                    if !incoming_blocks.insert(source.predecessor) {
                        self.error(format!(
                            "function {} block {} phi instruction {} repeats predecessor {}",
                            function.id, block.id, instruction.id, source.predecessor
                        ));
                    }
                    self.verify_operand(
                        &source.value,
                        values,
                        format!(
                            "function {} block {} phi instruction {} incoming {}",
                            function.id, block.id, instruction.id, index
                        ),
                    );
                }
                if incoming_blocks != *expected {
                    self.error(format!(
                        "function {} block {} phi instruction {} predecessors do not match the CFG",
                        function.id, block.id, instruction.id
                    ));
                }
            }
            InstructionKind::StackAlloc {
                size, alignment, ..
            } => {
                if *size == 0 {
                    self.error(format!(
                        "function {} instruction {} stack allocation has zero size",
                        function.id, instruction.id
                    ));
                }
                if *alignment == 0 || !alignment.is_power_of_two() {
                    self.error(format!(
                        "function {} instruction {} stack allocation alignment must be a nonzero power of two",
                        function.id, instruction.id
                    ));
                }
            }
            InstructionKind::Unary { operand, .. } => self.verify_operand(
                operand,
                values,
                format!(
                    "function {} instruction {} operand",
                    function.id, instruction.id
                ),
            ),
            InstructionKind::Binary { left, right, .. }
            | InstructionKind::Compare { left, right, .. } => {
                self.verify_operand(
                    left,
                    values,
                    format!(
                        "function {} instruction {} left operand",
                        function.id, instruction.id
                    ),
                );
                self.verify_operand(
                    right,
                    values,
                    format!(
                        "function {} instruction {} right operand",
                        function.id, instruction.id
                    ),
                );
            }
            InstructionKind::Cast { operand, to, .. } => {
                self.verify_operand(
                    operand,
                    values,
                    format!(
                        "function {} instruction {} operand",
                        function.id, instruction.id
                    ),
                );
                self.require_type(
                    *to,
                    format!(
                        "function {} instruction {} cast target",
                        function.id, instruction.id
                    ),
                );
            }
            InstructionKind::Load { address, .. } => self.verify_operand(
                address,
                values,
                format!(
                    "function {} instruction {} address",
                    function.id, instruction.id
                ),
            ),
            InstructionKind::Store { address, value, .. } => {
                self.verify_operand(
                    address,
                    values,
                    format!(
                        "function {} instruction {} address",
                        function.id, instruction.id
                    ),
                );
                self.verify_operand(
                    value,
                    values,
                    format!(
                        "function {} instruction {} value",
                        function.id, instruction.id
                    ),
                );
            }
            InstructionKind::GetElementPointer { base, indices } => {
                self.verify_operand(
                    base,
                    values,
                    format!(
                        "function {} instruction {} base",
                        function.id, instruction.id
                    ),
                );
                for (index, operand) in indices.iter().enumerate() {
                    self.verify_operand(
                        operand,
                        values,
                        format!(
                            "function {} instruction {} index {}",
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
                for (name, operand) in [
                    ("condition", condition),
                    ("then value", then_value),
                    ("else value", else_value),
                ] {
                    self.verify_operand(
                        operand,
                        values,
                        format!(
                            "function {} instruction {} {}",
                            function.id, instruction.id, name
                        ),
                    );
                }
            }
            InstructionKind::Call {
                callee, arguments, ..
            } => {
                match callee {
                    Callee::Direct(id) => self.require_function(
                        *id,
                        format!(
                            "function {} instruction {} direct callee",
                            function.id, instruction.id
                        ),
                    ),
                    Callee::Indirect(operand) => self.verify_operand(
                        operand,
                        values,
                        format!(
                            "function {} instruction {} indirect callee",
                            function.id, instruction.id
                        ),
                    ),
                }
                for (index, argument) in arguments.iter().enumerate() {
                    self.verify_operand(
                        argument,
                        values,
                        format!(
                            "function {} instruction {} argument {}",
                            function.id, instruction.id, index
                        ),
                    );
                }
            }
            InstructionKind::Intrinsic { arguments, .. } => {
                for (index, argument) in arguments.iter().enumerate() {
                    self.verify_operand(
                        argument,
                        values,
                        format!(
                            "function {} instruction {} argument {}",
                            function.id, instruction.id, index
                        ),
                    );
                }
            }
        }
    }

    fn verify_terminator(
        &mut self,
        function: &Function,
        block: &Block,
        values: &BTreeSet<ValueId>,
    ) {
        match &block.terminator {
            Terminator::Jump(_) | Terminator::Unreachable => {}
            Terminator::Branch { condition, .. } => self.verify_operand(
                condition,
                values,
                format!(
                    "function {} block {} branch condition",
                    function.id, block.id
                ),
            ),
            Terminator::Switch {
                selector, cases, ..
            } => {
                self.verify_operand(
                    selector,
                    values,
                    format!(
                        "function {} block {} switch selector",
                        function.id, block.id
                    ),
                );
                let mut case_values = BTreeSet::new();
                for (value, _) in cases {
                    if !case_values.insert(*value) {
                        self.error(format!(
                            "function {} block {} has duplicate switch case {}",
                            function.id, block.id, value
                        ));
                    }
                }
            }
            Terminator::Return(value) => {
                if let Some(result_type) = self
                    .module
                    .types
                    .iter()
                    .find(|ty| ty.id == function.signature.result)
                {
                    let returns_void = matches!(result_type.kind, TypeKind::Void);
                    if returns_void == value.is_some() {
                        self.error(format!(
                            "function {} block {} return value does not match its result type",
                            function.id, block.id
                        ));
                    }
                }
                if let Some(value) = value {
                    self.verify_operand(
                        value,
                        values,
                        format!("function {} block {} return value", function.id, block.id),
                    );
                }
            }
        }
    }

    fn verify_operand(&mut self, operand: &Operand, values: &BTreeSet<ValueId>, context: String) {
        match operand {
            Operand::Value(id) if !values.contains(id) => {
                self.error(format!("{} references unknown value {}", context, id));
            }
            Operand::Value(_) => {}
            Operand::Constant(constant) => self.verify_constant(constant, context),
        }
    }

    fn verify_constant(&mut self, constant: &TypedConstant, context: String) {
        self.require_type(constant.type_id, format!("{} type", context));
        self.verify_constant_value(&constant.value, context);
    }

    fn verify_constant_value(&mut self, constant: &Constant, context: String) {
        match constant {
            Constant::Aggregate(values) => {
                for (index, value) in values.iter().enumerate() {
                    self.verify_constant(value, format!("{} aggregate element {}", context, index));
                }
            }
            Constant::RelocatableBytes { bytes, relocations } => {
                self.verify_relocatable_bytes(bytes, relocations, context);
            }
            Constant::GlobalAddress { global, .. } => {
                self.require_global(*global, format!("{} global address", context));
            }
            Constant::FunctionAddress(function) => {
                self.require_function(*function, format!("{} function address", context));
            }
            Constant::Integer(_)
            | Constant::Float(_)
            | Constant::Null
            | Constant::Undefined
            | Constant::Bytes(_) => {}
        }
    }

    fn verify_relocatable_bytes(
        &mut self,
        bytes: &[u8],
        relocations: &[GlobalRelocation],
        context: String,
    ) {
        let byte_length = bytes.len() as u64;
        let mut patches = Vec::new();

        for (index, relocation) in relocations.iter().enumerate() {
            self.require_global(
                relocation.target,
                format!("{context} relocation {index} target"),
            );
            if relocation.width == 0 {
                self.error(format!(
                    "{context} relocation {index} has a zero-width patch"
                ));
                continue;
            }
            let Some(end) = relocation.offset.checked_add(u64::from(relocation.width)) else {
                self.error(format!(
                    "{context} relocation {index} patch range overflows its byte offset"
                ));
                continue;
            };
            if end > byte_length {
                self.error(format!(
                    "{context} relocation {index} patch range {}..{end} is outside {} bytes",
                    relocation.offset, byte_length
                ));
                continue;
            }
            for &(previous_offset, previous_end, previous_index) in &patches {
                if relocation.offset < previous_end && previous_offset < end {
                    self.error(format!(
                        "{context} relocation {index} patch range {}..{end} overlaps relocation {previous_index} patch range {previous_offset}..{previous_end}",
                        relocation.offset
                    ));
                }
            }
            patches.push((relocation.offset, end, index));
        }
    }

    fn verify_value_type(&mut self, value: &Value, context: String) {
        self.require_type(value.type_id, format!("{} type", context));
    }

    fn require_type(&mut self, id: TypeId, context: String) {
        if !self.type_ids.contains(&id) {
            self.error(format!("{} references unknown type {}", context, id));
        }
    }

    fn require_global(&mut self, id: GlobalId, context: String) {
        if !self.global_ids.contains(&id) {
            self.error(format!("{} references unknown global {}", context, id));
        }
    }

    fn require_function(&mut self, id: FunctionId, context: String) {
        if !self.function_ids.contains(&id) {
            self.error(format!("{} references unknown function {}", context, id));
        }
    }

    fn error(&mut self, message: String) {
        self.diagnostics
            .push(Diagnostic::new(Severity::Error, message));
    }
}

fn terminator_targets(terminator: &Terminator) -> Vec<BlockId> {
    match terminator {
        Terminator::Jump(target) => vec![*target],
        Terminator::Branch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        Terminator::Switch { cases, default, .. } => {
            let mut targets = cases.iter().map(|(_, target)| *target).collect::<Vec<_>>();
            targets.push(*default);
            targets
        }
        Terminator::Return(_) | Terminator::Unreachable => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        AddressSpace, Block, Constant, Function, FunctionId, Global, GlobalRelocation, Instruction,
        InstructionId, InstructionKind, Linkage, Module, Signature, Terminator, Type, TypeId,
        TypeKind, Value,
    };

    fn void_type() -> Type {
        Type {
            id: TypeId::new(0),
            kind: TypeKind::Void,
        }
    }

    fn minimal_module() -> Module {
        Module {
            name: "minimal".into(),
            types: vec![void_type()],
            globals: Vec::new(),
            functions: vec![Function {
                id: FunctionId::new(0),
                name: "main".into(),
                signature: Signature {
                    result: TypeId::new(0),
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: super::super::CallingConvention::FarPascal,
                },
                linkage: Linkage::Internal,
                attributes: Vec::new(),
                parameters: Vec::new(),
                blocks: vec![Block {
                    id: BlockId::new(0),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                }],
            }],
        }
    }

    #[test]
    fn accepts_a_minimal_defined_function() {
        assert!(verify(&minimal_module()).is_ok());
    }

    #[test]
    fn reports_multiple_structural_errors() {
        let mut module = minimal_module();
        module.types.push(Type {
            id: TypeId::new(0),
            kind: TypeKind::Integer { bits: 0 },
        });
        module.functions[0].parameters.push(Value {
            id: ValueId::new(0),
            type_id: TypeId::new(7),
        });
        module.functions[0].blocks[0].terminator = Terminator::Jump(BlockId::new(9));

        let diagnostics = verify(&module).expect_err("malformed module must be rejected");
        assert!(diagnostics.len() >= 4);
    }

    #[test]
    fn rejects_a_missing_non_void_return_value() {
        let mut module = minimal_module();
        module.types.push(Type {
            id: TypeId::new(1),
            kind: TypeKind::Integer { bits: 16 },
        });
        module.functions[0].signature.result = TypeId::new(1);

        let diagnostics = verify(&module).expect_err("non-void return needs a value");

        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message
            == "function 0 block 0 return value does not match its result type"));
    }

    #[test]
    fn rejects_zero_sized_or_misaligned_stack_allocations() {
        let mut module = minimal_module();
        module.types.push(Type {
            id: TypeId::new(1),
            kind: TypeKind::Pointer {
                address_space: AddressSpace::NearData,
            },
        });
        module.functions[0].blocks[0]
            .instructions
            .push(Instruction {
                id: InstructionId::new(0),
                results: vec![Value {
                    id: ValueId::new(0),
                    type_id: TypeId::new(1),
                }],
                kind: InstructionKind::StackAlloc {
                    size: 0,
                    alignment: 3,
                    address_space: AddressSpace::NearData,
                },
            });

        let diagnostics = verify(&module).expect_err("invalid stack allocation must be rejected");
        let messages = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();

        assert!(
            messages
                .iter()
                .any(|message| message.contains("stack allocation has zero size"))
        );
        assert!(messages.iter().any(|message| {
            message.contains("stack allocation alignment must be a nonzero power of two")
        }));
    }

    #[test]
    fn relocation_bounds_use_explicit_width_not_target_address_space() {
        let mut module = minimal_module();
        module.types.extend([
            Type {
                id: TypeId::new(1),
                kind: TypeKind::Integer { bits: 8 },
            },
            Type {
                id: TypeId::new(2),
                kind: TypeKind::Array {
                    element: TypeId::new(1),
                    length: 1,
                },
            },
        ]);
        module.globals.push(Global {
            id: GlobalId::new(0),
            name: "data".into(),
            type_id: TypeId::new(2),
            linkage: Linkage::Internal,
            constant: true,
            initializer: Some(Constant::RelocatableBytes {
                bytes: vec![0],
                relocations: vec![GlobalRelocation {
                    offset: 0,
                    target: GlobalId::new(0),
                    addend: 0,
                    width: 1,
                    address_space: AddressSpace::Generic,
                }],
            }),
        });

        assert!(verify(&module).is_ok());
    }

    #[test]
    fn rejects_invalid_relocatable_byte_patches() {
        let mut module = minimal_module();
        module.types.extend([
            Type {
                id: TypeId::new(1),
                kind: TypeKind::Integer { bits: 8 },
            },
            Type {
                id: TypeId::new(2),
                kind: TypeKind::Array {
                    element: TypeId::new(1),
                    length: 4,
                },
            },
        ]);
        module.globals.push(Global {
            id: GlobalId::new(0),
            name: "data".into(),
            type_id: TypeId::new(2),
            linkage: Linkage::Internal,
            constant: true,
            initializer: Some(Constant::RelocatableBytes {
                bytes: vec![0; 4],
                relocations: vec![
                    GlobalRelocation {
                        offset: 0,
                        target: GlobalId::new(9),
                        addend: 0,
                        width: 2,
                        address_space: AddressSpace::NearData,
                    },
                    GlobalRelocation {
                        offset: 3,
                        target: GlobalId::new(0),
                        addend: 0,
                        width: 4,
                        address_space: AddressSpace::FarData,
                    },
                    GlobalRelocation {
                        offset: 0,
                        target: GlobalId::new(0),
                        addend: 0,
                        width: 2,
                        address_space: AddressSpace::Segment,
                    },
                    GlobalRelocation {
                        offset: 2,
                        target: GlobalId::new(0),
                        addend: 0,
                        width: 0,
                        address_space: AddressSpace::Generic,
                    },
                ],
            }),
        });

        let diagnostics = verify(&module).expect_err("invalid relocation patches must be rejected");
        let messages = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();

        assert!(
            messages
                .iter()
                .any(|message| message.contains("relocation 0 target references unknown global 9"))
        );
        assert!(messages
            .iter()
            .any(|message| message.contains("relocation 1 patch range 3..7 is outside 4 bytes")));
        assert!(messages.iter().any(|message| {
            message.contains("relocation 2 patch range 0..2 overlaps relocation 0")
        }));
        assert!(
            messages
                .iter()
                .any(|message| { message.contains("relocation 3 has a zero-width patch") })
        );
    }
}
