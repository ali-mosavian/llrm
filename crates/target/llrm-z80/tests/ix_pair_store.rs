//! No instruction the z80 lacks.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// byval_inline at -Os printed `ld -43(ix), hl`: a 16-bit store through ix,
/// which sdasz80 rejects ("Invalid Addressing Mode"). #7802 folds a spilled
/// copy into the store; the cell showed as a link failure in every sweep from
/// 11-07. A pair goes through ix as two byte `ld`s.
#[test]
fn a_pair_is_never_stored_or_loaded_through_ix_whole() {
    let asm = text(include_str!("data/byval_inline_main.ll"), "main", 0, &Request::new(Level::Os)).expect("compiles");
    let whole: Vec<&str> = asm
        .lines()
        .map(str::trim)
        .filter(|line| {
            let Some(rest) = line.strip_prefix("ld ") else { return false };
            let Some((to, from)) = rest.split_once(", ") else { return false };
            let pair = |x: &str| matches!(x, "hl" | "de" | "bc");
            (pair(to) && from.ends_with("(ix)")) || (pair(from) && to.ends_with("(ix)"))
        })
        .collect();
    assert!(whole.is_empty(), "{whole:?}\n{asm}");
}
