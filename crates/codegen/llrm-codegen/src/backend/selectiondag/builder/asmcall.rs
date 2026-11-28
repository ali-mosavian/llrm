//! A call of a Nib/QB `llrm.ia16.asm.*` block: the registers and symbols the
//! name states, as the target's inline-assembly node of
//! [`inline_asm`](super::super::inline_asm). The pin has no such node; the
//! shape is the emitter's contract.

use llrm_mir::module::{Instruction, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_target::inline_asm::AsmRegister;

use super::super::dag::SDValue;
use super::super::inline_asm::lower_asm;
use super::super::mvt::{EVT, MVT};
use super::LoweringNodes;
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
        // The x87 stack has no register to copy to: a value on it is an
        // operand of the node, which the emitter pushes before the block, the
        // deepest first (`GetRegistersForValue`'s fixed stack registers: the
        // stack's entries are the block's operands in place).
        let mut x87 = Vec::new();
        for (operand, register) in inputs.iter().zip(&block.inputs) {
            let value = self.get_value(*operand)?;
            let vt = self.dag.value_type(value);
            match self.arch.asm_register(register) {
                AsmRegister::Stack(depth) => x87.push((depth, self.to_x87(value)?)),
                AsmRegister::Register(id) => registers.push((self.sized_register(id, vt), value)),
            }
        }
        // Top of the stack first, the order the emitter reads them in.
        x87.sort_by_key(|(position, _)| *position);
        let x87: Vec<SDValue> = x87.into_iter().map(|(_, value)| value).collect();
        // An output on the x87 stack: its top, read in turn as a call's float
        // result is (the next is what the first read left under it).
        let top = self.arch.x87_stack_top();
        // The stack holds 80-bit values: an output of a narrower float is read
        // as `x86_fp80` and rounded to its type.
        let mut declared: Vec<EVT> = Vec::new();
        let output_register = |this: &Self, register: &str, vt| match this.arch.asm_register(register) {
            AsmRegister::Stack(_) => top.map(|one| (one, EVT::Simple(MVT::F80))),
            AsmRegister::Register(id) => Some((this.sized_register(id, vt), vt)),
        };
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
                declared.push(vt);
                outputs
                    .push(output_register(self, register, vt).ok_or(Refused("an x87 output on a target without one"))?);
            }
        } else if let Some(register) = block.outputs.first() {
            let vt = self.type_of(instruction.ty)?;
            declared.push(vt);
            outputs.push(output_register(self, register, vt).ok_or(Refused("an x87 output on a target without one"))?);
        }
        let root = self.root();
        let (chain, results) = lower_asm(
            &mut self.dag,
            LoweringNodes::need(self.target.dag.nodes.asm, "inline asm")?,
            root,
            name,
            &fixups,
            &registers,
            &x87,
            &outputs,
        );
        self.dag.set_root(chain);
        let results = results
            .into_iter()
            .zip(declared)
            .map(|(value, vt)| self.from_x87(value, vt))
            .collect::<Result<Vec<_>, _>>()?;
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

    /// A float operand for the x87 stack, which holds `x86_fp80`: a narrower
    /// value is extended first (on an SSE target, through memory).
    fn to_x87(
        &mut self,
        value: SDValue,
    ) -> Result<SDValue, Refused> {
        let f80 = EVT::Simple(MVT::F80);
        match self.dag.value_type(value) {
            EVT::Simple(MVT::F32 | MVT::F64) => self.cast_value(CastOp::FPExt, value, f80, Default::default()),
            _ => Ok(value),
        }
    }

    /// An output read from the x87 stack as `x86_fp80`, as the type the
    /// source declared.
    fn from_x87(
        &mut self,
        value: SDValue,
        declared: EVT,
    ) -> Result<SDValue, Refused> {
        match (self.dag.value_type(value), declared) {
            (EVT::Simple(MVT::F80), EVT::Simple(MVT::F32 | MVT::F64)) => {
                self.cast_value(CastOp::FPTrunc, value, declared, Default::default())
            }
            _ => Ok(value),
        }
    }
}
