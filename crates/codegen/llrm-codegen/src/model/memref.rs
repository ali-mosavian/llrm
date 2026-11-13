//! What a load or store reaches (`MachineMemOperand`) and whether two accesses
//! may overlap (`MachineInstr::mayAlias`, MachineInstr.cpp#L1558 at
//! d1106deb71cc).
//!
//! No shared pass builds one: a target that knows an access's object puts it on
//! the `Insn`. An access without one may reach anything, as a load or store
//! with no memory operand does in the pin.
//!
//! DEVIATE MS6-ONE-MMO (owner arm64-phi, 2026-11-09): one operand per
//! instruction; the pin's list of up to `getMemOperandAACheckLimit` pairs is one
//! pair. Exit: a target with a two-operand access.

use llrm_analysis::aa::AliasResult;
use llrm_analysis::placealias::{Object, alias_of};

/// An access's object (a symbol by index into `LirBody::symbols`), the byte
/// offset in it and the bytes it touches, where each is known.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemRef {
    pub object: Option<Object<u32>>,
    pub offset: Option<i64>,
    pub bytes: Option<u64>,
    /// `MachineMemOperand::isStore`.
    pub store: bool,
}

impl MemRef {
    /// `MemOperandsHaveAlias` (MachineInstr.cpp#L1473) with `AA` answering by
    /// the objects alone (`placealias::alias_of`): the same object overlaps where
    /// the byte ranges do, an unknown width or offset overlaps; different
    /// objects are apart when the kinds or names are.
    #[must_use]
    pub fn may_alias(
        &self,
        other: &MemRef,
    ) -> bool {
        // The pin's `SameVal && BothMMONonScalable` branch.
        if self.object.is_some() && self.object == other.object {
            let (Some(a), Some(b), Some(width_a), Some(width_b)) = (self.offset, other.offset, self.bytes, other.bytes)
            else {
                return true;
            };
            let (low, high, low_width) = if a <= b { (a, b, width_a) } else { (b, a, width_b) };
            return low + low_width as i64 > high;
        }
        alias_of(None, self.object.as_ref(), self.bytes, other.object.as_ref(), other.bytes) != AliasResult::NoAlias
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(
        object: Object<u32>,
        offset: i64,
        bytes: u64,
        store: bool,
    ) -> MemRef {
        MemRef { object: Some(object), offset: Some(offset), bytes: Some(bytes), store }
    }

    const SLOT_1: Object<u32> = Object::Frame { id: 1, fixed: false };
    const SLOT_2: Object<u32> = Object::Frame { id: 2, fixed: false };

    #[test]
    fn different_frame_slots_do_not_overlap() {
        assert!(!at(SLOT_1, 0, 8, true).may_alias(&at(SLOT_2, 0, 8, false)));
    }

    #[test]
    fn a_slot_and_a_global_do_not_overlap() {
        assert!(!at(SLOT_1, 0, 8, true).may_alias(&at(Object::Global(3), 0, 8, false)));
    }

    #[test]
    fn the_same_object_overlaps_where_the_bytes_do() {
        assert!(at(SLOT_1, 0, 8, true).may_alias(&at(SLOT_1, 4, 8, false)));
        assert!(!at(SLOT_1, 0, 8, true).may_alias(&at(SLOT_1, 8, 8, false)));
        assert!(at(SLOT_1, 8, 4, true).may_alias(&at(SLOT_1, 8, 4, false)));
    }

    #[test]
    fn the_same_object_with_an_unknown_offset_or_width_may_overlap() {
        let unknown = MemRef { offset: None, ..at(SLOT_1, 0, 8, false) };
        assert!(at(SLOT_1, 0, 8, true).may_alias(&unknown));
        let wide = MemRef { bytes: None, ..at(SLOT_1, 0, 8, false) };
        assert!(at(SLOT_1, 16, 8, true).may_alias(&wide));
    }

    #[test]
    fn an_access_with_no_object_may_overlap_anything() {
        let nowhere = MemRef { object: None, ..at(SLOT_1, 0, 8, false) };
        assert!(at(SLOT_2, 0, 8, true).may_alias(&nowhere));
        assert!(nowhere.may_alias(&at(Object::Global(1), 0, 8, true)));
    }
}
