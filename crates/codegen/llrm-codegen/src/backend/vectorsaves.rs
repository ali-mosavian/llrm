//! The callee-saved vector registers a body writes get a 16-byte frame cell
//! each (X86FrameLowering::spillCalleeSavedRegisters: xmm is saved with
//! `movaps` into the callee-saved area, Win64's xmm6-15). Only the cells are
//! made here, after allocation and before the call area is reserved; the
//! entry and the exits save and reload them (`masm::lowered`).
//!
//! # Example
//! A Win64 function that writes xmm6 and xmm7 has `vector_saves` of
//! two 16-byte cells below its spills.

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
    body.insns().iter().filter_map(|one| one.what.as_ref()).any(|what| {
        what.dests.iter().any(|dest| matches!(dest, Loc::Reg(one) if one.register == register))
    })
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
        let align = std::mem::replace(&mut frame.align, 16);
        let blocks: Vec<_> =
            (0..used.len()).map(|at| frame.cell(("vector-save", at as i64), 16 + self.bias)).collect();
        frame.align = align;
        for (register, block) in used.into_iter().zip(blocks) {
            let mut cell = block.map_err(|refused| refused.0)?;
            if let Some(addr) = cell.addr.as_mut() {
                addr.disp += self.bias;
            }
            cell.width = 16;
            saves.push((register, cell));
        }
        Ok(LirBody { vector_saves: saves, ..body })
    }
}
