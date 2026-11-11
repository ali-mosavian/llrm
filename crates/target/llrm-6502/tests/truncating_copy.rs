//! At -O0 a word truncated to a byte is a copy of a pair into `a`. It was
//! emitted as a pair-to-pair copy with `a` as a pair: `sta
//! rc18446744073709551606`, a cell no program has, undefined at assembly.

mod common;

use llrm_6502::driver::{Level, Request, assembly};

const LOW_BIT: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8\"

define i8 @low(ptr %p) {
entry:
  %a = ptrtoint ptr %p to i16
  %b = and i16 %a, 1
  %c = trunc i16 %b to i8
  ret i8 %c
}

define i8 @frame() {
entry:
  %x = alloca i16, align 1
  %a = ptrtoint ptr %x to i16
  %b = and i16 %a, 1
  %c = trunc i16 %b to i8
  ret i8 %c
}
";

#[test]
fn a_word_truncated_to_a_byte_at_o0_names_only_cells_that_exist() {
    let text = assembly(LOW_BIT, &Request::new(Level::O0)).unwrap();
    for cell in text.split(|c: char| !c.is_ascii_alphanumeric()).filter_map(|word| word.strip_prefix("rc")) {
        if let Ok(n) = cell.parse::<u64>() {
            assert!(n < 256, "rc{n}: {text}");
        }
    }
}

#[test]
fn the_low_byte_of_a_frame_address_at_o0_assembles_and_runs() {
    let text = assembly(LOW_BIT, &Request::new(Level::O0)).unwrap();
    let functions: String =
        text.lines().filter(|line| !line.contains(" = $")).map(|line| format!("{line}\n")).collect();
    // rs0 = $F000 and a two-byte frame: the address is even
    let main = "  lda #0\n  sta rc0\n  lda #$F0\n  sta rc1\n  jsr frame\n";
    assert_eq!(common::run_with("truncating-copy", main, &functions), 0);
}
