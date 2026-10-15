//! Each body MIR text run by llrm-mir's interpreter before and after.

use llrm_analysis::testing::DOS;

use super::MemCpyOpt;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn merged(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, MemCpyOpt { size: false });
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

/// `count` pairs of `bits`-bit cells from @s to `to` (`delta` bytes on), each
/// load before its store.
fn pairs_to(
    count: usize,
    bits: u32,
    to: &str,
    delta: u32,
) -> String {
    let step = bits / 8;
    let mut body = String::new();
    for at in 0..count {
        body += &format!(
            "  %ps{at} = getelementptr i8, ptr @s, i16 {off}\n  %v{at} = load i{bits}, ptr %ps{at}\n  %pd{at} = getelementptr i8, ptr {to}, i16 {on}\n  store i{bits} %v{at}, ptr %pd{at}\n",
            off = at as u32 * step,
            on = at as u32 * step + delta
        );
    }
    format!(
        "@s = global [64 x i8] zeroinitializer\n@d = global [64 x i8] zeroinitializer\n\ndefine i16 @f(i16 %x) {{\nb0:\n  store i16 %x, ptr @s\n{body}  %r = load i16, ptr @d\n  ret i16 %r\n}}\n"
    )
}

fn pairs(
    count: usize,
    bits: u32,
) -> String {
    pairs_to(count, bits, "@d", 0)
}

/// Twenty word pairs were forty moves; past what isel expands, one `rep movsw`.
#[test]
fn ten_dword_pairs_are_one_memcpy() {
    let after = merged(&pairs(20, 16), &[&[7]]);
    assert!(after.contains("@llvm.memcpy.p0.p0.i16(ptr %pd0, ptr %ps0, i16 40, i1 false)"), "{after}");
    assert!(!after.contains("load i16, ptr %ps1"), "{after}");
}

/// Two dword pairs are what isel makes of a memcpy: nothing is gained, and a
/// call in their place would be no better.
#[test]
fn two_pairs_stay_as_they_are() {
    let after = merged(&pairs(2, 32), &[&[7]]);
    assert!(!after.contains("llvm.memcpy"), "{after}");
}

/// Eight byte pairs are sixteen moves, and two dword pairs once copied.
#[test]
fn eight_byte_pairs_are_one_copy_of_two_dwords() {
    let after = merged(&pairs(8, 8), &[&[7]]);
    assert!(after.contains("llvm.memcpy") && after.contains("i16 8, i1 false"), "{after}");
}

/// A store to the bytes a later pair loads, between them, makes the order of
/// the loads and stores the result: nothing is merged across it.
#[test]
fn a_store_between_pairs_ends_the_run() {
    let text = pairs(20, 16).replace("  %pd10 =", "  store i16 9, ptr @s\n  %pd10 =");
    let after = merged(&text, &[&[7]]);
    assert!(!after.contains("i16 40, i1 false"), "{after}");
}

/// Source and destination that are one object, the destination a word on, make
/// each pair read what the one before wrote: not a memcpy.
#[test]
fn pairs_that_overlap_are_not_a_memcpy() {
    let text = pairs_to(20, 16, "@s", 2);
    let after = merged(&text, &[&[7]]);
    assert!(!after.contains("llvm.memcpy"), "{after}");
}
