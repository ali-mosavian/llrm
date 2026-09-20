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
    ProgramModuleCount { count: usize },
    GenericSegmentCount { count: usize },
    ExistingGroups { count: usize },
    ExistingNonCodePublic { segment: u16 },
    MissingModuleHeader,
    InvalidModuleSignature { actual: [u8; 2] },
    ExistingHeaderRelocation { offset: u32 },
    InvalidStatementTable { offset: u32, code_length: u32 },
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
                "the initial BASIC object slice requires one generic code segment, found {count}"
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
/// This initial vertical slice accepts only the single generic code segment
/// emitted for scalar programs. Source data placement remains a later QB
/// frontend responsibility and is refused instead of being guessed here.
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
    if module.segments.len() != 1 {
        return Err(ObjectEnvelopeError::GenericSegmentCount {
            count: module.segments.len(),
        });
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

    let mut output = module.clone();
    let code = &mut output.segments[0];
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

    output.segments.extend([
        empty_segment(b"BR_DATA", b"BLANK", Alignment::Paragraph, Combine::Public),
        empty_segment(b"BR_SKYS", b"BLANK", Alignment::Paragraph, Combine::Public),
        empty_segment(b"COMMON", b"BLANK", Alignment::Paragraph, Combine::Common),
        data_segment(
            b"BC_DATA",
            b"BC_DATA",
            Alignment::Word,
            Combine::Public,
            vec![0; 6],
        ),
        empty_segment(b"NMALLOC", b"BC_VARS", Alignment::Word, Combine::Common),
        empty_segment(b"ENMALLOC", b"BC_VARS", Alignment::Word, Combine::Common),
        empty_segment(b"BC_FT", b"BC_SEGS", Alignment::Word, Combine::Public),
        empty_segment(b"BC_CN", b"BC_SEGS", Alignment::Paragraph, Combine::Public),
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
        output.segments.extend([
            empty_segment(
                b"FDATA",
                b"FAR_DATA",
                Alignment::Paragraph,
                Combine::Private,
            ),
            empty_segment(
                b"FSL_CONST",
                b"FAR_DATA",
                Alignment::Paragraph,
                Combine::Private,
            ),
        ]);
    }
    output.groups.push(ObjectGroup {
        name: b"DGROUP".to_vec(),
        members: (FIRST_DGROUP_SEGMENT..=BC_SA).collect(),
    });
    Ok(output)
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
        ObjectModule {
            name: b"emission.bas".to_vec(),
            segments: vec![ObjectSegment {
                name: b".text".to_vec(),
                class_name: b"CODE".to_vec(),
                alignment: Alignment::Byte,
                combine: Combine::Public,
                length: code.len() as u32,
                initialized: vec![InitializedSpan {
                    offset: 0,
                    bytes: code,
                }],
                relocations: Vec::new(),
            }],
            groups: Vec::new(),
            externals: Vec::new(),
            publics: vec![PublicSymbol {
                name: b"ADDONE".to_vec(),
                group_index: 0,
                segment_index: 1,
                offset: 48,
            }],
        }
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
    fn refuses_unmodeled_data_and_invalid_header_state() {
        let program = program(RuntimeProfile::Qb45);
        let mut with_data = generic(&program);
        with_data.segments.push(empty_segment(
            b"data",
            b"DATA",
            Alignment::Word,
            Combine::Public,
        ));
        assert!(matches!(
            add_object_envelope(&with_data, &program, 52),
            Err(ObjectEnvelopeError::GenericSegmentCount { count: 2 })
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
