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

/// The parameter or instruction result of `function` named `name`.
pub fn value(function: &Function, name: &str) -> llrm_mir::module::ValueId {
    let results = function.walk().filter_map(|(_, inst)| function.instruction(inst).result);
    function.parameters().iter().copied().chain(results).find(|&one| function.value(one).name.as_deref() == Some(name)).unwrap_or_else(|| panic!("no %{name}"))
}
