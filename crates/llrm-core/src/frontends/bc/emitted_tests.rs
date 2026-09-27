//! Machine facts of emitted objects: tests of `llrm-bcmachine` that need the
//! whole pipeline, moved here from its `blocks_tests.rs` and `extent_tests.rs`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::abi::handlers::error_entries;
use crate::frontends::bc::blocks::{code_map, event_stub, instructions, statement_table};
use crate::frontends::bc::extent::{Body, BodyKind, Partition, partition};
use crate::objectfile::testing::loaded;
use crate::testing;
use crate::wholeseg::Emission;

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("LLRM_ROOT")).join(relative.to_lowercase())
}

fn bodies(found: &Partition, kind: BodyKind) -> Vec<&Body> {
    found.bodies.iter().filter(|body| body.kind == kind).collect()
}

/// Rebuilt ADDRM hid 0035..0046 from the assembly dump and target scorer.
#[test]
fn test_rebuilt_event_code_is_fully_visible() {
    for tag in ["p-evt", "v-evt"] {
        let result = crate::testing::emitted(&crate::testing::data(format!("{}/tests/fixtures/omf/addrm-{tag}.obj", env!("LLRM_ROOT"))));
        assert_eq!(result.outcome, crate::wholeseg::Emission::Lir, "{tag}");
        let found = crate::testing::loaded_bytes(&result.data).unwrap();
        let code = instructions(&found).unwrap();
        let stub = event_stub(&found).unwrap();
        assert!(code.iter().any(|one| one.at == stub), "{tag}");
    }
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
