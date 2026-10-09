//! A landing pad, which the runtime enters rather than an edge: the pad
//! starts by forgetting every register, and the machine CFG reaches it by a
//! branch from a root block that is never taken, as the old route's
//! temporary side-entry switch reached an ON ERROR handler.

use std::sync::Arc;

use iced_x86::Register;
use llrm_mir::Opcode;
use llrm_mir::module::{BlockId, InstId};

use super::{Selector, Unselected, insn, semantics};
use crate::abi::runtime::EVERY;
use crate::backend::callregs::{call_clobbered_high, call_clobbers};
use crate::backend::lower_int64::_helper;
use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBlock};

/// What starts a pad: `cld`, the one state it sets, as a call that
/// leaves no register as it was.
const LANDING: &str = "__LANDING";
const CLD: u8 = 0xFC;

impl Selector<'_, '_, '_> {
    /// The blocks a landingpad starts.
    pub(super) fn pads(&self) -> Vec<BlockId> {
        let function = self.function;
        function
            .layout()
            .iter()
            .copied()
            .filter(|&block| {
                function
                    .block(block)
                    .instructions()
                    .iter()
                    .any(|&one| matches!(function.instruction(one).opcode, Opcode::LandingPad { .. }))
            })
            .collect()
    }

    pub(super) fn landing_pad(
        &mut self,
        inst: InstId,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        if self.function.instruction(inst).result.is_some_and(|value| !self.function.users(value).is_empty()) {
            return Err(Unselected("a landing pad's value read after the backend prepared it".to_owned()));
        }
        if self.landing.replace(at).is_some() {
            return Err(Unselected("more than one landing pad".to_owned()));
        }
        let contract = _helper(LANDING, Default::default(), EVERY.clone());
        out.push(Arc::new(Insn {
            clobbers: call_clobbers(&contract, self.segments, &self.cpu.general),
            clobbers_high: call_clobbered_high(&contract, self.segments, &self.cpu.general),
            ..Insn::new(at, Some((at, at)), Some(semantics(Operation::Call, "call", vec![], vec![])), vec![], vec![])
        }));
        self.calls.insert(at, LANDING.to_owned());
        self.inline.insert(at, vec![CLD]);
        Ok(())
    }

    /// `blocks` entered at a new root `at` when there is a pad: a branch
    /// that is never taken to it, and on to `entry`. The root, and `blocks`.
    pub(super) fn rooted(
        &self,
        blocks: Vec<LirBlock>,
        entry: i64,
        pad: Option<i64>,
        at: i64,
    ) -> (Vec<LirBlock>, i64) {
        let Some(pad) = pad else { return (blocks, entry) };
        let sp = Loc::Reg(Reg { register: Register::SP, width: 2 });
        let zero = Loc::Imm(Imm { value: 0, width: 2, address: None });
        let never = [
            insn(at, semantics(Operation::Compare, "cmp", vec![], vec![sp, zero])),
            insn(at, Semantics { target: Some(pad), ..semantics(Operation::Branch, "je", vec![], vec![]) }),
        ];
        let root = LirBlock { succ: vec![pad, entry], ..LirBlock::new(at, never.to_vec()) };
        (std::iter::once(root).chain(blocks).collect(), at)
    }
}
