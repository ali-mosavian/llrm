//! The 6502 C ABI: `machines/calling.toml` over `registers.regs`.

use llrm_lir::registers::RegId;
use llrm_target::calling::{Calling, Convention};

use crate::REGISTER_INFO;

thread_local! {
    static FIRST_WORD_IN_AX: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `f` with the first word argument passed in A:X (`wide_pairs` of
/// `calling.toml` lists `a`,`x` first) or, off, in the cells like the others:
/// the staging of the ABI change, until the day the toml's pair is the only
/// word.
pub fn with_first_word_in_ax<R>(
    on: bool,
    f: impl FnOnce() -> R,
) -> R {
    let before = FIRST_WORD_IN_AX.with(|flag| flag.replace(on));
    let result = f();
    FIRST_WORD_IN_AX.with(|flag| flag.set(before));
    result
}

/// The `mos` convention: arguments in a, x, rc2.., results in A:X. A word
/// takes the first free pair of `wide_pairs`; while the change is staged the
/// pair of the byte registers is left out unless asked for.
pub fn mos() -> Convention {
    let calling = Calling::parse(crate::CALLING).expect("calling.toml");
    let mut convention = calling.conventions.into_iter().find(|one| one.name == "mos").expect("convention mos");
    if !FIRST_WORD_IN_AX.with(std::cell::Cell::get) {
        convention.wide_pairs.retain(|pair| pair[0] != "a");
    }
    convention
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
