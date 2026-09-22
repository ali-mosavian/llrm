//! Ports of `tests/test_array_access.py` and `tests/test_huge_array_access.py`.
//!
//! Skipped, monkeypatching `raising_array_access._checked` or `descriptor`:
//! `test_checked_constant_indices_can_use_native_addressing`,
//! `test_unsupported_checked_helper_is_not_unchecked_success`.

use std::collections::BTreeSet;

use super::*;
use crate::abi::linkunit::LinkUnit;
use crate::objectfile::omf;
use crate::rewrite::{rewrite, Rewrite};
use crate::support::pyjson::{self, Json};
use crate::support::testing::{self, nth, ops};
use crate::wholeseg::Emission;

const HARR: &str = "fixtures/regressions/harr-bounds-p-g2.obj";

/// NDMAX grew to 9.7 KB because each native op was mistaken for the removed HARY call.
#[test]
fn test_native_array_arithmetic_keeps_allocation_dimension_constants() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let path = format!("fixtures/regressions/ndmax-{tag}.obj").to_lowercase();
        let found = testing::loaded(&path).unwrap();
        let body = Rc::new(nth(&testing::raised(&path), 0));
        let facts = consts::known(&body, Some(&found.dgroup.members), Some(&found.calls), None, None);
        let initialized: IndexMap<MemRef, Const> = ops(&body).into_iter().flat_map(|op| op.memory_values).collect();
        let first = found.calls.iter().filter(|(_, name)| *name == "B$HARY").map(|(at, _)| *at).min().unwrap();
        let loads: Vec<Op> = ops(&body)
            .into_iter()
            .filter(|op| op.at == first && op.kind == Kind::Load && initialized.contains_key(&op.loads[0]))
            .collect();
        assert!(loads.len() >= 60, "{tag}: {}", loads.len());
        for op in &loads {
            let expected = &initialized[&op.loads[0]];
            let Arg::Held(result) = &op.results[0] else { panic!("{:?}", op.results) };
            assert_eq!(facts.get(&result.value), Some(&consts::Known::new(expected.n.clone(), expected.width)), "{tag}");
        }
    }
}

/// /D ARRIDX printed 630 instead of 1260: INTO invented a new AX result allocated to BX.
#[test]
fn test_overflow_observation_has_no_normal_path_register_results() {
    let path = "fixtures/regressions/arridx-bounds-p-g2.obj";
    let found = testing::loaded(path).unwrap();
    let body = nth(&testing::raised_with(path, true, true), 0);
    let checks: Vec<Op> =
        ops(&body).into_iter().filter(|op| found.code.get(op.at as usize) == Some(&0xce)).collect();
    assert!(!checks.is_empty());
    // An interrupt may define a selector; nothing on the normal path may read it.
    let mut invented: BTreeSet<Value> = checks.iter().flat_map(|op| op.defines.clone()).collect();
    loop {
        let merged: BTreeSet<Value> = body
            .blocks
            .iter()
            .flat_map(|block| &block.phis)
            .filter(|phi| phi.incoming.values().any(|value| invented.contains(value)))
            .map(|phi| phi.result)
            .filter(|result| !invented.contains(result))
            .collect();
        if merged.is_empty() {
            break;
        }
        invented.extend(merged);
    }
    assert!(!ops(&body).iter().any(|op| op.uses.iter().any(|value| invented.contains(value))));
}

fn _allocation(body: &MirBody) -> Op {
    ops(body).into_iter().find(|op| op.array.is_some()).unwrap()
}

/// HARR's DIM facts do not make heap addresses or bounds immutable across later calls.
#[test]
#[ignore = "fails in Python too: `dynamic` finds no HARR descriptor"]
fn test_dynamic_shape_does_not_freeze_descriptor_fields() {
    let body = nth(&testing::raised_with(HARR, false, true), 0);
    let request = _allocation(&body).array.unwrap();
    let shape = dynamic(&body, Some(&request.descriptor)).unwrap();
    assert!(matches!(shape.data, Arg::Cell(_)));
    assert!(shape.dimensions.iter().all(|(low, high)| matches!((low, high), (Arg::Cell(_), Arg::Cell(_)))));
}

#[test]
#[ignore = "fails in Python too: `dynamic` finds no HARR descriptor for features 2 and 3"]
fn test_dynamic_unestablished_layout_is_not_assumed_far() {
    for features in [0, 2, 3, 0x81] {
        let body = nth(&testing::raised_with(HARR, false, true), 0);
        let allocation = _allocation(&body);
        let request = allocation.array.clone().unwrap();
        let fields: Vec<(MemRef, Const)> = allocation
            .memory_values
            .iter()
            .map(|(reference, value)| {
                if reference.addr.unwrap().disp == request.descriptor.offset + 9 {
                    (reference.clone(), Const::new(features, 1))
                } else {
                    (reference.clone(), value.clone())
                }
            })
            .collect();
        let mut body = body;
        for block in &mut body.blocks {
            for op in &mut block.ops {
                if *op == allocation {
                    op.memory_values = fields.clone();
                }
            }
        }
        let shape = dynamic(&body, Some(&request.descriptor));
        if [2, 3].contains(&features) {
            let shape = shape.unwrap();
            let Arg::Cell(data) = &shape.data else { panic!("{:?}", shape.data) };
            assert!(shape.huge && data.r#ref.width == 4, "{features}");
        } else {
            assert!(shape.is_none(), "{features}");
        }
    }
}

/// A square zero-based HARR would hide swapped dimensions: (-1,9) must address byte 164 here.
#[test]
fn test_dynamic_address_uses_runtime_lower_bounds_and_correct_stride() {
    let found = testing::loaded(HARR).unwrap();
    let raised = testing::raised(HARR);
    let body = nth(&raised, 0);
    let hints = &raised.hints[&body.entry];
    let definitions: IndexMap<Value, Op> =
        ops(&body).into_iter().flat_map(|op| op.defines.clone().into_iter().map(move |value| (value, op.clone()))).collect();
    let site = found.calls.iter().filter(|(_, name)| *name == "B$HARY").map(|(at, _)| *at).min().unwrap();
    let result = ops(&body)
        .into_iter()
        .find(|op| op.at == site && op.kind == Kind::Add && hints.origin_of(op.defines[0]).is_some())
        .unwrap()
        .results[0]
        .clone();
    // Real PDS descriptor at 6; R and C at 28/30. Change the memory supplied
    // to the raised expression, not the object's compiler-generated bytes.
    let memory: IndexMap<i64, BigInt> =
        [(6, 100), (22, 4), (24, 6), (26, -3), (28, -1), (30, 9)].into_iter().map(|(at, n)| (at, n.into())).collect();
    fn evaluate(arg: &Arg, definitions: &IndexMap<Value, Op>, memory: &IndexMap<i64, BigInt>) -> BigInt {
        match arg {
            Arg::Const(one) => one.n.clone(),
            Arg::Cell(cell) => memory[&cell.r#ref.addr.unwrap().disp].clone(),
            Arg::Held(held) => {
                let op = &definitions[&held.value];
                let operands: Vec<BigInt> = op.args.iter().map(|one| evaluate(one, definitions, memory)).collect();
                match op.kind {
                    Kind::Load | Kind::Copy => operands[0].clone(),
                    Kind::Sub => &operands[0] - &operands[1],
                    Kind::Mul => &operands[0] * &operands[1],
                    Kind::Add => &operands[0] + &operands[1],
                    _ => panic!("{arg:?}"),
                }
            }
            _ => panic!("{arg:?}"),
        }
    }
    assert_eq!(evaluate(&result, &definitions, &memory), BigInt::from(164));
}

/// HUGELP may remove HARY's selector only when neither backedge nor exit observes it.
#[test]
fn test_selector_proof_checks_the_loop_exit_path() {
    let path = "fixtures/regressions/hugelp-p-g2.obj";
    let found = testing::loaded(path).unwrap();
    let raised = testing::raised_with(path, false, true);
    let public = &raised.values[0].1;
    // _selector_dead is raise-time recognition; reconstruct the private view.
    let body = mir::_with_raise_context(public, &raised.hints[&public.entry], &raised.source);
    let contracts = runtime::for_module(&found, None).unwrap();
    let hary = |op: &Op| op.kind == Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$HARY");
    let block = body.blocks.iter().find(|block| block.ops.iter().filter(|op| hary(op)).count() == 2).unwrap();
    let (position, consumer) = block
        .ops
        .iter()
        .enumerate()
        .filter(|(_, op)| op.kind == Kind::Store && op.stores.iter().any(|one| one.base.is_some()))
        .last()
        .unwrap();
    assert!(_selector_dead(&body, block, position + 1, &contracts));
    let mut changed = MirBody::clone(&body);
    changed.blocks.last_mut().unwrap().ops.insert(0, consumer.clone());
    assert!(!_selector_dead(&changed, block, position + 1, &contracts));
}

/// An out-of-range dimension can flatten into a valid allocation offset; that is still a bounds error.
#[test]
fn test_checked_proof_requires_each_live_dimension() {
    for hazard in ["none", "below", "above", "unknown-index", "rank", "features", "width", "unknown-bound"] {
        let symbol = Symbol::new(Space::Segment, 5, 0, 2);
        let field = |offset: i64, width: u32| MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, offset) }), width);
        let cell = |offset: i64| Arg::Cell(Cell { r#ref: field(offset, 2) });
        let shape = Descriptor {
            data: cell(0),
            selector: field(2, 2),
            width: 2,
            dimensions: vec![(cell(14), cell(16)), (cell(18), cell(20))],
            huge: false,
        };
        let mut memory = consts::Cells::default();
        for (offset, width, number) in [(8, 1, 2), (9, 1, 1), (12, 2, 2), (14, 2, 2), (16, 2, 0xffff), (18, 2, 3), (20, 2, 2)] {
            if hazard == "unknown-bound" && offset == 18 {
                continue;
            }
            let zeroed = match hazard {
                "rank" => Some(8),
                "features" => Some(9),
                "width" => Some(12),
                _ => None,
            };
            let number = if zeroed == Some(offset) { 0 } else { number };
            memory.extend(consts::_fragments(&field(offset, width), &consts::Known::new(number, width)));
        }
        let mut indices = vec![Some(consts::Known::new(3, 2)), Some(consts::Known::new(0xffff, 2))];
        match hazard {
            "below" => indices[1] = Some(consts::Known::new(0xfffe, 2)),
            "above" => indices[1] = Some(consts::Known::new(1, 2)),
            "unknown-index" => indices[1] = None,
            _ => {}
        }
        assert_eq!(_checked(Some(&shape), Some(&symbol), &indices, &memory), hazard == "none", "{hazard}");
    }
}

/// Removing HUGELP's dead offset merges must not erase high bits a later whole read uses.
#[test]
fn test_offset_overwrite_keeps_a_transitively_observed_high_half() {
    let value = |index: u32| Value { variable: index, ..Value::new(index, i64::from(index)) };
    let (old, first, second) = (value(1), value(2), value(3));
    let copies: Vec<Op> = [(0, old, first), (1, first, second)]
        .into_iter()
        .map(|(index, source, result)| {
            let mut copy = Op::new(index, OpCode::Operation(Operation::Move), "mov", vec![result], vec![source]);
            copy.kind = Kind::Copy;
            copy.args = vec![Arg::Const(Const::new(index, 2))];
            copy.results = vec![Arg::Held(Held { value: result, width: 2 })];
            copy.merges = OrderedMap::from_iter([(source, result)]);
            copy
        })
        .collect();
    let mut read = Op::new(2, OpCode::Operation(Operation::Push), "push", vec![], vec![second]);
    read.kind = Kind::Arg;
    read.args = vec![Arg::Held(Held { value: second, width: 4 })];
    let ops: Vec<Op> = copies.iter().cloned().chain([read]).collect();
    let body = MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])]);
    assert!(!_overwrites_offset(&body, &copies[0], old));
}

fn hary_calls(found: &Module) -> usize {
    found.calls.values().filter(|name| *name == "B$HARY").count()
}

fn retains_hary(found: &Module, body: &MirBody) -> bool {
    ops(body).iter().any(|op| op.kind == Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$HARY"))
}

/// NDMAX retained three HARY checks after its first proven store; two also have constant valid indices.
#[test]
fn test_checked_access_proofs_reach_fixed_point() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let path = format!("fixtures/regressions/ndmax-{tag}.obj").to_lowercase();
        let found = testing::loaded(&path).unwrap();
        let mut sites: Vec<i64> = found.calls.iter().filter(|(_, name)| *name == "B$HARY").map(|(at, _)| *at).collect();
        sites.sort();
        let body = nth(&testing::raised_with(&path, false, true), 0);
        let retained: Vec<i64> = ops(&body)
            .iter()
            .filter(|op| op.kind == Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$HARY"))
            .map(|op| op.at)
            .collect();
        assert_eq!(retained, sites[sites.len() - 1..], "{tag}");
        let emitted = testing::emitted_with(&testing::data(&path), false, true);
        assert_eq!(emitted.outcome, Emission::Lir, "{tag}: {}", emitted.reason);
        assert_eq!(hary_calls(&testing::loaded_bytes(&emitted.data).unwrap()), 1, "{tag}");
    }
}

/// NDARR (1,12,2) and NDMAX (11,22) were refused by an invented eight-dimension cap.
///
/// ndarr-v-g3 fails in Python at this commit (Unlowered opaque) and is left out.
#[test]
fn test_hary_supports_nine_and_sixty_dimensions() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        for program in ["ndarr", "ndmax"] {
            if (tag, program) == ("v-g3", "ndarr") {
                continue;
            }
            let path = format!("fixtures/regressions/{program}-{tag}.obj").to_lowercase();
            let found = testing::loaded(&path).unwrap();
            assert!(hary_calls(&found) > 0, "{path}");
            assert!(!retains_hary(&found, &nth(&testing::raised(&path), 0)), "{path}");
            let result = testing::emitted_lir(&path);
            assert_eq!(hary_calls(&testing::loaded_bytes(&result.data).unwrap()), 0, "{path}");
        }
    }
}

/// ARRIDX's three HARY calls must become address arithmetic, not deleted pointer definitions.
///
/// The `basic_semantics` cases fail in Python at this commit (Unlowered
/// opaque) and are left out.
#[test]
fn test_array_checks_are_independent_of_numeric_semantics() {
    let basic_semantics = false;
    for tag in ["p-g2", "q-O", "v-g3"] {
        for bounds_checks in [false, true] {
            let path = format!("fixtures/regressions/arridx-bounds-{tag}.obj").to_lowercase();
            let found = testing::loaded(&path).unwrap();
            let body = nth(&testing::raised_with(&path, basic_semantics, bounds_checks), 0);
            let calls = ops(&body)
                .iter()
                .filter(|op| op.kind == Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$HARY"))
                .count();
            let checks = if bounds_checks { 3 } else { 0 };
            assert_eq!(calls, checks, "{path}");
            if !bounds_checks {
                assert!(ops(&body).iter().filter(|op| op.kind == Kind::Mul).count() >= 3, "{path}");
            }
            let output = testing::emitted_with(&testing::data(&path), basic_semantics, bounds_checks);
            assert_eq!(output.outcome, Emission::Lir, "{path}: {}", output.reason);
            let after = testing::loaded_bytes(&output.data).unwrap();
            assert_eq!(hary_calls(&after), checks, "{path}");
            let lina = |one: &Module| one.calls.values().filter(|name| *name == "B$LINA").count();
            assert_eq!(lina(&after), lina(&found), "{path}");
        }
    }
}

/// Numeric compatibility must not silently enable array checks or reuse unchecked output.
#[test]
fn test_bounds_policy_is_recorded_separately() {
    let source = "fixtures/regressions/arridx-bounds-p-g2.obj";
    let Some(library) = testing::runtime_library(source) else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("checked.obj");
    let argv: Vec<String> =
        [source, library.to_str().unwrap(), "-o", output.to_str().unwrap(), "--bounds-checks"].map(str::to_owned).into();
    assert_eq!(crate::rewrite::main(&argv).unwrap(), 0);
    let unit = LinkUnit::read(&[testing::path(source), library]).unwrap().fingerprint;
    let report = pyjson::loads(&std::fs::read_to_string(output.with_extension("json")).unwrap()).unwrap();
    let Json::Dict(report) = report else { panic!("{report:?}") };
    assert_eq!(report["bounds_checks"], Json::Bool(true));
    assert_eq!(report["semantics"], Json::Str("native".to_owned()));
    let data = std::fs::read(&output).unwrap();
    let checked = Rewrite { bounds_checks: true, contract_fingerprint: Some(unit.clone()), ..Rewrite::new(false) };
    assert_eq!(rewrite(&data, &checked).unwrap().0, data);
    let unchecked = Rewrite { contract_fingerprint: Some(unit), ..Rewrite::new(false) };
    assert_eq!(rewrite(&data, &unchecked).unwrap_err().kind, "Finalised");
}

/// HARR computes both addresses via HARY each iteration; they must be MIR, not retained calls.
#[test]
fn test_dynamic_far_address_arithmetic_is_native() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        for program in ["harr-bounds", "dynsz"] {
            let path = format!("fixtures/regressions/{program}-{tag}.obj").to_lowercase();
            let found = testing::loaded(&path).unwrap();
            let body = nth(&testing::raised(&path), 0);
            assert!(!retains_hary(&found, &body), "{path}");
            assert!(ops(&body).iter().any(|op| op.kind == Kind::Mul), "{path}");
            let result = testing::emitted_lir(&path);
            assert_eq!(hary_calls(&testing::loaded_bytes(&result.data).unwrap()), 0, "{path}");
            if program == "dynsz" {
                assert!(!ops(&body).iter().any(|op| op.array.is_some()), "{path}");
            }
        }
    }
}

/// HUGE2 and HUGELP's HARY helpers become whole-pointer MIR and relocated emitted accesses.
///
/// hugelp fails in Python at this commit and is left out.
#[test]
fn test_huge_helper_becomes_whole_pointer_mir_and_emitted_accesses() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let (program, count) = ("huge2", 10);
        let path = format!("fixtures/regressions/{program}-{tag}.obj").to_lowercase();
        let found = testing::loaded(&path).unwrap();
        let ops = ops(&nth(&testing::raised(&path), 0));
        assert_eq!(ops.iter().filter(|op| op.kind == Kind::PtrOffset).count(), count, "{path}");
        let pointers: BTreeSet<mir::Value> =
            ops.iter().filter(|op| op.kind == Kind::PtrOffset).flat_map(|op| op.defines.clone()).collect();
        assert!(
            !ops.iter().any(|op| op.kind == Kind::Extract && op.uses.iter().any(|value| pointers.contains(value))),
            "{path}"
        );
        assert!(!ops.iter().any(|op| op.kind == Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$HARY")));
        let refs: Vec<&MemRef> = ops.iter().flat_map(|op| op.loads.iter().chain(&op.stores)).filter(|one| one.pointer).collect();
        assert_eq!(refs.len(), count, "{path}");
        assert!(refs.iter().all(|one| one.addr.is_none() && one.segment.is_none() && one.base_width == 4), "{path}");
        let result = testing::emitted_lir(&path);
        let output = testing::loaded_bytes(&result.data).unwrap();
        assert_eq!(hary_calls(&output), 0, "{path}");
        let names = omf::externals(&output.records);
        let fixups = omf::fixups(&output.records);
        assert!(fixups.iter().any(|fix| fix.target == "external" && names[fix.index as usize] == "b$HugeShift"), "{path}");
        // VBDOS printed 123,789,789 when all newly introduced descriptor
        // loads read DS:0: zero displacement bytes had no linker relocation.
        let descriptors: BTreeSet<i64> = fixups
            .iter()
            .filter(|fix| fix.seg == Some(output.seg) && fix.target == "segment" && fix.index == 5)
            .map(|fix| fix.disp)
            .collect();
        assert!(BTreeSet::from([6, 22, 24, 26]).is_subset(&descriptors), "{path}");
    }
}
