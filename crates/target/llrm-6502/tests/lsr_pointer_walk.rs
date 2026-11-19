//! LSR on a loop that indexes a pointer: `s[n]` against a pointer that steps.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `strnlen`: the byte at `s + n` is read every trip, `n` also bounds the loop.
const EVERY_TRIP: &str = "define i16 @f(ptr %s, i16 %max) {\n\
    entry:\n  %z = icmp eq i16 %max, 0\n  br i1 %z, label %out, label %loop\n\
    loop:\n  %n = phi i16 [ 0, %entry ], [ %n1, %next ]\n  %p = getelementptr inbounds nuw i8, ptr %s, i16 %n\n  \
    %c = load i8, ptr %p, align 1\n  %e = icmp eq i8 %c, 0\n  br i1 %e, label %done, label %next\n\
    next:\n  %n1 = add nuw i16 %n, 1\n  %lt = icmp ult i16 %n1, %max\n  br i1 %lt, label %loop, label %done\n\
    done:\n  %r = phi i16 [ %n, %loop ], [ %max, %next ]\n  br label %out\n\
    out:\n  %res = phi i16 [ 0, %entry ], [ %r, %done ]\n  ret i16 %res\n}";

/// The same read on one trip in four (`n & 3 == 0`).
const SOME_TRIPS: &str = "define i16 @f(ptr %s, i16 %max) {\n\
    entry:\n  %z = icmp eq i16 %max, 0\n  br i1 %z, label %out, label %loop\n\
    loop:\n  %n = phi i16 [ 0, %entry ], [ %n1, %next ]\n  %q = and i16 %n, 3\n  %g = icmp eq i16 %q, 0\n  \
    br i1 %g, label %use, label %next\n\
    use:\n  %p = getelementptr inbounds nuw i8, ptr %s, i16 %n\n  %c = load i8, ptr %p, align 1\n  \
    %e = icmp eq i8 %c, 0\n  br i1 %e, label %done, label %next\n\
    next:\n  %n1 = add nuw i16 %n, 1\n  %lt = icmp ult i16 %n1, %max\n  br i1 %lt, label %loop, label %done\n\
    done:\n  %r = phi i16 [ %n, %use ], [ %max, %next ]\n  br label %out\n\
    out:\n  %res = phi i16 [ 0, %entry ], [ %r, %done ]\n  ret i16 %res\n}";

fn compile(function: &str) -> String {
    let ir = format!("target datalayout = \"{}\"\n{function}\n", datalayout());
    assembly(&ir, &Request::new(Level::O2)).unwrap()
}

/// k_strnlen -O2 added `s + n` (`clc; lda; adc; sta` twice, 12 B) every trip
/// and kept the counter too: 69 B and 917 clk (sdcc 60 B, cc65 hand 608 clk).
/// LSR now walks a pointer beside the counter, as it does for the same loop
/// written with a pointer: 50 B, 607 clk.
#[test]
fn an_indexed_read_every_trip_walks_a_pointer() {
    let text = compile(EVERY_TRIP);
    assert!(!text.contains("adc"), "the address is added each trip:\n{text}");
}

/// A pointer that steps costs an add on every trip; a read on one in four does
/// not repay it (switch_peeled_table: +64 B, +20% clk when it did).
#[test]
fn an_indexed_read_on_some_trips_keeps_the_index() {
    let text = compile(SOME_TRIPS);
    assert!(text.contains("adc"), "a pointer walks every trip for a read on a few:\n{text}");
}
