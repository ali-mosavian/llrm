//! `combine_returned_arg`: a call returning one of its arguments is that
//! argument.

use llrm_mir::datalayout::DataLayout;

use crate::combine::combine_with_sinking;
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let mut declared = llrm_mir::passes::Declared::over(std::rc::Rc::new(globals.to_vec()), metadata.len());
    combine_with_sinking(
        context,
        &globals,
        &callees,
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
        false,
        &mut declared,
    );
    printed(&module)
}

/// `report(m1(x))` with `m1` returning its argument kept the call and passed
/// its result on: wasm mulconst_i64 +22 B over the pin, which passes `x`.
#[test]
fn a_use_of_a_call_returning_its_argument_reads_the_argument() {
    let out = combined(
        "declare i32 @id(i32 returned)
declare void @use(i32)

define void @f(i32 %x) {
b0:
  %r = call i32 @id(i32 %x)
  call void @use(i32 %r)
  ret void
}
",
    );
    assert!(out.contains("call void @use(i32 %x)"), "{out}");
}

/// A callee with no `returned` parameter is not that argument.
#[test]
fn a_call_with_no_returned_parameter_is_left() {
    let out = combined(
        "declare i32 @id(i32)
declare void @use(i32)

define void @f(i32 %x) {
b0:
  %r = call i32 @id(i32 %x)
  call void @use(i32 %r)
  ret void
}
",
    );
    assert!(out.contains("call void @use(i32 %r)"), "{out}");
}
