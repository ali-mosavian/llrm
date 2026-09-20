//! Structural verification for target-independent Machine IR.
//!
//! This verifier checks only facts carried by Machine IR itself.  Target
//! opcode legality, register-class meaning, ABI requirements, and frame
//! layout are deliberately left to their owning layers.

use std::collections::BTreeSet;

use crate::support::diagnostic::{Diagnostic, Severity};

use super::{
    FrameIndex, MachineBlock, MachineBlockId, MachineFunction, MachineFunctionId,
    MachineInstruction, MachineModule, MachineOperandKind, MachineRegister, OperandRole,
    VirtualRegisterId,
};

/// Validates the structural invariants of a Machine IR module.
///
/// The returned diagnostics are deterministic: functions, blocks, and
/// instructions are inspected in their stored order, while declared IDs are
/// looked up in ordered sets.  Verification never mutates Machine IR or
/// applies target-specific policy.
pub fn verify(module: &MachineModule) -> Result<(), Vec<Diagnostic>> {
    let mut verifier = Verifier::new(module);
    verifier.verify_module();
    if verifier.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(verifier.diagnostics)
    }
}

impl MachineModule {
    /// Validates this module's target-independent Machine IR invariants.
    pub fn verify(&self) -> Result<(), Vec<Diagnostic>> {
        verify(self)
    }
}

struct Verifier<'module> {
    module: &'module MachineModule,
    function_ids: BTreeSet<MachineFunctionId>,
    diagnostics: Vec<Diagnostic>,
}

struct FunctionEntities {
    virtual_register_ids: BTreeSet<VirtualRegisterId>,
    frame_indices: BTreeSet<FrameIndex>,
    block_ids: BTreeSet<MachineBlockId>,
}

impl<'module> Verifier<'module> {
    fn new(module: &'module MachineModule) -> Self {
        Self {
            module,
            function_ids: BTreeSet::new(),
            diagnostics: Vec::new(),
        }
    }

    fn verify_module(&mut self) {
        for function in &self.module.functions {
            if !self.function_ids.insert(function.id) {
                self.error(format!("duplicate machine function id {}", function.id));
            }
        }

        for function in &self.module.functions {
            self.verify_function(function);
        }
    }

    fn verify_function(&mut self, function: &MachineFunction) {
        let entities = FunctionEntities {
            virtual_register_ids: self.collect_virtual_register_ids(function),
            frame_indices: self.collect_frame_indices(function),
            block_ids: self.collect_block_ids(function),
        };
        self.verify_instruction_ids(function);

        self.verify_frame_objects(function);
        for block in &function.blocks {
            self.verify_block(function, block, &entities);
        }
    }

    fn collect_virtual_register_ids(
        &mut self,
        function: &MachineFunction,
    ) -> BTreeSet<VirtualRegisterId> {
        let mut ids = BTreeSet::new();
        for virtual_register in &function.virtual_registers {
            if !ids.insert(virtual_register.id) {
                self.error(format!(
                    "machine function {} has duplicate virtual register id {}",
                    function.id, virtual_register.id
                ));
            }
        }
        ids
    }

    fn collect_frame_indices(&mut self, function: &MachineFunction) -> BTreeSet<FrameIndex> {
        let mut indices = BTreeSet::new();
        for frame_object in &function.frame_objects {
            if !indices.insert(frame_object.index) {
                self.error(format!(
                    "machine function {} has duplicate frame index {}",
                    function.id, frame_object.index
                ));
            }
        }
        indices
    }

    fn collect_block_ids(&mut self, function: &MachineFunction) -> BTreeSet<MachineBlockId> {
        let mut ids = BTreeSet::new();
        for block in &function.blocks {
            if !ids.insert(block.id) {
                self.error(format!(
                    "machine function {} has duplicate machine block id {}",
                    function.id, block.id
                ));
            }
        }
        ids
    }

    fn verify_instruction_ids(&mut self, function: &MachineFunction) {
        let mut ids = BTreeSet::new();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if !ids.insert(instruction.id) {
                    self.error(format!(
                        "machine function {} has duplicate machine instruction id {}",
                        function.id, instruction.id
                    ));
                }
            }
        }
    }

    fn verify_frame_objects(&mut self, function: &MachineFunction) {
        for frame_object in &function.frame_objects {
            if frame_object.size == 0 {
                self.error(format!(
                    "machine function {} frame index {} has zero size",
                    function.id, frame_object.index
                ));
            }
            if frame_object.alignment == 0 || !frame_object.alignment.is_power_of_two() {
                self.error(format!(
                    "machine function {} frame index {} has invalid alignment {}",
                    function.id, frame_object.index, frame_object.alignment
                ));
            }
        }
    }

    fn verify_block(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        entities: &FunctionEntities,
    ) {
        let mut successor_ids = BTreeSet::new();
        for successor in &block.successors {
            if !successor_ids.insert(*successor) {
                self.error(format!(
                    "machine function {} block {} has duplicate successor {}",
                    function.id, block.id, successor
                ));
            }
            if !entities.block_ids.contains(successor) {
                self.error(format!(
                    "machine function {} block {} has unknown successor {}",
                    function.id, block.id, successor
                ));
            }
        }

        for (position, instruction) in block.instructions.iter().enumerate() {
            if instruction.flags.terminator && position + 1 != block.instructions.len() {
                self.error(format!(
                    "machine function {} block {} has terminator instruction {} before the end of the block",
                    function.id, block.id, instruction.id
                ));
            }
            self.verify_instruction(function, block, instruction, entities);
        }
    }

    fn verify_instruction(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        entities: &FunctionEntities,
    ) {
        if instruction.operands.len() > usize::from(u16::MAX) {
            self.error(format!(
                "machine function {} block {} instruction {} has {} operands; maximum is 65535",
                function.id,
                block.id,
                instruction.id,
                instruction.operands.len()
            ));
        }

        if instruction.flags.volatile && !instruction.flags.may_load && !instruction.flags.may_store
        {
            self.error(format!(
                "machine function {} block {} instruction {} is volatile but neither loads nor stores",
                function.id, block.id, instruction.id
            ));
        }

        for position in 0..instruction.operands.len() {
            self.verify_operand(function, block, instruction, position, entities);
        }
        self.verify_ties(function, block, instruction);
        self.verify_copy(function, block, instruction);
    }

    fn verify_operand(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        entities: &FunctionEntities,
    ) {
        let operand = &instruction.operands[position];
        match &operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(id)) => {
                if !entities.virtual_register_ids.contains(id) {
                    self.error(format!(
                        "machine function {} block {} instruction {} operand {} references unknown virtual register {}",
                        function.id, block.id, instruction.id, position, id
                    ));
                }
                if matches!(operand.role, OperandRole::None) {
                    self.error(format!(
                        "machine function {} block {} instruction {} register operand {} has no role",
                        function.id, block.id, instruction.id, position
                    ));
                }
            }
            MachineOperandKind::Register(MachineRegister::Physical(_)) => {
                if matches!(operand.role, OperandRole::None) {
                    self.error(format!(
                        "machine function {} block {} instruction {} register operand {} has no role",
                        function.id, block.id, instruction.id, position
                    ));
                }
            }
            MachineOperandKind::FrameIndex { index, .. } => {
                if !entities.frame_indices.contains(index) {
                    self.error(format!(
                        "machine function {} block {} instruction {} operand {} references unknown frame index {}",
                        function.id, block.id, instruction.id, position, index
                    ));
                }
                self.verify_non_register_role(function, block, instruction, position, operand.role);
            }
            MachineOperandKind::Block(target) => {
                if !entities.block_ids.contains(target) {
                    self.error(format!(
                        "machine function {} block {} instruction {} operand {} references unknown block {}",
                        function.id, block.id, instruction.id, position, target
                    ));
                }
                self.verify_non_register_role(function, block, instruction, position, operand.role);
            }
            MachineOperandKind::Immediate(_)
            | MachineOperandKind::Global { .. }
            | MachineOperandKind::ExternalSymbol { .. } => {
                self.verify_non_register_role(function, block, instruction, position, operand.role);
            }
        }

        if operand.constraint.is_some()
            && !matches!(
                operand.kind,
                MachineOperandKind::Register(MachineRegister::Virtual(_))
            )
        {
            self.error(format!(
                "machine function {} block {} instruction {} operand {} constrains a non-virtual register",
                function.id, block.id, instruction.id, position
            ));
        }
    }

    fn verify_non_register_role(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        role: OperandRole,
    ) {
        if !matches!(role, OperandRole::None) {
            self.error(format!(
                "machine function {} block {} instruction {} non-register operand {} has a register role",
                function.id, block.id, instruction.id, position
            ));
        }
    }

    fn verify_ties(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        for (position, operand) in instruction.operands.iter().enumerate() {
            let Some(tied_to) = operand.tied_to else {
                continue;
            };
            let target_position = usize::from(tied_to.get());
            let Some(target) = instruction.operands.get(target_position) else {
                self.error(format!(
                    "machine function {} block {} instruction {} operand {} is tied to absent operand {}",
                    function.id, block.id, instruction.id, position, target_position
                ));
                continue;
            };
            if target_position == position {
                self.error(format!(
                    "machine function {} block {} instruction {} operand {} is tied to itself",
                    function.id, block.id, instruction.id, position
                ));
            }
            if !is_register(&operand.kind) || !is_register(&target.kind) {
                self.error(format!(
                    "machine function {} block {} instruction {} operand {} is tied to non-register operand {}",
                    function.id, block.id, instruction.id, position, target_position
                ));
            }
            if !operand.role.writes() || !target.role.reads() {
                self.error(format!(
                    "machine function {} block {} instruction {} operand {} must define a value tied to use operand {}",
                    function.id, block.id, instruction.id, position, target_position
                ));
            }
        }
    }

    fn verify_copy(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        if !instruction.flags.copy {
            return;
        }

        let valid_copy = matches!(
            instruction.operands.as_slice(),
            [
                first @ super::MachineOperand {
                    kind: MachineOperandKind::Register(_),
                    ..
                },
                second @ super::MachineOperand {
                    kind: MachineOperandKind::Register(_),
                    ..
                },
            ] if (matches!(first.role, OperandRole::Def) && matches!(second.role, OperandRole::Use))
                || (matches!(first.role, OperandRole::Use) && matches!(second.role, OperandRole::Def))
        );
        if !valid_copy {
            self.error(format!(
                "machine function {} block {} instruction {} is marked copy but does not have exactly one register def and one register use",
                function.id, block.id, instruction.id
            ));
        }
    }

    fn error(&mut self, message: String) {
        self.diagnostics
            .push(Diagnostic::new(Severity::Error, message));
    }
}

fn is_register(kind: &MachineOperandKind) -> bool {
    matches!(kind, MachineOperandKind::Register(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        FrameObject, FrameObjectKind, InstructionFlags, MachineBlock, MachineFunction,
        MachineInstruction, MachineInstructionId, MachineModule, MachineOperand,
        MachineOperandKind, MachineRegister, OperandIndex, PhysicalRegister, RegisterClass,
        RegisterConstraint, TargetOpcode, VirtualRegister,
    };

    fn register(id: u32, role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(
                id,
            ))),
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn instruction(id: u32, operands: Vec<MachineOperand>) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(id),
            opcode: TargetOpcode::new(1),
            operands,
            flags: InstructionFlags::NONE,
        }
    }

    fn valid_function() -> MachineFunction {
        let mut destination = register(0, OperandRole::Def);
        destination.constraint = Some(RegisterConstraint::Class(RegisterClass::new(0)));
        destination.tied_to = Some(OperandIndex::new(1));
        let source = register(0, OperandRole::Use);

        MachineFunction {
            id: MachineFunctionId::new(0),
            name: "two_address".to_owned(),
            virtual_registers: vec![VirtualRegister {
                id: VirtualRegisterId::new(0),
                class: RegisterClass::new(0),
            }],
            blocks: vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(0, vec![destination, source])],
                successors: Vec::new(),
            }],
            frame_objects: vec![FrameObject {
                index: FrameIndex::new(0),
                size: 4,
                alignment: 4,
                kind: FrameObjectKind::Local,
            }],
        }
    }

    fn messages(module: MachineModule) -> Vec<String> {
        verify(&module)
            .expect_err("constructed module should violate the selected invariant")
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect()
    }

    #[test]
    fn accepts_a_valid_two_address_function() {
        verify(&MachineModule {
            functions: vec![valid_function()],
        })
        .expect("a valid target-independent two-address function must verify");
    }

    #[test]
    fn rejects_duplicate_ids_at_each_machine_scope() {
        let mut function = valid_function();
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId::new(0),
            class: RegisterClass::new(1),
        });
        function.frame_objects.push(FrameObject {
            index: FrameIndex::new(0),
            size: 1,
            alignment: 1,
            kind: FrameObjectKind::Spill,
        });
        function.blocks.push(MachineBlock {
            id: MachineBlockId::new(0),
            instructions: vec![instruction(0, Vec::new())],
            successors: Vec::new(),
        });
        let mut second = valid_function();
        second.id = MachineFunctionId::new(0);

        let diagnostic_messages = messages(MachineModule {
            functions: vec![function, second],
        });
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("duplicate machine function id 0"))
        );
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("duplicate virtual register id 0"))
        );
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("duplicate frame index 0"))
        );
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("duplicate machine block id 0"))
        );
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("duplicate machine instruction id 0"))
        );
    }

    #[test]
    fn rejects_unknown_and_repeated_successors() {
        let mut function = valid_function();
        function.blocks[0].successors = vec![MachineBlockId::new(8), MachineBlockId::new(8)];

        let diagnostic_messages = messages(MachineModule {
            functions: vec![function],
        });
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("unknown successor 8"))
        );
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("duplicate successor 8"))
        );
    }

    #[test]
    fn rejects_unknown_virtual_registers_and_frame_indices() {
        let mut function = valid_function();
        function.blocks[0].instructions[0].operands = vec![
            register(8, OperandRole::Use),
            MachineOperand {
                kind: MachineOperandKind::FrameIndex {
                    index: FrameIndex::new(5),
                    addend: 0,
                },
                role: OperandRole::None,
                constraint: None,
                tied_to: None,
            },
        ];

        let diagnostic_messages = messages(MachineModule {
            functions: vec![function],
        });
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("unknown virtual register 8"))
        );
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("unknown frame index 5"))
        );
    }

    #[test]
    fn rejects_invalid_frame_layout_facts() {
        let mut function = valid_function();
        function.frame_objects[0].size = 0;
        function.frame_objects[0].alignment = 3;

        let diagnostic_messages = messages(MachineModule {
            functions: vec![function],
        });
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("has zero size"))
        );
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("invalid alignment 3"))
        );
    }

    #[test]
    fn rejects_constraints_on_non_virtual_operands() {
        let mut function = valid_function();
        function.blocks[0].instructions[0].operands[0].kind =
            MachineOperandKind::Register(MachineRegister::Physical(PhysicalRegister::new(1)));

        let diagnostic_messages = messages(MachineModule {
            functions: vec![function],
        });
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("constrains a non-virtual register"))
        );
    }

    #[test]
    fn rejects_malformed_ties() {
        let mut function = valid_function();
        function.blocks[0].instructions[0].operands[0].tied_to = Some(OperandIndex::new(3));
        function.blocks[0].instructions[0].operands[1].tied_to = Some(OperandIndex::new(1));

        let diagnostic_messages = messages(MachineModule {
            functions: vec![function],
        });
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("tied to absent operand 3"))
        );
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("is tied to itself"))
        );
    }

    #[test]
    fn rejects_ties_without_a_register_def_and_use() {
        let mut function = valid_function();
        function.blocks[0].instructions[0].operands[0].kind = MachineOperandKind::Immediate(1);
        function.blocks[0].instructions[0].operands[0].role = OperandRole::None;

        let diagnostic_messages = messages(MachineModule {
            functions: vec![function],
        });
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("tied to non-register operand 1"))
        );
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("must define a value tied to use operand 1"))
        );
    }

    #[test]
    fn rejects_terminators_before_following_instructions() {
        let mut function = valid_function();
        function.blocks[0].instructions[0].flags.terminator = true;
        function.blocks[0]
            .instructions
            .push(instruction(1, Vec::new()));

        let diagnostic_messages = messages(MachineModule {
            functions: vec![function],
        });
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("before the end of the block"))
        );
    }

    #[test]
    fn rejects_volatile_instructions_without_memory_effects() {
        let mut function = valid_function();
        function.blocks[0].instructions[0].flags.volatile = true;

        let diagnostic_messages = messages(MachineModule {
            functions: vec![function],
        });
        assert!(
            diagnostic_messages
                .iter()
                .any(|message| message.contains("volatile but neither loads nor stores"))
        );
    }

    #[test]
    fn rejects_copy_without_exactly_one_register_def_and_use() {
        let mut function = valid_function();
        function.blocks[0].instructions[0].flags.copy = true;
        function.blocks[0].instructions[0]
            .operands
            .push(MachineOperand {
                kind: MachineOperandKind::Immediate(0),
                role: OperandRole::None,
                constraint: None,
                tied_to: None,
            });

        let diagnostic_messages = messages(MachineModule {
            functions: vec![function],
        });
        assert!(diagnostic_messages.iter().any(|message| message.contains(
            "marked copy but does not have exactly one register def and one register use"
        )));
    }
}
