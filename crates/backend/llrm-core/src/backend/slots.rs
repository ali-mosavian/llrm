//! The one rule for sharing a frame slot: two things share when they are never
//! live together. A slot is a home, the most bytes any holder needs, and what
//! its holders occupy over time; `spiller` shares spill slots by it and `isel`
//! the slots of block locals by their lifetimes.

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
