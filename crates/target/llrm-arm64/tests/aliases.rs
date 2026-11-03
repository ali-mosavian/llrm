//! An alias is a second symbol at its aliasee's address; on the text path,
//! which has no symbol to name it, it is an error: the symbol was dropped, and
//! a second file naming it failed to link.

use llrm_arm64::driver::{assembly, object, platform_or_default};
use llrm_arm64::machine::Platform;
use llrm_object::Definition;

const ALIAS: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"

@a = alias i32 (), ptr @f

define i32 @f() {
entry:
  ret i32 0
}
";

fn linux() -> Platform {
    platform_or_default("aarch64-unknown-linux-gnu").expect("a platform")
}

/// `@a = alias ..., @f` was refused on arm64 ("cannot emit the alias @a"); the
/// object now defines `a` where `f` is, as the pin's `.set a, f`.
#[test]
fn an_alias_is_a_symbol_at_its_aliasees_address() {
    let image = object(ALIAS, linux(), "t", true).expect("an object");
    let place = |name: &str| {
        let symbol =
            image.symbols.iter().find(|symbol| symbol.name == name).unwrap_or_else(|| panic!("no symbol {name}"));
        match symbol.definition {
            Definition::Defined { section, offset } => (section, offset),
            _ => panic!("{name} is undefined"),
        }
    };
    assert_eq!(place("a"), place("f"));
}

#[test]
fn an_alias_is_refused_in_assembly_text() {
    let error = assembly(ALIAS, linux(), true).err().expect("an error");
    assert!(error.contains("cannot emit the alias @a"), "{error}");
}
