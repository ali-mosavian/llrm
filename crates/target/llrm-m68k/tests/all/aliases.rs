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
fn an_alias_is_refused_on_m68k() {
    let error = llrm_m68k::driver::program(ALIAS).err().expect("an error");
    assert!(error.contains("cannot emit the alias @a"), "{error}");
}
