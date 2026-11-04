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
fn an_alias_is_refused_on_arm64() {
    use llrm_arm64::driver::{platform_or_default, program};
    let platform = platform_or_default("arm64-apple-macosx").expect("a platform");
    let error = program(ALIAS, platform).err().expect("an error");
    assert!(error.contains("cannot emit the alias @a"), "{error}");
}
