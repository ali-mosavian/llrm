//! The pin's machine opcodes, by name and by code.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::LazyLock;

use super::generated::{m68k_m68000, m68k_m68020};

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

/// The 68000's rows, then the 68020's: a row only a 68020 selects (`cas`) has a name and a code too.
fn rows() -> impl DoubleEndedIterator<Item = &'static (u16, &'static str)> {
    m68k_m68000::MACHINE_OPCODES.iter().chain(m68k_m68020::MACHINE_OPCODES.iter())
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| Tables {
    // the first row of a name wins, as the scan it replaces found it
    codes: rows().rev().map(|&(code, name)| (name, code)).collect(),
    names: rows().rev().map(|&(code, name)| (code, name)).collect(),
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
