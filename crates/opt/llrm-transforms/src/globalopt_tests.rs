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
