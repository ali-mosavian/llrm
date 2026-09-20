//! Lowering of an allocated x86 Machine module into target-neutral MC.
//!
//! This boundary assigns module-wide symbols and ordered fragments, but does
//! not encode instructions, choose fixups, lay out sections, or expand ABI
//! pseudos.  Those responsibilities stay on their respective side of MC.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    MachineBlockId, MachineFunction, MachineFunctionId, MachineInstruction, MachineInstructionId,
    MachineLinkage, MachineModule, MachineOperand, MachineOperandKind,
};
use crate::mc::{
    self, AlignFragment, DataFragment, FragmentId, InstructionFragment, MCExpression, MCFragment,
    MCInstruction, MCModule, MCOperand, MCSection, MCSymbol, SectionFlags, SectionId, SectionKind,
    SymbolBinding, SymbolDefinition, SymbolId, SymbolVisibility,
};
use crate::support::diagnostic::Diagnostic;

use super::mc::{McLowerError, UnresolvedOperand, validate_allocated_operand, validate_opcode};

const TEXT_SECTION: SectionId = SectionId::new(0);
const RODATA_SECTION: SectionId = SectionId::new(1);
const DATA_SECTION: SectionId = SectionId::new(2);

/// The kind of defined symbol participating in a duplicate-name diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefinedSymbolKind {
    Data,
    Function,
}

/// The finite namespace which consumes an MC identifier sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McModuleIdKind {
    Symbol,
    Fragment,
}

/// A failure lowering an allocated x86 Machine module to MC.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86McModuleLowerError {
    DuplicateDefinedName {
        name: String,
        first: DefinedSymbolKind,
        second: DefinedSymbolKind,
    },
    DuplicateFunctionId {
        function: MachineFunctionId,
    },
    DuplicateBlockId {
        function: MachineFunctionId,
        block: MachineBlockId,
    },
    UnknownEntryBlock {
        function: MachineFunctionId,
        block: MachineBlockId,
    },
    UnknownBlock {
        function: MachineFunctionId,
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        target: MachineBlockId,
    },
    UnknownFunction {
        function: MachineFunctionId,
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        target: MachineFunctionId,
    },
    UnknownGlobal {
        function: MachineFunctionId,
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        name: String,
    },
    InvalidDataAlignment {
        name: String,
        alignment: u32,
    },
    Instruction {
        function: MachineFunctionId,
        block: MachineBlockId,
        instruction: MachineInstructionId,
        error: McLowerError,
    },
    IdExhausted {
        kind: McModuleIdKind,
    },
    Verification {
        diagnostics: Vec<Diagnostic>,
    },
}

impl fmt::Display for X86McModuleLowerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDefinedName {
                name,
                first,
                second,
            } => write!(
                formatter,
                "defined name {name:?} is both a {first:?} and a {second:?}"
            ),
            Self::DuplicateFunctionId { function } => {
                write!(formatter, "duplicate Machine function {function}")
            }
            Self::DuplicateBlockId { function, block } => {
                write!(formatter, "function {function} has duplicate block {block}")
            }
            Self::UnknownEntryBlock { function, block } => {
                write!(
                    formatter,
                    "function {function} has unknown entry block {block}"
                )
            }
            Self::UnknownBlock {
                function,
                block,
                instruction,
                operand,
                target,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} operand {operand} refers to unknown block {target}"
            ),
            Self::UnknownFunction {
                function,
                block,
                instruction,
                operand,
                target,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} operand {operand} refers to unknown function {target}"
            ),
            Self::UnknownGlobal {
                function,
                block,
                instruction,
                operand,
                name,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} operand {operand} refers to unknown global {name:?}"
            ),
            Self::InvalidDataAlignment { name, alignment } => {
                write!(
                    formatter,
                    "data object {name:?} has invalid alignment {alignment}"
                )
            }
            Self::Instruction {
                function,
                block,
                instruction,
                error,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction}: {error}"
            ),
            Self::IdExhausted { kind } => write!(formatter, "MC {kind:?} IDs are exhausted"),
            Self::Verification { diagnostics } => {
                write!(formatter, "lowered MC module failed verification")?;
                for diagnostic in diagnostics {
                    write!(formatter, ": {}", diagnostic.message)?;
                }
                Ok(())
            }
        }
    }
}

impl Error for X86McModuleLowerError {}

/// Lowers a fully allocated x86 Machine module into ordered MC sections.
pub fn lower_allocated_module(module: &MachineModule) -> Result<MCModule, X86McModuleLowerError> {
    for data in &module.data_objects {
        if data.alignment == 0 || !data.alignment.is_power_of_two() {
            return Err(X86McModuleLowerError::InvalidDataAlignment {
                name: data.name.clone(),
                alignment: data.alignment,
            });
        }
    }

    let mut ids = IdAllocator::default();
    let mut symbols = SymbolTable::default();

    for data in &module.data_objects {
        let symbol =
            symbols.declare_defined(&mut ids, &data.name, DefinedSymbolKind::Data, data.linkage)?;
        symbols.data.insert(data.name.clone(), symbol);
    }
    for function in &module.functions {
        if symbols.functions.contains_key(&function.id) {
            return Err(X86McModuleLowerError::DuplicateFunctionId {
                function: function.id,
            });
        }
        let symbol = symbols.declare_defined(
            &mut ids,
            &function.name,
            DefinedSymbolKind::Function,
            function.linkage,
        )?;
        symbols.functions.insert(function.id, symbol);
    }
    for function in &module.functions {
        for block in &function.blocks {
            let key = (function.id, block.id);
            if symbols.blocks.contains_key(&key) {
                return Err(X86McModuleLowerError::DuplicateBlockId {
                    function: function.id,
                    block: block.id,
                });
            }
            let symbol = ids.symbol()?;
            symbols.entries.push(MCSymbol {
                id: symbol,
                name: block_symbol_name(function.id, block.id),
                binding: SymbolBinding::Local,
                visibility: SymbolVisibility::Default,
                definition: SymbolDefinition::Undefined,
            });
            symbols.blocks.insert(key, symbol);
        }
        if !symbols.blocks.contains_key(&(function.id, function.entry)) {
            return Err(X86McModuleLowerError::UnknownEntryBlock {
                function: function.id,
                block: function.entry,
            });
        }
    }

    let mut text = Vec::new();
    let mut rodata = Vec::new();
    let mut data = Vec::new();

    for object in &module.data_objects {
        let fragments = if object.constant {
            &mut rodata
        } else {
            &mut data
        };
        fragments.push(MCFragment::Align(AlignFragment {
            id: ids.fragment()?,
            alignment: object.alignment,
            fill: 0,
        }));
        let fragment = ids.fragment()?;
        fragments.push(MCFragment::Data(DataFragment {
            id: fragment,
            bytes: object.bytes.clone(),
            fixups: Vec::new(),
        }));
        let symbol = symbols.data[&object.name];
        symbols.define(
            symbol,
            SymbolDefinition::Fragment {
                fragment,
                offset: 0,
            },
        );
    }

    for function in &module.functions {
        lower_function(function, &mut ids, &mut symbols, &mut text)?;
    }

    let lowered = MCModule {
        sections: vec![
            MCSection {
                id: TEXT_SECTION,
                name: ".text".to_owned(),
                kind: SectionKind::Text,
                flags: SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE),
                alignment: 1,
                fragments: text,
            },
            MCSection {
                id: RODATA_SECTION,
                name: ".rodata".to_owned(),
                kind: SectionKind::ReadOnlyData,
                flags: SectionFlags::ALLOC,
                alignment: 1,
                fragments: rodata,
            },
            MCSection {
                id: DATA_SECTION,
                name: ".data".to_owned(),
                kind: SectionKind::Data,
                flags: SectionFlags::ALLOC.union(SectionFlags::WRITABLE),
                alignment: 1,
                fragments: data,
            },
        ],
        symbols: symbols.entries,
    };
    lowered
        .verify()
        .map_err(|diagnostics| X86McModuleLowerError::Verification { diagnostics })?;
    Ok(lowered)
}

fn lower_function(
    function: &MachineFunction,
    ids: &mut IdAllocator,
    symbols: &mut SymbolTable,
    fragments: &mut Vec<MCFragment>,
) -> Result<(), X86McModuleLowerError> {
    for block in &function.blocks {
        let anchor = ids.fragment()?;
        fragments.push(MCFragment::Data(DataFragment {
            id: anchor,
            bytes: Vec::new(),
            fixups: Vec::new(),
        }));
        let block_symbol = symbols.blocks[&(function.id, block.id)];
        symbols.define(
            block_symbol,
            SymbolDefinition::Fragment {
                fragment: anchor,
                offset: 0,
            },
        );
        if block.id == function.entry {
            let function_symbol = symbols.functions[&function.id];
            symbols.define(
                function_symbol,
                SymbolDefinition::Fragment {
                    fragment: anchor,
                    offset: 0,
                },
            );
        }

        for instruction in &block.instructions {
            let instruction =
                lower_module_instruction(function, block.id, instruction, ids, symbols)?;
            fragments.push(MCFragment::Instruction(InstructionFragment {
                id: ids.fragment()?,
                instruction,
                fixups: Vec::new(),
            }));
        }
    }
    Ok(())
}

fn lower_module_instruction(
    function: &MachineFunction,
    block: MachineBlockId,
    instruction: &MachineInstruction,
    ids: &mut IdAllocator,
    symbols: &mut SymbolTable,
) -> Result<MCInstruction, X86McModuleLowerError> {
    let opcode = validate_opcode(instruction)
        .map_err(|error| instruction_error(function, block, instruction, error))?;
    let mut operands = Vec::with_capacity(instruction.operands.len());
    for (index, operand) in instruction.operands.iter().enumerate() {
        validate_allocated_operand(index, operand)
            .map_err(|error| instruction_error(function, block, instruction, error))?;
        if !operand.is_register()
            && !matches!(operand.role, crate::codegen::machine::OperandRole::None)
        {
            return Err(instruction_error(
                function,
                block,
                instruction,
                McLowerError::InvalidRole { operand: index },
            ));
        }
        operands.push(lower_module_operand(
            function,
            block,
            instruction,
            index,
            operand,
            ids,
            symbols,
        )?);
    }
    Ok(MCInstruction {
        opcode: mc::TargetOpcode::new(opcode.machine_opcode().get()),
        operands,
    })
}

fn lower_module_operand(
    function: &MachineFunction,
    block: MachineBlockId,
    instruction: &MachineInstruction,
    index: usize,
    operand: &MachineOperand,
    ids: &mut IdAllocator,
    symbols: &mut SymbolTable,
) -> Result<MCOperand, X86McModuleLowerError> {
    let expression = |symbol, addend| MCOperand::Expression(MCExpression { symbol, addend });
    match &operand.kind {
        MachineOperandKind::Register(crate::codegen::machine::MachineRegister::Physical(
            register,
        )) => Ok(MCOperand::Register(mc::PhysicalRegister::new(
            register.get(),
        ))),
        MachineOperandKind::Register(crate::codegen::machine::MachineRegister::Virtual(_)) => {
            Err(instruction_error(
                function,
                block,
                instruction,
                McLowerError::UnresolvedOperand {
                    operand: index,
                    kind: UnresolvedOperand::VirtualRegister,
                },
            ))
        }
        MachineOperandKind::Immediate(value) => Ok(MCOperand::Immediate(*value)),
        MachineOperandKind::FrameIndex { .. } => Err(instruction_error(
            function,
            block,
            instruction,
            McLowerError::UnresolvedOperand {
                operand: index,
                kind: UnresolvedOperand::FrameIndex,
            },
        )),
        MachineOperandKind::Block(target) => {
            let Some(symbol) = symbols.blocks.get(&(function.id, *target)) else {
                return Err(X86McModuleLowerError::UnknownBlock {
                    function: function.id,
                    block,
                    instruction: instruction.id,
                    operand: index,
                    target: *target,
                });
            };
            Ok(expression(*symbol, 0))
        }
        MachineOperandKind::Function(target) => {
            let Some(symbol) = symbols.functions.get(target) else {
                return Err(X86McModuleLowerError::UnknownFunction {
                    function: function.id,
                    block,
                    instruction: instruction.id,
                    operand: index,
                    target: *target,
                });
            };
            Ok(expression(*symbol, 0))
        }
        MachineOperandKind::Global { name, addend } => {
            let Some(symbol) = symbols.data.get(name) else {
                return Err(X86McModuleLowerError::UnknownGlobal {
                    function: function.id,
                    block,
                    instruction: instruction.id,
                    operand: index,
                    name: name.clone(),
                });
            };
            Ok(expression(*symbol, *addend))
        }
        MachineOperandKind::ExternalSymbol { name, addend } => {
            let symbol = symbols.external(ids, name)?;
            Ok(expression(symbol, *addend))
        }
    }
}

fn instruction_error(
    function: &MachineFunction,
    block: MachineBlockId,
    instruction: &MachineInstruction,
    error: McLowerError,
) -> X86McModuleLowerError {
    X86McModuleLowerError::Instruction {
        function: function.id,
        block,
        instruction: instruction.id,
        error,
    }
}

fn block_symbol_name(function: MachineFunctionId, block: MachineBlockId) -> String {
    format!(".Lblock.{}.{}", function.get(), block.get())
}

#[derive(Default)]
struct IdAllocator {
    next_symbol: u32,
    next_fragment: u32,
}

impl IdAllocator {
    fn symbol(&mut self) -> Result<SymbolId, X86McModuleLowerError> {
        let raw = self.next_symbol;
        self.next_symbol =
            self.next_symbol
                .checked_add(1)
                .ok_or(X86McModuleLowerError::IdExhausted {
                    kind: McModuleIdKind::Symbol,
                })?;
        Ok(SymbolId::new(raw))
    }

    fn fragment(&mut self) -> Result<FragmentId, X86McModuleLowerError> {
        let raw = self.next_fragment;
        self.next_fragment =
            self.next_fragment
                .checked_add(1)
                .ok_or(X86McModuleLowerError::IdExhausted {
                    kind: McModuleIdKind::Fragment,
                })?;
        Ok(FragmentId::new(raw))
    }
}

#[derive(Default)]
struct SymbolTable {
    entries: Vec<MCSymbol>,
    defined_names: BTreeMap<String, (SymbolId, DefinedSymbolKind)>,
    data: BTreeMap<String, SymbolId>,
    functions: BTreeMap<MachineFunctionId, SymbolId>,
    blocks: BTreeMap<(MachineFunctionId, MachineBlockId), SymbolId>,
    externals: BTreeMap<String, SymbolId>,
}

impl SymbolTable {
    fn declare_defined(
        &mut self,
        ids: &mut IdAllocator,
        name: &str,
        kind: DefinedSymbolKind,
        linkage: MachineLinkage,
    ) -> Result<SymbolId, X86McModuleLowerError> {
        if let Some((_, first)) = self.defined_names.get(name) {
            return Err(X86McModuleLowerError::DuplicateDefinedName {
                name: name.to_owned(),
                first: *first,
                second: kind,
            });
        }
        let id = ids.symbol()?;
        self.entries.push(MCSymbol {
            id,
            name: name.to_owned(),
            binding: linkage_binding(linkage),
            visibility: SymbolVisibility::Default,
            definition: SymbolDefinition::Undefined,
        });
        self.defined_names.insert(name.to_owned(), (id, kind));
        Ok(id)
    }

    fn define(&mut self, id: SymbolId, definition: SymbolDefinition) {
        let index = usize::try_from(id.get()).expect("symbol ID fits usize");
        self.entries[index].definition = definition;
    }

    fn external(
        &mut self,
        ids: &mut IdAllocator,
        name: &str,
    ) -> Result<SymbolId, X86McModuleLowerError> {
        if let Some((symbol, _)) = self.defined_names.get(name) {
            return Ok(*symbol);
        }
        if let Some(symbol) = self.externals.get(name) {
            return Ok(*symbol);
        }
        let id = ids.symbol()?;
        self.entries.push(MCSymbol {
            id,
            name: name.to_owned(),
            binding: SymbolBinding::Global,
            visibility: SymbolVisibility::Default,
            definition: SymbolDefinition::Undefined,
        });
        self.externals.insert(name.to_owned(), id);
        Ok(id)
    }
}

fn linkage_binding(linkage: MachineLinkage) -> SymbolBinding {
    match linkage {
        MachineLinkage::Internal => SymbolBinding::Local,
        MachineLinkage::External => SymbolBinding::Global,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        InstructionFlags, MachineBlock, MachineDataObject, MachineRegister, MachineSignature,
        OperandIndex, OperandRole, PhysicalRegister, RegisterConstraint, VirtualRegisterId,
    };
    use crate::target::x86::{X86Opcode, X86Register, X86RegisterClass};

    fn signature() -> MachineSignature {
        MachineSignature {
            result: None,
            parameters: Vec::new(),
            variadic: false,
            calling_convention: crate::codegen::machine::MachineCallingConvention::C,
        }
    }

    fn function(id: u32, name: &str, entry: u32, blocks: Vec<MachineBlock>) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(id),
            name: name.to_owned(),
            linkage: MachineLinkage::External,
            signature: signature(),
            entry: MachineBlockId::new(entry),
            virtual_registers: Vec::new(),
            blocks,
            frame_objects: Vec::new(),
        }
    }

    fn block(id: u32, instructions: Vec<MachineInstruction>) -> MachineBlock {
        MachineBlock {
            id: MachineBlockId::new(id),
            instructions,
            successors: Vec::new(),
        }
    }

    fn instruction(id: u32, operands: Vec<MachineOperand>) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(id),
            opcode: X86Opcode::Mov.machine_opcode(),
            operands,
            flags: InstructionFlags::NONE,
        }
    }

    fn operand(kind: MachineOperandKind) -> MachineOperand {
        MachineOperand {
            kind,
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        }
    }

    fn module(functions: Vec<MachineFunction>) -> MachineModule {
        MachineModule {
            data_objects: Vec::new(),
            functions,
        }
    }

    fn symbol<'module>(module: &'module MCModule, name: &str) -> &'module MCSymbol {
        module
            .symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .expect("test module must declare the requested symbol")
    }

    #[test]
    fn preserves_deterministic_section_symbol_and_fragment_order() {
        let input = MachineModule {
            data_objects: vec![
                MachineDataObject {
                    name: "constant".to_owned(),
                    bytes: vec![1],
                    alignment: 1,
                    constant: true,
                    linkage: MachineLinkage::Internal,
                },
                MachineDataObject {
                    name: "mutable".to_owned(),
                    bytes: vec![2],
                    alignment: 2,
                    constant: false,
                    linkage: MachineLinkage::External,
                },
            ],
            functions: vec![function(
                4,
                "entry",
                7,
                vec![block(7, vec![instruction(8, vec![])]), block(9, vec![])],
            )],
        };

        let first = lower_allocated_module(&input).unwrap();
        let second = lower_allocated_module(&input).unwrap();

        assert_eq!(first, second);
        assert_eq!(
            first
                .sections
                .iter()
                .map(|section| section.name.as_str())
                .collect::<Vec<_>>(),
            vec![".text", ".rodata", ".data"]
        );
        assert_eq!(
            first
                .symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["constant", "mutable", "entry", ".Lblock.4.7", ".Lblock.4.9"]
        );
        assert!(matches!(
            first.sections[0].fragments[0],
            MCFragment::Data(_)
        ));
        assert!(matches!(
            first.sections[0].fragments[1],
            MCFragment::Instruction(_)
        ));
        assert!(matches!(
            first.sections[0].fragments[2],
            MCFragment::Data(_)
        ));
    }

    #[test]
    fn defines_a_nonfirst_entry_function_on_its_block_anchor() {
        let lowered = lower_allocated_module(&module(vec![function(
            2,
            "second",
            9,
            vec![block(4, vec![]), block(9, vec![])],
        )]))
        .unwrap();

        let function = symbol(&lowered, "second");
        let entry = symbol(&lowered, ".Lblock.2.9");
        assert_eq!(function.definition, entry.definition);
        assert_ne!(
            function.definition,
            symbol(&lowered, ".Lblock.2.4").definition
        );
        assert!(matches!(
            function.definition,
            SymbolDefinition::Fragment { .. }
        ));
    }

    #[test]
    fn places_constant_and_mutable_data_with_explicit_alignment() {
        let lowered = lower_allocated_module(&MachineModule {
            data_objects: vec![
                MachineDataObject {
                    name: "read_only".to_owned(),
                    bytes: Vec::new(),
                    alignment: 8,
                    constant: true,
                    linkage: MachineLinkage::Internal,
                },
                MachineDataObject {
                    name: "writeable".to_owned(),
                    bytes: vec![9],
                    alignment: 4,
                    constant: false,
                    linkage: MachineLinkage::External,
                },
            ],
            functions: Vec::new(),
        })
        .unwrap();

        assert!(matches!(
            lowered.sections[1].fragments[0],
            MCFragment::Align(AlignFragment { alignment: 8, .. })
        ));
        assert!(
            matches!(&lowered.sections[1].fragments[1], MCFragment::Data(DataFragment { bytes, .. }) if bytes.is_empty())
        );
        assert!(matches!(
            lowered.sections[2].fragments[0],
            MCFragment::Align(AlignFragment { alignment: 4, .. })
        ));
        assert!(
            matches!(&lowered.sections[2].fragments[1], MCFragment::Data(DataFragment { bytes, .. }) if bytes.as_slice() == [9])
        );
        assert!(matches!(
            symbol(&lowered, "read_only").definition,
            SymbolDefinition::Fragment { .. }
        ));
        assert_eq!(symbol(&lowered, "read_only").binding, SymbolBinding::Local);
        assert_eq!(symbol(&lowered, "writeable").binding, SymbolBinding::Global);
    }

    #[test]
    fn resolves_all_symbol_operands_and_preserves_addends() {
        let first = function(
            1,
            "first",
            3,
            vec![block(
                3,
                vec![instruction(
                    5,
                    vec![
                        operand(MachineOperandKind::Block(MachineBlockId::new(3))),
                        operand(MachineOperandKind::Function(MachineFunctionId::new(2))),
                        operand(MachineOperandKind::Global {
                            name: "object".to_owned(),
                            addend: -4,
                        }),
                        operand(MachineOperandKind::ExternalSymbol {
                            name: "outside".to_owned(),
                            addend: 12,
                        }),
                    ],
                )],
            )],
        );
        let lowered = lower_allocated_module(&MachineModule {
            data_objects: vec![MachineDataObject {
                name: "object".to_owned(),
                bytes: vec![0],
                alignment: 1,
                constant: false,
                linkage: MachineLinkage::Internal,
            }],
            functions: vec![first, function(2, "second", 4, vec![block(4, vec![])])],
        })
        .unwrap();

        let instruction = match &lowered.sections[0].fragments[1] {
            MCFragment::Instruction(fragment) => &fragment.instruction,
            _ => panic!("test function must contain its instruction after the anchor"),
        };
        let expressions = instruction
            .operands
            .iter()
            .map(|operand| match operand {
                MCOperand::Expression(expression) => *expression,
                _ => panic!("all test operands are symbolic"),
            })
            .collect::<Vec<_>>();
        assert_eq!(expressions[0].symbol, symbol(&lowered, ".Lblock.1.3").id);
        assert_eq!(expressions[1].symbol, symbol(&lowered, "second").id);
        assert_eq!(
            expressions[2],
            MCExpression {
                symbol: symbol(&lowered, "object").id,
                addend: -4
            }
        );
        assert_eq!(
            expressions[3],
            MCExpression {
                symbol: symbol(&lowered, "outside").id,
                addend: 12
            }
        );
        assert_eq!(
            symbol(&lowered, "outside").definition,
            SymbolDefinition::Undefined
        );
    }

    #[test]
    fn external_symbol_reuses_a_matching_defined_symbol() {
        let lowered = lower_allocated_module(&module(vec![function(
            0,
            "defined",
            0,
            vec![block(
                0,
                vec![instruction(
                    0,
                    vec![operand(MachineOperandKind::ExternalSymbol {
                        name: "defined".to_owned(),
                        addend: 0,
                    })],
                )],
            )],
        )]))
        .unwrap();

        assert_eq!(
            lowered
                .symbols
                .iter()
                .filter(|symbol| symbol.name == "defined")
                .count(),
            1
        );
        let instruction = match &lowered.sections[0].fragments[1] {
            MCFragment::Instruction(fragment) => &fragment.instruction,
            _ => panic!("test function must contain its instruction after the anchor"),
        };
        assert_eq!(
            instruction.operands,
            vec![MCOperand::Expression(MCExpression {
                symbol: symbol(&lowered, "defined").id,
                addend: 0
            })]
        );
    }

    #[test]
    fn declares_undefined_externals_in_first_use_order_and_reuses_them() {
        let lowered = lower_allocated_module(&module(vec![function(
            0,
            "caller",
            0,
            vec![block(
                0,
                vec![instruction(
                    0,
                    vec![
                        operand(MachineOperandKind::ExternalSymbol {
                            name: "first_external".to_owned(),
                            addend: 0,
                        }),
                        operand(MachineOperandKind::ExternalSymbol {
                            name: "second_external".to_owned(),
                            addend: 0,
                        }),
                        operand(MachineOperandKind::ExternalSymbol {
                            name: "first_external".to_owned(),
                            addend: 4,
                        }),
                    ],
                )],
            )],
        )]))
        .unwrap();

        assert_eq!(
            lowered
                .symbols
                .iter()
                .rev()
                .take(2)
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["second_external", "first_external"]
        );
        assert_eq!(
            lowered
                .symbols
                .iter()
                .filter(|symbol| symbol.name == "first_external")
                .count(),
            1
        );
    }

    #[test]
    fn reports_unknown_block_function_and_global_references() {
        let cases = [
            (
                operand(MachineOperandKind::Block(MachineBlockId::new(8))),
                "unknown block 8",
            ),
            (
                operand(MachineOperandKind::Function(MachineFunctionId::new(8))),
                "unknown function 8",
            ),
            (
                operand(MachineOperandKind::Global {
                    name: "missing".to_owned(),
                    addend: 0,
                }),
                "unknown global \"missing\"",
            ),
        ];
        for (operand, expected) in cases {
            let error = lower_allocated_module(&module(vec![function(
                1,
                "one",
                2,
                vec![block(2, vec![instruction(3, vec![operand])])],
            )]))
            .unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn reports_duplicate_defined_names_without_selecting_a_kind() {
        let error = lower_allocated_module(&MachineModule {
            data_objects: vec![MachineDataObject {
                name: "same".to_owned(),
                bytes: Vec::new(),
                alignment: 1,
                constant: true,
                linkage: MachineLinkage::Internal,
            }],
            functions: vec![function(0, "same", 0, vec![block(0, vec![])])],
        })
        .unwrap_err();
        assert_eq!(
            error,
            X86McModuleLowerError::DuplicateDefinedName {
                name: "same".to_owned(),
                first: DefinedSymbolKind::Data,
                second: DefinedSymbolKind::Function,
            }
        );
    }

    #[test]
    fn reports_residual_allocation_state_with_instruction_context() {
        let mut constrained = operand(MachineOperandKind::Register(MachineRegister::Physical(
            PhysicalRegister::new(X86Register::Ax as u32),
        )));
        constrained.constraint = Some(RegisterConstraint::Class(
            X86RegisterClass::Word.machine_class(),
        ));
        let mut tied = operand(MachineOperandKind::Immediate(0));
        tied.tied_to = Some(OperandIndex::new(0));
        let mut invalid_role = operand(MachineOperandKind::ExternalSymbol {
            name: "target".to_owned(),
            addend: 0,
        });
        invalid_role.role = OperandRole::Use;
        let cases = vec![
            (
                operand(MachineOperandKind::Register(MachineRegister::Virtual(
                    VirtualRegisterId::new(4),
                ))),
                McLowerError::UnresolvedOperand {
                    operand: 0,
                    kind: UnresolvedOperand::VirtualRegister,
                },
            ),
            (
                operand(MachineOperandKind::FrameIndex {
                    index: crate::codegen::machine::FrameIndex::new(2),
                    addend: 0,
                }),
                McLowerError::UnresolvedOperand {
                    operand: 0,
                    kind: UnresolvedOperand::FrameIndex,
                },
            ),
            (constrained, McLowerError::ResidualConstraint { operand: 0 }),
            (tied, McLowerError::ResidualTie { operand: 0 }),
            (invalid_role, McLowerError::InvalidRole { operand: 0 }),
        ];

        for (operand, expected_error) in cases {
            let error = lower_allocated_module(&module(vec![function(
                6,
                "context",
                7,
                vec![block(7, vec![instruction(8, vec![operand])])],
            )]))
            .unwrap_err();
            assert_eq!(
                error,
                X86McModuleLowerError::Instruction {
                    function: MachineFunctionId::new(6),
                    block: MachineBlockId::new(7),
                    instruction: MachineInstructionId::new(8),
                    error: expected_error,
                }
            );
        }
    }

    #[test]
    fn leaves_input_unchanged_and_produces_a_verifiable_module() {
        let input = module(vec![function(
            3,
            "immutable",
            1,
            vec![block(
                1,
                vec![instruction(
                    2,
                    vec![operand(MachineOperandKind::Immediate(4))],
                )],
            )],
        )]);
        let before = input.clone();

        let lowered = lower_allocated_module(&input).unwrap();

        assert_eq!(input, before);
        assert_eq!(lowered.verify(), Ok(()));
    }
}
