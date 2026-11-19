//! What the selection prices the 16-bit divides at is read from the runtime's
//! text (`routine_clocks`), not kept as numbers beside it. `SAVED_AT_SPEED_16`
//! was 215 and `CLOCKS_COMPACT_16` 896, measured on sim65 before the divide
//! loop was edited twice: the runtime then took 159 and 767.

mod common;

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::hotdivision::{SHAPES_16, clocks_compact_16, saved_at_speed_16};
use llrm_6502::routine_clocks::run;

fn cells((a, b): (u16, u16)) -> [(u8, u8); 4] {
    [(2, a as u8), (3, (a >> 8) as u8), (4, b as u8), (5, (b >> 8) as u8)]
}

/// sim65's clocks of one `jsr routine`, the argument setup taken off. The run
/// leaves out the `jsr` (6) that sim65 counts.
fn sim65(
    tag: &str,
    routine: &str,
    shape: (u16, u16),
) -> i64 {
    let mut setup = String::new();
    for (n, value) in cells(shape) {
        setup += &format!("  lda #{value}\n  sta rc{n}\n");
    }
    let name = format!("pricing-{tag}-{routine}-{}-{}", shape.0, shape.1);
    common::cycles_with(&name, &format!("{setup}  jsr {routine}\n"), "")
        - common::cycles_with(&format!("{name}-setup"), &setup, "")
        - 6
}

/// The model of the routine returns the quotient the routine returns, and its
/// clocks are sim65's to within the page crossings sim65 adds.
#[test]
fn the_run_of_a_divide_gives_its_quotient_and_sim65s_clocks() {
    for routine in ["__udivhi3c", "__udivhi3"] {
        for shape in SHAPES_16 {
            let priced = run(Cpu::Mos6502, routine, &cells(shape)).unwrap();
            let quotient = i32::from(shape.0 / shape.1);
            assert_eq!(i32::from(priced.x) << 8 | i32::from(priced.a), quotient, "{routine} {shape:?}");
            let measured = sim65("run", routine, shape);
            let slack = measured / 50;
            assert!(
                (priced.clocks as i64 - measured).abs() <= slack,
                "{routine} {shape:?}: priced {} clocks, sim65 {measured}",
                priced.clocks
            );
        }
    }
}

/// The figures `select::speed_division16_pays` prices with are the runtime's:
/// the mean over the shapes of the compact divide and of what the speed
/// divide saves, on sim65.
#[test]
fn the_priced_division_clocks_are_what_sim65_takes_over_the_shapes() {
    let mean = |routine: &str| {
        SHAPES_16.iter().map(|&shape| sim65("mean", routine, shape) as f64).sum::<f64>() / SHAPES_16.len() as f64
    };
    let (compact, speed) = (mean("__udivhi3c"), mean("__udivhi3"));
    let (priced_compact, priced_saved) = (clocks_compact_16(Cpu::Mos6502), saved_at_speed_16(Cpu::Mos6502));
    assert!((priced_compact - compact).abs() <= compact / 50.0, "compact: priced {priced_compact}, sim65 {compact}");
    assert!(
        (priced_saved - (compact - speed)).abs() <= 10.0,
        "saved: priced {priced_saved}, sim65 {}",
        compact - speed
    );
}
