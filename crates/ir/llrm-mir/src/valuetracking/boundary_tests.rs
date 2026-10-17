//! The constructs of ValueTracking.cpp that MIR cannot say, each as the fact it rests on: a test fails the
//! day MIR gains the construct, so its translation is owed then and is not forgotten.

use crate::parse;

fn rejects(text: &str) -> bool {
    parse::module(text).is_err()
}

#[test]
fn mir_has_no_vectors() {
    assert!(rejects("define <4 x i32> @f(<4 x i32> %a) {\n  ret <4 x i32> %a\n}\n"));
    assert!(rejects(
        "define i32 @f(<4 x i32> %a) {\n  %x = extractelement <4 x i32> %a, i32 0\n  ret i32 %x\n}\n"
    ));
}

#[test]
fn mir_has_no_scalable_vectors_and_no_vscale_range() {
    assert!(rejects("define i32 @f(<vscale x 4 x i32> %a) {\n  ret i32 0\n}\n"));
    assert!(rejects("define i32 @f() vscale_range(1,16) {\n  ret i32 0\n}\n"));
}

#[test]
fn mir_has_no_undef() {
    assert!(rejects("define i32 @f() {\n  ret i32 undef\n}\n"));
}

#[test]
fn mir_has_no_operand_bundles() {
    assert!(rejects(
        "declare void @g()\ndefine void @f() {\n  call void @g() [ \"deopt\"(i32 1) ]\n  ret void\n}\n"
    ));
}

#[test]
fn mir_has_no_ptrtoaddr() {
    assert!(rejects("define i64 @f(ptr %p) {\n  %x = ptrtoaddr ptr %p to i64\n  ret i64 %x\n}\n"));
}

#[test]
fn mir_has_no_target_triple() {
    assert!(rejects("target triple = \"riscv64\"\ndefine void @f() {\n  ret void\n}\n"));
}

#[test]
fn mir_has_no_half() {
    assert!(rejects("define half @f(half %a) {\n  ret half %a\n}\n"));
}

/// `DataLayout` has no preferred alignment (`getPrefTypeAlign`), which `getPointerAlignment` uses for a global.
#[test]
fn the_data_layout_has_no_preferred_alignment() {
    let source = include_str!("../datalayout.rs");
    assert!(!source.contains("fn preferred_align"), "translate Value::getPointerAlignment's preferred alignment");
}
