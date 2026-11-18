//! A word `and` whose result is read as a byte is narrowed by
//! `ShrinkDemandedOp`; the fold `aext (and (trunc x), c)` to a wide `and` must
//! leave that narrow form where the target's casts are free. Both ran in turn
//! forever: `bench/histo` -O1 never finished (2026-11-13).

mod common;

use std::sync::mpsc;
use std::time::Duration;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const MASKED_WORD: &str = "define i8 @f(i16 %x, i16 %y) {
  %a = and i16 %x, 15
  %b = add i16 %a, %y
  %t = trunc i16 %b to i8
  ret i8 %t
}";

/// The same from a long: the byte `and` of the truncated long is widened to a
/// word, narrowed again by `ShrinkDemandedOp`, and so on (`signbit_idioms` -O1,
/// `lowbit_i64`).
const LOW_BIT_OF_LONG: &str = "define i16 @low(i64 %x, i16 %y) {
  %a = and i64 %x, 1
  %c = icmp eq i64 %a, 0
  %z = zext i1 %c to i16
  %r = add i16 %z, %y
  ret i16 %r
}";

fn compiles(body: &str) {
    let ir = format!("target datalayout = \"{}\"\n{body}", datalayout());
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(assembly(&ir, &Request::new(Level::O2)).is_ok());
    });
    assert_eq!(finished.recv_timeout(Duration::from_secs(30)), Ok(true), "the combiner does not terminate");
}

#[test]
fn a_word_and_read_as_a_byte_compiles() {
    compiles(MASKED_WORD);
}

#[test]
fn the_low_bit_of_a_long_compiles() {
    compiles(LOW_BIT_OF_LONG);
}
