//! `machines/routines.toml`: what a call of a runtime routine costs in clocks,
//! for pricing the same function with the copy a call and a loop.

pub const ROUTINES: &str = include_str!("machines/routines.toml");

/// The prices of a copy as a call, and the worth of a byte against them.
pub struct CopyPrices {
    /// What a call of `memcpy` adds to the `jsr`, for a copy of unknown length.
    pub call_clocks: u64,
    /// The clocks a byte of code is worth, at speed and at size.
    pub byte_clocks: (u64, u64),
}

pub fn copy_prices() -> CopyPrices {
    let doc: toml::Table = ROUTINES.parse().expect("routines.toml is well formed");
    let table = doc.get("copy").and_then(|t| t.as_table()).expect("a [copy] table");
    let number = |key: &str| {
        table
            .get(key)
            .and_then(|v| v.as_integer())
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or_else(|| panic!("copy.{key} is not a count"))
    };
    CopyPrices {
        call_clocks: number("entry_clocks") + number("typical_bytes") * number("clocks_per_byte"),
        byte_clocks: (number("byte_clocks_speed"), number("byte_clocks_size")),
    }
}

/// The compact variant of the runtime routine `name`, called at size, if it has
/// one.
pub fn compact_variant(name: &str) -> Option<&'static str> {
    static TABLE: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| {
            let doc: toml::Table = ROUTINES.parse().expect("routines.toml is well formed");
            doc.get("compact")
                .and_then(|t| t.as_table())
                .expect("a [compact] table")
                .iter()
                .map(|(fast, compact)| (fast.clone(), compact.as_str().expect("a routine name").to_owned()))
                .collect()
        })
        .iter()
        .find(|(fast, _)| fast == name)
        .map(|(_, compact)| compact.as_str())
}

/// The share of its clocks a module must save, in percent, for each percent of
/// its bytes a speed routine adds at size (`[share] percent_per_percent`).
pub fn share_percent() -> u64 {
    let doc: toml::Table = ROUTINES.parse().expect("routines.toml is well formed");
    let table = doc.get("share").and_then(|t| t.as_table()).expect("a [share] table");
    table
        .get("percent_per_percent")
        .and_then(|v| v.as_integer())
        .and_then(|v| u64::try_from(v).ok())
        .expect("share.percent_per_percent is a count")
}

/// Whether a call of the runtime routine `name` reads and writes none of the
/// `rcN` cells: its text and the routines it calls name none (the long
/// arithmetic works in `__sa`/`__sb`). A store to an argument cell before such
/// a call is not an argument.
///
/// The pin's analogue is the call's operand list: only the registers it lists
/// are read (`MachineInstr::implicit_operands`, `LivePhysRegs`).
pub fn leaves_cells_alone(name: &str) -> bool {
    use std::collections::HashMap;
    static ANSWERS: std::sync::Mutex<Option<HashMap<String, bool>>> = std::sync::Mutex::new(None);
    let mut answers = ANSWERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *answers.get_or_insert_default().entry(name.to_owned()).or_insert_with(|| scanned_leaves_cells_alone(name))
}

fn scanned_leaves_cells_alone(name: &str) -> bool {
    use std::collections::{HashMap, HashSet};
    struct Runtime {
        routines: HashMap<&'static str, &'static str>,
        macros: HashMap<&'static str, bool>,
    }
    static RUNTIME: std::sync::OnceLock<Runtime> = std::sync::OnceLock::new();
    fn mentions_cell(text: &str) -> bool {
        text.lines().any(|line| {
            let code = line.split(';').next().unwrap_or("");
            code.match_indices("rc").any(|(at, _)| {
                let before = code[..at].chars().next_back();
                let after = code[at + 2..].chars().next();
                !before.is_some_and(|c| c.is_alphanumeric() || c == '_') && after.is_some_and(|c| c.is_ascii_digit())
            })
        })
    }
    let runtime = RUNTIME.get_or_init(|| {
        let source: &'static str = crate::RUNTIME;
        let (mut routines, mut macros) = (HashMap::new(), HashMap::new());
        let mut start: Option<(&'static str, usize)> = None;
        let mut macro_start: Option<(&'static str, usize)> = None;
        let mut offset = 0;
        for line in source.split_inclusive('\n') {
            let margin = line.chars().next().is_some_and(|c| !c.is_whitespace() && c != ';' && c != '@');
            let code = line.split(';').next().unwrap_or("").trim_end();
            if let Some((label, from)) = start.filter(|_| margin) {
                routines.insert(label, &source[from..offset]);
                start = None;
            }
            if let Some(rest) = code.strip_prefix(".macro ") {
                let name = rest.split_whitespace().next().unwrap_or("");
                macro_start = Some((&source[offset + 7..offset + 7 + name.len()], offset));
            } else if code.starts_with(".endmacro") {
                if let Some((name, from)) = macro_start.take() {
                    macros.insert(name, mentions_cell(&source[from..offset]));
                }
            } else if margin && macro_start.is_none() {
                if let Some(label) = code.strip_suffix(':').filter(|label| !label.contains(char::is_whitespace)) {
                    start = Some((&source[offset..offset + label.len()], offset));
                }
            }
            offset += line.len();
        }
        if let Some((label, from)) = start {
            routines.insert(label, &source[from..]);
        }
        Runtime { routines, macros }
    });
    let mut seen = HashSet::new();
    let mut pending = vec![name];
    while let Some(routine) = pending.pop() {
        if !seen.insert(routine) {
            continue;
        }
        let Some(text) = runtime.routines.get(routine) else { return false };
        if mentions_cell(text) {
            return false;
        }
        for line in text.lines() {
            let code = line.split(';').next().unwrap_or("").trim();
            let mut words = code.split_whitespace();
            let (Some(op), arg) = (words.next(), words.next()) else { continue };
            if runtime.macros.get(op).is_some_and(|&mentions| mentions) {
                return false;
            }
            if matches!(op, "jsr" | "jmp") {
                match arg {
                    Some(target) if !target.starts_with('@') => pending.push(target),
                    Some(_) => {}
                    None => return false,
                }
            }
        }
    }
    true
}
