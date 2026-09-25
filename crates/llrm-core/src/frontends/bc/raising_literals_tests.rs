//! Port of `tests/test_literal_initializers.py`.
//!
//! `test_literal_relocation_exclusion_covers_the_whole_patch` monkeypatches
//! `omf.fixups` to return one fixup; here the records hold exactly that one
//! FIXUPP instead. Its `loc=99` does not fit the record's four location bits,
//! so 15, likewise no location a literal pool admits, stands in for it.
//!
//! Skipped, monkeypatching `module.escaped`, `omf.fixups` and `omf.ledata`:
//! `test_unknown_literal_pool_layout_does_not_exclude_call_writes`.
//!
//! `test_fpdeep_mix_outputs_fold_across_string_prints` keeps only its q-O
//! case: p-g2 and v-g3 fail in Python at this commit (Unlowered opaque).

use std::rc::Rc;

use super::*;
use crate::analysis::{consts, floatfacts};
use crate::analysis::regions::overlapping;
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Cell, Held, MirBlock, MirBody, OpCode, Value};
use crate::objectfile::omf::Record;
use crate::testing;

fn load(path: &str) -> Module {
    module::load(std::path::Path::new(env!("LLRM_ROOT")).join(path)).unwrap().unwrap()
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
    let found = load(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/fpdeep-p-g2.obj"));
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
    let found = load(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/fpdeep-p-g2.obj"));
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

const FPCSE_Q: &str = concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/fpcse-q-o.obj");

/// B$CENP may enter a No RESUME handler before exit; noreturn does not mean no reads.
#[test]
fn test_nonreturning_call_does_not_justify_narrowing_unknown_memory_reads() {
    let routine = runtime::contract(Some("B$CENP"));
    assert_eq!(routine.control, runtime::Control::Never);
    assert_eq!(routine.reads, Memory::Any);
    let found = testing::loaded(FPCSE_Q).unwrap();
    let body = testing::nth(&testing::raised(FPCSE_Q), 0);
    let ops = testing::ops(&body);
    let terminal = ops
        .iter()
        .find(|op| op.kind == Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$CENP"))
        .unwrap();
    let stored = ops
        .iter()
        .flat_map(|op| &op.stores)
        .find(|one| one.addr.is_some_and(|addr| addr.space == Space::Segment))
        .unwrap();
    assert!(terminal.loads.iter().any(|one| testing::overlapping(one, stored, Some(&found.dgroup))));
}

/// FPBENCH lost its 1.0 literal fact because array descriptors elsewhere have far fixups.
#[test]
#[ignore = "fails in Python too: no initial fact at [seg:9+0x0]"]
fn test_fpbench_one_survives_unrelated_pointer_relocations() {
    let body = testing::nth(&testing::raised(concat!(env!("LLRM_ROOT"), "/tests/fixtures/bench/fpbench-v-g3.obj")), 0);
    let reference = MemRef::new(Some(Addr { index: 9, ..Addr::new(Space::Segment, 0) }), 4);
    let initial: IndexMap<MemRef, Const> = body.initial.iter().cloned().collect();
    assert_eq!(initial[&reference], Const::new(0x3F800000, 4));
}

/// FPCSE's QB object retained ten iterations while PDS/VBDOS proved 487.5.
#[test]
fn test_quickbasic_literal_initializers_prove_the_same_floating_exit() {
    let found = testing::loaded(FPCSE_Q).unwrap();
    let body = Rc::new(testing::nth(&testing::raised(FPCSE_Q), 0));
    let proofs = floatfacts::loop_exits(&body, &found.dgroup.members, &found.calls);
    assert!(proofs.len() == 1 && proofs[0].count == 10.into());
    assert!(proofs[0].stores.iter().any(|(_, fact)| fact.n == 0x43F3C000.into()));
    assert_eq!(mir::resolved(&body, None).unwrap().initial, body.initial);
    assert!(floatfacts::known(&body, &found.dgroup.members, &found.calls, Some(&IndexMap::default())).is_empty());
}

#[test]
fn test_unknown_write_invalidates_literal_entry_facts() {
    let found = testing::loaded(FPCSE_Q).unwrap();
    let mut body = testing::nth(&testing::raised(FPCSE_Q), 0);
    let entry = body.blocks.iter().position(|block| block.at == body.entry).unwrap();
    let mut clobber = body.blocks[entry].ops[0].clone();
    clobber.kind = Kind::Store;
    clobber.floating = None;
    clobber.floating_origin = None;
    clobber.args = vec![Arg::Const(Const::new(0, 4))];
    clobber.results = vec![Arg::Cell(Cell { r#ref: MemRef::new(None, 4) })];
    clobber.stores = vec![MemRef::new(None, 4)];
    clobber.loads = vec![];
    clobber.uses = vec![];
    clobber.defines = vec![];
    clobber.stack = None;
    body.blocks[entry].ops.insert(0, clobber);
    assert!(floatfacts::loop_exits(&Rc::new(body), &found.dgroup.members, &found.calls).is_empty());
}

#[test]
fn test_literal_entry_requires_unmodified_complete_loader_bytes() {
    for change in ["relocation", "missing_byte", "procedure"] {
        let found = testing::loaded(FPCSE_Q).unwrap();
        let mut body = testing::nth(&testing::raised(FPCSE_Q), 0);
        body.initial = vec![];
        let reference = body.blocks[0].ops[0].loads[0].clone();
        let addr = reference.addr.unwrap();
        let (record, index, start, payload) =
            omf::ledata(&found.records).into_iter().find(|item| item.1 == addr.index && item.2 == addr.disp).unwrap();
        let mut records = found.records.clone();
        let at = records.iter().position(|one| Rc::ptr_eq(one, &record)).unwrap();
        match change {
            "relocation" => {
                let template = omf::fixups(&records).into_iter().find(|fixup| fixup.seg == Some(index)).unwrap();
                records.insert(at + 1, omf::fixupp_record(&[omf::reemit(&template, Some(0), None).unwrap()]));
            }
            "missing_byte" => records[at] = omf::ledata_record(index, start + 1, &payload[1..]).unwrap(),
            _ => body.entry += 1,
        }
        let result = initialized(raised(body), &Module { records, ..found.clone() }, None).unwrap();
        assert!(!result.initial.iter().any(|(one, _)| *one == reference), "{change}");
    }
}

/// FPDEEP kept CLNG(q*1024) because PRINT invalidated the unrelated numeric literal.
#[test]
fn test_fpdeep_mix_outputs_fold_across_string_prints() {
    for tag in ["q-O"] {
        let data = testing::data(format!("tests/fixtures/omf/fpdeep-{tag}.obj").to_lowercase());
        let (result, states) = testing::emitted_mir(&data, "mir-widen", "");
        assert_eq!(result.outcome, crate::wholeseg::Emission::Lir, "{tag}: {}", result.reason);
        let printed: std::collections::BTreeSet<_> = states
            .iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.ops)
            .filter(|op| op.kind == Kind::Arg)
            .flat_map(|op| &op.args)
            .filter_map(|arg| match arg {
                Arg::Const(one) if one.width == 4 => Some(one.n.clone()),
                _ => None,
            })
            .collect();
        for n in [512, 768, 896] {
            assert!(printed.contains(&BigInt::from(n)), "{tag}: {n}");
        }
        for body in &states {
            let literal = body
                .initial
                .iter()
                .find(|(_, value)| value.n == BigInt::from(0x4480_0000) && value.width == 4)
                .map(|(one, _)| one.clone())
                .unwrap();
            assert!(
                !body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.ops)
                    .any(|op| op.kind == Kind::Fmul && op.loads.iter().any(|one| one.addr == literal.addr)),
                "{tag}"
            );
        }
    }
}
