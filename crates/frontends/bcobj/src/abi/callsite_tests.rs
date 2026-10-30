//! Port of `tests/test_callsite_abi.py`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use llrm_qbruntime::{self as runtime, Contract, EVERY, Memory, Reg, barrier, contract};

use super::{caller_cleanup, for_module};
use crate::frontends::bc::declen::decode;
use crate::objectfile::module::{self, Module};
use crate::objectfile::omf::{self, Record};
use crate::support::hash::IndexMap;

fn fpcsex() -> Module {
    module::load(Path::new(env!("LLRM_ROOT")).join("tests/inputs/omf/fpcsex-p-g2.obj")).unwrap().unwrap()
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
    let rule = for_module(&found, None).unwrap()[&at].clone();
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
        assert_eq!(for_module(&found, None).unwrap()[&at].inputs, None, "{name}");
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
    assert_eq!(for_module(&found, Some(&external)).unwrap()[&at], explicit);
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("LLRM_ROOT")).join(name)
}

fn loaded(name: &str) -> Module {
    module::load(fixture(name)).unwrap().unwrap()
}

#[test]
fn test_event_stub_near_call_has_no_register_arguments() {
    // ADDRM /V refused at 0048 before its first statement could execute.
    for tag in ["p-evt", "v-evt"] {
        let found = loaded(&format!("{}/tests/inputs/omf/addrm-{tag}.obj", env!("LLRM_ROOT")));
        let routine = for_module(&found, None).unwrap()[&0x48].clone();
        assert_eq!(routine.inputs, Some(BTreeSet::new()), "{tag}");
        assert_eq!(routine.cleanup, Some(0), "{tag}");
        assert!(routine.enters_user_code && barrier(&routine), "{tag}");
        assert!(routine.reads == Memory::Any && routine.writes == Memory::Any, "{tag}");
    }
}

#[test]
fn test_changed_event_stub_remains_unknown() {
    // Only instruction bytes: a relocated field's addend is folded into its
    // fixup before recognition.
    let found = loaded(concat!(env!("LLRM_ROOT"), "/tests/inputs/omf/addrm-p-evt.obj"));
    let width = |loc: i64| match loc {
        omf::LOC_OFF16 => 2,
        omf::LOC_PTR32 => 4,
        _ => 2,
    };
    let relocated: BTreeSet<i64> = omf::fixups(&found.records)
        .into_iter()
        .filter(|one| one.seg == Some(found.seg))
        .flat_map(|one| one.offset..one.offset + width(one.loc))
        .collect();
    for at in (0x30..0x42).filter(|at| !relocated.contains(at)) {
        let mut changed = found.clone();
        changed.code[at as usize] ^= 1;
        assert!(!for_module(&changed, None).unwrap().contains_key(&0x48), "{at:#x}");
    }
}

#[test]
fn test_event_stub_requires_exact_relocation() {
    let found = loaded(concat!(env!("LLRM_ROOT"), "/tests/inputs/omf/addrm-p-evt.obj"));
    for field in [0x34, 0x3E] {
        let fixup = omf::fixups(&found.records)
            .into_iter()
            .find(|one| one.seg == Some(found.seg) && one.offset == field)
            .unwrap();
        // Give the fixup an explicit displacement of 1, adding the field
        // where the subrecord had none.
        let mut body = fixup.record.body.clone();
        match fixup.disp_pos {
            Some(at) => body[at..at + 2].copy_from_slice(&1u16.to_le_bytes()),
            None => {
                let mut sub = body[fixup.lo..fixup.hi].to_vec();
                sub[2] &= !0x04;
                sub.extend_from_slice(&1u16.to_le_bytes());
                body.splice(fixup.lo..fixup.hi, sub);
            }
        }
        let edited = Rc::new(Record { r#type: fixup.record.r#type, body, raw: None });
        let mut changed = found.clone();
        changed.records = found
            .records
            .iter()
            .map(|one| if Rc::ptr_eq(one, &fixup.record) { edited.clone() } else { one.clone() })
            .collect();
        let disp = omf::fixups(&changed.records)
            .into_iter()
            .find(|one| one.seg == Some(found.seg) && one.offset == field)
            .map(|one| one.disp);
        assert_eq!(disp, Some(1), "{field:#x}");
        assert!(!for_module(&changed, None).unwrap().contains_key(&0x48), "{field:#x}");
    }
}

/// Every `B$` routine a committed OMF fixture calls.
fn _runtime_targets() -> BTreeSet<String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(fixture(concat!(env!("LLRM_ROOT"), "/tests/inputs/omf")))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "obj"))
        .collect();
    paths.sort();
    let mut named = BTreeSet::new();
    for path in paths {
        let records = omf::parse(&std::fs::read(&path).unwrap()).unwrap();
        if let Some(found) = module::of(&records) {
            named.extend(found.calls.values().filter(|name| name.starts_with("B$")).cloned());
        }
    }
    named
}

#[test]
fn test_every_runtime_routine_the_corpus_calls_has_an_entry() {
    let names = _runtime_targets();
    assert!(!names.is_empty());
    for name in names {
        assert!(contract(Some(&name)).established, "{name}");
    }
}
