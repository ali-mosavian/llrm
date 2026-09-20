//! QB's module-header contribution to an already selected MC module.
//!
//! This adapter owns only the immutable prefix required by the QB runtime.
//! Section selection and all object-format policy remain explicit at its
//! caller and outside this frontend boundary.

use std::error::Error;
use std::fmt;

use crate::frontend::qb::module_header::MODULE_HEADER_SIZE;
use crate::mc::{
    DataFragment, FragmentId, MCFragment, MCModule, SectionFlags, SectionId, SectionKind,
    SymbolBinding, SymbolDefinition, SymbolId, SymbolVisibility,
};
use crate::support::diagnostic::Diagnostic;

const HEADER_SYMBOL: &str = "$QB$HEADER";

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
    ReservedSymbol {
        name: String,
    },
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
            Self::ReservedSymbol { name } => {
                write!(
                    formatter,
                    "QB header reserved symbol {name:?} already exists"
                )
            }
            Self::FragmentIdExhausted => write!(formatter, "MC fragment identifiers are exhausted"),
            Self::SymbolIdExhausted => write!(formatter, "MC symbol identifiers are exhausted"),
        }
    }
}

impl Error for ModuleMcError {}

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
    use crate::frontend::qb::module_header::module_header;
    use crate::hir::{
        ArrayOrder, Dialect, FloatMode, Module, ModuleId, Program, RuntimeProfile, TargetProfile,
        FORMAT_VERSION,
    };
    use crate::mc::{Fixup, MCExpression, MCSection, MCSymbol, SectionFlags, SymbolDefinition};
    use crate::target::x86::{lower_to_omf, X86FixupKind};

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
