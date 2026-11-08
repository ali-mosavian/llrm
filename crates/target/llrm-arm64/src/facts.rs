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
    /// Its results are written before its inputs are dead (a store-exclusive
    /// style sequence, a jump table).
    pub early_clobber: bool,
}

/// The facts of `op`.
pub fn of(op: &Op) -> Facts {
    Facts {
        reads_flags: matches!(
            op,
            Op::I(Inst::BranchCond { .. } | Inst::CondSelect { .. } | Inst::CondCompare { .. } | Inst::FpSelect { .. })
        ),
        sets_flags: writes_flags(op),
        may_load: matches!(
            op,
            Op::I(Inst::Load { .. } | Inst::LoadPair { .. } | Inst::Atomic { .. } | Inst::Prefetch { .. })
        ),
        may_store: matches!(
            op,
            Op::I(Inst::Store { .. } | Inst::StorePair { .. } | Inst::Atomic { .. })
        ),
        side_effects: matches!(
            op,
            Op::I(Inst::Svc { .. } | Inst::Dmb { .. } | Inst::Atomic { .. })
        ),
        early_clobber: matches!(op, Op::I(Inst::Atomic { .. }) | Op::JumpTable { .. }),
    }
}
