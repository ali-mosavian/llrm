//! Port of `tests/test_module.py`, and the corpus helpers of
//! `tests/conftest.py` and `tests/corpus.py` the other object-core tests share.
//!
//! Skipped, needing `legacy.lift.lift`, which is not ported:
//! `test_a_static_operand_is_invisible_without_the_fixups`,
//! `test_the_fixups_are_what_make_a_static_pair_visible`,
//! `test_the_fixups_make_the_pairs_visible`,
//! `test_every_value_starts_where_ndisasm_says_an_instruction_does`.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use iced_x86::Register;

use super::*;
use crate::analysis::regions::{self, RegionLayout};
use crate::objectfile::omf;

/// conftest's `tests/fixtures`.
pub fn fixtures() -> PathBuf {
    Path::new(env!("LLRM_ROOT")).join("tests/fixtures/omf")
}

/// conftest's `obj`: every committed OMF object, sorted by name. Python's
/// `mapped_obj` is the same list: all 504 map.
pub fn objects() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(fixtures())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "obj"))
        .collect();
    found.sort();
    assert_eq!(found.len(), 504);
    found
}

/// `corpus.loaded`.
pub fn loaded(path: impl AsRef<Path>) -> Option<Module> {
    of(&omf::read(path).unwrap())
}

/// `Module(records, seg, name, code, start, end)` with every other field defaulted.
pub fn bare(found: &Module, code: Vec<u8>, start: i64, end: i64) -> Module {
    Module {
        records: found.records.clone(),
        seg: found.seg,
        name: found.name.clone(),
        code,
        start,
        end,
        operands: IndexMap::default(),
        calls: IndexMap::default(),
        targets: BTreeSet::new(),
        publics: BTreeSet::new(),
        lines: BTreeSet::new(),
        chunks: Vec::new(),
        sites: BTreeSet::new(),
        fixup_at: IndexMap::default(),
        dgroup: Group::default(),
        program_data: None,
        refs: IndexMap::default(),
        float_protocols: IndexMap::default(),
        absorbed: IndexMap::default(),
        coverage: IndexMap::default(),
    }
}

const OPERATOR_OBJECTS: [&str; 4] = ["pds-g2.obj", "qb45.obj", "vbdos-g2.obj", "vbdos-g3.obj"];
const WITH_PAIRS: &str = "jumptable.obj";

/// Expected strings printed by `repr(Addr(...))` in Python.
#[test]
fn addr_repr_matches_python() {
    assert_eq!(frame_relative(-8).repr(), "[bp-0x8]");
    assert_eq!(Addr::new(Space::Literal, 0).repr(), "[abs+0x0]");
    assert_eq!(Addr { index: 3, ..Addr::new(Space::Segment, 0x12) }.repr(), "[seg:3+0x12]");
    assert_eq!(Addr { base: Register::SI, ..Addr::new(Space::Frame, 2) }.repr(), "[bp+si+0x2]");
    assert_eq!(far_pointer(2, Register::BX, Register::ES).repr(), "[es:bx+0x2]");
    assert_eq!(Addr { base: Register::AX, ..Addr::new(Space::Frame, 0) }.repr(), "[bp+r21+0x0]");
    assert_eq!(Space::Frame.repr(), "<Space.FRAME: 'bp'>");
}

#[test]
fn test_a_runtime_call_is_a_lookup_not_a_guess() {
    for name in OPERATOR_OBJECTS {
        let found = loaded(fixtures().join(name)).unwrap();
        let names: BTreeSet<&str> = found.calls.values().map(String::as_str).collect();
        assert!(names.contains("B$CPI4") && names.contains("B$DVI4"), "{name}");
        for (&at, called) in &found.calls {
            assert_eq!(found.code[at as usize], CALL_FAR, "{called} is not reached by a far call");
        }
    }
}

#[test]
fn test_an_indirect_jump_has_findable_targets() {
    let found = loaded(fixtures().join(WITH_PAIRS)).unwrap();
    assert_eq!(found.targets.iter().copied().collect::<Vec<_>>(), [0x46, 0x52, 0x5E, 0xEA]);
    assert!(found.targets.iter().all(|&target| target < found.end));
}

#[test]
fn test_the_module_header_is_a_data_structure_not_code() {
    for obj in objects() {
        let found = loaded(&obj).unwrap();
        assert!(found.operands.keys().any(|&at| at <= 0x20), "{obj:?}: the header carries relocated fields");
        assert!(
            !found.operands.keys().any(|&at| 0x20 < at && at < 0x31),
            "{obj:?}: and nothing between it and the first operand"
        );
    }
}

#[test]
fn test_the_last_write_to_a_byte_is_the_one_that_counts() {
    for (at, written) in [(0x50, 21), (0x5C, 9), (0x89, 80), (0xA7, 50), (0xC5, 20)] {
        let records = omf::read(fixtures().join(WITH_PAIRS)).unwrap();
        let (seg, _name, size) = omf::code_segment(&records).unwrap();
        assert_eq!(omf::segment_image(&records, seg, size)[at], written, "{at:#x}");
    }
}

#[test]
fn test_nothing_in_the_corpus_has_to_be_refused() {
    for obj in objects() {
        assert_eq!(omf::refusals(&omf::read(&obj).unwrap()), Vec::<String>::new(), "{obj:?}");
    }
}

#[test]
fn test_a_record_nothing_here_decodes_is_refused() {
    for (kind, why) in [(omf::LIDATA, "LIDATA"), (omf::FIXUPP + 1, "32-bit"), (0xC2, "COMDAT")] {
        let mut records = omf::read(fixtures().join(WITH_PAIRS)).unwrap();
        records.push(Rc::new(Record::new(kind, vec![0])));
        assert!(omf::refusals(&records).iter().any(|reason| reason.contains(why)), "{why}");
    }
}

#[test]
fn test_dgroup_is_populated_on_every_object() {
    for obj in objects() {
        let found = loaded(&obj).unwrap();
        assert_eq!(found.dgroup.members.len(), 11, "{obj:?}");
        assert!(!found.dgroup.contains(found.seg), "the code segment is never in DGROUP");
    }
}

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

#[test]
fn test_the_compiler_that_made_an_object_is_read_off_it() {
    for (name, want) in [
        ("bools-q-O.obj", Family::Quickbasic),
        ("fpemu-p-evt.obj", Family::Pds),
        ("cmpord-v-g3.obj", Family::Vbdos),
    ] {
        let at = fixtures().join(name.to_lowercase());
        assert!(at.exists(), "{name} is checked in and this test needs it");
        let got = family(&omf::parse(&std::fs::read(&at).unwrap()).unwrap());
        assert_eq!(got, want, "{name} reads as {got}");
    }
}
