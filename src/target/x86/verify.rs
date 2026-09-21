//! Target-owned legality checks for selected x86 Machine IR.
//!
//! The generic Machine IR verifier establishes representation invariants. This
//! layer validates only the x86 opcode contracts that are stable before
//! encoding, frame layout, or register allocation.

use std::collections::BTreeMap;

use crate::codegen::machine::{
    FrameIndex, FrameObject, InstructionFlags, MachineBlock, MachineFunction, MachineInstruction,
    MachineModule, MachineOperand, MachineOperandKind, MachineRegister, OperandRole, RegisterClass,
    RegisterConstraint, VirtualRegisterId,
};
use crate::support::diagnostic::{Diagnostic, Severity};

use super::{OperandSize, X86Opcode, X86Register, X86RegisterClass};

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

        match opcode {
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
            X86Opcode::SignExtendWordToDword => {
                self.verify_sign_extend_word_to_dword(function, block, instruction, classes)
            }
            X86Opcode::ShiftLeftDouble => {
                self.verify_shift_left_double(function, block, instruction)
            }
            X86Opcode::CallFar => self.verify_far_call(function, block, instruction, classes),
            X86Opcode::ReturnFar => self.verify_far_return(function, block, instruction, classes),
            X86Opcode::Push => self.verify_push(function, block, instruction),
            _ => {}
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
                    "load requires [register def, address] or [register def, bp use, displacement]",
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
            (base, Some(displacement)) => self.verify_materialized_frame_address(
                function,
                block,
                instruction,
                1,
                base,
                displacement,
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
        let (address, source) = match instruction.operands.as_slice() {
            [address, source] => ((address, None), source),
            [base, source, selector]
                if matches!(selector.kind, MachineOperandKind::Register(_)) =>
            {
                ((base, Some(selector)), source)
            }
            [base, displacement, source] => ((base, Some(displacement)), source),
            _ => {
                self.instruction_error(
                    function,
                    block,
                    instruction,
                    "store requires [address, register use] or [bp use, displacement, register use]",
                );
                return;
            }
        };
        let width = self.require_sized_register(
            function,
            block,
            instruction,
            1,
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
            (base, Some(displacement)) => self.verify_materialized_frame_address(
                function,
                block,
                instruction,
                0,
                base,
                displacement,
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
                if !matches!(address.role, OperandRole::None) || *addend != 0 {
                    self.operand_error(
                        function,
                        block,
                        instruction,
                        position,
                        "must be a frame index with role none and addend zero",
                    );
                }
                if let (Some(frame), Some(width)) = (frames.get(index), width) {
                    if width > frame.size {
                        self.operand_error(
                            function,
                            block,
                            instruction,
                            position,
                            format!(
                                "access width {width} exceeds frame index {} size {}",
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

    fn verify_materialized_frame_address(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        position: usize,
        base: &MachineOperand,
        displacement: &MachineOperand,
    ) {
        if !matches!(
            base,
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                role: OperandRole::Use,
                constraint: None,
                tied_to: None,
            } if *register == X86Register::Bp.physical()
        ) {
            self.operand_error(
                function,
                block,
                instruction,
                position,
                "materialized frame base must be an unconstrained physical BP use",
            );
        }
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
                "materialized frame displacement must be an unconstrained immediate",
            );
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
                    "lea requires [register def, frame index or global] or [register def, bp use, displacement]",
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
                MachineOperandKind::FrameIndex { addend, .. }
                    if matches!(address.role, OperandRole::None) && *addend == 0 => {}
                MachineOperandKind::Global { .. } if matches!(address.role, OperandRole::None) => {}
                _ => self.operand_error(
                    function,
                    block,
                    instruction,
                    1,
                    "must be a frame index with addend zero or global with role none",
                ),
            },
            (base, Some(displacement)) => self.verify_materialized_frame_address(
                function,
                block,
                instruction,
                1,
                base,
                displacement,
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

    fn verify_sign_extend_word_to_dword(
        &mut self,
        function: &MachineFunction,
        block: &MachineBlock,
        instruction: &MachineInstruction,
        classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    ) {
        let [destination, source] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "sign extension requires [dword register def, word register use]",
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
                "sign extension must have no flags",
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
    ) {
        let [source] = instruction.operands.as_slice() else {
            self.instruction_error(
                function,
                block,
                instruction,
                "push requires one register use",
            );
            return;
        };
        self.require_register_role(function, block, instruction, 0, source, OperandRole::Use);
        if instruction.flags != InstructionFlags::NONE {
            self.instruction_error(function, block, instruction, "push must have no flags");
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

fn is_call_flags(flags: InstructionFlags) -> bool {
    flags.call
        && !flags.terminator
        && !flags.copy
        && (!flags.volatile || flags.may_load || flags.may_store)
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
    use crate::codegen::machine::{
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
    fn accepts_only_the_materialized_bp_frame_tuple() {
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
                physical(X86Register::Bx, OperandRole::Use),
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
            message.contains("materialized frame base must be an unconstrained physical BP use")
        }));
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
}
