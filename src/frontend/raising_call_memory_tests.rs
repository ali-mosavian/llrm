//! Port of `tests/test_raising_call_memory.py`.
//!
//! Skipped, needing `mir.bodies`:
//! `test_angle_compare_does_not_clobber_program_data`,
//! `test_read_and_write_contracts_are_independent`,
//! `test_selected_unknown_contract_keeps_program_data_live`,
//! `test_resumable_handler_summary_invalidates_only_cells_the_handler_modifies`.

use super::*;

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
