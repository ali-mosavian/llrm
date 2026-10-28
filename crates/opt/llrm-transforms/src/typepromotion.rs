//! CodeGenPrepare's `TypePromotionTransaction`: a run of edits to a function
//! that can be taken back to any point it passed, or kept.
//! ([CodeGenPrepare.cpp#L3381](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L3381)).
//!
//! # How it works
//! Every edit is made on the function at once and recorded with what undoes
//! it: an operand set (`OperandSetter`), the operands of an instruction hidden
//! (`OperandsHider`), a `trunc`, `sext` or `zext` made (`TruncBuilder`,
//! `SExtBuilder`, `ZExtBuilder`), a type mutated (`TypeMutator`), the uses of a
//! value moved to another (`UsesReplacer`), an instruction moved
//! (`InstructionMoveBefore`) and an instruction removed (`InstructionRemover`:
//! its operands hidden, its uses given to the value that replaces it, and its
//! erasure left to `commit`). [`Transaction::restoration_point`] is a position
//! in the record; [`Transaction::rollback`] undoes the edits after it, last
//! first; [`Transaction::commit`] erases what was removed and forgets the
//! record.
//!
//! ```text
//!   let point = tpt.restoration_point();
//!   let wide  = tpt.create_sext(function, context, narrow, ty, before);
//!   tpt.rollback(function, point);        // the sext is gone again
//! ```
//!
//! MIRROR: the nine actions and the order they undo in; a removed instruction
//! stays in its block, unread, until `commit` (the pin keeps it unlinked: MIR
//! cannot unlink without erasing).
//!
//! # Example
//! The tests in `typepromotion_tests.rs`:
//! `test_a_created_extension_is_erased_by_a_rollback` is the sketch above.
//!
//! # Runs
//! Under `cgpaddr`'s `optimizeExt`, which makes a transaction for each
//! extension it considers.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::types::TypeId;

/// One edit and what undoes it.
enum Action {
    /// `OperandSetter`: operand `index` of `inst` was `was`.
    Operand { inst: InstId, index: usize, was: Operand },
    /// `OperandsHider`: the operands of `inst` were `was`.
    Operands { inst: InstId, was: Vec<Operand> },
    /// A builder: `inst` was made.
    Created { inst: InstId },
    /// `TypeMutator`: the type of `inst` was `was`.
    Type { inst: InstId, was: TypeId },
    /// `UsesReplacer`: these uses read `from` before.
    Replaced { from: ValueId, uses: Vec<(InstId, u32)> },
    /// `InstructionMoveBefore`: `inst` stood at `was`.
    Moved { inst: InstId, was: Position },
    /// `InstructionRemover`: `inst` is to be erased by `commit`.
    Removed { inst: InstId },
}

/// Where `inst` stands now, as a position that puts it back: after the
/// instruction before it, or before the one after it where it is first.
fn position_of(
    function: &Function,
    inst: InstId,
) -> Position {
    let block = function.parent(inst).expect("a placed instruction");
    let mut before = None;
    let mut found = false;
    for one in function.iter_block(block) {
        if found {
            return before.map_or(Position::Before(one), Position::After);
        }
        if one == inst {
            found = true;
        } else {
            before = Some(one);
        }
    }
    before.map_or(Position::End(block), Position::After)
}

/// A position in the record, to roll back to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestorationPoint(usize);

#[derive(Default)]
pub struct Transaction {
    actions: Vec<Action>,
}

impl Transaction {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getRestorationPoint`.
    pub fn restoration_point(&self) -> RestorationPoint {
        RestorationPoint(self.actions.len())
    }

    /// `setOperand`.
    pub fn set_operand(
        &mut self,
        function: &mut Function,
        inst: InstId,
        index: usize,
        value: Operand,
    ) {
        let was = function.instruction(inst).operands[index];
        function.set_operand(inst, index, value);
        self.actions.push(Action::Operand { inst, index, was });
    }

    /// `mutateType`.
    pub fn mutate_type(
        &mut self,
        function: &mut Function,
        inst: InstId,
        ty: TypeId,
    ) {
        let was = function.instruction(inst).ty;
        function.set_type(inst, ty);
        self.actions.push(Action::Type { inst, was });
    }

    /// `createTrunc`, `createSExt` and `createZExt`: `kind` of `operand` to
    /// `ty`, made just before `before`.
    pub fn create_cast(
        &mut self,
        function: &mut Function,
        kind: CastOp,
        operand: Operand,
        ty: TypeId,
        before: InstId,
    ) -> Operand {
        let inst = function.create_instruction(Opcode::Cast(kind), ty, vec![operand], Flags::default(), None);
        function.insert(inst, Position::Before(before)).expect("a position before an instruction");
        self.actions.push(Action::Created { inst });
        Operand::Value(function.instruction(inst).result.expect("a cast has a value"))
    }

    /// `replaceAllUsesWith`.
    pub fn replace_all_uses_with(
        &mut self,
        function: &mut Function,
        inst: InstId,
        with: Operand,
    ) {
        let Some(result) = function.instruction(inst).result else { return };
        let uses: Vec<(InstId, u32)> = function.users(result).iter().map(|one| (one.user, one.index)).collect();
        function.replace_all_uses_with(result, with);
        self.actions.push(Action::Replaced { from: result, uses });
    }

    /// `moveBefore`.
    pub fn move_before(
        &mut self,
        function: &mut Function,
        inst: InstId,
        before: InstId,
    ) {
        let was = position_of(function, inst);
        function.move_to(inst, Position::Before(before)).expect("a placed instruction moves");
        self.actions.push(Action::Moved { inst, was });
    }

    /// `eraseInstruction`: the operands of `inst` hidden, its uses given to
    /// `with` (if any), and it erased by `commit`.
    pub fn erase_instruction(
        &mut self,
        function: &mut Function,
        context: &Context,
        inst: InstId,
        with: Option<Operand>,
    ) {
        if let Some(with) = with {
            self.replace_all_uses_with(function, inst, with);
        }
        // Hidden: the instruction reads nothing, so what it read has one use
        // fewer, as the pin's `OperandsHider` makes it.
        let was = function.instruction(inst).operands.clone();
        let hidden: Vec<Operand> = was
            .iter()
            .map(|&operand| match function.operand_type(context, operand) {
                Some(ty) => Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison })),
                None => operand,
            })
            .collect();
        function.set_operands(inst, hidden);
        self.actions.push(Action::Operands { inst, was });
        self.actions.push(Action::Removed { inst });
    }

    /// `rollback`: the edits after `point`, last first.
    pub fn rollback(
        &mut self,
        function: &mut Function,
        point: RestorationPoint,
    ) {
        while self.actions.len() > point.0 {
            match self.actions.pop().expect("an action after the point") {
                Action::Operand { inst, index, was } => function.set_operand(inst, index, was),
                Action::Operands { inst, was } => function.set_operands(inst, was),
                Action::Created { inst } => function.erase(inst).expect("what nothing reads goes"),
                Action::Type { inst, was } => function.set_type(inst, was),
                Action::Replaced { from, uses } => {
                    for (user, index) in uses {
                        function.set_operand(user, index as usize, Operand::Value(from));
                    }
                }
                Action::Moved { inst, was } => function.move_to(inst, was).expect("a placed instruction moves"),
                Action::Removed { .. } => {}
            }
        }
    }

    /// `commit`: what was removed is erased, and the record forgotten.
    pub fn commit(
        &mut self,
        function: &mut Function,
    ) {
        let removed: Vec<InstId> = self
            .actions
            .iter()
            .filter_map(|action| if let Action::Removed { inst } = action { Some(*inst) } else { None })
            .collect();
        self.actions.clear();
        for inst in removed.into_iter().rev() {
            if !function.is_erased(inst) {
                function.erase(inst).expect("a removed instruction is read by nothing");
            }
        }
    }
}

#[cfg(test)]
#[path = "typepromotion_tests.rs"]
mod tests;
