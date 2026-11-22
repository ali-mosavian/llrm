//! A global operand of an add or subtract is the `adc`/`sbc` operand itself.

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

/// `t + tab[i & 15]` on longs: the carry chain loaded each `tab` byte into a
/// register first (`lda tab,x; tay; ...; adc` per byte, `frameless`'s `acc`,
/// +6 B and +4% clocks); `adc tab,x` takes it directly.
#[test]
fn a_long_add_of_a_global_element_uses_adc_absolute_x() {
    let ir = "@tab = global [16 x i32] zeroinitializer
define i32 @acc(i32 %t, i8 %i) {
  %m = and i8 %i, 15
  %x = zext i8 %m to i16
  %p = getelementptr i32, ptr @tab, i16 %x
  %v = load i32, ptr %p
  %s = add i32 %t, %v
  ret i32 %s
}";
    let text = compile(ir, "acc");
    for form in ["adc tab,x", "adc tab+1,x"] {
        assert!(text.contains(form), "{form}\n{text}");
    }
}
