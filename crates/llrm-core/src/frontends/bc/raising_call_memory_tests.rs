//! Port of `tests/test_raising_call_memory.py`.

use super::*;
use crate::model::mir;
use crate::testing::{self, overlapping};

const PL_MOVE: &str = concat!(env!("LLRM_ROOT"), "/tests/fixtures/regressions/qrender-pl-move-v-g3.obj");

/// The B$FCMP calls in PL_MOVE's MDL_ANGLEMOD, raised with `contracts`.
fn angle_compares(found: &Module, contracts: Option<&mut IndexMap<i64, Contract>>) -> Vec<Op> {
    let raised = mir::bodies(found, &testing::partitioned(PL_MOVE), contracts, true, false).unwrap();
    let body = raised.values.iter().find(|(name, _)| name.ends_with(" MDL_ANGLEMOD")).unwrap().1.clone();
    body.blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| op.kind == Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$FCMP"))
        .cloned()
        .collect()
}

fn program(found: &Module) -> MemRef {
    MemRef::new(Some(Addr { index: found.program_data.unwrap(), ..Addr::new(Space::Segment, 6) }), 2)
}

fn fcmp() -> Contract {
    runtime::contract(Some("B$FCMP"))
}

#[test]
fn test_unproven_or_exceptional_call_keeps_unknown_effects() {
    let contracts = [
        Some(Contract { established: false, ..fcmp() }),
        Some(Contract { enters_user_code: true, ..fcmp() }),
        Some(Contract { error_handling: true, ..fcmp() }),
        Some(Contract { raises_error: true, ..fcmp() }),
        Some(Contract { control: Control::Unknown, ..fcmp() }),
        None,
    ];
    for contract in &contracts {
        let escaped: Reach = (5, BTreeSet::new());
        assert_eq!(reachable(contract.as_ref(), Memory::None, Some(&escaped), true), None, "{contract:?}");
    }
}

#[test]
fn test_access_reach_preserves_escape_and_unknown_information() {
    for access in Memory::ALL {
        let escaped: Reach = (5, BTreeSet::from([(5, 6)]));
        let result = reachable(Some(&fcmp()), access, Some(&escaped), true);
        let expected = if access == Memory::Any {
            None
        } else if access <= Memory::Arguments {
            Some((5, BTreeSet::new()))
        } else {
            Some(escaped.clone())
        };
        assert_eq!(result, expected, "{access:?}");
        assert_eq!(reachable(Some(&fcmp()), access, None, true), None);
    }
}

#[test]
fn test_error_capability_is_not_an_unconditional_callback() {
    let routine = runtime::contract(Some("B$PSSD"));
    assert!(routine.raises_error);
    let escaped: Reach = (5, BTreeSet::from([(5, 6)]));
    assert_eq!(reachable(Some(&routine), routine.writes, Some(&escaped), false), Some(escaped.clone()));
    assert_eq!(reachable(Some(&routine), routine.writes, Some(&escaped), true), None);
}

#[test]
fn test_angle_compare_does_not_clobber_program_data() {
    let found = testing::loaded(PL_MOVE).unwrap();
    assert!(found.program_data.is_some());
    let calls = angle_compares(&found, None);
    assert!(!calls.is_empty());
    for op in &calls {
        assert!(op.memory_complete, "the selected read/write contract is the complete callee footprint");
        let program = program(&found);
        let scratch = MemRef::new(Some(Addr::new(Space::Literal, 0)), 2);
        for effects in [&op.stores, &op.loads] {
            assert!(!effects.is_empty());
            assert!(!effects.iter().any(|one| overlapping(&program, one, None)));
            assert!(effects.iter().any(|one| overlapping(&scratch, one, None)));
        }
    }
}

#[test]
fn test_read_and_write_contracts_are_independent() {
    for (reads, writes) in [(Memory::Any, Memory::None), (Memory::None, Memory::Any)] {
        let found = testing::loaded(PL_MOVE).unwrap();
        assert!(found.program_data.is_some());
        let mut contracts: IndexMap<i64, Contract> = runtime::for_module(&found, None)
            .unwrap()
            .into_iter()
            .map(|(at, contract)| {
                if contract.name == "B$FCMP" { (at, Contract { reads, writes, ..contract }) } else { (at, contract) }
            })
            .collect();
        let calls = angle_compares(&found, Some(&mut contracts));
        assert!(!calls.is_empty());
        let program = program(&found);
        for op in &calls {
            for (access, effects) in [(reads, &op.loads), (writes, &op.stores)] {
                let hit = effects.iter().any(|one| overlapping(&program, one, None));
                assert_eq!(hit, access == Memory::Any, "{reads:?} {writes:?}");
            }
        }
    }
}

#[test]
fn test_selected_unknown_contract_keeps_program_data_live() {
    let found = testing::loaded(PL_MOVE).unwrap();
    assert!(found.program_data.is_some());
    let mut contracts: IndexMap<i64, Contract> = runtime::for_module(&found, None)
        .unwrap()
        .into_iter()
        .map(|(at, contract)| {
            if contract.name == "B$FCMP" { (at, runtime::worst(&contract.name)) } else { (at, contract) }
        })
        .collect();
    let calls = angle_compares(&found, Some(&mut contracts));
    assert!(!calls.is_empty());
    assert!(!calls.iter().any(|op| op.memory_complete));
    assert!(calls.iter().all(|op| op.loads.iter().chain(&op.stores).all(|one| one.beyond.is_none())));
}

/// DIVMOD's handler writes `caught`; treating it as writing `a` left every constant divide live.
#[test]
fn test_resumable_handler_summary_invalidates_only_cells_the_handler_modifies() {
    let path = concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/divmod-p-g2.obj");
    let found = testing::loaded(path).unwrap();
    let raised = testing::raised(path);
    let ops = |wanted: &str| -> Vec<Op> {
        let body = &raised.values.iter().find(|(name, _)| name == wanted).unwrap().1;
        body.blocks.iter().flat_map(|block| block.ops.clone()).collect()
    };
    let (main, handler) = (ops("main (main)"), ops("error-handler error handler"));
    let a = main.iter().find(|op| op.at == 0x3A && !op.stores.is_empty()).unwrap().stores[0].clone();
    let caught = handler.iter().find(|op| op.kind == Kind::Store).unwrap().stores[0].clone();
    let printing = main
        .iter()
        .find(|op| op.kind == Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$PSSD"))
        .unwrap();
    assert!(printing.memory_complete);
    assert!(printing.stores.iter().any(|one| overlapping(&caught, one, Some(&found.dgroup))));
    assert!(!printing.stores.iter().any(|one| overlapping(&a, one, Some(&found.dgroup))));
}
