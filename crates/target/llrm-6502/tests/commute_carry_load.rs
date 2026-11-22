//! A wide sum whose loaded array element is the first operand of the carry
//! chain (`total + buf[i]` written `buf[i] + total`) read the bytes where they
//! lie only when the load was the second operand: the matcher commutes `uaddo`
//! but not `uaddo_carry`, so the high bytes were loaded into cells first (`lda
//! buf+2,x; sta rc7; ..; lda rc7; adc rc2`, 4 instructions and 14 clocks a trip
//! in ring's loop).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

#[test]
fn a_loaded_element_summed_first_is_read_where_it_lies() {
    let ir = format!(
        "target datalayout = \"{}\"\n@buf = global [64 x i32] zeroinitializer\n\
         define i32 @f(i32 %t, i8 %i) {{\n  %m = and i8 %i, 63\n  %z = zext i8 %m to i16\n\
           %p = getelementptr [64 x i32], ptr @buf, i16 0, i16 %z\n  %v = load i32, ptr %p\n\
           %r = add i32 %v, %t\n  ret i32 %r\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let summed = text.lines().filter(|line| line.trim_start().starts_with("adc buf")).count();
    assert_eq!(summed, 4, "{text}");
}
