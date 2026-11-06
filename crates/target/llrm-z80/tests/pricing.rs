//! What a variant costs: the bytes and T-states of the printed lines.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::{Line, forget_trips, price_emitted, price_in_loops};

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

fn counted_loop(counter: &str) -> Vec<Line> {
    ["L0_1:", "inc hl", &format!("dec {counter}"), "jp nz, L0_1"]
        .iter()
        .map(|text| match text.strip_suffix(':') {
            Some(label) => Line::Label(label.to_owned()),
            None => Line::Inst((*text).to_owned()),
        })
        .collect()
}

/// A counter in `b` ends as `djnz` (2 B, 13 T), one in `c` as `dec c; jr nz`
/// (3 B, 16 T): the two were priced alike, before the jumps are relaxed.
#[test]
fn a_counter_in_b_prices_below_one_in_c() {
    forget_trips();
    let (b, c) = (price_emitted(&counted_loop("b"), true), price_emitted(&counted_loop("c"), true));
    assert!(b.0 < c.0 && b.1 < c.1, "b {b:?} c {c:?}");
}

/// byval_struct -O1 took the allocation with the 41-trip counter in `c`
/// (`dec c; jr nz`, 339 B and 11925 T) over the one with `djnz` (340 B and
/// 11842 T): +83 T for -1 B, priced as a gain.
#[test]
fn a_counted_loop_keeps_its_djnz_at_o1() {
    let out = text(include_str!("kernels/ir/byval_struct.ll"), "main", 0, &Request::new(Level::O1)).expect("compiles");
    assert!(out.contains("djnz"), "{out}");
}

fn lines_of(text: &[&str]) -> Vec<Line> {
    text.iter()
        .map(|text| match text.strip_suffix(':') {
            Some(label) => Line::Label(label.to_owned()),
            None => Line::Inst((*text).to_owned()),
        })
        .collect()
}

/// A loop that counts 41 trips costs 41 trips of its body, not eight: the
/// 10-trip loop `a` saved 4 T in and the 41-trip loop that lost 3 T a trip
/// weighed alike (byval_struct -O1, +83 T for -1 B).
#[test]
fn a_counted_loop_is_priced_by_its_trips() {
    let loop_of = |trips: &str| {
        let init = format!("ld a, #{trips}");
        price_in_loops(&lines_of(&[&init, "L0_1:", "inc hl", "dec a", "jr nz, L0_1"]))
    };
    let (few, many) = (loop_of("10"), loop_of("40"));
    assert!(many.1 > 3 * few.1, "10 trips {few:?}, 40 trips {many:?}");
}

/// The same 20 trips written another way weigh the same: loads between the
/// `dec a` and the `jr nz` (wide_frame -O1) and a jump into the loop at its
/// test with the counter kept in the frame (memcpy_libcall -O1) read as
/// uncounted, eight trips, and priced 2x to 25x too low.
#[test]
fn a_counted_loop_weighs_the_same_in_any_shape() {
    let plain = price_in_loops(&lines_of(&["ld a, #200", "L0_1:", "inc hl", "dec a", "jr nz, L0_1"]));
    let loads_between =
        price_in_loops(&lines_of(&["ld a, #200", "L0_1:", "inc hl", "dec a", "ld c, l", "ld b, h", "jr nz, L0_1"]));
    let entered_at_the_test = price_in_loops(&lines_of(&[
        "ld a, #200", "ld -1(ix), a", "jr L0_2", "L0_1:", "inc hl", "L0_2:", "dec a", "jr nz, L0_1",
    ]));
    // counted: 22 or 30 T a trip for 200 trips; read as uncounted, 32 trips, they stay under 1000
    assert!(plain.1 > 2000, "plain {plain:?}");
    assert!(loads_between.1 > 2000, "loads {loads_between:?}");
    assert!(entered_at_the_test.1 > 2000, "entered {entered_at_the_test:?}");
}

/// A loop one variant counts weighs the same in the variant that closes it
/// another way: loop_interchange -O1 closed its loop by `jp` after a test at
/// its top, was priced at 8 trips against 20, and came out 32 B and 78416 T
/// worse than the variant it replaced.
#[test]
fn a_loop_counted_in_one_variant_weighs_the_same_in_another() {
    forget_trips();
    let counted = lines_of(&["ld a, #100", "L0_1:", "inc hl", "dec a", "jr nz, L0_1"]);
    let tested_at_the_top = lines_of(&["ld a, #100", "L0_1:", "inc hl", "dec a", "jr z, L0_9", "jp L0_1", "L0_9:"]);
    let before = price_emitted(&tested_at_the_top, true);
    price_emitted(&counted, true);
    let after = price_emitted(&tested_at_the_top, true);
    forget_trips();
    assert!(after.1 > 2 * before.1, "before {before:?}, after {after:?}");
}

/// A loop nothing counts runs the pin's 32 trips (its back edge is taken 124 in
/// 128), not the 8 this port had: split_loops -O1 ranked a variant saving 4 T
/// in a loop of 32 below one costing 3 T in a loop of 10, +870 T.
#[test]
fn a_loop_nothing_counts_runs_thirty_two_trips() {
    let uncounted = price_in_loops(&lines_of(&["L0_1:", "inc hl", "jp L0_1"]));
    assert_eq!(uncounted.1, 32 * (6 + 10), "{uncounted:?}");
}
