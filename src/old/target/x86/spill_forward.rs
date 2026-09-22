//! Forward exact frame-cell facts across allocated x86 Machine IR.
//!
//! This is the target-owned translation of Python
//! `qbopt/backend/spillforward.py::{_held,_available,_transfer,forwarded}`.
//! It intentionally runs before `apply_assignment`: the assignment supplies
//! the physical lanes for virtual operands while an eliminated load keeps its
//! virtual definition in a zero-byte ownership anchor.

use std::collections::{BTreeMap, BTreeSet};

use crate::old::codegen::machine::{
    FrameIndex, MachineBlock, MachineBlockId, MachineFunction, MachineInstruction,
    MachineInstructionId, MachineOperand, MachineOperandKind, MachineRegister, OperandRole,
    RegisterAssignment,
};

use super::{X86Opcode, X86Register, X87MemoryFormat};

type Facts = BTreeSet<Fact>;
type Lanes = BTreeSet<(X86Register, u8)>;

/// A physical register contains exactly these bytes from an abstract frame
/// object.  The addend and accessed width are part of the fact: wider and
/// overlapping accesses are not interchangeable facts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Fact {
    register: X86Register,
    cell: FrameCell,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FrameCell {
    index: FrameIndex,
    addend: i64,
    width: u32,
}

enum FrameWrite {
    None,
    Exact {
        cell: FrameCell,
        source: Option<X86Register>,
    },
    Unknown,
}

/// Removes exact frame loads already held in the assigned physical register.
///
/// This is deliberately independent of `spill_reload` and `FrameObjectKind`.
/// Python forwards both allocator reloads and selected program loads when the
/// exact register/cell fact is available on every incoming path.
pub fn forward_frame_reloads(
    function: &MachineFunction,
    assignment: &RegisterAssignment,
) -> MachineFunction {
    let into = available(function, assignment);
    let mut rewritten = function.clone();

    for block in &mut rewritten.blocks {
        let redundant = transfer(
            block,
            into.get(&block.id).cloned().unwrap_or_default(),
            assignment,
        )
        .0;
        if redundant.is_empty() {
            continue;
        }
        for instruction in &mut block.instructions {
            if redundant.contains(&instruction.id) {
                *instruction = instruction.anchor(X86Opcode::Nothing.machine_opcode());
            }
        }
    }
    rewritten
}

/// Python `_available`: entry and no-predecessor blocks begin at bottom;
/// unreached non-entry blocks begin at top (`None`) and meet every predecessor.
fn available(
    function: &MachineFunction,
    assignment: &RegisterAssignment,
) -> BTreeMap<MachineBlockId, Facts> {
    let mut predecessors = function
        .blocks
        .iter()
        .map(|block| (block.id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    for block in &function.blocks {
        for successor in &block.successors {
            if let Some(incoming) = predecessors.get_mut(successor) {
                incoming.push(block.id);
            }
        }
    }

    let mut into = BTreeMap::<MachineBlockId, Option<Facts>>::new();
    for block in &function.blocks {
        let no_predecessors = predecessors.get(&block.id).is_some_and(Vec::is_empty);
        into.insert(
            block.id,
            if block.id == function.entry || no_predecessors {
                Some(Facts::new())
            } else {
                None
            },
        );
    }
    let mut outof = BTreeMap::<MachineBlockId, Facts>::new();

    let mut changing = true;
    while changing {
        changing = false;
        for (block_id, facts) in &into {
            let Some(facts) = facts else {
                continue;
            };
            let leaving = transfer(blocks[block_id], facts.clone(), assignment).1;
            if outof.get(block_id) != Some(&leaving) {
                outof.insert(*block_id, leaving);
                changing = true;
            }
        }
        for block in &function.blocks {
            if block.id == function.entry || predecessors[&block.id].is_empty() {
                continue;
            }
            let mut met = None::<Facts>;
            for predecessor in &predecessors[&block.id] {
                let Some(leaving) = outof.get(predecessor) else {
                    continue;
                };
                met = Some(match met {
                    None => leaving.clone(),
                    Some(facts) => facts.intersection(leaving).copied().collect(),
                });
            }
            if let Some(met) = met {
                if into.get(&block.id) != Some(&Some(met.clone())) {
                    into.insert(block.id, Some(met));
                    changing = true;
                }
            }
        }
    }

    into.into_iter()
        .map(|(block, facts)| (block, facts.unwrap_or_default()))
        .collect()
}

/// Python `_transfer`.
fn transfer(
    block: &MachineBlock,
    mut facts: Facts,
    assignment: &RegisterAssignment,
) -> (BTreeSet<MachineInstructionId>, Facts) {
    let mut redundant = BTreeSet::new();
    for instruction in &block.instructions {
        let (held, drop) = held(instruction, facts, assignment);
        facts = held;
        if drop {
            redundant.insert(instruction.id);
        }
    }
    (redundant, facts)
}

/// Python `_held` with selected Machine-IR operands in place of iced decode.
fn held(
    instruction: &MachineInstruction,
    mut facts: Facts,
    assignment: &RegisterAssignment,
) -> (Facts, bool) {
    let opcode = X86Opcode::from_machine_opcode(instruction.opcode);
    if opcode.is_none() {
        return (Facts::new(), false);
    }
    if instruction.flags.anchor || opcode == Some(X86Opcode::Nothing) {
        return (facts, false);
    }
    if matches!(opcode, Some(X86Opcode::Jump | X86Opcode::JumpConditional)) {
        return (facts, false);
    }
    if instruction.flags.call || matches!(opcode, Some(X86Opcode::CallNear | X86Opcode::CallFar)) {
        return (Facts::new(), false);
    }
    if opcode == Some(X86Opcode::X87StoreStatusWord) {
        // Python represents the status-word transfer emitted for an x87
        // comparison as a BARRIER. Its `_register_effects` therefore
        // returns unknown and ends every held-slot fact before SAHF. Keep
        // that boundary even though this target opcode names AX explicitly.
        return (Facts::new(), false);
    }
    let Some(writes) = register_writes(instruction, assignment) else {
        return (Facts::new(), false);
    };
    if overlaps_bp(&writes) {
        return (Facts::new(), false);
    }

    let write = frame_write(instruction, opcode, assignment);
    let FrameWrite::Unknown = write else {
        if let FrameWrite::Exact { cell, .. } = write {
            facts.retain(|fact| !overlapping(fact.cell, cell));
        }
        if let Some((register, cell)) = frame_load(instruction, opcode, assignment) {
            let fact = Fact { register, cell };
            if facts.contains(&fact) {
                return (facts, removable_load(instruction));
            }
            facts.retain(|fact| {
                !register_lanes(fact.register).is_some_and(|lanes| !lanes.is_disjoint(&writes))
            });
            facts.insert(fact);
            return (facts, false);
        }
        facts.retain(|fact| {
            !register_lanes(fact.register).is_some_and(|lanes| !lanes.is_disjoint(&writes))
        });
        if let FrameWrite::Exact {
            cell,
            source: Some(source),
        } = write
        {
            if !register_lanes(source).is_some_and(|lanes| !lanes.is_disjoint(&writes)) {
                facts.insert(Fact {
                    register: source,
                    cell,
                });
            }
        }
        return (facts, false);
    };
    (Facts::new(), false)
}

fn removable_load(instruction: &MachineInstruction) -> bool {
    instruction.flags.may_load
        && !instruction.flags.may_store
        && !instruction.flags.volatile
        && !instruction.flags.side_effects
        && !instruction.flags.call
        && !instruction.flags.terminator
        && !instruction.flags.anchor
}

fn frame_load(
    instruction: &MachineInstruction,
    opcode: Option<X86Opcode>,
    assignment: &RegisterAssignment,
) -> Option<(X86Register, FrameCell)> {
    if opcode != Some(X86Opcode::Load) || instruction.operands.len() != 2 {
        return None;
    }
    let [destination, frame] = instruction.operands.as_slice() else {
        return None;
    };
    if destination.role != OperandRole::Def {
        return None;
    }
    let register = register(destination, assignment)?;
    let width = u32::from(register.byte_width()?);
    Some((register, frame_cell(frame, width)?))
}

fn frame_write(
    instruction: &MachineInstruction,
    opcode: Option<X86Opcode>,
    assignment: &RegisterAssignment,
) -> FrameWrite {
    match opcode {
        Some(X86Opcode::Store) => {
            direct_store(instruction, assignment).unwrap_or(FrameWrite::Unknown)
        }
        Some(
            X86Opcode::X87Store
            | X86Opcode::X87StorePop
            | X86Opcode::X87IntegerStore
            | X86Opcode::X87IntegerStorePop
            | X86Opcode::X87IntegerStoreTrunc,
        ) => x87_store(instruction, 1, 2).unwrap_or(FrameWrite::Unknown),
        Some(X86Opcode::X87StoreControlWord) => {
            x87_store(instruction, 0, 1).unwrap_or(FrameWrite::Unknown)
        }
        _ if instruction.flags.may_store => FrameWrite::Unknown,
        _ => FrameWrite::None,
    }
}

fn direct_store(
    instruction: &MachineInstruction,
    assignment: &RegisterAssignment,
) -> Option<FrameWrite> {
    match instruction.operands.as_slice() {
        [frame, source] if source.role == OperandRole::Use => {
            let source = register(source, assignment)?;
            let cell = frame_cell(frame, u32::from(source.byte_width()?))?;
            Some(FrameWrite::Exact {
                cell,
                source: Some(source),
            })
        }
        [frame, width, value]
            if matches!(value.kind, MachineOperandKind::Immediate(_))
                && matches!(width.kind, MachineOperandKind::Immediate(8 | 16 | 32)) =>
        {
            let MachineOperandKind::Immediate(bits) = width.kind else {
                return None;
            };
            Some(FrameWrite::Exact {
                cell: frame_cell(frame, u32::try_from(bits / 8).ok()?)?,
                source: None,
            })
        }
        _ => None,
    }
}

fn x87_store(
    instruction: &MachineInstruction,
    format_position: usize,
    frame_position: usize,
) -> Option<FrameWrite> {
    let MachineOperandKind::Immediate(raw) = instruction.operands.get(format_position)?.kind else {
        return None;
    };
    let format = X87MemoryFormat::from_raw(u8::try_from(raw).ok()?)?;
    Some(FrameWrite::Exact {
        cell: frame_cell(
            instruction.operands.get(frame_position)?,
            format.byte_width(),
        )?,
        source: None,
    })
}

fn frame_cell(operand: &MachineOperand, width: u32) -> Option<FrameCell> {
    let MachineOperandKind::FrameIndex { index, addend } = operand.kind else {
        return None;
    };
    Some(FrameCell {
        index,
        addend,
        width,
    })
}

fn overlapping(left: FrameCell, right: FrameCell) -> bool {
    left.index == right.index
        && left.addend < right.addend.saturating_add(i64::from(right.width))
        && right.addend < left.addend.saturating_add(i64::from(left.width))
}

fn register_writes(
    instruction: &MachineInstruction,
    assignment: &RegisterAssignment,
) -> Option<Lanes> {
    let mut writes = Lanes::new();
    for operand in &instruction.operands {
        let MachineOperandKind::Register(_) = operand.kind else {
            continue;
        };
        let register = register(operand, assignment)?;
        if operand.role.writes() {
            writes.extend(register_lanes(register)?);
        }
    }
    match X86Opcode::from_machine_opcode(instruction.opcode) {
        Some(
            X86Opcode::Push
            | X86Opcode::Pop
            | X86Opcode::CallNear
            | X86Opcode::CallFar
            | X86Opcode::ReturnNear
            | X86Opcode::ReturnFar,
        ) => {
            writes.extend(register_lanes(X86Register::Sp)?);
        }
        Some(X86Opcode::Leave) => {
            writes.extend(register_lanes(X86Register::Bp)?);
            writes.extend(register_lanes(X86Register::Sp)?);
        }
        _ => {}
    }
    Some(writes)
}

fn register(operand: &MachineOperand, assignment: &RegisterAssignment) -> Option<X86Register> {
    let MachineOperandKind::Register(register) = operand.kind else {
        return None;
    };
    let physical = match register {
        MachineRegister::Physical(register) => register,
        MachineRegister::Virtual(register) => assignment.get(register)?,
    };
    X86Register::from_physical(physical)
}

fn register_lanes(register: X86Register) -> Option<Lanes> {
    let root = register.root()?;
    let Some(width) = root.byte_width() else {
        // Python `_lanes` makes x87 stack registers a deliberately empty
        // GPR-lane effect.  They cannot form an integer load fact, but their
        // arithmetic must not erase one between two frame loads.
        return Some(Lanes::new());
    };
    let mask = register.lanes();
    Some(
        (0..width)
            .filter(|lane| mask & (1_u8 << lane) != 0)
            .map(|lane| (root, lane))
            .collect(),
    )
}

fn overlaps_bp(writes: &Lanes) -> bool {
    register_lanes(X86Register::Bp).is_some_and(|lanes| !lanes.is_disjoint(writes))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::old::codegen::machine::{
        FrameObject, FrameObjectKind, InstructionFlags, MachineCallingConvention,
        MachineFunctionId, MachineLinkage, MachineSignature, TargetOpcode, VirtualRegister,
        VirtualRegisterId, apply_assignment,
    };
    use crate::old::target::x86::X86RegisterClass;

    fn operand(kind: MachineOperandKind, role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind,
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn frame(index: u32, addend: i64) -> MachineOperand {
        operand(
            MachineOperandKind::FrameIndex {
                index: FrameIndex::new(index),
                addend,
            },
            OperandRole::None,
        )
    }

    fn virtual_register(id: u32, role: OperandRole) -> MachineOperand {
        operand(
            MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(id))),
            role,
        )
    }

    fn physical_register(register: X86Register, role: OperandRole) -> MachineOperand {
        operand(
            MachineOperandKind::Register(MachineRegister::Physical(register.physical())),
            role,
        )
    }

    fn load(id: u32, register: u32, index: u32) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(id),
            opcode: X86Opcode::Load.machine_opcode(),
            operands: vec![
                virtual_register(register, OperandRole::Def),
                frame(index, 0),
            ],
            flags: InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
        }
    }

    fn store(id: u32, register: u32, index: u32) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(id),
            opcode: X86Opcode::Store.machine_opcode(),
            operands: vec![
                frame(index, 0),
                virtual_register(register, OperandRole::Use),
            ],
            flags: InstructionFlags {
                may_store: true,
                side_effects: true,
                ..InstructionFlags::NONE
            },
        }
    }

    fn function(entry: u32, blocks: Vec<MachineBlock>, registers: &[u32]) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(1),
            name: "spill-forward".into(),
            linkage: MachineLinkage::Internal,
            signature: MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::C,
            },
            entry: MachineBlockId::new(entry),
            virtual_registers: registers
                .iter()
                .map(|id| VirtualRegister {
                    id: VirtualRegisterId::new(*id),
                    class: X86RegisterClass::Dword.machine_class(),
                })
                .collect(),
            blocks,
            frame_objects: vec![
                FrameObject {
                    index: FrameIndex::new(0),
                    size: 4,
                    alignment: 2,
                    kind: FrameObjectKind::Spill,
                },
                FrameObject {
                    index: FrameIndex::new(1),
                    size: 4,
                    alignment: 2,
                    kind: FrameObjectKind::IncomingArgument { parameter: 0 },
                },
            ],
        }
    }

    fn assignment(pairs: &[(u32, X86Register)]) -> RegisterAssignment {
        RegisterAssignment::from_assignments(
            pairs
                .iter()
                .map(|(virtual_register, physical)| {
                    (
                        VirtualRegisterId::new(*virtual_register),
                        physical.physical(),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
        )
    }

    #[test]
    fn entry_reload_needs_exact_agreement_on_every_edge_and_plain_load_is_eligible() {
        for mismatch in [
            "none", "register", "slot", "width", "write", "unknown", "opcode", "entry",
        ] {
            let left = store(1, 0, 0);
            let mut right = store(2, 0, 0);
            let mut registers = vec![0, 1];
            let mut assigned = vec![(0, X86Register::Eax), (1, X86Register::Eax)];
            match mismatch {
                "register" => {
                    right.operands[1] = virtual_register(1, OperandRole::Use);
                    assigned[1] = (1, X86Register::Ecx);
                }
                "slot" => right.operands[0] = frame(1, 0),
                "width" => {
                    right.operands[1] = physical_register(X86Register::Ax, OperandRole::Use);
                }
                "write" => right
                    .operands
                    .push(physical_register(X86Register::Eax, OperandRole::Def)),
                "unknown" => {
                    right.opcode = X86Opcode::Mov.machine_opcode();
                    right.flags.may_store = true;
                }
                "opcode" => {
                    right.id = MachineInstructionId::new(4);
                    right.opcode = TargetOpcode::new(999);
                    right.flags = InstructionFlags::NONE;
                }
                "entry" => {}
                "none" => {}
                _ => unreachable!(),
            }
            if mismatch == "width" {
                registers.retain(|id| *id != 1);
                assigned.retain(|(id, _)| *id != 1);
            }
            let reload = load(3, 1, 0);
            let body = function(
                if mismatch == "entry" { 3 } else { 0 },
                vec![
                    MachineBlock {
                        id: MachineBlockId::new(0),
                        instructions: Vec::new(),
                        successors: vec![MachineBlockId::new(1), MachineBlockId::new(2)],
                    },
                    MachineBlock {
                        id: MachineBlockId::new(1),
                        instructions: vec![left],
                        successors: vec![MachineBlockId::new(3)],
                    },
                    MachineBlock {
                        id: MachineBlockId::new(2),
                        instructions: if mismatch == "opcode" {
                            vec![store(2, 0, 0), right]
                        } else {
                            vec![right]
                        },
                        successors: vec![MachineBlockId::new(3)],
                    },
                    MachineBlock {
                        id: MachineBlockId::new(3),
                        instructions: vec![reload],
                        successors: Vec::new(),
                    },
                ],
                &registers,
            );
            let done = forward_frame_reloads(&body, &assignment(&assigned));
            let removed = done.blocks[3].instructions[0].flags.anchor;
            assert_eq!(removed, mismatch == "none", "{mismatch}");
        }
    }

    #[test]
    fn forwarded_reload_retains_virtual_definition_for_assignment() {
        let body = function(
            0,
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![load(1, 0, 0), load(2, 1, 0), store(3, 1, 1)],
                successors: Vec::new(),
            }],
            &[0, 1],
        );
        let done = forward_frame_reloads(
            &body,
            &assignment(&[(0, X86Register::Eax), (1, X86Register::Eax)]),
        );
        assert!(done.blocks[0].instructions[1].flags.anchor);
        assert_eq!(
            done.blocks[0].instructions[1].operands,
            vec![virtual_register(1, OperandRole::Def)]
        );
        let applied = apply_assignment(
            &done,
            &assignment(&[(0, X86Register::Eax), (1, X86Register::Eax)]),
        )
        .expect("the anchored definition remains assignable to its later store");
        assert_eq!(applied.virtual_registers.len(), 1);
        assert_eq!(applied.virtual_registers[0].id, VirtualRegisterId::new(1));
        assert!(matches!(
            applied.blocks[0].instructions[2].operands[1].kind,
            MachineOperandKind::Register(MachineRegister::Physical(register)) if register == X86Register::Eax.physical()
        ));
    }

    #[test]
    fn parameter_loads_cross_read_only_work_but_not_assignment_mismatch_and_is_idempotent() {
        let mut address_work = MachineInstruction {
            id: MachineInstructionId::new(2),
            opcode: X86Opcode::Lea.machine_opcode(),
            operands: vec![virtual_register(2, OperandRole::Def), frame(1, 0)],
            flags: InstructionFlags::NONE,
        };
        let body = function(
            0,
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![
                    load(1, 0, 1),
                    address_work.clone(),
                    MachineInstruction {
                        id: MachineInstructionId::new(3),
                        opcode: X86Opcode::X87Add.machine_opcode(),
                        operands: vec![
                            physical_register(X86Register::St0, OperandRole::UseDef),
                            physical_register(X86Register::St1, OperandRole::Use),
                        ],
                        flags: InstructionFlags::NONE,
                    },
                    load(4, 1, 1),
                ],
                successors: Vec::new(),
            }],
            &[0, 1, 2],
        );
        let same = assignment(&[
            (0, X86Register::Si),
            (1, X86Register::Si),
            (2, X86Register::Di),
        ]);
        let mut body = body;
        for virtual_register in &mut body.virtual_registers[..2] {
            virtual_register.class = X86RegisterClass::Address16.machine_class();
        }
        let done = forward_frame_reloads(&body, &same);
        assert!(!done.blocks[0].instructions[0].flags.anchor);
        assert!(done.blocks[0].instructions[3].flags.anchor);
        assert_eq!(forward_frame_reloads(&done, &same), done);
        let mismatch = assignment(&[
            (0, X86Register::Si),
            (1, X86Register::Di),
            (2, X86Register::Bx),
        ]);
        assert!(
            !forward_frame_reloads(&body, &mismatch).blocks[0].instructions[3]
                .flags
                .anchor
        );
        address_work.flags.may_store = true;
        let blocked = function(
            0,
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![load(1, 0, 1), address_work, load(3, 1, 1)],
                successors: Vec::new(),
            }],
            &[0, 1, 2],
        );
        let mut blocked = blocked;
        for virtual_register in &mut blocked.virtual_registers[..2] {
            virtual_register.class = X86RegisterClass::Address16.machine_class();
        }
        assert!(
            !forward_frame_reloads(&blocked, &same).blocks[0].instructions[2]
                .flags
                .anchor
        );
    }

    #[test]
    fn x87_status_barrier_ends_python_held_frame_facts() {
        let body = function(
            0,
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![
                    load(1, 0, 0),
                    MachineInstruction {
                        id: MachineInstructionId::new(2),
                        opcode: X86Opcode::X87StoreStatusWord.machine_opcode(),
                        operands: vec![physical_register(X86Register::Ax, OperandRole::Def)],
                        flags: InstructionFlags::NONE,
                    },
                    load(3, 1, 0),
                ],
                successors: Vec::new(),
            }],
            &[0, 1],
        );
        let assigned = assignment(&[(0, X86Register::Bx), (1, X86Register::Bx)]);

        let forwarded = forward_frame_reloads(&body, &assigned);

        assert!(!forwarded.blocks[0].instructions[2].flags.anchor);
    }

    #[test]
    fn low_and_high_byte_lanes_are_disjoint() {
        let mut body = function(
            0,
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![
                    load(1, 0, 0),
                    MachineInstruction {
                        id: MachineInstructionId::new(2),
                        opcode: X86Opcode::Mov.machine_opcode(),
                        operands: vec![physical_register(X86Register::Ah, OperandRole::Def)],
                        flags: InstructionFlags::NONE,
                    },
                    load(3, 1, 0),
                ],
                successors: Vec::new(),
            }],
            &[0, 1],
        );
        for virtual_register in &mut body.virtual_registers {
            virtual_register.class = X86RegisterClass::Byte.machine_class();
        }
        let assigned = assignment(&[(0, X86Register::Al), (1, X86Register::Al)]);
        assert!(
            forward_frame_reloads(&body, &assigned).blocks[0].instructions[2]
                .flags
                .anchor
        );
    }
}
