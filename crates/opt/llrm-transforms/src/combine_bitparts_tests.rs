//! `combine_bitparts`: a mirror of the bits of one value is `bswap` or
//! `bitreverse`.

use llrm_mir::datalayout::DataLayout;

use crate::combine::combine_with_declared;
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(
        context,
        &globals,
        &metadata,
        &DataLayout::default(),
        function,
        crate::combine::visit,
        &mut declared,
    );
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// x_popcount's `rev32`: five stages of swaps, each pair `(x >> k) & m | (x &
/// m) << k`.
const REV32: &str = "define i32 @f(i32 %x) {
b0:
  %a0 = lshr i32 %x, 1
  %a1 = and i32 %a0, 1431655765
  %a2 = and i32 %x, 1431655765
  %a3 = shl i32 %a2, 1
  %a = or i32 %a1, %a3
  %b0 = lshr i32 %a, 2
  %b1 = and i32 %b0, 858993459
  %b2 = and i32 %a, 858993459
  %b3 = shl i32 %b2, 2
  %b = or i32 %b1, %b3
  %c0 = lshr i32 %b, 4
  %c1 = and i32 %c0, 252645135
  %c2 = and i32 %b, 252645135
  %c3 = shl i32 %c2, 4
  %c = or i32 %c1, %c3
  %d0 = lshr i32 %c, 8
  %d1 = and i32 %d0, 16711935
  %d2 = and i32 %c, 16711935
  %d3 = shl i32 %d2, 8
  %d = or i32 %d1, %d3
  %e0 = lshr i32 %d, 16
  %e1 = shl i32 %d, 16
  %e = or i32 %e0, %e1
  ret i32 %e
}
";

/// rev32(s) & 255 kept all five stages: 31 instructions where the pin's
/// `bitreverse` of the top byte is 17.
#[test]
fn five_stages_of_bit_swaps_are_a_bitreverse() {
    let after = combined(REV32);
    assert!(after.contains("@llvm.bitreverse.i32(i32 %x)"), "{after}");
    assert!(!after.contains("shl") && !after.contains("lshr"), "{after}");
}

/// `(x >> 24) | ((x >> 8) & 0xff00) | ((x << 8) & 0xff0000) | (x << 24)`.
#[test]
fn four_shifted_bytes_are_a_bswap() {
    let after = combined(
        "define i32 @f(i32 %x) {
b0:
  %a = lshr i32 %x, 24
  %b0 = lshr i32 %x, 8
  %b = and i32 %b0, 65280
  %c0 = shl i32 %x, 8
  %c = and i32 %c0, 16711680
  %d = shl i32 %x, 24
  %ab = or i32 %a, %b
  %cd = or i32 %c, %d
  %r = or i32 %ab, %cd
  ret i32 %r
}
",
    );
    assert!(after.contains("@llvm.bswap.i32(i32 %x)"), "{after}");
}

/// Two bytes in one order and two in another are no permutation of a byte swap.
#[test]
fn bytes_not_mirrored_are_left_alone() {
    let after = combined(
        "define i32 @f(i32 %x) {
b0:
  %a = lshr i32 %x, 8
  %b = shl i32 %x, 8
  %r = or i32 %a, %b
  ret i32 %r
}
",
    );
    assert!(!after.contains("bswap") && !after.contains("bitreverse"), "{after}");
}
