//! The T-state table: a few rows worked out by hand from the manual's machine
//! cycles, and its shape.

use llrm_z80::timing::{cost, cost_z180, forms, forms_z180};

/// A register load is one M1 cycle; `(hl)` adds a 3 T read; `(ix+d)` adds the
/// prefix, the displacement and 5 T of address arithmetic.
#[test]
fn loads_cost_their_machine_cycles() {
    assert_eq!(cost("ld b;c").map(|c| (c.bytes, c.taken)), Some((1, 4)));
    assert_eq!(cost("ld b;(hl)").map(|c| (c.bytes, c.taken)), Some((1, 4 + 3)));
    assert_eq!(cost("ld b;(ix+d)").map(|c| (c.bytes, c.taken)), Some((3, 4 + 4 + 3 + 5 + 3)));
}

/// A taken `jr` is 5 T longer than a skipped one; `djnz` the same; `ldir`
/// repeats at 21 and ends at 16.
#[test]
fn conditional_forms_have_two_timings() {
    let jr = cost("jr nz;e").unwrap();
    assert_eq!((jr.taken, jr.not_taken), (12, Some(7)));
    let djnz = cost("djnz e").unwrap();
    assert_eq!((djnz.taken, djnz.not_taken), (13, Some(8)));
    let ldir = cost("ldir").unwrap();
    assert_eq!((ldir.taken, ldir.not_taken), (21, Some(16)));
}

/// A 16-bit add is one instruction of 11 T; the same through two bytes (`add
/// a,l; ld l,a; adc a,h; ld h,a`) is 4 + 4 + 4 + 4 = 16 T and four bytes.
#[test]
fn a_word_add_is_cheaper_than_its_bytes() {
    let word = cost("add hl;bc").unwrap();
    let bytes = ["add a;b", "ld b;c", "adc a;b", "ld b;c"].map(|form| cost(form).unwrap());
    assert!(word.taken < bytes.iter().map(|c| c.taken).sum::<u32>());
    assert!(word.bytes < bytes.iter().map(|c| c.bytes).sum::<u32>());
}

/// `mlt bc` was a row of the Z80 table, priced from memory, and `cost` returned
/// it for a plain Z80. It is the Z180 set's, with its own source note, until
/// the Z180 manual is fetched.
#[test]
fn the_z80_table_has_no_z180_form() {
    assert_eq!(cost("mlt bc"), None);
    assert!(forms().all(|(_, c)| c.cpu == "z80"));
    assert_eq!(cost_z180("mlt bc").map(|c| (c.bytes, c.taken, c.cpu)), Some((2, 17, "z180")));
    assert!(forms_z180().all(|(_, c)| c.cpu == "z180"));
}
