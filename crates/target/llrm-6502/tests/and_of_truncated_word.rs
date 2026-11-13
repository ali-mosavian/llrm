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

#[test]
fn a_word_and_read_as_a_byte_compiles() {
    let ir = format!("target datalayout = \"{}\"\n{MASKED_WORD}", datalayout());
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(assembly(&ir, &Request::new(Level::O2)).is_ok());
    });
    assert_eq!(finished.recv_timeout(Duration::from_secs(30)), Ok(true), "the combiner does not terminate");
}
