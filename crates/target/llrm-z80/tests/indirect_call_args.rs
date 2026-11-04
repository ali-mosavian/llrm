//! An indirect call with three word arguments failed with "greedy could not
//! allocate": the callee pointer, copied to IY right before `call (iy)`, was
//! live in a pair while HL, DE and BC all held arguments (`sbb_wide_sub`).

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"
@t = global ptr null
define void @main() {
  %f = load volatile ptr, ptr @t
  call void %f(i16 5, i16 7, i16 9)
  ret void
}
";

#[test]
fn an_indirect_call_with_three_word_arguments_allocates() {
    text(IR, "main", 0, &Request::new(Level::O2)).expect("compiles");
}
