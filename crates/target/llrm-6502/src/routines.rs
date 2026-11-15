//! `machines/routines.toml`: what a call of a runtime routine costs in clocks,
//! for pricing the same function with the copy a call and a loop.

pub const ROUTINES: &str = include_str!("machines/routines.toml");

/// The price of a copy as a call.
pub struct CopyPrices {
    /// What a call of `memcpy` adds to the `jsr`, for a copy of unknown length.
    pub call_clocks: u64,
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
    CopyPrices { call_clocks: number("entry_clocks") + number("typical_bytes") * number("clocks_per_byte") }
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
/// `rcN` cells: its text and the routines it calls name none (the 32-bit
/// routines name rc2..rc7, their arguments). A store to an argument cell before
/// such a call is not an argument.
///
/// The pin's analogue is the call's operand list: only the registers it lists
/// are read (`MachineInstr::implicit_operands`, `LivePhysRegs`).
pub fn leaves_cells_alone(name: &str) -> bool {
    cells_named(name).is_some_and(|cells| cells.is_empty())
}

/// The `rcN` cells a call of the runtime routine `name` may read or write: the
/// ones its text and its callees' name. `None` when that is not known (not a
/// runtime routine, a cell reached by index, a macro that names one, a span
/// that falls through its last line). Its registers `a`, `x`, `y` are the
/// caller's to treat as clobbered.
///
/// The pin's analogue is the call's register mask from the callee's own use
/// (`RegUsageInfo`, `-enable-ipra`; gcc's `-fipa-ra`).
pub fn cells_named(name: &str) -> Option<std::collections::BTreeSet<u8>> {
    use std::collections::{BTreeSet, HashMap};
    type Answers = HashMap<String, Option<BTreeSet<u8>>>;
    static ANSWERS: std::sync::Mutex<Option<Answers>> = std::sync::Mutex::new(None);
    let mut answers = ANSWERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    answers.get_or_insert_default().entry(name.to_owned()).or_insert_with(|| scan_cells(name)).clone()
}

fn scan_cells(name: &str) -> Option<std::collections::BTreeSet<u8>> {
    use std::collections::{BTreeSet, HashMap, HashSet};
    struct Runtime {
        routines: HashMap<&'static str, &'static str>,
        /// The routine each one falls into: the next label after it, with no
        /// code between.
        falls_into: HashMap<&'static str, &'static str>,
        macros: HashMap<&'static str, Option<Vec<u8>>>,
    }
    static RUNTIME: std::sync::OnceLock<Runtime> = std::sync::OnceLock::new();
    /// The cells a line names, or `None` for one reached by index. A routine
    /// that indexes says which cells it reaches in a comment line `; reaches
    /// rc2 rc3`: those are added, and its indexed operands are then known.
    fn cells_of(text: &str) -> Option<Vec<u8>> {
        let mut found = Vec::new();
        let reached: Vec<u8> = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("; reaches"))
            .flat_map(|names| names.split_whitespace().filter_map(|name| name.strip_prefix("rc")?.parse().ok()))
            .collect();
        found.extend(&reached);
        for line in text.lines() {
            let (code, comment) = line.split_once(';').unwrap_or((line, ""));
            // an index into cells names its span in the line's comment:
            // `[rc4..rc7]`
            let span = comment
                .split_once('[')
                .and_then(
                    |(_, rest)| {
                        let (from, rest) = rest.strip_prefix("rc")?.split_once("..rc")?;
                        let (to, _) = rest.split_once(']')?;
                        Some(from.parse::<u8>().ok()?..=to.parse::<u8>().ok()?)
                    },
                );
            if code.match_indices("rs").any(|(at, _)| {
                !code[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_')
                    && code[at + 2..].starts_with(|c: char| c.is_ascii_digit())
            }) {
                return None;
            }
            for (at, _) in code.match_indices("rc") {
                let before = code[..at].chars().next_back();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let digits: String = code[at + 2..].chars().take_while(char::is_ascii_digit).collect();
                if digits.is_empty() {
                    continue;
                }
                let after = &code[at + 2 + digits.len()..];
                if after
                    .strip_prefix(',')
                    .is_some_and(|index| matches!(index.trim().to_ascii_lowercase().as_str(), "x" | "y"))
                    && reached.is_empty()
                {
                    found.extend(span.clone()?);
                    continue;
                }
                found.push(digits.parse().ok()?);
            }
        }
        Some(found)
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
                    macros.insert(name, cells_of(&source[from..offset]));
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
        let mut order: Vec<(usize, &'static str)> = routines
            .iter()
            .map(|(label, text): (&&'static str, &&'static str)| {
                (text.as_ptr() as usize - source.as_ptr() as usize, *label)
            })
            .collect();
        order.sort_unstable();
        let mut falls_into = HashMap::new();
        for pair in order.windows(2) {
            let (from, label) = pair[0];
            let end = from + routines[label].len();
            let gap = &source[end..pair[1].0];
            let quiet = gap
                .lines()
                .all(
                    |line| {
                        let code = line.split(';').next().unwrap_or("").trim();
                        code.is_empty() || code.starts_with(".export") || code.starts_with(".global")
                    },
                );
            if quiet {
                falls_into.insert(label, pair[1].1);
            }
        }
        Runtime { routines, falls_into, macros }
    });
    let mut cells = BTreeSet::new();
    let mut seen = HashSet::new();
    let mut pending = vec![name];
    while let Some(routine) = pending.pop() {
        if !seen.insert(routine) {
            continue;
        }
        let text = runtime.routines.get(routine)?;
        cells.extend(cells_of(text)?);
        // a span that ends in neither a return nor a jump falls into the next
        // routine
        let last = text
            .lines()
            .map(|line| line.split(';').next().unwrap_or("").trim())
            .rfind(|code| !code.is_empty() && !code.ends_with(':') && !code.starts_with('.'))?;
        if !(last == "rts" || last.starts_with("jmp ")) {
            pending.push(runtime.falls_into.get(routine)?);
        }
        for line in text.lines() {
            let code = line.split(';').next().unwrap_or("").trim();
            let mut words = code.split_whitespace();
            let (Some(op), arg) = (words.next(), words.next()) else { continue };
            // a macro names its own cells where it is expanded
            if let Some(named) = runtime.macros.get(op) {
                cells.extend(named.as_ref()?);
            }
            // a call, a jump or a branch to another routine's label (`bne
            // udivmod16c`)
            if let Some(target) = arg.filter(|target| runtime.routines.contains_key(target)) {
                pending.push(target);
            } else if matches!(op, "jsr" | "jmp") && arg.is_none_or(|target| !target.starts_with('@')) {
                return None;
            }
        }
    }
    Some(cells)
}
