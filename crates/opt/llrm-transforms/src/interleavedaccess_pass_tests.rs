use llrm_mir::context::Context;
use llrm_mir::passes::Declared;

use super::lower_interleaved_stores;
use crate::testing::{parsed, printed};

fn arm64() -> llrm_arm64::tti::AArch64 {
    llrm_arm64::tti::AArch64::of_cpu("generic").expect("generic")
}

/// @f of `text` after the pass; whether anything changed.
fn lowered(text: &str) -> (bool, String) {
    let mut module = parsed(text);
    let mut declared = Declared::of(&module);
    let tti = arm64();
    let changed = {
        let (context, function): (&mut Context, _) = module.function_mut("f").expect("@f");
        lower_interleaved_stores(context, function, &mut declared, &tti)
    };
    let added = declared.since(module.globals.len(), &module.context);
    module.globals.extend(added);
    (changed, printed(&module))
}

fn store_of(
    vector: &str,
    lanes: usize,
    mask: &str,
    tail: &str,
) -> String {
    format!(
        "define void @f(<{lanes} x i8> %a, <{lanes} x i8> %b, ptr %p) {{\n\
         %s = shufflevector <{lanes} x i8> %a, <{lanes} x i8> %b, <{vector} x i32> <{mask}>\n\
         store <{vector} x i8> %s, ptr %p{tail}\n  ret void\n}}\n"
    )
}

fn zip(lanes: usize) -> String {
    (0..lanes).flat_map(|at| [at, at + lanes]).map(|lane| format!("i32 {lane}")).collect::<Vec<_>>().join(", ")
}

/// Textfill O2/O3 kept `zip1`/`zip2` plus two stores of a `<32 x i8>` zip
/// (32009 instructions to the pin's 4120); the pin stores it with one `st2`.
#[test]
fn test_a_stored_zip_of_two_byte_vectors_is_one_st2() {
    let (changed, text) = lowered(&store_of("32", 16, &zip(16), ""));
    assert!(changed, "{text}");
    assert!(text.contains("call void @llvm.aarch64.neon.st2.v16i8.p0("), "{text}");
    assert!(!text.contains("store "), "{text}");
}

#[test]
fn test_a_stored_three_way_interleave_is_one_st3() {
    // Members start at lanes 0, 8 and 16 of two `<16 x i8>` vectors.
    let mask = (0..8).flat_map(|at| [at, at + 8, at + 16]).map(|lane| format!("i32 {lane}")).collect::<Vec<_>>();
    let text = format!(
        "define void @f(<16 x i8> %a, <16 x i8> %b, ptr %p) {{\n\
         %s = shufflevector <16 x i8> %a, <16 x i8> %b, <24 x i32> <{}>\n\
         store <24 x i8> %s, ptr %p\n  ret void\n}}\n",
        mask.join(", ")
    );
    // Eight bytes per member: a 64-bit sub-vector, which `st3` takes.
    let (changed, text) = lowered(&text);
    assert!(changed, "{text}");
    assert!(text.contains("@llvm.aarch64.neon.st3.v8i8.p0("), "{text}");
}

/// The shuffled value has another use: the pin leaves it to `zip1`/`zip2`.
#[test]
fn test_a_zip_with_another_user_stays_a_store() {
    let text = format!(
        "define <32 x i8> @f(<16 x i8> %a, <16 x i8> %b, ptr %p) {{\n\
         %s = shufflevector <16 x i8> %a, <16 x i8> %b, <32 x i32> <{}>\n\
         store <32 x i8> %s, ptr %p\n  ret <32 x i8> %s\n}}\n",
        zip(16)
    );
    let (changed, text) = lowered(&text);
    assert!(!changed, "{text}");
}

#[test]
fn test_a_volatile_zip_store_stays_a_store() {
    let text = store_of("32", 16, &zip(16), "").replace("store <32", "store volatile <32");
    let (changed, text) = lowered(&text);
    assert!(!changed, "{text}");
}

#[test]
fn test_a_non_interleaving_shuffle_stays_a_store() {
    let mask = (0..32).map(|lane| format!("i32 {lane}")).collect::<Vec<_>>().join(", ");
    let (changed, text) = lowered(&store_of("32", 16, &mask, ""));
    assert!(!changed, "{text}");
}

fn load_text(
    users: &str,
    returns: &str,
) -> String {
    format!("define {returns} @f(ptr %p) {{\n  %w = load <32 x i8>, ptr %p\n{users}}}\n")
}

fn deinterleave(index: usize) -> String {
    (0..16).map(|at| format!("i32 {}", index + at * 2)).collect::<Vec<_>>().join(", ")
}

/// A deinterleaving load of bytes stayed `uzp1`/`uzp2` of two wide loads
/// (textfill O2/O3); the pin loads it with one `ld2`.
#[test]
fn test_two_deinterleaving_shuffles_of_a_load_are_one_ld2() {
    let users = format!(
        "  %e = shufflevector <32 x i8> %w, <32 x i8> poison, <16 x i32> <{}>\n\
         %o = shufflevector <32 x i8> %w, <32 x i8> poison, <16 x i32> <{}>\n\
         %r = add <16 x i8> %e, %o\n  ret <16 x i8> %r\n",
        deinterleave(0),
        deinterleave(1)
    );
    let (changed, text) = lowered(&load_text(&users, "<16 x i8>"));
    assert!(changed, "{text}");
    assert!(text.contains("call { <16 x i8>, <16 x i8> } @llvm.aarch64.neon.ld2.v16i8.p0(ptr %p)"), "{text}");
    assert!(text.contains("extractvalue { <16 x i8>, <16 x i8> }"), "{text}");
    assert!(!text.contains("load "), "{text}");
}

/// The load has a user that is no shuffle: the pin leaves it a load.
#[test]
fn test_a_load_with_a_plain_user_stays_a_load() {
    let users = format!(
        "  %e = shufflevector <32 x i8> %w, <32 x i8> poison, <16 x i32> <{}>\n\
         %x = extractelement <32 x i8> %w, i32 0\n  ret <16 x i8> %e\n",
        deinterleave(0)
    );
    let (changed, text) = lowered(&load_text(&users, "<16 x i8>"));
    assert!(!changed, "{text}");
}
