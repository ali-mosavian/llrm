//! A function whose long adds are carry ops is priced against the generic form
//! and keeps the cheaper.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::{Line, price, price_emitted};

const DATALAYOUT: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n";

fn lines_of(text: &str) -> Vec<Line> {
    text.lines().filter(|one| one.starts_with('\t')).map(|one| Line::Inst(one.trim().to_owned())).collect()
}

/// `descend` of register_upper_half (-Os) with its long adds as carry ops was
/// 243 B / 1504 T: three long values live across a recursive call, and `adc hl,
/// rr` ties each sum to `hl`, so the allocation spilled more than the
/// generic add and compare of the sum, which is 215 B / 1339 T. The cell was
/// 350 B against SDCC's 323 B.
#[test]
fn a_function_the_carry_ops_make_larger_keeps_the_generic_form() {
    let ir = include_str!("data/register_upper_half_os.ll");
    let out = text(ir, "descend", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let (bytes, clocks) = price(&lines_of(&out));
    assert!(bytes <= 215 && clocks <= 1339, "descend is {bytes} B / {clocks} T, was 215 B / 1339 T");
}

/// The words of a plain long add stay `add hl, rr` and `adc hl, rr`: the
/// generic form is only the alternative.
#[test]
fn a_plain_long_add_keeps_the_carry_ops() {
    let ir = format!("{DATALAYOUT}define i32 @sum(i32 %a, i32 %b) {{\n  %c = add i32 %a, %b\n  ret i32 %c\n}}\n");
    for level in [Level::O1, Level::Os, Level::O2] {
        let out = text(&ir, "sum", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{why}"));
        assert!(out.contains("adc hl,"), "{level:?}: the long add has no `adc hl`:\n{out}");
    }
}

/// -O1 is strict on both axes. `main` of byte_multiply with the generic form of
/// its long adds is smaller but slower (the program 340 B against 349 B, +13007
/// T, 1.2%): the regret rule took it, giving up the clocks the carry
/// ops win. It is taken only where it is no worse on either axis. Priced as the
/// choice prices it (as emitted, by loop).
#[test]
fn the_generic_form_is_taken_at_o1_only_if_no_slower() {
    let ir = include_str!("data/byte_multiply_o1.ll");
    let lines = llrm_z80::compile::lines(ir, "main", 0, &Request::new(Level::O1)).unwrap_or_else(|why| panic!("{why}"));
    let (bytes, clocks) = price_emitted(&lines, true);
    assert!(clocks <= 1_082_902, "main is {bytes} B / {clocks} T at -O1, the carry ops' form is 349 B / 1082902 T");
}
