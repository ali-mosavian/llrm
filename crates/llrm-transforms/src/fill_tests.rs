//! `fill` had no tests of its own in llrm-core; these are behaviour tests,
//! each body MIR text run by llrm-mir's interpreter before and after.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use super::filled;
use crate::testing::{parsed, printed, results};

const MEMSET: &str = "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\n";

/// `text`'s @f filled: its printed form, and whether it changed. @f
/// computes what it did on `inputs`.
fn fill(text: &str, inputs: &[&[i128]]) -> (String, bool) {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = filled(context, &layout, &callees, function, &outer);
    let after = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (after, changed)
}

fn kept(text: &str, inputs: &[&[i128]]) {
    let (after, changed) = fill(text, inputs);
    assert!(!changed, "{after}");
    assert_eq!(after, printed(&parsed(&format!("{DOS}{text}"))));
}

/// `i` from 0 while `i < bound`, the latch running `body` with `%p`
/// addressing `@buf`'s cells of `cell`; @f returns cell `%q` and `result`.
fn looped(cell: &str, bound: &str, body: &str, result: &str) -> String {
    let widened = if cell == "i16" { "or i16 %v, 0".to_owned() } else { format!("zext {cell} %v to i16") };
    format!(
        "@buf = global [64 x {cell}] zeroinitializer

{MEMSET}define i16 @f(i16 %n, i16 %q) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, {bound}
  br i1 %c, label %b2, label %b3

b2:
{body}  %next = add i16 %i, 1
  br label %b1

b3:
  %e = phi i16 [ %i, %b1 ]
  %r = getelementptr [64 x {cell}], ptr @buf, i16 0, i16 %q
  %v = load {cell}, ptr %r
  %w = {widened}
  %x = add i16 %w, {result}
  ret i16 %x
}}
"
    )
}

const TRIPS: &[&[i128]] = &[&[0, 0], &[1, 0], &[1, 1], &[40, 39], &[40, 40], &[-5, 0]];

/// Bytes a symbolic count of trips stores are one memset, guarded by the
/// header's test; the counter leaves with its exit value.
#[test]
fn a_byte_loop_is_one_memset() {
    let body = "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  store i8 65, ptr %p\n";
    let (text, changed) = fill(&looped("i8", "%n", body, "%e"), TRIPS);
    assert!(changed);
    assert!(text.contains("b1:\n  %c = icmp slt i16 0, %n\n  br i1 %c, label %b2, label %b3\n"), "{text}");
    assert!(text.contains("  call void @llvm.memset.p0.i16(ptr %p, i8 65, i16 %1, i1 false)\n") && text.contains("  br label %b3\n\nb3:\n  %e = phi i16 [ 0, %b1 ], [ %2, %b2 ]\n"), "{text}");
}

/// A known positive count enters without a test; a word of one repeated
/// byte fills as bytes, twice the trips.
#[test]
fn a_counted_word_loop_is_one_memset_of_its_bytes() {
    for (value, byte) in [(0, 0), (0x4141, 65)] {
        let body = format!("  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 {value}, ptr %p\n");
        let (text, changed) = fill(&looped("i16", "10", &body, "0"), &[&[0, 0], &[0, 9], &[0, 10]]);
        assert!(changed);
        assert!(text.contains(&format!("b1:\n  %c = icmp slt i16 0, 10\n  br label %b2\n")), "{text}");
        assert!(text.contains(&format!("call void @llvm.memset.p0.i16(ptr %p, i8 {byte}, i16 20, i1 false)")), "{text}");
    }
}

/// A trip that fills four bytes, stepping four: one memset of them all.
#[test]
fn a_loop_of_memsets_is_one_memset() {
    let body = "  %j = mul i16 %i, 4\n  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %j\n  call void @llvm.memset.p0.i16(ptr %p, i8 7, i16 4, i1 false)\n";
    let (text, changed) = fill(&looped("i8", "10", body, "%e"), &[&[0, 0], &[0, 39], &[0, 40]]);
    assert!(changed && text.contains("call void @llvm.memset.p0.i16(ptr %p, i8 7, i16 40, i1 false)"), "{text}");
}

#[test]
fn a_fill_whose_stride_is_not_the_element_size_is_kept() {
    let body = "  %j = mul i16 %i, 2\n  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %j\n  store i8 65, ptr %p\n";
    kept(&looped("i8", "20", body, "%e"), &[&[0, 0], &[0, 1], &[0, 2]]);
}

#[test]
fn a_loop_with_a_volatile_store_is_kept() {
    let body = "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  store volatile i8 65, ptr %p\n";
    kept(&looped("i8", "%n", body, "%e"), TRIPS);
}

/// `rep stosw` of a word whose bytes differ is isel's, no memset.
#[test]
fn a_word_of_two_bytes_is_kept() {
    let body = "  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 4660, ptr %p\n";
    kept(&looped("i16", "10", body, "0"), &[&[0, 0], &[0, 9]]);
}

#[test]
fn a_value_the_loop_changes_is_kept() {
    let body = "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  %b = trunc i16 %i to i8\n  store i8 %b, ptr %p\n";
    kept(&looped("i8", "%n", body, "%e"), TRIPS);
}

/// A function pass declares nothing: with no memset declared, no fill.
#[test]
fn an_undeclared_memset_is_no_fill() {
    let body = "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  store i8 65, ptr %p\n";
    kept(&looped("i8", "%n", body, "%e").replace(MEMSET, ""), TRIPS);
}

/// Trips times two bytes may wrap the index where no GEP is `inbounds`
/// and nothing bounds the trips.
#[test]
fn a_word_count_that_may_wrap_is_kept() {
    let body = |inbounds: &str| format!("  %p = getelementptr {inbounds}[64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 0, ptr %p\n");
    kept(&looped("i16", "%n", &body(""), "%e"), TRIPS);
    let (text, changed) = fill(&looped("i16", "%n", &body("inbounds "), "%e"), TRIPS);
    assert!(changed && text.contains("call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 %2, i1 false)"), "{text}");
}
