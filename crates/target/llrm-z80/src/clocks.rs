//! What z80 code costs by operation in T-states: `opcosts.txt`'s sums over the
//! instruction forms, priced by the Zilog manual's table (`timing/z80.csv`)
//! where the bytes side is priced by the forms' lengths. An operation that is a
//! library call adds the routine's typical clocks (`runtime_size`); a fill and
//! a copy are `ldir`, 21 T a byte.

use llrm_mir::target::OperationCosts;

use crate::driver::DESCRIPTION;
use crate::runtime_size::routine_clocks;
use crate::timing::cost;

/// The alias of `opcosts.txt` and the csv row it names (the bytes side is
/// checked against the same rows).
pub const FORMS: [(&str, &str); 19] = [
    ("add_hl", "add hl;bc"),
    ("mov_rr", "ld b;c"),
    ("ld_ix", "ld b;(ix+d)"),
    ("st_ix", "ld (ix+d);b"),
    ("st_ix_n", "ld (ix+d);n"),
    ("inc_ix", "inc (ix+d)"),
    ("jr", "jr e"),
    ("call", "call nn"),
    ("ret", "ret"),
    ("push", "push bc"),
    ("pop", "pop bc"),
    ("ld_hl_nn", "ld hl;nn"),
    ("add_hl_sp", "add hl;sp"),
    ("ld_sp_hl", "ld sp;hl"),
    ("srl_r", "srl b"),
    ("rr_r", "rr b"),
    ("ld_rn", "ld b;n"),
    ("rla", "rla"),
    ("ldir", "ldir"),
];

/// T-states of the form `alias` names: the taken path of a conditional one.
fn form_clocks(alias: &str) -> i64 {
    let (_, row) = FORMS.iter().find(|(name, _)| *name == alias).unwrap_or_else(|| panic!("no form `{alias}`"));
    i64::from(cost(row).unwrap_or_else(|| panic!("no row `{row}`")).taken)
}

/// A call to `routine` with `moves` byte moves around it: the call, the
/// routine's typical body.
fn library(
    routine: &str,
    moves: i64,
) -> i64 {
    form_clocks("call") + i64::from(routine_clocks(&format!("call {routine}"), &[])) + moves * form_clocks("mov_rr")
}

/// What an operation costs in T-states.
pub fn sheet() -> OperationCosts {
    static SHEET: std::sync::LazyLock<OperationCosts> = std::sync::LazyLock::new(compute);
    SHEET.clone()
}

fn compute() -> OperationCosts {
    let sheet = DESCRIPTION.operations(&form_clocks, 0);
    let ldir_byte = i64::from(cost("ldir").expect("ldir").taken);
    // `ld (hl),v; ld e,l; ld d,h; inc de; ld bc,n; ldir` for a fill: the setup,
    // then 21 T for each byte after the first; the generic count is in
    // cells of four bytes.
    let fill_setup = 7 + 4 + 4 + 6 + 10 + i64::from(cost("ldir").expect("ldir").not_taken.expect("last pass"));
    OperationCosts {
        multiply: library("__mulhi3", 2),
        divide: library("__divhi3", 2),
        multiply_long: library("__mulsi3", 4),
        multiply_high: library("__mulsi3", 4),
        divide_long: library("__divsi3", 4),
        fill: fill_setup,
        fill_cell: 4 * ldir_byte,
        copy_cell: 4 * ldir_byte,
        jump_table: 4,
        ..sheet
    }
}
