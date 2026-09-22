//! Port of `tests/test_literal_initializers.py`.
//!
//! `test_literal_relocation_exclusion_covers_the_whole_patch` monkeypatches
//! `omf.fixups` to return one fixup; here the records hold exactly that one
//! FIXUPP instead. Its `loc=99` does not fit the record's four location bits,
//! so 15, likewise no location a literal pool admits, stands in for it.
//!
//! Skipped, needing `mir.bodies`:
//! `test_nonreturning_call_does_not_justify_narrowing_unknown_memory_reads`,
//! `test_unknown_literal_pool_layout_does_not_exclude_call_writes`,
//! `test_fpbench_one_survives_unrelated_pointer_relocations`,
//! `test_quickbasic_literal_initializers_prove_the_same_floating_exit`,
//! `test_unknown_write_invalidates_literal_entry_facts`,
//! `test_literal_entry_requires_unmodified_complete_loader_bytes`.
//! Skipped, needing `wholeseg`: `test_fpdeep_mix_outputs_fold_across_string_prints`.

use std::rc::Rc;

use super::*;
use crate::analysis::consts;
use crate::analysis::regions::overlapping;
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Cell, Held, MirBlock, MirBody, OpCode, Value};
use crate::objectfile::omf::Record;

fn load(path: &str) -> Module {
    module::load(path).unwrap().unwrap()
}

fn raised(body: MirBody) -> RaisedBody {
    RaisedBody::new(body)
}

/// A protected literal does not protect adjacent descriptor bytes or a straddling write.
#[test]
fn test_call_exclusion_requires_whole_byte_range() {
    for (offset, width, disjoint) in [(0, 1, true), (3, 1, true), (0, 4, true), (-1, 2, false), (3, 2, false)] {
        let start = Addr { index: 9, ..Addr::new(Space::Segment, 30) };
        let effect = MemRef { excludes: vec![(start, 4)], ..MemRef::new(None, 0) };
        let access = MemRef::new(Some(start.plus(offset)), width);
        assert_eq!(overlapping(&effect, &access, None, None, None).unwrap(), !disjoint, "{offset} {width}");
    }
}

/// FPDEEP's copied DOUBLE needs integer literal reads, not only x87 reads.
#[test]
fn test_literal_bytes_are_available_to_scalar_loads() {
    let found = load("fixtures/omf/fpdeep-p-g2.obj");
    let segments = omf::segments(&found.records);
    let (_, segment, start, payload) = omf::ledata(&found.records)
        .into_iter()
        .find(|item| {
            segments[item.1 as usize].as_ref().is_some_and(|one| one.0 == "BC_CN")
                && item.2 <= 0x22
                && item.2 + item.3.len() as i64 >= 0x2A
        })
        .unwrap();
    let payload = &payload[(0x22 - start) as usize..];
    for width in [1u32, 2, 4, 8] {
        let reference = MemRef::new(Some(Addr { index: segment, ..Addr::new(Space::Segment, 0x22) }), width);
        let value = Value { variable: 1, version: 1, ..Value::new(1, 0x30) };
        let mut load = Op::new(0x30, OpCode::Operation(Operation::Move), "mov", vec![value], Vec::new());
        load.loads = vec![reference.clone()];
        load.kind = Kind::Load;
        load.args = vec![Arg::Cell(Cell { r#ref: reference })];
        load.results = vec![Arg::Held(Held { value, width })];
        let body = MirBody::new(0x30, vec![MirBlock::new(0x30, Vec::new(), vec![load], Vec::new())]);
        let body = initialized(raised(body), &found, None).unwrap();
        let facts = consts::known(
            &Rc::new(body.body.clone()),
            Some(&found.dgroup.members),
            Some(&IndexMap::default()),
            None,
            None,
        );
        let expected = BigInt::from_bytes_le(num_bigint::Sign::Plus, &payload[..width as usize]);
        assert!(facts[&value] == consts::Known::new(expected, width), "{width}");
    }
}

/// FPDEEP's DOUBLE shares a LEDATA record with relocated string descriptors.
#[test]
fn test_literal_relocation_exclusion_covers_the_whole_patch() {
    let found = load("fixtures/omf/fpdeep-p-g2.obj");
    let segment = omf::segments(&found.records)
        .iter()
        .position(|item| item.as_ref().is_some_and(|one| one.0 == "BC_CN"))
        .unwrap() as i64;
    let reference = MemRef::new(Some(Addr { index: segment, ..Addr::new(Space::Segment, 0x22) }), 8);
    let mut read = Op::new(0x30, OpCode::Operation(Operation::Move), "mov", Vec::new(), Vec::new());
    read.loads = vec![reference.clone()];
    read.kind = Kind::Load;
    let body = MirBody::new(0x30, vec![MirBlock::new(0x30, Vec::new(), vec![read], Vec::new())]);
    for (offset, location, admitted) in [
        (-1, omf::LOC_OFF16, false),
        (0, omf::LOC_OFF16, false),
        (7, omf::LOC_OFF16, false),
        (8, omf::LOC_OFF16, true),
        (-3, omf::LOC_PTR32, false),
        (-4, omf::LOC_PTR32, true),
        (-1, omf::LOC_BASE, false),
        (-2, omf::LOC_BASE, true),
        (8, omf::LOC_PTR32, true),
        (8, 15, false),
    ] {
        // One segment-relative fixup at 0x22 + offset, frame and target this
        // segment, after the LEDATA record holding that byte.
        let (record, _, base, _) = omf::ledata(&found.records)
            .into_iter()
            .filter(|item| item.1 == segment && item.2 <= 0x22 + offset)
            .last()
            .unwrap();
        let at = 0x22 + offset - base;
        let mut fixup = vec![0xC0 | (location as u8) << 2 | (at >> 8) as u8, (at & 0xFF) as u8, 0x00];
        fixup.extend(omf::_emit_index(segment).unwrap());
        fixup.extend(omf::_emit_index(segment).unwrap());
        fixup.extend([0, 0]);
        let mut records: Vec<Rc<Record>> =
            found.records.iter().filter(|one| one.r#type & 0xFE != omf::FIXUPP).cloned().collect();
        let after = records.iter().position(|one| Rc::ptr_eq(one, &record)).unwrap();
        records.insert(after + 1, omf::fixupp_record(&[fixup]));
        let fixups = omf::fixups(&records);
        assert_eq!(fixups.len(), 1);
        assert_eq!((fixups[0].seg, fixups[0].offset, fixups[0].loc), (Some(segment), 0x22 + offset, location));
        let changed = Module { records, ..found.clone() };
        let result = initialized(raised(body.clone()), &changed, None).unwrap();
        assert_eq!(result.initial.iter().any(|(one, _)| *one == reference), admitted, "{offset} {location}");
    }
}
