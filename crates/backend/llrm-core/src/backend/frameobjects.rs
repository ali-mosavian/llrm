//! Where each frame object goes and how big the frame is: LLVM's
//! `PEIImpl::calculateFrameObjectOffsets`
//! ([PrologEpilogInserter.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp#L856)).
//!
//! # How it works
//! The objects are numbered; fixed ones (arguments, the return address) have
//! offsets already and sit at the negative numbers of the pin, here the
//! first `fixed` entries. For a stack that grows down the walk is:
//!
//! 1. `Offset` starts at the local area (the return address) and rises to the
//!    far end of every fixed object (`FixedOff`).
//! 2. The callee-saved objects, last to first, then every other live object in
//!    index order, each `AdjustStackOffset`: add its size, align to its
//!    alignment, take the negative as its offset.
//! 3. A call frame reserved in the frame adds the largest outgoing size; the
//!    total aligns to the stack alignment (the transient one where the function
//!    neither adjusts the stack nor has a variable-sized object), raised to the
//!    largest object alignment.
//!
//! The stack size is `Offset` less the local area.
//!
//! ```text
//!   objects (size/align):  a 4/4, b 1/1, c 8/8     local area 4
//!   a: 4+4 = 8  -> [-8]    b: 8+1 = 9 -> [-9]    c: 9+8 = 17 -> 24 -> [-24]
//!   no call: transient alignment 4, size 24 - 4 = 20
//!   a call with 12 outgoing bytes: 24 + 12 = 36 -> 48 (16), size 44
//! ```
//!
//! DEVIATE FO-1 (exit: frame indices, owner w-6, 2026-11-20): nothing calls
//! this yet. `frame::Frame` hands out a slot when the spiller asks and the
//! displacement goes into the instruction at once; the pin names a frame index
//! and `replaceFrameIndices` fills the offset after this runs.
//!
//! DEVIATE FO-2 (exit: with FO-1): the stack protector, the local stack
//! allocation block, scavenging slots, `orderFrameObjects`,
//! `computeFreeStackSlots` and `StackSymbolOrdering` are not translated; i486
//! enables none (`enableStackSlotScavenging` is false, no register scavenger,
//! no `-fstack-protector` frontend).
//!
//! # Example
//! The unit test `objects_are_laid_below_the_local_area_by_size_and_alignment`
//! is the sketch above.
//!
//! # Used by
//! Nothing yet (FO-1).

/// One `MachineFrameInfo` object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Object {
    pub size: i64,
    pub align: i64,
    /// The offset from the frame's top of an object that has one already; the
    /// result for the others.
    pub offset: i64,
    pub callee_saved: bool,
    pub dead: bool,
}

/// What the layout depends on besides the objects.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// `getOffsetOfLocalArea`, negated for a stack that grows down: 4 on
    /// x86-32.
    pub local_area: i64,
    /// The largest alignment of any object (`MFI.getMaxAlign`).
    pub max_align: i64,
    /// `getStackAlign`, and `getTransientStackAlign` for a function that does
    /// not adjust the stack.
    pub stack_align: i64,
    pub transient_align: i64,
    pub adjusts_stack: bool,
    pub has_var_sized: bool,
    pub realigned: bool,
    pub reserved_call_frame: bool,
    pub max_call_frame_size: i64,
}

fn align_to(
    value: i64,
    align: i64,
) -> i64 {
    (value + align - 1) / align * align
}

/// `AdjustStackOffset` for a stack that grows down.
fn adjust(
    object: &mut Object,
    offset: &mut i64,
    max_align: &mut i64,
) {
    *offset += object.size;
    *max_align = (*max_align).max(object.align);
    *offset = align_to(*offset, object.align);
    object.offset = -*offset;
}

/// Lay `objects` out, the first `fixed` of them already placed; the stack
/// size, in bytes below the local area.
pub fn calculate_frame_object_offsets(
    objects: &mut [Object],
    fixed: usize,
    frame: Frame,
) -> i64 {
    let mut offset = frame.local_area;
    for one in &objects[..fixed] {
        offset = offset.max(-one.offset);
    }
    let mut max_align = frame.max_align;
    for one in objects.iter_mut().rev().filter(|one| one.callee_saved) {
        adjust(one, &mut offset, &mut max_align);
    }
    for one in objects[fixed..].iter_mut().filter(|one| !one.callee_saved && !one.dead) {
        adjust(one, &mut offset, &mut max_align);
    }
    if frame.adjusts_stack && frame.reserved_call_frame {
        offset += frame.max_call_frame_size;
    }
    let stack_align = if frame.adjusts_stack || frame.has_var_sized || (frame.realigned && !objects.is_empty()) {
        frame.stack_align
    } else {
        frame.transient_align
    };
    offset = align_to(offset, stack_align.max(max_align));
    offset - frame.local_area
}

/// `LLRM_FRAME_ORACLE`: the frame area of `name` as ours lays it out (the bytes
/// below BP, `reserve`) and as `calculate_frame_object_offsets` would lay the
/// same objects: each selector extent and each spill slot, in the order the
/// selector made them then the spill slots by home, aligned to their natural
/// alignment up to the frame's. No callee-saved object, call frame or stack
/// alignment: the objects' area only.
pub fn oracle_line(
    name: &str,
    frame: &crate::backend::frame::Frame,
    reserve: i64,
) -> String {
    let natural = |size: i64| {
        let mut align = 1;
        while align < size.min(frame.align) {
            align *= 2;
        }
        align
    };
    let mut objects: Vec<Object> = Vec::new();
    for &(_, size) in &frame.extents {
        objects.push(Object { size, align: natural(size), offset: 0, callee_saved: false, dead: false });
    }
    let mut homes: Vec<(&i64, &i64)> = frame.slots.iter().map(|(_, home)| home).zip(std::iter::repeat(&0)).collect();
    homes.sort();
    homes.dedup_by_key(|(home, _)| **home);
    for (home, _) in homes {
        let size = frame.capacities.get(home).copied().unwrap_or(frame.align).max(1);
        objects.push(Object { size, align: natural(size), offset: 0, callee_saved: false, dead: false });
    }
    let layout = Frame {
        local_area: 0,
        max_align: 1,
        stack_align: 1,
        transient_align: 1,
        adjusts_stack: false,
        has_var_sized: false,
        realigned: false,
        reserved_call_frame: false,
        max_call_frame_size: 0,
    };
    let pin = calculate_frame_object_offsets(&mut objects, 0, layout);
    format!("FRAME-ORACLE {name} objects={} ours={reserve} pin={pin}", objects.len())
}

#[cfg(test)]
mod tests {
    use super::{Frame, Object, calculate_frame_object_offsets};

    fn object(
        size: i64,
        align: i64,
    ) -> Object {
        Object { size, align, offset: 0, callee_saved: false, dead: false }
    }

    fn frame() -> Frame {
        Frame {
            local_area: 4,
            max_align: 1,
            stack_align: 16,
            transient_align: 4,
            adjusts_stack: false,
            has_var_sized: false,
            realigned: false,
            reserved_call_frame: true,
            max_call_frame_size: 0,
        }
    }

    /// Without the alignment of `c` the frame was 9 bytes: a double at
    /// `[ebp-9]` costs a 486 three clocks per access.
    #[test]
    fn objects_are_laid_below_the_local_area_by_size_and_alignment() {
        let mut objects = vec![object(4, 4), object(1, 1), object(8, 8)];
        let size = calculate_frame_object_offsets(&mut objects, 0, frame());
        assert_eq!(objects.iter().map(|one| one.offset).collect::<Vec<_>>(), [-8, -9, -24]);
        assert_eq!(size, 20);
    }

    /// A function that calls adds its outgoing area and rounds to the stack
    /// alignment; one that does not rounds to the transient one.
    #[test]
    fn a_call_frame_adds_and_the_stack_alignment_rounds() {
        let mut objects = vec![object(4, 4)];
        let calls = Frame { adjusts_stack: true, max_call_frame_size: 12, ..frame() };
        // 4 + 4 = 8, +12 = 20, aligned to 16 = 32, less the local area 4.
        assert_eq!(calculate_frame_object_offsets(&mut objects, 0, calls), 28);
        assert_eq!(calculate_frame_object_offsets(&mut objects, 0, frame()), 4);
    }

    /// Callee-saved objects come first, last to first; fixed ones raise the
    /// start; a dead object takes no space.
    #[test]
    fn saved_registers_precede_the_rest_and_the_dead_take_nothing() {
        let saved = |size| Object { callee_saved: true, ..object(size, 4) };
        let mut objects = vec![
            Object { offset: -12, ..object(8, 4) },
            object(4, 4),
            saved(4),
            saved(4),
            Object { dead: true, ..object(64, 4) },
        ];
        let size = calculate_frame_object_offsets(&mut objects, 1, frame());
        assert_eq!(objects.iter().map(|one| one.offset).collect::<Vec<_>>(), [-12, -24, -20, -16, 0]);
        assert_eq!(size, 20);
    }

    /// The oracle lays the selector's extents and the spill slots as the
    /// pin's function: a 1-byte, a 4-byte and an 8-byte extent are 1, 4 (after
    /// 1 aligned to 4) and 8: 8 + 8 = 16.
    #[test]
    fn the_oracle_lays_extents_and_slots_by_the_pins_function() {
        let mut frame = crate::backend::frame::Frame::new(0);
        frame.align = 8;
        frame.extents = vec![(-1, 1), (-8, 4), (-16, 8)];
        let line = super::oracle_line("f", &frame, 16);
        assert_eq!(line, "FRAME-ORACLE f objects=3 ours=16 pin=16", "{line}");
    }
}
