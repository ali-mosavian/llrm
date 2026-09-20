//! Target-independent machine-code fragments.
//!
//! This is the boundary between selected physical machine operations and an
//! object writer.  It deliberately has no virtual registers, SSA values,
//! layout addresses, or target instruction semantics.  Targets supply the
//! numeric opcode, register, and fixup-kind values; layout and encoding turn
//! these ordered fragments into bytes later.

use std::fmt;

macro_rules! entity_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

entity_id!(SectionId);
entity_id!(SymbolId);
entity_id!(FragmentId);
entity_id!(FixupKind);
entity_id!(TargetOpcode);
entity_id!(PhysicalRegister);

/// An ordered unit of object output.
///
/// Section and symbol vectors deliberately preserve declaration order.  An
/// object writer may choose a different layout policy, but it must make that
/// choice explicitly rather than inheriting hash-map iteration order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MCModule {
    pub sections: Vec<MCSection>,
    pub symbols: Vec<MCSymbol>,
}

/// One named output section and the fragments it owns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MCSection {
    pub id: SectionId,
    pub name: String,
    pub kind: SectionKind,
    pub flags: SectionFlags,
    pub alignment: u32,
    pub fragments: Vec<MCFragment>,
}

/// The broad storage role of a section.
///
/// `Other` keeps an object format's numeric section class at the target/object
/// boundary without teaching MC a format-specific vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionKind {
    Text,
    ReadOnlyData,
    Data,
    Bss,
    Metadata,
    Other(u16),
}

/// Generic linker-visible properties of a section.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SectionFlags(u16);

impl SectionFlags {
    pub const NONE: Self = Self(0);
    pub const ALLOC: Self = Self(1 << 0);
    pub const EXECUTABLE: Self = Self(1 << 1);
    pub const WRITABLE: Self = Self(1 << 2);
    pub const MERGEABLE: Self = Self(1 << 3);

    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A contiguous output contribution whose final address is assigned by layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MCFragment {
    Data(DataFragment),
    ZeroFill(ZeroFillFragment),
    Align(AlignFragment),
    Instruction(InstructionFragment),
}

impl MCFragment {
    pub const fn id(&self) -> FragmentId {
        match self {
            Self::Data(fragment) => fragment.id,
            Self::ZeroFill(fragment) => fragment.id,
            Self::Align(fragment) => fragment.id,
            Self::Instruction(fragment) => fragment.id,
        }
    }
}

/// Already encoded bytes and the relocations that patch them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataFragment {
    pub id: FragmentId,
    pub bytes: Vec<u8>,
    pub fixups: Vec<Fixup>,
}

/// A region that occupies address space but has no bytes in the object output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ZeroFillFragment {
    pub id: FragmentId,
    pub size: u64,
}

/// Padding chosen by layout to satisfy an alignment boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlignFragment {
    pub id: FragmentId,
    pub alignment: u32,
    pub fill: u8,
}

/// A selected physical instruction before target encoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionFragment {
    pub id: FragmentId,
    pub instruction: MCInstruction,
    /// Fixup offsets are measured from this instruction's eventual encoding.
    pub fixups: Vec<Fixup>,
}

/// A numeric target opcode and operands that are legal after register allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MCInstruction {
    pub opcode: TargetOpcode,
    pub operands: Vec<MCOperand>,
}

/// An operand visible to an encoder.
///
/// There is intentionally no generic target escape hatch: if a target needs a
/// new machine concept, it should add an explicit MC operand rather than hide
/// typed data behind bytes or strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MCOperand {
    Register(PhysicalRegister),
    Immediate(i64),
    Expression(MCExpression),
}

/// The initial relocatable expression language: one symbol and a signed addend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MCExpression {
    pub symbol: SymbolId,
    pub addend: i64,
}

/// A relocation to apply at one byte offset in an encoded fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fixup {
    pub offset: u32,
    pub kind: FixupKind,
    pub expression: MCExpression,
    pub pc_relative: bool,
}

/// One linker-visible name declared by a module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MCSymbol {
    pub id: SymbolId,
    pub name: String,
    pub binding: SymbolBinding,
    pub visibility: SymbolVisibility,
    pub definition: SymbolDefinition,
}

/// How other modules may resolve a symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolBinding {
    Local,
    Global,
    Weak,
}

/// Whether a defined symbol participates in normal external lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolVisibility {
    Default,
    Hidden,
    Protected,
}

/// The location of a symbol before final layout, or an external declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolDefinition {
    Undefined,
    SectionOffset { section: SectionId, offset: u64 },
    Fragment { fragment: FragmentId, offset: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_section_and_symbol_declaration_order() {
        let module = MCModule {
            sections: vec![
                MCSection {
                    id: SectionId::new(4),
                    name: ".rodata".to_owned(),
                    kind: SectionKind::ReadOnlyData,
                    flags: SectionFlags::ALLOC,
                    alignment: 1,
                    fragments: vec![],
                },
                MCSection {
                    id: SectionId::new(9),
                    name: ".text".to_owned(),
                    kind: SectionKind::Text,
                    flags: SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE),
                    alignment: 16,
                    fragments: vec![],
                },
            ],
            symbols: vec![
                MCSymbol {
                    id: SymbolId::new(2),
                    name: "message".to_owned(),
                    binding: SymbolBinding::Local,
                    visibility: SymbolVisibility::Default,
                    definition: SymbolDefinition::SectionOffset {
                        section: SectionId::new(4),
                        offset: 0,
                    },
                },
                MCSymbol {
                    id: SymbolId::new(7),
                    name: "puts".to_owned(),
                    binding: SymbolBinding::Global,
                    visibility: SymbolVisibility::Default,
                    definition: SymbolDefinition::Undefined,
                },
            ],
        };

        assert_eq!(module.sections[0].id, SectionId::new(4));
        assert_eq!(module.sections[1].id, SectionId::new(9));
        assert_eq!(module.symbols[0].id, SymbolId::new(2));
        assert_eq!(module.symbols[1].id, SymbolId::new(7));
    }

    #[test]
    fn instruction_uses_physical_operand_and_symbol_fixup() {
        let target = SymbolId::new(3);
        let expression = MCExpression {
            symbol: target,
            addend: -4,
        };
        let fragment = MCFragment::Instruction(InstructionFragment {
            id: FragmentId::new(1),
            instruction: MCInstruction {
                opcode: TargetOpcode::new(12),
                operands: vec![
                    MCOperand::Register(PhysicalRegister::new(5)),
                    MCOperand::Expression(expression),
                ],
            },
            fixups: vec![Fixup {
                offset: 2,
                kind: FixupKind::new(1),
                expression,
                pc_relative: true,
            }],
        });

        let MCFragment::Instruction(instruction) = fragment else {
            panic!("expected an instruction fragment");
        };
        assert_eq!(instruction.id, FragmentId::new(1));
        assert_eq!(instruction.fixups[0].expression.symbol, target);
        assert!(instruction.fixups[0].pc_relative);
        assert_eq!(
            instruction.instruction.operands[0],
            MCOperand::Register(PhysicalRegister::new(5))
        );
    }
}
