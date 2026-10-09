//! What an analysis reads of a function, declared once, and the manager's
//! filter of a change log against it: LLVM's `Result::invalidate`, which asks
//! each result whether a pass left it true, in the shape of a declaration.
//!
//! A change is invisible to the result when
//! - an instruction was erased that held no entry of it and has no effect the
//!   analysis models: an erased instruction has no users left (a user was
//!   rewritten or erased first, and logged), so nothing derived from its value
//!   can have changed; or
//! - an instruction was inserted, moved or rewritten that the analysis does not
//!   model, and neither does any instruction its value reaches.

use crate::context::Context;
use crate::dense::IdSet;
use crate::module::{Change, Function, InstId};
use crate::opcode::Opcode;

pub struct Depends {
    /// Whether the result has a part for this instruction, or reads it in
    /// working out another's: the kinds it models.
    pub models: fn(&Context, &Function, InstId) -> bool,
    /// Whether the result holds an entry keyed by this instruction, which
    /// erasing it removes.
    pub keyed: fn(&Context, &Function, InstId) -> bool,
    /// Whether a change to an instruction reaches the users of its value, past
    /// any user this says the value stops at.
    pub flows: Option<fn(&Opcode) -> bool>,
}

/// An instruction that does something besides make its value: what erasing it
/// unused still changes.
pub fn effectful(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Store { .. }
            | Opcode::Load { volatile: true, .. }
            | Opcode::Call(_)
            | Opcode::Invoke(_)
            | Opcode::LandingPad { .. }
            | Opcode::Resume
    ) || opcode.is_terminator()
}

impl Depends {
    /// Whether `changes` leave a result of this declaration as it was.
    pub fn unaffected(
        &self,
        changes: &[Change],
        context: &Context,
        function: &Function,
    ) -> bool {
        let mut work: Vec<InstId> = Vec::new();
        for change in changes {
            match *change {
                Change::BlockCreated(_) | Change::BlockErased(_) => return false,
                Change::Erased { inst, .. } => {
                    if (self.keyed)(context, function, inst)
                        || (effectful(&function.instruction(inst).opcode) && (self.models)(context, function, inst))
                    {
                        return false;
                    }
                }
                Change::Inserted { inst, .. }
                | Change::Moved { inst, .. }
                | Change::Rewritten(inst)
                | Change::Cloned { to: inst, .. } => work.push(inst),
            }
        }
        let mut seen: IdSet<InstId> = IdSet::new();
        for &inst in &work {
            seen.insert(inst);
        }
        while let Some(inst) = work.pop() {
            if (self.models)(context, function, inst) {
                return false;
            }
            let (Some(result), Some(stops)) = (function.instruction(inst).result, self.flows) else { continue };
            for user in function.users(result) {
                if stops(&function.instruction(user.user).opcode) {
                    continue;
                }
                if seen.insert(user.user) {
                    work.push(user.user);
                }
            }
        }
        true
    }
}
