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
    share_table()
        .get("percent_per_percent")
        .and_then(|v| v.as_integer())
        .and_then(|v| u64::try_from(v).ok())
        .expect("share.percent_per_percent is a count")
}

/// The same for `routine` at the size levels (`[share.size]`), where a routine
/// has a rate of its own; the general one otherwise.
pub fn size_share_percent(routine: &str) -> u64 {
    share_table()
        .get("size")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get(routine))
        .and_then(|v| v.as_integer())
        .and_then(|v| u64::try_from(v).ok())
        .unwrap_or_else(share_percent)
}

/// The same for `routine` at the speed levels (`[share.speed]`).
pub fn speed_share_percent(routine: &str) -> u64 {
    share_table()
        .get("speed")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get(routine))
        .and_then(|v| v.as_integer())
        .and_then(|v| u64::try_from(v).ok())
        .unwrap_or_else(share_percent)
}

fn share_table() -> toml::Table {
    let doc: toml::Table = ROUTINES.parse().expect("routines.toml is well formed");
    doc.get("share").and_then(|t| t.as_table()).cloned().expect("a [share] table")
}

/// Whether a call of the runtime routine `name` reads and writes none of the
/// `rcN` cells: its text and the routines it calls name none (the 32-bit
/// routines name rc2..rc7, their arguments). A store to an argument cell before
/// such a call is not an argument.
///
/// The pin's analogue is the call's operand list: only the registers it lists
/// are read (`MachineInstr::implicit_operands`, `LivePhysRegs`).
/// Whether `name` is one of the runtime's own routines (`mos_rt.s`), whose
/// text, not the call that passes it arguments, says what it reads.
pub fn is_runtime_routine(name: &str) -> bool {
    runtime().routines.contains_key(name)
}

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
    named(name, |cells| cells.clone())
}

/// Whether `name` reaches a cell by index (`cells_named` is `None`), without a
/// copy of the set: the dead-store scans ask it of every call they pass.
pub fn reaches_cells_by_index(name: &str) -> bool {
    named(name, |cells| cells.is_none())
}

fn named<T>(
    name: &str,
    answer: impl Fn(&Option<std::collections::BTreeSet<u8>>) -> T,
) -> T {
    use std::collections::BTreeSet;

    use llrm_support::hash::HashMap;
    type Answers = HashMap<String, Option<BTreeSet<u8>>>;
    static ANSWERS: std::sync::Mutex<Option<Answers>> = std::sync::Mutex::new(None);
    let mut answers = ANSWERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let answers = answers.get_or_insert_default();
    if let Some(known) = answers.get(name) {
        return answer(known);
    }
    answer(answers.entry(name.to_owned()).or_insert_with(|| scan_cells(name)))
}

use llrm_support::hash::HashMap;

/// The runtime's routines (top-level labels) and macros, by name.
pub(crate) struct Runtime {
    pub(crate) routines: HashMap<&'static str, &'static str>,
    pub(crate) macros: HashMap<&'static str, Option<Vec<u8>>>,
}
/// The cells a line names, or `None` for one reached by index.
pub(crate) fn cells_of(text: &str) -> Option<Vec<u8>> {
    let mut found = Vec::new();
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
            {
                found.extend(span.clone()?);
                continue;
            }
            found.push(digits.parse().ok()?);
        }
    }
    Some(found)
}
pub(crate) fn runtime() -> &'static Runtime {
    static RUNTIME: std::sync::OnceLock<Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| {
        let source: &'static str = crate::RUNTIME;
        let (mut routines, mut macros) = (HashMap::default(), HashMap::default());
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
        Runtime { routines, macros }
    })
}

fn scan_cells(name: &str) -> Option<std::collections::BTreeSet<u8>> {
    use std::collections::BTreeSet;

    use llrm_support::hash::HashSet;
    let runtime = runtime();
    let mut cells = BTreeSet::new();
    let mut seen = HashSet::default();
    let mut pending = vec![name];
    while let Some(routine) = pending.pop() {
        if !seen.insert(routine) {
            continue;
        }
        let text = runtime.routines.get(routine)?;
        cells.extend(cells_of(text)?);
        // the span must end in a return or a jump: a fall-through is a callee
        // the scan did not follow
        let last = text
            .lines()
            .map(|line| line.split(';').next().unwrap_or("").trim())
            .rfind(|code| !code.is_empty() && !code.ends_with(':') && !code.starts_with('.'))?;
        if !(last == "rts" || last.starts_with("jmp ")) {
            return None;
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

/// The bytes the pin's flow links for a signed 16-bit remainder
/// (`[pin.division16] signed`, llvm-mos a40207a, linked image).
pub fn pin_signed_division16_image() -> u64 {
    let doc: toml::Table = ROUTINES.parse().expect("routines.toml is well formed");
    doc.get("pin")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get("division16"))
        .and_then(|t| t.as_table())
        .and_then(|t| t.get("signed"))
        .and_then(|v| v.as_integer())
        .and_then(|v| u64::try_from(v).ok())
        .expect("pin.division16 has a signed image")
}

/// Whether the speed routines in `image` (the routines a module links) include a
/// signed 16-bit division and link no more than the pin does for it. The
/// unsigned kind is not admitted here (k_itoa -Os 200 -> 325 B, over sdcc's 288).
pub fn within_pin_division16_image(image: &std::collections::BTreeSet<String>) -> bool {
    let signed = ["__divhi3", "__modhi3", "__divmodhi4"].iter().any(|one| image.contains(*one));
    signed && crate::mulimage::image_bytes(image) as u64 <= pin_signed_division16_image()
}
