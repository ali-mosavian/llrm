use std::collections::BTreeSet;

use crate::support::diagnostic::{Diagnostic, Severity};

use super::{
    BlockId, CallableId, DataId, FORMAT_VERSION, Function, InstructionId, Module, Operand, Program,
    Terminator, TypeId, ValueId,
};

pub fn verify(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut verifier = Verifier {
        diagnostics: Vec::new(),
    };
    verifier.program(program);
    if verifier.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(verifier.diagnostics)
    }
}

impl Program {
    pub fn verify(&self) -> Result<(), Vec<Diagnostic>> {
        verify(self)
    }
}

struct Verifier {
    diagnostics: Vec<Diagnostic>,
}

impl Verifier {
    fn program(&mut self, program: &Program) {
        if program.version != FORMAT_VERSION {
            self.error(format!(
                "unsupported qhir version {}; expected {FORMAT_VERSION}",
                program.version
            ));
        }
        if program.modules.is_empty() {
            self.error("program has no modules");
        }

        let mut ids = BTreeSet::new();
        for module in &program.modules {
            if !ids.insert(module.id) {
                self.error(format!("duplicate module {}", module.id));
            }
            self.module(module);
        }
    }

    fn module(&mut self, module: &Module) {
        let type_ids = collect_ids(
            module.types.iter().map(|type_| type_.id),
            "type",
            &mut self.diagnostics,
        );
        let data_ids = collect_ids(
            module.data.iter().map(|data| data.id),
            "data object",
            &mut self.diagnostics,
        );
        let callable_ids = collect_ids(
            module.callables.iter().map(|callable| callable.id),
            "callable",
            &mut self.diagnostics,
        );
        collect_ids(
            module.functions.iter().map(|function| function.id),
            "function",
            &mut self.diagnostics,
        );

        for type_ in &module.types {
            if let Some(element) = type_.element {
                self.type_exists(element, &type_ids, "type element");
            }
            for (lower, upper) in &type_.bounds {
                if lower > upper {
                    self.error(format!(
                        "type {} has inverted bound {lower} to {upper}",
                        type_.id
                    ));
                }
            }
        }

        for callable in &module.callables {
            if let Some(result) = callable.result_type {
                self.type_exists(result, &type_ids, "callable result");
            }
            for parameter in &callable.parameters {
                self.type_exists(parameter.type_id, &type_ids, "callable parameter");
            }
        }

        for data in &module.data {
            for relocation in &data.relocations {
                if relocation.at >= data.bytes.len() {
                    self.error(format!(
                        "relocation at {} lies outside data object {}",
                        relocation.at, data.id
                    ));
                }
                if !data_ids.contains(&relocation.target) {
                    self.error(format!(
                        "data object {} relocates to unknown data object {}",
                        data.id, relocation.target
                    ));
                }
            }
        }

        for function in &module.functions {
            self.function(function, &type_ids, &data_ids, &callable_ids);
        }
    }

    fn function(
        &mut self,
        function: &Function,
        type_ids: &BTreeSet<TypeId>,
        data_ids: &BTreeSet<DataId>,
        callable_ids: &BTreeSet<CallableId>,
    ) {
        self.type_exists(function.result_type, type_ids, "function result");

        let value_ids = collect_ids(
            function.values.iter().map(|value| value.id),
            "value",
            &mut self.diagnostics,
        );
        let place_ids = collect_ids(
            function.places.iter().map(|place| place.id),
            "place",
            &mut self.diagnostics,
        );
        let block_ids = collect_ids(
            function.blocks.iter().map(|block| block.id),
            "block",
            &mut self.diagnostics,
        );

        if !block_ids.contains(&function.entry) {
            self.error(format!(
                "function {} has unknown entry block {}",
                function.id, function.entry
            ));
        }
        if let Some(handler) = function.error_handler {
            self.block_exists(handler, &block_ids, "error handler");
        }
        for entry in &function.external_entries {
            self.block_exists(*entry, &block_ids, "external entry");
        }
        for parameter in &function.parameters {
            self.value_exists(*parameter, &value_ids, "parameter");
        }
        for value in &function.values {
            self.type_exists(value.type_id, type_ids, "value");
        }
        for place in &function.places {
            self.type_exists(place.type_id, type_ids, "place");
            if place.symbol.get() != 0 && !data_ids.contains(&place.symbol) {
                self.error(format!(
                    "place {} refers to unknown data object {}",
                    place.id, place.symbol
                ));
            }
        }

        let mut instruction_ids = BTreeSet::new();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if !instruction_ids.insert(instruction.id) {
                    self.error(format!("duplicate instruction {}", instruction.id));
                }
                for result in &instruction.results {
                    self.value_exists(*result, &value_ids, "instruction result");
                }
                for operand in &instruction.operands {
                    self.operand(operand, type_ids, &value_ids, &place_ids);
                }
            }
            self.terminator(
                &block.terminator,
                type_ids,
                &value_ids,
                &place_ids,
                &block_ids,
            );
        }

        for call in &function.calls {
            self.instruction_exists(call.instruction, &instruction_ids, "call ABI");
            if let Some(callee) = call.callee {
                if !callable_ids.contains(&callee) {
                    self.error(format!("call ABI refers to unknown callable {callee}"));
                }
            }
        }
    }

    fn terminator(
        &mut self,
        terminator: &Terminator,
        type_ids: &BTreeSet<TypeId>,
        value_ids: &BTreeSet<ValueId>,
        place_ids: &BTreeSet<super::PlaceId>,
        block_ids: &BTreeSet<BlockId>,
    ) {
        match terminator {
            Terminator::Jump(target) => self.block_exists(*target, block_ids, "jump"),
            Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                self.operand(condition, type_ids, value_ids, place_ids);
                self.block_exists(*then_block, block_ids, "branch");
                self.block_exists(*else_block, block_ids, "branch");
            }
            Terminator::Switch {
                selector,
                cases,
                default,
            } => {
                self.operand(selector, type_ids, value_ids, place_ids);
                let mut values = BTreeSet::new();
                for (value, target) in cases {
                    if !values.insert(*value) {
                        self.error(format!("duplicate switch case {value}"));
                    }
                    self.block_exists(*target, block_ids, "switch");
                }
                self.block_exists(*default, block_ids, "switch default");
            }
            Terminator::Return(Some(value)) => {
                self.operand(value, type_ids, value_ids, place_ids);
            }
            Terminator::Return(None) | Terminator::Unreachable => {}
        }
    }

    fn operand(
        &mut self,
        operand: &Operand,
        type_ids: &BTreeSet<TypeId>,
        value_ids: &BTreeSet<ValueId>,
        place_ids: &BTreeSet<super::PlaceId>,
    ) {
        match operand {
            Operand::Value(value) => self.value_exists(*value, value_ids, "operand"),
            Operand::Constant { type_id, .. } => self.type_exists(*type_id, type_ids, "constant"),
            Operand::Place(place) => {
                if !place_ids.contains(place) {
                    self.error(format!("operand refers to unknown place {place}"));
                }
            }
            Operand::Element { place, indices } => {
                if !place_ids.contains(place) {
                    self.error(format!("element refers to unknown place {place}"));
                }
                for index in indices {
                    self.operand(index, type_ids, value_ids, place_ids);
                }
            }
            Operand::Projection {
                place,
                indices,
                type_id,
                ..
            } => {
                if !place_ids.contains(place) {
                    self.error(format!("projection refers to unknown place {place}"));
                }
                self.type_exists(*type_id, type_ids, "projection");
                for index in indices {
                    self.operand(index, type_ids, value_ids, place_ids);
                }
            }
            Operand::Indirect { base, type_id, .. } => {
                self.value_exists(*base, value_ids, "indirect base");
                self.type_exists(*type_id, type_ids, "indirect result");
            }
        }
    }

    fn type_exists(&mut self, id: TypeId, ids: &BTreeSet<TypeId>, context: &str) {
        if !ids.contains(&id) {
            self.error(format!("{context} refers to unknown type {id}"));
        }
    }

    fn value_exists(&mut self, id: ValueId, ids: &BTreeSet<ValueId>, context: &str) {
        if !ids.contains(&id) {
            self.error(format!("{context} refers to unknown value {id}"));
        }
    }

    fn block_exists(&mut self, id: BlockId, ids: &BTreeSet<BlockId>, context: &str) {
        if !ids.contains(&id) {
            self.error(format!("{context} refers to unknown block {id}"));
        }
    }

    fn instruction_exists(
        &mut self,
        id: InstructionId,
        ids: &BTreeSet<InstructionId>,
        context: &str,
    ) {
        if !ids.contains(&id) {
            self.error(format!("{context} refers to unknown instruction {id}"));
        }
    }

    fn error(&mut self, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::new(Severity::Error, message));
    }
}

fn collect_ids<I, T>(ids: I, kind: &str, diagnostics: &mut Vec<Diagnostic>) -> BTreeSet<T>
where
    I: IntoIterator<Item = T>,
    T: Copy + Ord + std::fmt::Display,
{
    let mut collected = BTreeSet::new();
    for id in ids {
        if !collected.insert(id) {
            diagnostics.push(Diagnostic::new(
                Severity::Error,
                format!("duplicate {kind} {id}"),
            ));
        }
    }
    collected
}
