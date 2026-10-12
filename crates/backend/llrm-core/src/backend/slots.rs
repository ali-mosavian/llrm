//! Lets two values share one frame slot when they are never live together, so
//! a frame holds the most that is live at once, not the sum of all spills.
//!
//! # How it works
//! A slot (`Color`) is its home, the capacity in bytes of the largest holder,
//! and the interval each holder occupies. `fits` accepts a new holder when the
//! slot is big enough and its interval overlaps no holder's. `choose` returns
//! the first slot listed in `partners` (homes the caller prefers) that fits,
//! else the first slot that fits, else none, and the caller opens a new slot.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','textColor':'#64748b','titleColor':'#64748b','gridColor':'#94a3b8','sectionBkgColor':'transparent','sectionBkgColor2':'transparent','altSectionBkgColor':'transparent','taskTextColor':'#ffffff','taskTextOutsideColor':'#64748b','taskTextLightColor':'#ffffff','taskTextDarkColor':'#ffffff','doneTaskBkgColor':'#16a34a','doneTaskBorderColor':'#166534','activeTaskBkgColor':'#2563eb','activeTaskBorderColor':'#1e40af','critBkgColor':'#dc2626','critBorderColor':'#991b1b','taskBkgColor':'#475569','taskBorderColor':'#334155','todayLineColor':'transparent'},'gantt':{'barHeight':34,'barGap':12,'topPadding':48,'leftPadding':150,'fontSize':14,'sectionFontSize':14,'numberSectionStyles':2}}}%%
//! gantt
//!   dateFormat X
//!   axisFormat %s
//!   tickInterval 1second
//!   section slot A (4 bytes)
//!   x :done, t10, 1, 2
//!   y∶ shares A with x :done, t11, 4, 5
//!   section slot B (4 bytes)
//!   z overlaps both∶ own slot :crit, t12, 2, 4
//! ```
//!
//! *Slots shared by intervals that do not overlap*
//!
//! # Example
//! None: this is a helper that answers one question per call and has no output
//! of its own. Its effect shows in the spill slots of the `spiller` and
//! `ssaspill` examples.
//!
//! # Used by
//! - [`crate::backend::spiller`], which shares spill slots by the live range of
//!   each spilled value and prefers the slot of a related value.
//! - [`crate::backend::isel`], which shares the slots of block locals by their
//!   lifetimes.

use crate::analysis::intervals::Interval;

/// A slot: its home, its capacity in bytes, and what each holder occupies.
pub type Color = (i64, u32, Vec<Interval>);

/// Whether `color` can also hold something of `capacity` bytes live over
/// `interval`.
pub fn fits(
    color: &Color,
    interval: &Interval,
    capacity: u32,
) -> bool {
    color.1 >= capacity && color.2.iter().all(|other| !interval.overlaps(other))
}

/// The slot to put something in: one of the homes `partners` names that fits,
/// else the first that does; none where a new slot is needed.
pub fn choose(
    colors: &[Color],
    interval: &Interval,
    capacity: u32,
    partners: &[i64],
) -> Option<usize> {
    partners
        .iter()
        .find_map(|home| colors.iter().position(|one| one.0 == *home && fits(one, interval, capacity)))
        .or_else(|| colors.iter().position(|one| fits(one, interval, capacity)))
}
