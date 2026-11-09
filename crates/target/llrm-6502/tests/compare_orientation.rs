//! `a != b` is the same test with the operands the other way round; which of
//! the two is compared in a register is decided by what the rest of the
//! function does with them.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// Euclid's loop: `a > b` reads `a` in a register and `b` in a cell, and the `a
/// != b` that closes the loop reads them the other way round.
const GCD: &str = "define zeroext i8 @gcd(i8 zeroext %0, i8 zeroext %1) {
  %3 = icmp eq i8 %0, %1
  br i1 %3, label %17, label %4
4:
  br label %5
5:
  %6 = phi i8 [ %15, %13 ], [ %1, %4 ]
  %7 = phi i8 [ %14, %13 ], [ %0, %4 ]
  %8 = icmp ugt i8 %7, %6
  br i1 %8, label %9, label %11
9:
  %10 = sub nuw i8 %7, %6
  br label %13
11:
  %12 = sub nuw i8 %6, %7
  br label %13
13:
  %14 = phi i8 [ %10, %9 ], [ %7, %11 ]
  %15 = phi i8 [ %6, %9 ], [ %12, %11 ]
  %16 = icmp eq i8 %14, %15
  br i1 %16, label %17, label %5
17:
  %18 = phi i8 [ %0, %2 ], [ %14, %13 ]
  ret i8 %18
}
";

/// The closing compare of `gcd` was `stx rc3; ldy rc2; cpy rc3`: each operand
/// copied to the side the other compare had not put it, 40 B and 158 clocks for
/// what `cpx rc2` is.
#[test]
fn an_equality_compare_takes_the_operand_order_the_registers_already_have() {
    let ir = format!("target datalayout = \"{}\"\n{GCD}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("gcd:").unwrap_or_else(|| panic!("no gcd in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= 37, "gcd: {bytes} B\n{body}");
}
