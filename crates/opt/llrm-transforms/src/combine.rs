//! The worklist LLVM's InstCombine runs its rules from: every instruction is
//! queued once, a rule is tried on what is popped, and what a change touched is
//! queued again, so a body converges inside the one pass and an instruction no
//! change reached is not visited twice.
//!
//! # How it works
//! A `Worklist` queues every instruction in layout order (the first
//! is tried first) and pops from the back, as InstCombine does. A rule that
//! rewrites an instruction is followed by what the edit log (`Change`) says it
//! touched: the instruction again, the definitions of its operands (one may now
//! have a single user, or none), and the users of its result (an operand
//! changed under them); an instruction the rule erased queues the definitions
//! of what it read. Nothing but the log tells the worklist what a rule did,
//! so a rule is written as it is for a walk and needs no hook.
//!
//! LLVM: `InstCombinerImpl::run` and `InstCombineWorklist`
//! (llvm/lib/Transforms/InstCombine/InstructionCombining.cpp:5780-5963,
//! InstCombineInternal.h:504-520).
//!
//! # See also
//! [`crate::algebraic`], whose per-instruction rules it drives.

use llrm_mir::dense::IdSet;
use llrm_mir::module::{Change, Function, InstId, Mark, Operand, ValueDef};

/// The instructions to try a rule on, each once until a change reaches it.
pub struct Worklist {
    stack: Vec<InstId>,
    queued: IdSet<InstId>,
}

impl Worklist {
    /// Every instruction of `function`, the first in layout order tried first.
    pub fn of(function: &Function) -> Self {
        let mut stack: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
        stack.reverse();
        let queued = stack.iter().copied().collect();
        Self { stack, queued }
    }

    /// What the changes since `mark` reached, queued: for edits that were not
    /// made by a rule of the worklist.
    pub fn reached_since(
        &mut self,
        function: &Function,
        mark: Mark,
    ) {
        let touched = match function.changes_since(mark) {
            Some(changes) => touched_by(function, changes),
            // The log was cut short: every instruction may have been reached.
            None => function.walk().map(|(_, inst)| inst).collect(),
        };
        for one in touched {
            if !function.is_erased(one) && self.queued.insert(one) {
                self.stack.push(one);
            }
        }
    }

    /// Tries `rule` on what is queued, and again on what each change touched,
    /// until none is left; whether any rule changed anything. `rule` says
    /// whether it changed the instruction it was given.
    pub fn drain(
        &mut self,
        function: &mut Function,
        mut rule: impl FnMut(&mut Function, InstId) -> bool,
    ) -> bool {
        let mut changed = false;
        while let Some(inst) = self.stack.pop() {
            self.queued.remove(&inst);
            if function.is_erased(inst) {
                continue;
            }
            let mark = function.mark();
            if rule(function, inst) {
                changed = true;
                self.reached_since(function, mark);
            }
        }
        changed
    }
}

/// What `changes` reached, as instructions: each one written, the definitions of
/// what it reads and the readers of what it makes.
fn touched_by(
    function: &Function,
    changes: &[Change],
) -> Vec<InstId> {
    let mut out = Vec::new();
    let defining = |operand: Operand, out: &mut Vec<InstId>| {
        if let Operand::Value(value) = operand
            && let ValueDef::Instruction(made) = function.value(value).def
        {
            out.push(made);
        }
    };
    let around = |inst: InstId, out: &mut Vec<InstId>| {
        out.push(inst);
        let instruction = function.instruction(inst);
        for &operand in &instruction.operands {
            defining(operand, out);
        }
        if let Some(result) = instruction.result {
            out.extend(function.users(result).iter().map(|one| one.user));
        }
    };
    for change in changes {
        match change {
            Change::Inserted { inst, .. } | Change::Moved { inst, .. } | Change::Rewritten(inst) => {
                around(*inst, &mut out)
            }
            Change::Cloned { to, .. } => around(*to, &mut out),
            Change::Substituted { to, .. } => {
                if let Operand::Value(value) = to {
                    out.extend(function.users(*value).iter().map(|one| one.user));
                    defining(Operand::Value(*value), &mut out);
                }
            }
            Change::Operand { was, .. } => defining(*was, &mut out),
            Change::Operands { was, .. } => was.iter().for_each(|&one| defining(one, &mut out)),
            Change::Erased { inst, .. } => {
                for &operand in &function.instruction(*inst).operands {
                    defining(operand, &mut out);
                }
            }
            Change::BlockCreated(_) | Change::BlockErased(_) | Change::Parameters => {}
        }
    }
    out
}
