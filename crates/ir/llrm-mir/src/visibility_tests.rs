use crate::module::{Linkage, Visibility};
use crate::{parse, print};

const WASM_MAIN: &str = "@__main_void = hidden alias i32 (), ptr @main

define hidden i32 @main() {
entry:
  ret i32 0
}
";

/// clang's wasm32 IR for any `int main(void)` had `define hidden` and a
/// `hidden alias`; the parser stopped at "expected a type, found `hidden`" and
/// no C program reached the wasm backend.
#[test]
fn a_hidden_function_and_its_alias_round_trip() {
    let module = parse::module(WASM_MAIN).unwrap();
    assert_eq!(module.globals[0].visibility, Visibility::Hidden);
    assert_eq!(module.aliases.len(), 1);
    let alias = &module.aliases[0];
    assert_eq!(
        (alias.name.as_str(), alias.visibility, alias.linkage),
        ("__main_void", Visibility::Hidden, Linkage::External)
    );
    assert_eq!(module.globals[alias.aliasee.0 as usize].name.as_deref(), Some("main"));
    assert_eq!(print::module(&module), WASM_MAIN);
}

#[test]
fn a_protected_variable_keeps_its_visibility() {
    let text = "@v = protected global i32 3\n";
    let module = parse::module(text).unwrap();
    assert_eq!(module.globals[0].visibility, Visibility::Protected);
    assert_eq!(print::module(&module), text);
}

/// `int other(int) __attribute__((alias("real")))` called from `main`: the
/// call named the alias, which the parser refused ("@other is an alias, which
/// MIR takes as no operand"), so the C failed on every target.
#[test]
fn an_operand_naming_an_alias_names_its_aliasee() {
    let text = "@a = alias i32, ptr @v
@v = global i32 3

define i32 @f() {
entry:
  %x = load i32, ptr @a
  ret i32 %x
}
";
    let module = parse::module(text).unwrap();
    assert_eq!(module.aliases.len(), 1);
    assert!(print::module(&module).contains("load i32, ptr @v"), "{}", print::module(&module));
}

#[test]
fn an_operand_naming_a_weak_alias_is_refused() {
    let text = "@a = weak alias i32, ptr @v
@v = global i32 3

define i32 @f() {
entry:
  %x = load i32, ptr @a
  ret i32 %x
}
";
    let error = parse::module(text).unwrap_err();
    assert!(error.to_string().contains("@a is an alias a linker may replace"), "{error}");
}

/// A pass that deleted a dead global renumbered the rest and left each alias
/// on the old number: the mid-end at -O2 made `__main_void` alias a
/// declaration, and the backend refused the module.
#[test]
fn an_alias_follows_its_aliasee_when_a_global_before_it_is_deleted() {
    let text = "@dead = internal global i32 1
@a = alias i32, ptr @v
@v = global i32 3
";
    let mut module = parse::module(text).unwrap();
    let dead = module.named("dead").unwrap();
    module.retain_globals(&|id| id != dead);
    let aliasee = module.aliases[0].aliasee;
    assert_eq!(module.global(aliasee).name.as_deref(), Some("v"));
    assert_eq!(print::module(&module), "@v = global i32 3\n\n@a = alias i32, ptr @v\n");
}
