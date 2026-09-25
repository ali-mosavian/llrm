//! Port of `tests/test_contract_profile.py`.
//!
//! `test_stale_dependency_does_not_overwrite_cli_output` lives in `rewrite.rs`.
//! Skipped, spying through a `Mock`:
//! `test_stage_dumps_use_the_same_profile_and_native_mode`,
//! `test_cli_forwards_profile_and_marks_its_identity`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{ProfileError, load};
use crate::abi::runtime::{self, Contract, Control, EVERY, Memory, Reg};
use crate::support::hash::IndexMap;
use crate::support::pyjson::{self, Json};

fn hexdigest(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn text(value: &str) -> Json {
    Json::Str(value.to_owned())
}

fn dict<const N: usize>(items: [(&str, Json); N]) -> Json {
    Json::Dict(items.into_iter().map(|(key, value)| (key.to_owned(), value)).collect())
}

/// The `declaration` fixture: two artifacts and a profile naming one symbol.
fn declaration(directory: &Path) -> (PathBuf, Json) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read(root.join("tests/fixtures/regressions/qrender-main-v-g3.obj")).unwrap();
    let dependency = std::fs::read(root.join("tests/fixtures/omf/hotlop-p-g2.obj")).unwrap();
    std::fs::write(directory.join("main.obj"), &source).unwrap();
    std::fs::write(directory.join("dependency.obj"), &dependency).unwrap();
    let document = dict([
        ("version", Json::Int(1)),
        ("artifacts", dict([("main.obj", text(&hexdigest(&source))), ("dependency.obj", text(&hexdigest(&dependency)))])),
        (
            "contracts",
            dict([(
                "HOST_SHUTDOWN",
                dict([
                    ("defined_in", text("main.obj")),
                    ("inputs", Json::List(["ax", "bx", "cx", "dx", "si", "di"].into_iter().map(text).collect())),
                    (
                        "evidence",
                        text("B$ENRA precedes flag reads; all six GP inputs and unknown effects retained."),
                    ),
                ]),
            )]),
        ),
    ]);
    let path = directory.join("contracts.json");
    write(&path, &document, None);
    (path, document)
}

/// `path.write_text(json.dumps(document, indent=indent))`.
fn write(path: &Path, document: &Json, indent: Option<usize>) {
    std::fs::write(path, pyjson::dumps(document, indent, None, false)).unwrap();
}

fn host_shutdown(document: &mut Json) -> &mut IndexMap<String, Json> {
    let Json::Dict(top) = document else { unreachable!() };
    let Json::Dict(contracts) = &mut top["contracts"] else { unreachable!() };
    let Json::Dict(row) = &mut contracts["HOST_SHUTDOWN"] else { unreachable!() };
    row
}

#[test]
fn test_audited_profile_keeps_unknown_effects() {
    let directory = tempfile::tempdir().unwrap();
    let (path, _) = declaration(directory.path());
    let loaded = load(&path, None).unwrap();
    let [rule] = <[Contract; 1]>::try_from(loaded.rules).unwrap();
    assert_eq!(
        rule,
        Contract {
            inputs: Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
            evidence: rule.evidence.clone(),
            ..runtime::worst("HOST_SHUTDOWN")
        }
    );
    assert_eq!(rule.control, Control::Unknown);
    assert_eq!(rule.cleanup, None);
    assert!(rule.reads == Memory::Any && rule.writes == Memory::Any);
    assert_eq!(rule.clobbers, *EVERY);
}

#[test]
fn test_invalid_profile_refuses() {
    let cases = [
        ("defined_in", text("dependency.obj")),
        ("inputs", Json::List(vec![text("eax")])),
        ("inputs", Json::List(vec![text("ax"), text("ax")])),
        ("cleanup", Json::Int(-2)),
        ("cleanup", Json::Int(3)),
        ("cleanup", Json::Bool(true)),
        ("evidence", text("")),
        ("clobbers", Json::List(Vec::new())),
    ];
    for (field, value) in cases {
        let directory = tempfile::tempdir().unwrap();
        let (path, mut document) = declaration(directory.path());
        host_shutdown(&mut document).insert(field.to_owned(), value.clone());
        write(&path, &document, None);
        assert!(matches!(load(&path, None), Err(ProfileError::ValueError(_))), "{field}={value:?}");
    }
}

#[test]
fn test_profile_fingerprint_is_order_independent() {
    let directory = tempfile::tempdir().unwrap();
    let (path, document) = declaration(directory.path());
    let first = load(&path, None).unwrap();
    let Json::Dict(top) = document else { unreachable!() };
    let reversed = Json::Dict(top.into_iter().rev().collect());
    write(&path, &reversed, Some(4));
    assert_eq!(load(&path, None).unwrap(), first);
}

#[test]
fn test_duplicate_json_keys_are_not_silently_replaced() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("duplicate.json");
    std::fs::write(&path, r#"{"version":1,"version":2}"#).unwrap();
    match load(&path, None) {
        Err(ProfileError::ValueError(text)) => assert!(text.contains("duplicate"), "{text}"),
        other => panic!("{other:?}"),
    }
}
