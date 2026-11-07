//! `generated/foldtables.rs`: the pin's memory-fold tables, read backwards.
//!
//! # How it works
//! `X86GenFoldTables.inc` (`-gen-x86-fold-tables`) lists `{register form,
//! memory form, flags}` rows in `Table2Addr` and `Table0` to `Table4`.
//! `X86MemUnfoldTable` ([X86InstrFoldTables.cpp#L175][pin]) turns them round:
//! one row per memory form, keyed by it, naming the register form, the operand
//! that was the memory reference and whether a load, a store or both were
//! folded. A row marked `TB_NO_REVERSE` is left out, as the pin does. The
//! broadcast tables are not read (DEVIATE FT1: vector code only; exit: when
//! vector MIR lands).
//!
//! ```text
//! {X86::ADD64rr, X86::ADD64rm, 0} in Table2   ->   (ADD64rm, ADD64rr, index 2 | folded load)
//! ```
//!
//! # Used by
//! `llrm-dagisel`'s `unfold_memory_operand`.
//!
//! [pin]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrFoldTables.cpp#L175

use std::fmt::Write as _;

use crate::inc::Ids;

const TB_NO_REVERSE: u16 = 1 << 3;
const TB_FOLDED_LOAD: u16 = 1 << 5;
const TB_FOLDED_STORE: u16 = 1 << 6;

/// The tables read, and the flags `X86MemUnfoldTable` adds to each row.
const TABLES: [(&str, u16); 6] = [
    ("Table2Addr", TB_FOLDED_LOAD | TB_FOLDED_STORE),
    ("Table0", 0),
    ("Table1", 1 | TB_FOLDED_LOAD),
    ("Table2", 2 | TB_FOLDED_LOAD),
    ("Table3", 3 | TB_FOLDED_LOAD),
    ("Table4", 4 | TB_FOLDED_LOAD),
];

fn flag(name: &str) -> Result<u16, String> {
    Ok(match name {
        "0" => 0,
        "TB_NO_REVERSE" => TB_NO_REVERSE,
        "TB_FOLDED_LOAD" => TB_FOLDED_LOAD,
        "TB_FOLDED_STORE" => TB_FOLDED_STORE,
        "TB_NO_FORWARD" => 1 << 4,
        other => match other.strip_prefix("TB_INDEX_") {
            Some(n) => n.parse().map_err(|_| format!("flag `{other}`"))?,
            None => match other.strip_prefix("TB_ALIGN_") {
                Some(n) => match n {
                    "1" => 0,
                    "16" => 4 << 7,
                    "32" => 5 << 7,
                    "64" => 6 << 7,
                    _ => return Err(format!("flag `{other}`")),
                },
                None => return Err(format!("flag `{other}`")),
            },
        },
    })
}

/// The rows of `table` as `(register form, memory form, flags)`.
fn rows<'a>(
    text: &'a str,
    table: &str,
) -> Result<Vec<(&'a str, &'a str, u16)>, String> {
    let head = format!("static const X86FoldTableEntry {table}[] = {{");
    let start = text.find(&head).ok_or_else(|| format!("no `{table}` in the fold tables"))? + head.len();
    let body = &text[start..];
    let body = &body[..body.find("};").ok_or("unterminated table")?];
    let mut out = Vec::new();
    for line in body.lines().map(str::trim).filter(|l| l.starts_with('{')) {
        let inner = line.trim_start_matches('{').trim_end_matches(',').trim_end_matches('}');
        let mut parts = inner.splitn(3, ',').map(str::trim);
        let (Some(reg), Some(mem), Some(flags)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(format!("bad row `{line}`"));
        };
        let reg = reg.strip_prefix("X86::").ok_or_else(|| format!("bad row `{line}`"))?;
        let mem = mem.strip_prefix("X86::").ok_or_else(|| format!("bad row `{line}`"))?;
        let mut bits = 0;
        for f in flags.split('|').map(str::trim) {
            bits |= flag(f)?;
        }
        out.push((reg, mem, bits));
    }
    Ok(out)
}

/// The unfold table: one row per memory form, by opcode number, for the rows
/// whose both forms the ids file numbers and whose memory form is one of
/// `emitted` (the opcodes a selector makes: only those reach the scheduler).
/// The register forms of the rows kept are added to `emitted`, so that their
/// `MCInstrDesc` is written too.
pub fn render(
    text: &str,
    ids: &Ids,
    emitted: &mut std::collections::BTreeSet<u16>,
) -> Result<String, String> {
    let mut table: Vec<(u16, u16, u16)> = Vec::new();
    for (name, extra) in TABLES {
        for (reg, mem, flags) in rows(text, name)? {
            if flags & TB_NO_REVERSE != 0 {
                continue;
            }
            let (Some(&reg), Some(&mem)) = (ids.0.get(&format!("X86::{reg}")), ids.0.get(&format!("X86::{mem}")))
            else {
                continue;
            };
            if emitted.contains(&(mem as u16)) {
                table.push((mem as u16, reg as u16, flags | extra));
            }
        }
    }
    // `array_pod_sort` by the key, then the pin asserts the keys are unique.
    table.sort_by_key(|&(mem, _, _)| mem);
    for pair in table.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(format!("the memory form {} is unfolded to two register forms", pair[0].0));
        }
    }
    emitted.extend(table.iter().map(|&(_, reg, _)| reg));
    let mut out = String::from(
        "//! The pin's memory-fold tables read backwards: `(memory form, register form, flags)`, by memory form.\n//!\n//! Generated by `tools/dagisel/regen.sh`; do not edit.\n\n/// `TB_INDEX_MASK`.\npub const INDEX_MASK: u16 = 0x7;\n/// `TB_FOLDED_LOAD`.\npub const FOLDED_LOAD: u16 = 1 << 5;\n/// `TB_FOLDED_STORE`.\npub const FOLDED_STORE: u16 = 1 << 6;\n\npub const UNFOLD: &[(u16, u16, u16)] = &[\n",
    );
    for (mem, reg, flags) in table {
        writeln!(out, "    ({mem}, {reg}, {flags}),").unwrap();
    }
    out.push_str("];\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INC: &str = "static const X86FoldTableEntry Table2Addr[] = {\n  {X86::ADD64rr, X86::ADD64mr, TB_NO_REVERSE},\n  {X86::SUB64rr, X86::SUB64mr, 0},\n};\n\nstatic const X86FoldTableEntry Table0[] = {\n};\n\nstatic const X86FoldTableEntry Table1[] = {\n};\n\nstatic const X86FoldTableEntry Table2[] = {\n  {X86::ADD64rr, X86::ADD64rm, 0},\n  {X86::AND64rr, X86::AND64rm, TB_NO_REVERSE},\n};\n\nstatic const X86FoldTableEntry Table3[] = {\n};\n\nstatic const X86FoldTableEntry Table4[] = {\n};\n";

    fn all() -> std::collections::BTreeSet<u16> {
        [680, 681, 901, 11].into_iter().collect()
    }

    fn ids() -> Ids {
        Ids::parse("X86::ADD64rr 685\nX86::ADD64rm 680\nX86::ADD64mr 681\nX86::SUB64rr 900\nX86::SUB64mr 901\nX86::AND64rr 10\nX86::AND64rm 11\n")
            .unwrap()
    }

    #[test]
    fn test_a_register_form_names_the_unfolded_form_at_its_index() {
        let text = render(INC, &ids(), &mut all()).unwrap();
        // ADD64rm unfolds to ADD64rr, operand 2, a folded load (2 | 32).
        assert!(text.contains("(680, 685, 34),"), "{text}");
    }

    #[test]
    fn test_a_row_marked_no_reverse_is_left_out() {
        let text = render(INC, &ids(), &mut all()).unwrap();
        assert!(!text.contains("(11, 10,"), "AND64rm is TB_NO_REVERSE: {text}");
        assert!(!text.contains("(681,"), "ADD64mr is TB_NO_REVERSE: {text}");
    }

    #[test]
    fn test_a_two_address_row_is_a_folded_load_and_store_at_index_zero() {
        let text = render(INC, &ids(), &mut all()).unwrap();
        assert!(text.contains("(901, 900, 96),"), "{text}");
    }
}
