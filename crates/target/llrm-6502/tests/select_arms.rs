//! A select is a branch on the 6502; the arm only it reads is made on its own
//! path.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// `stricmp`'s case folds as the pin's `opt -O2` leaves them: a range test and
/// a select of `c - 32` for `c`, twice a trip.
const STRICMP: &str = "define i16 @k_stricmp(ptr %0, ptr %1) {
  br label %3
3:
  %4 = phi ptr [ %0, %2 ], [ %22, %20 ]
  %5 = phi ptr [ %1, %2 ], [ %21, %20 ]
  %6 = load i8, ptr %4, align 1
  %7 = load i8, ptr %5, align 1
  %8 = add i8 %6, -97
  %9 = icmp ult i8 %8, 26
  %10 = add i8 %6, -32
  %11 = select i1 %9, i8 %10, i8 %6
  %12 = add i8 %7, -97
  %13 = icmp ult i8 %12, 26
  %14 = add i8 %7, -32
  %15 = select i1 %13, i8 %14, i8 %7
  %16 = icmp eq i8 %11, %15
  br i1 %16, label %20, label %17
17:
  %18 = icmp ult i8 %11, %15
  %19 = select i1 %18, i16 -1, i16 1
  br label %24
20:
  %21 = getelementptr inbounds nuw i8, ptr %5, i16 1
  %22 = getelementptr inbounds nuw i8, ptr %4, i16 1
  %23 = icmp eq i8 %11, 0
  br i1 %23, label %24, label %3
24:
  %25 = phi i16 [ %19, %17 ], [ 0, %20 ]
  ret i16 %25
}
";

/// The folds computed `c + 159` and `c + 224` ahead of the select and chose
/// between them: 23 B a fold against llvm-mos's 11, 101 B for the routine
/// against its 70 (matrix k_stricmp O2).
#[test]
fn a_case_fold_makes_its_subtraction_on_the_taken_path() {
    let ir = format!("target datalayout = \"{}\"\n{STRICMP}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("k_stricmp:").unwrap_or_else(|| panic!("no k_stricmp in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= 90, "{bytes} B\n{body}");
}

/// k_itoa's digit loop: `u % r`, a `d > 9` fix-up whose arm is sunk, and `u /
/// r`. The pair is one runtime call in a block; the sunk arm split it into two
/// (k_itoa -O2: 2747 -> 4720 clocks, 374 -> 415 B).
const DIGITS: &str = "define i16 @k_digits(i16 %u0, ptr %buf, i16 %r) {
  br label %loop
loop:
  %u = phi i16 [ %u0, %0 ], [ %q, %loop ]
  %n = phi i8 [ 0, %0 ], [ %n1, %loop ]
  %m = urem i16 %u, %r
  %t = trunc i16 %m to i8
  %a = add i8 %t, 48
  %c = icmp ugt i8 %a, 57
  %b = add i8 %t, 55
  %s = select i1 %c, i8 %b, i8 %a
  %n1 = add i8 %n, 1
  %z = zext i8 %n to i16
  %g = getelementptr inbounds nuw i8, ptr %buf, i16 %z
  store i8 %s, ptr %g, align 1
  %q = udiv i16 %u, %r
  %e = icmp ugt i16 %r, %u
  br i1 %e, label %done, label %loop
done:
  ret i16 %q
}
";

#[test]
fn a_sunk_arm_does_not_part_a_quotient_from_its_remainder() {
    let ir = format!("target datalayout = \"{}\"\n{DIGITS}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let calls = text.lines().filter(|l| l.trim_start().starts_with("jsr __u")).count();
    assert_eq!(calls, 1, "{text}");
}
