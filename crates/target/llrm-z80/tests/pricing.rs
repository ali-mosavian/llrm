//! What a variant costs: the bytes and T-states of the printed lines.

use llrm_z80::expand::{Line, price_in_loops};

fn price(text: &str) -> (u32, u64) {
    price_in_loops(&[Line::Inst(text.to_owned())])
}

/// A form the table did not recognise cost 2 B and 8 T: `ex de, hl` (1 B), a
/// conditional `jp` (3 B), the frame forms (`ld iy,#n` is 4). At -Os, where the
/// price is bytes, write_through_parameter's `main` chose a variant 3 B bigger
/// than the one it passed over (231 -> 234).
#[test]
fn every_form_the_printer_writes_is_in_the_table() {
    for (text, bytes, clocks) in [
        ("ex de, hl", 1, 4),
        ("jp nz, L0_9", 3, 10),
        ("jp z, L0_2", 3, 10),
        ("jr z, L0_2", 2, 12),
        ("ret z", 1, 11),
        ("ld sp, ix", 2, 10),
        ("ld sp, iy", 2, 10),
        ("add ix, sp", 2, 15),
        ("add iy, sp", 2, 15),
        ("ld iy, #-14", 4, 14),
    ] {
        assert_eq!(price(text), (bytes, clocks), "{text}");
    }
}
