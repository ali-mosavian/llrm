//! Encoding of address-independent x86 MC instruction fragments.
//!
//! Symbols and fixups remain target-neutral MC data. This boundary only asks
//! the x86 encoder for bytes and target fixups, then replaces each selected
//! instruction fragment with a same-ID data fragment. Address-dependent
//! branches remain explicit refusals until layout and relaxation own them.

use std::error::Error;
use std::fmt;

use crate::mc::{DataFragment, FragmentId, MCFragment, MCModule, SectionId};
use crate::support::diagnostic::Diagnostic;

use super::{encode_with_fixups, EncodeError};

/// Which side of the immutable encoding boundary failed MC verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McEncodeVerificationStage {
    Input,
    Output,
}

/// A failure while encoding one physical x86 MC module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86McEncodeError {
    Verification {
        stage: McEncodeVerificationStage,
        diagnostics: Vec<Diagnostic>,
    },
    PreexistingFixups {
        section: SectionId,
        fragment: FragmentId,
        count: usize,
    },
    Instruction {
        section: SectionId,
        fragment: FragmentId,
        error: EncodeError,
    },
}

impl fmt::Display for X86McEncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verification { stage, diagnostics } => {
                write!(formatter, "{stage:?} MC module failed verification")?;
                if let Some(diagnostic) = diagnostics.first() {
                    write!(formatter, ": {}", diagnostic.message)?;
                }
                Ok(())
            }
            Self::PreexistingFixups {
                section,
                fragment,
                count,
            } => write!(
                formatter,
                "section {section} instruction fragment {fragment} already carries {count} fixup(s)"
            ),
            Self::Instruction {
                section,
                fragment,
                error,
            } => write!(
                formatter,
                "cannot encode section {section} instruction fragment {fragment}: {error}"
            ),
        }
    }
}

impl Error for X86McEncodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Instruction { error, .. } => Some(error),
            Self::Verification { .. } | Self::PreexistingFixups { .. } => None,
        }
    }
}

/// Encodes every address-independent x86 instruction in `module`.
///
/// The input is verified and never mutated. Existing data, alignment, and
/// zero-fill fragments are preserved exactly. Instruction fragments must not
/// already carry fixups: the encoder is their sole owner, so merging two
/// independently produced lists would risk duplicate relocations.
pub fn encode_mc_module(module: &MCModule) -> Result<MCModule, X86McEncodeError> {
    module
        .verify()
        .map_err(|diagnostics| X86McEncodeError::Verification {
            stage: McEncodeVerificationStage::Input,
            diagnostics,
        })?;

    let mut output = module.clone();
    for section in &mut output.sections {
        let mut fragments = Vec::with_capacity(section.fragments.len());
        for fragment in std::mem::take(&mut section.fragments) {
            match fragment {
                MCFragment::Instruction(instruction) => {
                    if !instruction.fixups.is_empty() {
                        return Err(X86McEncodeError::PreexistingFixups {
                            section: section.id,
                            fragment: instruction.id,
                            count: instruction.fixups.len(),
                        });
                    }
                    let encoded =
                        encode_with_fixups(&instruction.instruction).map_err(|error| {
                            X86McEncodeError::Instruction {
                                section: section.id,
                                fragment: instruction.id,
                                error,
                            }
                        })?;
                    fragments.push(MCFragment::Data(DataFragment {
                        id: instruction.id,
                        bytes: encoded.bytes,
                        fixups: encoded.fixups,
                    }));
                }
                other => fragments.push(other),
            }
        }
        section.fragments = fragments;
    }

    output
        .verify()
        .map_err(|diagnostics| X86McEncodeError::Verification {
            stage: McEncodeVerificationStage::Output,
            diagnostics,
        })?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mc::{
        AlignFragment, Fixup, FixupKind, InstructionFragment, MCExpression, MCInstruction,
        MCOperand, MCSection, MCSymbol, PhysicalRegister, SectionFlags, SectionKind, SymbolBinding,
        SymbolDefinition, SymbolId, SymbolVisibility, TargetOpcode,
    };
    use crate::target::x86::{X86FixupKind, X86Opcode, X86Register};

    fn instruction(id: u32, opcode: X86Opcode, operands: Vec<MCOperand>) -> MCFragment {
        MCFragment::Instruction(InstructionFragment {
            id: FragmentId::new(id),
            instruction: MCInstruction {
                opcode: TargetOpcode::new(opcode as u32),
                operands,
            },
            fixups: Vec::new(),
        })
    }

    fn register(register: X86Register) -> MCOperand {
        MCOperand::Register(PhysicalRegister::new(register as u32))
    }

    fn module(fragments: Vec<MCFragment>) -> MCModule {
        MCModule {
            sections: vec![MCSection {
                id: SectionId::new(0),
                name: ".text".into(),
                kind: SectionKind::Text,
                flags: SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE),
                alignment: 1,
                fragments,
            }],
            symbols: vec![MCSymbol {
                id: SymbolId::new(0),
                name: "external".into(),
                binding: SymbolBinding::Global,
                visibility: SymbolVisibility::Default,
                definition: SymbolDefinition::Undefined,
            }],
        }
    }

    #[test]
    fn encodes_register_call_and_cleanup_fragments_without_losing_fixups() {
        let source = module(vec![
            instruction(
                1,
                X86Opcode::Mov,
                vec![register(X86Register::Ax), register(X86Register::Bx)],
            ),
            instruction(
                2,
                X86Opcode::CallFar,
                vec![MCOperand::Expression(MCExpression {
                    symbol: SymbolId::new(0),
                    addend: 6,
                })],
            ),
            instruction(3, X86Opcode::ReturnFar, vec![MCOperand::Immediate(4)]),
        ]);
        let before = source.clone();

        let first = encode_mc_module(&source).unwrap();
        let second = encode_mc_module(&source).unwrap();

        assert_eq!(source, before);
        assert_eq!(first, second);
        let MCFragment::Data(moved) = &first.sections[0].fragments[0] else {
            panic!("register instruction must become data");
        };
        assert_eq!(moved.id, FragmentId::new(1));
        assert_eq!(moved.bytes, [0x89, 0xd8]);
        assert!(moved.fixups.is_empty());

        let MCFragment::Data(call) = &first.sections[0].fragments[1] else {
            panic!("far call must become data");
        };
        assert_eq!(call.id, FragmentId::new(2));
        assert_eq!(call.bytes, [0x9a, 0, 0, 0, 0]);
        assert_eq!(call.fixups.len(), 1);
        assert_eq!(call.fixups[0].offset, 1);
        assert_eq!(call.fixups[0].kind, X86FixupKind::FarPointer1616.into());
        assert_eq!(call.fixups[0].expression.symbol, SymbolId::new(0));
        assert_eq!(call.fixups[0].expression.addend, 6);

        let MCFragment::Data(returned) = &first.sections[0].fragments[2] else {
            panic!("far return must become data");
        };
        assert_eq!(returned.bytes, [0xca, 0x04, 0x00]);
        first.verify().unwrap();
    }

    #[test]
    fn preserves_noninstruction_fragments_and_symbol_definitions() {
        let mut source = module(vec![
            MCFragment::Data(DataFragment {
                id: FragmentId::new(4),
                bytes: vec![1, 2],
                fixups: Vec::new(),
            }),
            MCFragment::Align(AlignFragment {
                id: FragmentId::new(5),
                alignment: 2,
                fill: 0x90,
            }),
        ]);
        source.symbols[0].definition = SymbolDefinition::Fragment {
            fragment: FragmentId::new(4),
            offset: 1,
        };

        assert_eq!(encode_mc_module(&source).unwrap(), source);
    }

    #[test]
    fn refuses_address_dependent_and_already_relocated_instructions() {
        let jump = module(vec![instruction(
            7,
            X86Opcode::Jump,
            vec![MCOperand::Expression(MCExpression {
                symbol: SymbolId::new(0),
                addend: 0,
            })],
        )]);
        assert!(matches!(
            encode_mc_module(&jump),
            Err(X86McEncodeError::Instruction {
                fragment,
                error: EncodeError::UnsupportedForm {
                    opcode: X86Opcode::Jump,
                    ..
                },
                ..
            }) if fragment == FragmentId::new(7)
        ));

        let mut relocated = module(vec![instruction(
            8,
            X86Opcode::CallFar,
            vec![MCOperand::Expression(MCExpression {
                symbol: SymbolId::new(0),
                addend: 0,
            })],
        )]);
        let MCFragment::Instruction(fragment) = &mut relocated.sections[0].fragments[0] else {
            unreachable!();
        };
        fragment.fixups.push(Fixup {
            offset: 1,
            kind: FixupKind::new(99),
            expression: MCExpression {
                symbol: SymbolId::new(0),
                addend: 0,
            },
            pc_relative: false,
        });
        assert_eq!(
            encode_mc_module(&relocated),
            Err(X86McEncodeError::PreexistingFixups {
                section: SectionId::new(0),
                fragment: FragmentId::new(8),
                count: 1,
            })
        );
    }

    #[test]
    fn refuses_an_invalid_input_before_encoding_it() {
        let mut invalid = module(vec![instruction(
            1,
            X86Opcode::CallFar,
            vec![MCOperand::Expression(MCExpression {
                symbol: SymbolId::new(9),
                addend: 0,
            })],
        )]);
        invalid.symbols.clear();

        assert!(matches!(
            encode_mc_module(&invalid),
            Err(X86McEncodeError::Verification {
                stage: McEncodeVerificationStage::Input,
                ..
            })
        ));
    }
}
