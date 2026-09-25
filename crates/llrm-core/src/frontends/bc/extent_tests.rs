//! Port of `tests/test_extent.py`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::*;
use crate::abi::handlers::error_entries;
use crate::frontends::bc::blocks::{code_map, statement_table};
use crate::testing;
use crate::wholeseg::Emission;
use crate::objectfile::module::tests::{bare, fixtures, loaded, objects};

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("LLRM_ROOT")).join(relative.to_lowercase())
}

fn bodies(found: &Partition, kind: BodyKind) -> Vec<&Body> {
    found.bodies.iter().filter(|body| body.kind == kind).collect()
}

fn extents(name: &str) -> (Module, Partition) {
    let found = loaded(fixtures().join(name)).unwrap();
    let partitioned = partition(&found).unwrap();
    (found, partitioned)
}

#[test]
fn test_registered_error_handler_has_independent_entry_and_emits() {
    for tag in ["p-g2", "q-O", "v-g3", "p-evt", "q-evt", "v-evt"] {
        let found = loaded(fixture(&format!("{}/tests/fixtures/omf/divmod-{tag}.obj", env!("LLRM_ROOT")))).unwrap();
        let result = partition(&found).unwrap();
        assert!(result.complete(), "{tag}");
        assert!(result.bodies.iter().any(|body| body.kind.value() == "error-handler"), "{tag}");
        let emitted = testing::emitted(&testing::data(fixture(&format!("{}/tests/fixtures/omf/divmod-{tag}.obj", env!("LLRM_ROOT")))));
        assert_eq!(emitted.outcome, Emission::Lir, "{tag}: {}", emitted.reason);
    }
}

/// ERRENT must print 11 then DONE through ON ERROR / RESUME NEXT, not lose its handler.
#[test]
fn test_error_resume_fixture_keeps_registered_entry() {
    for tag in ["p-g2", "q-O", "v-g3", "p-evt", "q-evt", "v-evt"] {
        let emitted =
            testing::emitted_with(&testing::data(fixture(&format!("{}/tests/fixtures/regressions/errent-{tag}.obj", env!("LLRM_ROOT")))), true, false);
        assert_eq!(emitted.outcome, Emission::Lir, "{tag}: {}", emitted.reason);
        let found = testing::loaded_bytes(&emitted.data).unwrap();
        let result = partition(&found).unwrap();
        assert!(result.complete(), "{tag}");
        let handlers: BTreeSet<i64> = bodies(&result, BodyKind::ErrorHandler)
            .iter()
            .map(|body| i64::try_from(body.seed).unwrap())
            .collect();
        assert_eq!(error_entries(&found), handlers, "{tag}");
        assert!(!error_entries(&found).is_empty(), "{tag}");
    }
}

#[test]
fn test_timer_handler_has_its_own_entry() {
    for (tag, entry) in [("p-evt", 0xFA), ("v-evt", 0xF0)] {
        let found = loaded(fixture(&format!("{}/tests/fixtures/regressions/evtrap-{tag}.obj", env!("LLRM_ROOT")))).unwrap();
        let result = partition(&found).unwrap();
        let handler = result.bodies.iter().find(|body| body.seed == entry).unwrap();
        assert_eq!(handler.kind.value(), "event-handler", "{tag}");
        assert!(
            !bodies(&result, BodyKind::Main).iter().flat_map(|body| &body.ranges).any(|&(lo, hi)| lo <= entry && entry < hi),
            "{tag}"
        );
    }
}

#[test]
fn test_empty_statement_table_is_data_not_a_handler_instruction() {
    let found = loaded(fixture(concat!(env!("LLRM_ROOT"), "/tests/fixtures/regressions/evtrap-v-evt.obj"))).unwrap();
    let mapped = code_map(&found).unwrap();
    assert!(mapped.tables.contains(&(0x116, 0x118)));
    assert!(!mapped.starts.contains(&0x116));
    assert!(partition(&found).unwrap().complete());
}

/// ADDRM VBDOS refused OF_STA at 00bc when layout omitted trailing data.
#[test]
fn test_emission_preserves_the_empty_statement_table() {
    let result = testing::emitted_lir(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/addrm-v-g3.obj"));
    let found = testing::loaded_bytes(&result.data).unwrap();
    assert!(statement_table(&found).is_some());
}

/// DIVMOD event output had unmapped bytes after its RESUME table.
#[test]
fn test_emitted_statement_table_does_not_hide_code() {
    for tag in ["p-evt", "v-evt"] {
        let result = testing::emitted_lir(format!("{}/tests/fixtures/omf/divmod-{tag}.obj", env!("LLRM_ROOT")));
        let found = testing::loaded_bytes(&result.data).unwrap();
        let mapped = code_map(&found);
        assert!(mapped.is_ok(), "{tag}: {mapped:?}");
    }
}

#[test]
fn test_every_fixture_partitions_completely() {
    for obj in objects() {
        let found = loaded(&obj).unwrap();
        let result = partition(&found).unwrap_or_else(|why| panic!("{obj:?}: {why}"));
        assert!(result.complete(), "{obj:?}: {:?} {:?}", result.unexplained, result.conflicts);
    }
}

#[test]
fn test_procs_v_g3_bodies_match_the_measured_layout() {
    let (_found, result) = extents("procs-v-g3.obj");
    assert!(result.complete());

    let main = bodies(&result, BodyKind::Main);
    assert_eq!(main.len(), 1);
    assert_eq!(main[0].ranges, [(0x30, 0xEA), (0x11B, 0x11E), (0x14C, 0x153)]);

    let procs: std::collections::BTreeMap<Option<&str>, Vec<(usize, usize)>> = bodies(&result, BodyKind::Procedure)
        .into_iter()
        .map(|body| (body.name.as_deref(), body.ranges.clone()))
        .collect();
    assert_eq!(
        procs,
        [(Some("TWICE"), vec![(0xEA, 0x11B)]), (Some("REPORT"), vec![(0x11E, 0x14C)])].into_iter().collect()
    );
}

#[test]
fn test_procedure_ptot_has_no_runtime_call_but_still_partitions() {
    let (found, result) = extents("procs-p-ot.obj");
    assert!(!found.calls.values().any(|name| name == "B$ENRA" || name == "B$EXSA"));
    assert!(result.complete());
    let names: BTreeSet<Option<&str>> =
        bodies(&result, BodyKind::Procedure).into_iter().map(|body| body.name.as_deref()).collect();
    assert_eq!(names, BTreeSet::from([Some("TWICE"), Some("REPORT")]));
}

#[test]
fn test_event_stub_is_its_own_body_not_a_gap() {
    let (_found, result) = extents("arith-v-evt.obj");
    assert!(result.complete());
    let stub = bodies(&result, BodyKind::EventStub);
    assert_eq!(stub.len(), 1);
    assert_eq!(stub[0].ranges, [(0x32, 0x42)]);
}

#[test]
fn test_qb45_under_evt_has_no_stub_body() {
    let (_found, result) = extents("arith-q-evt.obj");
    assert!(result.complete());
    assert!(bodies(&result, BodyKind::EventStub).is_empty());
}

#[test]
fn test_the_resume_map_fallthrough_does_not_leak_module_targets() {
    let (found, result) = extents("divmod-v-g3.obj");
    assert!(found.publics.is_empty());
    assert!(result.complete());
    let main = bodies(&result, BodyKind::Main);
    let handler = bodies(&result, BodyKind::ErrorHandler);
    assert_eq!((main.len(), handler.len()), (1, 1));
    assert_eq!(main[0].ranges.last().unwrap().1, handler[0].seed);
    assert_eq!(handler[0].ranges.last().unwrap().1 as i64, found.end);
}

#[test]
fn test_a_module_with_no_header_is_refused_not_guessed() {
    let found = loaded(fixtures().join("procs-v-g3.obj")).unwrap();
    let headerless = bare(&found, found.code[0x30..].to_vec(), 0, found.code.len() as i64 - 0x30);
    assert!(partition(&headerless).is_err());
}
