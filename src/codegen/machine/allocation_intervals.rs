//! Segmented virtual-register live intervals and reference weights.
//!
//! This is the Machine IR translation of `qbopt/analysis/intervals.py`.
//! It deliberately stops before allocation: it neither chooses registers nor
//! discovers loops.  The caller supplies loop depth as a target- and
//! frontend-independent callback when it asks for spill-reference weights.
//!
//! Machine IR has no phi nodes or parallel copy groups.  We nevertheless
//! reserve the same two-slot block boundary as Python's `indexed()` reserves
//! for phis: it gives a value live through an empty block a real range, and
//! keeps intervals in emitted block order.  No Machine IR value is defined at
//! that boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::{
    compute_liveness, MachineBlockId, MachineFunction, MachineInstruction, MachineInstructionId,
    MachineLivenessError, MachineOperandKind, MachineRegister, VirtualRegisterId,
};

/// The slot where an instruction reads its operands.
pub const USE: u64 = 0;
/// The slot where an instruction writes its results.
pub const DEF: u64 = 1;
/// Slots reserved for each instruction and each block boundary.
pub const PER_INSN: u64 = 2;

/// Estimated loop iterations per nesting level, from `intervals.py`.
pub const PER_LEVEL: u64 = 10;
/// Live-slot grace before normalizing a reference count, from `intervals.py`.
pub const GRACE: u64 = 25 * PER_INSN;

/// A half-open run of slot indexes occupied by one virtual register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveSegment {
    pub start: u64,
    pub end: u64,
}

impl LiveSegment {
    /// Whether `slot` belongs to this half-open segment.
    pub const fn contains(self, slot: u64) -> bool {
        self.start <= slot && slot < self.end
    }

    /// Whether this and `other` occupy at least one common slot.
    pub const fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// Every segment occupied by one virtual register and its spill weight.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveInterval {
    pub register: VirtualRegisterId,
    pub segments: Vec<LiveSegment>,
    pub weight: f64,
}

impl LiveInterval {
    /// The number of occupied slots, not the span from first to last slot.
    pub fn size(&self) -> u64 {
        self.segments
            .iter()
            .map(|segment| segment.end - segment.start)
            .sum()
    }

    /// Whether any segment of this interval contains `slot`.
    pub fn contains(&self, slot: u64) -> bool {
        self.segments.iter().any(|segment| segment.contains(slot))
    }

    /// Whether any segment of this interval overlaps a segment of `other`.
    ///
    /// Both interval builders and `merge_segments` leave segments sorted, so
    /// this is the same linear walk as Python's `Interval.overlaps()`.
    pub fn overlaps(&self, other: &Self) -> bool {
        let mut mine = self.segments.iter();
        let mut theirs = other.segments.iter();
        let mut one = mine.next();
        let mut two = theirs.next();

        while let (Some(left), Some(right)) = (one, two) {
            if left.overlaps(*right) {
                return true;
            }
            if left.end <= right.end {
                one = mine.next();
            } else {
                two = theirs.next();
            }
        }
        false
    }
}

/// Instruction slots and block spans in stored Machine IR block order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineIntervalIndexes {
    instruction_slots: BTreeMap<MachineInstructionId, u64>,
    block_spans: BTreeMap<MachineBlockId, LiveSegment>,
    block_order: Vec<MachineBlockId>,
}

impl MachineIntervalIndexes {
    /// The first slot assigned to `instruction`.
    pub fn instruction_slot(&self, instruction: MachineInstructionId) -> Option<u64> {
        self.instruction_slots.get(&instruction).copied()
    }

    /// The half-open span assigned to `block`, including its boundary slots.
    pub fn block_span(&self, block: MachineBlockId) -> Option<LiveSegment> {
        self.block_spans.get(&block).copied()
    }

    /// Blocks in the deterministic order used to number their instructions.
    pub fn block_order(&self) -> &[MachineBlockId] {
        &self.block_order
    }
}

/// A malformed Machine IR fact which prevents interval analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineIntervalError {
    Liveness {
        errors: Vec<MachineLivenessError>,
    },
    DuplicateBlockId {
        block: MachineBlockId,
    },
    DuplicateInstructionId {
        instruction: MachineInstructionId,
        first_block: MachineBlockId,
        duplicate_block: MachineBlockId,
    },
    SlotOverflow,
    MissingBlockSpan {
        block: MachineBlockId,
    },
    MissingBlockLiveness {
        block: MachineBlockId,
    },
    LoopWeightOverflow {
        block: MachineBlockId,
        depth: u32,
    },
    WeightOverflow {
        register: VirtualRegisterId,
    },
}

impl fmt::Display for MachineIntervalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Liveness { errors } => write!(
                formatter,
                "cannot compute live intervals: {} liveness error(s)",
                errors.len()
            ),
            Self::DuplicateBlockId { block } => {
                write!(formatter, "duplicate machine block id {block}")
            }
            Self::DuplicateInstructionId {
                instruction,
                first_block,
                duplicate_block,
            } => write!(
                formatter,
                "machine instruction {instruction} occurs in both block {first_block} and block {duplicate_block}"
            ),
            Self::SlotOverflow => write!(formatter, "machine instruction slots overflow u64"),
            Self::MissingBlockSpan { block } => {
                write!(formatter, "machine block {block} has no interval span")
            }
            Self::MissingBlockLiveness { block } => {
                write!(formatter, "machine block {block} has no liveness fact")
            }
            Self::LoopWeightOverflow { block, depth } => write!(
                formatter,
                "machine block {block} loop depth {depth} cannot be represented as a finite reference weight"
            ),
            Self::WeightOverflow { register } => write!(
                formatter,
                "virtual register {register} has a reference weight too large to represent"
            ),
        }
    }
}

impl Error for MachineIntervalError {}

/// Numbers instruction use/definition positions in stored block order.
///
/// This translates Python's `indexed()`.  As in that implementation, every
/// block first consumes `PER_INSN` boundary slots.  Machine IR's verifier
/// requires unique instruction IDs; this analysis repeats that check because
/// it needs an unambiguous ID-to-slot map rather than guessing a position.
pub fn index_intervals(
    function: &MachineFunction,
) -> Result<MachineIntervalIndexes, Vec<MachineIntervalError>> {
    let mut instruction_slots = BTreeMap::new();
    let mut instruction_blocks = BTreeMap::new();
    let mut block_ids = BTreeSet::new();
    let mut block_spans = BTreeMap::new();
    let mut block_order = Vec::with_capacity(function.blocks.len());
    let mut errors = Vec::new();
    let mut next_slot = 0_u64;

    for block in &function.blocks {
        if !block_ids.insert(block.id) {
            errors.push(MachineIntervalError::DuplicateBlockId { block: block.id });
        }
        let first = next_slot;
        let Some(after_boundary) = next_slot.checked_add(PER_INSN) else {
            errors.push(MachineIntervalError::SlotOverflow);
            break;
        };
        next_slot = after_boundary;

        for instruction in &block.instructions {
            if let Some(first_block) = instruction_blocks.insert(instruction.id, block.id) {
                errors.push(MachineIntervalError::DuplicateInstructionId {
                    instruction: instruction.id,
                    first_block,
                    duplicate_block: block.id,
                });
            } else {
                instruction_slots.insert(instruction.id, next_slot);
            }
            let Some(after_instruction) = next_slot.checked_add(PER_INSN) else {
                errors.push(MachineIntervalError::SlotOverflow);
                break;
            };
            next_slot = after_instruction;
        }

        block_spans.insert(
            block.id,
            LiveSegment {
                start: first,
                end: next_slot,
            },
        );
        block_order.push(block.id);
    }

    if errors.is_empty() {
        Ok(MachineIntervalIndexes {
            instruction_slots,
            block_spans,
            block_order,
        })
    } else {
        Err(errors)
    }
}

/// Builds unweighted segmented intervals from Machine IR liveness.
///
/// This translates `_ranges()` in `qbopt/analysis/intervals.py`.  Machine IR
/// has neither `phis` nor instruction `group`s, so there is no equivalent
/// edge definition or parallel-copy boundary to process here.  All virtual
/// uses occur at `USE` and all definitions at `DEF` of their instruction.
pub fn live_intervals(
    function: &MachineFunction,
) -> Result<BTreeMap<VirtualRegisterId, LiveInterval>, Vec<MachineIntervalError>> {
    let indexes = index_intervals(function)?;
    let liveness = compute_liveness(function)
        .map_err(|errors| vec![MachineIntervalError::Liveness { errors }])?;
    let mut pieces = BTreeMap::<VirtualRegisterId, Vec<LiveSegment>>::new();

    for block in &function.blocks {
        let span = indexes
            .block_span(block.id)
            .ok_or_else(|| vec![MachineIntervalError::MissingBlockSpan { block: block.id }])?;
        let block_liveness = liveness
            .blocks
            .get(&block.id)
            .ok_or_else(|| vec![MachineIntervalError::MissingBlockLiveness { block: block.id }])?;
        let mut alive = block_liveness
            .live_out
            .iter()
            .map(|register| (*register, span.end))
            .collect::<BTreeMap<_, _>>();

        for instruction in block.instructions.iter().rev() {
            let slot = indexes
                .instruction_slot(instruction.id)
                .ok_or_else(|| vec![MachineIntervalError::MissingBlockSpan { block: block.id }])?;
            let boundary = slot
                .checked_add(DEF)
                .ok_or_else(|| vec![MachineIntervalError::SlotOverflow])?;
            let one_after_boundary = boundary
                .checked_add(1)
                .ok_or_else(|| vec![MachineIntervalError::SlotOverflow])?;

            for register in instruction_definitions(instruction) {
                let end = alive.remove(&register).unwrap_or(one_after_boundary);
                pieces.entry(register).or_default().push(LiveSegment {
                    start: boundary,
                    end,
                });
            }
            for register in instruction_uses(instruction) {
                alive.entry(register).or_insert(boundary);
            }
        }

        for (register, end) in alive {
            if end > span.start {
                pieces.entry(register).or_default().push(LiveSegment {
                    start: span.start,
                    end,
                });
            }
        }
    }

    Ok(pieces
        .into_iter()
        .map(|(register, segments)| {
            (
                register,
                LiveInterval {
                    register,
                    segments: merge_segments(segments),
                    weight: 0.0,
                },
            )
        })
        .collect())
}

/// Computes Python `weights()` without choosing how loop depth is discovered.
///
/// `loop_depth` returns nesting depth for a block.  A caller with a map can
/// pass `|block| depths.get(&block).copied().unwrap_or(0)`; no frontend,
/// target, or loop-analysis dependency enters Machine IR.
pub fn interval_weights<LoopDepth>(
    function: &MachineFunction,
    intervals: &BTreeMap<VirtualRegisterId, LiveInterval>,
    loop_depth: LoopDepth,
) -> Result<BTreeMap<VirtualRegisterId, f64>, Vec<MachineIntervalError>>
where
    LoopDepth: Fn(MachineBlockId) -> u32,
{
    index_intervals(function)?;
    compute_liveness(function).map_err(|errors| vec![MachineIntervalError::Liveness { errors }])?;
    let mut total = BTreeMap::<VirtualRegisterId, f64>::new();

    for block in &function.blocks {
        let depth = loop_depth(block.id);
        let each = loop_frequency(block.id, depth).map_err(|error| vec![error])?;
        for instruction in &block.instructions {
            // This is deliberately not `instruction_definitions()` plus
            // `instruction_uses()`: those sets are right for liveness but
            // lose Python `_weights()`'s repeated operand occurrences.
            for operand in &instruction.operands {
                if operand.role.writes() {
                    if let MachineOperandKind::Register(MachineRegister::Virtual(register)) =
                        operand.kind
                    {
                        add_reference(&mut total, register, each)?;
                    }
                }
            }
            for operand in &instruction.operands {
                if operand.role.reads() {
                    if let MachineOperandKind::Register(MachineRegister::Virtual(register)) =
                        operand.kind
                    {
                        add_reference(&mut total, register, each)?;
                    }
                }
            }
        }
    }

    total
        .into_iter()
        .map(|(register, references)| {
            let size = intervals.get(&register).map_or(0, LiveInterval::size);
            let denominator = size
                .checked_add(GRACE)
                .ok_or_else(|| vec![MachineIntervalError::WeightOverflow { register }])?;
            let weight = references / denominator as f64;
            if !weight.is_finite() {
                return Err(vec![MachineIntervalError::WeightOverflow { register }]);
            }
            Ok((register, weight))
        })
        .collect()
}

/// Builds intervals and attaches Python `_weights()` reference costs.
pub fn weighted_live_intervals<LoopDepth>(
    function: &MachineFunction,
    loop_depth: LoopDepth,
) -> Result<BTreeMap<VirtualRegisterId, LiveInterval>, Vec<MachineIntervalError>>
where
    LoopDepth: Fn(MachineBlockId) -> u32,
{
    let mut intervals = live_intervals(function)?;
    let weights = interval_weights(function, &intervals, loop_depth)?;
    for (register, weight) in weights {
        if let Some(interval) = intervals.get_mut(&register) {
            interval.weight = weight;
        }
    }
    Ok(intervals)
}

fn instruction_uses(instruction: &MachineInstruction) -> BTreeSet<VirtualRegisterId> {
    instruction
        .operands
        .iter()
        .filter_map(|operand| match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register))
                if operand.role.reads() =>
            {
                Some(register)
            }
            _ => None,
        })
        .collect()
}

fn instruction_definitions(instruction: &MachineInstruction) -> BTreeSet<VirtualRegisterId> {
    instruction
        .operands
        .iter()
        .filter_map(|operand| match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register))
                if operand.role.writes() =>
            {
                Some(register)
            }
            _ => None,
        })
        .collect()
}

fn add_reference(
    total: &mut BTreeMap<VirtualRegisterId, f64>,
    register: VirtualRegisterId,
    frequency: f64,
) -> Result<(), Vec<MachineIntervalError>> {
    let found = total.entry(register).or_insert(0.0);
    *found += frequency;
    if found.is_finite() {
        Ok(())
    } else {
        Err(vec![MachineIntervalError::WeightOverflow { register }])
    }
}

fn merge_segments(mut segments: Vec<LiveSegment>) -> Vec<LiveSegment> {
    segments.sort_unstable_by_key(|segment| (segment.start, segment.end));
    let mut merged = Vec::<LiveSegment>::new();
    for segment in segments {
        if let Some(previous) = merged.last_mut() {
            if segment.start < previous.end {
                previous.end = previous.end.max(segment.end);
                continue;
            }
        }
        merged.push(segment);
    }
    merged
}

fn loop_frequency(block: MachineBlockId, depth: u32) -> Result<f64, MachineIntervalError> {
    if depth > i32::MAX as u32 {
        return Err(MachineIntervalError::LoopWeightOverflow { block, depth });
    }
    let frequency = (PER_LEVEL as f64).powi(depth as i32);
    if frequency.is_finite() {
        Ok(frequency)
    } else {
        Err(MachineIntervalError::LoopWeightOverflow { block, depth })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        InstructionFlags, MachineBlock, MachineCallingConvention, MachineFunctionId,
        MachineInstruction, MachineLinkage, MachineOperand, MachineSignature, OperandRole,
        RegisterClass, TargetOpcode, VirtualRegister,
    };

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

    fn instruction(id: u32, operands: Vec<MachineOperand>) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(id),
            opcode: TargetOpcode::new(0),
            operands,
            flags: InstructionFlags::NONE,
        }
    }

    fn function(blocks: Vec<MachineBlock>, registers: &[u32]) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(0),
            name: "intervals".to_owned(),
            linkage: MachineLinkage::Internal,
            signature: MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::FarPascal,
            },
            entry: blocks
                .first()
                .map(|block| block.id)
                .unwrap_or(MachineBlockId::new(0)),
            virtual_registers: registers
                .iter()
                .map(|id| VirtualRegister {
                    id: VirtualRegisterId::new(*id),
                    class: RegisterClass::new(0),
                })
                .collect(),
            blocks,
            frame_objects: Vec::new(),
        }
    }

    #[test]
    fn cfg_hole_forms_segmented_range_as_python_ranges_do() {
        // Translation of `intervals._ranges()`: layout order has a block
        // between a definition and its CFG successor, so the interval must
        // have two segments rather than claim the intervening block live.
        let function = function(
            vec![
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: vec![instruction(0, vec![virtual_register(0, OperandRole::Def)])],
                    successors: vec![MachineBlockId::new(2)],
                },
                MachineBlock {
                    id: MachineBlockId::new(1),
                    instructions: vec![instruction(1, vec![virtual_register(1, OperandRole::Def)])],
                    successors: Vec::new(),
                },
                MachineBlock {
                    id: MachineBlockId::new(2),
                    instructions: vec![instruction(2, vec![virtual_register(0, OperandRole::Use)])],
                    successors: Vec::new(),
                },
            ],
            &[0, 1],
        );

        let indexes = index_intervals(&function).expect("well-formed instruction IDs");
        assert_eq!(
            indexes.block_span(MachineBlockId::new(0)),
            Some(LiveSegment { start: 0, end: 4 })
        );
        assert_eq!(
            indexes.block_span(MachineBlockId::new(2)),
            Some(LiveSegment { start: 8, end: 12 })
        );
        assert_eq!(
            live_intervals(&function).expect("well-formed Machine IR")[&VirtualRegisterId::new(0)]
                .segments,
            vec![
                LiveSegment { start: 3, end: 4 },
                LiveSegment { start: 8, end: 11 },
            ]
        );
    }

    #[test]
    fn use_precedes_definition_at_the_same_instruction_boundary() {
        // Translation of the `USE`/`DEF` invariant in intervals.py: an input
        // ending at DEF and an output beginning there do not overlap.
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(
                    0,
                    vec![
                        virtual_register(0, OperandRole::Use),
                        virtual_register(1, OperandRole::Def),
                    ],
                )],
                successors: Vec::new(),
            }],
            &[0, 1],
        );

        let ranges = live_intervals(&function).expect("well-formed Machine IR");
        assert_eq!(
            ranges[&VirtualRegisterId::new(0)].segments,
            vec![LiveSegment { start: 0, end: 3 }]
        );
        assert_eq!(
            ranges[&VirtualRegisterId::new(1)].segments,
            vec![LiveSegment { start: 3, end: 4 }]
        );
        assert!(!ranges[&VirtualRegisterId::new(0)].overlaps(&ranges[&VirtualRegisterId::new(1)]));
    }

    #[test]
    fn half_open_overlap_retains_touching_boundaries() {
        // Translation of tests/test_flow.py::test_the_coalescer_joins_the_intervals_it_merges.
        let left = LiveInterval {
            register: VirtualRegisterId::new(0),
            segments: vec![LiveSegment { start: 0, end: 15 }],
            weight: 0.0,
        };
        let right = LiveInterval {
            register: VirtualRegisterId::new(1),
            segments: vec![LiveSegment { start: 15, end: 16 }],
            weight: 0.0,
        };

        assert!(!left.overlaps(&right));
        assert!(left.contains(14));
        assert!(!left.contains(15));
        assert_eq!(
            merge_segments(vec![
                LiveSegment { start: 15, end: 16 },
                LiveSegment { start: 0, end: 15 },
            ]),
            vec![
                LiveSegment { start: 0, end: 15 },
                LiveSegment { start: 15, end: 16 }
            ]
        );
    }

    #[test]
    fn loop_depth_prices_each_reference_by_python_per_level() {
        // Translation of tests/test_flow.py::test_a_value_in_a_loop_costs_more_than_one_outside.
        let function = function(
            vec![
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: vec![
                        instruction(0, vec![virtual_register(0, OperandRole::Def)]),
                        instruction(1, vec![virtual_register(0, OperandRole::Use)]),
                    ],
                    successors: Vec::new(),
                },
                MachineBlock {
                    id: MachineBlockId::new(1),
                    instructions: vec![
                        instruction(2, vec![virtual_register(1, OperandRole::Def)]),
                        instruction(3, vec![virtual_register(1, OperandRole::Use)]),
                    ],
                    successors: Vec::new(),
                },
            ],
            &[0, 1],
        );

        let ranges = weighted_live_intervals(&function, |block| {
            u32::from(block == MachineBlockId::new(1))
        })
        .expect("well-formed Machine IR and finite loop depth");
        let outside = &ranges[&VirtualRegisterId::new(0)];
        let inside = &ranges[&VirtualRegisterId::new(1)];
        assert_eq!(outside.size(), 2);
        assert_eq!(inside.size(), 2);
        assert_eq!(outside.weight, 2.0 / (2.0 + GRACE as f64));
        assert_eq!(
            inside.weight,
            (2.0 * PER_LEVEL as f64) / (2.0 + GRACE as f64)
        );
        assert!(inside.weight > outside.weight);
    }

    #[test]
    fn repeated_operands_and_use_defs_each_count_as_references() {
        // Translation of Python `_weights()`'s `(*one.defines, *one.uses)`.
        // In particular, tests/test_spiller.py relies on duplicate uses
        // counting twice; a UseDef contributes once to each tuple.
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(
                    0,
                    vec![
                        virtual_register(0, OperandRole::UseDef),
                        virtual_register(0, OperandRole::Use),
                    ],
                )],
                successors: Vec::new(),
            }],
            &[0],
        );

        let interval = &weighted_live_intervals(&function, |_| 0).expect("well-formed Machine IR")
            [&VirtualRegisterId::new(0)];
        assert_eq!(interval.size(), 4);
        assert_eq!(interval.weight, 3.0 / (4.0 + GRACE as f64));
    }

    #[test]
    fn duplicate_instruction_ids_are_refused_instead_of_reindexed() {
        let function = function(
            vec![
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: vec![instruction(0, Vec::new())],
                    successors: Vec::new(),
                },
                MachineBlock {
                    id: MachineBlockId::new(1),
                    instructions: vec![instruction(0, Vec::new())],
                    successors: Vec::new(),
                },
            ],
            &[],
        );

        assert_eq!(
            index_intervals(&function),
            Err(vec![MachineIntervalError::DuplicateInstructionId {
                instruction: MachineInstructionId::new(0),
                first_block: MachineBlockId::new(0),
                duplicate_block: MachineBlockId::new(1),
            }])
        );
    }

    #[test]
    fn duplicate_block_ids_are_refused_instead_of_overwriting_spans() {
        let function = function(
            vec![
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: Vec::new(),
                    successors: Vec::new(),
                },
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: Vec::new(),
                    successors: Vec::new(),
                },
            ],
            &[],
        );

        assert_eq!(
            index_intervals(&function),
            Err(vec![MachineIntervalError::DuplicateBlockId {
                block: MachineBlockId::new(0),
            }])
        );
    }
}
