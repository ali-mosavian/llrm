//! x86 relocation lowering into the target-neutral OMF construction model.
//!
//! Generic MC deliberately keeps fixup kinds opaque. This adapter is the only
//! layer which knows both their x86 field meaning and the corresponding OMF
//! relocation location. Source-language module envelopes belong above it.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::mc::{
    self, Fixup, FragmentId, MCFragment, MCModule, MCSymbol, SectionId, SectionKind, SymbolBinding,
    SymbolId, SymbolLayout, SymbolVisibility,
};
use crate::object::omf::fixups::{FixupMode, Location};
use crate::object::omf::segments::{Alignment, Combine};
use crate::object::omf::write::{
    ExternalSymbol, InitializedSpan, ObjectModule, ObjectRelocation, ObjectSegment, PublicSymbol,
    RelocationFrame, RelocationTarget,
};
use crate::support::diagnostic::Diagnostic;

use super::X86FixupKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86OmfError {
    Verification(Vec<Diagnostic>),
    ResidualInstruction {
        section: SectionId,
        fragment: FragmentId,
    },
    UnsupportedSection {
        section: SectionId,
        kind: SectionKind,
    },
    UnsupportedAlignment {
        section: SectionId,
        alignment: u32,
    },
    SectionTooLarge {
        section: SectionId,
        size: u64,
    },
    TooManySections {
        count: usize,
    },
    TooManyExternals {
        count: usize,
    },
    UnknownSymbol {
        section: SectionId,
        fragment: FragmentId,
        symbol: SymbolId,
    },
    UnsupportedUndefinedSymbol {
        symbol: SymbolId,
        binding: SymbolBinding,
        visibility: SymbolVisibility,
    },
    UnsupportedDefinedSymbol {
        symbol: SymbolId,
        binding: SymbolBinding,
        visibility: SymbolVisibility,
    },
    CrossSectionPcRelative {
        section: SectionId,
        fragment: FragmentId,
        symbol: SymbolId,
        target_section: SectionId,
    },
    UnknownFixupKind {
        section: SectionId,
        fragment: FragmentId,
        raw: u32,
    },
    PcRelativeMismatch {
        section: SectionId,
        fragment: FragmentId,
        kind: X86FixupKind,
        actual: bool,
    },
    FixupOutsideFragment {
        section: SectionId,
        fragment: FragmentId,
        offset: u32,
        width: u8,
        length: usize,
    },
    OverlappingFixups {
        section: SectionId,
        fragment: FragmentId,
        previous_end: u32,
        next_start: u32,
    },
    Layout(mc::LayoutError),
}

impl fmt::Display for X86OmfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verification(diagnostics) => {
                write!(formatter, "encoded MC module failed verification")?;
                if let Some(diagnostic) = diagnostics.first() {
                    write!(formatter, ": {}", diagnostic.message)?;
                }
                Ok(())
            }
            Self::ResidualInstruction { section, fragment } => write!(
                formatter,
                "section {section} fragment {fragment} remains an instruction at OMF lowering"
            ),
            Self::UnsupportedSection { section, kind } => {
                write!(
                    formatter,
                    "section {section} has unsupported OMF role {kind:?}"
                )
            }
            Self::UnsupportedAlignment { section, alignment } => write!(
                formatter,
                "section {section} alignment {alignment} has no supported OMF16 encoding"
            ),
            Self::SectionTooLarge { section, size } => {
                write!(
                    formatter,
                    "section {section} is {size} bytes; OMF16 permits 65536"
                )
            }
            Self::TooManySections { count } => {
                write!(formatter, "{count} sections exceed the OMF index range")
            }
            Self::TooManyExternals { count } => {
                write!(formatter, "{count} externals exceed the OMF index range")
            }
            Self::UnknownSymbol {
                section,
                fragment,
                symbol,
            } => write!(
                formatter,
                "section {section} fragment {fragment} fixup names unknown symbol {symbol}"
            ),
            Self::UnsupportedUndefinedSymbol {
                symbol,
                binding,
                visibility,
            } => write!(
                formatter,
                "undefined symbol {symbol} has unsupported {binding:?}/{visibility:?} linkage for OMF"
            ),
            Self::UnsupportedDefinedSymbol {
                symbol,
                binding,
                visibility,
            } => write!(
                formatter,
                "defined symbol {symbol} has unsupported {binding:?}/{visibility:?} linkage for OMF"
            ),
            Self::CrossSectionPcRelative {
                section,
                fragment,
                symbol,
                target_section,
            } => write!(
                formatter,
                "section {section} fragment {fragment} PC-relative fixup names symbol {symbol} in different section {target_section}"
            ),
            Self::UnknownFixupKind {
                section,
                fragment,
                raw,
            } => write!(
                formatter,
                "section {section} fragment {fragment} uses unknown x86 fixup kind {raw}"
            ),
            Self::PcRelativeMismatch {
                section,
                fragment,
                kind,
                actual,
            } => write!(
                formatter,
                "section {section} fragment {fragment} x86 fixup {kind:?} has pc_relative={actual}"
            ),
            Self::FixupOutsideFragment {
                section,
                fragment,
                offset,
                width,
                length,
            } => write!(
                formatter,
                "section {section} fragment {fragment} fixup {offset:#x}..{:#x} exceeds {length} bytes",
                u64::from(*offset) + u64::from(*width)
            ),
            Self::OverlappingFixups {
                section,
                fragment,
                previous_end,
                next_start,
            } => write!(
                formatter,
                "section {section} fragment {fragment} fixups overlap at {next_start:#x} before {previous_end:#x}"
            ),
            Self::Layout(error) => error.fmt(formatter),
        }
    }
}

impl Error for X86OmfError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error),
            _ => None,
        }
    }
}

/// Converts a fully encoded x86 MC module into target-neutral OMF facts.
pub fn lower_to_omf(
    module_name: impl AsRef<[u8]>,
    module: &MCModule,
) -> Result<ObjectModule, X86OmfError> {
    module.verify().map_err(X86OmfError::Verification)?;
    for section in &module.sections {
        for fragment in &section.fragments {
            if let MCFragment::Instruction(instruction) = fragment {
                return Err(X86OmfError::ResidualInstruction {
                    section: section.id,
                    fragment: instruction.id,
                });
            }
        }
    }

    if module.sections.len() > 0x7fff {
        return Err(X86OmfError::TooManySections {
            count: module.sections.len(),
        });
    }
    let layout = mc::layout(module, |_| None).map_err(X86OmfError::Layout)?;
    let section_indices = module
        .sections
        .iter()
        .enumerate()
        .map(|(position, section)| (section.id, (position + 1) as u16))
        .collect::<BTreeMap<_, _>>();
    let symbols = module
        .symbols
        .iter()
        .map(|symbol| (symbol.id, symbol))
        .collect::<BTreeMap<_, _>>();

    let referenced = referenced_symbols(module);
    let mut external_indices = BTreeMap::new();
    let mut externals = Vec::new();
    for symbol in &module.symbols {
        if !referenced.contains(&symbol.id)
            || !matches!(layout.symbols[&symbol.id], SymbolLayout::Undefined)
        {
            continue;
        }
        require_external(symbol)?;
        let index =
            u16::try_from(externals.len() + 1).map_err(|_| X86OmfError::TooManyExternals {
                count: externals.len() + 1,
            })?;
        if index > 0x7fff {
            return Err(X86OmfError::TooManyExternals {
                count: externals.len() + 1,
            });
        }
        external_indices.insert(symbol.id, index);
        externals.push(ExternalSymbol {
            name: symbol.name.as_bytes().to_vec(),
        });
    }

    let mut publics = Vec::new();
    for symbol in &module.symbols {
        let SymbolLayout::Defined { section, offset } = layout.symbols[&symbol.id] else {
            continue;
        };
        match (symbol.binding, symbol.visibility) {
            (SymbolBinding::Global, SymbolVisibility::Default) => publics.push(PublicSymbol {
                name: symbol.name.as_bytes().to_vec(),
                group_index: 0,
                segment_index: section_indices[&section],
                offset: offset as u32,
            }),
            (SymbolBinding::Local, _) => {}
            (binding, visibility) => {
                return Err(X86OmfError::UnsupportedDefinedSymbol {
                    symbol: symbol.id,
                    binding,
                    visibility,
                });
            }
        }
    }

    let mut segments = Vec::with_capacity(module.sections.len());
    for section in &module.sections {
        let size = layout.section_sizes[&section.id];
        if size > 0x1_0000 {
            return Err(X86OmfError::SectionTooLarge {
                section: section.id,
                size,
            });
        }
        let (class_name, combine) = section_policy(section.id, section.kind)?;
        let alignment = omf_alignment(section.id, section.alignment)?;
        let mut initialized = Vec::new();
        let mut relocations = Vec::new();

        for fragment in &section.fragments {
            let location = layout.fragments[&fragment.id()];
            match fragment {
                MCFragment::Data(data) => {
                    let mut bytes = data.bytes.clone();
                    let mut fixups = data.fixups.iter().collect::<Vec<_>>();
                    fixups.sort_by_key(|fixup| fixup.offset);
                    validate_fixup_ranges(section.id, data.id, bytes.len(), &fixups)?;
                    for fixup in fixups {
                        if let Some(lowered) = lower_fixup(
                            section.id,
                            data.id,
                            location.offset,
                            fixup,
                            &mut bytes,
                            &symbols,
                            &layout.symbols,
                            &section_indices,
                            &external_indices,
                        )? {
                            relocations.push(lowered);
                        }
                    }
                    push_initialized(&mut initialized, location.offset as u32, bytes);
                }
                MCFragment::Align(align) => {
                    push_initialized(
                        &mut initialized,
                        location.offset as u32,
                        vec![align.fill; location.size as usize],
                    );
                }
                MCFragment::ZeroFill(_) => {}
                MCFragment::Instruction(_) => unreachable!("checked before layout"),
            }
        }

        segments.push(ObjectSegment {
            name: section.name.as_bytes().to_vec(),
            class_name: class_name.to_vec(),
            alignment,
            combine,
            length: size as u32,
            initialized,
            relocations,
        });
    }

    Ok(ObjectModule {
        name: module_name.as_ref().to_vec(),
        segments,
        groups: Vec::new(),
        externals,
        publics,
    })
}

fn referenced_symbols(module: &MCModule) -> BTreeSet<SymbolId> {
    module
        .sections
        .iter()
        .flat_map(|section| &section.fragments)
        .filter_map(|fragment| match fragment {
            MCFragment::Data(data) => Some(&data.fixups),
            _ => None,
        })
        .flatten()
        .map(|fixup| fixup.expression.symbol)
        .collect()
}

fn require_external(symbol: &MCSymbol) -> Result<(), X86OmfError> {
    if (symbol.binding, symbol.visibility) == (SymbolBinding::Global, SymbolVisibility::Default) {
        Ok(())
    } else {
        Err(X86OmfError::UnsupportedUndefinedSymbol {
            symbol: symbol.id,
            binding: symbol.binding,
            visibility: symbol.visibility,
        })
    }
}

fn section_policy(
    section: SectionId,
    kind: SectionKind,
) -> Result<(&'static [u8], Combine), X86OmfError> {
    match kind {
        SectionKind::Text => Ok((b"CODE", Combine::Public)),
        SectionKind::ReadOnlyData => Ok((b"CONST", Combine::Public)),
        SectionKind::Data => Ok((b"DATA", Combine::Public)),
        SectionKind::Bss => Ok((b"BSS", Combine::Public)),
        SectionKind::Metadata | SectionKind::Other(_) => {
            Err(X86OmfError::UnsupportedSection { section, kind })
        }
    }
}

fn omf_alignment(section: SectionId, alignment: u32) -> Result<Alignment, X86OmfError> {
    match alignment {
        1 => Ok(Alignment::Byte),
        2 => Ok(Alignment::Word),
        4 => Ok(Alignment::DoubleWord),
        16 => Ok(Alignment::Paragraph),
        256 => Ok(Alignment::Page),
        4096 => Ok(Alignment::Page4K),
        _ => Err(X86OmfError::UnsupportedAlignment { section, alignment }),
    }
}

fn validate_fixup_ranges(
    section: SectionId,
    fragment: FragmentId,
    length: usize,
    fixups: &[&Fixup],
) -> Result<(), X86OmfError> {
    let mut previous_end = 0;
    for fixup in fixups {
        let kind = decode_kind(section, fragment, fixup.kind.get())?;
        let end = fixup.offset.checked_add(u32::from(kind.width())).ok_or(
            X86OmfError::FixupOutsideFragment {
                section,
                fragment,
                offset: fixup.offset,
                width: kind.width(),
                length,
            },
        )?;
        if end as usize > length {
            return Err(X86OmfError::FixupOutsideFragment {
                section,
                fragment,
                offset: fixup.offset,
                width: kind.width(),
                length,
            });
        }
        if fixup.offset < previous_end {
            return Err(X86OmfError::OverlappingFixups {
                section,
                fragment,
                previous_end,
                next_start: fixup.offset,
            });
        }
        previous_end = end;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_fixup(
    section: SectionId,
    fragment: FragmentId,
    fragment_offset: u64,
    fixup: &Fixup,
    bytes: &mut [u8],
    symbols: &BTreeMap<SymbolId, &MCSymbol>,
    layouts: &BTreeMap<SymbolId, SymbolLayout>,
    section_indices: &BTreeMap<SectionId, u16>,
    external_indices: &BTreeMap<SymbolId, u16>,
) -> Result<Option<ObjectRelocation>, X86OmfError> {
    let kind = decode_kind(section, fragment, fixup.kind.get())?;
    if fixup.pc_relative != kind.pc_relative() {
        return Err(X86OmfError::PcRelativeMismatch {
            section,
            fragment,
            kind,
            actual: fixup.pc_relative,
        });
    }
    let symbol =
        symbols
            .get(&fixup.expression.symbol)
            .copied()
            .ok_or(X86OmfError::UnknownSymbol {
                section,
                fragment,
                symbol: fixup.expression.symbol,
            })?;
    let layout = layouts
        .get(&symbol.id)
        .copied()
        .ok_or(X86OmfError::UnknownSymbol {
            section,
            fragment,
            symbol: symbol.id,
        })?;
    let (target, symbol_offset, defined_in_module) =
        match layout {
            SymbolLayout::Undefined => {
                require_external(symbol)?;
                let index = external_indices.get(&symbol.id).copied().ok_or(
                    X86OmfError::UnknownSymbol {
                        section,
                        fragment,
                        symbol: symbol.id,
                    },
                )?;
                (RelocationTarget::External(index), 0, false)
            }
            SymbolLayout::Defined {
                section: target_section,
                offset,
            } => {
                if kind.pc_relative() && target_section != section {
                    return Err(X86OmfError::CrossSectionPcRelative {
                        section,
                        fragment,
                        symbol: symbol.id,
                        target_section,
                    });
                }
                (
                    RelocationTarget::Segment(section_indices[&target_section]),
                    offset,
                    true,
                )
            }
        };

    let value = if kind.pc_relative() && defined_in_module {
        // A local near-call displacement is invariant under segment placement.
        // Resolve it now, exactly as the established Python writer resolves a
        // `Near` target found in its own label map, and do not leave a needless
        // relocation in the object.  E8 measures from the end of its rel16.
        let next_instruction =
            i128::from(fragment_offset) + i128::from(fixup.offset) + i128::from(kind.width());
        (i128::from(symbol_offset) + i128::from(fixup.expression.addend) - next_instruction)
            & 0xffff
    } else {
        // For undefined targets retain the zero/addend skeleton and a
        // self-relative OFFSET fixup.  OMF applies the place adjustment when
        // the linker resolves the EXTDEF.
        (i128::from(symbol_offset) + i128::from(fixup.expression.addend)) & 0xffff
    };
    let value = value as u16;
    let start = fixup.offset as usize;
    bytes[start..start + 2].copy_from_slice(&value.to_le_bytes());
    let location = match kind {
        X86FixupKind::FarPointer1616 => {
            bytes[start + 2..start + 4].fill(0);
            Location::Pointer16_16
        }
        X86FixupKind::Absolute16 | X86FixupKind::PcRelative16 => Location::Offset16,
    };
    if kind.pc_relative() && defined_in_module {
        return Ok(None);
    }
    Ok(Some(ObjectRelocation {
        offset: (fragment_offset + u64::from(fixup.offset)) as u32,
        location,
        mode: if kind.pc_relative() {
            FixupMode::SelfRelative
        } else {
            FixupMode::SegmentRelative
        },
        frame: RelocationFrame::Target,
        target,
    }))
}

fn decode_kind(
    section: SectionId,
    fragment: FragmentId,
    raw: u32,
) -> Result<X86FixupKind, X86OmfError> {
    match raw {
        value if value == X86FixupKind::FarPointer1616 as u32 => Ok(X86FixupKind::FarPointer1616),
        value if value == X86FixupKind::Absolute16 as u32 => Ok(X86FixupKind::Absolute16),
        value if value == X86FixupKind::PcRelative16 as u32 => Ok(X86FixupKind::PcRelative16),
        raw => Err(X86OmfError::UnknownFixupKind {
            section,
            fragment,
            raw,
        }),
    }
}

fn push_initialized(spans: &mut Vec<InitializedSpan>, offset: u32, bytes: Vec<u8>) {
    if bytes.is_empty() {
        return;
    }
    if let Some(previous) = spans.last_mut() {
        if previous.offset + previous.bytes.len() as u32 == offset {
            previous.bytes.extend(bytes);
            return;
        }
    }
    spans.push(InitializedSpan { offset, bytes });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mc::{
        AlignFragment, DataFragment, FixupKind, MCExpression, MCSection, SectionFlags,
        SymbolDefinition, ZeroFillFragment,
    };

    fn symbol(id: u32, name: &str, definition: SymbolDefinition) -> MCSymbol {
        MCSymbol {
            id: SymbolId::new(id),
            name: name.into(),
            binding: SymbolBinding::Global,
            visibility: SymbolVisibility::Default,
            definition,
        }
    }

    fn module(fragments: Vec<MCFragment>, symbols: Vec<MCSymbol>) -> MCModule {
        MCModule {
            sections: vec![MCSection {
                id: SectionId::new(0),
                name: "text".into(),
                kind: SectionKind::Text,
                flags: SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE),
                alignment: 2,
                fragments,
            }],
            symbols,
        }
    }

    fn data(id: u32, bytes: Vec<u8>, fixups: Vec<Fixup>) -> MCFragment {
        MCFragment::Data(DataFragment {
            id: FragmentId::new(id),
            bytes,
            fixups,
        })
    }

    #[test]
    fn lowers_external_far_pointer_and_materializes_addend_once() {
        let target = SymbolId::new(0);
        let source = module(
            vec![data(
                0,
                vec![0x9a, 0, 0, 0, 0],
                vec![Fixup {
                    offset: 1,
                    kind: X86FixupKind::FarPointer1616.into(),
                    expression: MCExpression {
                        symbol: target,
                        addend: 7,
                    },
                    pc_relative: false,
                }],
            )],
            vec![symbol(0, "callee", SymbolDefinition::Undefined)],
        );

        let object = lower_to_omf(b"unit.c", &source).unwrap();

        assert_eq!(object.externals[0].name, b"callee");
        assert_eq!(object.segments[0].initialized[0].bytes, [0x9a, 7, 0, 0, 0]);
        assert_eq!(
            object.segments[0].relocations[0],
            ObjectRelocation {
                offset: 1,
                location: Location::Pointer16_16,
                mode: FixupMode::SegmentRelative,
                frame: RelocationFrame::Target,
                target: RelocationTarget::External(1),
            }
        );
    }

    #[test]
    fn lowers_external_near_call_as_a_self_relative_offset_fixup() {
        use crate::object::omf::{fixups, write};

        let target = SymbolId::new(0);
        let source = module(
            vec![data(
                0,
                vec![0xe8, 0, 0],
                vec![Fixup {
                    offset: 1,
                    kind: X86FixupKind::PcRelative16.into(),
                    expression: MCExpression {
                        symbol: target,
                        addend: 0,
                    },
                    pc_relative: true,
                }],
            )],
            vec![symbol(0, "callee", SymbolDefinition::Undefined)],
        );

        let object = lower_to_omf(b"unit.c", &source).unwrap();

        assert_eq!(object.segments[0].initialized[0].bytes, [0xe8, 0, 0]);
        assert_eq!(
            object.segments[0].relocations[0],
            ObjectRelocation {
                offset: 1,
                location: Location::Offset16,
                mode: FixupMode::SelfRelative,
                frame: RelocationFrame::Target,
                target: RelocationTarget::External(1),
            }
        );

        let records = write::records(&object).unwrap();
        let decoded = fixups::parse(&records).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].patch_offset, 1);
        assert_eq!(decoded[0].location, Location::Offset16);
        assert_eq!(decoded[0].mode, FixupMode::SelfRelative);
        assert_eq!(
            decoded[0].frame.method,
            crate::object::omf::fixups::FrameMethod::Target
        );
        assert_eq!(
            decoded[0].target.method,
            crate::object::omf::fixups::TargetMethod::External
        );
        assert_eq!(decoded[0].target.datum, 1);
    }

    #[test]
    fn resolves_defined_near_call_displacement_without_a_relocation() {
        let target = SymbolId::new(0);
        let source = module(
            vec![
                data(
                    0,
                    vec![0xe8, 0, 0],
                    vec![Fixup {
                        offset: 1,
                        kind: X86FixupKind::PcRelative16.into(),
                        expression: MCExpression {
                            symbol: target,
                            addend: 4,
                        },
                        pc_relative: true,
                    }],
                ),
                data(1, vec![0x90], Vec::new()),
            ],
            vec![symbol(
                0,
                "callee",
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(1),
                    offset: 0,
                },
            )],
        );

        let object = lower_to_omf(b"unit.c", &source).unwrap();

        // The target starts at offset 3.  E8's rel16 is measured from offset
        // 3, so only the expression addend remains in the instruction.
        assert_eq!(object.segments[0].initialized[0].bytes, [0xe8, 4, 0, 0x90]);
        assert!(object.segments[0].relocations.is_empty());
    }

    #[test]
    fn refuses_defined_near_call_into_a_different_section() {
        let target = SymbolId::new(0);
        let mut source = module(
            vec![data(
                0,
                vec![0xe8, 0, 0],
                vec![Fixup {
                    offset: 1,
                    kind: X86FixupKind::PcRelative16.into(),
                    expression: MCExpression {
                        symbol: target,
                        addend: 0,
                    },
                    pc_relative: true,
                }],
            )],
            vec![symbol(
                0,
                "other",
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(1),
                    offset: 0,
                },
            )],
        );
        source.sections.push(MCSection {
            id: SectionId::new(1),
            name: "other".into(),
            kind: SectionKind::ReadOnlyData,
            flags: SectionFlags::ALLOC,
            alignment: 1,
            fragments: vec![data(1, vec![0], Vec::new())],
        });

        assert!(matches!(
            lower_to_omf(b"unit", &source),
            Err(X86OmfError::CrossSectionPcRelative {
                section,
                fragment,
                symbol,
                target_section,
            }) if section == SectionId::new(0)
                && fragment == FragmentId::new(0)
                && symbol == target
                && target_section == SectionId::new(1)
        ));
    }

    #[test]
    fn adds_defined_symbol_offset_to_absolute_field_and_exports_globals() {
        let target = SymbolId::new(0);
        let source = module(
            vec![
                data(
                    0,
                    vec![0, 0],
                    vec![Fixup {
                        offset: 0,
                        kind: X86FixupKind::Absolute16.into(),
                        expression: MCExpression {
                            symbol: target,
                            addend: -1,
                        },
                        pc_relative: false,
                    }],
                ),
                data(1, vec![0xaa], Vec::new()),
            ],
            vec![symbol(
                0,
                "item",
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(1),
                    offset: 0,
                },
            )],
        );

        let object = lower_to_omf(b"unit", &source).unwrap();

        assert_eq!(object.segments[0].initialized[0].bytes[..2], [1, 0]);
        assert_eq!(object.publics[0].name, b"item");
        assert_eq!(object.publics[0].offset, 2);
        assert_eq!(
            object.segments[0].relocations[0].target,
            RelocationTarget::Segment(1)
        );
    }

    #[test]
    fn preserves_zero_fill_gaps_and_materializes_alignment_fill() {
        let source = module(
            vec![
                data(0, vec![1], Vec::new()),
                MCFragment::ZeroFill(ZeroFillFragment {
                    id: FragmentId::new(1),
                    size: 2,
                }),
                MCFragment::Align(AlignFragment {
                    id: FragmentId::new(2),
                    alignment: 4,
                    fill: 0x90,
                }),
                data(3, vec![2], Vec::new()),
            ],
            Vec::new(),
        );

        let object = lower_to_omf(b"unit", &source).unwrap();

        assert_eq!(object.segments[0].length, 5);
        assert_eq!(
            object.segments[0].initialized,
            vec![
                InitializedSpan {
                    offset: 0,
                    bytes: vec![1]
                },
                InitializedSpan {
                    offset: 3,
                    bytes: vec![0x90, 2]
                }
            ]
        );
    }

    #[test]
    fn local_definitions_are_not_public_and_output_is_deterministic() {
        let mut local = symbol(
            0,
            "anchor",
            SymbolDefinition::Fragment {
                fragment: FragmentId::new(0),
                offset: 0,
            },
        );
        local.binding = SymbolBinding::Local;
        let source = module(vec![data(0, vec![0], Vec::new())], vec![local]);
        let first = lower_to_omf(b"unit", &source).unwrap();
        let second = lower_to_omf(b"unit", &source).unwrap();
        assert_eq!(first, second);
        assert!(first.publics.is_empty());
    }

    #[test]
    fn refuses_unknown_fixups_and_unsupported_external_linkage() {
        let target = SymbolId::new(0);
        let fixup = |kind| Fixup {
            offset: 0,
            kind,
            expression: MCExpression {
                symbol: target,
                addend: 0,
            },
            pc_relative: false,
        };
        let unknown = module(
            vec![data(0, vec![0; 2], vec![fixup(FixupKind::new(99))])],
            vec![symbol(0, "external", SymbolDefinition::Undefined)],
        );
        assert!(matches!(
            lower_to_omf(b"unit", &unknown),
            Err(X86OmfError::UnknownFixupKind { raw: 99, .. })
        ));

        let mut weak = symbol(0, "external", SymbolDefinition::Undefined);
        weak.binding = SymbolBinding::Weak;
        let unsupported = module(
            vec![data(
                0,
                vec![0; 2],
                vec![fixup(X86FixupKind::Absolute16.into())],
            )],
            vec![weak],
        );
        assert!(matches!(
            lower_to_omf(b"unit", &unsupported),
            Err(X86OmfError::UnsupportedUndefinedSymbol { .. })
        ));
    }
}
