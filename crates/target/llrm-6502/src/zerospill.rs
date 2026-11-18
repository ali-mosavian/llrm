//! The static cells of a whole program in zero page.
//!
//! # How it works
//! The allocator keeps what its registers cannot hold in `s_<function>_<n>`
//! cells, one BSS byte or more each (`.res`), addressed by three bytes and
//! four clocks. A zero-page address is two bytes and three clocks. Where the
//! platform row has zero page to spare (`static_zero_page`), the cells that
//! the program touches most are given addresses there, as equates
//! (`s_main_0 = $A9`), and leave BSS; the rest stay where they were. A cell
//! counts as often as it is read or written, ten times more for each level of
//! loop it sits in (a backward branch to a label of the same function), and a
//! cell is taken in order of that count a byte until the budget is spent.
//!
//! The program is the whole link but its runtime (`-fwhole-program`): two
//! objects would give the same address to two cells. A cell that is indexed
//! (`s_x,y`: zero page wraps at 256) or whose address is taken stays BSS, as
//! does any that is not named `s_`: the static locals that the pass of
//! [`crate::staticlocals`] makes are zero-initialised arrays that code indexes.
//!
//! DEVIATE 6502-STATIC-ZP: not in the pin; llvm-mos's spills are imaginary
//! registers, zero page by construction. Owner: llrm-target-6502. Exit:
//! 2026-11-24, with the budget shared by the objects of a link.

use std::collections::BTreeMap;

/// What the text does with one cell.
#[derive(Default)]
struct Cell {
    size: u32,
    /// Accesses, each weighted by ten per level of loop.
    weight: u64,
    /// Indexed or address taken: it cannot be an equate.
    pinned: bool,
}

/// `text` with the hottest `s_` cells that fit in `budget` bytes made zero-page
/// equates from `first`.
pub fn static_cells_to_zero_page(
    text: &str,
    first: u16,
    budget: u16,
) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let depth = loop_depths(&lines);
    let mut cells: BTreeMap<&str, Cell> = BTreeMap::new();
    for line in &lines {
        if let Some((name, size)) = declaration(line) {
            cells.entry(name).or_default().size = size;
        }
    }
    if cells.is_empty() {
        return text.to_owned();
    }
    for (at, line) in lines.iter().enumerate() {
        if declaration(line).is_some() {
            continue;
        }
        let code = line.split(';').next().unwrap_or("");
        for (name, cell) in cells.iter_mut() {
            let mut rest = code;
            while let Some(found) = rest.find(*name) {
                let after = &rest[found + name.len()..];
                let before = &rest[..found];
                let whole = !before.ends_with(|c: char| c.is_alphanumeric() || c == '_')
                    && !after.starts_with(|c: char| c.is_alphanumeric() || c == '_');
                if whole {
                    let offset =
                        after.strip_prefix('+').map_or(after, |n| n.trim_start_matches(|c: char| c.is_ascii_digit()));
                    // `#<(s_x)`, `#>(s_x)` and `s_x,y` or `s_x,x`
                    if before.trim_end().ends_with("(") || offset.trim_start().starts_with(',') {
                        cell.pinned = true;
                    }
                    cell.weight += 10u64.pow(depth[at].min(3));
                }
                rest = after;
            }
        }
    }
    let mut ranked: Vec<(&str, &Cell)> =
        cells.iter().filter(|(_, cell)| !cell.pinned && cell.weight > 0).map(|(n, c)| (*n, c)).collect();
    // the most accesses a byte first; the name breaks a tie
    ranked.sort_by(|a, b| {
        let density = |c: &Cell| u128::from(c.weight) * 1000 / u128::from(c.size.max(1));
        density(b.1).cmp(&density(a.1)).then(a.0.cmp(b.0))
    });
    let mut at = u32::from(first);
    let end = u32::from(first) + u32::from(budget);
    let mut taken: BTreeMap<&str, u32> = BTreeMap::new();
    for (name, cell) in ranked {
        if at + cell.size <= end {
            taken.insert(name, at);
            at += cell.size;
        }
    }
    if taken.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    for (name, address) in &taken {
        out.push_str(&format!("{name} = ${address:02X}\n"));
    }
    for line in &lines {
        if declaration(line).is_some_and(|(name, _)| taken.contains_key(name)) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `s_main_0: .res 1`: the cell and its bytes.
fn declaration(line: &str) -> Option<(&str, u32)> {
    let (name, rest) = line.split_once(':')?;
    if !name.starts_with("s_") || name.contains(char::is_whitespace) {
        return None;
    }
    let size = rest.trim().strip_prefix(".res")?.trim().parse().ok()?;
    Some((name, size))
}

/// The loops each line is in: a branch or jump back to a label of its own
/// function covers the lines between.
fn loop_depths(lines: &[&str]) -> Vec<u32> {
    let mut start_of_function = 0;
    let mut labels: BTreeMap<&str, usize> = BTreeMap::new();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let code = line.split(';').next().unwrap_or("").trim_end();
        if let Some(label) = code.strip_suffix(':').filter(|l| !l.contains(char::is_whitespace)) {
            let starts_function = at > 0
                && (lines[at - 1].trim() == ".segment \"CODE\"" || lines[at - 1].trim_start().starts_with(".export"));
            if starts_function {
                start_of_function = at;
            }
            labels.entry(label).or_insert(at);
            continue;
        }
        let mut words = code.split_whitespace();
        let (Some(op), Some(target)) = (words.next(), words.next()) else { continue };
        if matches!(
            op,
            "bne" | "beq" | "bcc" | "bcs" | "bpl" | "bmi" | "bvc" | "bvs" | "bra" | "jmp"
        )
            && let Some(&label) = labels.get(target)
            && label >= start_of_function
            && label < at
        {
            spans.push((label, at));
        }
    }
    let mut depth = vec![0u32; lines.len()];
    for (from, to) in spans {
        for d in &mut depth[from..=to] {
            *d += 1;
        }
    }
    depth
}
