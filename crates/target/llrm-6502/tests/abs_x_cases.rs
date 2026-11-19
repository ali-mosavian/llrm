//! Which `global + index` forms select `abs,x` (`SelectAbsX`).

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8";

fn compile(
    ir: &str,
    name: &str,
) -> String {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let selection = select_function(&module, name, Cpu::Mos6502).expect("selects");
    function_text(&module, name, &selection).expect("emits")
}

/// `t[i & 255]` on a word `i`: the index is a byte in a word, as
/// `SelectIndexed` already takes it for a pointer base. `SelectAbsX` took only
/// a literal `zext i8`, so the global's address went through a 16-bit
/// add and `(rc2),y` (11 instructions, 17 B against `ldx; lda t,x`).
#[test]
fn a_word_index_with_a_zero_high_byte_is_absolute_x() {
    let ir = "@t = global [200 x i8] zeroinitializer\ndefine i8 @f(i16 %i) {\n  %m = and i16 %i, 255\n  %p = getelementptr i8, ptr @t, i16 %m\n  %v = load i8, ptr %p\n  ret i8 %v\n}";
    let text = compile(ir, "f");
    assert!(text.contains("lda t,x") && !text.contains("adc"), "{text}");
}

/// `t[i & 63]` on a word array: a word is two byte loads, and the second is
/// `t+1,x`; the long is the same twice (`t+2,x`, `t+3,x`). Only a byte took
/// `abs,x`: the word went through a 16-bit add and `(rc2),y` (`frameless`'s
/// `ring`: `lda #<t; clc; adc; ...; lda (rc4),y` four times a trip, +75 B
/// against llvm-mos).
#[test]
fn a_word_or_long_indexed_by_a_byte_is_absolute_x() {
    let ir = "@t = global [64 x i32] zeroinitializer
define i32 @get(i16 %i) {
  %m = and i16 %i, 63
  %p = getelementptr i32, ptr @t, i16 %m
  %v = load i32, ptr %p
  ret i32 %v
}
define void @put(i16 %i, i32 %v) {
  %m = and i16 %i, 63
  %p = getelementptr i32, ptr @t, i16 %m
  store i32 %v, ptr %p
  ret void
}";
    for name in ["get", "put"] {
        let text = compile(ir, name);
        for part in ["t,x", "t+1,x", "t+2,x", "t+3,x"] {
            assert!(text.contains(part), "{name} {part}\n{text}");
        }
        assert!(!text.contains("adc"), "{name}\n{text}");
    }
}

/// A byte of a global indexed by a byte, widened and added to a long
/// (`t + buf[i]`): the zero- and any-extending load patterns had the pointer
/// forms only, so the address went through `lda #<buf; clc; adc i; ...;
/// ldy #0; lda (p),y` (memmove_overlap's `sum`: 24 clk a trip against 7 for
/// `ldx i; lda buf,x`).
#[test]
fn a_widened_byte_indexed_by_a_byte_is_absolute_x() {
    for extend in ["zext", "sext"] {
        let ir = format!(
            "@buf = global [32 x i8] zeroinitializer\ndefine i32 @f(i8 %i, i32 %t) {{\n  %x = zext i8 %i to i16\n  %p = getelementptr i8, ptr @buf, i16 %x\n  %v = load i8, ptr %p\n  %w = {extend} i8 %v to i32\n  %s = add i32 %t, %w\n  ret i32 %s\n}}"
        );
        let text = compile(&ir, "f");
        assert!(text.contains("lda buf,x") && !text.contains("(rc"), "{extend}:\n{text}");
    }
}
