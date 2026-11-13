//! A store at `d + 650` made a register of the 650 (`move.w #650, %a1; move.l
//! #x, (0,%a0,%a1)`, 12 B): the pin's `ARII` matches before `ARID`, and a
//! constant past the 8-bit displacement became the index. `move.l #x,
//! (650,%a0)` is 8 B and needs no register (memory_expansion: eight such stores
//! per memset).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define void @u(ptr %d) {
  %p = getelementptr i8, ptr %d, i32 650
  store i32 858993459, ptr %p, align 2
  %q = getelementptr i8, ptr %d, i32 654
  store i32 858993459, ptr %q, align 2
  ret void
}
";

#[test]
fn a_constant_offset_past_a_byte_is_a_displacement_not_an_index_register() {
    for cpu in ["68000", "68020"] {
        let out = assembly(IR, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(out.contains("(650,%a0)") && out.contains("(654,%a0)"), "{cpu}:\n{out}");
        assert!(!out.contains("move.w\t#65"), "{cpu}: a register for the offset:\n{out}");
    }
}
