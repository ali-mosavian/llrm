//! `clang -msoft-float` marks every function `noimplicitfloat`; the parser
//! refused it ("expected `}`, found `noimplicitfloat`"), so no soft-float
//! program built.

use llrm_mir::parse::module;

#[test]
fn test_a_function_marked_noimplicitfloat_parses() {
    let text = r#"
define float @f(float %a, float %b) #0 {
  %c = fadd float %a, %b
  ret float %c
}
attributes #0 = { noimplicitfloat nounwind "use-soft-float"="true" }
"#;
    assert!(module(text).is_ok(), "{:?}", module(text).err());
}
