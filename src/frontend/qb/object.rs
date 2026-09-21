//! Microsoft BASIC's object envelope around target-neutral OMF facts.
//!
//! Generic MC and OMF deliberately know nothing about BASIC's runtime
//! segments. This adapter adds only the measured module envelope after x86
//! lowering has produced one code segment with the 48-byte `MODULE_CODE`
//! prefix already attached.

use std::error::Error;
use std::fmt;

use crate::frontend::qb::module_header::{MODULE_HEADER_SIZE, MODULE_SIGNATURE, object_name};
use crate::hir::{Program, RuntimeProfile};
use crate::object::omf::fixups::{FixupMode, Location};
use crate::object::omf::segments::{Alignment, Combine};
use crate::object::omf::write::{
    InitializedSpan, ObjectGroup, ObjectModule, ObjectRelocation, ObjectSegment, RelocationFrame,
    RelocationTarget,
};

const CODE_SEGMENT: u16 = 1;
const DGROUP: u16 = 1;
const FIRST_DGROUP_SEGMENT: u16 = 2;
const COMMON: u16 = 4;
const BC_DATA: u16 = 5;
const BC_FT: u16 = 8;
const BC_CN: u16 = 9;
const BC_DS: u16 = 10;
const BC_SA: u16 = 12;
const FSL_CONST: u16 = 14;

const GENERIC_CODE: &[u8] = b".text";
const GENERIC_BC_DATA: &[u8] = b"BC_DATA";
const GENERIC_BC_CN: &[u8] = b"BC_CN";
const GENERIC_FSL_CONST: &[u8] = b"FSL_CONST";

const HEADER_RELOCATIONS: [(u32, u16); 5] = [
    (12, BC_DS),
    (14, BC_DATA),
    (16, BC_FT),
    (24, COMMON),
    (32, BC_CN),
];

/// Why generic OMF facts cannot receive the BASIC runtime envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectEnvelopeError {
    ProgramModuleCount {
        count: usize,
    },
    GenericSegmentCount {
        count: usize,
    },
    UnexpectedGenericSegment {
        index: usize,
        expected: &'static [u8],
        actual: Vec<u8>,
    },
    ExistingGroups {
        count: usize,
    },
    ExistingNonCodePublic {
        segment: u16,
    },
    MissingModuleHeader,
    InvalidModuleSignature {
        actual: [u8; 2],
    },
    ExistingHeaderRelocation {
        offset: u32,
    },
    UnsupportedRelocationTarget {
        source: u16,
        target: u16,
    },
    DgroupFarPointer {
        source: u16,
        target: u16,
    },
    InvalidStatementTable {
        offset: u32,
        code_length: u32,
    },
    ModuleName(crate::frontend::qb::module_header::ModuleHeaderError),
}

impl fmt::Display for ObjectEnvelopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProgramModuleCount { count } => write!(
                formatter,
                "a BASIC object envelope requires exactly one HIR module, found {count}"
            ),
            Self::GenericSegmentCount { count } => write!(
                formatter,
                "generic BASIC OMF must contain the required preplaced segments, found {count}"
            ),
            Self::UnexpectedGenericSegment {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "generic BASIC segment {} is {:?}, expected {:?}",
                index + 1,
                actual,
                expected
            ),
            Self::ExistingGroups { count } => write!(
                formatter,
                "generic OMF already contains {count} groups; BASIC must establish DGROUP"
            ),
            Self::ExistingNonCodePublic { segment } => write!(
                formatter,
                "generic public symbol names segment {segment}, not the sole code segment"
            ),
            Self::MissingModuleHeader => write!(
                formatter,
                "generic code does not begin with a complete initialized MODULE_CODE header"
            ),
            Self::InvalidModuleSignature { actual } => write!(
                formatter,
                "generic code begins with module signature {:02x?}, not {:02x?}",
                actual, MODULE_SIGNATURE
            ),
            Self::ExistingHeaderRelocation { offset } => write!(
                formatter,
                "generic code already relocates MODULE_CODE field {offset:#x}"
            ),
            Self::UnsupportedRelocationTarget { source, target } => write!(
                formatter,
                "generic BASIC segment {source} relocation targets unsupported segment {target}"
            ),
            Self::DgroupFarPointer { source, target } => write!(
                formatter,
                "generic BASIC segment {source} has a far pointer relocation to DGROUP segment {target}"
            ),
            Self::InvalidStatementTable {
                offset,
                code_length,
            } => write!(
                formatter,
                "statement table offset {offset:#x} is outside BASIC code {:#x}..{code_length:#x}",
                MODULE_HEADER_SIZE
            ),
            Self::ModuleName(error) => error.fmt(formatter),
        }
    }
}

impl Error for ObjectEnvelopeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ModuleName(error) => Some(error),
            _ => None,
        }
    }
}

/// Add the Microsoft BASIC segment and relocation envelope to generic OMF.
///
/// The MC adapter has already placed the initialized-data forms it can
/// represent in source-owned generic segments. This frontend boundary retains
/// those bytes and gives them the measured BASIC runtime positions, classes,
/// and group-relative fixups. Unsupported relocation forms remain refusals.
pub fn add_object_envelope(
    module: &ObjectModule,
    program: &Program,
    statement_table_offset: u32,
) -> Result<ObjectModule, ObjectEnvelopeError> {
    let [hir_module] = program.modules.as_slice() else {
        return Err(ObjectEnvelopeError::ProgramModuleCount {
            count: program.modules.len(),
        });
    };
    let expected_segments: &[&[u8]] = match program.runtime {
        RuntimeProfile::Vbdos => &[
            GENERIC_CODE,
            GENERIC_BC_DATA,
            GENERIC_BC_CN,
            GENERIC_FSL_CONST,
        ],
        _ => &[GENERIC_CODE, GENERIC_BC_DATA, GENERIC_BC_CN],
    };
    if module.segments.len() != expected_segments.len() {
        return Err(ObjectEnvelopeError::GenericSegmentCount {
            count: module.segments.len(),
        });
    }
    for (index, (segment, expected)) in module
        .segments
        .iter()
        .zip(expected_segments.iter())
        .enumerate()
    {
        if segment.name != *expected {
            return Err(ObjectEnvelopeError::UnexpectedGenericSegment {
                index,
                expected,
                actual: segment.name.clone(),
            });
        }
    }
    if !module.groups.is_empty() {
        return Err(ObjectEnvelopeError::ExistingGroups {
            count: module.groups.len(),
        });
    }
    if let Some(public) = module
        .publics
        .iter()
        .find(|public| public.segment_index != CODE_SEGMENT)
    {
        return Err(ObjectEnvelopeError::ExistingNonCodePublic {
            segment: public.segment_index,
        });
    }

    let mut generic = module.segments.clone().into_iter();
    let mut code = generic.next().expect("checked generic code segment count");
    let mut generic_bc_data = generic
        .next()
        .expect("checked generic BC_DATA segment count");
    let mut generic_bc_cn = generic.next().expect("checked generic BC_CN segment count");
    let mut generic_fsl_const = generic.next();
    let remap = generic_segment_remap(program.runtime);
    remap_segment_relocations(&mut code, CODE_SEGMENT, &remap)?;
    remap_segment_relocations(&mut generic_bc_data, BC_DATA, &remap)?;
    remap_segment_relocations(&mut generic_bc_cn, BC_CN, &remap)?;
    if let Some(segment) = &mut generic_fsl_const {
        remap_segment_relocations(segment, FSL_CONST, &remap)?;
    }
    if statement_table_offset < MODULE_HEADER_SIZE as u32
        || statement_table_offset > u32::from(u16::MAX)
        || statement_table_offset >= code.length
    {
        return Err(ObjectEnvelopeError::InvalidStatementTable {
            offset: statement_table_offset,
            code_length: code.length,
        });
    }
    if let Some(relocation) = code
        .relocations
        .iter()
        .find(|relocation| relocation.offset < MODULE_HEADER_SIZE as u32)
    {
        return Err(ObjectEnvelopeError::ExistingHeaderRelocation {
            offset: relocation.offset,
        });
    }
    let header = code
        .initialized
        .iter_mut()
        .find(|span| span.offset == 0 && span.bytes.len() >= MODULE_HEADER_SIZE)
        .ok_or(ObjectEnvelopeError::MissingModuleHeader)?;
    let actual = [header.bytes[0], header.bytes[1]];
    if actual != MODULE_SIGNATURE {
        return Err(ObjectEnvelopeError::InvalidModuleSignature { actual });
    }

    header.bytes[10..12].copy_from_slice(&(statement_table_offset as u16).to_le_bytes());
    code.name = format!(
        "{}_CODE",
        object_name(&hir_module.name).map_err(ObjectEnvelopeError::ModuleName)?
    )
    .into_bytes();
    code.class_name = b"BC_CODE".to_vec();
    code.alignment = Alignment::Paragraph;
    code.combine = Combine::Public;
    code.relocations
        .push(offset_relocation(10, RelocationFrame::Target, CODE_SEGMENT));
    code.relocations.extend(
        HEADER_RELOCATIONS.map(|(offset, target)| {
            offset_relocation(offset, RelocationFrame::Group(DGROUP), target)
        }),
    );
    code.relocations.sort_by_key(|relocation| relocation.offset);

    generic_bc_data.class_name = b"BC_DATA".to_vec();
    generic_bc_data.alignment = Alignment::Word;
    generic_bc_data.combine = Combine::Public;
    generic_bc_cn.class_name = b"BC_SEGS".to_vec();
    generic_bc_cn.alignment = Alignment::Paragraph;
    generic_bc_cn.combine = Combine::Public;

    let mut segments = vec![code];
    segments.extend([
        empty_segment(b"BR_DATA", b"BLANK", Alignment::Paragraph, Combine::Public),
        empty_segment(b"BR_SKYS", b"BLANK", Alignment::Paragraph, Combine::Public),
        empty_segment(b"COMMON", b"BLANK", Alignment::Paragraph, Combine::Common),
        generic_bc_data,
        empty_segment(b"NMALLOC", b"BC_VARS", Alignment::Word, Combine::Common),
        empty_segment(b"ENMALLOC", b"BC_VARS", Alignment::Word, Combine::Common),
        empty_segment(b"BC_FT", b"BC_SEGS", Alignment::Word, Combine::Public),
        generic_bc_cn,
        data_segment(
            b"BC_DS",
            b"BC_SEGS",
            Alignment::Paragraph,
            Combine::Public,
            vec![0xff, 0xff, 0x01],
        ),
        empty_segment(b"BC_SAB", b"BC_SEGS", Alignment::Word, Combine::Public),
        bc_sa_segment(),
    ]);
    if program.runtime == RuntimeProfile::Vbdos {
        let mut fsl_const = generic_fsl_const.expect("checked VBDOS FSL_CONST segment count");
        fsl_const.class_name = b"FAR_DATA".to_vec();
        fsl_const.alignment = Alignment::Paragraph;
        fsl_const.combine = Combine::Private;
        segments.extend([
            empty_segment(
                b"FDATA",
                b"FAR_DATA",
                Alignment::Paragraph,
                Combine::Private,
            ),
            fsl_const,
        ]);
    }
    Ok(ObjectModule {
        name: module.name.clone(),
        segments,
        groups: vec![ObjectGroup {
            name: b"DGROUP".to_vec(),
            members: (FIRST_DGROUP_SEGMENT..=BC_SA).collect(),
        }],
        externals: module.externals.clone(),
        publics: remap_publics(&module.publics, &remap)?,
    })
}

fn generic_segment_remap(runtime: RuntimeProfile) -> Vec<u16> {
    let mut remap = vec![0, CODE_SEGMENT, BC_DATA, BC_CN];
    if runtime == RuntimeProfile::Vbdos {
        remap.push(FSL_CONST);
    }
    remap
}

fn remap_publics(
    publics: &[crate::object::omf::write::PublicSymbol],
    remap: &[u16],
) -> Result<Vec<crate::object::omf::write::PublicSymbol>, ObjectEnvelopeError> {
    publics
        .iter()
        .cloned()
        .map(|mut public| {
            public.segment_index = remap_target(CODE_SEGMENT, public.segment_index, remap)?;
            Ok(public)
        })
        .collect()
}

fn remap_segment_relocations(
    segment: &mut ObjectSegment,
    final_source: u16,
    remap: &[u16],
) -> Result<(), ObjectEnvelopeError> {
    for relocation in &mut segment.relocations {
        let RelocationTarget::Segment(target) = relocation.target else {
            continue;
        };
        let target = remap_target(final_source, target, remap)?;
        if relocation.location == Location::Pointer16_16 && is_dgroup_member(target) {
            return Err(ObjectEnvelopeError::DgroupFarPointer {
                source: final_source,
                target,
            });
        }
        relocation.target = RelocationTarget::Segment(target);
        relocation.frame = if relocation.location == Location::Offset16 && is_dgroup_member(target)
        {
            RelocationFrame::Group(DGROUP)
        } else {
            RelocationFrame::Target
        };
    }
    Ok(())
}

fn remap_target(source: u16, target: u16, remap: &[u16]) -> Result<u16, ObjectEnvelopeError> {
    remap
        .get(usize::from(target))
        .copied()
        .filter(|target| *target != 0)
        .ok_or(ObjectEnvelopeError::UnsupportedRelocationTarget { source, target })
}

fn is_dgroup_member(segment: u16) -> bool {
    (FIRST_DGROUP_SEGMENT..=BC_SA).contains(&segment)
}

fn offset_relocation(offset: u32, frame: RelocationFrame, target: u16) -> ObjectRelocation {
    ObjectRelocation {
        offset,
        location: Location::Offset16,
        mode: FixupMode::SegmentRelative,
        frame,
        target: RelocationTarget::Segment(target),
    }
}

fn empty_segment(
    name: &[u8],
    class_name: &[u8],
    alignment: Alignment,
    combine: Combine,
) -> ObjectSegment {
    ObjectSegment {
        name: name.to_vec(),
        class_name: class_name.to_vec(),
        alignment,
        combine,
        length: 0,
        initialized: Vec::new(),
        relocations: Vec::new(),
    }
}

fn data_segment(
    name: &[u8],
    class_name: &[u8],
    alignment: Alignment,
    combine: Combine,
    bytes: Vec<u8>,
) -> ObjectSegment {
    ObjectSegment {
        name: name.to_vec(),
        class_name: class_name.to_vec(),
        alignment,
        combine,
        length: bytes.len() as u32,
        initialized: vec![InitializedSpan { offset: 0, bytes }],
        relocations: Vec::new(),
    }
}

fn bc_sa_segment() -> ObjectSegment {
    let mut segment = data_segment(
        b"BC_SA",
        b"BC_SEGS",
        Alignment::Word,
        Combine::Public,
        vec![0; 4],
    );
    segment.relocations.push(ObjectRelocation {
        offset: 0,
        location: Location::Pointer16_16,
        mode: FixupMode::SegmentRelative,
        frame: RelocationFrame::Target,
        target: RelocationTarget::Segment(CODE_SEGMENT),
    });
    segment
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::qb::module_header::module_header;
    use crate::hir::{
        ArrayOrder, Dialect, FORMAT_VERSION, FloatMode, Module, ModuleId, RuntimeProfile,
        TargetProfile,
    };
    use crate::object::omf::fixups::{FrameDatum, FrameMethod, TargetMethod};
    use crate::object::omf::module::DecodedModule;
    use crate::object::omf::record;
    use crate::object::omf::write::{PublicSymbol, to_bytes};

    fn program(runtime: RuntimeProfile) -> Program {
        Program {
            version: FORMAT_VERSION,
            dialect: Dialect::Qb45,
            runtime,
            target: TargetProfile::I386RealMode,
            array_order: ArrayOrder::ColumnMajor,
            float_mode: FloatMode::Inline,
            modules: vec![Module {
                id: ModuleId::new(1),
                name: "emission".to_owned(),
                types: Vec::new(),
                functions: Vec::new(),
                data: Vec::new(),
                callables: Vec::new(),
            }],
        }
    }

    fn generic(program: &Program) -> ObjectModule {
        let mut code = module_header(program).unwrap().to_vec();
        code.extend_from_slice(&[0x90; 16]);
        let mut object = ObjectModule {
            name: b"emission.bas".to_vec(),
            segments: vec![
                ObjectSegment {
                    name: GENERIC_CODE.to_vec(),
                    class_name: b"CODE".to_vec(),
                    alignment: Alignment::Byte,
                    combine: Combine::Public,
                    length: code.len() as u32,
                    initialized: vec![InitializedSpan {
                        offset: 0,
                        bytes: code,
                    }],
                    relocations: Vec::new(),
                },
                data_segment(
                    GENERIC_BC_DATA,
                    b"DATA",
                    Alignment::Byte,
                    Combine::Private,
                    vec![0; 6],
                ),
                empty_segment(GENERIC_BC_CN, b"DATA", Alignment::Byte, Combine::Private),
            ],
            groups: Vec::new(),
            externals: Vec::new(),
            publics: vec![PublicSymbol {
                name: b"ADDONE".to_vec(),
                group_index: 0,
                segment_index: 1,
                offset: 48,
            }],
        };
        if program.runtime == RuntimeProfile::Vbdos {
            object.segments.push(empty_segment(
                GENERIC_FSL_CONST,
                b"DATA",
                Alignment::Byte,
                Combine::Public,
            ));
        }
        object
    }

    fn scalar_vbdos_generic(program: &Program) -> ObjectModule {
        let mut object = generic(program);
        object.segments[0].relocations.push(ObjectRelocation {
            offset: 48,
            location: Location::Offset16,
            mode: FixupMode::SegmentRelative,
            frame: RelocationFrame::Target,
            target: RelocationTarget::Segment(2),
        });
        object.segments[1] = data_segment(
            GENERIC_BC_DATA,
            b"DATA",
            Alignment::Byte,
            Combine::Private,
            vec![0, 0],
        );
        object.segments[1].relocations.push(ObjectRelocation {
            offset: 0,
            location: Location::Offset16,
            mode: FixupMode::SegmentRelative,
            frame: RelocationFrame::Target,
            target: RelocationTarget::Segment(3),
        });
        object.segments[2] = data_segment(
            GENERIC_BC_CN,
            b"DATA",
            Alignment::Byte,
            Combine::Private,
            vec![0, 0, 0, 0],
        );
        object.segments[2].relocations.extend([
            ObjectRelocation {
                offset: 0,
                location: Location::Offset16,
                mode: FixupMode::SegmentRelative,
                frame: RelocationFrame::Target,
                target: RelocationTarget::Segment(2),
            },
            ObjectRelocation {
                offset: 2,
                location: Location::Base16,
                mode: FixupMode::SegmentRelative,
                frame: RelocationFrame::Target,
                target: RelocationTarget::Segment(4),
            },
        ]);
        object.segments[3] = data_segment(
            GENERIC_FSL_CONST,
            b"DATA",
            Alignment::Byte,
            Combine::Public,
            vec![0x3f],
        );
        object
    }

    #[test]
    fn emits_the_measured_basic_segments_group_and_fixups() {
        let program = program(RuntimeProfile::Qb45);
        let object = add_object_envelope(&generic(&program), &program, 52).unwrap();
        let expected_names = [
            "EMISSION_CODE",
            "BR_DATA",
            "BR_SKYS",
            "COMMON",
            "BC_DATA",
            "NMALLOC",
            "ENMALLOC",
            "BC_FT",
            "BC_CN",
            "BC_DS",
            "BC_SAB",
            "BC_SA",
        ];
        assert_eq!(
            object
                .segments
                .iter()
                .map(|segment| String::from_utf8_lossy(&segment.name).into_owned())
                .collect::<Vec<_>>(),
            expected_names
        );
        assert_eq!(object.groups[0].members, (2..=12).collect::<Vec<_>>());
        assert_eq!(object.segments[4].initialized[0].bytes, [0; 6]);
        assert_eq!(object.segments[9].initialized[0].bytes, [0xff, 0xff, 1]);
        assert_eq!(
            &object.segments[0].initialized[0].bytes[10..18],
            &[52, 0, 2, 0, 0, 0, 0, 0]
        );

        let bytes = to_bytes(&object).unwrap();
        let records = record::parse(&bytes).unwrap();
        let decoded = DecodedModule::parse(&records).unwrap();
        let segment_names = decoded
            .segments
            .segments
            .iter()
            .skip(1)
            .map(|segment| {
                let segment = segment.as_ref().unwrap();
                decoded.symbols.names[usize::from(segment.name_index)].clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            segment_names,
            expected_names.map(|name| name.as_bytes().to_vec())
        );
        assert_eq!(decoded.declarations.groups.len(), 1);
        assert_eq!(decoded.fixups.len(), 7);
        let statement = decoded
            .fixups
            .iter()
            .find(|fixup| fixup.segment_index == 1 && fixup.patch_offset == 10)
            .unwrap();
        assert_eq!(statement.location, Location::Offset16);
        assert_eq!(statement.frame.method, FrameMethod::Target);
        assert_eq!(statement.target.method, TargetMethod::Segment);
        assert_eq!(statement.target.datum, 1);
        for (offset, target) in [(12, 10), (14, 5), (16, 8), (24, 4), (32, 9)] {
            let fixup = decoded
                .fixups
                .iter()
                .find(|fixup| fixup.segment_index == 1 && fixup.patch_offset == offset)
                .unwrap();
            assert_eq!(fixup.frame.method, FrameMethod::Group);
            assert_eq!(fixup.frame.datum, Some(FrameDatum::Index(1)));
            assert_eq!(fixup.target.method, TargetMethod::Segment);
            assert_eq!(fixup.target.datum, target);
        }
        let registration = decoded
            .fixups
            .iter()
            .find(|fixup| fixup.segment_index == 12)
            .unwrap();
        assert_eq!(registration.location, Location::Pointer16_16);
        assert_eq!(registration.frame.method, FrameMethod::Target);
        assert_eq!(registration.target.datum, 1);
    }

    #[test]
    fn vbdos_adds_private_far_segments_outside_dgroup() {
        let program = program(RuntimeProfile::Vbdos);
        let object = add_object_envelope(&generic(&program), &program, 48).unwrap();
        assert_eq!(object.segments[12].name, b"FDATA");
        assert_eq!(object.segments[12].combine, Combine::Private);
        assert_eq!(object.segments[13].name, b"FSL_CONST");
        assert_eq!(object.groups[0].members, (2..=12).collect::<Vec<_>>());
    }

    #[test]
    fn preserves_preplaced_vbdos_data_and_remaps_its_relocations() {
        let program = program(RuntimeProfile::Vbdos);
        let object = add_object_envelope(&scalar_vbdos_generic(&program), &program, 52).unwrap();
        assert_eq!(
            object
                .segments
                .iter()
                .map(|segment| segment.name.as_slice())
                .collect::<Vec<_>>(),
            [
                b"EMISSION_CODE".as_slice(),
                b"BR_DATA",
                b"BR_SKYS",
                b"COMMON",
                b"BC_DATA",
                b"NMALLOC",
                b"ENMALLOC",
                b"BC_FT",
                b"BC_CN",
                b"BC_DS",
                b"BC_SAB",
                b"BC_SA",
                b"FDATA",
                b"FSL_CONST",
            ]
        );
        assert_eq!(object.groups[0].members, (2..=12).collect::<Vec<_>>());
        assert_eq!(object.segments[4].initialized[0].bytes, [0, 0]);
        assert_eq!(object.segments[8].initialized[0].bytes, [0, 0, 0, 0]);
        assert_eq!(object.segments[13].initialized[0].bytes, [0x3f]);
        assert_eq!(object.segments[13].class_name, b"FAR_DATA");
        assert_eq!(object.segments[13].combine, Combine::Private);

        let code_data = object.segments[0]
            .relocations
            .iter()
            .find(|relocation| relocation.offset == 48)
            .unwrap();
        assert_eq!(code_data.target, RelocationTarget::Segment(BC_DATA));
        assert_eq!(code_data.frame, RelocationFrame::Group(DGROUP));
        let data_constant = &object.segments[4].relocations[0];
        assert_eq!(data_constant.target, RelocationTarget::Segment(BC_CN));
        assert_eq!(data_constant.frame, RelocationFrame::Group(DGROUP));
        let descriptor_data = &object.segments[8].relocations[0];
        assert_eq!(descriptor_data.target, RelocationTarget::Segment(BC_DATA));
        assert_eq!(descriptor_data.frame, RelocationFrame::Group(DGROUP));
        let fsl_segment = &object.segments[8].relocations[1];
        assert_eq!(fsl_segment.location, Location::Base16);
        assert_eq!(fsl_segment.target, RelocationTarget::Segment(FSL_CONST));
        assert_eq!(fsl_segment.frame, RelocationFrame::Target);

        assert_eq!(
            &object.segments[0].initialized[0].bytes[10..18],
            &[52, 0, 2, 0, 0, 0, 0, 0]
        );
        let statement = object.segments[0]
            .relocations
            .iter()
            .find(|relocation| relocation.offset == 10)
            .unwrap();
        assert_eq!(statement.frame, RelocationFrame::Target);
        assert_eq!(statement.target, RelocationTarget::Segment(CODE_SEGMENT));
        assert_eq!(
            object.segments[11].relocations[0].location,
            Location::Pointer16_16
        );
        assert_eq!(
            object.segments[11].relocations[0].frame,
            RelocationFrame::Target
        );
        assert_eq!(
            object.segments[11].relocations[0].target,
            RelocationTarget::Segment(CODE_SEGMENT)
        );
    }

    #[test]
    fn refuses_unmodeled_data_and_invalid_header_state() {
        let program = program(RuntimeProfile::Qb45);
        let mut with_data = generic(&program);
        with_data.segments[1].name = b"data".to_vec();
        assert!(matches!(
            add_object_envelope(&with_data, &program, 52),
            Err(ObjectEnvelopeError::UnexpectedGenericSegment { index: 1, .. })
        ));

        let mut data_public = generic(&program);
        data_public.publics.push(PublicSymbol {
            name: b"DATA".to_vec(),
            group_index: 0,
            segment_index: 2,
            offset: 0,
        });
        assert!(matches!(
            add_object_envelope(&data_public, &program, 52),
            Err(ObjectEnvelopeError::ExistingNonCodePublic { segment: 2 })
        ));

        let mut dgroup_far_pointer = generic(&program);
        dgroup_far_pointer.segments[1]
            .relocations
            .push(ObjectRelocation {
                offset: 0,
                location: Location::Pointer16_16,
                mode: FixupMode::SegmentRelative,
                frame: RelocationFrame::Target,
                target: RelocationTarget::Segment(3),
            });
        assert!(matches!(
            add_object_envelope(&dgroup_far_pointer, &program, 52),
            Err(ObjectEnvelopeError::DgroupFarPointer {
                source: BC_DATA,
                target: BC_CN,
            })
        ));

        let mut unknown_target = generic(&program);
        unknown_target.segments[1]
            .relocations
            .push(ObjectRelocation {
                offset: 0,
                location: Location::Offset16,
                mode: FixupMode::SegmentRelative,
                frame: RelocationFrame::Target,
                target: RelocationTarget::Segment(4),
            });
        assert!(matches!(
            add_object_envelope(&unknown_target, &program, 52),
            Err(ObjectEnvelopeError::UnsupportedRelocationTarget {
                source: BC_DATA,
                target: 4,
            })
        ));

        let mut bad_header = generic(&program);
        bad_header.segments[0].initialized[0].bytes[0] = b'x';
        assert!(matches!(
            add_object_envelope(&bad_header, &program, 52),
            Err(ObjectEnvelopeError::InvalidModuleSignature { .. })
        ));

        assert!(matches!(
            add_object_envelope(&generic(&program), &program, 47),
            Err(ObjectEnvelopeError::InvalidStatementTable { .. })
        ));
    }
}
