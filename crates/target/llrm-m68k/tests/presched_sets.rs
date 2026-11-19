//! The pre-RA scheduler counted pressure per class: Dr8, Dr16 and Dr32 were
//! three sets of the same eight data registers, and an Xr32 value counted
//! against neither data nor address. It underestimated pressure and grew five
//! larger files by 66 to 96 bytes of spill code.

use llrm_m68k::mfunc::Class;
use llrm_m68k::regalloc::M68kRegisters;
use llrm_spike_mf::tolir::{pressure_sets, sets_of};

const ALL: [Class; 5] = [Class::Dr8, Class::Dr16, Class::Dr32, Class::Ar32, Class::Xr32];

#[test]
fn classes_over_the_same_registers_are_one_pressure_set() {
    let sets = pressure_sets::<M68kRegisters>(&ALL);
    assert_eq!(sets.len(), 3, "data, address and the whole file: {sets:?}");
    let of = |class| sets_of::<M68kRegisters>(&sets, class);
    assert_eq!(of(Class::Dr8), of(Class::Dr32));
    assert_eq!(of(Class::Dr16), of(Class::Dr32));
}

#[test]
fn a_value_counts_in_every_set_it_shares_a_register_with() {
    let sets = pressure_sets::<M68kRegisters>(&ALL);
    let of = |class| sets_of::<M68kRegisters>(&sets, class).len();
    assert_eq!(of(Class::Dr32), 2, "data and the whole file");
    assert_eq!(of(Class::Ar32), 2, "address and the whole file");
    assert_eq!(of(Class::Xr32), 3, "data, address and the whole file");
}
