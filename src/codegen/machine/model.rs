//! Target-independent machine intermediate representation.
//!
//! This model describes selected machine operations before register allocation
//! and encoding.  Target backends provide numeric opcodes, registers, and
//! register classes; this module deliberately assigns no architecture-specific
//! meaning to those numbers.

use std::error::Error;
use std::fmt;

pub use crate::support::PhysicalRegister;

macro_rules! entity_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

entity_id!(MachineFunctionId);
entity_id!(MachineDataObjectId);
entity_id!(MachineBlockId);
entity_id!(MachineInstructionId);
entity_id!(VirtualRegisterId);
entity_id!(FrameIndex);
entity_id!(TargetOpcode);
entity_id!(RegisterClass);

/// Position of an operand in one instruction.
///
/// It is distinct from all other IDs so a tie cannot accidentally be made to
/// an instruction, block, or virtual register.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperandIndex(u16);

impl OperandIndex {
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

/// An ordered collection of selected functions.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MachineModule {
    /// Defined data objects in deterministic source order.
    pub data_objects: Vec<MachineDataObject>,
    pub functions: Vec<MachineFunction>,
}

/// One defined data object with a byte initializer and symbolic patches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineDataObject {
    pub id: MachineDataObjectId,
    pub name: String,
    pub bytes: Vec<u8>,
    pub address_space: MachineAddressSpace,
    /// Relocations in increasing, non-overlapping offset order.
    pub relocations: Vec<MachineDataRelocation>,
    pub alignment: u32,
    pub constant: bool,
    pub linkage: MachineLinkage,
}

/// A target-independent symbolic patch in initialized machine data.
///
/// The address space and width retain the portable IR's layout intent.  A
/// target maps that intent to one of its fixup kinds when lowering to MC.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineDataRelocation {
    pub offset: u64,
    pub target: MachineDataObjectId,
    pub addend: i64,
    pub width: u8,
    pub address_space: MachineAddressSpace,
}

/// Visibility of a defined machine symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineLinkage {
    Internal,
    External,
}

/// One function after instruction selection and before allocation.
///
/// Blocks and frame objects retain their supplied order.  That makes the
/// representation deterministic without imposing a target-specific layout
/// policy here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineFunction {
    pub id: MachineFunctionId,
    pub name: String,
    pub linkage: MachineLinkage,
    pub signature: MachineSignature,
    /// The block where control enters this function.
    ///
    /// Every function must contain this block; consequently empty functions
    /// are invalid Machine IR.
    pub entry: MachineBlockId,
    pub virtual_registers: Vec<VirtualRegister>,
    pub blocks: Vec<MachineBlock>,
    pub frame_objects: Vec<FrameObject>,
}

/// Source-level ABI facts attached to a selected function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineSignature {
    pub result: Option<MachineValueType>,
    pub parameters: Vec<MachineValueType>,
    pub variadic: bool,
    pub calling_convention: MachineCallingConvention,
}

/// A language-neutral calling convention retained for target lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineCallingConvention {
    /// Near call with right-to-left arguments and caller stack cleanup.
    C,
    /// Far call with right-to-left arguments and caller stack cleanup.
    FarCdecl,
    /// Far call, left-to-right arguments, and callee stack cleanup.
    FarPascal,
}

/// A source-level value type, independent of target instruction encodings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineValueType {
    Integer {
        bits: u16,
    },
    Pointer {
        bits: u16,
        address_space: MachineAddressSpace,
    },
    Float {
        kind: MachineFloatKind,
    },
}

/// A portable binary floating-point format used by a machine ABI value.
///
/// This identifies only the value format. Register assignment, storage,
/// rounding, and target calling-convention details belong to later layers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineFloatKind {
    Binary32,
    Binary64,
    Extended80,
}

/// An abstract address space for machine-level ABI values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineAddressSpace {
    Generic,
    NearData,
    FarData,
    HugeData,
    Code,
    Segment,
}

/// One allocatable virtual register and its target-defined register class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualRegister {
    pub id: VirtualRegisterId,
    pub class: RegisterClass,
}

/// A basic block with its selected instructions and explicit outgoing edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineBlock {
    pub id: MachineBlockId,
    pub instructions: Vec<MachineInstruction>,
    pub successors: Vec<MachineBlockId>,
}

/// One stack-frame object.  Offset assignment belongs to frame layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameObject {
    pub index: FrameIndex,
    pub size: u32,
    pub alignment: u32,
    pub kind: FrameObjectKind,
}

/// Why a frame object exists, independent of target stack layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameObjectKind {
    Local,
    Spill,
    OutgoingArgument,
    IncomingArgument { parameter: u32 },
}

/// One target instruction and its target-independent operand contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineInstruction {
    pub id: MachineInstructionId,
    pub opcode: TargetOpcode,
    pub operands: Vec<MachineOperand>,
    pub flags: InstructionFlags,
}

impl MachineInstruction {
    /// Builds an instruction after checking local operand invariants.
    ///
    /// Ties model two-address relations.  They must connect register operands
    /// and one side must write while the other reads.  Register constraints
    /// are allocation requirements for virtual registers, so physical and
    /// non-register operands cannot carry one.
    pub fn new(
        id: MachineInstructionId,
        opcode: TargetOpcode,
        operands: Vec<MachineOperand>,
        flags: InstructionFlags,
    ) -> Result<Self, MachineInstructionError> {
        if operands.len() > usize::from(u16::MAX) {
            return Err(MachineInstructionError::TooManyOperands {
                count: operands.len(),
            });
        }

        for (position, operand) in operands.iter().enumerate() {
            let index = OperandIndex::new(position as u16);
            if operand.is_register() && matches!(operand.role, OperandRole::None) {
                return Err(MachineInstructionError::RegisterRequiresRole { index });
            }
            if !operand.is_register() && !matches!(operand.role, OperandRole::None) {
                return Err(MachineInstructionError::NonRegisterHasRole { index });
            }
            if operand.constraint.is_some() && !operand.is_virtual_register() {
                return Err(MachineInstructionError::ConstraintRequiresVirtualRegister { index });
            }

            let Some(tied_to) = operand.tied_to else {
                continue;
            };
            let target_position = usize::from(tied_to.get());
            let Some(target) = operands.get(target_position) else {
                return Err(MachineInstructionError::TieOutOfBounds { index, tied_to });
            };
            if index == tied_to {
                return Err(MachineInstructionError::SelfTie { index });
            }
            if !operand.is_register() || !target.is_register() {
                return Err(MachineInstructionError::TieRequiresRegisters { index, tied_to });
            }
            if !operand.role.writes() || !target.role.reads() {
                return Err(MachineInstructionError::TieRequiresDefAndUse { index, tied_to });
            }
        }

        Ok(Self {
            id,
            opcode,
            operands,
            flags,
        })
    }
}

/// A machine instruction operand and its allocation contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineOperand {
    pub kind: MachineOperandKind,
    pub role: OperandRole,
    pub constraint: Option<RegisterConstraint>,
    pub tied_to: Option<OperandIndex>,
}

impl MachineOperand {
    pub const fn is_register(&self) -> bool {
        matches!(self.kind, MachineOperandKind::Register(_))
    }

    pub const fn is_virtual_register(&self) -> bool {
        matches!(
            self.kind,
            MachineOperandKind::Register(MachineRegister::Virtual(_))
        )
    }
}

/// The value or reference carried by a machine operand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineOperandKind {
    Register(MachineRegister),
    Immediate(i64),
    FrameIndex { index: FrameIndex, addend: i64 },
    Block(MachineBlockId),
    Function(MachineFunctionId),
    Global { name: String, addend: i64 },
    ExternalSymbol { name: String, addend: i64 },
}

/// A register before or after allocation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MachineRegister {
    Virtual(VirtualRegisterId),
    Physical(PhysicalRegister),
}

/// Whether an operand reads, writes, or both reads and writes its value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperandRole {
    None,
    Use,
    Def,
    UseDef,
}

impl OperandRole {
    pub const fn reads(self) -> bool {
        matches!(self, Self::Use | Self::UseDef)
    }

    pub const fn writes(self) -> bool {
        matches!(self, Self::Def | Self::UseDef)
    }
}

/// A requirement which allocation must satisfy for a virtual register use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterConstraint {
    Fixed(PhysicalRegister),
    Class(RegisterClass),
}

/// Instruction properties needed by control-flow and allocation phases.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InstructionFlags {
    pub terminator: bool,
    pub call: bool,
    pub copy: bool,
    pub side_effects: bool,
    pub may_load: bool,
    pub may_store: bool,
    pub volatile: bool,
}

impl InstructionFlags {
    pub const NONE: Self = Self {
        terminator: false,
        call: false,
        copy: false,
        side_effects: false,
        may_load: false,
        may_store: false,
        volatile: false,
    };
}

/// A local operand-contract failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineInstructionError {
    TooManyOperands {
        count: usize,
    },
    ConstraintRequiresVirtualRegister {
        index: OperandIndex,
    },
    RegisterRequiresRole {
        index: OperandIndex,
    },
    NonRegisterHasRole {
        index: OperandIndex,
    },
    TieOutOfBounds {
        index: OperandIndex,
        tied_to: OperandIndex,
    },
    SelfTie {
        index: OperandIndex,
    },
    TieRequiresRegisters {
        index: OperandIndex,
        tied_to: OperandIndex,
    },
    TieRequiresDefAndUse {
        index: OperandIndex,
        tied_to: OperandIndex,
    },
}

impl fmt::Display for MachineInstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyOperands { count } => {
                write!(
                    formatter,
                    "instruction has {count} operands; maximum is 65535"
                )
            }
            Self::ConstraintRequiresVirtualRegister { index } => {
                write!(
                    formatter,
                    "operand {} has a constraint but is not virtual",
                    index.get()
                )
            }
            Self::RegisterRequiresRole { index } => {
                write!(formatter, "register operand {} has no role", index.get())
            }
            Self::NonRegisterHasRole { index } => write!(
                formatter,
                "non-register operand {} has a register role",
                index.get()
            ),
            Self::TieOutOfBounds { index, tied_to } => write!(
                formatter,
                "operand {} is tied to absent operand {}",
                index.get(),
                tied_to.get()
            ),
            Self::SelfTie { index } => {
                write!(formatter, "operand {} is tied to itself", index.get())
            }
            Self::TieRequiresRegisters { index, tied_to } => write!(
                formatter,
                "operand {} is tied to non-register operand {}",
                index.get(),
                tied_to.get()
            ),
            Self::TieRequiresDefAndUse { index, tied_to } => write!(
                formatter,
                "operand {} must define a value tied to use operand {}",
                index.get(),
                tied_to.get()
            ),
        }
    }
}

impl Error for MachineInstructionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_two_address_tied_virtual_registers() {
        let destination = MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(2))),
            role: OperandRole::Def,
            constraint: Some(RegisterConstraint::Class(RegisterClass::new(4))),
            tied_to: Some(OperandIndex::new(1)),
        };
        let source = MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(2))),
            role: OperandRole::Use,
            constraint: None,
            tied_to: None,
        };

        let instruction = MachineInstruction::new(
            MachineInstructionId::new(7),
            TargetOpcode::new(12),
            vec![destination, source],
            InstructionFlags::NONE,
        )
        .expect("a def tied to a register use is a valid two-address contract");

        assert_eq!(instruction.operands[0].tied_to, Some(OperandIndex::new(1)));
    }

    #[test]
    fn preserves_mixed_operands_and_symbol_addends() {
        let instruction = MachineInstruction::new(
            MachineInstructionId::new(3),
            TargetOpcode::new(8),
            vec![
                MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Physical(
                        PhysicalRegister::new(1),
                    )),
                    role: OperandRole::Use,
                    constraint: None,
                    tied_to: None,
                },
                MachineOperand {
                    kind: MachineOperandKind::Immediate(-12),
                    role: OperandRole::None,
                    constraint: None,
                    tied_to: None,
                },
                MachineOperand {
                    kind: MachineOperandKind::FrameIndex {
                        index: FrameIndex::new(0),
                        addend: 6,
                    },
                    role: OperandRole::None,
                    constraint: None,
                    tied_to: None,
                },
                MachineOperand {
                    kind: MachineOperandKind::Block(MachineBlockId::new(9)),
                    role: OperandRole::None,
                    constraint: None,
                    tied_to: None,
                },
                MachineOperand {
                    kind: MachineOperandKind::Global {
                        name: "table".to_owned(),
                        addend: 4,
                    },
                    role: OperandRole::None,
                    constraint: None,
                    tied_to: None,
                },
                MachineOperand {
                    kind: MachineOperandKind::ExternalSymbol {
                        name: "runtime".to_owned(),
                        addend: -2,
                    },
                    role: OperandRole::None,
                    constraint: None,
                    tied_to: None,
                },
            ],
            InstructionFlags {
                call: true,
                side_effects: true,
                ..InstructionFlags::NONE
            },
        )
        .expect("mixed non-constrained operands are valid");

        assert!(instruction.flags.call);
        assert_eq!(instruction.operands.len(), 6);
        assert!(matches!(
            instruction.operands[5].kind,
            MachineOperandKind::ExternalSymbol { addend: -2, .. }
        ));
    }

    #[test]
    fn typed_ids_keep_deterministic_ordering_distinct() {
        let function = MachineFunction {
            id: MachineFunctionId::new(1),
            name: "first".to_owned(),
            linkage: MachineLinkage::Internal,
            signature: MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::FarPascal,
            },
            entry: MachineBlockId::new(7),
            virtual_registers: vec![VirtualRegister {
                id: VirtualRegisterId::new(2),
                class: RegisterClass::new(4),
            }],
            blocks: vec![
                MachineBlock {
                    id: MachineBlockId::new(4),
                    instructions: Vec::new(),
                    successors: vec![MachineBlockId::new(7)],
                },
                MachineBlock {
                    id: MachineBlockId::new(7),
                    instructions: Vec::new(),
                    successors: Vec::new(),
                },
            ],
            frame_objects: vec![FrameObject {
                index: FrameIndex::new(0),
                size: 4,
                alignment: 2,
                kind: FrameObjectKind::Local,
            }],
        };

        let module = MachineModule {
            data_objects: Vec::new(),
            functions: vec![function],
        };
        assert_eq!(module.functions[0].blocks[0].id, MachineBlockId::new(4));
        assert_eq!(module.functions[0].entry, MachineBlockId::new(7));
        assert_eq!(
            module.functions[0].blocks[0].successors,
            vec![MachineBlockId::new(7)]
        );
        assert_ne!(
            MachineFunctionId::new(1).get(),
            MachineBlockId::new(4).get()
        );
    }
}
