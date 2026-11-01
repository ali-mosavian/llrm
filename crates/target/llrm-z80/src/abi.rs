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

/// The `z80` convention.
pub fn z80() -> llrm_target::calling::Convention {
    let calling = llrm_target::calling::Calling::parse(crate::CALLING).expect("calling.toml");
    calling.conventions.into_iter().find(|one| one.name == "z80").expect("convention z80")
}

/// The pair register that holds a byte register (`l` is half of `hl`): the
/// 16-bit view a word argument is copied through.
pub fn pair_of(byte: &str) -> RegId {
    let file = llrm_target::registers::parse(crate::REGISTERS).expect("registers.regs");
    let root = file.iter().find(|one| one.name == byte).map(|one| one.root.clone()).expect("a register of the file");
    register(&root)
}
