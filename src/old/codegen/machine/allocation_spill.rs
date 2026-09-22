//! Target-independent baseline spill materialization around register assignment.
//!
//! A failed colour is only an allocation decision until the value is made to
//! live in a frame object.  This conservative baseline gives every spilled
//! definition a store and every spilled use a reload into one fresh, short
//! value per instruction.  The target owns the slot width and the selected
//! load/store instructions; this module owns neither of those facts.
//!
//! This is deliberately not yet a complete port of Python's
//! `RegAlloc.transform` and `spiller.spilled`.  Rematerialization, direct
//! memory-source folding, tied and grouped-operation handling, slot coloring,
//! range splitting, and Python's weighted victim policy remain deferred.  The
//! explicit, conservative behavior here is preferable to silently claiming
//! any of those richer mechanisms.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::{
    FrameIndex, FrameObject, MachineBlockId, MachineFunction, MachineInstruction,
    MachineInstructionId, MachineOperandKind, MachineRegister, RegisterClass, RegisterConstraint,
    VirtualRegister, VirtualRegisterId,
};

/// Target facts needed to turn explicit generic spill materialization into Machine IR.
///
/// The materializer never names an opcode, a register width, or a concrete
/// stack displacement.  The target supplies abstract frame-index loads and
/// stores, which are materialized after allocation and frame planning.
pub trait SpillTarget {
    type Error;

    /// Creates one target-sized stack object for a value of `class`.
    fn spill_frame_object(
        &self,
        index: FrameIndex,
        class: RegisterClass,
    ) -> Result<FrameObject, Self::Error>;

    /// Loads a spill slot into one fresh virtual register.
    fn spill_load(
        &self,
        id: MachineInstructionId,
        destination: VirtualRegisterId,
        frame: FrameIndex,
    ) -> MachineInstruction;

    /// Stores one fresh virtual register to a spill slot.
    fn spill_store(
        &self,
        id: MachineInstructionId,
        source: VirtualRegisterId,
        frame: FrameIndex,
    ) -> MachineInstruction;
}

/// A function after conservative materialization of an explicit spill set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpillMaterialization {
    pub function: MachineFunction,
    /// Fresh reload/store virtuals that a later allocator must not spill.
    pub unspillable: BTreeSet<VirtualRegisterId>,
}

/// A refusal while materializing explicit Machine-IR spills.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpillMaterializationError<TargetError> {
    Target(TargetError),
    MissingDeclaredClass {
        register: VirtualRegisterId,
    },
    FixedConstraint {
        register: VirtualRegisterId,
    },
    TiedOperand {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        register: VirtualRegisterId,
    },
    VirtualRegisterIdExhausted,
    FrameIndexExhausted,
    InstructionIdExhausted,
    InvalidSpillFrameIndex {
        expected: FrameIndex,
        actual: FrameIndex,
    },
}

impl<TargetError: fmt::Display> fmt::Display for SpillMaterializationError<TargetError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target(error) => error.fmt(formatter),
            Self::MissingDeclaredClass { register } => {
                write!(
                    formatter,
                    "virtual register {register} has no declared class"
                )
            }
            Self::FixedConstraint { register } => write!(
                formatter,
                "cannot conservatively spill virtual register {register} with a fixed constraint"
            ),
            Self::TiedOperand {
                block,
                instruction,
                register,
            } => write!(
                formatter,
                "cannot conservatively spill tied virtual register {register} in machine block {block} instruction {instruction}"
            ),
            Self::VirtualRegisterIdExhausted => {
                formatter.write_str("spill rewriting exhausted virtual-register IDs")
            }
            Self::FrameIndexExhausted => {
                formatter.write_str("spill rewriting exhausted frame indices")
            }
            Self::InstructionIdExhausted => {
                formatter.write_str("spill rewriting exhausted instruction IDs")
            }
            Self::InvalidSpillFrameIndex { expected, actual } => write!(
                formatter,
                "target spill frame object has index {actual}, expected fresh index {expected}"
            ),
        }
    }
}

impl<TargetError: Error + 'static> Error for SpillMaterializationError<TargetError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Target(error) => Some(error),
            Self::MissingDeclaredClass { .. }
            | Self::FixedConstraint { .. }
            | Self::TiedOperand { .. }
            | Self::VirtualRegisterIdExhausted
            | Self::FrameIndexExhausted
            | Self::InstructionIdExhausted
            | Self::InvalidSpillFrameIndex { .. } => None,
        }
    }
}

/// Materializes an explicit set of values into target-owned stack objects.
///
/// One original value has one fresh replacement per instruction, even when it
/// appears in several operands or as a use and definition.  The returned
/// replacement values are deliberately unspillable: recursively spilling a
/// one-instruction load/store range cannot converge.  Choosing the spill set,
/// retrying allocation, splitting, and eviction policy belong to a future
/// faithful allocator port, not this materialization boundary.
pub fn materialize_spills<Target>(
    function: &MachineFunction,
    spills: &[VirtualRegisterId],
    target: &Target,
) -> Result<SpillMaterialization, SpillMaterializationError<Target::Error>>
where
    Target: SpillTarget,
{
    let spills = spills.iter().copied().collect::<BTreeSet<_>>();
    if spills.is_empty() {
        return Ok(SpillMaterialization {
            function: function.clone(),
            unspillable: BTreeSet::new(),
        });
    }
    let classes = function
        .virtual_registers
        .iter()
        .map(|register| (register.id, register.class))
        .collect::<BTreeMap<_, _>>();
    for spilled in &spills {
        if !classes.contains_key(spilled) {
            return Err(SpillMaterializationError::MissingDeclaredClass { register: *spilled });
        }
    }
    preflight_spills(function, &spills)?;

    let mut fresh = FreshIds::new(function)?;
    let mut frames = BTreeMap::new();
    let mut frame_objects = Vec::new();
    for spilled in &spills {
        let frame = fresh.frame()?;
        let frame_object = target
            .spill_frame_object(frame, classes[spilled])
            .map_err(SpillMaterializationError::Target)?;
        if frame_object.index != frame {
            return Err(SpillMaterializationError::InvalidSpillFrameIndex {
                expected: frame,
                actual: frame_object.index,
            });
        }
        frames.insert(*spilled, frame);
        frame_objects.push(frame_object);
    }

    let mut rewritten = function.clone();
    rewritten.frame_objects.extend(frame_objects);
    rewritten
        .virtual_registers
        .retain(|register| !spills.contains(&register.id));
    let mut unspillable = BTreeSet::new();

    for block in &mut rewritten.blocks {
        let mut instructions = Vec::new();
        for mut instruction in std::mem::take(&mut block.instructions) {
            let mut replacements = BTreeMap::<VirtualRegisterId, Replacement>::new();
            let mut read_order = Vec::new();
            let mut write_order = Vec::new();
            for operand in &mut instruction.operands {
                let MachineOperandKind::Register(MachineRegister::Virtual(register)) = operand.kind
                else {
                    continue;
                };
                let Some(frame) = frames.get(&register).copied() else {
                    continue;
                };

                let replacement = match replacements.get(&register).copied() {
                    Some(replacement) => replacement.register,
                    None => {
                        let replacement = fresh.virtual_register()?;
                        rewritten.virtual_registers.push(VirtualRegister {
                            id: replacement,
                            class: classes[&register],
                        });
                        replacements.insert(register, Replacement::new(replacement, frame));
                        replacement
                    }
                };
                // Preserve role, fixed/class constraint, and any tie on this
                // occurrence.  Every occurrence of this spilled value uses
                // the same short range, matching Python's per-instruction
                // rename map and avoiding duplicate reload pressure.
                operand.kind = MachineOperandKind::Register(MachineRegister::Virtual(replacement));
                let state = replacements
                    .get_mut(&register)
                    .expect("the replacement was inserted above");
                if operand.role.reads() && !state.reads {
                    state.reads = true;
                    read_order.push(register);
                }
                if operand.role.writes() && !state.writes {
                    state.writes = true;
                    write_order.push(register);
                }
            }
            for register in read_order {
                let replacement = &replacements[&register];
                // A fresh range created around either side of one instruction
                // must get a real register.  In particular, recursively
                // spilling a reload puts a load in front of a load forever.
                unspillable.insert(replacement.register);
                let mut reload = target.spill_load(
                    fresh.instruction()?,
                    replacement.register,
                    replacement.frame,
                );
                // This fact belongs to the allocator, not to the target
                // opcode: an ordinary selected load is not deletable merely
                // because it has the same selected shape.
                reload.flags.spill_reload = true;
                reload.flags.spill_store = false;
                instructions.push(reload);
            }
            instructions.push(instruction);
            for register in write_order {
                let replacement = &replacements[&register];
                unspillable.insert(replacement.register);
                let mut store = target.spill_store(
                    fresh.instruction()?,
                    replacement.register,
                    replacement.frame,
                );
                store.flags.spill_reload = false;
                store.flags.spill_store = true;
                instructions.push(store);
            }
        }
        block.instructions = instructions;
    }

    Ok(SpillMaterialization {
        function: rewritten,
        unspillable,
    })
}

#[derive(Clone, Copy)]
struct Replacement {
    register: VirtualRegisterId,
    frame: FrameIndex,
    reads: bool,
    writes: bool,
}

impl Replacement {
    const fn new(register: VirtualRegisterId, frame: FrameIndex) -> Self {
        Self {
            register,
            frame,
            reads: false,
            writes: false,
        }
    }
}

fn preflight_spills<TargetError>(
    function: &MachineFunction,
    spills: &BTreeSet<VirtualRegisterId>,
) -> Result<(), SpillMaterializationError<TargetError>> {
    for block in &function.blocks {
        for instruction in &block.instructions {
            for (position, operand) in instruction.operands.iter().enumerate() {
                let MachineOperandKind::Register(MachineRegister::Virtual(register)) = operand.kind
                else {
                    continue;
                };
                if !spills.contains(&register) {
                    continue;
                }
                if matches!(operand.constraint, Some(RegisterConstraint::Fixed(_))) {
                    return Err(SpillMaterializationError::FixedConstraint { register });
                }
                let tied_from = instruction.operands.iter().any(|candidate| {
                    candidate
                        .tied_to
                        .is_some_and(|tied| usize::from(tied.get()) == position)
                });
                if operand.tied_to.is_some() || tied_from {
                    return Err(SpillMaterializationError::TiedOperand {
                        block: block.id,
                        instruction: instruction.id,
                        register,
                    });
                }
            }
        }
    }
    Ok(())
}

struct FreshIds {
    virtual_register: u32,
    frame: u32,
    instruction: u32,
}

impl FreshIds {
    fn new<TargetError>(
        function: &MachineFunction,
    ) -> Result<Self, SpillMaterializationError<TargetError>> {
        Ok(Self {
            virtual_register: next_id(
                function
                    .virtual_registers
                    .iter()
                    .map(|register| register.id.get())
                    .chain(function.blocks.iter().flat_map(|block| {
                        block.instructions.iter().flat_map(|instruction| {
                            instruction
                                .operands
                                .iter()
                                .filter_map(|operand| match operand.kind {
                                    MachineOperandKind::Register(MachineRegister::Virtual(
                                        register,
                                    )) => Some(register.get()),
                                    _ => None,
                                })
                        })
                    })),
            )
            .ok_or(SpillMaterializationError::VirtualRegisterIdExhausted)?,
            frame: next_id(
                function
                    .frame_objects
                    .iter()
                    .map(|object| object.index.get())
                    .chain(function.blocks.iter().flat_map(|block| {
                        block.instructions.iter().flat_map(|instruction| {
                            instruction
                                .operands
                                .iter()
                                .filter_map(|operand| match operand.kind {
                                    MachineOperandKind::FrameIndex { index, .. } => {
                                        Some(index.get())
                                    }
                                    _ => None,
                                })
                        })
                    })),
            )
            .ok_or(SpillMaterializationError::FrameIndexExhausted)?,
            instruction: next_id(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .map(|instruction| instruction.id.get()),
            )
            .ok_or(SpillMaterializationError::InstructionIdExhausted)?,
        })
    }

    fn virtual_register<TargetError>(
        &mut self,
    ) -> Result<VirtualRegisterId, SpillMaterializationError<TargetError>> {
        let result = VirtualRegisterId::new(self.virtual_register);
        self.virtual_register = self
            .virtual_register
            .checked_add(1)
            .ok_or(SpillMaterializationError::VirtualRegisterIdExhausted)?;
        Ok(result)
    }

    fn frame<TargetError>(&mut self) -> Result<FrameIndex, SpillMaterializationError<TargetError>> {
        let result = FrameIndex::new(self.frame);
        self.frame = self
            .frame
            .checked_add(1)
            .ok_or(SpillMaterializationError::FrameIndexExhausted)?;
        Ok(result)
    }

    fn instruction<TargetError>(
        &mut self,
    ) -> Result<MachineInstructionId, SpillMaterializationError<TargetError>> {
        let result = MachineInstructionId::new(self.instruction);
        self.instruction = self
            .instruction
            .checked_add(1)
            .ok_or(SpillMaterializationError::InstructionIdExhausted)?;
        Ok(result)
    }
}

fn next_id(values: impl Iterator<Item = u32>) -> Option<u32> {
    values.max().map_or(Some(0), |value| value.checked_add(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::old::codegen::machine::{
        FrameObjectKind, InstructionFlags, MachineBlock, MachineBlockId, MachineCallingConvention,
        MachineFunctionId, MachineInstruction, MachineLinkage, MachineModule, MachineOperand,
        OperandIndex, OperandRole, PhysicalRegister, TargetOpcode, allocate, apply_assignment,
        parse_text, write_text,
    };

    const GENERAL: RegisterClass = RegisterClass::new(0);
    const FIRST: PhysicalRegister = PhysicalRegister::new(0);

    #[derive(Clone, Copy)]
    struct TestTarget;

    impl SpillTarget for TestTarget {
        type Error = std::convert::Infallible;

        fn spill_frame_object(
            &self,
            index: FrameIndex,
            _: RegisterClass,
        ) -> Result<FrameObject, Self::Error> {
            Ok(FrameObject {
                index,
                size: 2,
                alignment: 2,
                kind: FrameObjectKind::Spill,
            })
        }

        fn spill_load(
            &self,
            id: MachineInstructionId,
            destination: VirtualRegisterId,
            frame: FrameIndex,
        ) -> MachineInstruction {
            instruction(
                id.get(),
                TargetOpcode::new(1),
                vec![
                    virtual_register(destination.get(), OperandRole::Def),
                    frame_operand(frame),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            )
        }

        fn spill_store(
            &self,
            id: MachineInstructionId,
            source: VirtualRegisterId,
            frame: FrameIndex,
        ) -> MachineInstruction {
            instruction(
                id.get(),
                TargetOpcode::new(2),
                vec![
                    frame_operand(frame),
                    virtual_register(source.get(), OperandRole::Use),
                ],
                InstructionFlags {
                    side_effects: true,
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            )
        }
    }

    fn virtual_register(id: u32, role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(
                id,
            ))),
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn fixed_virtual(id: u32, role: OperandRole) -> MachineOperand {
        MachineOperand {
            constraint: Some(RegisterConstraint::Fixed(FIRST)),
            ..virtual_register(id, role)
        }
    }

    fn frame_operand(index: FrameIndex) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::FrameIndex { index, addend: 0 },
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        }
    }

    fn instruction(
        id: u32,
        opcode: TargetOpcode,
        operands: Vec<MachineOperand>,
        flags: InstructionFlags,
    ) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(id),
            opcode,
            operands,
            flags,
        }
    }

    fn function(instructions: Vec<MachineInstruction>, registers: &[u32]) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(0),
            name: "spill".into(),
            linkage: MachineLinkage::Internal,
            signature: super::super::MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::FarPascal,
            },
            entry: MachineBlockId::new(0),
            virtual_registers: registers
                .iter()
                .map(|id| VirtualRegister {
                    id: VirtualRegisterId::new(*id),
                    class: GENERAL,
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

    #[test]
    fn empty_spill_set_is_an_exact_noop() {
        // Python spiller.spilled returns immediately for an empty set.  Even
        // exhausted ID spaces must therefore round-trip without allocating a
        // frame, virtual register, or instruction ID.
        let input = function(
            vec![instruction(
                u32::MAX,
                TargetOpcode::new(0),
                vec![virtual_register(u32::MAX, OperandRole::Def)],
                InstructionFlags::NONE,
            )],
            &[u32::MAX],
        );

        let rewritten = materialize_spills(&input, &[], &TestTarget).unwrap();

        assert_eq!(rewritten.function, input);
        assert!(rewritten.unspillable.is_empty());
    }

    #[test]
    fn usedef_uses_one_fresh_short_virtual_for_load_and_store() {
        let input = function(
            vec![instruction(
                0,
                TargetOpcode::new(0),
                vec![virtual_register(0, OperandRole::UseDef)],
                InstructionFlags::NONE,
            )],
            &[0],
        );
        let rewritten =
            materialize_spills(&input, &[VirtualRegisterId::new(0)], &TestTarget).unwrap();
        let instructions = &rewritten.function.blocks[0].instructions;
        let replacement = match instructions[1].operands[0].kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register)) => register,
            _ => panic!("the original occurrence must receive one short virtual"),
        };
        assert_eq!(instructions.len(), 3);
        assert!(rewritten.unspillable.contains(&replacement));
        assert!(matches!(
            instructions[0].operands[0].kind,
            MachineOperandKind::Register(MachineRegister::Virtual(register)) if register == replacement
        ));
        assert!(matches!(
            instructions[2].operands[1].kind,
            MachineOperandKind::Register(MachineRegister::Virtual(register)) if register == replacement
        ));
    }

    #[test]
    fn anchor_foundation_spill_provenance_survives_assignment_and_qmir_round_trip() {
        // Python lir.Insn.spill_reload/spill_store are allocator provenance,
        // not a property inferred from a selected load or store opcode.
        let input = function(
            vec![instruction(
                0,
                TargetOpcode::new(3),
                vec![virtual_register(0, OperandRole::UseDef)],
                InstructionFlags::NONE,
            )],
            &[0],
        );
        let materialized =
            materialize_spills(&input, &[VirtualRegisterId::new(0)], &TestTarget).unwrap();
        assert!(
            materialized.function.blocks[0].instructions[0]
                .flags
                .spill_reload
        );
        assert!(
            materialized.function.blocks[0].instructions[2]
                .flags
                .spill_store
        );

        let assignment = allocate(&materialized.function, |_| vec![FIRST], |_, _| false)
            .expect("short spill ranges receive their assigned physical register");
        let assigned = apply_assignment(&materialized.function, &assignment).unwrap();
        let module = MachineModule {
            data_objects: Vec::new(),
            functions: vec![assigned],
        };
        let round_tripped = parse_text(&write_text(&module)).unwrap();
        let instructions = &round_tripped.functions[0].blocks[0].instructions;
        assert!(instructions[0].flags.spill_reload);
        assert!(instructions[2].flags.spill_store);
    }

    #[test]
    fn repeated_operand_reloads_a_spill_only_once() {
        // NESTED emitted two identical frame reloads before each outer-loop
        // IMUL.  Ported from tests/test_spiller.py: the two operand
        // occurrences must name one short reload, not duplicate pressure.
        let input = function(
            vec![instruction(
                0,
                TargetOpcode::new(3),
                vec![
                    virtual_register(0, OperandRole::Use),
                    virtual_register(0, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            )],
            &[0],
        );

        let rewritten =
            materialize_spills(&input, &[VirtualRegisterId::new(0)], &TestTarget).unwrap();
        let instructions = &rewritten.function.blocks[0].instructions;

        assert_eq!(instructions.len(), 2);
        let replacement = match instructions[0].operands[0].kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register)) => register,
            _ => panic!("the spill reload must define one fresh virtual"),
        };
        assert!(matches!(
            instructions[1].operands.as_slice(),
            [
                MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Virtual(first)),
                    ..
                },
                MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Virtual(second)),
                    ..
                },
            ] if *first == replacement && *second == replacement
        ));
    }

    #[test]
    fn refuses_fixed_and_tied_values_until_their_occurrence_splits_are_ported() {
        let fixed = function(
            vec![instruction(
                0,
                TargetOpcode::new(0),
                vec![fixed_virtual(0, OperandRole::Use)],
                InstructionFlags::NONE,
            )],
            &[0],
        );
        assert!(matches!(
            materialize_spills(&fixed, &[VirtualRegisterId::new(0)], &TestTarget),
            Err(SpillMaterializationError::FixedConstraint {
                register
            }) if register == VirtualRegisterId::new(0)
        ));

        let mut tied = virtual_register(0, OperandRole::Def);
        tied.tied_to = Some(OperandIndex::new(1));
        let tied = function(
            vec![instruction(
                0,
                TargetOpcode::new(0),
                vec![tied, virtual_register(1, OperandRole::Use)],
                InstructionFlags::NONE,
            )],
            &[0, 1],
        );
        assert!(matches!(
            materialize_spills(&tied, &[VirtualRegisterId::new(0)], &TestTarget),
            Err(SpillMaterializationError::TiedOperand {
                register,
                ..
            }) if register == VirtualRegisterId::new(0)
        ));

        let mut tied_definition = virtual_register(1, OperandRole::Def);
        tied_definition.tied_to = Some(OperandIndex::new(0));
        let tied_target = function(
            vec![instruction(
                0,
                TargetOpcode::new(0),
                vec![virtual_register(0, OperandRole::Use), tied_definition],
                InstructionFlags::NONE,
            )],
            &[0, 1],
        );
        assert!(matches!(
            materialize_spills(
                &tied_target,
                &[VirtualRegisterId::new(0)],
                &TestTarget
            ),
            Err(SpillMaterializationError::TiedOperand {
                register,
                ..
            }) if register == VirtualRegisterId::new(0)
        ));
    }
}
