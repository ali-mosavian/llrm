//! Selects a landing pad, which the runtime enters rather than an edge reaches.
//!
//! # How it works
//! The runtime transfers control to a pad with no register intact, so the pad
//! begins with a pseudo-call, `__LANDING`, that clobbers every register and
//! carries the one byte `cld`. `landing_pad` emits it and records the pad's
//! position; a function has at most one pad, and its value must have no reader.
//! Selection treats the pad's blocks as roots of reachability and selects them
//! after the blocks the entry reaches.
//!
//! No edge leads to the pad, so `rooted` gives the function a new root block
//! that compares the stack pointer with zero and branches to the pad on equal.
//! The branch is never taken, and it makes the pad reachable in the machine
//! CFG:
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   r["<b>root</b><br/><small>cmp sp, 0<br/>je pad</small>"]:::mid
//!   e["<b>entry</b><br/><small>...</small>"]:::mid
//!   p["<b>pad: __LANDING</b><br/><small>(cld#59; every register<br/>forgotten)<br/>...</small>"]:::mid
//!   r --> e
//!   r -.->|"never taken"| p
//! ```
//!
//! *Landing-pad root block*
//!
//! # Example
//! A QB program with `ON ERROR GOTO`. After selection the root block holds the
//! never-taken branch, and the pad is a block that begins with the clobbering
//! pseudo-call:
//!
//! ```lir
//!     block 0x001d
//!       0x001d  cmp SP, 0x0  ; inserted
//!       0x001d  je   ; inserted
//!     block 0x0000
//!       ; ...
//!     block 0x0007
//!       0x0007  call   ; inserted; clobbers EAX,ECX,EDX,EBX,ES,DS,FS,GS
//! ```
//!
//! From `ISEL_DUMP=1` of a QB program (`ON ERROR GOTO handler`, an `OPEN`, a
//! `RESUME NEXT` handler) at `-O2`, stage `FarIndirectCalls`.
//!
//! # Runs
//! During `Selector::body` (`pads`, `landing_pad`) and at its end (`rooted`);
//! only in functions with a `landingpad`, which `ehprepare` has made enterable.
//!
//! # Needs
//! [`crate::backend::ehprepare`]; the pad's registers contract is
//! `abi::runtime::EVERY`.

use std::sync::Arc;

use llrm_mir::Opcode;
use llrm_mir::module::{BlockId, InstId};

use super::{Selector, Unselected, insn, semantics};
use crate::abi::runtime::EVERY;
use crate::backend::callregs::{call_clobbered_high, call_clobbers};
use crate::backend::lower_int64::_helper;
use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Block, Insn, LirBlock};

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
                    .iter_block(block)
                    .any(|one| matches!(function.instruction(one).opcode, Opcode::LandingPad { .. }))
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
        blocks: Vec<Block>,
        entry: i64,
        pad: Option<i64>,
        at: i64,
    ) -> (Vec<Block>, i64) {
        let Some(pad) = pad else { return (blocks, entry) };
        let sp = Loc::Reg(Reg { register: crate::model::ir::STACK, width: 2 });
        let zero = Loc::Imm(Imm { value: 0, width: 2, address: None });
        let never = [
            insn(at, semantics(Operation::Compare, "cmp", vec![], vec![sp, zero])),
            insn(at, Semantics { target: Some(pad), ..semantics(Operation::Branch, "je", vec![], vec![]) }),
        ];
        let root = LirBlock { succ: vec![pad, entry].into(), ..LirBlock::new(at, never.to_vec()) };
        (std::iter::once(Block::from(root)).chain(blocks).collect(), at)
    }
}
