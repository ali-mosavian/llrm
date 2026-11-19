//! `(a + b) >> 1` of two bytes keeps the sum's ninth bit in C: `adc; ror a`.
//! Before, the sum was widened to a word, shifted in two cells and read back
//! (bsearch8's midpoint: 19 bytes where `clc; adc; ror a` is 4, 2026-11-08).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn text(
    ir: &str,
    name: &str,
) -> String {
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    text[from..].split(".segment").next().unwrap().to_owned()
}

const MIDPOINT: &str = "define i8 @mid(i8 %a, i8 %b) {
  %x = zext i8 %a to i16
  %y = zext i8 %b to i16
  %s = add nuw nsw i16 %x, %y
  %h = lshr i16 %s, 1
  %r = trunc i16 %h to i8
  ret i8 %r
}";

#[test]
fn the_floor_average_of_two_bytes_rotates_the_carry_in() {
    let asm = text(MIDPOINT, "mid");
    assert!(asm.contains("  clc\n") && asm.contains("  adc ") && asm.contains("  ror a\n"), "{asm}");
    assert!(!asm.contains("lsr") && !asm.contains("rol"), "the ninth bit went through a word: {asm}");
}

#[test]
fn the_ceiling_average_of_two_bytes_sets_the_carry_first() {
    let ir = MIDPOINT
        .replace("@mid", "@ceil")
        .replace("%h = lshr i16 %s, 1", "%t = add nuw nsw i16 %s, 1\n  %h = lshr i16 %t, 1");
    let asm = text(&ir, "ceil");
    assert!(asm.contains("  sec\n") && asm.contains("  ror a\n"), "{asm}");
    assert!(!asm.contains("lsr") && !asm.contains("rol"), "{asm}");
}

/// Every pair of bytes through `mid` and `ceil` on sim65, against the sum
/// shifted as a word: the bench's one input never carries out of the add, so
/// it could not see a lost ninth bit.
#[test]
fn the_averages_are_right_for_all_65536_pairs() {
    let ir = format!(
        "{MIDPOINT}\n{}",
        MIDPOINT
            .replace("@mid", "@ceil")
            .replace("%h = lshr i16 %s, 1", "%t = add nuw nsw i16 %s, 1\n  %h = lshr i16 %t, 1")
    );
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let compiled = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let functions: String =
        compiled.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
    // a, b, the reference sum's low and high bytes; `bad` is set by any
    // mismatch
    let main = "  lda #0\n  sta bad\n  sta ia\nouter:\n  lda #0\n  sta ib\ninner:\n".to_owned()
        + &["mid", "ceil"]
            .iter()
            .enumerate()
            .map(|(n, name)| {
                let carry = if *name == "mid" { "clc" } else { "sec" };
                format!(
                    "  {carry}\n  lda ia\n  adc ib\n  sta lo\n  lda #0\n  rol a\n  sta hi\n  lsr hi\n  ror lo\n  \
                     lda ia\n  ldx ib\n  jsr {name}\n  cmp lo\n  beq ok{n}\n  lda #1\n  sta bad\nok{n}:\n"
                )
            })
            .collect::<String>()
        + "  inc ib\n  bne inner\n  inc ia\n  bne outer\n  lda bad\n";
    let extra = format!(
        "{functions}.segment \"BSS\"\nia: .res 1\nib: .res 1\nlo: .res 1\nhi: .res 1\nbad: .res 1\n.segment \"CODE\"\n"
    );
    for target in ["sim6502", "sim65c02"] {
        assert_eq!(common::run_on(target, &format!("avg-{target}"), &main, &extra).0, 0, "{target}");
    }
    // the instrument: the loop ran (a driver that fell through would also exit
    // 0)
    let cycles = common::cycles_on("sim6502", "avg-cycles", &main, &extra);
    assert!(cycles > 5_000_000, "the driver ran {cycles} cycles for 131072 calls");
}
