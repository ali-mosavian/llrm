//! Each body MIR text run by llrm-mir's interpreter before and after.

use llrm_analysis::testing::DOS;

use super::MemCpyOptimizer;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn merged(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, MemCpyOptimizer);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

/// `count` stores of `bits`-bit `value` to consecutive cells of @a, then the
/// word at its start read back.
fn stores(
    count: usize,
    bits: u32,
    value: &str,
) -> String {
    let step = bits / 8;
    let mut body = String::new();
    for at in 0..count {
        body += &format!(
            "  %p{at} = getelementptr i8, ptr @a, i16 {off}\n  store i{bits} {value}, ptr %p{at}\n",
            off = at as u32 * step
        );
    }
    format!(
        "@a = global [64 x i8] zeroinitializer\n\ndefine i16 @f(i16 %x) {{\nb0:\n  store i16 %x, ptr @a\n{body}  %r = load i16, ptr @a\n  ret i16 %r\n}}\n"
    )
}

/// Four byte stores of zero are one memset.
#[test]
fn four_byte_stores_are_one_memset() {
    let after = merged(&stores(4, 8, "0"), &[&[7]]);
    assert!(after.contains("@llvm.memset.p0.i16(ptr align 1 %p0, i8 0, i16 4, i1 false)"), "{after}");
    assert!(!after.contains("store i8 0, ptr %p1"), "{after}");
}

/// Two `double` zeros are one 16-byte memset (`isBytewiseValue` reads float
/// constants). Left as stores they were 6 `fldz; fst` where the pin's `movl $0`s
/// are free: nbody's x87 nest stored 18 cells against the pin's 10.
#[test]
fn two_double_zeros_are_one_memset() {
    let text = "@a = global [64 x i8] zeroinitializer\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 %x, ptr @a\n  %p1 = getelementptr i8, ptr @a, i16 8\n  store double 0.0, ptr %p1\n  %p2 = getelementptr i8, ptr @a, i16 16\n  store double 0.0, ptr %p2\n  %r = load i16, ptr @a\n  ret i16 %r\n}\n";
    let after = merged(text, &[&[7]]);
    assert!(after.contains("i8 0, i16 16, i1 false"), "{after}");
    assert!(!after.contains("store double"), "{after}");
}

/// Two stores are what the code generator pairs itself.
#[test]
fn two_word_stores_stay() {
    let after = merged(&stores(2, 16, "0"), &[&[7]]);
    assert!(!after.contains("llvm.memset"), "{after}");
}

/// A store of one byte repeated through a wider integer is the same byte.
#[test]
fn a_repeated_byte_in_words_is_one_memset() {
    let after = merged(&stores(8, 16, "257"), &[&[7]]);
    assert!(after.contains("i8 1, i16 16, i1 false"), "{after}");
}

/// Another byte ends the run: the first four are a memset, the rest stay.
#[test]
fn another_byte_ends_the_scan() {
    let mut text = stores(4, 8, "0");
    text = text.replace(
        "  %r = load i16, ptr @a",
        "  %q = getelementptr i8, ptr @a, i16 4\n  store i8 1, ptr %q\n  %r = load i16, ptr @a",
    );
    let after = merged(&text, &[&[7]]);
    assert!(after.contains("i16 4, i1 false"), "{after}");
    assert!(after.contains("store i8 1, ptr %q"), "{after}");
}

/// A load between the stores may read them: the scan stops, and 3 stores
/// stay below the profit bound.
#[test]
fn a_load_ends_the_scan() {
    let mut text = stores(6, 8, "0");
    text = text
        .replace("  %p3 = getelementptr", "  %m = load i8, ptr @a\n  %z = zext i8 %m to i16\n  %p3 = getelementptr");
    let after = merged(&text, &[&[7]]);
    assert!(!after.contains("llvm.memset.p0.i16(ptr %p0, i8 0, i16 6"), "{after}");
}

/// One value of an `i8` stored four times is one memset of that value.
#[test]
fn an_i8_value_is_the_byte() {
    let text = stores(4, 8, "%b").replace("(i16 %x)", "(i16 %x)").replace("b0:\n", "b0:\n  %b = trunc i16 %x to i8\n");
    let after = merged(&text, &[&[7]]);
    assert!(after.contains("llvm.memset.p0.i16(ptr align 1 %p0, i8 %b, i16 4, i1 false)"), "{after}");
}

/// A memset beside two stores of its byte is one memset (always worth it).
#[test]
fn a_memset_is_extended_by_a_store() {
    let text = "@a = global [64 x i8] zeroinitializer\ndeclare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine i16 @f(i16 %x) {\nb0:\n  call void @llvm.memset.p0.i16(ptr @a, i8 0, i16 8, i1 false)\n  %p = getelementptr i8, ptr @a, i16 8\n  store i8 0, ptr %p\n  %r = load i16, ptr @a\n  ret i16 %r\n}\n";
    let after = merged(text, &[&[7]]);
    assert!(after.contains("i8 0, i16 9, i1 false"), "{after}");
}

/// The scan costs what the block holds: a block of N separate runs is not N
/// scans of the whole block.
#[test]
fn separate_runs_cost_in_proportion() {
    let runs = |n: usize| {
        let mut body = String::new();
        for run in 0..n {
            for at in 0..4 {
                body += &format!(
                    "  %p{run}_{at} = getelementptr i8, ptr @a, i16 {off}\n  store i8 {byte}, ptr %p{run}_{at}\n",
                    off = run * 4 + at,
                    byte = run % 2
                );
            }
        }
        format!(
            "@a = global [4096 x i8] zeroinitializer\n\ndefine i16 @f(i16 %x) {{\nb0:\n{body}  %r = load i16, ptr @a\n  ret i16 %r\n}}\n"
        )
    };
    let before = super::steps();
    merged(&runs(100), &[&[7]]);
    let small = super::steps() - before;
    let before = super::steps();
    merged(&runs(200), &[&[7]]);
    let large = super::steps() - before;
    assert!(large as f64 <= 2.5 * small as f64, "{small} steps for 100 runs, {large} for 200");
}

/// The memset made keeps the alignment of the lowest access it replaces
/// (`MemsetRange::Alignment`): lru's two `align 2` fills became one memset with
/// no alignment, which the backend wrote as `rep stosb` of 28 bytes (+20%
/// clocks at -Os).
#[test]
fn a_merged_memset_keeps_the_alignment_of_its_first_access() {
    let text = "@a = global [64 x i8] zeroinitializer\ndeclare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine i16 @f(i16 %x) {\nb0:\n  call void @llvm.memset.p0.i16(ptr align 2 @a, i8 -1, i16 14, i1 false)\n  %p = getelementptr i8, ptr @a, i16 14\n  call void @llvm.memset.p0.i16(ptr align 2 %p, i8 -1, i16 14, i1 false)\n  %r = load i16, ptr @a\n  ret i16 %r\n}\n";
    let after = merged(text, &[&[7]]);
    assert!(after.contains("(ptr align 2 @a, i8 -1, i16 28, i1 false)"), "{after}");
    let stores = "@a = global [64 x i8] zeroinitializer\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 -1, ptr @a, align 2\n  %p = getelementptr i8, ptr @a, i16 2\n  store i16 -1, ptr %p, align 2\n  %q = getelementptr i8, ptr @a, i16 4\n  store i16 -1, ptr %q, align 2\n  %s = getelementptr i8, ptr @a, i16 6\n  store i16 -1, ptr %s, align 2\n  %r = load i16, ptr @a\n  ret i16 %r\n}\n";
    let after = merged(stores, &[&[7]]);
    assert!(after.contains("align 2 @a, i8 -1, i16 8, i1 false"), "{after}");
}
