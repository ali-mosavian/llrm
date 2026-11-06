//! The callee-saved vector registers a body writes get a 16-byte frame cell
//! each (X86FrameLowering::spillCalleeSavedRegisters: xmm is saved with
//! `movaps` into the callee-saved area, Win64's xmm6-15). Only the cells are
//! made here, after allocation and before the call area is reserved; the
//! entry and the exits save and reload them (`masm::lowered`).
//!
//! # Example
//! A Win64 function that writes xmm6 and xmm7 has `vector_saves` of
//! two adjacent 16-byte cells below its spills.

use std::cell::RefCell;
use std::rc::Rc;

use llrm_lir::registers::RegId;

use crate::backend::frame::Frame;
use crate::model::ir::Loc;
use crate::model::lir::LirBody;
use crate::model::passes::LIRTransform;

pub struct VectorSaves {
    pub frame: Option<Rc<RefCell<Frame>>>,
    /// The convention's callee-saved vector registers.
    pub saved: Vec<RegId>,
    /// The return address: the frame's base is one slot off the 16-byte
    /// boundary `movaps` needs, so a cell lies a slot above its 16-aligned
    /// block.
    pub bias: i64,
}

fn written(
    body: &LirBody,
    register: RegId,
) -> bool {
    body.insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .any(|what| what.dests.iter().any(|dest| matches!(dest, Loc::Reg(one) if one.register == register)))
}

impl LIRTransform for VectorSaves {
    fn class_name(&self) -> &'static str {
        "VectorSaves"
    }

    fn name(&self) -> &str {
        "vectorsaves"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        let Some(frame) = &self.frame else { return Ok(body) };
        let used: Vec<RegId> = self.saved.iter().copied().filter(|&register| written(&body, register)).collect();
        if used.is_empty() {
            return Ok(body);
        }
        let mut frame = frame.borrow_mut();
        let mut saves = Vec::new();
        // One block for all of them, the cells side by side, the lowest
        // register at the lowest address: the pin's callee-saved vector
        // objects (`assignCalleeSavedSpillSlots`) are 16 bytes each and
        // adjacent, not a block of their own to a register.
        let align = std::mem::replace(&mut frame.align, 16);
        let block = frame.cell(("vector-save", 0), 16 * used.len() as i64 + self.bias);
        frame.align = align;
        let block = block.map_err(|refused| refused.0)?;
        for (at, register) in used.into_iter().enumerate() {
            let mut cell = block.clone();
            if let Some(addr) = cell.addr.as_mut() {
                addr.disp += self.bias + 16 * at as i64;
            }
            cell.width = 16;
            saves.push((register, cell));
        }
        Ok(LirBody { vector_saves: saves, ..body })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use indexmap::IndexMap;

    use super::*;
    use crate::model::ir::{Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock};

    fn writes(
        at: i64,
        register: RegId,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("movaps".into()),
            dests: vec![Loc::Reg(Reg { register, width: 16 })],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
    }

    /// Each saved xmm took a 32-byte stride of its own (24-byte blocks on a
    /// 16-byte alignment), in descending order: `xmm6, xmm7, xmm8` sat at
    /// 0x70, 0x50, 0x30 of an 0x88 frame where the pin has them at 0x20, 0x30,
    /// 0x40 of an 0x58 one.
    #[test]
    fn the_saved_vector_registers_are_adjacent_cells_lowest_register_lowest() {
        let saved = [iced_x86::Register::XMM6, iced_x86::Register::XMM7, iced_x86::Register::XMM8].map(RegId::new);
        let insns = saved.iter().enumerate().map(|(at, register)| writes(at as i64, *register)).collect();
        let body = LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        let frame = Rc::new(RefCell::new(Frame::new(0)));
        let mut pass = VectorSaves { frame: Some(frame), saved: saved.to_vec(), bias: 8 };
        let made = pass.transform(body).expect("saves");
        let disps: Vec<i64> = made.vector_saves.iter().map(|(_, cell)| cell.addr.expect("a frame cell").disp).collect();
        assert_eq!(disps[1] - disps[0], 16);
        assert_eq!(disps[2] - disps[1], 16);
    }
}
