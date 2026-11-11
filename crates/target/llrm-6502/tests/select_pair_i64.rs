//! Two 64-bit selects on one compare, then a diamond per operand: the second operand read the first's value
//! (`double` add: 1.5 + 0.25 came out as 0.5, 2026-11-11). The mid-end IR below is what `swap; unpack; unpack` of
//! the soft-float add reduces to.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const IR: &str = "define internal i64 @f(i64 %0, i64 %1) noinline {
  %3 = and i64 %0, 9223372036854775807
  %4 = and i64 %1, 9223372036854775807
  %5 = icmp ult i64 %3, %4
  %6 = select i1 %5, i64 %0, i64 %1
  %7 = select i1 %5, i64 %1, i64 %0
  %8 = and i64 %7, 4503599627370495
  %9 = and i64 %7, 9218868437227405312
  %10 = icmp eq i64 %9, 0
  br i1 %10, label %13, label %11
11:
  %12 = or disjoint i64 %8, 4503599627370496
  br label %13
13:
  %14 = phi i64 [ %12, %11 ], [ %8, %2 ]
  %15 = and i64 %6, 4503599627370495
  %16 = and i64 %6, 9218868437227405312
  %17 = icmp eq i64 %16, 0
  br i1 %17, label %20, label %18
18:
  %19 = or disjoint i64 %15, 4503599627370496
  br label %20
20:
  %21 = phi i64 [ %19, %18 ], [ %15, %13 ]
  %22 = add nuw nsw i64 %21, %14
  ret i64 %22
}
define i16 @probe() {
  %pa = alloca i64
  %pb = alloca i64
  store volatile i64 4609434218613702656, ptr %pa
  store volatile i64 4598175219545276416, ptr %pb
  %a = load volatile i64, ptr %pa
  %b = load volatile i64, ptr %pb
  %r = call i64 @f(i64 %a, i64 %b)
  %s = lshr i64 %r, 48
  %t = trunc i64 %s to i16
  ret i16 %t
}";

/// 0x3FF8.. (1.5) and 0x3FD0.. (0.25): the sum of the two mantissas is 0x0028 in the top word; the bug gave 0x0030.
#[test]
fn the_second_of_two_selected_i64_operands_is_not_the_first() {
    let ir = format!("target datalayout = \"{}\"\n{IR}", datalayout());
    for level in [Level::O1, Level::O2, Level::Os] {
        let compiled = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        let functions: String = compiled
            .lines()
            .filter(|l| !l.starts_with("rc") || !l.contains(" = $"))
            .map(|l| format!("{l}\n"))
            .collect();
        for target in ["sim6502", "sim65c02"] {
            let name = format!("selpair-{target}-{level:?}");
            assert_eq!(common::run_on(target, &name, "  jsr probe\n", &functions).0, 0x28, "{target} {level:?}");
        }
    }
}
