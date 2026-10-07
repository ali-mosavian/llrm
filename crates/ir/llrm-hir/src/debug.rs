//! `-g`: builds a module's [`Debug`] as a frontend declares its source
//! types, procedures, parameters and variables. Each type is made once.

use crate::model::{Debug, DebugFunction, DebugGlobal, DebugKind, DebugMember, DebugParameter, DebugReach, DebugScalar, DebugType, DebugVariable};

#[derive(Default)]
pub struct Builder {
    debug: Debug,
    /// The function being compiled.
    parameters: Vec<DebugParameter>,
    variables: Vec<DebugVariable>,
    statics: Vec<DebugGlobal>,
}

impl Builder {
    /// The type `kind` and the rest describe, made once.
    fn intern(&mut self, kind: DebugKind, name: &str, target: Option<i64>, size: i64, reach: DebugReach, members: Vec<DebugMember>) -> i64 {
        let types = &mut self.debug.types;
        let same = |one: &&DebugType| {
            one.kind == kind && one.name == name && one.target == target && one.size == size && one.reach == reach && one.members == members
        };
        if let Some(one) = types.iter().find(same) {
            return one.id;
        }
        let id = types.len() as i64 + 1;
        types.push(DebugType { id, kind, name: name.to_owned(), target, size, reach, members });
        id
    }

    pub fn scalar(&mut self, scalar: DebugScalar) -> i64 {
        self.intern(DebugKind::Scalar, scalar.value(), None, 0, DebugReach::Near, Vec::new())
    }

    /// BASIC's `STRING * length`.
    pub fn fixed_string(&mut self, length: i64) -> i64 {
        self.intern(DebugKind::FixedString, "", None, length, DebugReach::Near, Vec::new())
    }

    /// BASIC's array of `element`, bounded by its descriptor.
    pub fn array(&mut self, element: i64) -> i64 {
        self.intern(DebugKind::Array, "", Some(element), 0, DebugReach::Near, Vec::new())
    }

    /// `bytes` of `element`s in place.
    pub fn sized(&mut self, element: i64, bytes: i64) -> i64 {
        self.intern(DebugKind::Sized, "", Some(element), bytes, DebugReach::Near, Vec::new())
    }

    /// `name`, `bytes` long, of (field, type, offset, bit field's start and
    /// width) `fields`.
    pub fn structure(&mut self, name: &str, bytes: i64, fields: &[(&str, i64, i64, Option<(i64, i64)>)]) -> i64 {
        let members = fields
            .iter()
            .map(|&(name, r#type, offset, bits)| DebugMember { name: name.to_owned(), r#type, offset, bit_start: bits.map(|one| one.0), bit_width: bits.map(|one| one.1) })
            .collect();
        self.intern(DebugKind::Struct, name, None, bytes, DebugReach::Near, members)
    }

    pub fn pointer(&mut self, target: i64, reach: DebugReach) -> i64 {
        self.intern(DebugKind::Pointer, "", Some(target), 0, reach, Vec::new())
    }

    /// A parameter passed by reference to a `target`.
    pub fn reference(&mut self, target: i64) -> i64 {
        self.intern(DebugKind::Reference, "", Some(target), 0, DebugReach::Near, Vec::new())
    }

    /// The function's `argument`th parameter, hidden ones counted.
    pub fn parameter(&mut self, argument: i64, name: &str, r#type: i64) {
        self.parameters.push(DebugParameter { argument, name: name.to_owned(), r#type });
    }

    /// A variable of the function, held in `place`.
    pub fn variable(&mut self, place: i64, name: &str, r#type: i64, parameter: bool) {
        self.variables.push(DebugVariable { place, name: name.to_owned(), r#type, parameter });
    }

    /// A variable of the module, `offset` bytes into data object `object`.
    pub fn global(&mut self, object: i64, offset: i64, name: &str, r#type: i64) {
        self.debug.globals.push(DebugGlobal { function: None, object, offset, name: name.to_owned(), r#type });
    }

    /// A variable of the function, in data: `offset` bytes into `object`.
    pub fn local_static(&mut self, object: i64, offset: i64, name: &str, r#type: i64) {
        self.statics.push(DebugGlobal { function: None, object, offset, name: name.to_owned(), r#type });
    }

    /// `place` holds no variable after all.
    pub fn forget(&mut self, place: i64) {
        self.variables.retain(|one| one.place != place);
    }

    /// A procedure returning `result`, None nothing, of `parameters`.
    pub fn procedure(&mut self, result: Option<i64>, parameters: &[i64]) -> i64 {
        let members = parameters.iter().map(|&r#type| DebugMember { name: String::new(), r#type, offset: 0, bit_start: None, bit_width: None }).collect();
        self.intern(DebugKind::Procedure, "", result, 0, DebugReach::Near, members)
    }

    /// The function `function` just compiled, returning `result`: a
    /// procedure of the parameters and variables declared since the last.
    pub fn function(&mut self, function: i64, name: &str, result: Option<i64>) {
        let parameters: Vec<i64> = self.parameters.iter().map(|one| one.r#type).collect();
        let r#type = self.procedure(result, &parameters);
        self.finished(function, false, name, r#type);
    }

    /// [`function`](Self::function), of the procedure type `r#type` the
    /// frontend states.
    pub fn typed_function(&mut self, function: i64, name: &str, r#type: i64) {
        self.finished(function, false, name, r#type);
    }

    /// The module's own code just compiled, `function`: its variables are
    /// the module's.
    pub fn module_code(&mut self, function: i64) {
        let r#type = self.procedure(None, &[]);
        self.finished(function, true, "", r#type);
    }

    fn finished(&mut self, function: i64, module: bool, name: &str, r#type: i64) {
        let parameters = std::mem::take(&mut self.parameters);
        let statics = std::mem::take(&mut self.statics);
        self.debug.globals.extend(statics.into_iter().map(|one| DebugGlobal { function: Some(function), ..one }));
        let variables = std::mem::take(&mut self.variables);
        self.debug.functions.push(DebugFunction { function, module, name: name.to_owned(), r#type, parameters, variables });
    }

    pub fn built(&self) -> &Debug {
        &self.debug
    }

    pub fn finish(self) -> Debug {
        self.debug
    }
}
