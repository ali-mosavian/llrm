//! A byte index scaled by its element, into a small global, is `abs,x`.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const TABLE: &str = "@g = global [8 x [4 x i8]] zeroinitializer
define i8 @f(i8 %x) {
  %z = zext i8 %x to i16
  %p = getelementptr inbounds nuw [4 x i8], ptr @g, i16 %z
  %v = load i8, ptr %p, align 1
  ret i8 %v
}";

fn compile(level: Level) -> String {
    let ir = format!("target datalayout = \"{}\"\n{TABLE}\n", datalayout());
    assembly(&ir, &Request::new(level)).unwrap()
}

/// `la[i]` of sat_builtins: the element is 4 bytes, so the index was `zext i8
/// << 2` in a word, built with 16-bit `asl; rol` twice and added to the address
/// in a pointer pair (13 B and `(zp),y` a site). The extent of `@g` (32 B)
/// makes the offset a byte: `asl a; asl a; tax; lda g,x`.
#[test]
fn a_scaled_byte_index_into_a_small_global_is_absolute_x() {
    for level in [Level::O2, Level::Os] {
        let text = compile(level);
        assert!(text.contains("g,x") && !text.contains("adc"), "{level:?}:\n{text}");
    }
}
