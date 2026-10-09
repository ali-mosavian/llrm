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
    pub fn relative(
        &self,
        pointer: Operand,
    ) -> Option<(Operand, i64)> {
        self.stepped(pointer, true)
    }

    /// `relative` through steps that need not be `inbounds`: where the address is what matters, not
    /// what the object it stays in is.
    pub fn fixed(
        &self,
        pointer: Operand,
    ) -> Option<(Operand, i64)> {
        self.stepped(pointer, false)
    }

    fn stepped(
        &self,
        pointer: Operand,
        inbounds: bool,
    ) -> Option<(Operand, i64)> {
        let (mut value, mut offset) = (pointer, 0_i64);
        let mut seen = Visited::default();
        while seen.insert(value) {
            let Operand::Value(id) = value else { return Some((value, offset)) };
            let ValueDef::Instruction(inst) = self.function.value(id).def else { return Some((value, offset)) };
            let instruction = self.function.instruction(inst);
            let Opcode::GetElementPtr { source } = instruction.opcode else { return Some((value, offset)) };
            if inbounds && !instruction.flags.contains(Flags::INBOUNDS) {
                return Some((value, offset));
            }
            // A step has a few indices: no vector for them.
            let (mut small, mut large) = ([None; 4], Vec::new());
            let operands = &instruction.operands[1..];
            let indices: &[Option<i128>] = if operands.len() <= small.len() {
                for (slot, &one) in small.iter_mut().zip(operands) {
                    *slot = self.int(one);
                }
                &small[..operands.len()]
            } else {
                large.extend(operands.iter().map(|&one| self.int(one)));
                &large
            };
            let (constant, variable) = self.layout.collect_offset(&self.context.types, source, indices);
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

    fn int(
        &self,
        operand: Operand,
    ) -> Option<i128> {
        let Operand::Constant(id) = operand else { return None };
        let constant = self.context.get(id);
        match constant.kind {
            ConstantKind::Int(bits) => Some(signed(bits, self.context.types.int_bits(constant.ty)?)),
            _ => None,
        }
    }

    pub fn comparable(
        &self,
        one: Location,
        other: Location,
    ) -> Option<(i64, i64)> {
        let (left, right) = (self.relative(one.pointer)?, self.relative(other.pointer)?);
        if left.0 != right.0 {
            return None;
        }
        Some((left.1, right.1))
    }

    pub fn disjoint(
        &self,
        one: Location,
        other: Location,
    ) -> bool {
        let Some((left, right)) = self.comparable(one, other) else {
            return false;
        };
        if one.bytes == 0 || other.bytes == 0 {
            return false;
        }
        left + one.bytes as i64 <= right || right + other.bytes as i64 <= left
    }

    pub fn same_bytes(
        &self,
        one: Location,
        other: Location,
    ) -> bool {
        if one == other {
            return true;
        }
        self.comparable(one, other).is_some_and(|(left, right)| left == right) && one.bytes == other.bytes
    }
}

/// The operands a walk has passed, to refuse a cycle: a chain is a few steps, so the first ones are
/// compared in place, and only a longer one is hashed.
#[derive(Default)]
struct Visited {
    first: [Option<Operand>; 16],
    count: usize,
    more: Option<llrm_support::hash::HashSet<Operand>>,
}

impl Visited {
    /// Whether `value` is new.
    fn insert(
        &mut self,
        value: Operand,
    ) -> bool {
        if let Some(more) = &mut self.more {
            return more.insert(value);
        }
        if self.first[..self.count].contains(&Some(value)) {
            return false;
        }
        if self.count < self.first.len() {
            self.first[self.count] = Some(value);
            self.count += 1;
            return true;
        }
        let mut more: llrm_support::hash::HashSet<Operand> = self.first.iter().flatten().copied().collect();
        more.insert(value);
        self.more = Some(more);
        true
    }
}

pub fn offsets<'a>(
    context: &'a Context,
    layout: &'a DataLayout,
    function: &'a Function,
) -> Offsets<'a> {
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

    /// `@f`'s module, each `%name`'s relative offset and the checks on locations.
    fn with_facts(
        text: &str,
        check: impl Fn(&Offsets, &dyn Fn(&str, u64) -> Location, &dyn Fn(&str) -> Operand),
    ) {
        let module = parsed(text);
        let layout = DataLayout::parse(module.datalayout.as_deref().unwrap_or("")).unwrap();
        let f = function(&module, "f");
        let facts = offsets(&module.context, &layout, f);
        let at = |name: &str| Operand::Value(value(f, name));
        let location = |name: &str, bytes| Location { pointer: at(name), bytes };
        check(&facts, &location, &at);
    }

    /// A step of five or more indices, which are not kept in place, is read as one of four or fewer is.
    #[test]
    fn a_step_of_many_constant_indices_is_an_offset() {
        with_facts(
            "define void @f(ptr %p) {
b:
  %a = getelementptr inbounds [2 x [2 x [2 x [2 x i16]]]], ptr %p, i16 0, i16 1, i16 1, i16 1, i16 1
  %b = getelementptr inbounds [2 x [2 x i16]], ptr %p, i16 0, i16 1, i16 1
  ret void
}
",
            |facts, _, at| {
                assert_eq!(facts.relative(at("a")), Some((at("p"), 30)));
                assert_eq!(facts.relative(at("b")), Some((at("p"), 6)));
            },
        );
    }

    #[test]
    fn two_unrelated_pointers_are_never_disjoint_or_the_same() {
        with_facts(
            "define void @f(ptr %p, ptr %q) {
b:
  ret void
}
",
            |facts, location, _| {
                assert!(!facts.disjoint(location("p", 2), location("q", 2)));
                assert!(!facts.same_bytes(location("p", 2), location("q", 2)));
                assert!(facts.same_bytes(location("p", 2), location("p", 2)));
            },
        );
    }

    #[test]
    fn adjacent_and_overlapping_accesses_from_one_base() {
        with_facts(
            "define void @f(ptr %p) {
b:
  %back = getelementptr inbounds i8, ptr %p, i16 -2
  %up = getelementptr inbounds i8, ptr %p, i16 1
  ret void
}
",
            |facts, location, at| {
                assert_eq!(facts.relative(at("back")), Some((at("p"), -2)));
                assert!(facts.disjoint(location("back", 2), location("p", 2)), "touching, not overlapping");
                assert!(!facts.disjoint(location("back", 3), location("p", 2)));
                assert!(!facts.disjoint(location("p", 2), location("up", 1)));
                assert!(!facts.same_bytes(location("p", 2), location("up", 2)));
                assert!(!facts.disjoint(location("p", 0), location("up", 0)), "an empty access proves nothing");
            },
        );
    }

    #[test]
    fn same_address_with_different_widths_is_not_the_same_bytes() {
        with_facts(
            "define void @f(ptr %p) {
b:
  %q = getelementptr inbounds i8, ptr %p, i16 0
  ret void
}
",
            |facts, location, _| {
                assert!(facts.same_bytes(location("p", 2), location("q", 2)));
                assert!(!facts.same_bytes(location("p", 2), location("q", 1)));
                assert!(!facts.disjoint(location("p", 2), location("q", 1)));
            },
        );
    }

    #[test]
    fn offsets_from_a_global_are_relative_to_the_global() {
        let module = parsed(
            "@g = global [4 x i16] zeroinitializer

define void @f() {
b:
  %a = getelementptr inbounds i16, ptr @g, i16 1
  %b = getelementptr inbounds i16, ptr @g, i16 2
  ret void
}
",
        );
        let layout = DataLayout::parse(module.datalayout.as_deref().unwrap_or("")).unwrap();
        let f = function(&module, "f");
        let facts = offsets(&module.context, &layout, f);
        let at = |name: &str| Operand::Value(value(f, name));
        let (base, offset) = facts.relative(at("b")).unwrap();
        assert!(matches!(base, Operand::Constant(_)));
        assert_eq!(offset, 4);
        assert!(facts.disjoint(Location { pointer: at("a"), bytes: 2 }, Location { pointer: at("b"), bytes: 2 }));
    }

    #[test]
    fn far_pointer_steps_are_followed_in_their_own_address_space() {
        with_facts(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16\"

define void @f(ptr addrspace(1) %p) {
b:
  %a = getelementptr inbounds i16, ptr addrspace(1) %p, i16 3
  %b = getelementptr inbounds i8, ptr addrspace(1) %a, i16 2
  ret void
}
",
            |facts, location, at| {
                assert_eq!(facts.relative(at("b")), Some((at("p"), 8)));
                assert!(facts.disjoint(location("a", 2), location("b", 2)));
            },
        );
    }

    /// Every walk of a pointer's offsets hashed each operand it passed, with SipHash: 5% of compiling
    /// matmul at -O2 (#560). A short chain is compared in place, and only a longer one hashed; a repeat is
    /// refused either way.
    #[test]
    fn a_short_walk_is_not_hashed_and_a_repeat_is_refused_in_a_long_one() {
        use llrm_mir::module::ValueId;
        let mut seen = Visited::default();
        for at in 0..16 {
            assert!(seen.insert(Operand::Value(ValueId(at))));
        }
        assert!(seen.more.is_none(), "16 operands hashed");
        assert!(!seen.insert(Operand::Value(ValueId(3))), "a repeat in the first 16");
        for at in 16..100 {
            assert!(seen.insert(Operand::Value(ValueId(at))));
        }
        assert!(seen.more.is_some());
        assert!(!seen.insert(Operand::Value(ValueId(3))), "a repeat after the first 16");
        assert!(!seen.insert(Operand::Value(ValueId(60))), "a repeat in the hashed ones");
    }

    #[test]
    fn an_offset_past_the_index_width_proves_nothing() {
        with_facts(
            "target datalayout = \"e-p:16:16\"

define void @f(ptr %p) {
b:
  %a = getelementptr inbounds i8, ptr %p, i16 30000
  %b = getelementptr inbounds i8, ptr %a, i16 30000
  ret void
}
",
            |facts, location, at| {
                assert_eq!(facts.relative(at("a")), Some((at("p"), 30000)));
                assert_eq!(facts.relative(at("b")), None);
                assert!(!facts.disjoint(location("p", 2), location("b", 2)));
            },
        );
    }
}
