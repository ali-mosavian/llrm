//! Port of the may-alias half of `tests/test_module.py`.

use std::path::Path;

use iced_x86::Register;

use crate::analysis::regions::{self, RegionLayout};
use crate::objectfile::module::{Addr, Space, WIDEST, landmarks, of, reach};
use crate::objectfile::omf;
use crate::support::hash::IndexMap;

fn seg(disp: i64, index: i64) -> Addr {
    Addr { index, ..Addr::new(Space::Segment, disp) }
}

fn frame(disp: i64) -> Addr {
    Addr::new(Space::Frame, disp)
}

fn es_bx(disp: i64, segment: Register) -> Addr {
    Addr { base: Register::BX, segment, ..Addr::new(Space::Far, disp) }
}

fn addresses(a: Option<Addr>, a_width: i64, b: Option<Addr>, b_width: i64, bounds: Option<&RegionLayout>) -> bool {
    regions::addresses(a, a_width as u32, b, b_width as u32, bounds).unwrap()
}

/// Python's `bounds` alone, with no `layout`.
fn bounds_only(bounds: &IndexMap<(Space, i64), Vec<i64>>) -> RegionLayout {
    RegionLayout { shared_segments: None, landmarks: bounds.iter().map(|(&at, disps)| (at, disps.clone())).collect() }
}

#[test]
fn test_may_alias() {
    let cases = [
        (frame(-4), seg(0, 9), false),
        (seg(0, 9), frame(-4), false),
        (frame(-4), seg(0, 1), false),
        (Addr::new(Space::Stack, -2), seg(6, 1), false),
        (seg(6, 1), Addr::new(Space::Stack, -2), false),
        (Addr::new(Space::Stack, -2), frame(-4), true),
        (seg(0, 1), seg(2, 9), false),
        (seg(0, 1), seg(64, 1), false),
        (seg(0, 1), seg(2, 1), true),
        (frame(-4), frame(-6), true),
        (Addr { base: Register::SI, ..seg(0, 1) }, seg(64, 1), true),
        (frame(-4), Addr { index: 1, ..Addr::new(Space::Group, 0) }, true),
        (es_bx(0, Register::ES), es_bx(64, Register::ES), true),
        (es_bx(0, Register::ES), es_bx(0, Register::SS), true),
        (es_bx(0, Register::ES), seg(0, 9), true),
    ];
    for (a, b, expected) in cases {
        assert_eq!(addresses(Some(a), WIDEST, Some(b), WIDEST, None), expected, "{a:?} {b:?}");
    }
}

#[test]
fn test_may_alias_narrows_with_a_known_width() {
    for (width, expected) in [(2, false), (4, true)] {
        assert_eq!(addresses(Some(frame(-4)), width, Some(frame(-6)), width, None), expected, "{width}");
    }
}

#[test]
fn test_may_alias_over_states_an_unstated_width() {
    assert!(addresses(Some(seg(0, 1)), WIDEST, Some(seg(WIDEST - 1, 1)), WIDEST, None));
}

#[test]
fn test_may_alias_is_conservative_about_the_unknown() {
    assert!(addresses(None, 2, Some(frame(-4)), 2, None));
    assert!(addresses(Some(seg(0, 9)), 2, None, 2, None));
}

#[test]
fn test_an_indexed_operand_is_bounded_by_the_next_thing_named_after_it() {
    let path = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/omf/matrix-p-g2.obj");
    let found = of(&omf::parse(&std::fs::read(path).unwrap()).unwrap()).unwrap();
    let bounds = landmarks(&found);
    let seen = &bounds[&(Space::Segment, 5)];
    assert!(seen[..2] == [0x0, 0x6] && seen.contains(&0x328), "{seen:?}");

    let array = Addr { base: Register::SI, ..seg(0x6, 5) };
    assert_eq!(reach(&array, 2, &bounds), Some((0x6, 0x328)), "up to the next name, and no further");

    let bounded = bounds_only(&bounds);
    for disp in [0x328, 0x32A, 0x32C] {
        let scalar = seg(disp, 5);
        assert!(addresses(Some(array), 2, Some(scalar), 2, None), "unbounded, it reaches everything");
        assert!(!addresses(Some(array), 2, Some(scalar), 2, Some(&bounded)), "bounded, it cannot reach {disp:#x}");
    }

    let inside = seg(0x100, 5);
    assert!(addresses(Some(array), 2, Some(inside), 2, Some(&bounded)));

    let empty = IndexMap::default();
    assert_eq!(reach(&array, 2, &empty), None);
    assert!(addresses(Some(array), 2, Some(seg(0x328, 5)), 2, Some(&bounds_only(&empty))));
}
