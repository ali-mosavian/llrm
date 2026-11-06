//! The z80 machine's byte costs come from `opcosts.txt`, whose instruction
//! forms are rows of the timing table.

use llrm_z80::driver::{DESCRIPTION, machine};
use llrm_z80::timing::cost;

/// Each alias of `opcosts.txt` is the bytes of the csv form it names (the csv
/// is checked against two assemblers by tests/timing/probe.py).
#[test]
fn every_form_of_the_description_is_the_bytes_of_its_row() {
    let rows = [
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
    for (alias, form) in rows {
        let bytes = cost(form).unwrap_or_else(|| panic!("no row `{form}`")).bytes;
        assert_eq!(DESCRIPTION.bytes(alias), Some(i64::from(bytes)), "{alias} is `{form}`");
    }
}

/// The machine priced every operation at 1 (`Neutral`'s table): a word load
/// through the frame, `ld c,5(ix); ld b,6(ix)`, is 6 B, a call 3, a jr 2, a
/// word add in hl 1. The unroll price read 7 B for a loop with an indirect
/// call (static_pointers -Os +4 B).
#[test]
fn the_machine_prices_operations_in_real_bytes() {
    let bytes = machine().byte_costs();
    assert_eq!((bytes.add, bytes.load, bytes.store, bytes.call, bytes.branch), (1, 6, 6, 3, 2));
    assert_eq!((bytes.r#move, bytes.return_, bytes.pop, bytes.adjust), (2, 1, 1, 5));
    assert_eq!((bytes.multiply, bytes.store_immediate), (5, 8));
}
