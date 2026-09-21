//! QB-owned contributions to an already selected MC module.
//!
//! This adapter owns the runtime's initialized-data grouping and immutable
//! module prefix. Generic MC remains free of BASIC section conventions; final
//! OMF segment policy stays in the adjacent QB object adapter.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::codegen::machine::{MachineAddressSpace, MachineDataObject, MachineDataObjectId};
use crate::frontend::qb::module_header::MODULE_HEADER_SIZE;
use crate::hir::RuntimeProfile;
use crate::mc::{
    DataFragment, FragmentId, MCFragment, MCModule, MCSection, SectionFlags, SectionId,
    SectionKind, SymbolBinding, SymbolDefinition, SymbolId, SymbolVisibility,
};
use crate::support::diagnostic::Diagnostic;

const HEADER_SYMBOL: &str = "$QB$HEADER";
const DATA_PREFIX_SYMBOL: &str = "$QB$DATA";

/// Why the QB module header cannot be added to an MC module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleMcError {
    InputVerification(Vec<Diagnostic>),
    OutputVerification(Vec<Diagnostic>),
    MissingTextSection {
        section: SectionId,
    },
    NonTextSection {
        section: SectionId,
        kind: SectionKind,
    },
    NonExecutableTextSection {
        section: SectionId,
        flags: SectionFlags,
    },
    NonEmptySection {
        section: SectionId,
        kind: SectionKind,
    },
    ReservedSymbol {
        name: String,
    },
    MissingGenericSection {
        kind: SectionKind,
    },
    DuplicateGenericSection {
        kind: SectionKind,
    },
    UnexpectedGenericSection {
        section: SectionId,
        kind: SectionKind,
    },
    DuplicateDataObjectName {
        name: String,
    },
    MissingDataSymbol {
        data: MachineDataObjectId,
        name: String,
    },
    DuplicateDataSymbol {
        data: MachineDataObjectId,
        name: String,
    },
    InvalidDataSymbol {
        data: MachineDataObjectId,
        symbol: SymbolId,
    },
    MissingDataFragments {
        data: MachineDataObjectId,
        fragment: FragmentId,
    },
    UnexpectedDataFragment {
        section: SectionId,
        fragment: FragmentId,
    },
    NonUnitDataAlignment {
        data: MachineDataObjectId,
        alignment: u32,
    },
    UnsupportedDataAddressSpace {
        data: MachineDataObjectId,
        address_space: MachineAddressSpace,
    },
    FarDataUnsupported {
        runtime: RuntimeProfile,
        data: MachineDataObjectId,
    },
    SectionIdExhausted,
    FragmentIdExhausted,
    SymbolIdExhausted,
}

impl fmt::Display for ModuleMcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputVerification(diagnostics) => {
                write!(formatter, "input MC module failed verification")?;
                if let Some(diagnostic) = diagnostics.first() {
                    write!(formatter, ": {}", diagnostic.message)?;
                }
                Ok(())
            }
            Self::OutputVerification(diagnostics) => {
                write!(formatter, "QB header MC module failed verification")?;
                if let Some(diagnostic) = diagnostics.first() {
                    write!(formatter, ": {}", diagnostic.message)?;
                }
                Ok(())
            }
            Self::MissingTextSection { section } => {
                write!(formatter, "QB header section {section} does not exist")
            }
            Self::NonTextSection { section, kind } => {
                write!(
                    formatter,
                    "QB header section {section} is {kind:?}, not text"
                )
            }
            Self::NonExecutableTextSection { section, flags } => write!(
                formatter,
                "QB header text section {section} lacks allocation or executable flags ({:#x})",
                flags.bits()
            ),
            Self::NonEmptySection { section, kind } => write!(
                formatter,
                "initial QB object emission cannot represent nonempty {kind:?} section {section}"
            ),
            Self::ReservedSymbol { name } => {
                write!(
                    formatter,
                    "QB header reserved symbol {name:?} already exists"
                )
            }
            Self::MissingGenericSection { kind } => {
                write!(
                    formatter,
                    "QB data placement requires one {kind:?} MC section"
                )
            }
            Self::DuplicateGenericSection { kind } => {
                write!(
                    formatter,
                    "QB data placement found multiple {kind:?} MC sections"
                )
            }
            Self::UnexpectedGenericSection { section, kind } => write!(
                formatter,
                "QB data placement cannot classify generic MC section {section} ({kind:?})"
            ),
            Self::DuplicateDataObjectName { name } => {
                write!(
                    formatter,
                    "QB data placement found duplicate data object name {name:?}"
                )
            }
            Self::MissingDataSymbol { data, name } => write!(
                formatter,
                "QB data object {data} ({name:?}) has no MC symbol"
            ),
            Self::DuplicateDataSymbol { data, name } => write!(
                formatter,
                "QB data object {data} ({name:?}) has multiple MC symbols"
            ),
            Self::InvalidDataSymbol { data, symbol } => write!(
                formatter,
                "QB data object {data} has invalid MC symbol {symbol}"
            ),
            Self::MissingDataFragments { data, fragment } => write!(
                formatter,
                "QB data object {data} is missing its alignment/data pair at fragment {fragment}"
            ),
            Self::UnexpectedDataFragment { section, fragment } => write!(
                formatter,
                "QB data placement found unexpected generic data fragment {fragment} in section {section}"
            ),
            Self::NonUnitDataAlignment { data, alignment } => write!(
                formatter,
                "QB data object {data} has unsupported alignment {alignment}; Python establishes unit alignment"
            ),
            Self::UnsupportedDataAddressSpace {
                data,
                address_space,
            } => write!(
                formatter,
                "QB data object {data} uses unsupported storage address space {address_space:?}"
            ),
            Self::FarDataUnsupported { runtime, data } => write!(
                formatter,
                "{runtime:?} cannot place data object {data} in VBDOS FSL_CONST"
            ),
            Self::SectionIdExhausted => write!(formatter, "MC section identifiers are exhausted"),
            Self::FragmentIdExhausted => write!(formatter, "MC fragment identifiers are exhausted"),
            Self::SymbolIdExhausted => write!(formatter, "MC symbol identifiers are exhausted"),
        }
    }
}

impl Error for ModuleMcError {}

/// Rehomes representable initialized data in the QB runtime's measured sections.
///
/// Generic lowering contributes one text, read-only-data, and writable-data
/// section. Each selected object owns one adjacent unit-alignment/data pair,
/// named by an MC symbol at the data fragment. This adapter preserves those
/// pairs, symbols, fixups, and text untouched while regrouping them in source
/// order for QB's runtime-owned data sections.
pub fn place_data(
    module: &MCModule,
    data_objects: &[MachineDataObject],
    runtime: RuntimeProfile,
) -> Result<MCModule, ModuleMcError> {
    module.verify().map_err(ModuleMcError::InputVerification)?;
    if module
        .symbols
        .iter()
        .any(|symbol| symbol.name == DATA_PREFIX_SYMBOL)
    {
        return Err(ModuleMcError::ReservedSymbol {
            name: DATA_PREFIX_SYMBOL.to_owned(),
        });
    }

    let (text, read_only, data) = generic_sections(module)?;
    let mut names = BTreeSet::new();
    let mut used = BTreeSet::new();
    let mut mutable_near = Vec::new();
    let mut constant_near = Vec::new();
    let mut far = Vec::new();

    for object in data_objects {
        if !names.insert(&object.name) {
            return Err(ModuleMcError::DuplicateDataObjectName {
                name: object.name.clone(),
            });
        }
        if object.alignment != 1 {
            return Err(ModuleMcError::NonUnitDataAlignment {
                data: object.id,
                alignment: object.alignment,
            });
        }
        let fragments = object_fragments(module, object, read_only, data, &mut used)?;
        match object.address_space {
            MachineAddressSpace::NearData if object.constant => constant_near.push(fragments),
            MachineAddressSpace::NearData => mutable_near.push(fragments),
            MachineAddressSpace::FarData | MachineAddressSpace::HugeData => {
                if runtime != RuntimeProfile::Vbdos {
                    return Err(ModuleMcError::FarDataUnsupported {
                        runtime,
                        data: object.id,
                    });
                }
                far.push(fragments);
            }
            MachineAddressSpace::Generic
            | MachineAddressSpace::Code
            | MachineAddressSpace::Segment => {
                return Err(ModuleMcError::UnsupportedDataAddressSpace {
                    data: object.id,
                    address_space: object.address_space,
                });
            }
        }
    }
    reject_unclaimed_fragments(read_only, &used)?;
    reject_unclaimed_fragments(data, &used)?;

    let prefix = next_fragment_id(module)?;
    let prefix_symbol = next_symbol_id(module)?;
    let mut symbols = module.symbols.clone();
    symbols.push(crate::mc::MCSymbol {
        id: prefix_symbol,
        name: DATA_PREFIX_SYMBOL.to_owned(),
        binding: SymbolBinding::Local,
        visibility: SymbolVisibility::Hidden,
        definition: SymbolDefinition::Fragment {
            fragment: prefix,
            offset: 0,
        },
    });

    let mut bc_data = data.clone();
    bc_data.name = "BC_DATA".to_owned();
    bc_data.fragments = vec![MCFragment::Data(DataFragment {
        id: prefix,
        bytes: vec![0; 6],
        fixups: Vec::new(),
    })];
    for fragments in mutable_near {
        bc_data.fragments.extend(fragments);
    }

    let mut bc_cn = read_only.clone();
    bc_cn.name = "BC_CN".to_owned();
    bc_cn.fragments = constant_near.into_iter().flatten().collect();

    let mut sections = vec![text.clone(), bc_data, bc_cn];
    if runtime == RuntimeProfile::Vbdos {
        sections.push(MCSection {
            id: next_section_id(module)?,
            name: "FSL_CONST".to_owned(),
            kind: SectionKind::Data,
            flags: SectionFlags::ALLOC.union(SectionFlags::WRITABLE),
            alignment: 1,
            fragments: far.into_iter().flatten().collect(),
        });
    }
    let output = MCModule { sections, symbols };
    output.verify().map_err(ModuleMcError::OutputVerification)?;
    Ok(output)
}

fn generic_sections(
    module: &MCModule,
) -> Result<(&MCSection, &MCSection, &MCSection), ModuleMcError> {
    let mut text = None;
    let mut read_only = None;
    let mut data = None;
    for section in &module.sections {
        let slot = match section.kind {
            SectionKind::Text => &mut text,
            SectionKind::ReadOnlyData => &mut read_only,
            SectionKind::Data => &mut data,
            kind => {
                return Err(ModuleMcError::UnexpectedGenericSection {
                    section: section.id,
                    kind,
                });
            }
        };
        if slot.replace(section).is_some() {
            return Err(ModuleMcError::DuplicateGenericSection { kind: section.kind });
        }
    }
    Ok((
        text.ok_or(ModuleMcError::MissingGenericSection {
            kind: SectionKind::Text,
        })?,
        read_only.ok_or(ModuleMcError::MissingGenericSection {
            kind: SectionKind::ReadOnlyData,
        })?,
        data.ok_or(ModuleMcError::MissingGenericSection {
            kind: SectionKind::Data,
        })?,
    ))
}

fn object_fragments(
    module: &MCModule,
    object: &MachineDataObject,
    read_only: &MCSection,
    data: &MCSection,
    used: &mut BTreeSet<FragmentId>,
) -> Result<Vec<MCFragment>, ModuleMcError> {
    let symbols = module
        .symbols
        .iter()
        .filter(|symbol| symbol.name == object.name)
        .collect::<Vec<_>>();
    let [symbol] = symbols.as_slice() else {
        return if symbols.is_empty() {
            Err(ModuleMcError::MissingDataSymbol {
                data: object.id,
                name: object.name.clone(),
            })
        } else {
            Err(ModuleMcError::DuplicateDataSymbol {
                data: object.id,
                name: object.name.clone(),
            })
        };
    };
    let SymbolDefinition::Fragment {
        fragment,
        offset: 0,
    } = symbol.definition
    else {
        return Err(ModuleMcError::InvalidDataSymbol {
            data: object.id,
            symbol: symbol.id,
        });
    };
    let expected = if object.constant { read_only } else { data };
    let Some(index) = expected
        .fragments
        .iter()
        .position(|candidate| candidate.id() == fragment)
    else {
        return Err(ModuleMcError::MissingDataFragments {
            data: object.id,
            fragment,
        });
    };
    let Some(MCFragment::Align(align)) = index
        .checked_sub(1)
        .and_then(|index| expected.fragments.get(index))
    else {
        return Err(ModuleMcError::MissingDataFragments {
            data: object.id,
            fragment,
        });
    };
    if align.alignment != 1
        || align.fill != 0
        || !matches!(&expected.fragments[index], MCFragment::Data(_))
    {
        return Err(ModuleMcError::MissingDataFragments {
            data: object.id,
            fragment,
        });
    }
    if !used.insert(align.id) || !used.insert(fragment) {
        return Err(ModuleMcError::MissingDataFragments {
            data: object.id,
            fragment,
        });
    }
    Ok(vec![
        expected.fragments[index - 1].clone(),
        expected.fragments[index].clone(),
    ])
}

fn reject_unclaimed_fragments(
    section: &MCSection,
    used: &BTreeSet<FragmentId>,
) -> Result<(), ModuleMcError> {
    for fragment in &section.fragments {
        if !used.contains(&fragment.id()) {
            return Err(ModuleMcError::UnexpectedDataFragment {
                section: section.id,
                fragment: fragment.id(),
            });
        }
    }
    Ok(())
}

/// Retain the sole text section for the initial data-free QB object slice.
///
/// Generic x86 MC deliberately declares empty data sections. Microsoft BASIC
/// objects must not contain even an empty C `_DATA` contribution because it
/// changes DGROUP layout and runtime heap initialization. Nonempty sections
/// are refused until the QB data adapter can spell their measured segments.
pub fn scalar_text_only(
    module: &MCModule,
    text_section: SectionId,
) -> Result<MCModule, ModuleMcError> {
    module.verify().map_err(ModuleMcError::InputVerification)?;
    let text = module
        .sections
        .iter()
        .find(|section| section.id == text_section)
        .ok_or(ModuleMcError::MissingTextSection {
            section: text_section,
        })?;
    if text.kind != SectionKind::Text {
        return Err(ModuleMcError::NonTextSection {
            section: text_section,
            kind: text.kind,
        });
    }
    for section in &module.sections {
        if section.id != text_section && !section.fragments.is_empty() {
            return Err(ModuleMcError::NonEmptySection {
                section: section.id,
                kind: section.kind,
            });
        }
    }
    let mut output = module.clone();
    output.sections.retain(|section| section.id == text_section);
    output.verify().map_err(ModuleMcError::OutputVerification)?;
    Ok(output)
}

/// Return an MC module with the QB module header as its first text fragment.
pub fn prepend_module_header(
    module: &MCModule,
    text_section: SectionId,
    header: [u8; MODULE_HEADER_SIZE],
) -> Result<MCModule, ModuleMcError> {
    module.verify().map_err(ModuleMcError::InputVerification)?;
    let section = module
        .sections
        .iter()
        .find(|section| section.id == text_section)
        .ok_or(ModuleMcError::MissingTextSection {
            section: text_section,
        })?;
    if section.kind != SectionKind::Text {
        return Err(ModuleMcError::NonTextSection {
            section: text_section,
            kind: section.kind,
        });
    }
    let text_flags = SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE);
    if !section.flags.contains(text_flags) {
        return Err(ModuleMcError::NonExecutableTextSection {
            section: text_section,
            flags: section.flags,
        });
    }
    if module
        .symbols
        .iter()
        .any(|symbol| symbol.name == HEADER_SYMBOL)
    {
        return Err(ModuleMcError::ReservedSymbol {
            name: HEADER_SYMBOL.to_owned(),
        });
    }

    let fragment = next_fragment_id(module)?;
    let symbol = next_symbol_id(module)?;
    let mut output = module.clone();
    let section = output
        .sections
        .iter_mut()
        .find(|section| section.id == text_section)
        .ok_or(ModuleMcError::MissingTextSection {
            section: text_section,
        })?;
    section.fragments.insert(
        0,
        MCFragment::Data(DataFragment {
            id: fragment,
            bytes: header.to_vec(),
            fixups: Vec::new(),
        }),
    );
    output.symbols.push(crate::mc::MCSymbol {
        id: symbol,
        name: HEADER_SYMBOL.to_owned(),
        binding: SymbolBinding::Local,
        visibility: SymbolVisibility::Hidden,
        definition: SymbolDefinition::Fragment {
            fragment,
            offset: 0,
        },
    });
    output.verify().map_err(ModuleMcError::OutputVerification)?;
    Ok(output)
}

fn next_fragment_id(module: &MCModule) -> Result<FragmentId, ModuleMcError> {
    let next = module
        .sections
        .iter()
        .flat_map(|section| section.fragments.iter())
        .map(|fragment| fragment.id().get())
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1).ok_or(ModuleMcError::FragmentIdExhausted)
        })?;
    Ok(FragmentId::new(next))
}

fn next_section_id(module: &MCModule) -> Result<SectionId, ModuleMcError> {
    let next = module
        .sections
        .iter()
        .map(|section| section.id.get())
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1).ok_or(ModuleMcError::SectionIdExhausted)
        })?;
    Ok(SectionId::new(next))
}

fn next_symbol_id(module: &MCModule) -> Result<SymbolId, ModuleMcError> {
    let next = module
        .symbols
        .iter()
        .map(|symbol| symbol.id.get())
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1).ok_or(ModuleMcError::SymbolIdExhausted)
        })?;
    Ok(SymbolId::new(next))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        MachineAddressSpace, MachineDataObject, MachineDataObjectId, MachineDataRelocation,
        MachineLinkage,
    };
    use crate::frontend::qb::module_header::module_header;
    use crate::hir::{
        ArrayOrder, Dialect, FORMAT_VERSION, FloatMode, Module, ModuleId, Program, RuntimeProfile,
        TargetProfile,
    };
    use crate::mc::{
        AlignFragment, Fixup, MCExpression, MCSection, MCSymbol, SectionFlags, SymbolDefinition,
    };
    use crate::target::x86::{X86FixupKind, lower_to_omf};

    const TEXT: SectionId = SectionId::new(4);

    fn header() -> [u8; MODULE_HEADER_SIZE] {
        module_header(&Program {
            version: FORMAT_VERSION,
            dialect: Dialect::Qb45,
            runtime: RuntimeProfile::Qb45,
            target: TargetProfile::I386RealMode,
            array_order: ArrayOrder::ColumnMajor,
            float_mode: FloatMode::Inline,
            modules: vec![Module {
                id: ModuleId::new(1),
                name: "emission.bas".to_owned(),
                types: Vec::new(),
                functions: Vec::new(),
                data: Vec::new(),
                callables: Vec::new(),
            }],
        })
        .unwrap()
    }

    fn module() -> MCModule {
        MCModule {
            sections: vec![MCSection {
                id: TEXT,
                name: "not-inferred".to_owned(),
                kind: SectionKind::Text,
                flags: SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE),
                alignment: 1,
                fragments: vec![MCFragment::Data(DataFragment {
                    id: FragmentId::new(7),
                    bytes: vec![0, 0],
                    fixups: vec![Fixup {
                        offset: 0,
                        kind: X86FixupKind::Absolute16.into(),
                        expression: MCExpression {
                            symbol: SymbolId::new(2),
                            addend: 9,
                        },
                        pc_relative: false,
                    }],
                })],
            }],
            symbols: vec![
                MCSymbol {
                    id: SymbolId::new(2),
                    name: "external".to_owned(),
                    binding: SymbolBinding::Global,
                    visibility: SymbolVisibility::Default,
                    definition: SymbolDefinition::Undefined,
                },
                MCSymbol {
                    id: SymbolId::new(5),
                    name: "anchor".to_owned(),
                    binding: SymbolBinding::Global,
                    visibility: SymbolVisibility::Default,
                    definition: SymbolDefinition::Fragment {
                        fragment: FragmentId::new(7),
                        offset: 0,
                    },
                },
            ],
        }
    }

    fn data_object(
        id: u32,
        name: &str,
        bytes: Vec<u8>,
        address_space: MachineAddressSpace,
        constant: bool,
        relocations: Vec<MachineDataRelocation>,
    ) -> MachineDataObject {
        MachineDataObject {
            id: MachineDataObjectId::new(id),
            name: name.to_owned(),
            bytes,
            address_space,
            relocations,
            alignment: 1,
            constant,
            linkage: MachineLinkage::Internal,
        }
    }

    fn scalar_data_shape() -> (MCModule, Vec<MachineDataObject>) {
        let objects = vec![
            data_object(
                1,
                "$fslSegment",
                vec![0, 0],
                MachineAddressSpace::NearData,
                true,
                vec![MachineDataRelocation {
                    offset: 0,
                    target: MachineDataObjectId::new(2),
                    addend: 0,
                    width: 2,
                    address_space: MachineAddressSpace::Segment,
                }],
            ),
            data_object(
                2,
                "far-first",
                vec![1, 2],
                MachineAddressSpace::FarData,
                true,
                Vec::new(),
            ),
            data_object(
                3,
                "near-constant-descriptor",
                vec![0, 0, 0, 0],
                MachineAddressSpace::NearData,
                true,
                Vec::new(),
            ),
            data_object(
                4,
                "far-second",
                vec![3, 4],
                MachineAddressSpace::HugeData,
                false,
                Vec::new(),
            ),
            data_object(
                5,
                "near-mutable-descriptor",
                vec![0, 0],
                MachineAddressSpace::NearData,
                false,
                Vec::new(),
            ),
        ];
        let symbols = objects
            .iter()
            .enumerate()
            .map(|(index, object)| MCSymbol {
                id: SymbolId::new(20 + index as u32),
                name: object.name.clone(),
                binding: SymbolBinding::Local,
                visibility: SymbolVisibility::Hidden,
                definition: SymbolDefinition::Fragment {
                    fragment: FragmentId::new(11 + 2 * index as u32),
                    offset: 0,
                },
            })
            .collect::<Vec<_>>();
        let fixup = Fixup {
            offset: 0,
            kind: X86FixupKind::Segment16.into(),
            expression: MCExpression {
                symbol: SymbolId::new(21),
                addend: 0,
            },
            pc_relative: false,
        };
        let pair = |index: u32, object: &MachineDataObject, fixups: Vec<Fixup>| {
            vec![
                MCFragment::Align(AlignFragment {
                    id: FragmentId::new(10 + 2 * index),
                    alignment: 1,
                    fill: 0,
                }),
                MCFragment::Data(DataFragment {
                    id: FragmentId::new(11 + 2 * index),
                    bytes: object.bytes.clone(),
                    fixups,
                }),
            ]
        };
        let mut read_only = pair(0, &objects[0], vec![fixup]);
        read_only.extend(pair(1, &objects[1], Vec::new()));
        read_only.extend(pair(2, &objects[2], Vec::new()));
        let mut data = pair(3, &objects[3], Vec::new());
        data.extend(pair(4, &objects[4], Vec::new()));
        (
            MCModule {
                sections: vec![
                    MCSection {
                        id: TEXT,
                        name: ".text".to_owned(),
                        kind: SectionKind::Text,
                        flags: SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE),
                        alignment: 1,
                        fragments: Vec::new(),
                    },
                    MCSection {
                        id: SectionId::new(5),
                        name: ".rodata".to_owned(),
                        kind: SectionKind::ReadOnlyData,
                        flags: SectionFlags::ALLOC,
                        alignment: 1,
                        fragments: read_only,
                    },
                    MCSection {
                        id: SectionId::new(6),
                        name: ".data".to_owned(),
                        kind: SectionKind::Data,
                        flags: SectionFlags::ALLOC.union(SectionFlags::WRITABLE),
                        alignment: 1,
                        fragments: data,
                    },
                ],
                symbols,
            },
            objects,
        )
    }

    #[test]
    fn places_vbdos_scalar_literals_in_python_runtime_section_order() {
        // Python compile._data emits the near selector/descriptors in BC_CN,
        // far payloads in FSL_CONST, and the mutable near descriptor after
        // BC_DATA's exact six-byte $QB$DATA prefix.
        let (input, objects) = scalar_data_shape();
        let placed = place_data(&input, &objects, RuntimeProfile::Vbdos).unwrap();

        assert_eq!(
            placed
                .sections
                .iter()
                .map(|section| section.name.as_str())
                .collect::<Vec<_>>(),
            vec![".text", "BC_DATA", "BC_CN", "FSL_CONST"]
        );
        let MCFragment::Data(prefix) = &placed.sections[1].fragments[0] else {
            panic!("BC_DATA must begin with the QB prefix data fragment");
        };
        assert_eq!(prefix.bytes, vec![0; 6]);
        assert_eq!(
            placed.sections[1]
                .fragments
                .iter()
                .map(MCFragment::id)
                .collect::<Vec<_>>(),
            vec![prefix.id, FragmentId::new(18), FragmentId::new(19)]
        );
        assert_eq!(
            placed.sections[2]
                .fragments
                .iter()
                .map(MCFragment::id)
                .collect::<Vec<_>>(),
            vec![
                FragmentId::new(10),
                FragmentId::new(11),
                FragmentId::new(14),
                FragmentId::new(15)
            ]
        );
        assert_eq!(
            placed.sections[3]
                .fragments
                .iter()
                .map(MCFragment::id)
                .collect::<Vec<_>>(),
            vec![
                FragmentId::new(12),
                FragmentId::new(13),
                FragmentId::new(16),
                FragmentId::new(17)
            ]
        );
        assert_eq!(placed.symbols[..5], input.symbols);
        assert_eq!(placed.symbols[5].name, DATA_PREFIX_SYMBOL);
        assert_eq!(
            match &placed.sections[2].fragments[1] {
                MCFragment::Data(data) => data.fixups.clone(),
                _ => panic!("selector must remain data"),
            },
            match &input.sections[1].fragments[1] {
                MCFragment::Data(data) => data.fixups.clone(),
                _ => panic!("input selector must be data"),
            }
        );
    }

    #[test]
    fn refuses_far_data_for_non_vbdos_runtime() {
        let (input, objects) = scalar_data_shape();
        assert_eq!(
            place_data(&input, &objects, RuntimeProfile::Qb45),
            Err(ModuleMcError::FarDataUnsupported {
                runtime: RuntimeProfile::Qb45,
                data: MachineDataObjectId::new(2),
            })
        );
    }

    #[test]
    fn prepending_is_immutable_and_places_the_exact_header_first() {
        let source = module();
        let prefixed = prepend_module_header(&source, TEXT, header()).unwrap();

        assert_eq!(source, module());
        let MCFragment::Data(data) = &prefixed.sections[0].fragments[0] else {
            panic!("QB header must be a data fragment");
        };
        assert_eq!(data.bytes, header());
        assert!(data.fixups.is_empty());
        assert_eq!(data.id, FragmentId::new(8));
        assert_eq!(
            prefixed.symbols.last(),
            Some(&MCSymbol {
                id: SymbolId::new(6),
                name: HEADER_SYMBOL.to_owned(),
                binding: SymbolBinding::Local,
                visibility: SymbolVisibility::Hidden,
                definition: SymbolDefinition::Fragment {
                    fragment: FragmentId::new(8),
                    offset: 0,
                },
            })
        );
    }

    #[test]
    fn layout_and_omf_lowering_shift_only_the_qb_prefixed_module() {
        let source = module();
        let prefixed = prepend_module_header(&source, TEXT, header()).unwrap();
        let before = crate::mc::layout(&source, |_| None).unwrap();
        let after = crate::mc::layout(&prefixed, |_| None).unwrap();
        assert_eq!(
            before.symbols[&SymbolId::new(5)],
            crate::mc::SymbolLayout::Defined {
                section: TEXT,
                offset: 0,
            }
        );
        assert_eq!(
            after.symbols[&SymbolId::new(5)],
            crate::mc::SymbolLayout::Defined {
                section: TEXT,
                offset: MODULE_HEADER_SIZE as u64,
            }
        );
        assert_eq!(before.fragments[&FragmentId::new(7)].offset, 0);
        assert_eq!(
            after.fragments[&FragmentId::new(7)].offset,
            MODULE_HEADER_SIZE as u64
        );
        let fixup = match &prefixed.sections[0].fragments[1] {
            MCFragment::Data(data) => data.fixups[0],
            _ => panic!("preexisting text fragment must remain data"),
        };
        assert_eq!(fixup.expression.symbol, SymbolId::new(2));
        assert_eq!(fixup.expression.addend, 9);

        let generic = lower_to_omf(b"unit", &source).unwrap();
        let qb = lower_to_omf(b"unit", &prefixed).unwrap();
        assert_eq!(generic.publics[0].offset, 0);
        assert_eq!(generic.segments[0].relocations[0].offset, 0);
        assert_eq!(qb.publics[0].offset, MODULE_HEADER_SIZE as u32);
        assert_eq!(
            qb.segments[0].relocations[0].offset,
            MODULE_HEADER_SIZE as u32
        );
        assert_eq!(
            qb.segments[0].relocations[0].target,
            generic.segments[0].relocations[0].target
        );
        assert_eq!(
            qb.segments[0].initialized[0].bytes[MODULE_HEADER_SIZE..],
            [9, 0]
        );
    }

    #[test]
    fn removes_empty_generic_data_sections_but_refuses_program_data() {
        let mut source = module();
        source.sections.push(MCSection {
            id: SectionId::new(5),
            name: ".data".to_owned(),
            kind: SectionKind::Data,
            flags: SectionFlags::ALLOC.union(SectionFlags::WRITABLE),
            alignment: 1,
            fragments: Vec::new(),
        });
        let scalar = scalar_text_only(&source, TEXT).unwrap();
        assert_eq!(scalar.sections.len(), 1);
        assert_eq!(scalar.sections[0].id, TEXT);

        source.sections[1]
            .fragments
            .push(MCFragment::Data(DataFragment {
                id: FragmentId::new(9),
                bytes: vec![1],
                fixups: Vec::new(),
            }));
        assert!(matches!(
            scalar_text_only(&source, TEXT),
            Err(ModuleMcError::NonEmptySection {
                section,
                kind: SectionKind::Data,
            }) if section == SectionId::new(5)
        ));
    }

    #[test]
    fn refuses_bad_inputs_reserved_symbols_and_exhausted_ids() {
        let source = module();
        assert!(matches!(
            prepend_module_header(&source, SectionId::new(99), header()),
            Err(ModuleMcError::MissingTextSection { .. })
        ));

        let mut non_text = source.clone();
        non_text.sections[0].kind = SectionKind::Data;
        assert!(matches!(
            prepend_module_header(&non_text, TEXT, header()),
            Err(ModuleMcError::NonTextSection { .. })
        ));

        let mut bad_flags = source.clone();
        bad_flags.sections[0].flags = SectionFlags::ALLOC;
        assert!(matches!(
            prepend_module_header(&bad_flags, TEXT, header()),
            Err(ModuleMcError::NonExecutableTextSection { .. })
        ));

        let mut invalid = source.clone();
        invalid.sections[0]
            .fragments
            .push(MCFragment::Data(DataFragment {
                id: FragmentId::new(7),
                bytes: Vec::new(),
                fixups: Vec::new(),
            }));
        assert!(matches!(
            prepend_module_header(&invalid, TEXT, header()),
            Err(ModuleMcError::InputVerification(_))
        ));

        let mut reserved = source.clone();
        reserved.symbols[0].name = HEADER_SYMBOL.to_owned();
        assert!(matches!(
            prepend_module_header(&reserved, TEXT, header()),
            Err(ModuleMcError::ReservedSymbol { .. })
        ));

        let mut fragments_exhausted = source.clone();
        fragments_exhausted.sections[0].fragments[0] = MCFragment::Data(DataFragment {
            id: FragmentId::new(u32::MAX),
            bytes: vec![0, 0],
            fixups: vec![Fixup {
                offset: 0,
                kind: X86FixupKind::Absolute16.into(),
                expression: MCExpression {
                    symbol: SymbolId::new(2),
                    addend: 9,
                },
                pc_relative: false,
            }],
        });
        fragments_exhausted.symbols[1].definition = SymbolDefinition::Fragment {
            fragment: FragmentId::new(u32::MAX),
            offset: 0,
        };
        assert!(matches!(
            prepend_module_header(&fragments_exhausted, TEXT, header()),
            Err(ModuleMcError::FragmentIdExhausted)
        ));

        let mut symbols_exhausted = source;
        symbols_exhausted.symbols[1].id = SymbolId::new(u32::MAX);
        assert!(matches!(
            prepend_module_header(&symbols_exhausted, TEXT, header()),
            Err(ModuleMcError::SymbolIdExhausted)
        ));
    }
}
