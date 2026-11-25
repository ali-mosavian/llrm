//! Multiplies by a constant outside every loop became calls of `__mulsi3`
//! whenever the module was fewer bytes so, at every level: six of them saved
//! 136 B and cost some 11,000 clocks at -O2, where the judge prices a byte at
//! three clocks (call_keeps -O2: 21,053 clocks against 16,211 for llvm-mos,
//! whose in-line expansion it matched on bytes). -O1 keeps bytes first.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn straight_line() -> String {
    format!(
        "target datalayout = \"{}\"
define i32 @f(i32 %a, i32 %b, i32 %c, i32 %d) {{
  %p = mul i32 %a, 3
  %q = mul i32 %b, 5
  %r = mul i32 %c, 7
  %s = mul i32 %d, 5
  %t = mul i32 %a, 7
  %u = mul i32 %b, 3
  %v = add i32 %p, %q
  %w = add i32 %v, %r
  %x = add i32 %w, %s
  %y = add i32 %x, %t
  %z = add i32 %y, %u
  ret i32 %z
}}
",
        datalayout()
    )
}

fn calls(level: Level) -> usize {
    let text = assembly(&straight_line(), &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    text.matches("jsr __mulsi3").count()
}

#[test]
fn straight_line_multiplies_stay_in_line_at_the_speed_levels() {
    for level in [Level::O2, Level::O3] {
        assert_eq!(calls(level), 0, "{level:?}: multiplies by a constant became calls");
    }
}
