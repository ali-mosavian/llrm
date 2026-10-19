//! Each body MIR text run by llrm-mir's interpreter before and after.

use llrm_analysis::testing::DOS;

use super::DeadStoreElimination;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn eliminated(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, DeadStoreElimination);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

/// A store overwritten before any read goes.
#[test]
fn an_overwritten_store_goes() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  store i16 %x, ptr @g\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("store i16 1"), "{after}");
}

/// A load between the stores reads the first.
#[test]
fn a_store_that_is_read_stays() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  %a = load i16, ptr @g\n  store i16 %x, ptr @g\n  %b = load i16, ptr @g\n  %r = add i16 %a, %b\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(after.contains("store i16 1"), "{after}");
}

/// A word store over half of an earlier dword store leaves it.
#[test]
fn a_partial_overwrite_stays() {
    let after = eliminated(
        "@g = global i32 0\n\ndefine i32 @f(i16 %x) {\nb0:\n  store i32 1, ptr @g\n  store i16 %x, ptr @g\n  %r = load i32, ptr @g\n  ret i32 %r\n}\n",
        &[&[7]],
    );
    assert!(after.contains("store i32 1"), "{after}");
}

/// A store to a local nothing reads goes (the end of the function).
#[test]
fn a_store_to_an_unread_local_goes() {
    let after = eliminated(
        "define i16 @f(i16 %x) {\nb0:\n  %a = alloca i16\n  store i16 %x, ptr %a\n  ret i16 %x\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("store i16"), "{after}");
}

/// A store to a global that is overwritten on only one arm stays.
#[test]
fn a_store_overwritten_on_one_path_stays() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  %c = icmp eq i16 %x, 0\n  br i1 %c, label %b1, label %b2\n\nb1:\n  store i16 2, ptr @g\n  br label %b3\n\nb2:\n  br label %b3\n\nb3:\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[0], &[7]],
    );
    assert!(after.contains("store i16 1"), "{after}");
}

/// A store to a global overwritten on both arms goes.
#[test]
fn a_store_overwritten_on_every_path_goes() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  %c = icmp eq i16 %x, 0\n  br i1 %c, label %b1, label %b2\n\nb1:\n  store i16 2, ptr @g\n  br label %b3\n\nb2:\n  store i16 3, ptr @g\n  br label %b3\n\nb3:\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[0], &[7]],
    );
    assert!(!after.contains("store i16 1"), "{after}");
}

/// A store of the value just loaded from the same cell is a no-op.
#[test]
fn a_store_of_the_loaded_value_goes() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  %v = load i16, ptr @g\n  store i16 %v, ptr @g\n  ret i16 %v\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("store i16"), "{after}");
}

/// A memset covered by the stores after it goes with its bytes.
#[test]
fn a_memset_overwritten_by_a_store_goes() {
    let after = eliminated(
        "@g = global [4 x i8] zeroinitializer\ndeclare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine i16 @f(i16 %x) {\nb0:\n  call void @llvm.memset.p0.i16(ptr @g, i8 0, i16 2, i1 false)\n  store i16 %x, ptr @g\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("llvm.memset.p0.i16(ptr @g"), "{after}");
}

/// A chain of overwrites costs in proportion to its length.
#[test]
fn a_long_chain_of_overwrites_is_linear() {
    let chain = |n: usize| {
        let stores: String = (0..n).map(|k| format!("  store i16 {k}, ptr @g\n")).collect();
        format!(
            "@g = global i16 0\n\ndefine i16 @f(i16 %x) {{\nb0:\n{stores}  %r = load i16, ptr @g\n  ret i16 %r\n}}\n"
        )
    };
    let work = |n: usize| {
        let before = super::steps();
        let after = eliminated(&chain(n), &[&[1]]);
        assert_eq!(after.matches("store i16").count(), 1, "{after}");
        super::steps() - before
    };
    let (small, large) = (work(100), work(200));
    assert!(large as f64 <= 2.5 * small as f64, "{small} steps for 100 stores, {large} for 200");
}

/// Blocks are walked in LLVM's post-order, a branch's first target first.
/// Walked the other way, the memset arm killed the word store (through the
/// bytes the final store and the memset cover) before the first arm's byte
/// store had killed the dword store, which the read in the join then needs;
/// `opt` keeps it.
#[test]
fn the_first_target_of_a_branch_is_walked_first() {
    let after = eliminated(
        "@g = global [64 x i8] zeroinitializer\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  %c = icmp eq i16 %x, 0\n  br i1 %c, label %b1, label %b2\nb1:\n  store i32 3, ptr @g\n  %p1 = getelementptr i8, ptr @g, i16 1\n  store i8 1, ptr %p1\n  br label %b3\nb2:\n  %p2 = getelementptr i8, ptr @g, i16 1\n  call void @llvm.memset.p0.i16(ptr %p2, i8 1, i16 2, i1 false)\n  br label %b3\nb3:\n  store i8 0, ptr @g\n  %p3 = getelementptr i8, ptr @g, i16 2\n  store i16 1, ptr %p3\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n\ndeclare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n",
        &[&[0], &[1]],
    );
    assert!(after.contains("store i16 1, ptr @g"), "{after}");
}

/// A local whose address is returned, or stored into another returned local,
/// is read by the caller: its stores stay. All of `%view`'s and `%slot`'s went
/// (QB view descriptors), six BASIC programs read zeros.
#[test]
fn a_returned_local_keeps_its_stores() {
    let mut module = parsed(&format!(
        "{DOS}define ptr @f(i8 %x) addrspace(1) {{\nb0:\n  %slot = alloca i8\n  %view = alloca [4 x i8]\n  store i8 %x, ptr %slot\n  store i16 1, ptr %view\n  %p = getelementptr i8, ptr %view, i16 2\n  store ptr %slot, ptr %p\n  ret ptr %view\n}}\n"
    ));
    let after = managed(&mut module, DeadStoreElimination);
    assert!(
        after.contains("store i8 %x") && after.contains("store i16 1") && after.contains("store ptr %slot"),
        "{after}"
    );
}

/// A store to a local whose address escaped, and then `lifetime.end`: nothing
/// reads it again and the end sweep spares it (it is visible to the callee's
/// holder), but the terminator kills it (`isMemTerminator`).
#[test]
fn a_store_before_lifetime_end_goes() {
    let mut module = parsed(&format!(
        "{DOS}declare void @keep(ptr)\ndeclare void @llvm.lifetime.end.p0(i64, ptr)\n\ndefine void @f(i32 %x) addrspace(1) {{\nb0:\n  %a = alloca i32\n  call void @keep(ptr %a)\n  store i32 %x, ptr %a\n  call void @llvm.lifetime.end.p0(i64 4, ptr %a)\n  ret void\n}}\n"
    ));
    let after = managed(&mut module, DeadStoreElimination);
    assert!(!after.contains("store i32 %x"), "{after}");
}

/// `lifetime.end` on an alloca the function returns (QB's B$FMID shape) ends
/// nothing: the caller reads what was stored. The marker was wrong, and a DSE
/// that trusted it removed the stores (#2625).
#[test]
fn a_lifetime_end_on_a_returned_alloca_kills_nothing() {
    let mut module = parsed(&format!(
        "{DOS}declare void @llvm.lifetime.end.p0(i64, ptr)\n\ndefine ptr @f(i32 %x) addrspace(1) {{\nb0:\n  %a = alloca i32\n  store i32 %x, ptr %a\n  call void @llvm.lifetime.end.p0(i64 4, ptr %a)\n  ret ptr %a\n}}\n"
    ));
    let after = managed(&mut module, DeadStoreElimination);
    assert!(after.contains("store i32 %x"), "{after}");
}

/// A store the next call overwrites before reading, by the `initializes` of
/// the callee's parameter, is dead (`getInitializesArgMemLoc`).
#[test]
fn a_store_before_a_call_that_initializes_it_goes() {
    let mut module = parsed(&format!(
        "{DOS}@g = global i32 0\n\ndeclare void @init(ptr initializes((0, 4)) %p) memory(argmem: write) nounwind\n\ndefine i32 @f(i32 %x) addrspace(1) {{\nb0:\n  store i32 %x, ptr @g\n  call void @init(ptr @g)\n  %r = load i32, ptr @g\n  ret i32 %r\n}}\n"
    ));
    let after = managed(&mut module, DeadStoreElimination);
    assert!(!after.contains("store i32 %x"), "{after}");
}

/// A store over the head of a memset leaves its tail, which a load reads: the
/// memset went, and the load at byte 6 read an undefined byte.
#[test]
fn a_memset_whose_tail_is_read_stays() {
    let after = eliminated(
        "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine i16 @f(i16 %x) {\nb0:\n  %s = alloca [8 x i8]\n  call void @llvm.memset.p0.i16(ptr %s, i8 0, i16 8, i1 false)\n  store i16 %x, ptr %s\n  %s6 = getelementptr inbounds i8, ptr %s, i16 6\n  %v = load i16, ptr %s6\n  %w = load i16, ptr %s\n  %r = add i16 %v, %w\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(after.contains("i8 0, i16 6"), "{after}");
}

/// A call that fills a buffer reads it first through another argument: the
/// memset before it stays. It went, and the callee read poison.
#[test]
fn a_call_reading_the_buffer_it_fills_keeps_the_earlier_fill() {
    let after = eliminated(
        "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine void @fill(ptr %d, ptr %s) {\nb:\n  %v = load i16, ptr %s\n  store i16 %v, ptr %d\n  %e = getelementptr i8, ptr %d, i16 2\n  store i16 %v, ptr %e\n  ret void\n}\n\ndefine i16 @f(i16 %x) {\nb0:\n  %a = alloca [4 x i8]\n  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 4, i1 false)\n  call void @fill(ptr nocapture writeonly initializes((0, 4)) %a, ptr %a)\n  %v = load i16, ptr %a\n  ret i16 %v\n}\n",
        &[&[7]],
    );
    assert!(after.contains("llvm.memset.p0.i16(ptr %a"), "{after}");
}
