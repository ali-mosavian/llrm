//! Target-owned legality checks for selected x86 Machine IR.
//!
//! The generic Machine IR verifier establishes representation invariants. This
//! layer validates only the x86 opcode contracts that are stable before
//! encoding, frame layout, or register allocation.

use std::collections::BTreeMap;

use crate::old::codegen::machine::{
    FrameIndex, FrameObject, InstructionFlags, MachineBlock, MachineFunction, MachineInstruction,
    MachineModule, MachineOperand, MachineOperandKind, MachineRegister, OperandRole, RegisterClass,
    RegisterConstraint, VirtualRegisterId,
};
use crate::support::diagnostic::{Diagnostic, Severity};

use super::{OperandSize, X86Opcode, X86Register, X86RegisterClass, X87MemoryFormat};

/// Validates x86-specific Machine IR opcode and ABI contracts.
///
/// Diagnostics retain the module's stored function, block, and instruction
/// order. The verifier never changes the input. Generic Machine IR structural
/// diagnostics are included first so callers can use this as one complete
/// validation boundary after x86 instruction selection.
pub fn verify_machine(module: &MachineModule) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = module.verify().err().unwrap_or_default();
    let mut verifier = Verifier { diagnostics };
    verifier.verify_module(module);
    if verifier.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(verifier.diagnostics)
    }
}

struct Verifier {
    diagnostics: Vec<Diagnostic>,
}

impl Verifier {
    fn verify_module(&mut self, module: &MachineModule) {
        for function in &module.functions {
            self.verify_function(function);
        }
    }

    fn verify_function(&mut self, function: &MachineFunction) {
        let classes = self.collect_register_classes(function);
        let frames = function
            .frame_objects
            .iter()
            .map(|frame| (frame.index, frame))
            .collect::<BTreeMap<_, _>>();

        for block in &function.blocks {
            for instruction in &block.instructions {
                self.verify_instruction(function, block, instruction, &classes, &frames);
            }
        }
    }

    fn collect_register_classes(
        &mut self,
        function: &MachineFunction,
    ) -> BTreeMap<VirtualRegisterId, RegisterClass> {
        let mut classes = BTreeMap::new();
        for register in &function.virtual_registers {
            if X86RegisterClass::from_machine_class(register.class).is_none() {
                self.error(format!(
                    "machine function {} virtual register {} has unknown x86 register class {}",
                    function.id, register.id, register.class
                ));
            }
            classes.entry(register.id).or_insert(register.class);
        }
        classes
    }

    fn verify_instruction(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        frames: &BTreeMap<FrameIndex, &FrameObject>,
    ) {
        let Some(opcode) = X86Opcode::from_machine_opcode(instruction.opcode) else {
            self.instruction_error(function, block, instruction, "has unknown x86 opcode");
            return;
        };

        if !is_x87_opcode(opcode) {
            self.reject_x87_in_generic_instruction(function, block, instruction, opcode);
        }

        if instruction.flags.anchor && opcode != X86Opcode::Nothing {
            self.instruction_error(
                function,
                block,
                instruction,
                "has anchor lineage but is not the x86 NOTHING opcode",
            );
        }
        if opcode == X86Opcode::Nothing && !instruction.flags.anchor {
            self.instruction_error(
                function,
                block,
                instruction,
                "x86 NOTHING opcode lacks anchor lineage",
            );
        }
        if instruction.flags.spill_reload && instruction.flags.spill_store {
            self.instruction_error(
                function,
                block,
                instruction,
                "is both an allocator spill reload and spill store",
            );
        }
        if opcode == X86Opcode::Nothing && instruction.flags.anchor {
            self.verify_anchor(function, block, instruction);
        }
        self.verify_spill_provenance(function, block, instruction, opcode, frames);

        match opcode {
            X86Opcode::Nothing => {}
            X86Opcode::Load => self.verify_load(function, block, instruction, classes, frames),
            X86Opcode::Store => self.verify_store(function, block, instruction, classes, frames),
            X86Opcode::Lea => self.verify_lea(function, block, instruction, classes),
            X86Opcode::MergeWords => self.verify_word_merge(function, block, instruction, classes),
            X86Opcode::LowWord => self.verify_word_extract(
                function,
                block,
                instruction,
                classes,
                &[X86RegisterClass::Word, X86RegisterClass::Address16],
            ),
            X86Opcode::HighWord => self.verify_word_extract(
                function,
                block,
                instruction,
                classes,
                &[X86RegisterClass::Word],
            ),
            X86Opcode::SignExtendWordToDword => self.verify_word_to_dword_extension(
                function,
                block,
                instruction,
                classes,
                "sign extension",
            ),
            X86Opcode::ZeroExtendWordToDword => self.verify_word_to_dword_extension(
                function,
                block,
                instruction,
                classes,
                "zero extension",
            ),
            X86Opcode::CwdCdq => {
                self.verify_dividend_extension(function, block, instruction, classes)
            }
            X86Opcode::Div | X86Opcode::Idiv => {
                self.verify_divide(function, block, instruction, classes)
            }
            X86Opcode::ShiftLeftDouble => {
                self.verify_shift_left_double(function, block, instruction)
            }
            X86Opcode::CallFar => self.verify_far_call(function, block, instruction, classes),
            X86Opcode::ReturnFar => self.verify_far_return(function, block, instruction, classes),
            X86Opcode::Push => self.verify_push(function, block, instruction, classes, frames),
            X86Opcode::X87Load
            | X86Opcode::X87Store
            | X86Opcode::X87StorePop
            | X86Opcode::X87IntegerLoad
            | X86Opcode::X87IntegerStore
            | X86Opcode::X87IntegerStorePop
            | X86Opcode::X87IntegerStoreTrunc
            | X86Opcode::X87StoreControlWord
            | X86Opcode::X87LoadControlWord => {
                self.verify_x87_memory(function, block, instruction, classes, frames)
            }
            X86Opcode::X87Add
            | X86Opcode::X87Subtract
            | X86Opcode::X87SubtractReverse
            | X86Opcode::X87Multiply
            | X86Opcode::X87Divide
            | X86Opcode::X87DivideReverse
            | X86Opcode::X87Compare
            | X86Opcode::X87ComparePop => {
                self.verify_x87_arithmetic(function, block, instruction, classes, frames)
            }
            X86Opcode::X87AddPop
            | X86Opcode::X87SubtractPop
            | X86Opcode::X87SubtractReversePop
            | X86Opcode::X87MultiplyPop
            | X86Opcode::X87DividePop
            | X86Opcode::X87DivideReversePop => {
                self.verify_x87_pop_arithmetic(function, block, instruction)
            }
            X86Opcode::X87ComparePop2 => self.verify_x87_compare_pop2(function, block, instruction),
            X86Opcode::X87StackLoad => self.verify_x87_stack_load(function, block, instruction),
            X86Opcode::X87StackStorePop => {
                self.verify_x87_stack_store_pop(function, block, instruction)
            }
            X86Opcode::X87Exchange => self.verify_x87_exchange(function, block, instruction),
            X86Opcode::X87LoadZero | X86Opcode::X87LoadOne => {
                self.verify_x87_constant(function, block, instruction, classes)
            }
            X86Opcode::X87ChangeSign | X86Opcode::X87Absolute | X86Opcode::X87SquareRoot => {
                self.verify_x87_unary(function, block, instruction, classes)
            }
            X86Opcode::X87StoreStatusWord => {
                self.verify_x87_status_word(function, block, instruction)
            }
            X86Opcode::Wait | X86Opcode::Sahf => {
                self.verify_x87_no_operand(function, block, instruction)
            }
            _ => {}
        }
    }

    fn verify_spill_provenance(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        opcode: X86Opcode,
        frames: &BTreeMap<FrameIndex, &FrameObject>,
    ) {
        let marker = if instruction.flags.spill_reload {
            Some((X86Opcode::Load, "spill reload"))
        } else if instruction.flags.spill_store {
            Some((X86Opcode::Store, "spill store"))
        } else {
            None
        };
        let Some((expected_opcode, name)) = marker else {
            return;
        };
        if opcode != expected_opcode {
            self.instruction_error(
                function,
                block,
                instruction,
                &format!("{name} provenance is attached to {opcode:?}, not {expected_opcode:?}"),
            );
            return;
        }
        let frame = instruction
            .operands
            .iter()
            .find_map(|operand| match operand.kind {
                MachineOperandKind::FrameIndex { index, .. } => Some(index),
                _ => None,
            });
        let abstract_spill_frame = frame.is_some_and(|index| {
            matches!(
                frames.get(&index),
                Some(FrameObject {
                    kind: crate::old::codegen::machine::FrameObjectKind::Spill,
                    ..
                })
            )
        });
        // Frame-index materialization preserves the marker but replaces the
        // abstract slot by this target's exact BP-plus-immediate address
        // tuple.  Provenance remains explicit; this is the later spelling of
        // the same allocator-owned slot, not an arbitrary source load/store.
        let materialized_spill_frame = match (opcode, instruction.operands.as_slice()) {
            (
                X86Opcode::Load,
                [
                    _,
                    MachineOperand {
                        kind: MachineOperandKind::Register(MachineRegister::Physical(base)),
                        role: OperandRole::Use,
                        constraint: None,
                        tied_to: None,
                    },
                    MachineOperand {
                        kind: MachineOperandKind::Immediate(_),
                        role: OperandRole::None,
                        constraint: None,
                        tied_to: None,
                    },
                ],
            ) => X86Register::from_physical(*base) == Some(X86Register::Bp),
            (
                X86Opcode::Store,
                [
                    MachineOperand {
                        kind: MachineOperandKind::Register(MachineRegister::Physical(base)),
                        role: OperandRole::Use,
                        constraint: None,
                        tied_to: None,
                    },
                    MachineOperand {
                        kind: MachineOperandKind::Immediate(_),
                        role: OperandRole::None,
                        constraint: None,
                        tied_to: None,
                    },
                    _,
                ],
            ) => X86Register::from_physical(*base) == Some(X86Register::Bp),
            _ => false,
        };
        if !abstract_spill_frame && !materialized_spill_frame {
            self.instruction_error(
                function,
                block,
                instruction,
                &format!(
                    "{name} provenance does not name a spill frame object or materialized BP slot"
                ),
            );
        }
    }

    fn verify_anchor(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        if instruction.flags.terminator
            || instruction.flags.call
            || instruction.flags.copy
            || instruction.flags.side_effects
            || instruction.flags.may_load
            || instruction.flags.may_store
            || instruction.flags.volatile
            || instruction.flags.spill_reload
            || instruction.flags.spill_store
        {
            self.instruction_error(
                function,
                block,
                instruction,
                "anchor retains physical effects or spill provenance",
            );
        }
        for (position, operand) in instruction.operands.iter().enumerate() {
            if !matches!(
                operand.kind,
                MachineOperandKind::Register(MachineRegister::Virtual(_))
            ) || operand.constraint.is_some()
                || operand.tied_to.is_some()
            {
                self.operand_error(
                    function,
                    block,
                    instruction,
                    position,
                    "anchor must retain only unconstrained virtual logical def/use operands",
                );
            }
        }
    }

    fn reject_x87_in_generic_instruction(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        opcode: X86Opcode,
    ) {
        if !matches!(
            opcode,
            X86Opcode::Copy
                | X86Opcode::PhiCopy
                | X86Opcode::Mov
                | X86Opcode::Load
                | X86Opcode::Store
                | X86Opcode::Add
                | X86Opcode::Sub
                | X86Opcode::Imul
                | X86Opcode::Idiv
                | X86Opcode::Div
                | X86Opcode::And
                | X86Opcode::Or
                | X86Opcode::Xor
                | X86Opcode::Cmp
                | X86Opcode::Test
        ) {
            return;
        }
        for (position, operand) in instruction.operands.iter().enumerate() {
            if matches!(
                operand.kind,
                MachineOperandKind::Register(MachineRegister::Physical(register))
                    if X86Register::from_physical(register).is_some_and(is_x87_stack_register)
            ) {
                self.operand_error(
                    function,
                    block,
                    instruction,
                    position,
                    "physical x87 stack registers are legal only on x87 target opcodes",
                );
            }
        }
    }

    fn verify_x87_memory(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        frames: &BTreeMap<FrameIndex, &FrameObject>,
    ) {
        let opcode = X86Opcode::from_machine_opcode(instruction.opcode).unwrap();
        let control = matches!(
            opcode,
            X86Opcode::X87StoreControlWord | X86Opcode::X87LoadControlWord
        );
        let (value, format_position, address_position, value_role) = if control {
            (None, 0, 1, OperandRole::None)
        } else {
            let role = if matches!(opcode, X86Opcode::X87Load | X86Opcode::X87IntegerLoad) {
                OperandRole::Def
            } else {
                OperandRole::Use
            };
            (instruction.operands.first(), 1, 2, role)
        };
        let Some(format_operand) = instruction.operands.get(format_position) else {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 memory form has no format operand",
            );
            return;
        };
        if let Some(value) = value {
            self.require_x87_value(
                function,
                block,
                instruction,
                0,
                value,
                value_role,
                classes,
                true,
            );
        }
        let Some(format) = self.require_x87_format(
            function,
            block,
            instruction,
            format_position,
            format_operand,
        ) else {
            return;
        };
        if !x87_memory_format_is_legal(opcode, format) {
            self.operand_error(
                function,
                block,
                instruction,
                format_position,
                "is not legal for this x87 opcode",
            );
        }
        self.verify_memory_address_tail(
            function,
            block,
            instruction,
            address_position,
            format.byte_width(),
            classes,
            frames,
        );
        let valid_flags = match opcode {
            X86Opcode::X87Load | X86Opcode::X87IntegerLoad => is_load_flags(instruction.flags),
            X86Opcode::X87Store
            | X86Opcode::X87StorePop
            | X86Opcode::X87IntegerStore
            | X86Opcode::X87IntegerStorePop
            | X86Opcode::X87IntegerStoreTrunc => is_store_flags(instruction.flags),
            X86Opcode::X87LoadControlWord => is_x87_control_load_flags(instruction.flags),
            X86Opcode::X87StoreControlWord => is_x87_control_store_flags(instruction.flags),
            _ => unreachable!(),
        };
        if !valid_flags {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 memory instruction has invalid memory-effect flags",
            );
        }
    }

    fn verify_x87_arithmetic(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        frames: &BTreeMap<FrameIndex, &FrameObject>,
    ) {
        let opcode = X86Opcode::from_machine_opcode(instruction.opcode).unwrap();
        let is_memory = matches!(
            instruction.operands.get(1).map(|operand| &operand.kind),
            Some(MachineOperandKind::Immediate(_))
        );
        if is_memory {
            let Some(value) = instruction.operands.first() else {
                self.instruction_error(
                    function,
                    block,
                    instruction,
                    "x87 memory arithmetic has no stack operand",
                );
                return;
            };
            let value_role = if matches!(opcode, X86Opcode::X87Compare | X86Opcode::X87ComparePop) {
                OperandRole::Use
            } else {
                OperandRole::UseDef
            };
            self.require_x87_value(
                function,
                block,
                instruction,
                0,
                value,
                value_role,
                classes,
                true,
            );
            let Some(format_operand) = instruction.operands.get(1) else {
                return;
            };
            let Some(format) =
                self.require_x87_format(function, block, instruction, 1, format_operand)
            else {
                return;
            };
            if !x87_memory_format_is_legal(opcode, format) {
                self.operand_error(
                    function,
                    block,
                    instruction,
                    1,
                    "is not legal for this x87 arithmetic opcode",
                );
            }
            self.verify_memory_address_tail(
                function,
                block,
                instruction,
                2,
                format.byte_width(),
                classes,
                frames,
            );
            if !is_load_flags(instruction.flags) {
                self.instruction_error(
                    function,
                    block,
                    instruction,
                    "x87 memory arithmetic must have load-only flags",
                );
            }
            return;
        }
        match instruction.operands.as_slice() {
            [destination, left, right]
                if !matches!(opcode, X86Opcode::X87Compare | X86Opcode::X87ComparePop) =>
            {
                self.require_x87_value(
                    function,
                    block,
                    instruction,
                    0,
                    destination,
                    OperandRole::Def,
                    classes,
                    false,
                );
                self.require_x87_value(
                    function,
                    block,
                    instruction,
                    1,
                    left,
                    OperandRole::Use,
                    classes,
                    false,
                );
                self.require_x87_value(
                    function,
                    block,
                    instruction,
                    2,
                    right,
                    OperandRole::Use,
                    classes,
                    false,
                );
                if instruction.flags != InstructionFlags::NONE {
                    self.instruction_error(
                        function,
                        block,
                        instruction,
                        "selected x87 arithmetic pseudo must have no flags",
                    );
                }
            }
            [left, right] if matches!(opcode, X86Opcode::X87Compare | X86Opcode::X87ComparePop) => {
                if matches!(
                    left.kind,
                    MachineOperandKind::Register(MachineRegister::Physical(_))
                ) {
                    self.require_physical_x87_stack(
                        function,
                        block,
                        instruction,
                        0,
                        left,
                        OperandRole::Use,
                        Some(X86Register::St0),
                    );
                    self.require_physical_x87_stack(
                        function,
                        block,
                        instruction,
                        1,
                        right,
                        OperandRole::Use,
                        None,
                    );
                } else {
                    self.require_x87_value(
                        function,
                        block,
                        instruction,
                        0,
                        left,
                        OperandRole::Use,
                        classes,
                        false,
                    );
                    self.require_x87_value(
                        function,
                        block,
                        instruction,
                        1,
                        right,
                        OperandRole::Use,
                        classes,
                        false,
                    );
                }
                if instruction.flags != InstructionFlags::NONE {
                    self.instruction_error(
                        function,
                        block,
                        instruction,
                        "selected x87 comparison pseudo must have no flags",
                    );
                }
            }
            [destination, source] => {
                self.require_physical_x87_stack(
                    function,
                    block,
                    instruction,
                    0,
                    destination,
                    OperandRole::UseDef,
                    None,
                );
                self.require_physical_x87_stack(
                    function,
                    block,
                    instruction,
                    1,
                    source,
                    OperandRole::Use,
                    None,
                );
                if ![destination, source]
                    .iter()
                    .any(|operand| is_physical_x87_st0(operand))
                {
                    self.instruction_error(
                        function,
                        block,
                        instruction,
                        "physical x87 arithmetic requires st(0) as one operand",
                    );
                }
                if instruction.flags != InstructionFlags::NONE {
                    self.instruction_error(
                        function,
                        block,
                        instruction,
                        "physical x87 arithmetic must have no flags",
                    );
                }
            }
            _ => self.instruction_error(
                function,
                block,
                instruction,
                "x87 arithmetic has an invalid selected or physical operand shape",
            ),
        }
    }

    fn verify_x87_pop_arithmetic(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        let [destination, source] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 pop arithmetic requires [st(i) usedef, st0 use]",
            );
            return;
        };
        self.require_physical_x87_stack(
            function,
            block,
            instruction,
            0,
            destination,
            OperandRole::UseDef,
            None,
        );
        self.require_physical_x87_stack(
            function,
            block,
            instruction,
            1,
            source,
            OperandRole::Use,
            Some(X86Register::St0),
        );
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 pop arithmetic must have no flags",
            );
        }
    }

    fn verify_x87_compare_pop2(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        let [left, right] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "fcompp requires [st0 use, st1 use]",
            );
            return;
        };
        self.require_physical_x87_stack(
            function,
            block,
            instruction,
            0,
            left,
            OperandRole::Use,
            Some(X86Register::St0),
        );
        self.require_physical_x87_stack(
            function,
            block,
            instruction,
            1,
            right,
            OperandRole::Use,
            Some(X86Register::St1),
        );
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(function, block, instruction, "fcompp must have no flags");
        }
    }

    fn verify_x87_stack_load(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        let [destination, source] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 stack load requires [st0 def, st(i) use]",
            );
            return;
        };
        self.require_physical_x87_stack(
            function,
            block,
            instruction,
            0,
            destination,
            OperandRole::Def,
            Some(X86Register::St0),
        );
        self.require_physical_x87_stack(
            function,
            block,
            instruction,
            1,
            source,
            OperandRole::Use,
            None,
        );
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 stack load must have no flags",
            );
        }
    }

    fn verify_x87_stack_store_pop(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        let [destination, source] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 stack store-pop requires [st(i) def, st0 use]",
            );
            return;
        };
        self.require_physical_x87_stack(
            function,
            block,
            instruction,
            0,
            destination,
            OperandRole::Def,
            None,
        );
        self.require_physical_x87_stack(
            function,
            block,
            instruction,
            1,
            source,
            OperandRole::Use,
            Some(X86Register::St0),
        );
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 stack store-pop must have no flags",
            );
        }
    }

    fn verify_x87_exchange(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        let [left, right] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "fxch requires [st0 usedef, st(i) usedef]",
            );
            return;
        };
        self.require_physical_x87_stack(
            function,
            block,
            instruction,
            0,
            left,
            OperandRole::UseDef,
            Some(X86Register::St0),
        );
        self.require_physical_x87_stack(
            function,
            block,
            instruction,
            1,
            right,
            OperandRole::UseDef,
            None,
        );
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(function, block, instruction, "fxch must have no flags");
        }
    }

    fn verify_x87_constant(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        let [value] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 constant requires one stack definition",
            );
            return;
        };
        self.require_x87_value(
            function,
            block,
            instruction,
            0,
            value,
            OperandRole::Def,
            classes,
            true,
        );
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 constant must have no flags",
            );
        }
    }

    fn verify_x87_unary(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        match instruction.operands.as_slice() {
            [destination, source] => {
                self.require_x87_value(
                    function,
                    block,
                    instruction,
                    0,
                    destination,
                    OperandRole::Def,
                    classes,
                    false,
                );
                self.require_x87_value(
                    function,
                    block,
                    instruction,
                    1,
                    source,
                    OperandRole::Use,
                    classes,
                    false,
                );
            }
            [value] => self.require_physical_x87_stack(
                function,
                block,
                instruction,
                0,
                value,
                OperandRole::UseDef,
                Some(X86Register::St0),
            ),
            _ => self.instruction_error(
                function,
                block,
                instruction,
                "x87 unary requires selected [x87 def, x87 use] or physical [st0 usedef]",
            ),
        }
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(function, block, instruction, "x87 unary must have no flags");
        }
    }

    fn verify_x87_status_word(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        let [destination] = instruction.operands.as_slice() else {
            self.instruction_error(function, block, instruction, "fnstsw requires [ax def]");
            return;
        };
        let valid = matches!(destination, MachineOperand { kind: MachineOperandKind::Register(MachineRegister::Physical(register)), role: OperandRole::Def, constraint: None, tied_to: None } if *register == X86Register::Ax.physical());
        if !valid {
            self.operand_error(
                function,
                block,
                instruction,
                0,
                "fnstsw requires an unconstrained physical AX definition",
            );
        }
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(function, block, instruction, "fnstsw must have no flags");
        }
    }

    fn verify_x87_no_operand(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        if !instruction.operands.is_empty() {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 no-operand form must have no operands",
            );
        }
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(
                function,
                block,
                instruction,
                "x87 no-operand form must have no flags",
            );
        }
    }

    fn require_x87_value(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
        role: OperandRole,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        require_st0_if_physical: bool,
    ) {
        match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(id)) => {
                if operand.role != role {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        format!("must have {:?} role", role),
                    );
                }
                if classes.get(&id).copied() != Some(X86RegisterClass::X87.machine_class()) {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "must be an x87 virtual register",
                    );
                }
                if operand.constraint.is_some() || operand.tied_to.is_some() {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "selected x87 virtual register must not retain allocation constraint or tie metadata",
                    );
                }
            }
            MachineOperandKind::Register(MachineRegister::Physical(_)) => {
                self.require_physical_x87_stack(
                    function,
                    block,
                    instruction,
                    position,
                    operand,
                    role,
                    require_st0_if_physical.then_some(X86Register::St0),
                );
            }
            _ => self.operand_error(
                function,
                block,
                instruction,
                position,
                "must be an x87 register",
            ),
        }
    }

    fn require_physical_x87_stack(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
        role: OperandRole,
        expected: Option<X86Register>,
    ) {
        let valid = matches!(
            operand,
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                role: actual_role,
                constraint: None,
                tied_to: None,
            } if *actual_role == role
                && X86Register::from_physical(*register).is_some_and(|register| {
                    is_x87_stack_register(register) && expected.is_none_or(|expected| register == expected)
                })
        );
        if !valid {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                "must be an unconstrained physical x87 stack register with the required role",
            );
        }
    }

    fn require_x87_format(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
    ) -> Option<X87MemoryFormat> {
        let MachineOperand {
            kind: MachineOperandKind::Immediate(raw),
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        } = operand
        else {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                "must be an unconstrained x87 memory-format immediate",
            );
            return None;
        };
        let Ok(raw) = u8::try_from(*raw) else {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                "must be a valid x87 memory-format immediate",
            );
            return None;
        };
        let Some(format) = X87MemoryFormat::from_raw(raw) else {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                "must be a valid x87 memory-format immediate",
            );
            return None;
        };
        Some(format)
    }

    fn verify_memory_address_tail(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        width: u32,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        frames: &BTreeMap<FrameIndex, &FrameObject>,
    ) {
        self.verify_memory_address_range(
            function,
            block,
            instruction,
            position,
            instruction.operands.len(),
            width,
            classes,
            frames,
        );
    }

    fn verify_memory_address_range(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        end: usize,
        width: u32,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        frames: &BTreeMap<FrameIndex, &FrameObject>,
    ) {
        let tail = instruction.operands.get(position..end).unwrap_or_default();
        match tail {
            [address] => match address.kind {
                MachineOperandKind::Global { .. }
                    if matches!(address.role, OperandRole::None)
                        && address.constraint.is_none()
                        && address.tied_to.is_none() => {}
                _ => self.verify_memory_address(
                    function,
                    block,
                    instruction,
                    position,
                    address,
                    classes,
                    frames,
                    Some(width),
                ),
            },
            [base, selector]
                if matches!(selector.kind, MachineOperandKind::Register(_)) =>
            {
                self.verify_segmented_memory_address(
                    function,
                    block,
                    instruction,
                    position,
                    position + 1,
                    base,
                    selector,
                    classes,
                )
            }
            [base, displacement] => self.verify_materialized_memory_address(
                function,
                block,
                instruction,
                position,
                base,
                displacement,
                classes,
            ),
            _ => self.instruction_error(
                function,
                block,
                instruction,
                "memory operand requires [address], [global], [address16 use, displacement], or [address16 use, ES use]",
            ),
        }
    }

    fn verify_load(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        frames: &BTreeMap<FrameIndex, &FrameObject>,
    ) {
        let (destination, address) = match instruction.operands.as_slice() {
            [destination, address] => (destination, (address, None)),
            [destination, base, selector]
                if matches!(selector.kind, MachineOperandKind::Register(_)) =>
            {
                (destination, (base, Some(selector)))
            }
            [destination, base, displacement] => (destination, (base, Some(displacement))),
            _ => {
                self.instruction_error(
                    function,
                    block,
                    instruction,
                    "load requires [register def, address] or [register def, address16 use, displacement]",
                );
                return;
            }
        };
        let width = self.require_sized_register(
            function,
            block,
            instruction,
            0,
            destination,
            OperandRole::Def,
            classes,
        );
        match address {
            (address, None) => self.verify_memory_address(
                function,
                block,
                instruction,
                1,
                address,
                classes,
                frames,
                width,
            ),
            (base, Some(third)) if matches!(third.kind, MachineOperandKind::Register(_)) => self
                .verify_segmented_memory_address(
                    function,
                    block,
                    instruction,
                    1,
                    2,
                    base,
                    third,
                    classes,
                ),
            (base, Some(displacement)) => self.verify_materialized_memory_address(
                function,
                block,
                instruction,
                1,
                base,
                displacement,
                classes,
            ),
        }
        if !is_load_flags(instruction.flags) {
            self.instruction_error(
                function,
                block,
                instruction,
                "load must have load-only flags",
            );
        }
    }

    fn verify_store(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        frames: &BTreeMap<FrameIndex, &FrameObject>,
    ) {
        let immediate_position = instruction.operands.len().saturating_sub(2);
        if let [width, value] = &instruction.operands[immediate_position..] {
            if matches!(width.kind, MachineOperandKind::Immediate(_))
                && matches!(value.kind, MachineOperandKind::Immediate(_))
            {
                let Some(width) = plain_store_width(width) else {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        immediate_position,
                        "store immediate width must be a plain 8, 16, or 32 immediate",
                    );
                    return;
                };
                if value.role != OperandRole::None
                    || value.constraint.is_some()
                    || value.tied_to.is_some()
                {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        immediate_position + 1,
                        "stored immediate must be a plain immediate",
                    );
                }
                self.verify_memory_address_range(
                    function,
                    block,
                    instruction,
                    0,
                    immediate_position,
                    width / 8,
                    classes,
                    frames,
                );
                if !is_store_flags(instruction.flags) {
                    self.instruction_error(
                        function,
                        block,
                        instruction,
                        "immediate store must have store-only flags",
                    );
                }
                return;
            }
        }
        let (address, source_position, source) = match instruction.operands.as_slice() {
            [address, source] => ((address, None), 1, source),
            // The second operand distinguishes the two three-operand forms:
            // a displaced Store is [address16, displacement, source], while segmented
            // memory is [address, source, ES]. Testing only the final operand
            // classified a valid displaced Store as segmented whenever its source
            // was a register.
            [base, displacement, source]
                if matches!(displacement.kind, MachineOperandKind::Immediate(_)) =>
            {
                ((base, Some(displacement)), 2, source)
            }
            [base, source, selector]
                if matches!(selector.kind, MachineOperandKind::Register(_)) =>
            {
                ((base, Some(selector)), 1, source)
            }
            _ => {
                self.instruction_error(
                    function,
                    block,
                    instruction,
                    "store requires [address, register use] or [address16 use, displacement, register use]",
                );
                return;
            }
        };
        let width = self.require_sized_register(
            function,
            block,
            instruction,
            source_position,
            source,
            OperandRole::Use,
            classes,
        );
        match address {
            (address, None) => self.verify_memory_address(
                function,
                block,
                instruction,
                0,
                address,
                classes,
                frames,
                width,
            ),
            (base, Some(third)) if matches!(third.kind, MachineOperandKind::Register(_)) => self
                .verify_segmented_memory_address(
                    function,
                    block,
                    instruction,
                    0,
                    2,
                    base,
                    third,
                    classes,
                ),
            (base, Some(displacement)) => self.verify_materialized_memory_address(
                function,
                block,
                instruction,
                0,
                base,
                displacement,
                classes,
            ),
        }
        if !is_store_flags(instruction.flags) {
            self.instruction_error(
                function,
                block,
                instruction,
                "store must have store and side-effect flags only",
            );
        }
    }

    fn verify_memory_address(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        address: &MachineOperand,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        frames: &BTreeMap<FrameIndex, &FrameObject>,
        width: Option<u32>,
    ) {
        match &address.kind {
            MachineOperandKind::FrameIndex { index, addend } => {
                if !matches!(address.role, OperandRole::None) {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "must be a frame index with role none",
                    );
                }
                if let (Some(frame), Some(width)) = (frames.get(index), width) {
                    let end = addend.checked_add(i64::from(width));
                    if *addend < 0 || end.is_none_or(|end| end > i64::from(frame.size)) {
                        self.operand_error(
                            function,
                            block,
                            instruction,
                            position,
                            format!(
                                "access at addend {addend} with width {width} exceeds frame index {} size {}",
                                index, frame.size
                            ),
                        );
                    }
                }
            }
            MachineOperandKind::Register(MachineRegister::Virtual(id))
                if matches!(address.role, OperandRole::Use) =>
            {
                if classes.get(id).copied() != Some(X86RegisterClass::Address16.machine_class()) {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "must have address16 x86 register class",
                    );
                }
            }
            MachineOperandKind::Register(MachineRegister::Physical(register))
                if matches!(address.role, OperandRole::Use)
                    && address.constraint.is_none()
                    && address.tied_to.is_none() =>
            {
                let valid = X86Register::from_physical(*register).is_some_and(|register| {
                    X86RegisterClass::Address16.members().contains(&register)
                });
                if !valid {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "must use a physical address16 x86 register",
                    );
                }
            }
            _ => self.operand_error(
                function,
                block,
                instruction,
                position,
                "must be a frame index or address16 register use",
            ),
        }
    }

    fn verify_materialized_memory_address(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        base: &MachineOperand,
        displacement: &MachineOperand,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        self.verify_materialized_address_base(
            function,
            block,
            instruction,
            position,
            base,
            classes,
        );
        if !matches!(
            displacement,
            MachineOperand {
                kind: MachineOperandKind::Immediate(_),
                role: OperandRole::None,
                constraint: None,
                tied_to: None,
            }
        ) {
            self.operand_error(
                function,
                block,
                instruction,
                position + 1,
                "materialized address displacement must be an unconstrained immediate",
            );
        }
    }

    fn verify_materialized_address_base(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        base: &MachineOperand,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        match base {
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(id)),
                role: OperandRole::Use,
                constraint: None,
                tied_to: None,
            } if classes.get(id).copied() == Some(X86RegisterClass::Address16.machine_class()) => {}
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                role: OperandRole::Use,
                constraint: None,
                tied_to: None,
            } if X86Register::from_physical(*register).is_some_and(|register| {
                X86RegisterClass::Address16.members().contains(&register)
            }) => {}
            _ => self.operand_error(
                function,
                block,
                instruction,
                position,
                "materialized address base must be an unconstrained address16 register use",
            ),
        }
    }

    fn verify_segmented_memory_address(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        base_position: usize,
        selector_position: usize,
        base: &MachineOperand,
        selector: &MachineOperand,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        self.require_register_class(
            function,
            block,
            instruction,
            base_position,
            base,
            OperandRole::Use,
            X86RegisterClass::Address16,
            classes,
        );
        if !matches!(
            selector,
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                role: OperandRole::Use,
                constraint: None,
                tied_to: None,
            } if *register == X86Register::Es.physical()
        ) {
            self.operand_error(
                function,
                block,
                instruction,
                selector_position,
                "segmented memory selector must be an unconstrained physical ES use",
            );
        }
    }

    fn verify_lea(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        let (destination, address) = match instruction.operands.as_slice() {
            [destination, address] => (destination, (address, None)),
            [destination, base, displacement] => (destination, (base, Some(displacement))),
            _ => {
                self.instruction_error(
                    function,
                    block,
                    instruction,
                    "lea requires [register def, frame index or global] or [register def, address16 use, displacement]",
                );
                return;
            }
        };
        self.require_address_register(
            function,
            block,
            instruction,
            0,
            destination,
            OperandRole::Def,
            classes,
        );
        match address {
            (address, None) => match &address.kind {
                MachineOperandKind::FrameIndex { .. }
                    if matches!(address.role, OperandRole::None) => {}
                MachineOperandKind::Global { .. } if matches!(address.role, OperandRole::None) => {}
                _ => self.operand_error(
                    function,
                    block,
                    instruction,
                    1,
                    "must be a frame index or global with role none",
                ),
            },
            (base, Some(displacement)) => self.verify_materialized_memory_address(
                function,
                block,
                instruction,
                1,
                base,
                displacement,
                classes,
            ),
        }
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(function, block, instruction, "lea must have no flags");
        }
    }

    fn verify_word_merge(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        let [destination, low, high] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "mergewords requires [dword virtual def, word virtual use, word virtual use]",
            );
            return;
        };
        self.require_register_class(
            function,
            block,
            instruction,
            0,
            destination,
            OperandRole::Def,
            X86RegisterClass::Dword,
            classes,
        );
        for (position, operand) in [(1, low), (2, high)] {
            self.require_register_class(
                function,
                block,
                instruction,
                position,
                operand,
                OperandRole::Use,
                X86RegisterClass::Word,
                classes,
            );
        }
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(
                function,
                block,
                instruction,
                "mergewords must have no flags",
            );
        }
    }

    fn verify_word_extract(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        destination_classes: &[X86RegisterClass],
    ) {
        let [destination, source] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "word extraction requires [word virtual def, dword virtual use]",
            );
            return;
        };
        self.require_one_of_register_classes(
            function,
            block,
            instruction,
            0,
            destination,
            OperandRole::Def,
            destination_classes,
            classes,
        );
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(
                function,
                block,
                instruction,
                "word extraction must have no flags",
            );
        }
        self.require_register_class(
            function,
            block,
            instruction,
            1,
            source,
            OperandRole::Use,
            X86RegisterClass::Dword,
            classes,
        );
    }

    fn verify_word_to_dword_extension(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        name: &str,
    ) {
        let [destination, source] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                format!("{name} requires [dword register def, word register use]"),
            );
            return;
        };
        self.require_register_class(
            function,
            block,
            instruction,
            0,
            destination,
            OperandRole::Def,
            X86RegisterClass::Dword,
            classes,
        );
        self.require_register_class(
            function,
            block,
            instruction,
            1,
            source,
            OperandRole::Use,
            X86RegisterClass::Word,
            classes,
        );
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(
                function,
                block,
                instruction,
                format!("{name} must have no flags"),
            );
        }
    }

    fn verify_shift_left_double(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
    ) {
        let [destination, source, count] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "shift-left-double requires [physical dword usedef, physical dword use, immediate 16]",
            );
            return;
        };
        self.require_physical_dword(
            function,
            block,
            instruction,
            0,
            destination,
            OperandRole::UseDef,
        );
        self.require_physical_dword(function, block, instruction, 1, source, OperandRole::Use);
        if !matches!(
            count,
            MachineOperand {
                kind: MachineOperandKind::Immediate(16),
                role: OperandRole::None,
                constraint: None,
                tied_to: None,
            }
        ) {
            self.operand_error(
                function,
                block,
                instruction,
                2,
                "must be an unconstrained immediate count of 16",
            );
        }
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(
                function,
                block,
                instruction,
                "shift-left-double must have no flags",
            );
        }
    }

    fn verify_dividend_extension(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        let [high, low] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "cwd/cdq requires [fixed high register def, fixed low register use]",
            );
            return;
        };
        let Some(width) = self.divide_width(
            function,
            block,
            instruction,
            high,
            OperandRole::Def,
            low,
            OperandRole::Use,
            classes,
        ) else {
            return;
        };
        let (low_register, high_register) = divide_registers(width);
        self.require_fixed_register(
            function,
            block,
            instruction,
            0,
            high,
            OperandRole::Def,
            high_register,
            classes,
        );
        self.require_fixed_register(
            function,
            block,
            instruction,
            1,
            low,
            OperandRole::Use,
            low_register,
            classes,
        );
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(function, block, instruction, "cwd/cdq must have no flags");
        }
    }

    fn verify_divide(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        let [high, low, divisor, quotient, remainder] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "div/idiv requires [fixed high use, fixed low use, flexible divisor use, fixed quotient def, fixed remainder def]",
            );
            return;
        };
        let Some(width) = self.divide_width(
            function,
            block,
            instruction,
            high,
            OperandRole::Use,
            low,
            OperandRole::Use,
            classes,
        ) else {
            return;
        };
        let divisor_width = self.require_sized_register(
            function,
            block,
            instruction,
            2,
            divisor,
            OperandRole::Use,
            classes,
        );
        let quotient_width = self.require_sized_register(
            function,
            block,
            instruction,
            3,
            quotient,
            OperandRole::Def,
            classes,
        );
        let remainder_width = self.require_sized_register(
            function,
            block,
            instruction,
            4,
            remainder,
            OperandRole::Def,
            classes,
        );
        for (position, actual) in [
            (2, divisor_width),
            (3, quotient_width),
            (4, remainder_width),
        ] {
            if actual != Some(width) {
                self.operand_error(
                    function,
                    block,
                    instruction,
                    position,
                    "must have the dividend width",
                );
            }
        }
        if divisor.constraint.is_some() || divisor.tied_to.is_some() {
            self.operand_error(
                function,
                block,
                instruction,
                2,
                "divisor must be an unconstrained register use",
            );
        }
        let (low_register, high_register) = divide_registers(width);
        self.require_fixed_register(
            function,
            block,
            instruction,
            0,
            high,
            OperandRole::Use,
            high_register,
            classes,
        );
        self.require_fixed_register(
            function,
            block,
            instruction,
            1,
            low,
            OperandRole::Use,
            low_register,
            classes,
        );
        self.require_fixed_register(
            function,
            block,
            instruction,
            3,
            quotient,
            OperandRole::Def,
            low_register,
            classes,
        );
        self.require_fixed_register(
            function,
            block,
            instruction,
            4,
            remainder,
            OperandRole::Def,
            high_register,
            classes,
        );
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(function, block, instruction, "div/idiv must have no flags");
        }
    }

    fn divide_width(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        high: &MachineOperand,
        high_role: OperandRole,
        low: &MachineOperand,
        low_role: OperandRole,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) -> Option<u32> {
        let high_width =
            self.require_sized_register(function, block, instruction, 0, high, high_role, classes);
        let low_width =
            self.require_sized_register(function, block, instruction, 1, low, low_role, classes);
        let width = high_width?;
        if low_width != Some(width) || !matches!(width, 2 | 4) {
            self.operand_error(
                function,
                block,
                instruction,
                1,
                "must be the matching 16- or 32-bit dividend half",
            );
            return None;
        }
        Some(width)
    }

    fn require_fixed_register(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
        role: OperandRole,
        expected: X86Register,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        let class = if matches!(expected, X86Register::Ax | X86Register::Dx) {
            X86RegisterClass::Word
        } else {
            X86RegisterClass::Dword
        };
        self.require_register_class(
            function,
            block,
            instruction,
            position,
            operand,
            role,
            class,
            classes,
        );
        let valid = matches!(
            operand,
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(_)),
                constraint: Some(RegisterConstraint::Fixed(register)),
                tied_to: None,
                ..
            } if *register == expected.physical()
        ) || matches!(
            operand,
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                constraint: None,
                tied_to: None,
                ..
            } if *register == expected.physical()
        );
        if !valid {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                format!("must be a fixed virtual or allocated physical {expected:?} register"),
            );
        }
    }

    fn verify_far_call(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        let Some((callee, outputs)) = instruction.operands.split_first() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "far call requires a function or external symbol callee",
            );
            return;
        };
        if !matches!(
            callee.kind,
            MachineOperandKind::Function(_) | MachineOperandKind::ExternalSymbol { .. }
        ) || !matches!(callee.role, OperandRole::None)
        {
            self.operand_error(
                function,
                block,
                instruction,
                0,
                "must be a function or external symbol with role none",
            );
        }
        let mut saw_definition = false;
        let mut fixed_definitions = Vec::new();
        for (index, operand) in outputs.iter().enumerate() {
            let position = index + 1;
            match operand.role {
                OperandRole::Use => {
                    if saw_definition {
                        self.operand_error(
                            function,
                            block,
                            instruction,
                            position,
                            "fixed call uses must precede definitions",
                        );
                    }
                    self.require_abi_register(
                        function,
                        block,
                        instruction,
                        position,
                        operand,
                        OperandRole::Use,
                        classes,
                    );
                }
                OperandRole::Def => {
                    saw_definition = true;
                    if let Some(physical) = self.require_abi_register(
                        function,
                        block,
                        instruction,
                        position,
                        operand,
                        OperandRole::Def,
                        classes,
                    ) {
                        if fixed_definitions
                            .iter()
                            .any(|previous: &X86Register| previous.overlaps(physical))
                        {
                            self.operand_error(
                                function,
                                block,
                                instruction,
                                position,
                                "fixed call definition aliases an earlier definition",
                            );
                        }
                        fixed_definitions.push(physical);
                    }
                }
                OperandRole::None | OperandRole::UseDef => self.operand_error(
                    function,
                    block,
                    instruction,
                    position,
                    "must be an ABI register use or definition",
                ),
            }
        }
        if !is_call_flags(instruction.flags) {
            self.instruction_error(
                function,
                block,
                instruction,
                "far call must have call flags without terminator or copy",
            );
        }
    }

    fn verify_far_return(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        let operands = instruction.operands.as_slice();
        let (values, cleanup) = match operands.last() {
            Some(MachineOperand {
                kind: MachineOperandKind::Immediate(_),
                ..
            }) => (&operands[..operands.len() - 1], Some(operands.len() - 1)),
            _ => (operands, None),
        };
        for (index, value) in values.iter().enumerate() {
            self.require_abi_register(
                function,
                block,
                instruction,
                index,
                value,
                OperandRole::Use,
                classes,
            );
        }
        if let Some(index) = cleanup {
            if !matches!(instruction.operands[index].role, OperandRole::None) {
                self.operand_error(
                    function,
                    block,
                    instruction,
                    index,
                    "cleanup immediate must have role none",
                );
            }
            if let MachineOperandKind::Immediate(value) = instruction.operands[index].kind {
                if !(0..=i64::from(u16::MAX)).contains(&value) {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        index,
                        "cleanup immediate must be in 0..=65535",
                    );
                }
            }
        }
        if instruction.flags
            != (InstructionFlags {
                terminator: true,
                ..InstructionFlags::NONE
            })
        {
            self.instruction_error(
                function,
                block,
                instruction,
                "far return must have only the terminator flag",
            );
        }
    }

    fn verify_push(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
        frames: &BTreeMap<FrameIndex, &FrameObject>,
    ) {
        match instruction.operands.as_slice() {
            [source] => {
                self.require_register_role(
                    function,
                    block,
                    instruction,
                    0,
                    source,
                    OperandRole::Use,
                );
                if instruction.flags != InstructionFlags::NONE {
                    self.instruction_error(
                        function,
                        block,
                        instruction,
                        "register push must have no flags",
                    );
                }
            }
            [width, value]
                if matches!(value.kind, MachineOperandKind::Immediate(_))
                    && plain_push_width(width).is_some() =>
            {
                if value.role != OperandRole::None
                    || value.constraint.is_some()
                    || value.tied_to.is_some()
                {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        1,
                        "push immediate must be a plain immediate",
                    );
                }
                if instruction.flags != InstructionFlags::NONE {
                    self.instruction_error(
                        function,
                        block,
                        instruction,
                        "immediate push must have no flags",
                    );
                }
            }
            [width, ..] => {
                let Some(width) = plain_push_width(width) else {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        0,
                        "push memory width must be a plain 16 or 32 immediate",
                    );
                    return;
                };
                self.verify_memory_address_tail(
                    function,
                    block,
                    instruction,
                    1,
                    width / 8,
                    classes,
                    frames,
                );
                if !is_load_flags(instruction.flags) {
                    self.instruction_error(
                        function,
                        block,
                        instruction,
                        "memory push must have load-only flags",
                    );
                }
            }
            [] => self.instruction_error(
                function,
                block,
                instruction,
                "push requires a register or an explicit immediate or memory source",
            ),
        }
    }

    fn require_sized_register(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
        role: OperandRole,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) -> Option<u32> {
        self.require_register_role(function, block, instruction, position, operand, role);
        let width = match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(id)) => classes
                .get(&id)
                .and_then(|class| X86RegisterClass::from_machine_class(*class))
                .and_then(class_width),
            MachineOperandKind::Register(MachineRegister::Physical(register)) => {
                X86Register::from_physical(register).and_then(register_width)
            }
            _ => None,
        };
        if width.is_none() && matches!(operand.kind, MachineOperandKind::Register(_)) {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                "must have byte, word, or dword x86 width",
            );
        }
        width
    }

    fn require_register_class(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
        role: OperandRole,
        expected: X86RegisterClass,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        if operand.role != role {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                format!("must have {:?} role", role),
            );
        }
        let valid = match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(id)) => {
                classes.get(&id).copied() == Some(expected.machine_class())
            }
            MachineOperandKind::Register(MachineRegister::Physical(register))
                if operand.constraint.is_none() && operand.tied_to.is_none() =>
            {
                X86Register::from_physical(register)
                    .is_some_and(|register| expected.members().contains(&register))
            }
            _ => false,
        };
        if !valid {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                format!("must be a {} x86 register", class_name(expected)),
            );
        }
    }

    fn require_one_of_register_classes(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
        role: OperandRole,
        expected: &[X86RegisterClass],
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        if operand.role != role {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                format!("must have {:?} role", role),
            );
        }
        let valid = match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(id)) => classes
                .get(&id)
                .and_then(|class| X86RegisterClass::from_machine_class(*class))
                .is_some_and(|class| expected.contains(&class)),
            MachineOperandKind::Register(MachineRegister::Physical(register))
                if operand.constraint.is_none() && operand.tied_to.is_none() =>
            {
                X86Register::from_physical(register).is_some_and(|register| {
                    expected
                        .iter()
                        .any(|class| class.members().contains(&register))
                })
            }
            _ => false,
        };
        if !valid {
            let description = if expected.len() == 1 && expected[0] == X86RegisterClass::Word {
                "must be a word x86 register"
            } else {
                "must be a word or address16 x86 register"
            };
            self.operand_error(function, block, instruction, position, description);
        }
    }

    fn require_address_register(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
        role: OperandRole,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        if operand.role != role {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                format!("must have {:?} role", role),
            );
        }
        let valid = match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(id)) => {
                classes.get(&id).copied() == Some(X86RegisterClass::Address16.machine_class())
            }
            MachineOperandKind::Register(MachineRegister::Physical(register)) => {
                X86Register::from_physical(register).is_some_and(|register| {
                    X86RegisterClass::Address16.members().contains(&register)
                })
            }
            _ => false,
        };
        if !valid {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                "must be an address16 x86 register",
            );
        }
    }

    fn require_abi_register(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
        role: OperandRole,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) -> Option<X86Register> {
        if operand.role != role {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                format!("must have {:?} role", role),
            );
        }
        match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(id)) => {
                let Some(RegisterConstraint::Fixed(physical)) = operand.constraint else {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "must have a fixed ABI register constraint",
                    );
                    return None;
                };
                let Some(class) = classes
                    .get(&id)
                    .and_then(|class| X86RegisterClass::from_machine_class(*class))
                else {
                    return None;
                };
                let Some(physical) = X86Register::from_physical(physical) else {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "must constrain a known x86 physical register",
                    );
                    return None;
                };
                if !class.members().contains(&physical) {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "fixed ABI register is incompatible with the virtual register class",
                    );
                    return None;
                }
                Some(physical)
            }
            MachineOperandKind::Register(MachineRegister::Physical(physical)) => {
                if operand.constraint.is_some() || operand.tied_to.is_some() {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "allocated ABI register must not retain a constraint or tie",
                    );
                    return None;
                }
                X86Register::from_physical(physical).or_else(|| {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "must name a known x86 physical ABI register",
                    );
                    None
                })
            }
            _ => {
                self.operand_error(
                    function,
                    block,
                    instruction,
                    position,
                    "must be a virtual or physical ABI register",
                );
                None
            }
        }
    }

    fn require_register_role(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
        role: OperandRole,
    ) {
        if !matches!(operand.kind, MachineOperandKind::Register(_)) {
            self.operand_error(function, block, instruction, position, "must be a register");
            return;
        }
        if operand.role != role {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                format!("must have {:?} role", role),
            );
        }
    }

    fn require_physical_dword(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        operand: &MachineOperand,
        role: OperandRole,
    ) {
        let valid = matches!(
            operand,
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                role: actual_role,
                constraint: None,
                tied_to: None,
            } if *actual_role == role
                && X86Register::from_physical(*register).is_some_and(|register| {
                    X86RegisterClass::Dword.members().contains(&register)
                })
        );
        if !valid {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                format!(
                    "must be an unconstrained physical dword register with {:?} role",
                    role
                ),
            );
        }
    }

    fn instruction_error(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        detail: impl AsRef<str>,
    ) {
        self.error(format!(
            "machine function {} block {} instruction {} {}",
            function.id,
            block.id,
            instruction.id,
            detail.as_ref()
        ));
    }

    fn operand_error(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        detail: impl AsRef<str>,
    ) {
        self.error(format!(
            "machine function {} block {} instruction {} operand {} {}",
            function.id,
            block.id,
            instruction.id,
            position,
            detail.as_ref()
        ));
    }

    fn error(&mut self, message: String) {
        self.diagnostics
            .push(Diagnostic::new(Severity::Error, message));
    }
}

fn is_load_flags(flags: InstructionFlags) -> bool {
    flags.may_load
        && !flags.terminator
        && !flags.call
        && !flags.copy
        && !flags.side_effects
        && !flags.may_store
}

fn is_store_flags(flags: InstructionFlags) -> bool {
    flags.may_store
        && flags.side_effects
        && !flags.terminator
        && !flags.call
        && !flags.copy
        && !flags.may_load
}

fn plain_push_width(operand: &MachineOperand) -> Option<u32> {
    match operand {
        MachineOperand {
            kind: MachineOperandKind::Immediate(bits @ (16 | 32)),
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        } => u32::try_from(*bits).ok(),
        _ => None,
    }
}

fn plain_store_width(operand: &MachineOperand) -> Option<u32> {
    match operand {
        MachineOperand {
            kind: MachineOperandKind::Immediate(bits @ (8 | 16 | 32)),
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        } => u32::try_from(*bits).ok(),
        _ => None,
    }
}

fn is_x87_control_load_flags(flags: InstructionFlags) -> bool {
    flags.may_load
        && flags.side_effects
        && !flags.terminator
        && !flags.call
        && !flags.copy
        && !flags.may_store
}

fn is_x87_control_store_flags(flags: InstructionFlags) -> bool {
    flags.may_store
        && flags.side_effects
        && !flags.terminator
        && !flags.call
        && !flags.copy
        && !flags.may_load
}

fn is_x87_stack_register(register: X86Register) -> bool {
    matches!(
        register,
        X86Register::St0
            | X86Register::St1
            | X86Register::St2
            | X86Register::St3
            | X86Register::St4
            | X86Register::St5
            | X86Register::St6
            | X86Register::St7
    )
}

fn is_physical_x87_st0(operand: &MachineOperand) -> bool {
    matches!(
        operand.kind,
        MachineOperandKind::Register(MachineRegister::Physical(register))
            if register == X86Register::St0.physical()
    )
}

fn is_x87_opcode(opcode: X86Opcode) -> bool {
    matches!(
        opcode,
        X86Opcode::X87Load
            | X86Opcode::X87Store
            | X86Opcode::X87StorePop
            | X86Opcode::X87IntegerLoad
            | X86Opcode::X87IntegerStore
            | X86Opcode::X87IntegerStorePop
            | X86Opcode::X87IntegerStoreTrunc
            | X86Opcode::X87Add
            | X86Opcode::X87Subtract
            | X86Opcode::X87SubtractReverse
            | X86Opcode::X87Multiply
            | X86Opcode::X87Divide
            | X86Opcode::X87DivideReverse
            | X86Opcode::X87AddPop
            | X86Opcode::X87SubtractPop
            | X86Opcode::X87SubtractReversePop
            | X86Opcode::X87MultiplyPop
            | X86Opcode::X87DividePop
            | X86Opcode::X87DivideReversePop
            | X86Opcode::X87Compare
            | X86Opcode::X87ComparePop
            | X86Opcode::X87ComparePop2
            | X86Opcode::X87StackLoad
            | X86Opcode::X87StackStorePop
            | X86Opcode::X87Exchange
            | X86Opcode::X87LoadZero
            | X86Opcode::X87LoadOne
            | X86Opcode::X87ChangeSign
            | X86Opcode::X87Absolute
            | X86Opcode::X87SquareRoot
            | X86Opcode::X87StoreStatusWord
            | X86Opcode::X87StoreControlWord
            | X86Opcode::X87LoadControlWord
            | X86Opcode::Wait
            | X86Opcode::Sahf
    )
}

fn x87_memory_format_is_legal(opcode: X86Opcode, format: X87MemoryFormat) -> bool {
    match opcode {
        X86Opcode::X87Load => matches!(
            format,
            X87MemoryFormat::Float32 | X87MemoryFormat::Float64 | X87MemoryFormat::Float80
        ),
        X86Opcode::X87Store => {
            matches!(format, X87MemoryFormat::Float32 | X87MemoryFormat::Float64)
        }
        X86Opcode::X87StorePop => matches!(
            format,
            X87MemoryFormat::Float32 | X87MemoryFormat::Float64 | X87MemoryFormat::Float80
        ),
        X86Opcode::X87IntegerLoad => matches!(
            format,
            X87MemoryFormat::Signed16 | X87MemoryFormat::Signed32 | X87MemoryFormat::Signed64
        ),
        X86Opcode::X87IntegerStore
        | X86Opcode::X87IntegerStorePop
        | X86Opcode::X87IntegerStoreTrunc => {
            matches!(
                format,
                X87MemoryFormat::Signed16 | X87MemoryFormat::Signed32
            ) || matches!(opcode, X86Opcode::X87IntegerStorePop)
                && format == X87MemoryFormat::Signed64
        }
        X86Opcode::X87Add
        | X86Opcode::X87Subtract
        | X86Opcode::X87SubtractReverse
        | X86Opcode::X87Multiply
        | X86Opcode::X87Divide
        | X86Opcode::X87DivideReverse => matches!(
            format,
            X87MemoryFormat::Float32
                | X87MemoryFormat::Float64
                | X87MemoryFormat::Signed16
                | X87MemoryFormat::Signed32
        ),
        X86Opcode::X87Compare | X86Opcode::X87ComparePop => {
            matches!(format, X87MemoryFormat::Float32 | X87MemoryFormat::Float64)
        }
        X86Opcode::X87StoreControlWord | X86Opcode::X87LoadControlWord => {
            format == X87MemoryFormat::Control16
        }
        _ => false,
    }
}

fn is_call_flags(flags: InstructionFlags) -> bool {
    flags.call
        && !flags.terminator
        && !flags.copy
        && (!flags.volatile || flags.may_load || flags.may_store)
}

fn divide_registers(width: u32) -> (X86Register, X86Register) {
    match width {
        2 => (X86Register::Ax, X86Register::Dx),
        4 => (X86Register::Eax, X86Register::Edx),
        _ => unreachable!("division verifier accepts only word and dword widths"),
    }
}

fn class_width(class: X86RegisterClass) -> Option<u32> {
    match class {
        X86RegisterClass::Byte => Some(u32::from(OperandSize::Byte.bits() / 8)),
        X86RegisterClass::Word | X86RegisterClass::Address16 => {
            Some(u32::from(OperandSize::Word.bits() / 8))
        }
        X86RegisterClass::Dword => Some(u32::from(OperandSize::Dword.bits() / 8)),
        X86RegisterClass::Segment | X86RegisterClass::X87 => None,
    }
}

fn register_width(register: X86Register) -> Option<u32> {
    match register {
        X86Register::Al
        | X86Register::Cl
        | X86Register::Dl
        | X86Register::Bl
        | X86Register::Ah
        | X86Register::Ch
        | X86Register::Dh
        | X86Register::Bh => Some(u32::from(OperandSize::Byte.bits() / 8)),
        X86Register::Ax
        | X86Register::Cx
        | X86Register::Dx
        | X86Register::Bx
        | X86Register::Sp
        | X86Register::Bp
        | X86Register::Si
        | X86Register::Di => Some(u32::from(OperandSize::Word.bits() / 8)),
        X86Register::Eax
        | X86Register::Ecx
        | X86Register::Edx
        | X86Register::Ebx
        | X86Register::Esp
        | X86Register::Ebp
        | X86Register::Esi
        | X86Register::Edi => Some(u32::from(OperandSize::Dword.bits() / 8)),
        X86Register::Es
        | X86Register::Cs
        | X86Register::Ss
        | X86Register::Ds
        | X86Register::Fs
        | X86Register::Gs
        | X86Register::St0
        | X86Register::St1
        | X86Register::St2
        | X86Register::St3
        | X86Register::St4
        | X86Register::St5
        | X86Register::St6
        | X86Register::St7 => None,
    }
}

fn class_name(class: X86RegisterClass) -> &'static str {
    match class {
        X86RegisterClass::Byte => "byte",
        X86RegisterClass::Word => "word",
        X86RegisterClass::Dword => "dword",
        X86RegisterClass::Address16 => "address16",
        X86RegisterClass::Segment => "segment",
        X86RegisterClass::X87 => "x87",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::old::codegen::machine::{
        FrameObjectKind, MachineAddressSpace, MachineBlockId, MachineCallingConvention,
        MachineFunctionId, MachineInstructionId, MachineLinkage, MachineSignature,
        MachineValueType, PhysicalRegister, VirtualRegister,
    };

    fn virtual_register(id: u32, role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(
                id,
            ))),
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn fixed_virtual(id: u32, role: OperandRole, register: X86Register) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(
                id,
            ))),
            role,
            constraint: Some(RegisterConstraint::Fixed(register.physical())),
            tied_to: None,
        }
    }

    fn physical(register: X86Register, role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Physical(register.physical())),
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn immediate(value: i64) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Immediate(value),
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        }
    }

    fn frame(index: u32) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::FrameIndex {
                index: FrameIndex::new(index),
                addend: 0,
            },
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        }
    }

    fn instruction(
        id: u32,
        opcode: X86Opcode,
        operands: Vec<MachineOperand>,
        flags: InstructionFlags,
    ) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(id),
            opcode: opcode.machine_opcode(),
            operands,
            flags,
        }
    }

    fn module(instructions: Vec<MachineInstruction>) -> MachineModule {
        MachineModule {
            data_objects: Vec::new(),
            functions: vec![MachineFunction {
                id: MachineFunctionId::new(0),
                name: "main".to_owned(),
                linkage: MachineLinkage::Internal,
                signature: MachineSignature {
                    result: Some(MachineValueType::Pointer {
                        bits: 16,
                        address_space: MachineAddressSpace::NearData,
                    }),
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: MachineCallingConvention::FarPascal,
                },
                entry: MachineBlockId::new(0),
                virtual_registers: vec![
                    VirtualRegister {
                        id: VirtualRegisterId::new(0),
                        class: X86RegisterClass::Byte.machine_class(),
                    },
                    VirtualRegister {
                        id: VirtualRegisterId::new(1),
                        class: X86RegisterClass::Word.machine_class(),
                    },
                    VirtualRegister {
                        id: VirtualRegisterId::new(2),
                        class: X86RegisterClass::Dword.machine_class(),
                    },
                    VirtualRegister {
                        id: VirtualRegisterId::new(3),
                        class: X86RegisterClass::Address16.machine_class(),
                    },
                    VirtualRegister {
                        id: VirtualRegisterId::new(4),
                        class: X86RegisterClass::X87.machine_class(),
                    },
                    VirtualRegister {
                        id: VirtualRegisterId::new(5),
                        class: X86RegisterClass::X87.machine_class(),
                    },
                    VirtualRegister {
                        id: VirtualRegisterId::new(6),
                        class: X86RegisterClass::X87.machine_class(),
                    },
                ],
                blocks: vec![MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions,
                    successors: Vec::new(),
                }],
                frame_objects: vec![FrameObject {
                    index: FrameIndex::new(0),
                    size: 2,
                    alignment: 2,
                    kind: FrameObjectKind::Local,
                }],
            }],
        }
    }

    #[test]
    fn rejects_divide_with_a_wrong_implicit_result_constraint() {
        let invalid = module(vec![instruction(
            0,
            X86Opcode::Idiv,
            vec![
                fixed_virtual(2, OperandRole::Use, X86Register::Edx),
                fixed_virtual(2, OperandRole::Use, X86Register::Eax),
                virtual_register(2, OperandRole::Use),
                fixed_virtual(2, OperandRole::Def, X86Register::Edx),
                fixed_virtual(2, OperandRole::Def, X86Register::Edx),
            ],
            InstructionFlags::NONE,
        )]);
        let messages = verify_machine(&invalid)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(
            messages
                .iter()
                .any(|message| message.contains("allocated physical Eax register"))
        );
    }

    #[test]
    fn accepts_allocated_physical_division_pair() {
        let allocated = module(vec![
            instruction(
                0,
                X86Opcode::CwdCdq,
                vec![
                    physical(X86Register::Edx, OperandRole::Def),
                    physical(X86Register::Eax, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
            instruction(
                1,
                X86Opcode::Idiv,
                vec![
                    physical(X86Register::Edx, OperandRole::Use),
                    physical(X86Register::Eax, OperandRole::Use),
                    physical(X86Register::Ecx, OperandRole::Use),
                    physical(X86Register::Eax, OperandRole::Def),
                    physical(X86Register::Edx, OperandRole::Def),
                ],
                InstructionFlags::NONE,
            ),
        ]);
        assert_eq!(verify_machine(&allocated), Ok(()));
    }

    #[test]
    fn rejects_malformed_anchor_and_unowned_spill_provenance() {
        // Python anchor clears physical effects; spillforward may trust only
        // the allocator's marked frame reloads, never an arbitrary load.
        let invalid = module(vec![
            instruction(
                0,
                X86Opcode::Nothing,
                vec![physical(X86Register::Ax, OperandRole::Def)],
                InstructionFlags {
                    anchor: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Load,
                vec![virtual_register(1, OperandRole::Def), frame(0)],
                InstructionFlags {
                    may_load: true,
                    spill_reload: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]);
        let messages = verify_machine(&invalid)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(
            messages
                .iter()
                .any(|message| message.contains("anchor must retain only unconstrained virtual"))
        );
        assert!(messages.iter().any(|message| {
            message.contains("spill reload provenance does not name a spill frame object")
        }));
    }

    #[test]
    fn verifies_zero_extend_word_to_dword_before_and_after_allocation() {
        let selected = module(vec![instruction(
            0,
            X86Opcode::ZeroExtendWordToDword,
            vec![
                virtual_register(2, OperandRole::Def),
                virtual_register(1, OperandRole::Use),
            ],
            InstructionFlags::NONE,
        )]);
        assert_eq!(verify_machine(&selected), Ok(()));

        let allocated = module(vec![instruction(
            0,
            X86Opcode::ZeroExtendWordToDword,
            vec![
                physical(X86Register::Eax, OperandRole::Def),
                physical(X86Register::Cx, OperandRole::Use),
            ],
            InstructionFlags::NONE,
        )]);
        assert_eq!(verify_machine(&allocated), Ok(()));
    }

    #[test]
    fn rejects_zero_extend_with_a_non_word_source() {
        let invalid = module(vec![instruction(
            0,
            X86Opcode::ZeroExtendWordToDword,
            vec![
                virtual_register(2, OperandRole::Def),
                virtual_register(2, OperandRole::Use),
            ],
            InstructionFlags::NONE,
        )]);
        let messages = verify_machine(&invalid)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(
            messages
                .iter()
                .any(|message| message.contains("must be a word x86 register"))
        );
    }

    #[test]
    fn accepts_frame_load_store_and_word_abi_pseudos() {
        let mut call_result = virtual_register(2, OperandRole::Def);
        call_result.constraint = Some(RegisterConstraint::Fixed(X86Register::Eax.physical()));
        let mut return_value = virtual_register(1, OperandRole::Use);
        return_value.constraint = Some(RegisterConstraint::Fixed(X86Register::Ax.physical()));
        let cleanup = MachineOperand {
            kind: MachineOperandKind::Immediate(4),
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        };
        let callee = MachineOperand {
            kind: MachineOperandKind::ExternalSymbol {
                name: "runtime".to_owned(),
                addend: 0,
            },
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        };
        let mut module = module(vec![
            instruction(
                0,
                X86Opcode::Load,
                vec![virtual_register(1, OperandRole::Def), frame(0)],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Store,
                vec![frame(0), virtual_register(1, OperandRole::Use)],
                InstructionFlags {
                    side_effects: true,
                    may_store: true,
                    volatile: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::Load,
                vec![virtual_register(3, OperandRole::Def), frame(0)],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                3,
                X86Opcode::MergeWords,
                vec![
                    virtual_register(2, OperandRole::Def),
                    virtual_register(1, OperandRole::Use),
                    virtual_register(1, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
            instruction(
                4,
                X86Opcode::LowWord,
                vec![
                    virtual_register(1, OperandRole::Def),
                    virtual_register(2, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
            instruction(
                5,
                X86Opcode::CallFar,
                vec![callee, call_result],
                InstructionFlags {
                    call: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]);
        module.functions[0].blocks[0].instructions.push(instruction(
            6,
            X86Opcode::ReturnFar,
            vec![return_value, cleanup],
            InstructionFlags {
                terminator: true,
                ..InstructionFlags::NONE
            },
        ));

        assert_eq!(verify_machine(&module), Ok(()));
    }

    #[test]
    fn accepts_fixed_call_uses_before_definitions_and_rejects_ambiguous_order() {
        let callee = || MachineOperand {
            kind: MachineOperandKind::ExternalSymbol {
                name: "runtime".to_owned(),
                addend: 0,
            },
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        };
        let call_flags = InstructionFlags {
            call: true,
            side_effects: true,
            ..InstructionFlags::NONE
        };
        let accepted = module(vec![instruction(
            0,
            X86Opcode::CallFar,
            vec![
                callee(),
                fixed_virtual(1, OperandRole::Use, X86Register::Ax),
                fixed_virtual(2, OperandRole::Def, X86Register::Eax),
            ],
            call_flags,
        )]);
        assert_eq!(verify_machine(&accepted), Ok(()));

        let interleaved = module(vec![instruction(
            0,
            X86Opcode::CallFar,
            vec![
                callee(),
                fixed_virtual(2, OperandRole::Def, X86Register::Eax),
                fixed_virtual(1, OperandRole::Use, X86Register::Ax),
            ],
            call_flags,
        )]);
        let messages = verify_machine(&interleaved)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(
            messages
                .iter()
                .any(|message| message.contains("fixed call uses must precede definitions"))
        );

        let aliasing_definitions = module(vec![instruction(
            0,
            X86Opcode::CallFar,
            vec![
                callee(),
                fixed_virtual(1, OperandRole::Def, X86Register::Ax),
                fixed_virtual(1, OperandRole::Def, X86Register::Ax),
            ],
            call_flags,
        )]);
        let messages = verify_machine(&aliasing_definitions)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(messages.iter().any(|message| {
            message.contains("fixed call definition aliases an earlier definition")
        }));
    }

    #[test]
    fn accepts_general_materialized_address16_displacements() {
        let accepted = module(vec![
            instruction(
                0,
                X86Opcode::Load,
                vec![
                    physical(X86Register::Ax, OperandRole::Def),
                    physical(X86Register::Bp, OperandRole::Use),
                    immediate(6),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Store,
                vec![
                    physical(X86Register::Bp, OperandRole::Use),
                    immediate(-24),
                    physical(X86Register::Eax, OperandRole::Use),
                ],
                InstructionFlags {
                    side_effects: true,
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::Load,
                vec![
                    physical(X86Register::Ax, OperandRole::Def),
                    physical(X86Register::Bx, OperandRole::Use),
                    immediate(4),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                3,
                X86Opcode::Lea,
                vec![
                    physical(X86Register::Bx, OperandRole::Def),
                    physical(X86Register::Bp, OperandRole::Use),
                    immediate(-24),
                ],
                InstructionFlags::NONE,
            ),
        ]);
        assert_eq!(verify_machine(&accepted), Ok(()));

        let invalid = module(vec![instruction(
            0,
            X86Opcode::Load,
            vec![
                physical(X86Register::Ax, OperandRole::Def),
                virtual_register(1, OperandRole::Use),
                immediate(6),
            ],
            InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
        )]);
        let messages = verify_machine(&invalid)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(messages.iter().any(|message| {
            message.contains(
                "materialized address base must be an unconstrained address16 register use",
            )
        }));
    }

    #[test]
    fn frame_store_reports_its_source_at_operand_two() {
        let invalid = module(vec![instruction(
            0,
            X86Opcode::Store,
            vec![
                physical(X86Register::Bp, OperandRole::Use),
                immediate(-24),
                immediate(7),
            ],
            InstructionFlags {
                side_effects: true,
                may_store: true,
                ..InstructionFlags::NONE
            },
        )]);

        let messages = verify_machine(&invalid)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();

        assert!(
            messages
                .iter()
                .any(|message| message.contains("operand 2 must be a register"))
        );
    }

    #[test]
    fn far_memory_accepts_segmented_forms_and_rejects_wrong_base_or_selector() {
        let accepted = module(vec![
            instruction(
                0,
                X86Opcode::Load,
                vec![
                    virtual_register(1, OperandRole::Def),
                    virtual_register(3, OperandRole::Use),
                    physical(X86Register::Es, OperandRole::Use),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Store,
                vec![
                    virtual_register(3, OperandRole::Use),
                    virtual_register(1, OperandRole::Use),
                    physical(X86Register::Es, OperandRole::Use),
                ],
                InstructionFlags {
                    side_effects: true,
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]);
        assert_eq!(verify_machine(&accepted), Ok(()));

        let invalid = module(vec![instruction(
            0,
            X86Opcode::Load,
            vec![
                virtual_register(1, OperandRole::Def),
                virtual_register(1, OperandRole::Use),
                physical(X86Register::Ds, OperandRole::Use),
            ],
            InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
        )]);
        let messages = verify_machine(&invalid)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(
            messages
                .iter()
                .any(|message| message.contains("must be a address16 x86 register"))
        );
        assert!(messages.iter().any(|message| {
            message.contains("segmented memory selector must be an unconstrained physical ES use")
        }));
    }

    #[test]
    fn rejects_invalid_pseudo_contracts_and_oversized_frame_access() {
        let mut bad_call_output = virtual_register(2, OperandRole::Use);
        bad_call_output.constraint = Some(RegisterConstraint::Fixed(PhysicalRegister::new(99)));
        let bad_lea_address = immediate(1);
        let module = module(vec![
            instruction(
                0,
                X86Opcode::Load,
                vec![virtual_register(2, OperandRole::Def), frame(0)],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Load,
                vec![
                    virtual_register(0, OperandRole::Def),
                    virtual_register(1, OperandRole::Use),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::Lea,
                vec![virtual_register(3, OperandRole::Def), bad_lea_address],
                InstructionFlags::NONE,
            ),
            instruction(
                3,
                X86Opcode::CallFar,
                vec![
                    MachineOperand {
                        kind: MachineOperandKind::ExternalSymbol {
                            name: "runtime".to_owned(),
                            addend: 0,
                        },
                        role: OperandRole::None,
                        constraint: None,
                        tied_to: None,
                    },
                    bad_call_output,
                ],
                InstructionFlags {
                    call: true,
                    copy: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                4,
                X86Opcode::ReturnFar,
                vec![MachineOperand {
                    kind: MachineOperandKind::Immediate(-1),
                    role: OperandRole::None,
                    constraint: None,
                    tied_to: None,
                }],
                InstructionFlags {
                    terminator: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]);

        let diagnostics = verify_machine(&module).unwrap_err();
        let messages = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert!(
            messages
                .iter()
                .any(|message| message.contains("access width 4 exceeds frame index 0 size 2"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("must have address16 x86 register class"))
        );
        assert!(messages.iter().any(|message| {
            message.contains("frame index with addend zero or global with role none")
        }));
        assert!(
            messages
                .iter()
                .any(|message| message.contains("must constrain a known x86 physical register"))
        );
        assert!(messages.iter().any(|message| {
            message.contains("far call must have call flags without terminator or copy")
        }));
        assert!(
            messages
                .iter()
                .any(|message| message.contains("cleanup immediate must be in 0..=65535"))
        );
    }

    #[test]
    fn accepts_selected_and_physical_x87_forms() {
        let load_flags = InstructionFlags {
            may_load: true,
            ..InstructionFlags::NONE
        };
        let store_flags = InstructionFlags {
            may_store: true,
            side_effects: true,
            ..InstructionFlags::NONE
        };
        let control_load_flags = InstructionFlags {
            may_load: true,
            side_effects: true,
            ..InstructionFlags::NONE
        };
        let mut valid = module(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    virtual_register(4, OperandRole::Def),
                    immediate(i64::from(X87MemoryFormat::Float32.raw())),
                    frame(0),
                ],
                load_flags,
            ),
            instruction(
                1,
                X86Opcode::X87Add,
                vec![
                    virtual_register(6, OperandRole::Def),
                    virtual_register(4, OperandRole::Use),
                    virtual_register(5, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
            instruction(
                2,
                X86Opcode::X87StorePop,
                vec![
                    physical(X86Register::St0, OperandRole::Use),
                    immediate(i64::from(X87MemoryFormat::Float64.raw())),
                    physical(X86Register::Bp, OperandRole::Use),
                    immediate(-8),
                ],
                store_flags,
            ),
            instruction(
                3,
                X86Opcode::X87Multiply,
                vec![
                    physical(X86Register::St0, OperandRole::UseDef),
                    physical(X86Register::St2, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
            instruction(
                4,
                X86Opcode::X87ComparePop2,
                vec![
                    physical(X86Register::St0, OperandRole::Use),
                    physical(X86Register::St1, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
            instruction(
                5,
                X86Opcode::X87StoreControlWord,
                vec![
                    immediate(i64::from(X87MemoryFormat::Control16.raw())),
                    frame(0),
                ],
                store_flags,
            ),
            instruction(
                6,
                X86Opcode::X87LoadControlWord,
                vec![
                    immediate(i64::from(X87MemoryFormat::Control16.raw())),
                    MachineOperand {
                        kind: MachineOperandKind::Global {
                            name: "control".to_owned(),
                            addend: 0,
                        },
                        role: OperandRole::None,
                        constraint: None,
                        tied_to: None,
                    },
                ],
                control_load_flags,
            ),
            instruction(
                7,
                X86Opcode::X87StoreStatusWord,
                vec![physical(X86Register::Ax, OperandRole::Def)],
                InstructionFlags::NONE,
            ),
            instruction(8, X86Opcode::Sahf, vec![], InstructionFlags::NONE),
            instruction(
                9,
                X86Opcode::X87StackStorePop,
                vec![
                    physical(X86Register::St2, OperandRole::Def),
                    physical(X86Register::St0, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
            instruction(
                10,
                X86Opcode::X87Add,
                vec![
                    physical(X86Register::St0, OperandRole::UseDef),
                    immediate(i64::from(X87MemoryFormat::Float32.raw())),
                    frame(0),
                ],
                load_flags,
            ),
        ]);
        valid.functions[0].frame_objects[0].size = 10;
        assert_eq!(verify_machine(&valid), Ok(()));
    }

    #[test]
    fn rejects_x87_formats_frames_roles_and_generic_stack_registers() {
        let load_flags = InstructionFlags {
            may_load: true,
            ..InstructionFlags::NONE
        };
        let invalid = module(vec![
            instruction(
                0,
                X86Opcode::X87Add,
                vec![
                    physical(X86Register::St0, OperandRole::Use),
                    immediate(i64::from(X87MemoryFormat::Float80.raw())),
                    frame(0),
                ],
                load_flags,
            ),
            instruction(
                1,
                X86Opcode::X87Divide,
                vec![
                    physical(X86Register::St0, OperandRole::Use),
                    immediate(i64::from(X87MemoryFormat::Signed64.raw())),
                    frame(0),
                ],
                load_flags,
            ),
            instruction(
                2,
                X86Opcode::X87Compare,
                vec![
                    physical(X86Register::St0, OperandRole::Use),
                    immediate(i64::from(X87MemoryFormat::Signed16.raw())),
                    frame(0),
                ],
                load_flags,
            ),
            instruction(
                3,
                X86Opcode::X87Load,
                vec![
                    virtual_register(4, OperandRole::Def),
                    immediate(i64::from(X87MemoryFormat::Float32.raw())),
                    frame(0),
                ],
                load_flags,
            ),
            instruction(
                4,
                X86Opcode::X87StackLoad,
                vec![
                    physical(X86Register::St1, OperandRole::Def),
                    physical(X86Register::St2, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
            instruction(
                5,
                X86Opcode::Load,
                vec![physical(X86Register::St0, OperandRole::Def), frame(0)],
                load_flags,
            ),
            instruction(
                6,
                X86Opcode::X87LoadControlWord,
                vec![
                    immediate(i64::from(X87MemoryFormat::Float32.raw())),
                    frame(0),
                ],
                load_flags,
            ),
            instruction(
                7,
                X86Opcode::X87StoreStatusWord,
                vec![physical(X86Register::Dx, OperandRole::Def)],
                InstructionFlags::NONE,
            ),
        ]);
        let messages = verify_machine(&invalid)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(
            messages
                .iter()
                .any(|message| message.contains("not legal for this x87 arithmetic opcode"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("access width 10 exceeds frame index 0 size 2"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("access width 4 exceeds frame index 0 size 2"))
        );
        assert!(messages.iter().any(|message| {
            message.contains("physical x87 stack registers are legal only on x87 target opcodes")
        }));
        assert!(messages.iter().any(|message| {
            message.contains("must be an unconstrained physical x87 stack register")
        }));
        assert!(
            messages
                .iter()
                .any(|message| message.contains("is not legal for this x87 opcode"))
        );
        assert!(messages.iter().any(|message| {
            message.contains("fnstsw requires an unconstrained physical AX definition")
        }));
    }

    #[test]
    fn rejects_selected_x87_allocation_metadata() {
        let mut destination = virtual_register(4, OperandRole::Def);
        destination.constraint = Some(RegisterConstraint::Fixed(X86Register::St0.physical()));
        destination.tied_to = Some(crate::old::codegen::machine::OperandIndex::new(1));
        let invalid = module(vec![instruction(
            0,
            X86Opcode::X87Add,
            vec![
                destination,
                virtual_register(5, OperandRole::Use),
                virtual_register(6, OperandRole::Use),
            ],
            InstructionFlags::NONE,
        )]);
        let messages = verify_machine(&invalid)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(messages.iter().any(|message| {
            message.contains("selected x87 virtual register must not retain allocation constraint or tie metadata")
        }));
    }

    #[test]
    fn accepts_exact_immediate_and_memory_push_forms() {
        let memory_flags = InstructionFlags {
            may_load: true,
            ..InstructionFlags::NONE
        };
        let mut valid = module(vec![
            instruction(
                0,
                X86Opcode::Push,
                vec![immediate(16), immediate(-128)],
                InstructionFlags::NONE,
            ),
            instruction(
                1,
                X86Opcode::Push,
                vec![immediate(32), immediate(0x1234_5678)],
                InstructionFlags::NONE,
            ),
            instruction(
                2,
                X86Opcode::Push,
                vec![immediate(32), frame(0)],
                memory_flags,
            ),
            instruction(
                3,
                X86Opcode::Push,
                vec![immediate(16), virtual_register(3, OperandRole::Use)],
                memory_flags,
            ),
            instruction(
                4,
                X86Opcode::Push,
                vec![
                    immediate(16),
                    MachineOperand {
                        kind: MachineOperandKind::Global {
                            name: "source".to_owned(),
                            addend: 0,
                        },
                        role: OperandRole::None,
                        constraint: None,
                        tied_to: None,
                    },
                ],
                memory_flags,
            ),
            instruction(
                5,
                X86Opcode::Push,
                vec![
                    immediate(16),
                    physical(X86Register::Bp, OperandRole::Use),
                    immediate(-4),
                ],
                memory_flags,
            ),
        ]);
        valid.functions[0].frame_objects[0].size = 4;
        assert_eq!(verify_machine(&valid), Ok(()));
    }

    #[test]
    fn rejects_malformed_immediate_and_memory_push_forms() {
        let memory_flags = InstructionFlags {
            may_load: true,
            ..InstructionFlags::NONE
        };
        let mut non_plain_value = immediate(5);
        non_plain_value.role = OperandRole::Use;
        let invalid = module(vec![
            instruction(
                0,
                X86Opcode::Push,
                vec![immediate(8), immediate(1)],
                InstructionFlags::NONE,
            ),
            instruction(
                1,
                X86Opcode::Push,
                vec![immediate(16), non_plain_value],
                InstructionFlags::NONE,
            ),
            instruction(
                2,
                X86Opcode::Push,
                vec![immediate(32), immediate(1)],
                memory_flags,
            ),
            instruction(
                3,
                X86Opcode::Push,
                vec![immediate(16), physical(X86Register::Cx, OperandRole::Use)],
                memory_flags,
            ),
        ]);
        let messages = verify_machine(&invalid)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(messages.iter().any(|message| {
            message.contains("push memory width must be a plain 16 or 32 immediate")
        }));
        assert!(
            messages
                .iter()
                .any(|message| message.contains("push immediate must be a plain immediate"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("immediate push must have no flags"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("must use a physical address16 x86 register"))
        );
    }

    #[test]
    fn accepts_exact_immediate_store_address_forms() {
        let store_flags = InstructionFlags {
            may_store: true,
            side_effects: true,
            ..InstructionFlags::NONE
        };
        let mut valid = module(vec![
            instruction(
                0,
                X86Opcode::Store,
                vec![frame(0), immediate(32), immediate(0x4040_0000)],
                store_flags,
            ),
            instruction(
                1,
                X86Opcode::Store,
                vec![
                    virtual_register(3, OperandRole::Use),
                    immediate(8),
                    immediate(0x7f),
                ],
                store_flags,
            ),
            instruction(
                2,
                X86Opcode::Store,
                vec![
                    MachineOperand {
                        kind: MachineOperandKind::Global {
                            name: "float_bits".to_owned(),
                            addend: 0,
                        },
                        role: OperandRole::None,
                        constraint: None,
                        tied_to: None,
                    },
                    immediate(16),
                    immediate(0x1234),
                ],
                store_flags,
            ),
            instruction(
                3,
                X86Opcode::Store,
                vec![
                    physical(X86Register::Bp, OperandRole::Use),
                    immediate(-4),
                    immediate(32),
                    immediate(0x4040_0000),
                ],
                store_flags,
            ),
        ]);
        valid.functions[0].frame_objects[0].size = 4;
        assert_eq!(verify_machine(&valid), Ok(()));
    }

    #[test]
    fn rejects_malformed_immediate_store_forms() {
        let store_flags = InstructionFlags {
            may_store: true,
            side_effects: true,
            ..InstructionFlags::NONE
        };
        let mut non_plain_width = immediate(32);
        non_plain_width.role = OperandRole::Use;
        let mut non_plain_value = immediate(0x4040_0000);
        non_plain_value.role = OperandRole::Use;
        let invalid = module(vec![
            instruction(
                0,
                X86Opcode::Store,
                vec![frame(0), immediate(64), immediate(0)],
                store_flags,
            ),
            instruction(
                1,
                X86Opcode::Store,
                vec![frame(0), non_plain_width, immediate(0)],
                store_flags,
            ),
            instruction(
                2,
                X86Opcode::Store,
                vec![frame(0), immediate(32), non_plain_value],
                store_flags,
            ),
            instruction(
                3,
                X86Opcode::Store,
                vec![frame(0), immediate(16), immediate(1)],
                InstructionFlags::NONE,
            ),
            instruction(
                4,
                X86Opcode::Store,
                vec![
                    physical(X86Register::Cx, OperandRole::Use),
                    immediate(8),
                    immediate(1),
                ],
                store_flags,
            ),
        ]);
        let messages = verify_machine(&invalid)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert!(messages.iter().any(|message| {
            message.contains("store immediate width must be a plain 8, 16, or 32 immediate")
        }));
        assert!(
            messages
                .iter()
                .any(|message| message.contains("stored immediate must be a plain immediate"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("immediate store must have store-only flags"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("must use a physical address16 x86 register"))
        );
    }
}
