//! Port of `tests/test_callsite_abi.py`.

use std::collections::BTreeSet;
use std::path::Path;

use super::caller_cleanup;
use crate::abi::runtime::{self, Contract, EVERY, Memory, Reg};
use crate::frontends::bc::declen::decode;
use crate::objectfile::module::{self, Module};
use crate::support::hash::IndexMap;

fn fpcsex() -> Module {
    module::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/omf/fpcsex-p-g2.obj")).unwrap().unwrap()
}

fn first_call(found: &Module) -> i64 {
    *found.calls.keys().next().unwrap()
}

fn gp() -> BTreeSet<Reg> {
    BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])
}

#[test]
fn test_external_pascal_interface_keeps_unknown_effects() {
    // Qrender UGL calls were refused solely for an undeclared interface.
    let mut found = fpcsex();
    let at = first_call(&found);
    found.calls.insert(at, "EXTERNAL_RENDER".to_owned());
    let rule = runtime::for_module(&found, None).unwrap()[&at].clone();
    assert_eq!(rule.inputs, Some(gp()));
    assert_eq!(rule.cleanup, None);
    assert_eq!(rule.clobbers, *EVERY);
    assert!(rule.reads == Memory::Any && rule.writes == Memory::Any);
    assert!(!rule.established && runtime::barrier(&rule));
    assert!(rule.evidence.contains("assumed"));
}

#[test]
fn test_runtime_helpers_are_not_assumed_to_use_a_language_abi() {
    for name in ["B$UNKNOWN", "b$unknown"] {
        let mut found = fpcsex();
        let at = first_call(&found);
        found.calls = IndexMap::from_iter([(at, name.to_owned())]);
        assert_eq!(runtime::for_module(&found, None).unwrap()[&at].inputs, None, "{name}");
    }
}

#[test]
fn test_caller_cleanup_requires_positive_stack_adjustment() {
    // A negative or unrelated ADD must not claim C argument cleanup.
    for (raw, expected) in [
        (&[0x83, 0xc4, 0x08][..], true),
        (&[0x81, 0xc4, 0x08, 0x00][..], true),
        (&[0x83, 0xc4, 0xfe][..], false),
        (&[0x83, 0xc0, 0x08][..], false),
        (&[0x83, 0xec, 0x08][..], false),
        (&[0x90][..], false),
    ] {
        assert_eq!(caller_cleanup(decode(raw, 0).as_ref()), expected, "{raw:02x?}");
    }
}

#[test]
fn test_explicit_external_contract_overrides_assumption() {
    let mut found = fpcsex();
    let at = first_call(&found);
    found.calls = IndexMap::from_iter([(at, "EXTERNAL_RENDER".to_owned())]);
    let explicit = Contract { inputs: Some(BTreeSet::new()), cleanup: Some(4), ..runtime::worst("EXTERNAL_RENDER") };
    let external = IndexMap::from_iter([(explicit.name.clone(), explicit.clone())]);
    assert_eq!(runtime::for_module(&found, Some(&external)).unwrap()[&at], explicit);
}
