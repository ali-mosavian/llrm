//! LSR dropped every 16-bit pointer and index on this target (`n8`: no 16-bit
//! integer is legal), so `g[i]` for a 54-byte row called `__mulhi3` on each
//! trip: rep_count at -O1 spent 148,120 clocks where 133,418 do (2026-11-10).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `for (i = 0; i < n; i++) g[i][0] = 1;` over rows of 54 bytes.
const ROWS: &str = "@g = global [16 x [54 x i8]] zeroinitializer\n\
    define void @f(i16 %n) {\nentry:\n  %z = icmp eq i16 %n, 0\n  br i1 %z, label %out, label %l\n\
    l:\n  %i = phi i16 [0, %entry], [%i1, %l]\n  %p = getelementptr [54 x i8], ptr @g, i16 %i\n  \
    store i8 1, ptr %p\n  %i1 = add i16 %i, 1\n  %c = icmp ne i16 %i1, %n\n  br i1 %c, label %l, label %out\n\
    out:\n  ret void\n}\n";

#[test]
fn a_row_address_steps_by_the_row_size_and_calls_no_multiply() {
    let ir = format!("target datalayout = \"{}\"\n{ROWS}", datalayout());
    // -Os keeps the pin's widths (DEVIATE LSR-N holds at speed only).
    for level in [Level::O1, Level::O2] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        let body = text.split(".segment").next().unwrap();
        assert!(!body.contains("mulhi"), "{level:?}: {body}");
    }
}
