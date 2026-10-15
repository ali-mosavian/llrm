//! Lets two values share one frame slot when they are never live together, so
//! a frame holds the most that is live at once, not the sum of all spills.
//!
//! # How it works
//! A slot (`Color`) is its home, the capacity in bytes of the largest holder,
//! and the interval each holder occupies. `fits` accepts a new holder when the
//! slot is big enough and its interval overlaps no holder's. `choose` returns
//! the first slot listed in `partners` (homes the caller prefers) that fits,
//! else the first slot that fits, else none, and the caller opens a new slot.
//! `moved` rewrites a body's frame cells when slots are laid out again, each
//! cell keeping its place in its slot and taking the new slot's tag.
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
//!   each spilled value and prefers the slot of a related value, and
//!   `slots_colored`, which does so once over the allocation kept.
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

/// `body` with every frame cell of a slot in `moved` put in the slot it is
/// moved to (old first byte -> new first byte): the displacement keeps its
/// place in the slot, and the cell is tagged with the new slot. A cell that
/// names no moved slot is left as it is. None where a cell cannot be moved.
pub fn moved(
    body: &crate::model::lir::LirBody,
    moved: &crate::support::hash::IndexMap<i64, i64>,
) -> Option<crate::model::lir::LirBody> {
    use std::sync::Arc;

    use crate::model::ir::{Addr, AddressRef, Loc, Mem, Semantics, Space};
    use crate::model::lir::{DebugPlace, Insn, LirBlock};
    let addr = |addr: Addr, offset: i64| -> (Addr, i64) {
        let Some(home) = addr.slot_home().filter(|_| addr.space == Space::Frame) else { return (addr, offset) };
        let Some(&new) = moved.get(&home) else { return (addr, offset) };
        let disp = addr.disp - home + new;
        // `offset` repeats the displacement on a cell the spiller made from a
        // folded frame address.
        let offset = if addr.disp != 0 && offset == addr.disp { disp } else { offset };
        (Addr { disp, ..addr }.in_slot(new), offset)
    };
    let loc = |place: &Loc| -> Loc {
        match place {
            Loc::Mem(cell) => match cell.addr {
                Some(found) => {
                    let (found, offset) = addr(found, cell.offset);
                    Loc::Mem(Mem { addr: Some(found), offset, ..cell.clone() })
                }
                None => place.clone(),
            },
            Loc::Address(cell) => match cell.addr {
                Some(found) => {
                    let (found, offset) = addr(found, cell.offset);
                    Loc::Address(AddressRef { addr: Some(found), offset, ..cell.clone() })
                }
                None => place.clone(),
            },
            other => other.clone(),
        }
    };
    let mut blocks = Vec::with_capacity(body.blocks.len());
    for block in &body.blocks {
        let insns: Vec<Arc<Insn>> = block
            .insns
            .iter()
            .map(|one| {
                let Some(what) = &one.what else { return Arc::clone(one) };
                let (dests, sources): (Vec<Loc>, Vec<Loc>) =
                    (what.dests.iter().map(&loc).collect(), what.sources.iter().map(&loc).collect());
                if dests == what.dests && sources == what.sources {
                    return Arc::clone(one);
                }
                Arc::new(Insn { what: Some(Semantics { dests, sources, ..what.clone() }), ..(**one).clone() })
            })
            .collect();
        blocks.push(LirBlock { insns: insns.into(), ..block.clone().into_inner() }.into());
    }
    let mut out = body.with_blocks(blocks);
    out.homes = Arc::new(
        body.homes
            .iter()
            .map(|(value, cell)| {
                let cell = match cell.addr {
                    Some(found) => {
                        let (found, offset) = addr(found, cell.offset);
                        Mem { addr: Some(found), offset, ..cell.clone() }
                    }
                    None => cell.clone(),
                };
                (*value, cell)
            })
            .collect(),
    );
    out.variables = body
        .variables
        .iter()
        .map(|one| match &one.place {
            DebugPlace::At(found) if found.space == Space::Frame => {
                let (found, _) = addr(*found, 0);
                crate::model::lir::DebugVariable { place: DebugPlace::At(found), ..one.clone() }
            }
            _ => one.clone(),
        })
        .collect();
    Some(out)
}
