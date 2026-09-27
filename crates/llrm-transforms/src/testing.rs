//! What the tests share: a module read from its text, printed once it
//! verifies, and run.

use llrm_mir::interpret::{self, Val};
use llrm_mir::GlobalId;
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

/// @f's @g holds 7 across a call to a `readonly` @peek: only @peek's
/// declaration, read of the module, says the call leaves @g alone.
pub const ACROSS_READONLY_CALL: &str = "@g = global i16 0

declare i16 @peek() readonly

define i16 @f() {
b0:
  store i16 7, ptr @g
  %p = call i16 @peek()
  %v = load i16, ptr @g
  ret i16 %v
}
";

/// `module` through `pass` under the pass manager, printed.
pub fn managed(module: &mut Module, pass: impl llrm_mir::passes::FunctionPass + 'static) -> String {
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(pass);
    manager.run(module).unwrap();
    printed(module)
}

/// @h writes only what its argument points to, which only its summary
/// says: a call to it without `Summaries` may write anything.
pub const WRITES_ITS_ARGUMENT: &str = "@g = global [64 x i8] zeroinitializer
@k = global i16 0

define void @h(ptr %s) {
b0:
  store i16 9, ptr %s
  ret void
}

";

/// `module` through `pass` under a pass manager that requires `Summaries`
/// where `summaries`, as LLVM's `RequireAnalysisPass`, or a bare one;
/// printed. @f computes what it did on `inputs`.
pub fn summarized(module: &Module, pass: impl llrm_mir::passes::FunctionPass + 'static, summaries: bool, inputs: &[&[i128]]) -> String {
    let mut after = module.clone();
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.verify_each = true;
    if summaries {
        manager.require::<llrm_analysis::manager::Summaries>();
    }
    manager.add(pass);
    manager.run(&mut after).unwrap();
    let text = printed(&after);
    assert_eq!(results(&after, inputs), results(module, inputs), "{text}");
    text[text.find("@f(").expect("@f")..].to_owned()
}

/// Every function of `module` that has a body.
pub fn bodies(module: &Module) -> Vec<GlobalId> {
    module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(id, _, _)| id).collect()
}
