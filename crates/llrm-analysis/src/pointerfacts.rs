//! Adapted from llrm-core's `analysis/pointerfacts.rs`: relative byte
//! offsets of whole MIR pointers; no pointer encoding is assumed.
//!
//! Only accesses already proven inside the same allocation may use relative
//! offsets to establish disjointness. Unrelated pointer values never suffice.
//! Facts are rebuilt from the current SSA, not attached to operands that a
//! later transformation could retarget.
//!
//! The old walk followed copies and `PtrOffset`s of four-byte pointers and
//! took the allocation from the raise's `MemRef.allocation`. Here it
//! follows `getelementptr inbounds` with constant indices: `inbounds` is the
//! MIR's own proof that the result stays in its base's allocation, so two
//! pointers reached from one base lie in one allocation. The old width and
//! segment checks were for split pointer halves, which MIR does not have.

use std::collections::HashSet;

use llrm_mir::context::signed;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::types::Type;
use llrm_mir::{ConstantKind, Context};

/// An access: `bytes` bytes at `pointer`, as the old `MemRef` was.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    pub pointer: Operand,
    pub bytes: u64,
}

#[derive(Clone, Copy)]
pub struct Offsets<'a> {
    pub context: &'a Context,
    pub layout: &'a DataLayout,
    pub function: &'a Function,
}

impl Offsets<'_> {
    /// The value `pointer` is a constant byte offset from.
    pub fn relative(&self, pointer: Operand) -> Option<(Operand, i64)> {
        let (mut value, mut offset) = (pointer, 0_i64);
        let mut seen = HashSet::new();
        while seen.insert(value) {
            let Operand::Value(id) = value else { return Some((value, offset)) };
            let ValueDef::Instruction(inst) = self.function.value(id).def else { return Some((value, offset)) };
            let instruction = self.function.instruction(inst);
            let Opcode::GetElementPtr { source } = instruction.opcode else { return Some((value, offset)) };
            if !instruction.flags.contains(Flags::INBOUNDS) {
                return Some((value, offset));
            }
            let indices: Vec<Option<i128>> = instruction.operands[1..].iter().map(|&one| self.int(one)).collect();
            let (constant, variable) = self.layout.collect_offset(&self.context.types, source, &indices);
            if !variable.is_empty() {
                return Some((value, offset));
            }
            let Type::Pointer(space) = *self.context.types.get(instruction.ty) else { return Some((value, offset)) };
            let bits = self.layout.pointer(space).index_bits.clamp(1, 64);
            let bound = 1_i128 << (bits - 1);
            let moved = i128::from(offset) + constant;
            if !(-bound..bound).contains(&moved) {
                return None;
            }
            offset = moved as i64;
            value = instruction.operands[0];
        }
        None
    }

    fn int(&self, operand: Operand) -> Option<i128> {
        let Operand::Constant(id) = operand else { return None };
        let constant = self.context.get(id);
        match constant.kind {
            ConstantKind::Int(bits) => Some(signed(bits, self.context.types.int_bits(constant.ty)?)),
            _ => None,
        }
    }

    pub fn comparable(&self, one: Location, other: Location) -> Option<(i64, i64)> {
        let (left, right) = (self.relative(one.pointer)?, self.relative(other.pointer)?);
        if left.0 != right.0 {
            return None;
        }
        Some((left.1, right.1))
    }

    pub fn disjoint(&self, one: Location, other: Location) -> bool {
        let Some((left, right)) = self.comparable(one, other) else {
            return false;
        };
        if one.bytes == 0 || other.bytes == 0 {
            return false;
        }
        left + one.bytes as i64 <= right || right + other.bytes as i64 <= left
    }

    pub fn same_bytes(&self, one: Location, other: Location) -> bool {
        if one == other {
            return true;
        }
        self.comparable(one, other).is_some_and(|(left, right)| left == right) && one.bytes == other.bytes
    }
}

pub fn offsets<'a>(context: &'a Context, layout: &'a DataLayout, function: &'a Function) -> Offsets<'a> {
    Offsets { context, layout, function }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{function, parsed, value};

    #[test]
    fn offsets_follow_constant_inbounds_steps_only() {
        let module = parsed(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16\"

define void @f(ptr %p, i16 %i) {
b:
  %a = getelementptr inbounds i8, ptr %p, i16 2
  %b = getelementptr inbounds i16, ptr %a, i16 1
  %c = getelementptr i8, ptr %p, i16 4
  %d = getelementptr inbounds i8, ptr %p, i16 %i
  %e = getelementptr inbounds [2 x i16], ptr %p, i16 0, i16 2
  ret void
}
",
        );
        let layout = DataLayout::parse(module.datalayout.as_deref().unwrap_or("")).unwrap();
        let f = function(&module, "f");
        let facts = offsets(&module.context, &layout, f);
        let at = |name: &str| Operand::Value(value(f, name));
        let location = |name: &str, bytes| Location { pointer: at(name), bytes };
        assert_eq!(facts.relative(at("b")), Some((at("p"), 4)));
        assert_eq!(facts.relative(at("c")), Some((at("c"), 0)));
        assert_eq!(facts.relative(at("d")), Some((at("d"), 0)));
        assert!(facts.disjoint(location("a", 2), location("b", 2)));
        assert!(!facts.disjoint(location("a", 4), location("b", 2)));
        assert!(!facts.disjoint(location("a", 2), location("c", 2)));
        assert!(facts.same_bytes(location("b", 2), location("e", 2)));
    }
}
