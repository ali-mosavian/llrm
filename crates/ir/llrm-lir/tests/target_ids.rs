//! A second target numbers its own register file: ids are not limited to the
//! registers iced names, which had no way to hold an AArch64 register.

use llrm_lir::registers::{RegId, RegSet};

#[test]
fn an_id_a_target_numbers_survives_a_set_and_its_index() {
    let last = RegId::from_raw(255);
    let mut set = RegSet::new();
    set.insert(last);
    assert!(set.contains(&last));
    assert_eq!(last.index(), 255);
    assert_eq!(set.iter().collect::<Vec<_>>(), vec![last]);
}

#[test]
fn an_x86_id_is_still_the_number_iced_gives() {
    assert_eq!(RegId::AX.index(), iced_x86::Register::AX as usize);
    assert_eq!(RegId::AX.iced(), iced_x86::Register::AX);
}
