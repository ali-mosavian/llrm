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
