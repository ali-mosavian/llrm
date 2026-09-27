//! What the tests share: a module read from its text.

use llrm_mir::module::{Function, Module};

pub fn parsed(text: &str) -> Module {
    llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

/// The function `name` of `module`.
pub fn function<'m>(module: &'m Module, name: &str) -> &'m Function {
    let global = module.named(name).unwrap_or_else(|| panic!("no @{name}"));
    module.global(global).function().expect("a function")
}
