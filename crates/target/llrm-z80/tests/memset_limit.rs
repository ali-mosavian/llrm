//! A constant fill at a level that allows no trade is an `ldir` once it is more
//! than one store.

use llrm_z80::compile::lines;
use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::price_emitted;

fn fill(bytes: u32) -> String {
    format!(
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
         declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg)\n\
         define void @fill(ptr %p) {{\n  call void @llvm.memset.p0.i16(ptr align 2 %p, i8 0, i16 {bytes}, i1 false)\n  ret void\n}}\n"
    )
}

/// huffman_decode -O1 zeroed a 34 B array as nine 4-byte stores, 102 B through
/// IX, where -Os made an `ldir` fill of 13 B: the pin's x86 limit of 16 stores
/// (8 at size) counts stores that are two word stores here. The cell was
/// 796 B against SDCC's 719 B.
#[test]
fn a_34_byte_zero_fill_is_an_ldir_at_o1() {
    let ir = fill(34);
    let (bytes, _) =
        price_emitted(&lines(&ir, "fill", 0, &Request::new(Level::O1)).unwrap_or_else(|why| panic!("{why}")), true);
    assert!(bytes <= 20, "the fill is {bytes} B at -O1, nine 4-byte stores are over 40 B");
}
