//! The Z80 C ABI: `machines/calling.toml` over `registers.regs`.

use llrm_lir::registers::RegId;

use crate::REGISTER_INFO;

/// A register `calling.toml` names.
pub fn register(name: &str) -> RegId {
    REGISTER_INFO
        .entries()
        .find_map(|(id, entry)| (entry.name == name).then_some(id))
        .unwrap_or_else(|| panic!("calling.toml names no register {name}"))
}
