//! Deterministic, one-shot layout for target-independent MC fragments.
//!
//! Layout assigns offsets within each section.  It does not encode
//! instructions, apply fixups, or relax branches; those later stages may ask
//! to lay the same fragment sequence out again after changing its shape.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use super::{
    FragmentId, MCFragment, MCInstruction, MCModule, SectionId, SymbolDefinition, SymbolId,
    TargetOpcode,
};

/// The result of one deterministic, non-relaxing layout pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MCLayout {
    /// Final address-space size for each section.
    pub section_sizes: BTreeMap<SectionId, u64>,
    /// Start offset and occupied address-space size for every fragment.
    pub fragments: BTreeMap<FragmentId, FragmentLayout>,
    /// Resolved locations for defined symbols and explicit external symbols.
    pub symbols: BTreeMap<SymbolId, SymbolLayout>,
}

/// The assigned location of one fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FragmentLayout {
    pub section: SectionId,
    pub offset: u64,
    pub size: u64,
}

/// The resolved state of one symbol after layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolLayout {
    /// The symbol is declared here but must be supplied by another module.
    Undefined,
    /// The symbol has a section-relative location in this module.
    Defined { section: SectionId, offset: u64 },
}

/// The owner of an invalid alignment request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlignmentOwner {
    Section(SectionId),
    Fragment(FragmentId),
}

/// A layout failure that can be reported without target or object knowledge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutError {
    DuplicateSection {
        section: SectionId,
    },
    DuplicateFragment {
        fragment: FragmentId,
        first_section: SectionId,
        second_section: SectionId,
    },
    DuplicateSymbol {
        symbol: SymbolId,
    },
    InvalidAlignment {
        owner: AlignmentOwner,
        alignment: u32,
    },
    UnknownSection {
        symbol: SymbolId,
        section: SectionId,
    },
    UnknownFragment {
        symbol: SymbolId,
        fragment: FragmentId,
    },
    SectionOffsetOutOfBounds {
        symbol: SymbolId,
        section: SectionId,
        offset: u64,
        size: u64,
    },
    FragmentOffsetOutOfBounds {
        symbol: SymbolId,
        fragment: FragmentId,
        offset: u32,
        size: u64,
    },
    UnsupportedInstruction {
        fragment: FragmentId,
        opcode: TargetOpcode,
    },
    FragmentTooLarge {
        fragment: FragmentId,
    },
    AddressOverflow {
        section: SectionId,
        offset: u64,
        size: u64,
    },
}

impl fmt::Display for LayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSection { section } => {
                write!(formatter, "duplicate MC section {section}")
            }
            Self::DuplicateFragment {
                fragment,
                first_section,
                second_section,
            } => write!(
                formatter,
                "duplicate MC fragment {fragment} in sections {first_section} and {second_section}"
            ),
            Self::DuplicateSymbol { symbol } => write!(formatter, "duplicate MC symbol {symbol}"),
            Self::InvalidAlignment { owner, alignment } => {
                write!(
                    formatter,
                    "{owner:?} alignment {alignment} is not a nonzero power of two"
                )
            }
            Self::UnknownSection { symbol, section } => {
                write!(
                    formatter,
                    "symbol {symbol} refers to unknown section {section}"
                )
            }
            Self::UnknownFragment { symbol, fragment } => {
                write!(
                    formatter,
                    "symbol {symbol} refers to unknown fragment {fragment}"
                )
            }
            Self::SectionOffsetOutOfBounds {
                symbol,
                section,
                offset,
                size,
            } => write!(
                formatter,
                "symbol {symbol} offset {offset} is outside section {section} size {size}"
            ),
            Self::FragmentOffsetOutOfBounds {
                symbol,
                fragment,
                offset,
                size,
            } => write!(
                formatter,
                "symbol {symbol} offset {offset} is outside fragment {fragment} size {size}"
            ),
            Self::UnsupportedInstruction { fragment, opcode } => write!(
                formatter,
                "cannot determine encoded size of fragment {fragment} opcode {opcode}"
            ),
            Self::FragmentTooLarge { fragment } => {
                write!(formatter, "fragment {fragment} is too large to lay out")
            }
            Self::AddressOverflow {
                section,
                offset,
                size,
            } => write!(
                formatter,
                "section {section} address overflows while advancing offset {offset} by {size}"
            ),
        }
    }
}

impl Error for LayoutError {}

/// Lays out `module` once using `instruction_size` for target instruction sizes.
///
/// The callback is intentionally the only target hook.  Returning `None`
/// refuses an instruction whose selected encoding is not known at this layer.
/// A later relaxation stage can choose a different encoding and invoke layout
/// again; this function never changes the module or its fragments.
pub fn layout(
    module: &MCModule,
    mut instruction_size: impl FnMut(&MCInstruction) -> Option<u64>,
) -> Result<MCLayout, LayoutError> {
    let mut section_sizes = BTreeMap::new();
    let mut fragments: BTreeMap<FragmentId, FragmentLayout> = BTreeMap::new();

    for section in &module.sections {
        check_alignment(AlignmentOwner::Section(section.id), section.alignment)?;
        if section_sizes.insert(section.id, 0).is_some() {
            return Err(LayoutError::DuplicateSection {
                section: section.id,
            });
        }

        let mut offset = 0;
        for fragment in &section.fragments {
            let id = fragment.id();
            if let Some(previous) = fragments.get(&id) {
                return Err(LayoutError::DuplicateFragment {
                    fragment: id,
                    first_section: previous.section,
                    second_section: section.id,
                });
            }

            let size = fragment_size(fragment, offset, &mut instruction_size)?;
            fragments.insert(
                id,
                FragmentLayout {
                    section: section.id,
                    offset,
                    size,
                },
            );
            offset = advance(section.id, offset, size)?;
        }
        section_sizes.insert(section.id, offset);
    }

    let mut symbols = BTreeMap::new();
    for symbol in &module.symbols {
        if symbols.contains_key(&symbol.id) {
            return Err(LayoutError::DuplicateSymbol { symbol: symbol.id });
        }
        let location = match symbol.definition {
            SymbolDefinition::Undefined => SymbolLayout::Undefined,
            SymbolDefinition::SectionOffset { section, offset } => {
                let Some(size) = section_sizes.get(&section).copied() else {
                    return Err(LayoutError::UnknownSection {
                        symbol: symbol.id,
                        section,
                    });
                };
                if offset > size {
                    return Err(LayoutError::SectionOffsetOutOfBounds {
                        symbol: symbol.id,
                        section,
                        offset,
                        size,
                    });
                }
                SymbolLayout::Defined { section, offset }
            }
            SymbolDefinition::Fragment { fragment, offset } => {
                let Some(location) = fragments.get(&fragment).copied() else {
                    return Err(LayoutError::UnknownFragment {
                        symbol: symbol.id,
                        fragment,
                    });
                };
                if u64::from(offset) > location.size {
                    return Err(LayoutError::FragmentOffsetOutOfBounds {
                        symbol: symbol.id,
                        fragment,
                        offset,
                        size: location.size,
                    });
                }
                SymbolLayout::Defined {
                    section: location.section,
                    offset: advance(location.section, location.offset, u64::from(offset))?,
                }
            }
        };
        symbols.insert(symbol.id, location);
    }

    Ok(MCLayout {
        section_sizes,
        fragments,
        symbols,
    })
}

fn check_alignment(owner: AlignmentOwner, alignment: u32) -> Result<(), LayoutError> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(LayoutError::InvalidAlignment { owner, alignment });
    }
    Ok(())
}

fn fragment_size(
    fragment: &MCFragment,
    offset: u64,
    instruction_size: &mut impl FnMut(&MCInstruction) -> Option<u64>,
) -> Result<u64, LayoutError> {
    match fragment {
        MCFragment::Data(data) => u64::try_from(data.bytes.len())
            .map_err(|_| LayoutError::FragmentTooLarge { fragment: data.id }),
        MCFragment::ZeroFill(zero_fill) => Ok(zero_fill.size),
        MCFragment::Align(align) => {
            check_alignment(AlignmentOwner::Fragment(align.id), align.alignment)?;
            let alignment = u64::from(align.alignment);
            Ok((alignment - offset % alignment) % alignment)
        }
        MCFragment::Instruction(instruction) => {
            instruction_size(&instruction.instruction).ok_or(LayoutError::UnsupportedInstruction {
                fragment: instruction.id,
                opcode: instruction.instruction.opcode,
            })
        }
    }
}

fn advance(section: SectionId, offset: u64, size: u64) -> Result<u64, LayoutError> {
    offset
        .checked_add(size)
        .ok_or(LayoutError::AddressOverflow {
            section,
            offset,
            size,
        })
}

#[cfg(test)]
mod tests {
    use super::{FragmentLayout, LayoutError, SymbolLayout, layout};
    use crate::mc::{
        AlignFragment, DataFragment, FragmentId, InstructionFragment, MCFragment, MCInstruction,
        MCModule, MCSection, MCSymbol, SectionFlags, SectionId, SectionKind, SymbolBinding,
        SymbolDefinition, SymbolId, SymbolVisibility, TargetOpcode, ZeroFillFragment,
    };

    fn section(fragments: Vec<MCFragment>) -> MCSection {
        MCSection {
            id: SectionId::new(4),
            name: ".text".into(),
            kind: SectionKind::Text,
            flags: SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE),
            alignment: 16,
            fragments,
        }
    }

    fn symbol(id: u32, definition: SymbolDefinition) -> MCSymbol {
        MCSymbol {
            id: SymbolId::new(id),
            name: format!("symbol{id}"),
            binding: SymbolBinding::Local,
            visibility: SymbolVisibility::Default,
            definition,
        }
    }

    #[test]
    fn lays_out_mixed_fragments_and_fragment_symbols() {
        let module = MCModule {
            sections: vec![section(vec![
                MCFragment::Data(DataFragment {
                    id: FragmentId::new(1),
                    bytes: vec![0xaa, 0xbb, 0xcc],
                    fixups: vec![],
                }),
                MCFragment::Align(AlignFragment {
                    id: FragmentId::new(2),
                    alignment: 4,
                    fill: 0,
                }),
                MCFragment::ZeroFill(ZeroFillFragment {
                    id: FragmentId::new(3),
                    size: 5,
                }),
                MCFragment::Instruction(InstructionFragment {
                    id: FragmentId::new(4),
                    instruction: MCInstruction {
                        opcode: TargetOpcode::new(7),
                        operands: vec![],
                    },
                    fixups: vec![],
                }),
            ])],
            symbols: vec![
                symbol(
                    1,
                    SymbolDefinition::Fragment {
                        fragment: FragmentId::new(3),
                        offset: 2,
                    },
                ),
                symbol(2, SymbolDefinition::Undefined),
            ],
        };

        let layout = layout(&module, |instruction| {
            (instruction.opcode == TargetOpcode::new(7)).then_some(2)
        })
        .unwrap();

        assert_eq!(layout.section_sizes[&SectionId::new(4)], 11);
        assert_eq!(
            layout.fragments[&FragmentId::new(1)],
            FragmentLayout {
                section: SectionId::new(4),
                offset: 0,
                size: 3,
            }
        );
        assert_eq!(layout.fragments[&FragmentId::new(2)].offset, 3);
        assert_eq!(layout.fragments[&FragmentId::new(2)].size, 1);
        assert_eq!(layout.fragments[&FragmentId::new(3)].offset, 4);
        assert_eq!(layout.fragments[&FragmentId::new(4)].offset, 9);
        assert_eq!(
            layout.symbols[&SymbolId::new(1)],
            SymbolLayout::Defined {
                section: SectionId::new(4),
                offset: 6,
            }
        );
        assert_eq!(layout.symbols[&SymbolId::new(2)], SymbolLayout::Undefined);
    }

    #[test]
    fn rejects_overflow_and_unknown_instruction_size() {
        let overflow = MCModule {
            sections: vec![section(vec![
                MCFragment::Data(DataFragment {
                    id: FragmentId::new(1),
                    bytes: vec![0],
                    fixups: vec![],
                }),
                MCFragment::ZeroFill(ZeroFillFragment {
                    id: FragmentId::new(2),
                    size: u64::MAX,
                }),
            ])],
            symbols: vec![],
        };
        assert!(matches!(
            layout(&overflow, |_| Some(0)),
            Err(LayoutError::AddressOverflow {
                section,
                offset: 1,
                size: u64::MAX,
            }) if section == SectionId::new(4)
        ));

        let unsupported = MCModule {
            sections: vec![section(vec![MCFragment::Instruction(
                InstructionFragment {
                    id: FragmentId::new(9),
                    instruction: MCInstruction {
                        opcode: TargetOpcode::new(11),
                        operands: vec![],
                    },
                    fixups: vec![],
                },
            )])],
            symbols: vec![],
        };
        assert_eq!(
            layout(&unsupported, |_| None),
            Err(LayoutError::UnsupportedInstruction {
                fragment: FragmentId::new(9),
                opcode: TargetOpcode::new(11),
            })
        );
    }
}
