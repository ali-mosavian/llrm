//! The page walk of `strncmp` (one byte more, 108 clocks fewer: 84 B / 1189
//! clocks against 83 B / 1297) was priced at the copy rate, 500 clocks a byte,
//! and refused; it is priced at the scoreboard's 1 B = 3 clocks (2026-11-14).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const STRNCMP: &str = "define i16 @k_strncmp(ptr %a, ptr %b, i16 %n) {
entry:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %done, label %pre
pre:
  %n1 = add i16 %n, -1
  br label %head
head:
  %k = phi i16 [ %n1, %pre ], [ %k2, %latch ]
  %q = phi ptr [ %q2, %latch ], [ %b, %pre ]
  %p = phi ptr [ %p2, %latch ], [ %a, %pre ]
  %x = load i8, ptr %p
  %y = load i8, ptr %q
  %e = icmp eq i8 %x, %y
  br i1 %e, label %latch, label %diff
diff:
  %lt = icmp ult i8 %x, %y
  %v = select i1 %lt, i16 -1, i16 1
  br label %done
latch:
  %nul = icmp eq i8 %x, 0
  %p2 = getelementptr inbounds nuw i8, ptr %p, i16 1
  %q2 = getelementptr inbounds nuw i8, ptr %q, i16 1
  %last = icmp eq i16 %k, 0
  %stop = select i1 %nul, i1 true, i1 %last
  %k2 = add i16 %k, -1
  br i1 %stop, label %done, label %head
done:
  %r = phi i16 [ %v, %diff ], [ 0, %entry ], [ 0, %latch ]
  ret i16 %r
}
";

#[test]
fn a_counted_pointer_walk_steps_one_index() {
    let ir = format!("target datalayout = \"{}\"\n{STRNCMP}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    assert!(body.contains("iny"), "the loop steps both pointers each trip:\n{body}");
}

/// -O1 keeps no more bytes: the walk is taken there once it is the smaller
/// loop (it was one byte more, and huffman_decode grew 1233 B to 1235 B).
#[test]
fn at_o1_a_page_walk_that_is_no_larger_is_taken() {
    let ir = format!("target datalayout = \"{}\"\n{STRNCMP}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O1)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    assert!(body.contains("iny"), "-O1 kept the loop as written:\n{body}");
}

/// The counter's step sat before the exit test, so the old and the new value
/// lived side by side and were shuffled through two more cells each trip
/// (k_strncmp 84 B / 1189 clocks); the step moves to the successor that reads
/// it, in place.
#[test]
fn a_counted_loop_steps_its_counter_in_place() {
    let ir = format!("target datalayout = \"{}\"\n{STRNCMP}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    let size = llrm_6502::timing::estimate(llrm_6502::dagisel::hooks::Cpu::Mos6502, body).unwrap().bytes;
    assert!(size <= 72, "{size} B:\n{body}");
}
