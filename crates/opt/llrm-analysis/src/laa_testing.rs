//! How `opt` writes the values `print<access-info>` lists, for the tests that
//! compare with it: the instruction lines of the function as MIR prints them
//! (the same text, in order, for the same names), and an argument as `ptr %x`.
//!
//! # Example
//! `%0 = load i16, ptr %p` as the function prints it is `%0 = load i16, ptr %p`
//! here, and an argument `%p` is `ptr %p`, so a test can compare the pin's
//! `print<access-info>` line with the pass's.
//!
//! # Used by
//! The tests of the `loopaccessinfo` and `runtimecheck` analyses, which compare
//! their output with the pin's `print<access-info>`.

use llrm_mir::module::{Function, InstId, Module, Operand, ValueDef};
use llrm_mir::types::Type;
use llrm_support::hash::HashMap;

use crate::runtimecheck::ValueText;

pub struct PinText {
    instructions: llrm_mir::dense::IdMap<InstId, String>,
    values: HashMap<Operand, String>,
}

impl PinText {
    /// The text of `function` in `module` (the only function the module
    /// defines).
    pub fn of(
        module: &Module,
        function: &Function,
    ) -> Self {
        let printed = llrm_mir::print::module(module);
        let mut lines = printed
            .lines()
            .skip_while(|line| !line.starts_with("define "))
            .skip(1)
            .take_while(|line| *line != "}")
            .filter(|line| line.starts_with("  "));
        let mut instructions = llrm_mir::dense::IdMap::new();
        let mut values = HashMap::default();
        for (_, inst) in function.walk() {
            let line = lines.next().expect("a line for each instruction");
            instructions.insert(inst, line.to_owned());
            if let Some(result) = function.instruction(inst).result {
                values.insert(Operand::Value(result), line.to_owned());
            }
        }
        for &parameter in function.parameters() {
            let ty = match module.context.types.get(function.value(parameter).ty) {
                Type::Pointer(_) => "ptr".to_owned(),
                Type::Int(bits) => format!("i{bits}"),
                other => format!("{other:?}"),
            };
            let name = function.value(parameter).name.clone().unwrap_or_default();
            debug_assert!(matches!(function.value(parameter).def, ValueDef::Argument(_)));
            values.insert(Operand::Value(parameter), format!("{ty} %{name}"));
        }
        Self { instructions, values }
    }
}

impl ValueText for PinText {
    fn value(
        &self,
        value: Operand,
    ) -> String {
        self.values.get(&value).cloned().unwrap_or_else(|| format!("{value:?}"))
    }

    fn instruction(
        &self,
        inst: InstId,
    ) -> String {
        self.instructions[&inst].clone()
    }
}
