//! Microsoft BASIC runtime-frame planning for 16-bit x86.
//!
//! This is the target-owned equivalent of the Python source backend's
//! `qbopt/frontend/qb/compile.py::_runtime_frame` and the Pascal parameter
//! layout in `qbopt/frontend/qb/abi.py`.  It is deliberately an immutable
//! analysis: assigning BP displacements is separate from rewriting Machine IR
//! operands or emitting the `B$ENRA`/`B$EXSA` shell.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    FrameIndex, FrameObjectKind, MachineAddressSpace, MachineCallingConvention, MachineFunction,
    MachineFunctionId, MachineValueType,
};

const FAR_PASCAL_FIRST_ARGUMENT: u32 = 6;
const MAX_LOCAL_BYTES: u32 = 0x7ffe;

/// Microsoft BASIC runtime family whose frame entry owns BP and local storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BasicRuntime {
    Qb45,
    Pds71,
    Vbdos,
}

impl BasicRuntime {
    /// Bytes which the runtime owns below BP and above source locals.
    pub const fn frame_header_bytes(self) -> u16 {
        match self {
            Self::Qb45 => 10,
            Self::Pds71 => 18,
            Self::Vbdos => 20,
        }
    }
}

/// A complete, deterministic BP-relative plan for one BASIC procedure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicFramePlan {
    function: MachineFunctionId,
    runtime: BasicRuntime,
    offsets: BTreeMap<FrameIndex, i32>,
    local_bytes: u16,
    parameter_bytes: u16,
    temporary_strings: u16,
}

impl BasicFramePlan {
    pub const fn function(&self) -> MachineFunctionId {
        self.function
    }

    pub const fn runtime(&self) -> BasicRuntime {
        self.runtime
    }

    pub const fn header_bytes(&self) -> u16 {
        self.runtime.frame_header_bytes()
    }

    /// Value passed to `B$ENRA` in CX.
    pub const fn local_bytes(&self) -> u16 {
        self.local_bytes
    }

    /// Immediate consumed by the final `retf`, or zero for a bare `retf`.
    pub const fn parameter_bytes(&self) -> u16 {
        self.parameter_bytes
    }

    /// Value passed to `B$ENRA` in BX.
    pub const fn temporary_strings(&self) -> u16 {
        self.temporary_strings
    }

    /// Signed BP displacement for one abstract frame object.
    pub fn offset(&self, index: FrameIndex) -> Option<i32> {
        self.offsets.get(&index).copied()
    }

    pub fn offsets(&self) -> impl Iterator<Item = (FrameIndex, i32)> + '_ {
        self.offsets.iter().map(|(index, offset)| (*index, *offset))
    }
}

/// A function whose Machine-IR frame cannot be represented by the measured
/// Microsoft BASIC runtime convention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicFramePlanError {
    UnsupportedCallingConvention(MachineCallingConvention),
    VariadicFunction,
    UnsupportedParameter {
        parameter: usize,
        value_type: MachineValueType,
    },
    MissingIncomingArgument {
        parameter: usize,
    },
    DuplicateIncomingArgument {
        parameter: usize,
    },
    IncomingArgumentOutOfBounds {
        frame: FrameIndex,
        parameter: u32,
    },
    IncomingArgumentSize {
        frame: FrameIndex,
        parameter: usize,
        expected: u32,
        actual: u32,
    },
    DuplicateFrameIndex(FrameIndex),
    InvalidFrameObject {
        frame: FrameIndex,
        size: u32,
        alignment: u32,
    },
    UnsupportedOutgoingArgument(FrameIndex),
    TemporaryStringsTooLarge(u32),
    ParameterBytesTooLarge(u32),
    LocalReservationTooLarge(u32),
    ArithmeticOverflow,
}

impl fmt::Display for BasicFramePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedCallingConvention(convention) => write!(
                formatter,
                "BASIC frame planning does not support {convention:?} calling convention"
            ),
            Self::VariadicFunction => {
                write!(formatter, "a Microsoft BASIC procedure cannot be variadic")
            }
            Self::UnsupportedParameter {
                parameter,
                value_type,
            } => write!(
                formatter,
                "BASIC parameter {parameter} has unsupported ABI type {value_type:?}"
            ),
            Self::MissingIncomingArgument { parameter } => write!(
                formatter,
                "BASIC parameter {parameter} has no incoming frame object"
            ),
            Self::DuplicateIncomingArgument { parameter } => write!(
                formatter,
                "BASIC parameter {parameter} has multiple incoming frame objects"
            ),
            Self::IncomingArgumentOutOfBounds { frame, parameter } => write!(
                formatter,
                "frame index {frame} names incoming BASIC parameter {parameter} outside the signature"
            ),
            Self::IncomingArgumentSize {
                frame,
                parameter,
                expected,
                actual,
            } => write!(
                formatter,
                "frame index {frame} for BASIC parameter {parameter} has size {actual}, expected {expected}"
            ),
            Self::DuplicateFrameIndex(frame) => {
                write!(
                    formatter,
                    "BASIC frame contains duplicate frame index {frame}"
                )
            }
            Self::InvalidFrameObject {
                frame,
                size,
                alignment,
            } => write!(
                formatter,
                "frame index {frame} has invalid size {size} or alignment {alignment}"
            ),
            Self::UnsupportedOutgoingArgument(frame) => write!(
                formatter,
                "frame index {frame} is an outgoing argument; BASIC calls currently push arguments directly"
            ),
            Self::TemporaryStringsTooLarge(count) => write!(
                formatter,
                "BASIC frame requests {count} temporary string descriptors; maximum is 65535"
            ),
            Self::ParameterBytesTooLarge(bytes) => write!(
                formatter,
                "BASIC parameters occupy {bytes} bytes; maximum cleanup is 65535"
            ),
            Self::LocalReservationTooLarge(bytes) => write!(
                formatter,
                "BASIC local reservation is {bytes} bytes; maximum is {MAX_LOCAL_BYTES}"
            ),
            Self::ArithmeticOverflow => write!(formatter, "BASIC frame size arithmetic overflowed"),
        }
    }
}

impl Error for BasicFramePlanError {}

/// Plans the measured far-Pascal Microsoft BASIC frame without changing IR.
///
/// Source-order arguments are pushed left-to-right, so the first formal is
/// furthest from the return address and the last begins at `BP+6`.  Locals and
/// spills are placed below the runtime-family header in declared frame-object
/// order.  `B$ENRA` rounds its requested local reservation to a word.
pub fn plan_basic_frame(
    function: &MachineFunction,
    runtime: BasicRuntime,
    temporary_strings: u32,
) -> Result<BasicFramePlan, BasicFramePlanError> {
    if function.signature.calling_convention != MachineCallingConvention::FarPascal {
        return Err(BasicFramePlanError::UnsupportedCallingConvention(
            function.signature.calling_convention,
        ));
    }
    if function.signature.variadic {
        return Err(BasicFramePlanError::VariadicFunction);
    }
    let temporary_strings = u16::try_from(temporary_strings)
        .map_err(|_| BasicFramePlanError::TemporaryStringsTooLarge(temporary_strings))?;

    let parameter_widths = function
        .signature
        .parameters
        .iter()
        .copied()
        .enumerate()
        .map(|(parameter, value_type)| abi_width(parameter, value_type))
        .collect::<Result<Vec<_>, _>>()?;
    let parameter_bytes = parameter_widths.iter().try_fold(0_u32, |total, width| {
        total
            .checked_add(*width)
            .ok_or(BasicFramePlanError::ArithmeticOverflow)
    })?;
    let parameter_bytes_u16 = u16::try_from(parameter_bytes)
        .map_err(|_| BasicFramePlanError::ParameterBytesTooLarge(parameter_bytes))?;

    let mut incoming = BTreeMap::new();
    let mut frame_indices = BTreeSet::new();
    for frame in &function.frame_objects {
        if !frame_indices.insert(frame.index) {
            return Err(BasicFramePlanError::DuplicateFrameIndex(frame.index));
        }
        if frame.size == 0 || frame.alignment == 0 || !frame.alignment.is_power_of_two() {
            return Err(BasicFramePlanError::InvalidFrameObject {
                frame: frame.index,
                size: frame.size,
                alignment: frame.alignment,
            });
        }
        let FrameObjectKind::IncomingArgument { parameter } = frame.kind else {
            continue;
        };
        let parameter_index = usize::try_from(parameter).map_err(|_| {
            BasicFramePlanError::IncomingArgumentOutOfBounds {
                frame: frame.index,
                parameter,
            }
        })?;
        let Some(expected) = parameter_widths.get(parameter_index).copied() else {
            return Err(BasicFramePlanError::IncomingArgumentOutOfBounds {
                frame: frame.index,
                parameter,
            });
        };
        if frame.size != expected {
            return Err(BasicFramePlanError::IncomingArgumentSize {
                frame: frame.index,
                parameter: parameter_index,
                expected,
                actual: frame.size,
            });
        }
        if incoming.insert(parameter_index, frame.index).is_some() {
            return Err(BasicFramePlanError::DuplicateIncomingArgument {
                parameter: parameter_index,
            });
        }
    }
    for parameter in 0..parameter_widths.len() {
        if !incoming.contains_key(&parameter) {
            return Err(BasicFramePlanError::MissingIncomingArgument { parameter });
        }
    }

    let mut offsets = BTreeMap::new();
    let mut cursor = FAR_PASCAL_FIRST_ARGUMENT
        .checked_add(parameter_bytes)
        .ok_or(BasicFramePlanError::ArithmeticOverflow)?;
    for (parameter, width) in parameter_widths.iter().copied().enumerate() {
        cursor = cursor
            .checked_sub(width)
            .ok_or(BasicFramePlanError::ArithmeticOverflow)?;
        let frame = incoming[&parameter];
        let offset = i32::try_from(cursor).map_err(|_| BasicFramePlanError::ArithmeticOverflow)?;
        insert_offset(&mut offsets, frame, offset)?;
    }

    let header = u32::from(runtime.frame_header_bytes());
    let mut depth = 0_u32;
    let mut local_depths = Vec::new();
    for frame in &function.frame_objects {
        match frame.kind {
            FrameObjectKind::IncomingArgument { .. } => {}
            FrameObjectKind::Local | FrameObjectKind::Spill => {
                let capacity = frame.size.max(2);
                let end = depth
                    .checked_add(capacity)
                    .ok_or(BasicFramePlanError::ArithmeticOverflow)?;
                depth = align_up(end, frame.alignment)?;
                local_depths.push((frame.index, depth));
            }
            FrameObjectKind::OutgoingArgument => {
                return Err(BasicFramePlanError::UnsupportedOutgoingArgument(
                    frame.index,
                ));
            }
        }
    }
    let local_bytes = align_up(depth, 2)?;
    if local_bytes > MAX_LOCAL_BYTES {
        return Err(BasicFramePlanError::LocalReservationTooLarge(local_bytes));
    }
    for (frame, depth) in local_depths {
        let physical_depth = header
            .checked_add(depth)
            .ok_or(BasicFramePlanError::ArithmeticOverflow)?;
        let physical_depth =
            i32::try_from(physical_depth).map_err(|_| BasicFramePlanError::ArithmeticOverflow)?;
        insert_offset(&mut offsets, frame, -physical_depth)?;
    }
    let local_bytes = u16::try_from(local_bytes)
        .map_err(|_| BasicFramePlanError::LocalReservationTooLarge(local_bytes))?;

    Ok(BasicFramePlan {
        function: function.id,
        runtime,
        offsets,
        local_bytes,
        parameter_bytes: parameter_bytes_u16,
        temporary_strings,
    })
}

fn abi_width(parameter: usize, value_type: MachineValueType) -> Result<u32, BasicFramePlanError> {
    match value_type {
        MachineValueType::Integer { bits: 8 | 16 } => Ok(2),
        MachineValueType::Integer { bits: 32 } => Ok(4),
        MachineValueType::Pointer {
            bits: 16,
            address_space: MachineAddressSpace::NearData,
        } => Ok(2),
        _ => Err(BasicFramePlanError::UnsupportedParameter {
            parameter,
            value_type,
        }),
    }
}

fn align_up(value: u32, alignment: u32) -> Result<u32, BasicFramePlanError> {
    let mask = alignment
        .checked_sub(1)
        .ok_or(BasicFramePlanError::ArithmeticOverflow)?;
    value
        .checked_add(mask)
        .map(|value| value & !mask)
        .ok_or(BasicFramePlanError::ArithmeticOverflow)
}

fn insert_offset(
    offsets: &mut BTreeMap<FrameIndex, i32>,
    frame: FrameIndex,
    offset: i32,
) -> Result<(), BasicFramePlanError> {
    if offsets.insert(frame, offset).is_some() {
        return Err(BasicFramePlanError::DuplicateFrameIndex(frame));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        FrameObject, MachineBlock, MachineBlockId, MachineFunctionId, MachineLinkage,
        MachineSignature,
    };

    fn function(
        parameters: Vec<MachineValueType>,
        frame_objects: Vec<FrameObject>,
    ) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(0),
            name: "procedure".into(),
            linkage: MachineLinkage::External,
            signature: MachineSignature {
                result: Some(MachineValueType::Integer { bits: 32 }),
                parameters,
                variadic: false,
                calling_convention: MachineCallingConvention::FarPascal,
            },
            entry: MachineBlockId::new(0),
            virtual_registers: Vec::new(),
            blocks: vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: Vec::new(),
                successors: Vec::new(),
            }],
            frame_objects,
        }
    }

    fn incoming(index: u32, parameter: u32, size: u32) -> FrameObject {
        FrameObject {
            index: FrameIndex::new(index),
            size,
            alignment: 2,
            kind: FrameObjectKind::IncomingArgument { parameter },
        }
    }

    fn local(index: u32, size: u32, alignment: u32) -> FrameObject {
        FrameObject {
            index: FrameIndex::new(index),
            size,
            alignment,
            kind: FrameObjectKind::Local,
        }
    }

    #[test]
    fn preserves_measured_twice_parameter_and_local_offsets() {
        // BC's PDS object records Twice&(n AS LONG)'s incoming BYREF pointer
        // at BP+6 and its four-byte result local at BP-22.  Rebasing the
        // parameter below the runtime header made SYS read caller garbage.
        let procedure = function(
            vec![MachineValueType::Pointer {
                bits: 16,
                address_space: MachineAddressSpace::NearData,
            }],
            vec![incoming(0, 0, 2), local(1, 4, 1)],
        );

        let plan = plan_basic_frame(&procedure, BasicRuntime::Pds71, 0).unwrap();

        assert_eq!(plan.offset(FrameIndex::new(0)), Some(6));
        assert_eq!(plan.offset(FrameIndex::new(1)), Some(-22));
        assert_eq!(plan.parameter_bytes(), 2);
        assert_eq!(plan.local_bytes(), 4);
    }

    #[test]
    fn uses_profile_headers_and_large_vbdos_local_extent() {
        // The VBDOS 20-byte runtime header plus a 4096-byte local reaches
        // BP-4116.  Forgetting the header overwrote B$EXSA's frame link.
        let procedure = function(Vec::new(), vec![local(0, 4096, 1)]);
        let qb45 = plan_basic_frame(&procedure, BasicRuntime::Qb45, 0).unwrap();
        let pds = plan_basic_frame(&procedure, BasicRuntime::Pds71, 0).unwrap();
        let vbdos = plan_basic_frame(&procedure, BasicRuntime::Vbdos, 0).unwrap();

        assert_eq!(qb45.header_bytes(), 10);
        assert_eq!(pds.header_bytes(), 18);
        assert_eq!(vbdos.header_bytes(), 20);
        assert_eq!(vbdos.offset(FrameIndex::new(0)), Some(-4116));
        assert_eq!(vbdos.local_bytes(), 4096);
    }

    #[test]
    fn lays_out_pascal_parameters_in_source_push_order() {
        // SUBTRACTPAIR printed 8-50 when its two LONG formals were laid out
        // like C.  Pascal pushes source-left-to-right: first is BP+10 and
        // second is BP+6.
        let procedure = function(
            vec![
                MachineValueType::Integer { bits: 32 },
                MachineValueType::Integer { bits: 32 },
            ],
            vec![incoming(4, 0, 4), incoming(5, 1, 4)],
        );

        let plan = plan_basic_frame(&procedure, BasicRuntime::Vbdos, 0).unwrap();

        assert_eq!(plan.offset(FrameIndex::new(4)), Some(10));
        assert_eq!(plan.offset(FrameIndex::new(5)), Some(6));
        assert_eq!(plan.parameter_bytes(), 8);
    }

    #[test]
    fn gives_small_and_odd_objects_owned_word_rounded_storage() {
        let procedure = function(Vec::new(), vec![local(2, 1, 1), local(3, 3, 1)]);

        let plan = plan_basic_frame(&procedure, BasicRuntime::Qb45, 7).unwrap();

        assert_eq!(plan.offset(FrameIndex::new(2)), Some(-12));
        assert_eq!(plan.offset(FrameIndex::new(3)), Some(-15));
        assert_eq!(plan.local_bytes(), 6);
        assert_eq!(plan.temporary_strings(), 7);
    }

    #[test]
    fn refuses_missing_mismatched_and_unrepresentable_frame_facts() {
        let missing = function(vec![MachineValueType::Integer { bits: 16 }], Vec::new());
        assert_eq!(
            plan_basic_frame(&missing, BasicRuntime::Qb45, 0),
            Err(BasicFramePlanError::MissingIncomingArgument { parameter: 0 })
        );

        let mismatch = function(
            vec![MachineValueType::Integer { bits: 32 }],
            vec![incoming(0, 0, 2)],
        );
        assert!(matches!(
            plan_basic_frame(&mismatch, BasicRuntime::Qb45, 0),
            Err(BasicFramePlanError::IncomingArgumentSize {
                expected: 4,
                actual: 2,
                ..
            })
        ));

        let outgoing = function(
            Vec::new(),
            vec![FrameObject {
                index: FrameIndex::new(9),
                size: 2,
                alignment: 2,
                kind: FrameObjectKind::OutgoingArgument,
            }],
        );
        assert_eq!(
            plan_basic_frame(&outgoing, BasicRuntime::Qb45, 0),
            Err(BasicFramePlanError::UnsupportedOutgoingArgument(
                FrameIndex::new(9)
            ))
        );

        let too_large = function(Vec::new(), vec![local(0, 0x7fff, 1)]);
        assert_eq!(
            plan_basic_frame(&too_large, BasicRuntime::Qb45, 0),
            Err(BasicFramePlanError::LocalReservationTooLarge(0x8000))
        );
        assert_eq!(
            plan_basic_frame(
                &function(Vec::new(), Vec::new()),
                BasicRuntime::Qb45,
                0x1_0000
            ),
            Err(BasicFramePlanError::TemporaryStringsTooLarge(0x1_0000))
        );

        let mut c_function = function(Vec::new(), Vec::new());
        c_function.signature.calling_convention = MachineCallingConvention::C;
        assert_eq!(
            plan_basic_frame(&c_function, BasicRuntime::Qb45, 0),
            Err(BasicFramePlanError::UnsupportedCallingConvention(
                MachineCallingConvention::C
            ))
        );

        let far_pointer = function(
            vec![MachineValueType::Pointer {
                bits: 32,
                address_space: MachineAddressSpace::FarData,
            }],
            vec![incoming(0, 0, 4)],
        );
        assert!(matches!(
            plan_basic_frame(&far_pointer, BasicRuntime::Qb45, 0),
            Err(BasicFramePlanError::UnsupportedParameter { parameter: 0, .. })
        ));
    }
}
