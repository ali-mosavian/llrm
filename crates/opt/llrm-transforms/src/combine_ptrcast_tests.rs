//! `combine_ptrcast`: `ptrtoint` and `inttoptr` go through the pointer-sized
//! integer, and an add on a `ptrtoint` that stays an integer is a GEP.

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
        |c, inst| match c.function.instruction(inst).opcode {
            llrm_mir::opcode::Opcode::Cast(llrm_mir::opcode::CastOp::PtrToInt) => c.visit_ptr_to_int(inst),
            llrm_mir::opcode::Opcode::Cast(llrm_mir::opcode::CastOp::IntToPtr) => c.visit_int_to_ptr(inst),
            _ => crate::combine::Visit::None,
        },
        &mut declared,
    );
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// `ptrtoint ptr to i32` stayed a narrow cast: pinned instcombine does the
/// pointer-sized `ptrtoint` and a `trunc`, which later folds can see.
#[test]
fn a_ptrtoint_to_a_narrow_integer_goes_through_the_pointer_sized_one() {
    let after = combined(
        "define i32 @f(ptr %p) {
b0:
  %i = ptrtoint ptr %p to i32
  ret i32 %i
}
",
    );
    assert!(after.contains("ptrtoint ptr %p to i64"), "{after}");
    assert!(after.contains("trunc i64"), "{after}");
}

/// `inttoptr i32` stayed a narrow cast: pinned instcombine zero-extends to
/// the pointer width first.
#[test]
fn an_inttoptr_from_a_narrow_integer_goes_through_the_pointer_sized_one() {
    let after = combined(
        "define ptr @f(i32 %x) {
b0:
  %p = inttoptr i32 %x to ptr
  ret ptr %p
}
",
    );
    assert!(after.contains("zext i32 %x to i64"), "{after}");
    assert!(after.contains("inttoptr i64"), "{after}");
}

/// `inttoptr (ptrtoint p + n)` only compared stayed integer arithmetic:
/// pinned instcombine makes it a `getelementptr i8`.
#[test]
fn an_inttoptr_of_a_ptrtoint_sum_that_is_only_compared_is_a_gep() {
    let after = combined(
        "define i1 @f(ptr %p, ptr %o, i64 %n) {
b0:
  %i = ptrtoint ptr %p to i64
  %a = add i64 %i, %n
  %q = inttoptr i64 %a to ptr
  %c = icmp eq ptr %q, %o
  ret i1 %c
}
",
    );
    assert!(after.contains("getelementptr i8, ptr %p, i64 %n"), "{after}");
    assert!(!after.contains("inttoptr"), "{after}");
}
