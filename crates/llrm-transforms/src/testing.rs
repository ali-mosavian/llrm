//! What the tests share: a module read from its text, printed once it
//! verifies, and run.

use llrm_mir::interpret::{self, Val};
use llrm_mir::module::{Function, Module};

pub fn parsed(text: &str) -> Module {
    llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

/// `module`'s text, which must verify.
pub fn printed(module: &Module) -> String {
    let text = llrm_mir::print::module(module);
    assert_eq!(llrm_mir::verify::verify(module), Vec::<String>::new(), "{text}");
    text
}

/// @f of `module`.
pub fn f(module: &mut Module) -> &mut Function {
    module.function_mut("f").expect("@f").1
}

/// What @f returns for each of `inputs`, each argument an integer of the
/// width `@f` declares; it must return.
pub fn results(module: &Module, inputs: &[&[i128]]) -> Vec<Val> {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let widths = function.parameters().iter().map(|&one| module.context.types.int_bits(function.value(one).ty).expect("an integer")).collect::<Vec<_>>();
    inputs
        .iter()
        .map(|input| {
            let arguments = input.iter().zip(&widths).map(|(&value, &width)| Val::Int { bits: llrm_mir::context::mask(width) & value as u128, width }).collect();
            interpret::run(module, "f", arguments, 100_000).unwrap_or_else(|trap| panic!("@f{input:?}: {trap:?}"))
        })
        .collect()
}
