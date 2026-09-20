//! Materialize the x86 BASIC far-call clobber contract before allocation.
//!
//! Values live across a call must not reuse an alias the callee clobbers.
//! In particular, an earlier B$EXSA lowering lost one half of a `LONG` by
//! treating its normal `AX:DX` result as ordinary call-clobbered state.

use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    MachineFunction, MachineInstruction, MachineInstructionId, MachineOperand, MachineOperandKind,
    MachineRegister, OperandRole, PhysicalRegister, RegisterConstraint, VirtualRegister,
    VirtualRegisterId,
};

use super::{X86Opcode, X86Register, X86RegisterClass};

const FAR_CALL_CLOBBERS: [X86Register; 6] = [
    X86Register::Ax,
    X86Register::Cx,
    X86Register::Dx,
    X86Register::Bx,
    X86Register::Si,
    X86Register::Di,
];

/// Failure while materializing target-owned far-call clobbers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallClobberError {
    /// A materialized definition would need a virtual-register ID above u32.
    IdExhausted,
    /// A fixed Machine IR register is not an x86 architectural register view.
    UnknownFixedRegister {
        instruction: MachineInstructionId,
        operand: usize,
        register: PhysicalRegister,
    },
}

impl fmt::Display for CallClobberError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdExhausted => write!(formatter, "virtual-register IDs are exhausted"),
            Self::UnknownFixedRegister {
                instruction,
                operand,
                register,
            } => write!(
                formatter,
                "instruction {instruction} operand {operand} fixes unknown x86 register {}",
                register.get()
            ),
        }
    }
}

impl Error for CallClobberError {}

/// Adds artificial fixed definitions for every implicit x86 far-call clobber.
///
/// The result is a fresh function.  All fixed register identities are checked
/// before the clone is changed, so an error leaves the input untouched.
pub fn materialize_far_call_clobbers(
    function: &MachineFunction,
) -> Result<MachineFunction, CallClobberError> {
    validate_fixed_registers(function)?;

    let mut next = next_virtual_register(function);
    let mut additions = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            if instruction.opcode != X86Opcode::CallFar.machine_opcode() {
                continue;
            }

            let clobbers = far_call_clobbers(instruction);
            let mut registers = Vec::new();
            for clobber in clobbers {
                if has_fixed_definition_for(instruction, clobber) {
                    continue;
                }
                let id = next.ok_or(CallClobberError::IdExhausted)?;
                next = id.get().checked_add(1).map(VirtualRegisterId::new);
                registers.push((id, clobber));
            }
            if !registers.is_empty() {
                additions.push((block_index, instruction_index, registers));
            }
        }
    }

    let mut result = function.clone();
    for (_, _, registers) in &additions {
        result
            .virtual_registers
            .extend(registers.iter().map(|(id, _)| VirtualRegister {
                id: *id,
                class: X86RegisterClass::Word.machine_class(),
            }));
    }
    for (block, instruction, registers) in additions {
        result.blocks[block].instructions[instruction]
            .operands
            .extend(
                registers
                    .into_iter()
                    .map(|(id, register)| fixed_definition(id, register)),
            );
    }
    Ok(result)
}

fn validate_fixed_registers(function: &MachineFunction) -> Result<(), CallClobberError> {
    for block in &function.blocks {
        for instruction in &block.instructions {
            for (operand, value) in instruction.operands.iter().enumerate() {
                let Some(RegisterConstraint::Fixed(register)) = value.constraint else {
                    continue;
                };
                if X86Register::from_physical(register).is_none() {
                    return Err(CallClobberError::UnknownFixedRegister {
                        instruction: instruction.id,
                        operand,
                        register,
                    });
                }
            }
        }
    }
    Ok(())
}

fn next_virtual_register(function: &MachineFunction) -> Option<VirtualRegisterId> {
    function
        .virtual_registers
        .iter()
        .map(|register| register.id)
        .chain(function.blocks.iter().flat_map(|block| {
            block.instructions.iter().flat_map(|instruction| {
                instruction
                    .operands
                    .iter()
                    .filter_map(|operand| match operand.kind {
                        MachineOperandKind::Register(MachineRegister::Virtual(id)) => Some(id),
                        _ => None,
                    })
            })
        }))
        .map(VirtualRegisterId::get)
        .max()
        .map(|id| id.checked_add(1).map(VirtualRegisterId::new))
        .unwrap_or(Some(VirtualRegisterId::new(0)))
}

fn far_call_clobbers(instruction: &MachineInstruction) -> impl Iterator<Item = X86Register> {
    let preserves_long_result = matches!(
        instruction.operands.first().map(|operand| &operand.kind),
        Some(MachineOperandKind::ExternalSymbol { name, .. }) if name == "B$EXSA"
    );
    FAR_CALL_CLOBBERS.into_iter().filter(move |register| {
        !preserves_long_result || !matches!(register, X86Register::Ax | X86Register::Dx)
    })
}

fn has_fixed_definition_for(instruction: &MachineInstruction, family: X86Register) -> bool {
    instruction.operands.iter().any(|operand| {
        if !operand.role.writes() {
            return false;
        }
        let Some(RegisterConstraint::Fixed(register)) = operand.constraint else {
            return false;
        };
        // `validate_fixed_registers` ran before planning, so this is an x86
        // register rather than a numeric guess.
        X86Register::from_physical(register)
            .expect("validated fixed x86 register")
            .overlaps(family)
    })
}

fn fixed_definition(id: VirtualRegisterId, register: X86Register) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Register(MachineRegister::Virtual(id)),
        role: OperandRole::Def,
        constraint: Some(RegisterConstraint::Fixed(register.physical())),
        tied_to: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        InstructionFlags, MachineBlock, MachineCallingConvention, MachineFunctionId,
        MachineLinkage, MachineSignature,
    };

    fn function(
        instruction: MachineInstruction,
        registers: Vec<VirtualRegister>,
    ) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(1),
            name: "procedure".into(),
            linkage: MachineLinkage::Internal,
            signature: MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::FarPascal,
            },
            entry: crate::codegen::machine::MachineBlockId::new(2),
            virtual_registers: registers,
            blocks: vec![MachineBlock {
                id: crate::codegen::machine::MachineBlockId::new(2),
                instructions: vec![instruction],
                successors: Vec::new(),
            }],
            frame_objects: Vec::new(),
        }
    }

    fn callee(name: &str) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::ExternalSymbol {
                name: name.into(),
                addend: 0,
            },
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        }
    }

    fn fixed(id: u32, role: OperandRole, register: X86Register) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(
                id,
            ))),
            role,
            constraint: Some(RegisterConstraint::Fixed(register.physical())),
            tied_to: None,
        }
    }

    fn word_registers(ids: &[u32]) -> Vec<VirtualRegister> {
        ids.iter()
            .map(|id| VirtualRegister {
                id: VirtualRegisterId::new(*id),
                class: X86RegisterClass::Word.machine_class(),
            })
            .collect()
    }

    fn far_call(operands: Vec<MachineOperand>) -> MachineInstruction {
        MachineInstruction::new(
            MachineInstructionId::new(3),
            X86Opcode::CallFar.machine_opcode(),
            operands,
            InstructionFlags {
                call: true,
                ..InstructionFlags::NONE
            },
        )
        .unwrap()
    }

    fn fixed_definitions(function: &MachineFunction) -> Vec<X86Register> {
        function.blocks[0].instructions[0]
            .operands
            .iter()
            .filter(|operand| operand.role.writes())
            .filter_map(|operand| match operand.constraint {
                Some(RegisterConstraint::Fixed(register)) => X86Register::from_physical(register),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn ordinary_far_call_gets_six_stable_clobbers() {
        let result =
            materialize_far_call_clobbers(&function(far_call(vec![callee("B$FOO")]), vec![]))
                .unwrap();
        assert_eq!(fixed_definitions(&result), FAR_CALL_CLOBBERS);
    }

    #[test]
    fn long_result_definitions_are_not_duplicated() {
        let registers = vec![
            VirtualRegister {
                id: VirtualRegisterId::new(9),
                class: X86RegisterClass::Word.machine_class(),
            },
            VirtualRegister {
                id: VirtualRegisterId::new(10),
                class: X86RegisterClass::Word.machine_class(),
            },
        ];
        let result = materialize_far_call_clobbers(&function(
            far_call(vec![
                callee("B$FOO"),
                fixed(9, OperandRole::Def, X86Register::Ax),
                fixed(10, OperandRole::Def, X86Register::Dx),
            ]),
            registers,
        ))
        .unwrap();
        assert_eq!(
            fixed_definitions(&result),
            vec![
                X86Register::Ax,
                X86Register::Dx,
                X86Register::Cx,
                X86Register::Bx,
                X86Register::Si,
                X86Register::Di
            ]
        );
    }

    #[test]
    fn benra_entry_values_do_not_suppress_clobber_definitions() {
        let result = materialize_far_call_clobbers(&function(
            far_call(vec![
                callee("B$ENRA"),
                fixed(1, OperandRole::Use, X86Register::Cx),
                fixed(2, OperandRole::Use, X86Register::Bx),
            ]),
            word_registers(&[1, 2]),
        ))
        .unwrap();
        assert_eq!(fixed_definitions(&result), FAR_CALL_CLOBBERS);
    }

    #[test]
    fn bexsa_preserves_ax_and_dx_long_result_values() {
        let result = materialize_far_call_clobbers(&function(
            far_call(vec![
                callee("B$EXSA"),
                fixed(1, OperandRole::Use, X86Register::Ax),
                fixed(2, OperandRole::Use, X86Register::Dx),
            ]),
            word_registers(&[1, 2]),
        ))
        .unwrap();
        assert_eq!(
            fixed_definitions(&result),
            vec![
                X86Register::Cx,
                X86Register::Bx,
                X86Register::Si,
                X86Register::Di
            ]
        );
    }

    #[test]
    fn materialization_is_idempotent_and_leaves_input_unchanged() {
        let input = function(far_call(vec![callee("B$FOO")]), vec![]);
        let baseline = input.clone();
        let first = materialize_far_call_clobbers(&input).unwrap();
        assert_eq!(input, baseline);
        assert_eq!(materialize_far_call_clobbers(&first).unwrap(), first);
    }

    #[test]
    fn exhaustion_and_unknown_fixed_register_are_explicit() {
        let exhausted = function(
            far_call(vec![callee("B$FOO")]),
            vec![VirtualRegister {
                id: VirtualRegisterId::new(u32::MAX),
                class: X86RegisterClass::Word.machine_class(),
            }],
        );
        assert_eq!(
            materialize_far_call_clobbers(&exhausted),
            Err(CallClobberError::IdExhausted)
        );

        let unknown = MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(0))),
            role: OperandRole::Def,
            constraint: Some(RegisterConstraint::Fixed(PhysicalRegister::new(99))),
            tied_to: None,
        };
        assert!(matches!(
            materialize_far_call_clobbers(&function(
                far_call(vec![callee("B$FOO"), unknown]),
                vec![]
            )),
            Err(CallClobberError::UnknownFixedRegister { .. })
        ));
    }
}
