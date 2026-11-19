//! A module with an alias definition is an error on a target whose writer emits
//! no alias: the symbol was dropped, and a second file naming it failed to
//! link.

const ALIAS: &str = "@a = alias i32 (), ptr @f

define i32 @f() {
entry:
  ret i32 0
}
";

#[test]
fn an_alias_is_refused_on_the_6502() {
    use llrm_6502::driver::{Level, Request, assembly};
    let error = assembly(ALIAS, &Request::new(Level::O0)).unwrap_err();
    assert!(error.contains("cannot emit the alias"), "{error}");
}
