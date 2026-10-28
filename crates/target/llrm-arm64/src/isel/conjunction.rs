//! A select (or a branch that is no chain) on an `and`/`or` tree of integer
//! comparisons is one `cmp` and a `ccmp` for each comparison after it, then
//! one conditional instruction, not a `cset` for each (`emitConjunction`,
//! `AArch64ISelLowering.cpp`).
//!
//! # How it works
//! `ccmp rn, rm, #nzcv, cond` sets the flags of `rn - rm` where `cond` holds
//! and `nzcv` where it does not, so `a && b` is `cmp a; ccmp b, ..., eq`. An
//! `or` is `not (and (not a) (not b))`: a leaf negates by taking the inverse
//! condition, so an `or` needs one side that negates naturally and a subtree
//! that cannot is emitted first (`canEmitConjunction`). The leaves are the
//! one-use comparisons of the reader's block.
//!
//! # Example
//! ```text
//!   %a = icmp eq i8 %x, 72                  cmp  w8, #79
//!   %b = icmp eq i8 %x, 79                  mov  w9, #72
//!   %c = or i1 %a, %b               ==>     ccmp w8, w9, #4, ne
//!   %r = select i1 %c, i32 2, i32 1         csel ..., eq
//! ```
//!
//! MIRROR: `canEmitConjunction`, `emitConjunctionRec` and
//! `getNZCVToSatisfyCondCode`, with their depth limit of 6.
//!
//! DEVIATE 1: no floating-point leaves (`FCCMP`); a tree with one is a value.
//! Exit: an `fccmp` in the instruction set.
//!
//! DEVIATE 2: `PreferFirst` (a subtree whose `sub` already exists is emitted
//! first, to reuse it) is always false. Exit: the DAG's `doesNodeExist`.
//!
//! DEVIATE 3: the pin reaches this from `getAArch64Cmp`, `LowerBRCOND` and a
//! combine; here from the condition of a `select` and of a `br` that is no
//! chain (`chain` keeps a branch's conditions together by the pin's cost
//! policy), which is where the selector asks for flags (`condition`).

use llrm_mir::dense::IdSet;
use llrm_mir::{BinaryOp, Opcode, Operand, ValueId};

use super::Selector;
use super::int::condition_of;
use crate::mc::inst::Cond;

/// What `canEmitConjunction` says of a subtree.
#[derive(Clone, Copy)]
struct Shape {
    can_negate: bool,
    must_be_first: bool,
}

/// The `and`/`or` trees that are emitted as a chain of `ccmp`s: by root, and
/// every value in any (comparisons and joiners).
#[derive(Default)]
pub(crate) struct Conjunctions {
    roots: IdSet<ValueId>,
    members: IdSet<ValueId>,
}

impl Conjunctions {
    pub(super) fn contains(
        &self,
        value: ValueId,
    ) -> bool {
        self.members.contains(&value)
    }
}

/// `getNZCVToSatisfyCondCode`: the flags that make `cond` hold.
fn nzcv_to_satisfy(cond: Cond) -> u8 {
    const N: u8 = 8;
    const Z: u8 = 4;
    const C: u8 = 2;
    const V: u8 = 1;
    match cond {
        Cond::Eq | Cond::Le => Z,
        Cond::Hs | Cond::Hi => C,
        Cond::Mi | Cond::Lt => N,
        Cond::Vs => V,
        _ => 0,
    }
}

impl Selector<'_> {
    /// Finds the trees: the condition of each `select` and of each conditional
    /// `br` that is no chain, when it is a one-use `and`/`or` of
    /// comparisons all in the reader's block.
    pub(super) fn find_conjunctions(&mut self) {
        let mut found: Vec<(ValueId, Vec<ValueId>)> = Vec::new();
        for (_, inst) in self.function.walk() {
            let reader = self.function.instruction(inst);
            let condition = match reader.opcode {
                Opcode::Select => reader.operands[0],
                Opcode::Br if reader.operands.len() == 3 => reader.operands[0],
                _ => continue,
            };
            let Operand::Value(root) = condition else { continue };
            if self.chains.contains(root) || reader.operands[1..].contains(&condition) {
                continue;
            }
            let Some(root_inst) = super::value_def(self.function, root) else { continue };
            let block = self.function.parent(inst);
            if self.joined(root_inst).is_none() || self.function.parent(root_inst) != block {
                continue;
            }
            let mut members = Vec::new();
            if self.shape(root, block, false, 0, &mut members).is_some() {
                found.push((root, members));
            }
        }
        let mut conjunctions = Conjunctions::default();
        for (root, members) in found {
            conjunctions.roots.insert(root);
            for member in members {
                conjunctions.members.insert(member);
            }
        }
        self.conjunctions = conjunctions;
    }

    /// Whether the comparison `value` is read by one `and`/`or` of its block
    /// and otherwise only as the condition of selects and branches of it.
    /// The pin's `select (setcc a, b)` is a `select_cc a, b`
    /// before `canEmitConjunction` looks, so those readers do not count as
    /// uses; each selects its own compare.
    fn joined_once_and_otherwise_a_condition(
        &self,
        value: ValueId,
        block: Option<llrm_mir::BlockId>,
    ) -> bool {
        let condition = Operand::Value(value);
        let mut joiners = 0;
        let ok = self.function.users(value).into_iter().all(|user| {
            let reader = self.function.instruction(user.user);
            if self.function.parent(user.user) != block {
                return false;
            }
            match reader.opcode {
                Opcode::Br => reader.operands.len() == 3 && reader.operands[0] == condition,
                Opcode::Select if reader.operands[0] == condition && !reader.operands[1..].contains(&condition) => {
                    // A select on this value that is itself a
                    // logical `and`/`or` is a joiner.
                    if self.joined(user.user).is_some() && self.types().int_bits(reader.ty) == Some(1) {
                        joiners += 1;
                    }
                    true
                }
                _ if self.joined(user.user).is_some() => {
                    joiners += 1;
                    true
                }
                _ => false,
            }
        });
        ok && joiners == 1
    }

    /// `canEmitConjunction`: the shape of the subtree `value`, and its values
    /// in `members`.
    fn shape(
        &self,
        value: ValueId,
        block: Option<llrm_mir::BlockId>,
        will_negate: bool,
        depth: u32,
        members: &mut Vec<ValueId>,
    ) -> Option<Shape> {
        let inst = super::value_def(self.function, value)?;
        let is_compare = matches!(self.function.instruction(inst).opcode, Opcode::ICmp(_));
        if self.use_count(value) != 1 && !(is_compare && self.joined_once_and_otherwise_a_condition(value, block)) {
            return None;
        }
        if self.function.parent(inst) != block {
            return None;
        }
        let instruction = self.function.instruction(inst);
        if let Opcode::ICmp(_) = instruction.opcode {
            // A pointer compares as the 64-bit integer it is.
            let ty = self.operand_type(instruction.operands[0]).ok()?;
            let bits = match self.ty(ty) {
                llrm_mir::Type::Pointer(_) => 64,
                _ => self.types().int_bits(ty)?,
            };
            if bits > 64 {
                return None;
            }
            members.push(value);
            return Some(Shape { can_negate: true, must_be_first: false });
        }
        let (op, operands) = self.joined(inst)?;
        if self.types().int_bits(instruction.ty) != Some(1) || depth > 6 {
            return None;
        }
        let is_or = op == BinaryOp::Or;
        let (Operand::Value(first), Operand::Value(second)) = (operands[0], operands[1]) else { return None };
        let left = self.shape(first, block, is_or, depth + 1, members)?;
        let right = self.shape(second, block, is_or, depth + 1, members)?;
        if left.must_be_first && right.must_be_first {
            return None;
        }
        let shape = if is_or {
            // An `or` needs at least one side that negates naturally.
            if !left.can_negate && !right.can_negate {
                return None;
            }
            let can_negate = will_negate && left.can_negate && right.can_negate;
            Shape { can_negate, must_be_first: !can_negate }
        } else {
            Shape { can_negate: false, must_be_first: left.must_be_first || right.must_be_first }
        };
        members.push(value);
        Some(shape)
    }

    /// The flags for the tree rooted at `root`, and the condition that holds
    /// where it is true.
    pub(super) fn conjunction(
        &mut self,
        root: ValueId,
    ) -> super::Selected<Cond> {
        self.conjunction_rec(root, false, None)
    }

    /// `emitConjunctionRec`: `negate` has the subtree true where it is false,
    /// by its conditions alone; `after` is the condition the comparisons
    /// before this one are tested by, none for the first.
    fn conjunction_rec(
        &mut self,
        value: ValueId,
        negate: bool,
        after: Option<Cond>,
    ) -> super::Selected<Cond> {
        let inst = super::value_def(self.function, value).expect("a member is an instruction");
        let instruction = self.function.instruction(inst).clone();
        if let Opcode::ICmp(predicate) = instruction.opcode {
            let predicate = if negate { predicate.inverse() } else { predicate };
            let operands = self.compare_operands(predicate, instruction.operands[0], instruction.operands[1])?;
            let cond = condition_of(operands.predicate);
            match after {
                None => self.set_flags_of(&operands),
                Some(previous) => self.set_flags_if(&operands, nzcv_to_satisfy(cond.inverted()), previous),
            }
            return Ok(cond);
        }
        let block = self.function.parent(inst);
        let (op, operands) = self.joined(inst).expect("a member is a joiner");
        let is_or = op == BinaryOp::Or;
        let value_of = |operand: Operand| match operand {
            Operand::Value(value) => value,
            _ => unreachable!("a joiner of a conjunction reads values"),
        };
        let (mut left, mut right) = (value_of(operands[0]), value_of(operands[1]));
        let shape_of = |selector: &Self, value| selector.shape(value, block, is_or, 0, &mut Vec::new());
        let (mut shape_left, mut shape_right) =
            (shape_of(self, left).expect("valid tree"), shape_of(self, right).expect("valid tree"));
        // A subtree that must come first goes to the right, which is emitted
        // first.
        if shape_left.must_be_first {
            std::mem::swap(&mut left, &mut right);
            std::mem::swap(&mut shape_left, &mut shape_right);
        }
        let (negate_right, negate_after_right, negate_left, negate_after_all);
        if is_or {
            if !shape_left.can_negate {
                // The side that negates naturally goes to the left.
                std::mem::swap(&mut left, &mut right);
                negate_right = false;
                negate_after_right = true;
            } else {
                negate_right = shape_right.can_negate;
                negate_after_right = !shape_right.can_negate;
            }
            negate_left = true;
            negate_after_all = !negate;
        } else {
            (negate_right, negate_after_right, negate_left, negate_after_all) = (false, false, false, false);
        }
        let mut right_cond = self.conjunction_rec(right, negate_right, after)?;
        if negate_after_right {
            right_cond = right_cond.inverted();
        }
        let mut cond = self.conjunction_rec(left, negate_left, Some(right_cond))?;
        if negate_after_all {
            cond = cond.inverted();
        }
        Ok(cond)
    }

    /// Whether `value` is the root of a tree emitted as `ccmp`s.
    pub(super) fn is_conjunction_root(
        &self,
        value: ValueId,
    ) -> bool {
        self.conjunctions.roots.contains(&value)
    }
}
