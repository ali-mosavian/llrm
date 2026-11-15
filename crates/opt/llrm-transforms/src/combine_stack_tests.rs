//! `combine_stack`: a `stackrestore` nothing needs is gone, and its save with
//! it.

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
        |c, inst| {
            if matches!(
                c.function.instruction(inst).opcode,
                llrm_mir::opcode::Opcode::Call(_)
            ) {
                c.visit_call(inst)
            } else {
                crate::combine::Visit::None
            }
        },
        &mut declared,
    );
    declared.place(&mut module).unwrap();
    printed(&module)
}

const DECLARATIONS: &str = "declare ptr @llvm.stacksave.p0()
declare void @llvm.stackrestore.p0(ptr)
";

/// vla_callee_constant -O3 kept seven empty `stacksave`/`stackrestore` pairs
/// (+6 instructions, +18 B over the pin): the pin removes a restore that
/// follows its save with no alloca between, and then the dead save.
#[test]
fn a_restore_right_after_its_save_goes_with_the_save() {
    let after = combined(&format!(
        "{DECLARATIONS}
define i32 @f(i32 %x) {{
b0:
  %sp = call ptr @llvm.stacksave.p0()
  call void @llvm.stackrestore.p0(ptr %sp)
  br label %b1
b1:
  ret i32 %x
}}
"
    ));
    assert!(!after.contains("call ptr @llvm.stacksave"), "{after}");
    assert!(!after.contains("call void @llvm.stackrestore"), "{after}");
}

/// An alloca between the save and the restore: the restore is what frees it.
#[test]
fn a_restore_after_an_alloca_stays() {
    let after = combined(&format!(
        "{DECLARATIONS}
define i32 @f(i32 %x) {{
b0:
  %sp = call ptr @llvm.stacksave.p0()
  %a = alloca i8, i32 %x, align 1
  store i8 1, ptr %a, align 1
  call void @llvm.stackrestore.p0(ptr %sp)
  br label %b1
b1:
  ret i32 %x
}}
"
    ));
    assert!(after.contains("call void @llvm.stackrestore"), "{after}");
}

/// The earlier of two restores in a block, with no alloca or call between,
/// is covered by the later one.
#[test]
fn the_first_of_two_restores_in_a_block_goes() {
    let after = combined(&format!(
        "{DECLARATIONS}
define i32 @f(i32 %x) {{
b0:
  %outer = call ptr @llvm.stacksave.p0()
  %inner = call ptr @llvm.stacksave.p0()
  %a = alloca i8, i32 %x, align 1
  store i8 1, ptr %a, align 1
  call void @llvm.stackrestore.p0(ptr %inner)
  call void @llvm.stackrestore.p0(ptr %outer)
  br label %b1
b1:
  ret i32 %x
}}
"
    ));
    assert_eq!(after.matches("call void @llvm.stackrestore").count(), 1, "{after}");
    assert!(after.contains("stackrestore.p0(ptr %outer)"), "{after}");
}

/// A restore in a block that returns, with no alloca or call after it, frees
/// nothing the caller does not.
#[test]
fn a_restore_before_a_return_goes() {
    let after = combined(&format!(
        "{DECLARATIONS}
define i32 @f(i32 %x) {{
b0:
  %sp = call ptr @llvm.stacksave.p0()
  %a = alloca i8, i32 %x, align 1
  store i8 1, ptr %a, align 1
  call void @llvm.stackrestore.p0(ptr %sp)
  ret i32 %x
}}
"
    ));
    assert!(!after.contains("call void @llvm.stackrestore"), "{after}");
}
