//! The m68k C ABI: `machines/calling.toml` over `registers.regs`.

use llrm_lir::registers::RegId;
use llrm_target::calling::{Calling, Convention};

use crate::REGISTER_INFO;

const CALLING: &str = include_str!("machines/calling.toml");

/// The C convention (CC_M68k_C / RetCC_M68k_C).
pub fn c() -> Convention {
    let calling = Calling::parse(CALLING).expect("calling.toml");
    calling.conventions.into_iter().find(|one| one.name == "c").expect("convention c")
}

/// A register `calling.toml` names.
pub fn register(name: &str) -> RegId {
    REGISTER_INFO
        .entries()
        .find_map(|(id, entry)| (entry.name == name).then_some(id))
        .unwrap_or_else(|| panic!("calling.toml names no register {name}"))
}
