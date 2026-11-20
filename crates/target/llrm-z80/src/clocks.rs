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

/// The prices of the Os inline trial's byte estimate: `opcosts.txt`'s bytes,
/// refit to what the backend writes.
pub fn trial_bytes() -> OperationCosts {
    static BYTES: std::sync::LazyLock<OperationCosts> = std::sync::LazyLock::new(|| {
        let mut costs = DESCRIPTION.size_costs();
        calibrate(&mut costs);
        costs
    });
    BYTES.clone()
}

/// What the forms alone do not say, fit to the bytes the backend writes: a
/// non-negative least squares of each function's operation counts
/// (word-weighted) against its written size, over the 89 programs of the 6502's
/// `tests/data_size` at -Os (`tools/size_fit.py`, 178 functions). The forms
/// priced a function at 0.54 of its bytes: every word is held in a register
/// pair that the allocator shuffles (`ld a,l; add a,c; ld l,a`).
///
/// DEVIATE (Z80-TRIAL-BYTES, owner z80, exit 2026-11-26): measured here, the
/// pin has no byte model for the Z80. Exit: the generated scheduling
/// description of the instruction tables.
fn calibrate(costs: &mut OperationCosts) {
    costs.native_bits = 16;
    costs.width_scaled = true;
    costs.constant_word_shifts = true;
    costs.exact_call_arguments = true;
    costs.add = 9;
    costs.address = 6;
    costs.load = 5;
    costs.direct_load = 5;
    costs.store = 5;
    costs.direct_store = 5;
    costs.shift = 7;
    costs.extend = 2;
    costs.call = 3;
    // a word is two loads from its slot (`ld e,d(ix); ld d,d(ix+1)`), the ones
    // past the third also a `push` and a `pop`
    costs.argument = 6;
    costs.argument_registers = 3;
    costs.argument_stacked = 8;
    costs.multiply = 12;
    costs.multiply_long = 12;
    costs.divide = 23;
    costs.divide_long = 23;
    costs.free_intrinsics = true;
}
