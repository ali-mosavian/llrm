//! The m68k C ABI: `machines/calling.toml` over `registers.regs`.

use llrm_lir::registers::RegId;
use llrm_target::calling::{Calling, Convention};

use crate::REGISTER_INFO;

const CALLING: &str = include_str!("machines/calling.toml");

static PARSED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How many times `calling.toml` has been parsed: once per process, not once
/// per call lowering (3.2 M of grep's 132 M compile instructions at -O2).
pub fn parses() -> usize {
    PARSED.load(std::sync::atomic::Ordering::Relaxed)
}

/// The C convention (CC_M68k_C / RetCC_M68k_C).
pub fn c() -> Convention {
    static C: std::sync::OnceLock<Convention> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        PARSED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let calling = Calling::parse(CALLING).expect("calling.toml");
        calling.conventions.into_iter().find(|one| one.name == "c").expect("convention c")
    })
    .clone()
}

/// A register `calling.toml` names.
pub fn register(name: &str) -> RegId {
    REGISTER_INFO
        .entries()
        .find_map(|(id, entry)| (entry.name == name).then_some(id))
        .unwrap_or_else(|| panic!("calling.toml names no register {name}"))
}
