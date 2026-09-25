//! What the emitter meant by an instruction, for the module analyses.
//!
//! The HIR keeps every value and place but not the construct that made
//! them: a DIM is a runtime call with positional operands, an element read
//! is loads from descriptor offsets. The emitter records the meaning where
//! it lowers the construct, keyed by resolved identity, so no analysis
//! re-resolves names or pattern-matches the frontend's own runtime calls.
//!
//! An untagged runtime call is the frontend's promise that it changes no
//! descriptor's shape -- rank, counts, lower bounds, origin -- and neither
//! reads nor writes `b$seg`. A descriptor's location -- data pointer and
//! selector -- may change at any call: allocating can compact the far heap,
//! which moves arrays and rewrites their selectors (rt/fhinit.asm
//! B$FHCompact).

#![allow(dead_code)] // The module analyses that read these are next.

use super::Operand;

pub(super) enum Tag {
    /// DIM of a dynamic array.
    Allocate(Shape),
    /// REDIM of an existing descriptor.
    Reallocate(Shape),
    /// ERASE.
    Release { descriptor: u32 },
    /// A load of one descriptor field.
    DescriptorField { descriptor: u32, field: Slot },
    /// An element's address or offset from `descriptor`: the descriptor's
    /// offset at +0Ah plus the scaled subscripts, or the checked runtime call
    /// that computes both. `origin` is that +0Ah value where the result is a
    /// word offset or near pointer built on it.
    ElementOffset { descriptor: u32, origin: Option<u32> },
    /// DEF SEG.
    SetSegment,
    /// A read of the DEF SEG segment, here or inside the runtime call.
    ReadSegment,
    /// A call to a declared SUB or FUNCTION, BASIC or not, defined in this
    /// module or not: what each argument hands the callee.
    Invoke { arguments: Vec<Passing> },
}

/// The descriptor a DIM or REDIM fills, and each dimension record's bounds
/// in record order: B$DDIM fills record 0 from the pair pushed last.
pub(super) struct Shape {
    pub descriptor: u32,
    pub records: Vec<(Operand, Operand)>,
    pub element: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Slot {
    /// The whole data pointer at +0.
    Data,
    /// The data selector at +2.
    Selector,
    /// The rank byte at +8.
    Rank,
    /// The data offset at +0Ah, already less every lower bound's elements.
    Origin,
    /// Dimension record `k`'s element count; `Shape::records` says which
    /// source dimension each record holds.
    Count(usize),
    /// Dimension record `k`'s lower bound.
    Lower(usize),
}

impl Slot {
    /// The field at byte `offset` of an array descriptor.
    pub fn at(offset: usize) -> Option<Self> {
        match offset {
            0 => Some(Self::Data),
            2 => Some(Self::Selector),
            8 => Some(Self::Rank),
            10 => Some(Self::Origin),
            14.. if (offset - 14).is_multiple_of(4) => Some(Self::Count((offset - 14) / 4)),
            16.. if (offset - 16).is_multiple_of(4) => Some(Self::Lower((offset - 16) / 4)),
            _ => None,
        }
    }
}

/// What one argument hands the callee.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Passing {
    /// BYVAL.
    Value,
    /// A copy only the callee sees.
    Temporary,
    /// A scalar variable's place, by reference.
    Variable(u32),
    /// An element of a static array or record field, by reference.
    Element(u32),
    /// Memory reached through this pointer value: a dynamic array element or
    /// a parameter this procedure itself received.
    Pointer(u32),
    /// A far element copied into a slot the callee sees and back after it
    /// returns: written after the call, never aliased during it.
    Copied(u32),
    /// A whole array: its descriptor pointer value.
    Array(u32),
}

#[cfg(test)]
mod tests {
    use super::super::{built, Instruction, Number, Options};
    use super::*;
    use crate::{parse, Dialect};

    /// Each procedure's name and its tagged instructions.
    fn tagged(source: &str) -> Vec<(String, Vec<Instruction>)> {
        tagged_checked(source, false)
    }

    fn tagged_checked(source: &str, checked: bool) -> Vec<(String, Vec<Instruction>)> {
        let module = parse(source, Dialect::VbDos).expect("parses");
        let compiler = built(&module, "T", Dialect::VbDos, "vbdos", &Options { checked_arrays: checked, ..Options::default() })
            .unwrap_or_else(|error| panic!("{}", error.message));
        compiler
            .functions
            .into_iter()
            .map(|function| {
                let tagged = function
                    .blocks
                    .into_iter()
                    .flat_map(|block| block.instructions)
                    .filter(|one| one.tag.is_some())
                    .collect();
                (function.name, tagged)
            })
            .collect()
    }

    fn procedure<'a>(all: &'a [(String, Vec<Instruction>)], name: &str) -> Vec<&'a Instruction> {
        all.iter().find(|(named, _)| named.eq_ignore_ascii_case(name)).expect(name).1.iter().collect()
    }

    fn integer(operand: &Operand) -> Option<i64> {
        match operand {
            Operand::Constant(_, Number::Integer(value)) => Some(*value),
            _ => None,
        }
    }

    #[test]
    fn test_a_dim_and_an_element_read_name_their_descriptor() {
        let all = tagged("DEFINT A-Z\nSUB t\nDIM a(320)\nx = a(5)\nEND SUB\n");
        let insns = procedure(&all, "t");
        let shapes: Vec<&Shape> = insns
            .iter()
            .filter_map(|one| match &one.tag {
                Some(Tag::Allocate(shape)) => Some(shape),
                _ => None,
            })
            .collect();
        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].records.iter().map(|(low, high)| (integer(low), integer(high))).collect::<Vec<_>>(), [
            (Some(0), Some(320))
        ]);
        let offset = insns
            .iter()
            .find(|one| matches!(one.tag, Some(Tag::ElementOffset { .. })))
            .expect("an element offset");
        let Operand::Value(origin) = offset.operands[0] else { panic!("origin is a value") };
        let origin = insns.iter().find(|one| one.results.contains(&origin)).expect("the origin is loaded");
        assert!(matches!(origin.tag, Some(Tag::DescriptorField { field: Slot::Origin, .. })));
        assert!(insns.iter().any(|one| matches!(one.tag, Some(Tag::DescriptorField { field: Slot::Selector, .. }))));
    }

    /// The constants each call to `callee` pushes, first pushed first.
    fn pushed(source: &str, row_major: bool, huge: bool, callee: &str) -> Vec<Vec<Option<i64>>> {
        let module = parse(source, Dialect::VbDos).expect("parses");
        let compiler = built(&module, "T", Dialect::VbDos, "vbdos", &Options { row_major, huge_arrays: huge, ..Options::default() })
            .unwrap_or_else(|error| panic!("{}", error.message));
        let mut found = Vec::new();
        for function in &compiler.functions {
            for call in &function.calls {
                let instruction = function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .find(|one| one.id == call.instruction)
                    .expect("the call");
                if instruction.callee.as_deref() == Some(callee) {
                    found.push(call.order.iter().map(|at| integer(&instruction.operands[*at])).collect());
                }
            }
        }
        found
    }

    const TWO_BY_THREE: &str = "DEFINT A-Z\nSUB t\nDIM a(1 TO 2, 3 TO 5)\nx = a(1, 4)\nEND SUB\n";

    /// DIM pushed the last dimension's bounds first, so B$DDIM filled record
    /// 0 from the first dimension and UBOUND(a, 1) answered 5. BC pushes
    /// 1, 2, 3, 5, and 3, 5, 1, 2 under /R.
    #[test]
    fn test_dim_pushes_its_bounds_as_bc_does() {
        let bounds = |row_major| pushed(TWO_BY_THREE, row_major, false, "B$DDIM")[0][..4].to_vec();
        assert_eq!(bounds(false), [Some(1), Some(2), Some(3), Some(5)]);
        assert_eq!(bounds(true), [Some(3), Some(5), Some(1), Some(2)]);
    }

    /// B$HARY pairs the subscript pushed last with record 0; the subscripts
    /// were pushed reversed, pairing a(1, 4)'s 1 with the second dimension's
    /// record. BC pushes 1, 4, and 4, 1 under /R.
    #[test]
    fn test_a_huge_element_pushes_its_subscripts_as_bc_does() {
        let subscripts = |row_major| pushed(TWO_BY_THREE, row_major, true, "B$HARY")[0][..3].to_vec();
        assert_eq!(subscripts(false), [Some(1), Some(4), Some(2)]);
        assert_eq!(subscripts(true), [Some(4), Some(1), Some(2)]);
    }

    #[test]
    fn test_a_redim_in_a_procedure_carries_its_lower_bound() {
        let all = tagged("DEFINT A-Z\nREDIM SHARED a(10)\nSUB s\nREDIM a(5 TO 10)\nEND SUB\n");
        let lower: Vec<Option<i64>> = procedure(&all, "s")
            .iter()
            .filter_map(|one| match &one.tag {
                Some(Tag::Reallocate(shape)) => Some(integer(&shape.records[0].0)),
                _ => None,
            })
            .collect();
        assert_eq!(lower, [Some(5)]);
    }

    /// REDIM passed 0100h, a far numeric array, whatever the element: B$RDIM
    /// then allocated a string array's descriptors as data. BC passes 8001h.
    #[test]
    fn test_a_redim_of_a_string_array_says_so() {
        let all = tagged("REDIM b$(4)\n");
        let flags: Vec<Option<i64>> = procedure(&all, "__main")
            .iter()
            .filter(|one| matches!(one.tag, Some(Tag::Reallocate(_))))
            .map(|one| integer(&one.operands[one.operands.len() - 2]))
            .collect();
        assert_eq!(flags, [Some(0x8001)]);
    }

    #[test]
    fn test_def_seg_and_poke_set_and_read_the_segment() {
        let all = tagged("DEF SEG = &HA000\nPOKE 0, 1\n");
        let insns = procedure(&all, "__main");
        assert_eq!(insns.iter().filter(|one| matches!(one.tag, Some(Tag::SetSegment))).count(), 1);
        assert_eq!(insns.iter().filter(|one| matches!(one.tag, Some(Tag::ReadSegment))).count(), 1);
    }

    #[test]
    fn test_a_call_says_what_each_argument_hands_the_callee() {
        let all = tagged(
            "DEFINT A-Z\nDECLARE SUB s (p, q(), r, t, u)\nDIM b(5)\nREDIM a(5)\nCALL s(x, a(), a(3), x + 1, b(2))\n\
             SUB s (p, q(), r, t, u)\nEND SUB\n",
        );
        let passing = procedure(&all, "__main")
            .iter()
            .find_map(|one| match &one.tag {
                Some(Tag::Invoke { arguments }) => Some(arguments.clone()),
                _ => None,
            })
            .expect("the call is tagged");
        assert!(matches!(
            passing.as_slice(),
            [Passing::Variable(_), Passing::Array(_), Passing::Copied(_), Passing::Temporary, Passing::Element(_)]
        ), "{passing:?}");
    }

    /// A local array's release at exit was untagged, breaking the promise
    /// that an untagged call changes no descriptor.
    #[test]
    fn test_a_local_array_is_released_at_exit() {
        let all = tagged("DEFINT A-Z\nSUB t\nDIM a(9)\nEND SUB\n");
        assert!(procedure(&all, "t").iter().any(|one| matches!(one.tag, Some(Tag::Release { .. }))));
    }

    /// A checked element's address came from an untagged runtime call.
    #[test]
    fn test_a_checked_element_names_its_descriptor() {
        let all = tagged_checked("DEFINT A-Z\nSUB t\nDIM a(9)\nx = a(5)\nEND SUB\n", true);
        assert!(procedure(&all, "t").iter().any(|one| matches!(one.tag, Some(Tag::ElementOffset { .. }))));
    }

    /// BLOAD and BSAVE read DEF SEG inside the runtime, untagged.
    #[test]
    fn test_bload_and_bsave_read_the_segment() {
        let all = tagged("BLOAD \"A\", 0\nBSAVE \"A\", 0, 4\n");
        let reads = procedure(&all, "__main").iter().filter(|one| matches!(one.tag, Some(Tag::ReadSegment))).count();
        assert_eq!(reads, 2);
    }
}
