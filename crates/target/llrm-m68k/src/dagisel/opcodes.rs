//! The pin's machine opcodes, by name and by code.

use std::cell::Cell;
use std::sync::LazyLock;

use llrm_support::hash::HashMap;

use super::generated::m68k_m68000::MACHINE_OPCODES;

thread_local! {
    static PROBES: Cell<usize> = const { Cell::new(0) };
}

/// How many lookups ran on this thread: a scan of the whole table per
/// question was 4% of tile -O2's compile instructions in
/// `short::narrow` alone (204 k scans).
pub fn probes() -> usize {
    PROBES.with(Cell::get)
}

struct Tables {
    codes: HashMap<&'static str, u16>,
    names: HashMap<u16, &'static str>,
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| Tables {
    // the first row of a name wins, as the scan it replaces found it
    codes: MACHINE_OPCODES.iter().rev().map(|&(code, name)| (name, code)).collect(),
    names: MACHINE_OPCODES.iter().rev().map(|&(code, name)| (code, name)).collect(),
});

/// The machine code of the pin's instruction `name`.
pub fn code_of(name: &str) -> Option<u16> {
    PROBES.with(|count| count.set(count.get() + 1));
    TABLES.codes.get(name).copied()
}

/// The name of the pin's machine code `code`.
pub fn name_of(code: u32) -> Option<&'static str> {
    PROBES.with(|count| count.set(count.get() + 1));
    TABLES.names.get(&u16::try_from(code).ok()?).copied()
}
