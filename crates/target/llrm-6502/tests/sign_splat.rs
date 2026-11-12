//! `x >> 15` of a word recomputed the all-sign byte for each half: sext of a
//! char to long splatted its sign three times (rep_count -O1, 28 cells of the
//! table carried the extra `asl; lda #0; sbc #0; eor #$ff`, 2026-11-12).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn splats(body: &str) -> usize {
    let ir = format!("target datalayout = \"{}\"\n{body}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    text.split(".segment").next().unwrap().matches("eor #$ff").count()
}

#[test]
fn a_word_shifted_right_by_15_splats_its_sign_once() {
    let n = splats("define i16 @f(i16 %x) {\n  %s = ashr i16 %x, 15\n  ret i16 %s\n}\n");
    assert_eq!(n, 1, "{n} splats");
}
