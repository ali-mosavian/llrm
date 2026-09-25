//! Port of `tests/test_blocks.py`.

use std::path::Path;

use iced_x86::Mnemonic;

use super::*;
use crate::objectfile::module::tests::{bare, fixtures, loaded, objects};

/// helpers' `hx`.
fn hx(s: &str) -> Vec<u8> {
    let digits: String = s.split_whitespace().collect();
    (0..digits.len()).step_by(2).map(|at| u8::from_str_radix(&digits[at..at + 2], 16).unwrap()).collect()
}

#[test]
fn test_what_ends_a_block() {
    for (enc, ends) in [
        ("74 02", Ends::Conditional),
        ("E2 02", Ends::Conditional),
        ("EB 02", Ends::Jump),
        ("E9 02 00", Ends::Jump),
        ("C3", Ends::Return),
        ("CB", Ends::Return),
        ("CA 04 00", Ends::Return),
        ("EA 00 00 00 00", Ends::Leaves),
        ("FF 26 00 00", Ends::Indirect),
        ("FF 2E 00 00", Ends::Indirect),
        ("9A 00 00 00 00", Ends::FallsThrough),
        ("E8 02 00", Ends::FallsThrough),
        ("FF 16 00 00", Ends::FallsThrough),
        ("FF 1E 00 00", Ends::FallsThrough),
        ("CD 21", Ends::FallsThrough),
    ] {
        let insn = decode(&hx(enc), 0).unwrap();
        assert_eq!(terminator(&insn, None), ends, "{enc}");
    }
}

#[test]
fn test_runtime_return_does_not_fall_into_the_next_statement() {
    for tag in ["p-evt", "v-evt"] {
        let path = Path::new(env!("LLRM_ROOT")).join(format!("{}/tests/fixtures/omf/addrm-{tag}.obj", env!("LLRM_ROOT")));
        let mut found = loaded(path).unwrap();
        found.code = hx("9a 00 00 00 00 90 c3");
        found.start = 0;
        found.end = 7;
        found.calls = [(0, "B$RETA".to_owned())].into_iter().collect();
        found.targets = BTreeSet::new();
        found.publics = BTreeSet::new();
        let mapped = walk(&found, 0, &BTreeSet::new()).unwrap();
        assert_eq!(mapped.starts, BTreeSet::from([0]), "{tag}");
        let body = partition(&found, &mapped);
        assert_eq!(body.len(), 1, "{tag}");
        assert_eq!(body[0].ends, Ends::Leaves, "{tag}");
        assert!(body[0].succ.is_empty(), "{tag}");
    }
}

#[test]
fn test_every_module_is_mapped() {
    for obj in objects() {
        let found = loaded(&obj).unwrap();
        if let Err(why) = code_map(&found) {
            panic!("{obj:?}: {why}");
        }
    }
}

#[test]
fn test_the_code_begins_where_the_runtime_says_it_does() {
    for obj in objects() {
        let found = loaded(&obj).unwrap();
        assert!(has_header(&found), "{obj:?}: every object BC wrote carries a module header");
        let mapped = code_map(&found).unwrap();
        assert_eq!(mapped.starts.first(), Some(&ENTRY), "{obj:?}");
    }
}

#[test]
fn test_only_an_event_build_has_a_stub_and_it_sits_after_the_jump() {
    for obj in objects() {
        let found = loaded(&obj).unwrap();
        let stub = event_stub(&found);
        let mapped = code_map(&found).unwrap();
        match stub {
            None => assert_ne!(&found.code[ENTRY..ENTRY + 2], b"\xeb\x10", "{obj:?}"),
            Some(stub) => {
                assert_eq!(stub, ENTRY + 2, "{obj:?}");
                assert!(mapped.starts.contains(&stub), "{obj:?}: seeded, or nothing reaches it");
            }
        }
    }
}

/// Rebuilt ADDRM hid 0035..0046 from the assembly dump and target scorer.
#[test]
fn test_rebuilt_event_code_is_fully_visible() {
    for tag in ["p-evt", "v-evt"] {
        let result = crate::support::testing::emitted(&crate::support::testing::data(format!("{}/tests/fixtures/omf/addrm-{tag}.obj", env!("LLRM_ROOT"))));
        assert_eq!(result.outcome, crate::wholeseg::Emission::Lir, "{tag}");
        let found = crate::support::testing::loaded_bytes(&result.data).unwrap();
        let code = instructions(&found).unwrap();
        let stub = event_stub(&found).unwrap();
        assert!(code.iter().any(|one| one.at == stub), "{tag}");
    }
}

#[test]
fn test_the_instruction_stream_tiles() {
    for obj in objects() {
        let found = loaded(&obj).unwrap();
        let reached = instructions(&found).unwrap();
        for pair in reached.windows(2) {
            assert!(pair[0].end() <= pair[1].at, "{obj:?} at {:#x}", pair[1].at);
        }
    }
}

#[test]
fn test_an_on_goto_table_is_found_and_is_not_code() {
    let found = loaded(fixtures().join("jumptable.obj")).unwrap();
    let mapped = code_map(&found).unwrap();
    assert_eq!(mapped.tables, [(0x3F, 0x46), (0xEA, 0xEC)]);
    let (lo, hi) = mapped.tables[0];
    assert_eq!(found.code[lo], 3, "the count byte says three labels");
    assert!(!mapped.starts.iter().any(|&at| lo <= at && at < hi), "no instruction begins inside a table");
    assert_eq!(found.targets.iter().copied().collect::<Vec<_>>(), [0x46, 0x52, 0x5E, 0xEA]);
}

#[test]
fn test_on_goto_comes_back_past_the_table() {
    let found = loaded(fixtures().join("jumps-v-g3.obj")).unwrap();
    let mapped = code_map(&found).unwrap();
    assert!(!mapped.tables.is_empty());
    let statements = statement_table(&found);
    let inline: Vec<_> = mapped.tables.iter().filter(|&&table| Some(table) != statements).collect();
    assert!(!inline.is_empty());
    assert!(inline.iter().all(|(_lo, hi)| mapped.leaders.contains(hi)));
}

#[test]
fn test_padding_is_inert_but_a_branch_is_not() {
    let found = loaded(fixtures().join("arith-v-g3.obj")).unwrap();
    assert_eq!(benign(&found, (0, 0)), Some(Vec::new()));
    let padded = bare(&found, vec![PAD, PAD], 0, 2);
    assert_eq!(benign(&padded, (0, 2)), Some(Vec::new()));
    let with_branch = bare(&found, hx("EB 00"), 0, 2);
    let mnemonics: Vec<Mnemonic> =
        benign(&with_branch, (0, 2)).unwrap().iter().map(|one| one.insn.mnemonic()).collect();
    assert_eq!(mnemonics, [Mnemonic::Jmp]);
    assert_eq!(benign(&with_branch, (0, 1)), None, "an instruction running past the gap is not padding");
}
