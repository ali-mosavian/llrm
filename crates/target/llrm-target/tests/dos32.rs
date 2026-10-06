//! The code32 platform description is read by the parser that will own it, so
//! it cannot drift from the schema unnoticed. Moves into `llrm-x86-code32`
//! when that crate becomes a member.

use llrm_target::machine::{Addressing, Machine};

const DOS32: &str = include_str!("../../x86-code32/src/machines/dos32.toml");

#[test]
fn test_dos32_is_a_flat_machine_with_the_pc_ports() {
    let dos32 = Machine::parse(&format!("{DOS32}{}", llrm_target::PC_PORTS), &["486"]).expect("dos32.toml parses");
    assert_eq!(dos32.addressing, Addressing::Flat);
    assert!(dos32.segments.is_none());
    assert!(!dos32.ports.is_empty());
}
