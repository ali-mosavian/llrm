//! k_strnicmp at -Os: two case folds a trip, each a range test and a select of
//! `c - 32`.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

const STRNICMP: &str = "define i16 @k_strnicmp(ptr %0, ptr %1, i16 %2) {
entry:
  %z = icmp eq i16 %2, 0
  br i1 %z, label %done, label %loop
loop:
  %n = phi i16 [ %2, %entry ], [ %n1, %next ]
  %p = phi ptr [ %0, %entry ], [ %p1, %next ]
  %q = phi ptr [ %1, %entry ], [ %q1, %next ]
  %a = load i8, ptr %p, align 1
  %b = load i8, ptr %q, align 1
  %a0 = add i8 %a, -97
  %ac = icmp ult i8 %a0, 26
  %a1 = add i8 %a, -32
  %af = select i1 %ac, i8 %a1, i8 %a
  %b0 = add i8 %b, -97
  %bc = icmp ult i8 %b0, 26
  %b1 = add i8 %b, -32
  %bf = select i1 %bc, i8 %b1, i8 %b
  %same = icmp eq i8 %af, %bf
  br i1 %same, label %next, label %differ
differ:
  %lt = icmp ult i8 %af, %bf
  %r = select i1 %lt, i16 -1, i16 1
  br label %done
next:
  %p1 = getelementptr inbounds nuw i8, ptr %p, i16 1
  %q1 = getelementptr inbounds nuw i8, ptr %q, i16 1
  %n1 = add i16 %n, -1
  %zero = icmp eq i8 %af, 0
  %end = icmp eq i16 %n1, 0
  %stop = or i1 %zero, %end
  br i1 %stop, label %done, label %loop
done:
  %res = phi i16 [ 0, %entry ], [ %r, %differ ], [ 0, %next ]
  ret i16 %res
}
";

/// The folds' subtractions were made ahead of their selects, both, every trip:
/// 123 B against llvm-mos's 104 (matrix k_strnicmp Os).
fn k_strnicmp_bytes() -> (u32, String) {
    let ir = format!("target datalayout = \"{}\"\n{STRNICMP}", datalayout());
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("k_strnicmp:").unwrap_or_else(|| panic!("no k_strnicmp in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    (bytes, body)
}

/// The bar. #9525 (LSR keeps 16-bit widths at -Os) took it from 104 to 109 B:
/// row 6502-LSR-SIZE.
#[test]
#[ignore = "6502-LSR-SIZE: 109 B against 104, exit 2026-11-24"]
fn a_case_fold_at_os_costs_no_more_than_llvm_mos_104_bytes() {
    let (bytes, body) = k_strnicmp_bytes();
    assert!(bytes <= 104, "{bytes} B\n{body}");
}

/// The row's own number: a further regression is not hidden by the row.
#[test]
fn a_case_fold_at_os_stays_within_its_row_109_bytes() {
    let (bytes, body) = k_strnicmp_bytes();
    assert!(bytes <= 109, "{bytes} B\n{body}");
}
