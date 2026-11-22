//! queens (-Os 354 B against SDCC's 342): `q[r] == col` next to `d = q[r] -
//! col` was a second 6 B byte-wise compare of the two words. Reusing the
//! subtract, as X86 shares its flags, it is a test of `d` for zero.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define i16 @eq(i16 %a, i16 %b, ptr %p) nounwind {
  %d = sub i16 %a, %b
  store i16 %d, ptr %p
  %c = icmp eq i16 %a, %b
  br i1 %c, label %y, label %n
y:
  ret i16 1
n:
  ret i16 2
}
define i16 @ne(i16 %a, i16 %b, ptr %p) nounwind {
  %d = sub i16 %b, %a
  store i16 %d, ptr %p
  %c = icmp ne i16 %a, %b
  br i1 %c, label %y, label %n
y:
  ret i16 1
n:
  ret i16 2
}
"#;

#[test]
fn a_word_equality_next_to_the_difference_tests_the_difference() {
    for name in ["eq", "ne"] {
        for level in [Level::O1, Level::Os] {
            let asm = text(IR, name, 0, &Request::new(level)).expect("compiles");
            let compares = asm.lines().filter(|line| line.trim().starts_with("sub a,")).count();
            assert_eq!(compares, 0, "{name} {level:?} compares the words again:\n{asm}");
        }
    }
}
