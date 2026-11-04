//! Whether two memory accesses can touch the same byte, from what their
//! addresses name: the pin's `BaseIndexOffset::computeAliasing`, on facts a
//! caller has already read off its own IR (DAG nodes, machine operands). It
//! knows no IR: an [`Object`] is a kind and integers, a size a number.
//!
//! # How it works
//! [`alias_of`] takes the byte distance between the accesses when the caller
//! knows it (same base and index), and each access's [`Object`]. A known
//! distance decides by the sizes. Otherwise accesses of different kinds of
//! object, two different globals, or two frame objects of which one is an
//! alloca, are apart; anything else is `MayAlias`.
//!
//! # Example
//! Two 4-byte accesses on one base, 3 bytes apart, overlap by one byte; with
//! no distance, two different globals are apart:
//!
//! ```text
//! alias_of::<&str>(Some(3), None, Some(4), None, Some(4))
//!     == PartialAlias { offset: Some(-3) }
//! alias_of(None, Some(&Global("a")), Some(4), Some(&Global("b")), Some(4))
//!     == NoAlias
//! ```
//!
//! # Used by
//! The DAG combiner's chain walk (`mayAlias`) and arm64's load/store pairing.

use crate::aa::AliasResult;

/// What an access is relative to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Object<G> {
    /// A frame object; `fixed` ones (incoming arguments) sit at known places.
    Frame {
        id: i64,
        fixed: bool,
    },
    Global(G),
    /// A constant pool entry.
    Pool,
}

/// How accesses of `bytes_a` and `bytes_b` bytes (`None`: unknown) alias:
/// `MayAlias` when the addresses do not decide. `distance` is the bytes from
/// the first to the second, where the caller knows it.
#[must_use]
pub fn alias_of<G: Eq>(
    distance: Option<i64>,
    a: Option<&Object<G>>,
    bytes_a: Option<u64>,
    b: Option<&Object<G>>,
    bytes_b: Option<u64>,
) -> AliasResult {
    if let Some(distance) = distance {
        // If the size of memory access is unknown, do not use it.
        let (before, after) = if distance >= 0 { (bytes_a, distance) } else { (bytes_b, -distance) };
        return match before {
            None => AliasResult::MayAlias,
            Some(size) if size as i64 <= after => AliasResult::NoAlias,
            Some(size) if distance == 0 && bytes_a == bytes_b && size > 0 => AliasResult::MustAlias,
            Some(_) => AliasResult::PartialAlias { offset: i32::try_from(-distance).ok() },
        };
    }
    let apart = match (a, b) {
        (Some(Object::Frame { id: x, fixed: fx }), Some(Object::Frame { id: y, fixed: fy })) => x != y && !(*fx && *fy),
        (Some(Object::Global(x)), Some(Object::Global(y))) => x != y,
        (Some(Object::Pool), Some(Object::Pool)) | (None, _) | (_, None) => false,
        _ => true,
    };
    if apart { AliasResult::NoAlias } else { AliasResult::MayAlias }
}

#[cfg(test)]
mod tests {
    use AliasResult::{MayAlias, MustAlias, NoAlias, PartialAlias};

    use super::*;

    const GLOBAL: Object<&str> = Object::Global("g");

    fn at(
        distance: i64,
        bytes_a: Option<u64>,
        bytes_b: Option<u64>,
    ) -> AliasResult {
        alias_of::<&str>(Some(distance), None, bytes_a, None, bytes_b)
    }

    #[test]
    fn a_known_distance_decides_by_the_sizes() {
        assert_eq!(at(4, Some(4), Some(4)), NoAlias);
        assert_eq!(at(3, Some(4), Some(4)), PartialAlias { offset: Some(-3) });
        assert_eq!(at(-4, Some(4), Some(4)), NoAlias);
        assert_eq!(at(-3, Some(4), Some(4)), PartialAlias { offset: Some(3) });
        assert_eq!(at(0, Some(4), Some(4)), MustAlias);
        assert_eq!(at(8, None, Some(4)), MayAlias);
    }

    #[test]
    fn different_kinds_and_different_globals_are_apart() {
        let slot = Object::Frame { id: 1, fixed: false };
        assert_eq!(alias_of(None, Some(&slot), Some(4), Some(&GLOBAL), Some(4)), NoAlias);
        assert_eq!(alias_of(None, Some(&GLOBAL), Some(4), Some(&Object::Global("h")), Some(4)), NoAlias);
        assert_eq!(alias_of(None, Some(&GLOBAL), Some(4), Some(&GLOBAL), Some(4)), MayAlias);
    }

    #[test]
    fn frame_objects_are_apart_unless_both_are_fixed_or_the_same() {
        let slot = |id, fixed| Object::Frame { id, fixed };
        let of = |a, b| alias_of::<&str>(None, Some(&a), None, Some(&b), None);
        assert_eq!(of(slot(1, false), slot(2, false)), NoAlias);
        assert_eq!(of(slot(1, false), slot(2, true)), NoAlias);
        assert_eq!(of(slot(1, true), slot(2, true)), MayAlias);
        assert_eq!(of(slot(1, false), slot(1, false)), MayAlias);
    }

    #[test]
    fn an_unnamed_object_says_nothing() {
        assert_eq!(alias_of(None, None, Some(4), Some(&GLOBAL), Some(4)), MayAlias);
    }
}
