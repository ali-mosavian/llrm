//! The 6502 C ABI: `machines/calling.toml` over `registers.regs`.

use llrm_lir::registers::RegId;
use llrm_target::calling::{Calling, Convention};

use crate::REGISTER_INFO;

/// The `mos` convention: arguments in a, x, rc2.., results in A:X. An integer
/// word takes the first free pair of `wide_pairs`, A:X first; a pointer the
/// first free of `pointer_pairs`, which has no A:X.
pub fn mos() -> Convention {
    convention_named("mos")
}

/// `calling.toml`, parsed once.
fn calling() -> &'static Calling {
    static PARSED: std::sync::OnceLock<Calling> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| {
        PARSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Calling::parse(crate::CALLING).expect("calling.toml")
    })
}

static PARSES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How many times `calling.toml` was parsed in this process (a test asks for
/// one).
pub fn parses() -> usize {
    PARSES.load(std::sync::atomic::Ordering::Relaxed)
}

fn convention_named(name: &str) -> Convention {
    calling().conventions.iter().find(|one| one.name == name).unwrap_or_else(|| panic!("convention {name}")).clone()
}

/// LLVM's `fastcc`: the calling convention of a function private to its module.
pub const FASTCC: u32 = 8;

/// The convention of a call or a definition in LLVM calling convention `cc`:
/// `calling.toml`'s `private` row for `fastcc` (a function nothing outside the
/// module calls), the default row for the rest.
pub fn convention_of(cc: u32) -> Convention {
    match (cc, &calling().private) {
        (FASTCC, Some(private)) if PRIVATE_ROW.get() => convention_named(private),
        _ => mos(),
    }
}

thread_local! {
    static PRIVATE_ROW: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Whether the platform has the zero-page cells the `private` row names
/// (`rc16`..`rc25`): without them a private function keeps the `mos` row. The
/// driver sets it for each compile.
pub fn allow_private_row(yes: bool) {
    PRIVATE_ROW.set(yes);
}

/// `name`, a C symbol, as a platform with `c_symbol_prefix` spells it.
pub fn c_symbol(
    prefix: &str,
    name: &str,
) -> String {
    format!("{prefix}{name}")
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
    static ROOTS: std::sync::OnceLock<std::collections::HashMap<String, String>> = std::sync::OnceLock::new();
    let roots = ROOTS.get_or_init(|| {
        let file = llrm_target::registers::parse(crate::REGISTERS).expect("registers.regs");
        file.iter().map(|one| (one.name.clone(), one.root.clone())).collect()
    });
    register(roots.get(byte).expect("a register of the file"))
}
