//! The 6502 C ABI: `machines/calling.toml` over `registers.regs`.

use llrm_lir::registers::RegId;
use llrm_target::calling::{Calling, Convention};

use crate::REGISTER_INFO;

/// The `mos` convention: arguments in a, x, rc2.., results in A:X. An integer
/// word takes the first free pair of `wide_pairs`, A:X first; a pointer the
/// first free of `pointer_pairs`, which has no A:X.
pub fn mos() -> Convention {
    let calling = Calling::parse(crate::CALLING).expect("calling.toml");
    calling.conventions.into_iter().find(|one| one.name == "mos").expect("convention mos")
}

/// LLVM's `fastcc`: the calling convention of a function private to its module.
pub const FASTCC: u32 = 8;

/// The convention of a call or a definition in LLVM calling convention `cc`:
/// `calling.toml`'s `private` row for `fastcc` (a function nothing outside the
/// module calls), the default row for the rest.
pub fn convention_of(cc: u32) -> Convention {
    let calling = Calling::parse(crate::CALLING).expect("calling.toml");
    let name = match (cc, &calling.private) {
        (FASTCC, Some(private)) => private.as_str(),
        _ => "mos",
    };
    calling.conventions.into_iter().find(|one| one.name == name).unwrap_or_else(|| panic!("convention {name}"))
}

/// A register `calling.toml` names.
pub fn register(name: &str) -> RegId {
    REGISTER_INFO
        .entries()
        .find_map(|(id, entry)| (entry.name == name).then_some(id))
        .unwrap_or_else(|| panic!("calling.toml names no register {name}"))
}

/// The pair register that holds a zero-page byte (`rc4` is half of `rs2`): the
/// 16-bit view a word argument is copied through.
pub fn pair_of(byte: &str) -> RegId {
    let file = llrm_target::registers::parse(crate::REGISTERS).expect("registers.regs");
    let root = file.iter().find(|one| one.name == byte).map(|one| one.root.clone()).expect("a register of the file");
    register(&root)
}
