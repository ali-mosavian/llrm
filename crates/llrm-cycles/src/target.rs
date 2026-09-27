//! Real-mode DOS as a target, what analyses ask of it through
//! `llrm_mir::target::Machine`.

use llrm_mir::target::Machine;

/// Real-mode DOS's foreign memory, as its machine description states it:
/// VGA and text video memory, and the ROMs above.
pub struct Dos;

impl Machine for Dos {
    fn foreign_span(&self, selectors: (i64, i64), offsets: (i64, i64), width: i64) -> Option<(i64, i64)> {
        let word = |(low, high): (i64, i64)| 0 <= low && low <= high && high <= 0xFFFF;
        if !word(selectors) || !word(offsets) {
            return None;
        }
        let (start, end) = (selectors.0 * 16 + offsets.0, selectors.1 * 16 + offsets.1 + width);
        let mut reached = start;
        for (from, to) in [(0xA0000, 0xC0000), (0xC0000, 0x10_0000)] {
            if from <= reached && reached < to {
                reached = to;
            }
        }
        (end <= reached).then_some((start, end))
    }
}
