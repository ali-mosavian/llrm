//! What an operation does besides naming registers: whether it reads or sets
//! the status flags, touches memory, has effects nothing may move across.
//! The allocator's view (`lir_view`) and the shared body (`lir_body`) both
//! state them, so they come from one place.

use crate::flagcse::writes_flags;
use crate::mc::inst::Inst;
use crate::mfunc::Op;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Facts {
    pub reads_flags: bool,
    pub sets_flags: bool,
    pub may_load: bool,
    pub may_store: bool,
    /// Nothing is moved across it (a system call, a barrier, an atomic).
    pub side_effects: bool,
    /// A call, or a tail call (LLVM's `isCall`): it may read and write any
    /// memory and has effects the instruction does not state.
    pub call: bool,
    /// Its results are written before its inputs are dead (a store-exclusive
    /// style sequence, a jump table).
    pub early_clobber: bool,
}

/// The facts of `op`.
pub fn of(op: &Op) -> Facts {
    Facts {
        reads_flags: matches!(
            op,
            Op::I(
                Inst::BranchCond { .. }
                    | Inst::CondSelect { .. }
                    | Inst::CondCompare { .. }
                    | Inst::FpSelect { .. }
                    | Inst::AddCarry { .. }
            )
        ),
        sets_flags: writes_flags(op),
        call: matches!(
            op,
            Op::Call { .. }
                | Op::Return { tail: Some(_), .. }
                | Op::I(Inst::Branch { link: true, .. } | Inst::BranchReg { link: true, .. })
        ),
        may_load: matches!(
            op,
            Op::I(Inst::Load { .. } | Inst::LoadPair { .. } | Inst::Atomic { .. } | Inst::Prefetch { .. })
        ),
        may_store: matches!(
            op,
            Op::I(Inst::Store { .. } | Inst::StorePair { .. } | Inst::Atomic { .. } | Inst::VectorStoreLane { .. })
        ),
        side_effects: matches!(
            op,
            Op::I(Inst::Svc { .. } | Inst::Dmb { .. } | Inst::Atomic { .. } | Inst::Brk { .. })
        ),
        early_clobber: matches!(op, Op::I(Inst::Atomic { .. }) | Op::JumpTable { .. }),
    }
}
