//! The z80 machine's byte costs come from `opcosts.txt`, whose instruction
//! forms are rows of the timing table.

use llrm_z80::driver::{DESCRIPTION, machine};
use llrm_z80::timing::cost;

/// Each alias of `opcosts.txt` is the bytes of the csv form it names (the csv
/// is checked against two assemblers by tests/timing/probe.py).
#[test]
fn every_form_of_the_description_is_the_bytes_of_its_row() {
    let rows = llrm_z80::clocks::FORMS;
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

/// `costs()` was `Neutral`'s table (every operation 1, a library multiply 1, a fill 1 and a cell 1), so every
/// T-priced decision (inlining, unrolling, a fill against its loop) compared instruction counts: a word load through
/// the frame is `ld c,5(ix); ld b,6(ix)`, 38 T; a call 17; a `jr` taken 12; a word add in `hl` 11; a fill 47 T and
/// 84 T a cell of four bytes. The Zilog manual's rows, UM008011-0816.
#[test]
fn the_machine_prices_operations_in_t_states() {
    let t = machine().costs();
    assert_eq!((t.add, t.load, t.store, t.call, t.branch), (11, 38, 38, 17, 12));
    assert_eq!((t.r#move, t.return_, t.pop, t.store_immediate, t.shift), (8, 10, 10, 38, 16));
    assert_eq!((t.argument, t.adjust, t.memory_update), (11 + 38, 10 + 11 + 6, 46));
    assert_eq!((t.fill, t.fill_cell, t.copy_cell), (47, 84, 84));
    // a multiply and a divide are the library's: far dearer than the call that reaches them
    assert!(t.multiply > 100 && t.divide > 800 && t.multiply_long > t.multiply && t.divide_long > t.divide);
}

/// The size decisions took `costs()` by default, so stating T-states there made them price bytes in clocks: a 96-byte
/// memcpy at -Os became 48 word moves (copy_runs 953 B to 1315 B).
#[test]
fn the_size_levels_keep_their_unit_prices() {
    let size = machine().size_costs();
    assert_eq!((size.add, size.load, size.store, size.call, size.fill_cell), (1, 1, 1, 1, 1));
}

/// 48 adjacent cell copies, one `memcpy` of 96 bytes: at -Os it is the library call, not 48 word moves through `bc`
/// (copy_runs 953 B, 1315 B when the size decisions read T-states).
#[test]
fn adjacent_cell_copies_at_os_are_one_call() {
    use llrm_z80::compile::text;
    use llrm_z80::driver::{Level, Request};
    let mut ir = String::from("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n@s = global [48 x i16] zeroinitializer\n@d = global [48 x i16] zeroinitializer\ndefine void @g() {\n");
    for cell in 0..48 {
        ir += &format!("  %p{cell} = getelementptr inbounds [48 x i16], ptr @s, i16 0, i16 {cell}\n  %v{cell} = load i16, ptr %p{cell}\n  %q{cell} = getelementptr inbounds [48 x i16], ptr @d, i16 0, i16 {cell}\n  store i16 %v{cell}, ptr %q{cell}\n");
    }
    ir += "  ret void\n}\n";
    let asm = text(&ir, "g", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(asm.contains("__memcpy") && asm.lines().count() < 20, "{asm}");
}
