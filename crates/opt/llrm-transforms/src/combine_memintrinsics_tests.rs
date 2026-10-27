//! `combine_memintrinsics`: a small transfer is a load and a store.

use llrm_mir::datalayout::DataLayout;

use crate::combine::combine_with;
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, |c, inst| {
        if matches!(
            c.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Call(_)
        ) {
            c.visit_call(inst)
        } else {
            crate::combine::Visit::None
        }
    });
    printed(&module)
}

fn transfer(
    callee: &str,
    length: u32,
    volatile: &str,
) -> String {
    format!(
        "declare void @llvm.{callee}.p0.p0.i16(ptr, ptr, i16, i1)

define void @f(ptr %d, ptr %s) {{
b0:
  call void @llvm.{callee}.p0.p0.i16(ptr %d, ptr %s, i16 {length}, i1 {volatile})
  ret void
}}
"
    )
}

/// The C front end's struct assignment (#3621) of a 4-byte struct stayed a
/// `memcpy`, which promotion cannot see through: the pin's InstCombine makes it
/// a load and a store.
#[test]
fn a_four_byte_memcpy_is_a_load_and_a_store() {
    let after = combined(&transfer("memcpy", 4, "false"));
    assert!(after.contains("= load i32, ptr %s") && after.contains("store i32 %"), "{after}");
    assert!(!after.contains("call void @llvm.memcpy"), "{after}");
}

/// Every size the pin takes: 1, 2, 4 and 8, memmove too.
#[test]
fn sizes_one_two_four_and_eight_and_a_memmove_are_taken() {
    for (length, ty) in [(1, "i8"), (2, "i16"), (4, "i32"), (8, "i64")] {
        for callee in ["memcpy", "memmove"] {
            let after = combined(&transfer(callee, length, "false"));
            assert!(after.contains(&format!("= load {ty}, ptr %s")), "{callee} {length}: {after}");
        }
    }
}

/// Three, six and sixteen bytes are not a single load.
#[test]
fn other_lengths_stay_a_transfer() {
    for length in [3, 6, 12, 16] {
        let after = combined(&transfer("memcpy", length, "false"));
        assert!(after.contains("call void @llvm.memcpy") && !after.contains("= load"), "{length}: {after}");
    }
}

/// A volatile transfer keeps its volatility on the load and the store.
#[test]
fn a_volatile_transfer_is_a_volatile_load_and_store() {
    let after = combined(&transfer("memcpy", 4, "true"));
    assert!(after.contains("load volatile i32") && after.contains("store volatile i32"), "{after}");
}

/// A copy of 100 bytes between two aligned globals kept `align 1` on both
/// pointers, so m32 called `memcpy` where the pin's InstCombine raised them and
/// the selector moved dwords.
#[test]
fn a_transfer_gets_the_alignment_known_of_its_pointers() {
    let after = combined(
        "@a = global [100 x i8] zeroinitializer, align 4
@b = global [100 x i8] zeroinitializer, align 8
declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)

define void @f() {
b0:
  call void @llvm.memcpy.p0.p0.i16(ptr @b, ptr @a, i16 100, i1 false)
  ret void
}
",
    );
    assert!(after.contains("ptr align 8 @b, ptr align 4 @a"), "{after}");
}

/// A `memset` raises its destination's alignment too.
#[test]
fn a_memset_gets_the_alignment_known_of_its_destination() {
    let after = combined(
        "@b = global [100 x i8] zeroinitializer, align 8
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)

define void @f() {
b0:
  call void @llvm.memset.p0.i16(ptr @b, i8 0, i16 100, i1 false)
  ret void
}
",
    );
    assert!(after.contains("ptr align 8 @b"), "{after}");
}
