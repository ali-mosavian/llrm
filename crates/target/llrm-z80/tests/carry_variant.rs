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
#[ignore = "row Z80-RESULT-ORDER-DESCEND"]
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

/// The levels strict on both axes allow no trade. `main` of object_extent_loop
/// (-Os) with the generic form of its long adds is a byte smaller and slower
/// (+4896 T over the program): taken only where it is no worse on either axis.
/// Priced as the choice prices it (as emitted, by loop).
#[test]
#[ignore = "row Z80-OEL-OS-T"]
fn the_generic_form_is_taken_at_a_strict_level_only_if_no_slower() {
    let ir = include_str!("data/object_extent_loop_os.ll");
    let lines = llrm_z80::compile::lines(ir, "main", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let (bytes, clocks) = price_emitted(&lines, false);
    assert!(clocks <= 332_349, "main is {bytes} B / {clocks} T at -Os, the carry ops' form is 703 B / 332349 T");
}

/// call_keeps -O1: `churn` and `hold` have a multiply alternative (all
/// multiplies calls) and carry ops. The all-calls form with carry ops is 953 B
/// against 888 B with the generic add, and the inline form shrank from 1285 B
/// to 1194 B, so the regret rule flipped to the inline form: the cell went 1473
/// B / 64413 T to 1740 B / 55322 T (SDCC 1535 B). The carry choice is made for
/// each form, before the inline-or-calls choice.
#[test]
fn a_function_with_a_multiply_alternative_resolves_its_carry_ops_per_form() {
    let ir = include_str!("data/call_keeps_o1.ll");
    let size = |name: &str| {
        let lines =
            llrm_z80::compile::lines(ir, name, 0, &Request::new(Level::O1)).unwrap_or_else(|why| panic!("{why}"));
        price_emitted(&lines, true)
    };
    let (churn, hold) = (size("churn"), size("hold"));
    // `hold` is 453 B since a doubled long is one `SHL32`: its inline form (`x
    // * 3` as `add hl,hl; rl c; rl b`) is cheap enough for the O1 regret
    // rule (Z80-SHL32-WORSE, call_keeps O1 +28 B / -789 T)
    assert!(churn.0 <= 888 && hold.0 <= 453, "churn {churn:?} hold {hold:?}, the all-calls forms are 888 B and 434 B");
}
