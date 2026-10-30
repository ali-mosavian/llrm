//! A call of a Nib/QB `llrm.ia16.asm.*` block: the registers and symbols the
//! name states, as the [`LLRM_ASM`](super::super::x86isd::LLRM_ASM) node of
//! [`inline_asm`](super::super::inline_asm). The pin has no such node; the
//! shape is the emitter's contract.

use llrm_mir::module::{Instruction, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Opcode};

use super::super::inline_asm::lower_asm;
use super::{Builder, Refused};

impl Builder<'_> {
    pub(super) fn visit_asm(
        &mut self,
        instruction: &Instruction,
        name: &str,
    ) -> Result<(), Refused> {
        let block = llrm_mir::intrinsics::asm(name).ok_or(Refused("inline assembly that does not parse"))?;
        let arguments = &instruction.operands[..instruction.operands.len() - 1];
        let (symbols, inputs) = arguments.split_at(block.fixups.len());
        let pointer = self.target.dag.pointer;
        let mut fixups = Vec::new();
        for (operand, fixup) in symbols.iter().zip(&block.fixups) {
            // the constant, seen through a cast of its address space
            let mut operand = *operand;
            while let Operand::Value(value) = operand
                && let ValueDef::Instruction(inst) = self.function.value(value).def
                && self.function.instruction(inst).opcode == Opcode::Cast(CastOp::AddrSpaceCast)
            {
                operand = self.function.instruction(inst).operands[0];
            }
            let Operand::Constant(id) = operand else { return Err(Refused("a symbol named by a computed address")) };
            let (global, offset) = crate::backend::globals::target(self.module, self.layout, id)
                .map_err(|_| Refused("a symbol the block names"))?;
            fixups.push(self.dag.get_target_global_address(pointer, global.0, offset + fixup.addend));
        }
        let mut registers = Vec::new();
        for (operand, register) in inputs.iter().zip(&block.inputs) {
            let value = self.get_value(*operand)?;
            let vt = self.dag.value_type(value);
            registers.push((self.sized_register(self.arch.register_named(register), vt), value));
        }
        let mut outputs = Vec::new();
        if block.outputs.len() > 1 {
            // several outputs come back as the fields of an aggregate
            let llrm_mir::types::Type::Struct { fields, .. } = self.module.context.types.get(instruction.ty).clone()
            else {
                return Err(Refused("inline assembly with several outputs and no aggregate result"));
            };
            if fields.len() != block.outputs.len() {
                return Err(Refused("inline assembly whose outputs are not the result's fields"));
            }
            for (register, field) in block.outputs.iter().zip(fields) {
                let vt = self.type_of(field)?;
                outputs.push((self.sized_register(self.arch.register_named(register), vt), vt));
            }
        } else if let Some(register) = block.outputs.first() {
            let vt = self.type_of(instruction.ty)?;
            outputs.push((self.sized_register(self.arch.register_named(register), vt), vt));
        }
        let root = self.root();
        let (chain, results) = lower_asm(&mut self.dag, root, name, &fixups, &registers, &outputs);
        self.dag.set_root(chain);
        if let Some(result) = instruction.result {
            if results.len() > 1 {
                // the aggregate is one node whose results are the fields
                let vts: Vec<_> = results.iter().map(|value| self.dag.value_type(*value)).collect();
                let merged = self.dag.get_node_multi(super::super::isd::ISD::MergeValues, &vts, &results);
                self.set_value(result, merged);
            } else if let Some(value) = results.first() {
                self.set_value(result, *value);
            }
        }
        Ok(())
    }
}
