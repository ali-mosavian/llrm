//! A call of an `llrm.ia16.code.*` block: raw bytes laid down at the site,
//! with the frame places its name patches in as displacements from `BP`.
//! The node is `X86ISD::LLRM_CODE chain, symbol(name), frame index per place,
//! glue`; its `i32` result is read from the result registers after it. The
//! pin has no such node; the shape is the emitter's contract.

use llrm_mir::module::{Instruction, Operand, ValueDef};

use super::super::dag::SDValue;
use super::super::mvt::EVT;
use super::super::x86isd;
use super::{Builder, Refused};

impl Builder<'_> {
    pub(super) fn visit_code(
        &mut self,
        instruction: &Instruction,
        name: &str,
    ) -> Result<(), Refused> {
        let (_, places) = llrm_mir::intrinsics::code(name).ok_or(Refused("inline code that does not parse"))?;
        let arguments = &instruction.operands[..instruction.operands.len() - 1];
        if arguments.len() != places.len() {
            return Err(Refused("inline code with arguments pushed on the stack"));
        }
        let pointer = self.target.dag.pointer;
        let mut ops = vec![self.root(), self.dag.get_target_external_symbol(EVT::I32, name)];
        for argument in arguments {
            // a frame place is a stack object
            let Operand::Value(value) = *argument else {
                return Err(Refused("inline code naming other than a frame place"));
            };
            let ValueDef::Instruction(_) = self.function.value(value).def else {
                return Err(Refused("inline code naming other than a frame place"));
            };
            let index =
                *self.info.static_allocas.get(&value).ok_or(Refused("inline code naming other than a frame place"))?;
            ops.push(self.dag.get_target_frame_index(pointer, index));
        }
        let node = self.dag.get_node_multi(x86isd::LLRM_CODE, &[EVT::OTHER, EVT::GLUE], &ops);
        let mut chain = SDValue { node: node.node, res: 0 };
        let mut glue = SDValue { node: node.node, res: 1 };
        let vt = self.type_of(instruction.ty)?;
        let registers = self.full_arch()?.results(vt.size_in_bits() / 8);
        let (part_vt, parts) = self.parts_in(vt, registers.len())?;
        if registers.len() != parts {
            return Err(Refused("result in registers the value does not fill"));
        }
        let mut pieces = Vec::new();
        for register in registers {
            let register = self.sized_register(register, part_vt);
            let copy = self.dag.get_copy_from_phys_reg(chain, register, part_vt, glue);
            chain = SDValue { node: copy.node, res: 1 };
            glue = SDValue { node: copy.node, res: 2 };
            pieces.push(copy);
        }
        self.dag.set_root(chain);
        let value =
            if parts == 2 { self.dag.get_node(super::super::isd::ISD::BuildPair, vt, &pieces) } else { pieces[0] };
        self.set_value(instruction.result.ok_or(Refused("inline code without a result"))?, value);
        Ok(())
    }
}
