//! Port of `tests/test_module.py`.
//!
//! Skipped, needing `legacy.lift.lift`, which is not ported:
//! `test_a_static_operand_is_invisible_without_the_fixups`,
//! `test_the_fixups_are_what_make_a_static_pair_visible`,
//! `test_the_fixups_make_the_pairs_visible`,
//! `test_every_value_starts_where_ndisasm_says_an_instruction_does`.

use std::rc::Rc;

use iced_x86::Register;

use super::*;
use crate::testing::{fixtures, loaded, objects};
use crate::omf;

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
