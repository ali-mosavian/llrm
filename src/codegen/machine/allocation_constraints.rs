//! Splits fixed-register constraints into one-instruction virtual lifetimes.
//!
//! A fixed register is a property of an operand occurrence, not of the value's
//! whole live range.  The target supplies copies because only it knows the
//! opcode that moves one register class to another; this module decides where
//! those copies belong and never names a target opcode.
//!
//! This is occurrence splitting only, not complete parity with Python's
//! `constrain.py`: the caller must still decide which ABI-delivered values are
//! already valid whole-range pins and therefore do not need a split.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::{
    MachineBlockId, MachineFunction, MachineInstruction, MachineInstructionId, MachineOperandKind,
    MachineRegister, PhysicalRegister, RegisterClass, RegisterConstraint, VirtualRegister,
    VirtualRegisterId,
};

/// Target operation construction needed by fixed-register occurrence splitting.
pub trait ConstraintTarget {
    /// Builds an ordinary copy from `source` into `destination`.
    ///
    /// The target owns its opcode and any target-specific copy contract.  The
    /// splitter only relies on the read/write roles that the target puts on the
    /// two virtual-register operands.
    fn copy(
        &self,
        id: MachineInstructionId,
        destination: VirtualRegisterId,
        source: VirtualRegisterId,
    ) -> MachineInstruction;
}

/// A malformed fixed-register occurrence that prevents splitting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstraintError {
    MissingDeclaredClass {
        register: VirtualRegisterId,
    },
    ConflictingFixedRegisters {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        register: VirtualRegisterId,
        first: PhysicalRegister,
        second: PhysicalRegister,
    },
    VirtualRegisterIdExhausted,
    InstructionIdExhausted,
}

impl fmt::Display for ConstraintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDeclaredClass { register } => {
                write!(
                    formatter,
                    "virtual register {register} has no declared class"
                )
            }
            Self::ConflictingFixedRegisters {
                block,
                instruction,
                register,
                first,
                second,
            } => write!(
                formatter,
                "machine block {block} instruction {instruction} requires virtual register {register} in both physical registers {first} and {second}"
            ),
            Self::VirtualRegisterIdExhausted => {
                formatter.write_str("fixed-register splitting exhausted virtual-register IDs")
            }
            Self::InstructionIdExhausted => {
                formatter.write_str("fixed-register splitting exhausted instruction IDs")
            }
        }
    }
}

impl Error for ConstraintError {}

/// Splits every non-short fixed-register occurrence into a fresh short value.
///
/// A read gets `fresh <- original` immediately before its constrained
/// instruction; a write gets `original <- fresh` immediately after it.  All
/// occurrences of one original virtual within that instruction use the same
/// fresh virtual, preserving tied and repeated use/def operands.  The original
/// remains unconstrained at every other instruction.
pub fn split_fixed_occurrences<Target>(
    function: &MachineFunction,
    target: &Target,
) -> Result<MachineFunction, ConstraintError>
where
    Target: ConstraintTarget,
{
    let classes = declared_classes(function);
    validate_fixed_occurrences(function, &classes)?;
    let short = already_short(function);
    let mut fresh = FreshIds::new(function)?;
    let mut virtual_registers = function.virtual_registers.clone();
    let mut blocks = Vec::with_capacity(function.blocks.len());

    for block in &function.blocks {
        let mut instructions = Vec::new();
        for instruction in &block.instructions {
            let constrained = fixed_at(instruction);
            if constrained.is_empty() {
                instructions.push(instruction.clone());
                continue;
            }

            let mut replacements = BTreeMap::new();
            for (original, _) in constrained {
                if short.contains(&original) {
                    continue;
                }
                let class = classes
                    .get(&original)
                    .copied()
                    .ok_or(ConstraintError::MissingDeclaredClass { register: original })?;
                let replacement = fresh.virtual_register()?;
                virtual_registers.push(VirtualRegister {
                    id: replacement,
                    class,
                });
                replacements.insert(original, replacement);
            }
            if replacements.is_empty() {
                instructions.push(instruction.clone());
                continue;
            }

            let mut before = Vec::new();
            let mut after = Vec::new();
            for (original, replacement) in &replacements {
                let roles = roles_for(instruction, *original);
                if roles.reads {
                    before.push(target.copy(fresh.instruction()?, *replacement, *original));
                }
                if roles.writes {
                    after.push(target.copy(fresh.instruction()?, *original, *replacement));
                }
            }

            let mut rewritten = instruction.clone();
            for operand in &mut rewritten.operands {
                let MachineOperandKind::Register(MachineRegister::Virtual(register)) =
                    &operand.kind
                else {
                    continue;
                };
                if let Some(replacement) = replacements.get(register) {
                    operand.kind =
                        MachineOperandKind::Register(MachineRegister::Virtual(*replacement));
                }
            }
            instructions.extend(before);
            instructions.push(rewritten);
            instructions.extend(after);
        }
        blocks.push(super::MachineBlock {
            id: block.id,
            instructions,
            successors: block.successors.clone(),
        });
    }

    Ok(MachineFunction {
        id: function.id,
        name: function.name.clone(),
        linkage: function.linkage,
        signature: function.signature.clone(),
        entry: function.entry,
        virtual_registers,
        blocks,
        frame_objects: function.frame_objects.clone(),
    })
}

fn declared_classes(function: &MachineFunction) -> BTreeMap<VirtualRegisterId, RegisterClass> {
    function
        .virtual_registers
        .iter()
        .map(|register| (register.id, register.class))
        .collect()
}

/// Checks each instruction before constructing any output, preserving input on
/// every error path.
fn validate_fixed_occurrences(
    function: &MachineFunction,
    classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
) -> Result<(), ConstraintError> {
    for block in &function.blocks {
        for instruction in &block.instructions {
            let mut required = BTreeMap::new();
            for operand in &instruction.operands {
                let (
                    MachineOperandKind::Register(MachineRegister::Virtual(register)),
                    Some(RegisterConstraint::Fixed(physical)),
                ) = (&operand.kind, operand.constraint)
                else {
                    continue;
                };
                if !classes.contains_key(register) {
                    return Err(ConstraintError::MissingDeclaredClass {
                        register: *register,
                    });
                }
                if let Some(first) = required.insert(*register, physical) {
                    if first != physical {
                        return Err(ConstraintError::ConflictingFixedRegisters {
                            block: block.id,
                            instruction: instruction.id,
                            register: *register,
                            first,
                            second: physical,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn fixed_at(instruction: &MachineInstruction) -> BTreeMap<VirtualRegisterId, PhysicalRegister> {
    instruction
        .operands
        .iter()
        .filter_map(|operand| match (&operand.kind, operand.constraint) {
            (
                MachineOperandKind::Register(MachineRegister::Virtual(register)),
                Some(RegisterConstraint::Fixed(physical)),
            ) => Some((*register, physical)),
            _ => None,
        })
        .collect()
}

/// Values whose complete lifetime is one instruction already satisfy the rule
/// without copies.  This keeps synthetic call-clobber definitions short.
fn already_short(function: &MachineFunction) -> BTreeSet<VirtualRegisterId> {
    let mut sites =
        BTreeMap::<VirtualRegisterId, BTreeSet<(MachineBlockId, MachineInstructionId)>>::new();
    let mut fixed = BTreeMap::<VirtualRegisterId, BTreeSet<PhysicalRegister>>::new();
    for block in &function.blocks {
        for instruction in &block.instructions {
            for operand in &instruction.operands {
                let MachineOperandKind::Register(MachineRegister::Virtual(register)) =
                    &operand.kind
                else {
                    continue;
                };
                sites
                    .entry(*register)
                    .or_default()
                    .insert((block.id, instruction.id));
                if let Some(RegisterConstraint::Fixed(physical)) = operand.constraint {
                    fixed.entry(*register).or_default().insert(physical);
                }
            }
        }
    }
    fixed
        .into_iter()
        .filter_map(|(register, registers)| {
            (registers.len() == 1 && sites.get(&register).is_some_and(|sites| sites.len() == 1))
                .then_some(register)
        })
        .collect()
}

#[derive(Clone, Copy, Default)]
struct Roles {
    reads: bool,
    writes: bool,
}

fn roles_for(instruction: &MachineInstruction, register: VirtualRegisterId) -> Roles {
    instruction
        .operands
        .iter()
        .fold(Roles::default(), |mut roles, operand| {
            if matches!(
                &operand.kind,
                MachineOperandKind::Register(MachineRegister::Virtual(current)) if *current == register
            ) {
                roles.reads |= operand.role.reads();
                roles.writes |= operand.role.writes();
            }
            roles
        })
}

struct FreshIds {
    virtual_register: u32,
    instruction: u32,
}

impl FreshIds {
    fn new(function: &MachineFunction) -> Result<Self, ConstraintError> {
        let virtual_register = function
            .virtual_registers
            .iter()
            .map(|register| register.id.get())
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .flat_map(|instruction| {
                        instruction
                            .operands
                            .iter()
                            .filter_map(|operand| match &operand.kind {
                                MachineOperandKind::Register(MachineRegister::Virtual(
                                    register,
                                )) => Some(register.get()),
                                _ => None,
                            })
                    }),
            )
            .max()
            .map(|value| {
                value
                    .checked_add(1)
                    .ok_or(ConstraintError::VirtualRegisterIdExhausted)
            })
            .transpose()?
            .unwrap_or(0);
        let instruction = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .map(|instruction| instruction.id.get())
            .max()
            .map(|value| {
                value
                    .checked_add(1)
                    .ok_or(ConstraintError::InstructionIdExhausted)
            })
            .transpose()?
            .unwrap_or(0);
        Ok(Self {
            virtual_register,
            instruction,
        })
    }

    fn virtual_register(&mut self) -> Result<VirtualRegisterId, ConstraintError> {
        let id = VirtualRegisterId::new(self.virtual_register);
        self.virtual_register = self
            .virtual_register
            .checked_add(1)
            .ok_or(ConstraintError::VirtualRegisterIdExhausted)?;
        Ok(id)
    }

    fn instruction(&mut self) -> Result<MachineInstructionId, ConstraintError> {
        let id = MachineInstructionId::new(self.instruction);
        self.instruction = self
            .instruction
            .checked_add(1)
            .ok_or(ConstraintError::InstructionIdExhausted)?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::{ConstraintTarget, split_fixed_occurrences};
    use crate::codegen::machine::{
        InstructionFlags, MachineBlock, MachineBlockId, MachineCallingConvention, MachineFunction,
        MachineFunctionId, MachineInstruction, MachineInstructionId, MachineLinkage,
        MachineOperand, MachineOperandKind, MachineRegister, MachineSignature, OperandRole,
        PhysicalRegister, RegisterClass, RegisterConstraint, TargetOpcode, VirtualRegister,
        VirtualRegisterId,
    };

    const CLASS: RegisterClass = RegisterClass::new(1);
    const FIRST: PhysicalRegister = PhysicalRegister::new(1);
    const SECOND: PhysicalRegister = PhysicalRegister::new(2);
    const TEST_COPY: TargetOpcode = TargetOpcode::new(99);
    const TEST_OP: TargetOpcode = TargetOpcode::new(7);

    struct TestTarget;

    impl ConstraintTarget for TestTarget {
        fn copy(
            &self,
            id: MachineInstructionId,
            destination: VirtualRegisterId,
            source: VirtualRegisterId,
        ) -> MachineInstruction {
            MachineInstruction::new(
                id,
                TEST_COPY,
                vec![
                    virtual_operand(destination, OperandRole::Def),
                    virtual_operand(source, OperandRole::Use),
                ],
                InstructionFlags {
                    copy: true,
                    ..InstructionFlags::NONE
                },
            )
            .unwrap()
        }
    }

    fn virtual_operand(register: VirtualRegisterId, role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(register)),
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn fixed(register: u32, role: OperandRole, physical: PhysicalRegister) -> MachineOperand {
        MachineOperand {
            constraint: Some(RegisterConstraint::Fixed(physical)),
            ..virtual_operand(VirtualRegisterId::new(register), role)
        }
    }

    fn function(instructions: Vec<MachineInstruction>, registers: &[u32]) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(0),
            name: "one".into(),
            linkage: MachineLinkage::Internal,
            signature: MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::C,
            },
            entry: MachineBlockId::new(0),
            virtual_registers: registers
                .iter()
                .map(|id| VirtualRegister {
                    id: VirtualRegisterId::new(*id),
                    class: CLASS,
                })
                .collect(),
            blocks: vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions,
                successors: Vec::new(),
            }],
            frame_objects: Vec::new(),
        }
    }

    fn instruction(id: u32, operands: Vec<MachineOperand>) -> MachineInstruction {
        MachineInstruction::new(
            MachineInstructionId::new(id),
            TEST_OP,
            operands,
            InstructionFlags::NONE,
        )
        .unwrap()
    }

    fn register_at(instruction: &MachineInstruction, index: usize) -> VirtualRegisterId {
        match &instruction.operands[index].kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register)) => *register,
            _ => panic!("test operand must be virtual"),
        }
    }

    #[test]
    fn copies_a_required_source_before_its_fixed_occurrence() {
        // Ported from test_constrain.py::
        // test_a_required_source_is_copied_in_before_the_instruction: a
        // variable shift count must reach CL without pinning its whole range.
        let source = instruction(
            0,
            vec![virtual_operand(VirtualRegisterId::new(0), OperandRole::Def)],
        );
        let required = instruction(1, vec![fixed(0, OperandRole::Use, FIRST)]);
        let result =
            split_fixed_occurrences(&function(vec![source, required], &[0]), &TestTarget).unwrap();
        let instructions = &result.blocks[0].instructions;

        assert_eq!(instructions.len(), 3);
        assert_eq!(instructions[2].id, MachineInstructionId::new(1));
        assert_eq!(instructions[1].opcode, TEST_COPY);
        let fresh = register_at(&instructions[2], 0);
        assert_ne!(fresh, VirtualRegisterId::new(0));
        assert_eq!(register_at(&instructions[1], 0), fresh);
        assert_eq!(register_at(&instructions[1], 1), VirtualRegisterId::new(0));
        assert_eq!(
            instructions[2].operands[0].constraint,
            Some(RegisterConstraint::Fixed(FIRST))
        );
    }

    #[test]
    fn copies_a_required_destination_after_its_fixed_occurrence() {
        // Ported from test_constrain.py::
        // test_a_required_destination_is_copied_out_after_the_instruction:
        // CWD writes DX before the result returns to its ordinary range.
        let defined = instruction(0, vec![fixed(0, OperandRole::Def, FIRST)]);
        let use_after = instruction(
            1,
            vec![virtual_operand(VirtualRegisterId::new(0), OperandRole::Use)],
        );
        let result =
            split_fixed_occurrences(&function(vec![defined, use_after], &[0]), &TestTarget)
                .unwrap();
        let instructions = &result.blocks[0].instructions;

        assert_eq!(instructions.len(), 3);
        assert_eq!(instructions[0].id, MachineInstructionId::new(0));
        assert_eq!(instructions[1].opcode, TEST_COPY);
        let fresh = register_at(&instructions[0], 0);
        assert_eq!(register_at(&instructions[1], 0), VirtualRegisterId::new(0));
        assert_eq!(register_at(&instructions[1], 1), fresh);
        assert_eq!(register_at(&instructions[2], 0), VirtualRegisterId::new(0));
    }

    #[test]
    fn ties_and_repeated_occurrences_share_one_short_virtual() {
        // Ported from test_constrain.py::
        // test_a_tied_source_and_destination_share_one_fresh_value: IMUL's
        // low half is one value on both sides of the instruction.
        let source = instruction(
            0,
            vec![virtual_operand(VirtualRegisterId::new(0), OperandRole::Def)],
        );
        let mut use_operand = fixed(0, OperandRole::Use, FIRST);
        let mut define = fixed(0, OperandRole::Def, FIRST);
        define.tied_to = Some(crate::codegen::machine::OperandIndex::new(0));
        use_operand.tied_to = None;
        let tied = instruction(1, vec![use_operand, define]);
        let after = instruction(
            2,
            vec![virtual_operand(VirtualRegisterId::new(0), OperandRole::Use)],
        );
        let result =
            split_fixed_occurrences(&function(vec![source, tied, after], &[0]), &TestTarget)
                .unwrap();
        let instructions = &result.blocks[0].instructions;

        assert_eq!(instructions.len(), 5);
        let rewritten = &instructions[2];
        let fresh = register_at(rewritten, 0);
        assert_eq!(register_at(rewritten, 1), fresh);
        assert_eq!(register_at(&instructions[1], 0), fresh);
        assert_eq!(register_at(&instructions[3], 1), fresh);
    }

    #[test]
    fn refuses_conflicting_fixed_registers_without_mutating_input() {
        // Ported from test_constrain.py::
        // test_one_value_required_in_two_registers_is_refused: a shift count
        // and IMUL high half cannot demand different registers for one value.
        let original = function(
            vec![instruction(
                0,
                vec![
                    fixed(0, OperandRole::Use, FIRST),
                    fixed(0, OperandRole::Use, SECOND),
                ],
            )],
            &[0],
        );
        let error = split_fixed_occurrences(&original, &TestTarget).unwrap_err();

        assert!(matches!(
            error,
            super::ConstraintError::ConflictingFixedRegisters { .. }
        ));
        assert_eq!(original.blocks[0].instructions.len(), 1);
        assert_eq!(original.virtual_registers.len(), 1);
    }

    #[test]
    fn leaves_an_already_short_fixed_clobber_unchanged() {
        // Rust's synthetic far-call clobber is already a one-instruction
        // fixed definition, unlike a Python source value with a long range.
        // It needs no dead copy around its only occurrence.
        let original = function(
            vec![instruction(0, vec![fixed(0, OperandRole::Def, FIRST)])],
            &[0],
        );
        let result = split_fixed_occurrences(&original, &TestTarget).unwrap();

        assert_eq!(result, original);
    }
}
