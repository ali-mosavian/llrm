//! Layout and relaxation of direct, intra-section x86 jumps.
//!
//! MC deliberately has no branch encoding policy.  This target-local boundary
//! gives unconditional x86 jumps the same short-first, grow-only relaxation
//! policy as the established writer: each direct local jump starts as `EB`,
//! grows to `E9` only when its signed byte displacement is out of range, and
//! the complete fragment layout is recomputed after every growth round.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::mc::{
    self, DataFragment, FragmentId, LayoutError, MCExpression, MCFragment, MCInstruction, MCModule,
    MCOperand, SectionId, SymbolId, SymbolLayout,
};
use crate::support::diagnostic::Diagnostic;

use super::{
    encode_mc_module, encoded_size, ConditionCode, EncodeError, X86McEncodeError, X86Opcode,
};

/// Which immutable boundary failed MC verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JumpLayoutVerificationStage {
    Input,
}

/// A refusal or failure while relaxing direct x86 jumps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86JumpLayoutError {
    Verification {
        stage: JumpLayoutVerificationStage,
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
    InvalidJumpArity {
        section: SectionId,
        fragment: FragmentId,
        actual: usize,
    },
    InvalidJumpOperand {
        section: SectionId,
        fragment: FragmentId,
    },
    InvalidConditionalJumpArity {
        section: SectionId,
        fragment: FragmentId,
        actual: usize,
    },
    InvalidConditionalJumpOperand {
        section: SectionId,
        fragment: FragmentId,
        index: usize,
    },
    InvalidConditionCode {
        section: SectionId,
        fragment: FragmentId,
        value: i64,
    },
    JumpTargetAddend {
        section: SectionId,
        fragment: FragmentId,
        addend: i64,
    },
    UndefinedTarget {
        section: SectionId,
        fragment: FragmentId,
        symbol: SymbolId,
    },
    CrossSectionTarget {
        section: SectionId,
        fragment: FragmentId,
        symbol: SymbolId,
        target_section: SectionId,
    },
    Layout(LayoutError),
    DisplacementOutOfRange {
        section: SectionId,
        fragment: FragmentId,
        displacement: i128,
    },
    Encoding(X86McEncodeError),
}

impl fmt::Display for X86JumpLayoutError {
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
                "cannot size section {section} instruction fragment {fragment}: {error}"
            ),
            Self::InvalidJumpArity {
                section,
                fragment,
                actual,
            } => write!(
                formatter,
                "section {section} jump fragment {fragment} expects one target operand, found {actual}"
            ),
            Self::InvalidJumpOperand { section, fragment } => write!(
                formatter,
                "section {section} jump fragment {fragment} target must be a symbolic expression"
            ),
            Self::InvalidConditionalJumpArity {
                section,
                fragment,
                actual,
            } => write!(
                formatter,
                "section {section} conditional jump fragment {fragment} expects condition and target operands, found {actual}"
            ),
            Self::InvalidConditionalJumpOperand {
                section,
                fragment,
                index,
            } => write!(
                formatter,
                "section {section} conditional jump fragment {fragment} has an invalid operand at index {index}"
            ),
            Self::InvalidConditionCode {
                section,
                fragment,
                value,
            } => write!(
                formatter,
                "section {section} conditional jump fragment {fragment} has invalid condition discriminant {value}"
            ),
            Self::JumpTargetAddend {
                section,
                fragment,
                addend,
            } => write!(
                formatter,
                "section {section} jump fragment {fragment} has unsupported target addend {addend}"
            ),
            Self::UndefinedTarget {
                section,
                fragment,
                symbol,
            } => write!(
                formatter,
                "section {section} jump fragment {fragment} targets undefined symbol {symbol}"
            ),
            Self::CrossSectionTarget {
                section,
                fragment,
                symbol,
                target_section,
            } => write!(
                formatter,
                "section {section} jump fragment {fragment} targets symbol {symbol} in section {target_section}"
            ),
            Self::Layout(error) => error.fmt(formatter),
            Self::DisplacementOutOfRange {
                section,
                fragment,
                displacement,
            } => write!(
                formatter,
                "section {section} jump fragment {fragment} displacement {displacement} does not fit in i16"
            ),
            Self::Encoding(error) => error.fmt(formatter),
        }
    }
}

impl Error for X86JumpLayoutError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error),
            Self::Encoding(error) => Some(error),
            Self::Verification { .. }
            | Self::PreexistingFixups { .. }
            | Self::Instruction { .. }
            | Self::InvalidJumpArity { .. }
            | Self::InvalidJumpOperand { .. }
            | Self::InvalidConditionalJumpArity { .. }
            | Self::InvalidConditionalJumpOperand { .. }
            | Self::InvalidConditionCode { .. }
            | Self::JumpTargetAddend { .. }
            | Self::UndefinedTarget { .. }
            | Self::CrossSectionTarget { .. }
            | Self::DisplacementOutOfRange { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JumpForm {
    Dropped,
    Short,
    Near,
}

impl JumpForm {
    const fn size(self, conditional: bool) -> u64 {
        match self {
            Self::Dropped => 0,
            Self::Short => 2,
            Self::Near if conditional => 4,
            Self::Near => 3,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct DirectJump {
    section: SectionId,
    fragment: FragmentId,
    expression: MCExpression,
    condition: Option<ConditionCode>,
    target_fragment: Option<FragmentId>,
}

/// Relaxes and encodes direct, unconditional x86 jumps in `module`.
///
/// The input remains unchanged. A jump target must be a defined, zero-addend
/// symbol in the jump's section. External, cross-section, and addended jumps
/// are deliberately refused rather than guessing an object-format relocation
/// contract. Every remaining instruction is then encoded by
/// [`encode_mc_module`], preserving its established fixup policy.
pub fn relax_and_encode_jumps(module: &MCModule) -> Result<MCModule, X86JumpLayoutError> {
    module
        .verify()
        .map_err(|diagnostics| X86JumpLayoutError::Verification {
            stage: JumpLayoutVerificationStage::Input,
            diagnostics,
        })?;

    let jumps = collect_jumps(module)?;
    if jumps.is_empty() {
        return encode_mc_module(module).map_err(X86JumpLayoutError::Encoding);
    }

    let mut forms = jumps
        .iter()
        .map(|jump| (jump.fragment, JumpForm::Short))
        .collect::<BTreeMap<_, _>>();

    // Removing direct fallthrough jumps may make another jump a fallthrough.
    // Fragment identity, rather than a reused numeric/source address, decides
    // fallthrough. Alignment is deliberately not transparent: deleting a jump
    // can change how many padding bytes it emits.
    loop {
        let mut changed = false;
        for jump in &jumps {
            if jump.condition.is_some() || forms[&jump.fragment] != JumpForm::Short {
                continue;
            }
            if is_structural_fallthrough(module, jump, &forms) {
                forms.insert(jump.fragment, JumpForm::Dropped);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // This is the same grow-only relaxation as the Python writer. Each round
    // compares every short jump against one complete, pre-growth layout; a
    // change can only increase a fragment's size, so termination is bounded.
    loop {
        let layout = layout_for(module, &forms, &jumps)?;
        let mut changed = false;
        for jump in &jumps {
            if forms[&jump.fragment] != JumpForm::Short {
                continue;
            }
            let location = layout.fragments[&jump.fragment];
            let target = same_section_target(jump, &layout)?;
            let displacement = relative_displacement(
                target,
                location.offset,
                JumpForm::Short.size(jump.condition.is_some()),
            );
            if !fits_i8(displacement) {
                forms.insert(jump.fragment, JumpForm::Near);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let layout = layout_for(module, &forms, &jumps)?;
    let mut relaxed = module.clone();
    for section in &mut relaxed.sections {
        for fragment in &mut section.fragments {
            let MCFragment::Instruction(instruction) = fragment else {
                continue;
            };
            let Some(form) = forms.get(&instruction.id).copied() else {
                continue;
            };
            let jump = jumps
                .iter()
                .find(|jump| jump.fragment == instruction.id)
                .expect("a form exists only for a collected jump");
            let location = layout.fragments[&jump.fragment];
            let target = same_section_target(jump, &layout)?;
            let displacement =
                relative_displacement(target, location.offset, form.size(jump.condition.is_some()));
            let bytes = match form {
                JumpForm::Dropped => Vec::new(),
                JumpForm::Short => match jump.condition {
                    Some(condition) => {
                        vec![short_condition_opcode(condition), displacement as i8 as u8]
                    }
                    None => vec![0xeb, displacement as i8 as u8],
                },
                JumpForm::Near => {
                    let value = i16::try_from(displacement).map_err(|_| {
                        X86JumpLayoutError::DisplacementOutOfRange {
                            section: jump.section,
                            fragment: jump.fragment,
                            displacement,
                        }
                    })?;
                    let mut bytes = match jump.condition {
                        Some(condition) => vec![0x0f, near_condition_opcode(condition)],
                        None => vec![0xe9],
                    };
                    bytes.extend_from_slice(&value.to_le_bytes());
                    bytes
                }
            };
            let id = instruction.id;
            *fragment = MCFragment::Data(DataFragment {
                id,
                bytes,
                fixups: Vec::new(),
            });
        }
    }

    encode_mc_module(&relaxed).map_err(X86JumpLayoutError::Encoding)
}

fn collect_jumps(module: &MCModule) -> Result<Vec<DirectJump>, X86JumpLayoutError> {
    let mut jumps = Vec::new();
    for section in &module.sections {
        for fragment in &section.fragments {
            let MCFragment::Instruction(instruction) = fragment else {
                continue;
            };
            if !instruction.fixups.is_empty() {
                return Err(X86JumpLayoutError::PreexistingFixups {
                    section: section.id,
                    fragment: instruction.id,
                    count: instruction.fixups.len(),
                });
            }
            let opcode = instruction.instruction.opcode.get();
            if opcode != X86Opcode::Jump as u32 && opcode != X86Opcode::JumpConditional as u32 {
                encoded_size(&instruction.instruction).map_err(|error| {
                    X86JumpLayoutError::Instruction {
                        section: section.id,
                        fragment: instruction.id,
                        error,
                    }
                })?;
                continue;
            }
            let (expression, condition) = if opcode == X86Opcode::JumpConditional as u32 {
                conditional_jump_expression(section.id, instruction.id, &instruction.instruction)?
            } else {
                (
                    jump_expression(section.id, instruction.id, &instruction.instruction)?,
                    None,
                )
            };
            if expression.addend != 0 {
                return Err(X86JumpLayoutError::JumpTargetAddend {
                    section: section.id,
                    fragment: instruction.id,
                    addend: expression.addend,
                });
            }
            let target_fragment = module
                .symbols
                .iter()
                .find(|symbol| symbol.id == expression.symbol)
                .and_then(|symbol| match symbol.definition {
                    mc::SymbolDefinition::Fragment {
                        fragment,
                        offset: 0,
                    } => Some(fragment),
                    mc::SymbolDefinition::Undefined
                    | mc::SymbolDefinition::SectionOffset { .. }
                    | mc::SymbolDefinition::Fragment { .. } => None,
                });
            jumps.push(DirectJump {
                section: section.id,
                fragment: instruction.id,
                expression,
                condition,
                target_fragment,
            });
        }
    }
    Ok(jumps)
}

fn is_structural_fallthrough(
    module: &MCModule,
    jump: &DirectJump,
    forms: &BTreeMap<FragmentId, JumpForm>,
) -> bool {
    let Some(target) = jump.target_fragment else {
        return false;
    };
    if target == jump.fragment {
        return false;
    }
    let Some(section) = module
        .sections
        .iter()
        .find(|candidate| candidate.id == jump.section)
    else {
        return false;
    };
    let Some(jump_index) = section
        .fragments
        .iter()
        .position(|fragment| fragment.id() == jump.fragment)
    else {
        return false;
    };
    let Some(target_index) = section
        .fragments
        .iter()
        .position(|fragment| fragment.id() == target)
    else {
        return false;
    };
    jump_index < target_index
        && section.fragments[jump_index + 1..target_index]
            .iter()
            .all(|fragment| fragment_is_empty(fragment, forms))
}

fn fragment_is_empty(fragment: &MCFragment, forms: &BTreeMap<FragmentId, JumpForm>) -> bool {
    match fragment {
        MCFragment::Data(data) => data.bytes.is_empty(),
        MCFragment::ZeroFill(zero_fill) => zero_fill.size == 0,
        MCFragment::Instruction(instruction) => {
            forms.get(&instruction.id) == Some(&JumpForm::Dropped)
        }
        MCFragment::Align(_) => false,
    }
}

fn jump_expression(
    section: SectionId,
    fragment: FragmentId,
    instruction: &MCInstruction,
) -> Result<MCExpression, X86JumpLayoutError> {
    if instruction.operands.len() != 1 {
        return Err(X86JumpLayoutError::InvalidJumpArity {
            section,
            fragment,
            actual: instruction.operands.len(),
        });
    }
    let MCOperand::Expression(expression) = &instruction.operands[0] else {
        return Err(X86JumpLayoutError::InvalidJumpOperand { section, fragment });
    };
    Ok(*expression)
}

fn conditional_jump_expression(
    section: SectionId,
    fragment: FragmentId,
    instruction: &MCInstruction,
) -> Result<(MCExpression, Option<ConditionCode>), X86JumpLayoutError> {
    if instruction.operands.len() != 2 {
        return Err(X86JumpLayoutError::InvalidConditionalJumpArity {
            section,
            fragment,
            actual: instruction.operands.len(),
        });
    }
    let Some(MCOperand::Immediate(value)) = instruction.operands.first() else {
        return Err(X86JumpLayoutError::InvalidConditionalJumpOperand {
            section,
            fragment,
            index: 0,
        });
    };
    let Some(condition) = ConditionCode::ALL
        .into_iter()
        .find(|candidate| *candidate as i64 == *value)
    else {
        return Err(X86JumpLayoutError::InvalidConditionCode {
            section,
            fragment,
            value: *value,
        });
    };
    let Some(MCOperand::Expression(expression)) = instruction.operands.get(1) else {
        return Err(X86JumpLayoutError::InvalidConditionalJumpOperand {
            section,
            fragment,
            index: 1,
        });
    };
    Ok((*expression, Some(condition)))
}

fn layout_for(
    module: &MCModule,
    forms: &BTreeMap<FragmentId, JumpForm>,
    jumps: &[DirectJump],
) -> Result<mc::MCLayout, X86JumpLayoutError> {
    let mut shadow = module.clone();
    for section in &mut shadow.sections {
        for fragment in &mut section.fragments {
            let Some(form) = forms.get(&fragment.id()).copied() else {
                continue;
            };
            let conditional = jumps
                .iter()
                .find(|jump| jump.fragment == fragment.id())
                .is_some_and(|jump| jump.condition.is_some());
            let id = fragment.id();
            *fragment = MCFragment::Data(DataFragment {
                id,
                bytes: vec![0; form.size(conditional) as usize],
                fixups: Vec::new(),
            });
        }
    }
    mc::layout(&shadow, |instruction| encoded_size(instruction).ok())
        .map_err(X86JumpLayoutError::Layout)
}

fn same_section_target(
    jump: &DirectJump,
    layout: &mc::MCLayout,
) -> Result<u64, X86JumpLayoutError> {
    match layout.symbols[&jump.expression.symbol] {
        SymbolLayout::Undefined => Err(X86JumpLayoutError::UndefinedTarget {
            section: jump.section,
            fragment: jump.fragment,
            symbol: jump.expression.symbol,
        }),
        SymbolLayout::Defined { section, offset: _ } if section != jump.section => {
            Err(X86JumpLayoutError::CrossSectionTarget {
                section: jump.section,
                fragment: jump.fragment,
                symbol: jump.expression.symbol,
                target_section: section,
            })
        }
        SymbolLayout::Defined { offset, .. } => Ok(offset),
    }
}

fn relative_displacement(target: u64, start: u64, size: u64) -> i128 {
    // `layout_for` has already advanced this exact fragment by `size`;
    // a wrapping end address would therefore have been refused by generic MC
    // layout before this target-specific calculation.
    let end = start + size;
    i128::from(target) - i128::from(end)
}

fn fits_i8(value: i128) -> bool {
    value >= i128::from(i8::MIN) && value <= i128::from(i8::MAX)
}

const fn condition_opcode_offset(condition: ConditionCode) -> u8 {
    match condition {
        ConditionCode::Overflow => 0x0,
        ConditionCode::NotOverflow => 0x1,
        ConditionCode::Below => 0x2,
        ConditionCode::AboveOrEqual => 0x3,
        ConditionCode::Equal => 0x4,
        ConditionCode::NotEqual => 0x5,
        ConditionCode::BelowOrEqual => 0x6,
        ConditionCode::Above => 0x7,
        ConditionCode::Sign => 0x8,
        ConditionCode::NotSign => 0x9,
        ConditionCode::Parity => 0xa,
        ConditionCode::NotParity => 0xb,
        ConditionCode::Less => 0xc,
        ConditionCode::GreaterOrEqual => 0xd,
        ConditionCode::LessOrEqual => 0xe,
        ConditionCode::Greater => 0xf,
    }
}

const fn short_condition_opcode(condition: ConditionCode) -> u8 {
    0x70 + condition_opcode_offset(condition)
}

const fn near_condition_opcode(condition: ConditionCode) -> u8 {
    0x80 + condition_opcode_offset(condition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mc::{
        AlignFragment, Fixup, FixupKind, InstructionFragment, MCSection, MCSymbol, SectionFlags,
        SectionKind, SymbolBinding, SymbolDefinition, SymbolVisibility, ZeroFillFragment,
    };
    use crate::target::x86::{ConditionCode, X86FixupKind, X86Opcode, X86Register};

    fn text(fragments: Vec<MCFragment>) -> MCSection {
        MCSection {
            id: SectionId::new(0),
            name: ".text".into(),
            kind: SectionKind::Text,
            flags: SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE),
            alignment: 1,
            fragments,
        }
    }

    fn data(id: u32, bytes: impl Into<Vec<u8>>) -> MCFragment {
        MCFragment::Data(DataFragment {
            id: FragmentId::new(id),
            bytes: bytes.into(),
            fixups: Vec::new(),
        })
    }

    fn jump(id: u32, symbol: u32) -> MCFragment {
        MCFragment::Instruction(InstructionFragment {
            id: FragmentId::new(id),
            instruction: MCInstruction {
                opcode: crate::mc::TargetOpcode::new(X86Opcode::Jump as u32),
                operands: vec![MCOperand::Expression(MCExpression {
                    symbol: SymbolId::new(symbol),
                    addend: 0,
                })],
            },
            fixups: Vec::new(),
        })
    }

    fn conditional_jump(id: u32, condition: ConditionCode, symbol: u32) -> MCFragment {
        MCFragment::Instruction(InstructionFragment {
            id: FragmentId::new(id),
            instruction: MCInstruction {
                opcode: crate::mc::TargetOpcode::new(X86Opcode::JumpConditional as u32),
                operands: vec![
                    MCOperand::Immediate(condition as i64),
                    MCOperand::Expression(MCExpression {
                        symbol: SymbolId::new(symbol),
                        addend: 0,
                    }),
                ],
            },
            fixups: Vec::new(),
        })
    }

    fn mov(id: u32) -> MCFragment {
        MCFragment::Instruction(InstructionFragment {
            id: FragmentId::new(id),
            instruction: MCInstruction {
                opcode: crate::mc::TargetOpcode::new(X86Opcode::Mov as u32),
                operands: vec![
                    MCOperand::Register(crate::mc::PhysicalRegister::new(X86Register::Ax as u32)),
                    MCOperand::Register(crate::mc::PhysicalRegister::new(X86Register::Bx as u32)),
                ],
            },
            fixups: Vec::new(),
        })
    }

    fn symbol(id: u32, definition: SymbolDefinition) -> MCSymbol {
        MCSymbol {
            id: SymbolId::new(id),
            name: format!("L{id}"),
            binding: SymbolBinding::Local,
            visibility: SymbolVisibility::Default,
            definition,
        }
    }

    fn module(fragments: Vec<MCFragment>, symbols: Vec<MCSymbol>) -> MCModule {
        MCModule {
            sections: vec![text(fragments)],
            symbols,
        }
    }

    fn fragment_bytes(module: &MCModule, id: u32) -> &[u8] {
        let fragment = module.sections[0]
            .fragments
            .iter()
            .find(|fragment| fragment.id() == FragmentId::new(id))
            .unwrap();
        let MCFragment::Data(data) = fragment else {
            panic!("fragment {id} was not encoded");
        };
        &data.bytes
    }

    #[test]
    fn encodes_exact_forward_and_backward_jumps() {
        let forward = module(
            vec![jump(1, 0), data(2, [0; 3]), data(3, [])],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(3),
                    offset: 0,
                },
            )],
        );
        assert_eq!(
            fragment_bytes(&relax_and_encode_jumps(&forward).unwrap(), 1),
            [0xeb, 3]
        );

        let backward = module(
            vec![data(1, []), data(2, [0; 3]), jump(3, 0)],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(1),
                    offset: 0,
                },
            )],
        );
        assert_eq!(
            fragment_bytes(&relax_and_encode_jumps(&backward).unwrap(), 3),
            [0xeb, 0xfb]
        );
    }

    #[test]
    fn encodes_a_signed_less_conditional_jump() {
        let source = module(
            vec![
                conditional_jump(1, ConditionCode::Less, 0),
                data(2, [0; 3]),
                data(3, []),
            ],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(3),
                    offset: 0,
                },
            )],
        );

        assert_eq!(
            fragment_bytes(&relax_and_encode_jumps(&source).unwrap(), 1),
            [0x7c, 3]
        );
    }

    #[test]
    fn grows_conditional_jumps_at_the_short_range_boundary() {
        for (length, bytes) in [(127usize, vec![0x7c, 127]), (128, vec![0x0f, 0x8c, 128, 0])] {
            let source = module(
                vec![
                    conditional_jump(1, ConditionCode::Less, 0),
                    data(2, vec![0; length]),
                    data(3, []),
                ],
                vec![symbol(
                    0,
                    SymbolDefinition::Fragment {
                        fragment: FragmentId::new(3),
                        offset: 0,
                    },
                )],
            );
            assert_eq!(
                fragment_bytes(&relax_and_encode_jumps(&source).unwrap(), 1),
                bytes
            );
        }
    }

    #[test]
    fn rejects_malformed_conditional_branch_operands() {
        let invalid_condition = module(
            vec![MCFragment::Instruction(InstructionFragment {
                id: FragmentId::new(1),
                instruction: MCInstruction {
                    opcode: crate::mc::TargetOpcode::new(X86Opcode::JumpConditional as u32),
                    operands: vec![
                        MCOperand::Immediate(0),
                        MCOperand::Expression(MCExpression {
                            symbol: SymbolId::new(0),
                            addend: 0,
                        }),
                    ],
                },
                fixups: Vec::new(),
            })],
            vec![symbol(0, SymbolDefinition::Undefined)],
        );
        assert!(matches!(
            relax_and_encode_jumps(&invalid_condition),
            Err(X86JumpLayoutError::InvalidConditionCode { value: 0, .. })
        ));

        let wrong_arity = module(
            vec![MCFragment::Instruction(InstructionFragment {
                id: FragmentId::new(1),
                instruction: MCInstruction {
                    opcode: crate::mc::TargetOpcode::new(X86Opcode::JumpConditional as u32),
                    operands: vec![MCOperand::Immediate(ConditionCode::Less as i64)],
                },
                fixups: Vec::new(),
            })],
            Vec::new(),
        );
        assert!(matches!(
            relax_and_encode_jumps(&wrong_arity),
            Err(X86JumpLayoutError::InvalidConditionalJumpArity { actual: 1, .. })
        ));

        let wrong_target = module(
            vec![MCFragment::Instruction(InstructionFragment {
                id: FragmentId::new(1),
                instruction: MCInstruction {
                    opcode: crate::mc::TargetOpcode::new(X86Opcode::JumpConditional as u32),
                    operands: vec![
                        MCOperand::Immediate(ConditionCode::Less as i64),
                        MCOperand::Immediate(0),
                    ],
                },
                fixups: Vec::new(),
            })],
            Vec::new(),
        );
        assert!(matches!(
            relax_and_encode_jumps(&wrong_target),
            Err(X86JumpLayoutError::InvalidConditionalJumpOperand { index: 1, .. })
        ));
    }

    #[test]
    fn uses_inclusive_short_branch_endpoints() {
        for (length, bytes) in [(127usize, vec![0xeb, 127]), (128, vec![0xe9, 128, 0])] {
            let source = module(
                vec![jump(1, 0), data(2, vec![0; length]), data(3, [])],
                vec![symbol(
                    0,
                    SymbolDefinition::Fragment {
                        fragment: FragmentId::new(3),
                        offset: 0,
                    },
                )],
            );
            assert_eq!(
                fragment_bytes(&relax_and_encode_jumps(&source).unwrap(), 1),
                bytes
            );
        }

        for (length, bytes) in [(126usize, vec![0xeb, 128]), (127, vec![0xe9, 126, 255])] {
            let source = module(
                vec![data(1, []), data(2, vec![0; length]), jump(3, 0)],
                vec![symbol(
                    0,
                    SymbolDefinition::Fragment {
                        fragment: FragmentId::new(1),
                        offset: 0,
                    },
                )],
            );
            assert_eq!(
                fragment_bytes(&relax_and_encode_jumps(&source).unwrap(), 3),
                bytes
            );
        }
    }

    #[test]
    fn elides_only_direct_nonself_fallthroughs() {
        let fallthrough = module(
            vec![jump(1, 0), data(2, [])],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(2),
                    offset: 0,
                },
            )],
        );
        assert_eq!(
            fragment_bytes(&relax_and_encode_jumps(&fallthrough).unwrap(), 1),
            []
        );

        let self_target = module(
            vec![jump(1, 0)],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(1),
                    offset: 0,
                },
            )],
        );
        assert_eq!(
            fragment_bytes(&relax_and_encode_jumps(&self_target).unwrap(), 1),
            [0xeb, 0xfe]
        );

        let over_data = module(
            vec![jump(1, 0), data(2, [7]), data(3, [])],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(3),
                    offset: 0,
                },
            )],
        );
        assert_eq!(
            fragment_bytes(&relax_and_encode_jumps(&over_data).unwrap(), 1),
            [0xeb, 1]
        );
    }

    #[test]
    fn growth_before_backward_target_does_not_grow_the_backward_jump() {
        let source = module(
            vec![
                jump(1, 1),
                data(2, []),
                data(3, vec![0; 126]),
                jump(4, 0),
                data(5, vec![0; 200]),
                data(6, []),
            ],
            vec![
                symbol(
                    0,
                    SymbolDefinition::Fragment {
                        fragment: FragmentId::new(2),
                        offset: 0,
                    },
                ),
                symbol(
                    1,
                    SymbolDefinition::Fragment {
                        fragment: FragmentId::new(6),
                        offset: 0,
                    },
                ),
            ],
        );
        let encoded = relax_and_encode_jumps(&source).unwrap();
        assert_eq!(fragment_bytes(&encoded, 1), [0xe9, 72, 1]);
        assert_eq!(fragment_bytes(&encoded, 4), [0xeb, 128]);
    }

    #[test]
    fn chained_fallthroughs_are_elided_to_a_fixed_point() {
        let source = module(
            vec![jump(1, 0), jump(2, 0), data(3, [])],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(3),
                    offset: 0,
                },
            )],
        );
        let encoded = relax_and_encode_jumps(&source).unwrap();
        assert_eq!(fragment_bytes(&encoded, 1), []);
        assert_eq!(fragment_bytes(&encoded, 2), []);
    }

    #[test]
    fn keeps_each_jump_occurrence_independent_when_targets_match() {
        let source = module(
            vec![jump(1, 0), data(2, vec![0; 200]), jump(3, 0), data(4, [])],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(4),
                    offset: 0,
                },
            )],
        );
        let encoded = relax_and_encode_jumps(&source).unwrap();
        assert_eq!(fragment_bytes(&encoded, 1), [0xe9, 200, 0]);
        assert_eq!(fragment_bytes(&encoded, 3), []);
    }

    #[test]
    fn does_not_elide_a_jump_when_alignment_would_fill_its_gap() {
        let source = module(
            vec![
                data(1, [0, 0]),
                jump(2, 0),
                MCFragment::Align(AlignFragment {
                    id: FragmentId::new(3),
                    alignment: 4,
                    fill: 0x90,
                }),
                data(4, []),
            ],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(4),
                    offset: 0,
                },
            )],
        );
        assert_eq!(
            fragment_bytes(&relax_and_encode_jumps(&source).unwrap(), 2),
            [0xeb, 0]
        );
    }

    #[test]
    fn accounts_for_alignment_and_preserves_fragment_and_symbol_identity() {
        let source = module(
            vec![
                jump(1, 0),
                MCFragment::Align(AlignFragment {
                    id: FragmentId::new(2),
                    alignment: 4,
                    fill: 0x90,
                }),
                data(3, []),
                mov(4),
            ],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(3),
                    offset: 0,
                },
            )],
        );
        let encoded = relax_and_encode_jumps(&source).unwrap();
        assert_eq!(fragment_bytes(&encoded, 1), [0xeb, 2]);
        assert_eq!(encoded.symbols, source.symbols);
        assert_eq!(
            encoded.sections[0]
                .fragments
                .iter()
                .map(MCFragment::id)
                .collect::<Vec<_>>(),
            source.sections[0]
                .fragments
                .iter()
                .map(MCFragment::id)
                .collect::<Vec<_>>()
        );
        assert_eq!(fragment_bytes(&encoded, 4), [0x89, 0xd8]);
    }

    #[test]
    fn rejects_targets_without_a_direct_same_section_zero_addend_contract() {
        let undefined = module(
            vec![jump(1, 0)],
            vec![symbol(0, SymbolDefinition::Undefined)],
        );
        assert!(matches!(
            relax_and_encode_jumps(&undefined),
            Err(X86JumpLayoutError::UndefinedTarget { .. })
        ));

        let mut cross_section = module(
            vec![jump(1, 0)],
            vec![symbol(
                0,
                SymbolDefinition::SectionOffset {
                    section: SectionId::new(1),
                    offset: 0,
                },
            )],
        );
        cross_section.sections.push(MCSection {
            id: SectionId::new(1),
            name: ".data".into(),
            kind: SectionKind::Data,
            flags: SectionFlags::ALLOC,
            alignment: 1,
            fragments: vec![],
        });
        assert!(matches!(
            relax_and_encode_jumps(&cross_section),
            Err(X86JumpLayoutError::CrossSectionTarget { .. })
        ));

        let mut addended = module(
            vec![jump(1, 0), data(2, [])],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(2),
                    offset: 0,
                },
            )],
        );
        let MCFragment::Instruction(instruction) = &mut addended.sections[0].fragments[0] else {
            panic!();
        };
        instruction.instruction.operands[0] = MCOperand::Expression(MCExpression {
            symbol: SymbolId::new(0),
            addend: 1,
        });
        assert!(matches!(
            relax_and_encode_jumps(&addended),
            Err(X86JumpLayoutError::JumpTargetAddend { addend: 1, .. })
        ));
    }

    #[test]
    fn rejects_malformed_jumps_and_preexisting_instruction_fixups() {
        let wrong_arity = module(
            vec![MCFragment::Instruction(InstructionFragment {
                id: FragmentId::new(1),
                instruction: MCInstruction {
                    opcode: crate::mc::TargetOpcode::new(X86Opcode::Jump as u32),
                    operands: Vec::new(),
                },
                fixups: Vec::new(),
            })],
            vec![],
        );
        assert!(matches!(
            relax_and_encode_jumps(&wrong_arity),
            Err(X86JumpLayoutError::InvalidJumpArity { actual: 0, .. })
        ));

        let malformed = module(
            vec![MCFragment::Instruction(InstructionFragment {
                id: FragmentId::new(1),
                instruction: MCInstruction {
                    opcode: crate::mc::TargetOpcode::new(X86Opcode::Jump as u32),
                    operands: vec![MCOperand::Immediate(0)],
                },
                fixups: Vec::new(),
            })],
            vec![],
        );
        assert!(matches!(
            relax_and_encode_jumps(&malformed),
            Err(X86JumpLayoutError::InvalidJumpOperand { .. })
        ));

        let mut relocated = module(vec![mov(1)], vec![symbol(0, SymbolDefinition::Undefined)]);
        let MCFragment::Instruction(instruction) = &mut relocated.sections[0].fragments[0] else {
            panic!();
        };
        instruction.fixups.push(Fixup {
            offset: 0,
            kind: FixupKind::new(99),
            expression: MCExpression {
                symbol: SymbolId::new(0),
                addend: 0,
            },
            pc_relative: false,
        });
        assert!(matches!(
            relax_and_encode_jumps(&relocated),
            Err(X86JumpLayoutError::PreexistingFixups { count: 1, .. })
        ));
    }

    #[test]
    fn refuses_invalid_input_and_a_near_displacement_outside_i16() {
        let invalid = module(vec![jump(1, 0)], Vec::new());
        assert!(matches!(
            relax_and_encode_jumps(&invalid),
            Err(X86JumpLayoutError::Verification {
                stage: JumpLayoutVerificationStage::Input,
                ..
            })
        ));

        let too_far = module(
            vec![
                jump(1, 0),
                MCFragment::ZeroFill(ZeroFillFragment {
                    id: FragmentId::new(2),
                    size: 32_768,
                }),
                data(3, []),
            ],
            vec![symbol(
                0,
                SymbolDefinition::Fragment {
                    fragment: FragmentId::new(3),
                    offset: 0,
                },
            )],
        );
        assert!(matches!(
            relax_and_encode_jumps(&too_far),
            Err(X86JumpLayoutError::DisplacementOutOfRange {
                displacement: 32_768,
                ..
            })
        ));
    }

    #[test]
    fn is_immutable_deterministic_and_preserves_other_encoded_fixups() {
        let source = module(
            vec![
                jump(1, 0),
                MCFragment::Instruction(InstructionFragment {
                    id: FragmentId::new(2),
                    instruction: MCInstruction {
                        opcode: crate::mc::TargetOpcode::new(X86Opcode::CallFar as u32),
                        operands: vec![MCOperand::Expression(MCExpression {
                            symbol: SymbolId::new(1),
                            addend: 3,
                        })],
                    },
                    fixups: Vec::new(),
                }),
                data(3, []),
            ],
            vec![
                symbol(
                    0,
                    SymbolDefinition::Fragment {
                        fragment: FragmentId::new(3),
                        offset: 0,
                    },
                ),
                symbol(1, SymbolDefinition::Undefined),
            ],
        );
        let before = source.clone();
        let first = relax_and_encode_jumps(&source).unwrap();
        let second = relax_and_encode_jumps(&source).unwrap();
        assert_eq!(source, before);
        assert_eq!(first, second);
        let MCFragment::Data(call) = &first.sections[0].fragments[1] else {
            panic!();
        };
        assert_eq!(call.bytes, [0x9a, 0, 0, 0, 0]);
        assert_eq!(call.fixups.len(), 1);
        assert_eq!(call.fixups[0].kind, X86FixupKind::FarPointer1616.into());
        assert_eq!(call.fixups[0].expression.symbol, SymbolId::new(1));
        assert_eq!(call.fixups[0].expression.addend, 3);
    }
}
