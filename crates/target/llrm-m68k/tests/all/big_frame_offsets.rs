//! A constant offset past 127 into a stack array was an index register: the
//! pin's `matchAddressBase` makes a constant that does not fit the 8-bit
//! displacement the index of `(d8,An,Xn)`, so every store was
//! `move.w #248,%a1; move.l #63,(0,%a2,%a1)` (frameless -O3 68020: the 64
//! unrolled stores of a local array, 4% over gcc). The offset is a
//! displacement of the 16-bit form: `move.l #63,(248,%sp)`.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @use(ptr)
define void @f() {
  %buf = alloca [64 x i32], align 4
  %a = getelementptr i8, ptr %buf, i32 248
  store i32 63, ptr %a
  %b = getelementptr i8, ptr %buf, i32 244
  store i32 62, ptr %b
  call void @use(ptr %buf)
  ret void
}
";

#[test]
fn a_store_past_127_bytes_into_a_stack_array_is_a_displacement_not_an_index() {
    for cpu in ["68000", "68020"] {
        let out = assembly(IR, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(!out.contains("%a1"), "{cpu}: an index register holds the offset:\n{out}");
    }
}
