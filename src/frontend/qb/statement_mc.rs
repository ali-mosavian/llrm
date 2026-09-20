//! Final QB runtime statement-table contribution to an MC text section.
//!
//! Source identity is resolved by the caller. This adapter owns only the
//! measured BASIC table spelling and its local code-offset fixups.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::mc::{
    DataFragment, Fixup, FixupKind, FragmentId, MCExpression, MCFragment, MCModule, MCSymbol,
    SectionFlags, SectionId, SectionKind, SymbolBinding, SymbolDefinition, SymbolId,
    SymbolVisibility,
};
use crate::support::diagnostic::Diagnostic;

const TABLE_SYMBOL: &str = "$QB$STAT$DATA";
const FRAME_PREFIX: [u8; 3] = [0x55, 0x8b, 0xec];

/// One final statement address and its active numbered BASIC line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatementEntry {
    pub target: FragmentId,
    pub line: u16,
}

/// An MC module carrying the final table and its addressable data symbol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementTableMc {
    pub module: MCModule,
    pub table_symbol: SymbolId,
}

/// Why the measured statement-table contribution cannot be represented.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatementMcError {
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
    ReservedSymbol {
        name: String,
    },
    FragmentIdExhausted,
    SymbolIdExhausted,
    TableTooLarge,
}

impl fmt::Display for StatementMcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputVerification(diagnostics) => diagnostic_message(
                formatter,
                "input MC module failed verification",
                diagnostics,
            ),
            Self::OutputVerification(diagnostics) => diagnostic_message(
                formatter,
                "QB statement-table MC module failed verification",
                diagnostics,
            ),
            Self::MissingTextSection { section } => {
                write!(
                    formatter,
                    "QB statement-table section {section} does not exist"
                )
            }
            Self::NonTextSection { section, kind } => write!(
                formatter,
                "QB statement-table section {section} is {kind:?}, not text"
            ),
            Self::NonExecutableTextSection { section, flags } => write!(
                formatter,
                "QB statement-table text section {section} lacks allocation or executable flags ({:#x})",
                flags.bits()
            ),
            Self::ReservedSymbol { name } => {
                write!(
                    formatter,
                    "QB statement-table reserved symbol {name:?} already exists"
                )
            }
            Self::FragmentIdExhausted => {
                write!(formatter, "MC fragment identifiers are exhausted")
            }
            Self::SymbolIdExhausted => write!(formatter, "MC symbol identifiers are exhausted"),
            Self::TableTooLarge => write!(formatter, "the QB statement table exceeds MC limits"),
        }
    }
}

impl Error for StatementMcError {}

fn diagnostic_message(
    formatter: &mut fmt::Formatter<'_>,
    message: &str,
    diagnostics: &[Diagnostic],
) -> fmt::Result {
    formatter.write_str(message)?;
    if let Some(diagnostic) = diagnostics.first() {
        write!(formatter, ": {}", diagnostic.message)?;
    }
    Ok(())
}

/// Append the final `$QB$STAT` procedure after all real code procedures.
///
/// The first three bytes are the private BP frame prefix emitted by Python's
/// shared procedure writer. `table_symbol` names the first row after that
/// prefix, because `MODULE_CODE.OF_STA` must not point at `push bp`.
pub fn append_statement_table(
    module: &MCModule,
    text_section: SectionId,
    entries: &[StatementEntry],
    offset_fixup: FixupKind,
) -> Result<StatementTableMc, StatementMcError> {
    module
        .verify()
        .map_err(StatementMcError::InputVerification)?;
    let section = module
        .sections
        .iter()
        .find(|section| section.id == text_section)
        .ok_or(StatementMcError::MissingTextSection {
            section: text_section,
        })?;
    if section.kind != SectionKind::Text {
        return Err(StatementMcError::NonTextSection {
            section: text_section,
            kind: section.kind,
        });
    }
    let required = SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE);
    if !section.flags.contains(required) {
        return Err(StatementMcError::NonExecutableTextSection {
            section: text_section,
            flags: section.flags,
        });
    }

    let table_fragment = next_fragment_id(module)?;
    let mut next_symbol = next_symbol_id(module)?;
    let table_symbol = allocate_symbol(&mut next_symbol)?;
    reject_symbol(module, TABLE_SYMBOL)?;

    let mut target_symbols = BTreeMap::new();
    for entry in entries {
        if target_symbols.contains_key(&entry.target) {
            continue;
        }
        let name = target_symbol_name(entry.target);
        reject_symbol(module, &name)?;
        target_symbols.insert(entry.target, (allocate_symbol(&mut next_symbol)?, name));
    }

    let row_bytes = entries
        .len()
        .checked_mul(4)
        .and_then(|size| size.checked_add(FRAME_PREFIX.len() + 2))
        .ok_or(StatementMcError::TableTooLarge)?;
    u32::try_from(row_bytes).map_err(|_| StatementMcError::TableTooLarge)?;
    let mut bytes = Vec::with_capacity(row_bytes);
    bytes.extend_from_slice(&FRAME_PREFIX);
    let mut fixups = Vec::with_capacity(entries.len());
    for entry in entries {
        let offset = u32::try_from(bytes.len()).map_err(|_| StatementMcError::TableTooLarge)?;
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&entry.line.to_le_bytes());
        fixups.push(Fixup {
            offset,
            kind: offset_fixup,
            expression: MCExpression {
                symbol: target_symbols[&entry.target].0,
                addend: 0,
            },
            pc_relative: false,
        });
    }
    bytes.extend_from_slice(&[0, 0]);

    let mut output = module.clone();
    output.symbols.push(MCSymbol {
        id: table_symbol,
        name: TABLE_SYMBOL.to_owned(),
        binding: SymbolBinding::Local,
        visibility: SymbolVisibility::Hidden,
        definition: SymbolDefinition::Fragment {
            fragment: table_fragment,
            offset: FRAME_PREFIX.len() as u32,
        },
    });
    output.symbols.extend(
        target_symbols
            .into_iter()
            .map(|(fragment, (id, name))| MCSymbol {
                id,
                name,
                binding: SymbolBinding::Local,
                visibility: SymbolVisibility::Hidden,
                definition: SymbolDefinition::Fragment {
                    fragment,
                    offset: 0,
                },
            }),
    );
    output
        .sections
        .iter_mut()
        .find(|section| section.id == text_section)
        .ok_or(StatementMcError::MissingTextSection {
            section: text_section,
        })?
        .fragments
        .push(MCFragment::Data(DataFragment {
            id: table_fragment,
            bytes,
            fixups,
        }));
    output
        .verify()
        .map_err(StatementMcError::OutputVerification)?;
    Ok(StatementTableMc {
        module: output,
        table_symbol,
    })
}

fn reject_symbol(module: &MCModule, name: &str) -> Result<(), StatementMcError> {
    if module.symbols.iter().any(|symbol| symbol.name == name) {
        return Err(StatementMcError::ReservedSymbol {
            name: name.to_owned(),
        });
    }
    Ok(())
}

fn target_symbol_name(fragment: FragmentId) -> String {
    format!("$QB$STAT$TARGET{}", fragment.get())
}

fn next_fragment_id(module: &MCModule) -> Result<FragmentId, StatementMcError> {
    let raw = module
        .sections
        .iter()
        .flat_map(|section| &section.fragments)
        .map(|fragment| fragment.id().get())
        .max()
        .map_or(Some(0), |maximum| maximum.checked_add(1))
        .ok_or(StatementMcError::FragmentIdExhausted)?;
    Ok(FragmentId::new(raw))
}

fn next_symbol_id(module: &MCModule) -> Result<u32, StatementMcError> {
    module
        .symbols
        .iter()
        .map(|symbol| symbol.id.get())
        .max()
        .map_or(Some(0), |maximum| maximum.checked_add(1))
        .ok_or(StatementMcError::SymbolIdExhausted)
}

fn allocate_symbol(next: &mut u32) -> Result<SymbolId, StatementMcError> {
    let id = *next;
    *next = next
        .checked_add(1)
        .ok_or(StatementMcError::SymbolIdExhausted)?;
    Ok(SymbolId::new(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mc::{MCSection, SectionFlags};
    use crate::object::omf::fixups::{FixupMode, Location};
    use crate::object::omf::write::{RelocationFrame, RelocationTarget};
    use crate::target::x86::{X86FixupKind, lower_to_omf};

    const TEXT: SectionId = SectionId::new(1);

    fn module() -> MCModule {
        MCModule {
            sections: vec![MCSection {
                id: TEXT,
                name: ".text".to_owned(),
                kind: SectionKind::Text,
                flags: SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE),
                alignment: 1,
                fragments: vec![MCFragment::Data(DataFragment {
                    id: FragmentId::new(4),
                    bytes: vec![0x90],
                    fixups: Vec::new(),
                })],
            }],
            symbols: Vec::new(),
        }
    }

    #[test]
    fn emits_the_exact_empty_table_after_its_private_frame_prefix() {
        let source = module();
        let table =
            append_statement_table(&source, TEXT, &[], X86FixupKind::Absolute16.into()).unwrap();

        assert_eq!(source, module());
        let MCFragment::Data(data) = table.module.sections[0].fragments.last().unwrap() else {
            panic!("statement table must be encoded data");
        };
        assert_eq!(data.bytes, [0x55, 0x8b, 0xec, 0, 0]);
        assert!(data.fixups.is_empty());
        assert_eq!(
            table
                .module
                .symbols
                .iter()
                .find(|symbol| symbol.id == table.table_symbol)
                .unwrap()
                .definition,
            SymbolDefinition::Fragment {
                fragment: data.id,
                offset: 3,
            }
        );
    }

    #[test]
    fn emits_rows_in_order_with_code_offset_fixups_and_zero_sentinel() {
        let source = module();
        let entries = [
            StatementEntry {
                target: FragmentId::new(4),
                line: 100,
            },
            StatementEntry {
                target: FragmentId::new(4),
                line: 0,
            },
        ];
        let table =
            append_statement_table(&source, TEXT, &entries, X86FixupKind::Absolute16.into())
                .unwrap();
        let MCFragment::Data(data) = table.module.sections[0].fragments.last().unwrap() else {
            panic!("statement table must be encoded data");
        };

        assert_eq!(
            data.bytes,
            [0x55, 0x8b, 0xec, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(data.fixups.len(), 2);
        assert_eq!(data.fixups[0].offset, 3);
        assert_eq!(data.fixups[1].offset, 7);
        assert_eq!(data.fixups[0].expression, data.fixups[1].expression);
        assert_eq!(data.fixups[0].kind, X86FixupKind::Absolute16.into());
        assert!(!data.fixups[0].pc_relative);

        let object = lower_to_omf(b"statement", &table.module).unwrap();
        assert_eq!(
            object.segments[0]
                .relocations
                .iter()
                .map(|relocation| relocation.offset)
                .collect::<Vec<_>>(),
            [4, 8]
        );
        assert!(object.segments[0].relocations.iter().all(|relocation| {
            relocation.location == Location::Offset16
                && relocation.mode == FixupMode::SegmentRelative
                && relocation.frame == RelocationFrame::Target
                && relocation.target == RelocationTarget::Segment(1)
        }));
    }
}
