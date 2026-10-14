use llrm_mir::program::Program;

use crate::pipeline::{self, Applied};
use crate::testing::{parsed, printed, results};

/// A global only loaded is constant, and a load of it its initializer;
/// one another body stores stays a variable. fpdeep's BC_CN literals were
/// loaded, never folded.
#[test]
fn a_global_only_loaded_is_its_initializer() {
    let text = format!(
        "{}@c = internal global [2 x i16] [i16 5, i16 12]
@w = internal global i16 7

define i16 @f(i16 %x) {{
b0:
  %at = getelementptr i8, ptr @c, i16 2
  %a = load i16, ptr %at
  %b = load i16, ptr @w
  %s = add i16 %a, %b
  ret i16 %s
}}

define void @g(i16 %x) {{
b0:
  store i16 %x, ptr @w
  ret void
}}
",
        llrm_analysis::testing::DOS
    );
    let mut module = parsed(&text);
    let before = results(&module, &[&[0]]);
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, &[&[0]]), before, "{after}");
    // @c, folded into its one load, is dead and gone.
    assert!(!after.contains("@c = internal global") && after.contains("@w = internal global"), "{after}");
    assert!(!after.contains("load i16, ptr %at") && after.contains("load i16, ptr @w"), "{after}");
}

/// `module` through the pipeline, printed.
fn piped(text: &str) -> String {
    let mut module = parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    printed(&module)
}

/// A global stored to and never loaded, forwarding having taken its one load,
/// kept its stores and itself (#1621): LLVM's GlobalOpt and gcc's ipa find it
/// dead. Both bodies lose the stores and the variable goes.
#[test]
fn a_global_only_stored_loses_its_stores_and_itself() {
    let after = piped(
        "@t = internal global [2 x i8] zeroinitializer
@out = global i8 0

define void @f() {
b0:
  store i8 8, ptr @t
  %at = getelementptr i8, ptr @t, i16 1
  store i8 9, ptr %at
  %a = load i8, ptr @t
  store volatile i8 %a, ptr @out
  ret void
}

define void @g(i8 %x) {
b0:
  store i8 %x, ptr @t
  ret void
}
",
    );
    assert!(!after.contains("@t"), "a store or the variable is left: {after}");
}

/// What something loads, escapes or must see keeps its stores.
#[test]
fn a_global_that_is_loaded_escapes_or_is_stored_volatile_keeps_its_stores() {
    let loaded = piped(
        "@t = internal global i8 0
@out = global i8 0

define void @f(i8 %x) {
b0:
  store i8 %x, ptr @t
  ret void
}

define void @g() {
b0:
  %a = load i8, ptr @t
  store volatile i8 %a, ptr @out
  ret void
}
",
    );
    assert!(loaded.contains("store i8 %x, ptr @t"), "{loaded}");
    let escaped = piped(
        "@t = internal global i8 0
@slot = global ptr null
declare void @use(ptr)

define void @f(i8 %x) {
b0:
  store i8 %x, ptr @t
  call void @use(ptr @t)
  ret void
}
",
    );
    assert!(escaped.contains("store i8 %x, ptr @t"), "{escaped}");
    let volatile = piped(
        "@t = internal global i8 0

define void @f(i8 %x) {
b0:
  store volatile i8 %x, ptr @t
  ret void
}
",
    );
    assert!(volatile.contains("store volatile i8 %x, ptr @t"), "{volatile}");
}
