//! The bytes of the runtime routines a module reaches: a routine is linked
//! once however many calls name it, with the routines it calls or falls into.
//! The table is `runtime/sizes.csv`, written by `runtime/sizes.py` from the
//! assembler listing of `z80_rt.s` (a size is the distance to the next global
//! symbol, as the link map gives it).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::expand::Line;

/// One runtime routine: its bytes and the routines it calls or falls into.
struct Routine {
    bytes: u32,
    calls: Vec<&'static str>,
}

static TABLE: LazyLock<HashMap<&'static str, Routine>> = LazyLock::new(|| {
    include_str!("../runtime/sizes.csv")
        .lines()
        .map(|row| {
            let mut cells = row.split(',');
            let name = cells.next().expect("a name");
            let bytes = cells.next().and_then(|cell| cell.parse().ok()).expect("a size");
            let calls = cells.next().unwrap_or("").split(';').filter(|one| !one.is_empty()).collect();
            (name, Routine { bytes, calls })
        })
        .collect()
});

/// The symbol a `call` or `jp` of `text` names.
fn target(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("call ").or_else(|| text.strip_prefix("jp "))?;
    rest.rsplit(',').next().map(str::trim)
}

/// The bytes of routine `name` alone, if the runtime has it.
pub fn own_bytes(name: &str) -> Option<u32> {
    TABLE.get(name).map(|routine| routine.bytes)
}

/// The runtime routines the printed `lines` call or jump to.
pub fn referenced(lines: &[Line]) -> HashSet<String> {
    lines
        .iter()
        .filter_map(|line| match line {
            Line::Inst(text) => target(text).filter(|name| TABLE.contains_key(*name)).map(str::to_owned),
            _ => None,
        })
        .collect()
}

/// The bytes of `names` and every routine they reach, each once.
pub fn reached(names: &HashSet<String>) -> u32 {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut work: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut bytes = 0;
    while let Some(name) = work.pop() {
        let Some((key, routine)) = TABLE.get_key_value(name) else { continue };
        if seen.insert(key) {
            bytes += routine.bytes;
            work.extend(routine.calls.iter().copied());
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `__mulsi3` assembles to 57 B (sdasz80 listing of `runtime/z80_rt.s`).
    #[test]
    fn the_long_multiply_routine_is_57_bytes() {
        assert_eq!(own_bytes("__mulsi3"), Some(57));
    }

    /// `runtime/sizes.csv` is what `runtime/sizes.py` writes from `z80_rt.s`.
    #[test]
    fn the_size_table_is_the_assembled_runtime() {
        let done = std::process::Command::new("python3")
            .args([concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/sizes.py"), "--check"])
            .output();
        if let Ok(done) = done {
            let stderr = String::from_utf8_lossy(&done.stderr).into_owned();
            // no sdasz80 on the machine: the check cannot run
            if !stderr.contains("sdasz80") {
                assert!(done.status.success(), "{stderr}");
            }
        }
    }
}
