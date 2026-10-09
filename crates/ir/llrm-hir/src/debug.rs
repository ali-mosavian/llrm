//! `-g`: builds a module's [`Debug`] as a frontend declares its source
//! types, procedures, parameters and variables. Each type is made once.

use crate::model::{
    Debug, DebugDialect, DebugFunction, DebugGlobal, DebugKind, DebugLanguage, DebugMember, DebugParameter, DebugReach,
    DebugScalar, DebugType, DebugVariable,
};

#[derive(Default)]
pub struct Builder {
    debug: Debug,
    /// The function being compiled.
    parameters: Vec<DebugParameter>,
    variables: Vec<DebugVariable>,
    statics: Vec<DebugGlobal>,
}

impl Builder {
    /// A builder for a program written in `language`.
    pub fn for_language(
        language: DebugLanguage,
        dialect: DebugDialect,
    ) -> Self {
        Self {
            debug: Debug { language: Some(language), dialect: Some(dialect), ..Debug::default() },
            ..Self::default()
        }
    }

    /// The type `kind` and the rest describe, made once.
    fn intern(
        &mut self,
        kind: DebugKind,
        name: &str,
        target: Option<i64>,
        size: i64,
        reach: DebugReach,
        members: Vec<DebugMember>,
    ) -> i64 {
        self.intern_spelled(kind, name, target, size, reach, members, None)
    }

    fn intern_spelled(
        &mut self,
        kind: DebugKind,
        name: &str,
        target: Option<i64>,
        size: i64,
        reach: DebugReach,
        members: Vec<DebugMember>,
        spelling: Option<&str>,
    ) -> i64 {
        let types = &mut self.debug.types;
        let same = |one: &&DebugType| {
            one.kind == kind
                && one.name == name
                && one.target == target
                && one.size == size
                && one.reach == reach
                && one.members == members
                && one.spelling.as_deref() == spelling
        };
        if let Some(one) = types.iter().find(same) {
            return one.id;
        }
        let id = types.len() as i64 + 1;
        types.push(DebugType {
            id,
            kind,
            name: name.to_owned(),
            target,
            size,
            reach,
            members,
            spelling: spelling.map(str::to_owned),
        });
        id
    }

    pub fn scalar(
        &mut self,
        scalar: DebugScalar,
    ) -> i64 {
        self.intern(DebugKind::Scalar, scalar.value(), None, 0, DebugReach::Near, Vec::new())
    }

    /// `scalar` as the source spells it (`unsigned long`): one type for each
    /// spelling, though two share a width.
    pub fn spelled_scalar(
        &mut self,
        scalar: DebugScalar,
        spelling: &str,
    ) -> i64 {
        self.intern_spelled(DebugKind::Scalar, scalar.value(), None, 0, DebugReach::Near, Vec::new(), Some(spelling))
    }

    /// BASIC's `STRING * length`.
    pub fn fixed_string(
        &mut self,
        length: i64,
    ) -> i64 {
        self.intern(DebugKind::FixedString, "", None, length, DebugReach::Near, Vec::new())
    }

    /// BASIC's array of `element`, bounded by its descriptor.
    pub fn array(
        &mut self,
        element: i64,
    ) -> i64 {
        self.intern(DebugKind::Array, "", Some(element), 0, DebugReach::Near, Vec::new())
    }

    /// `bytes` of `element`s in place.
    pub fn sized(
        &mut self,
        element: i64,
        bytes: i64,
    ) -> i64 {
        self.intern(DebugKind::Sized, "", Some(element), bytes, DebugReach::Near, Vec::new())
    }

    /// `name`, `bytes` long, of (field, type, offset, bit field's start and
    /// width) `fields`.
    pub fn structure(
        &mut self,
        name: &str,
        bytes: i64,
        fields: &[(&str, i64, i64, Option<(i64, i64)>)],
    ) -> i64 {
        let members = fields
            .iter()
            .map(|&(name, r#type, offset, bits)| DebugMember {
                name: name.to_owned(),
                r#type,
                offset,
                bit_start: bits.map(|one| one.0),
                bit_width: bits.map(|one| one.1),
            })
            .collect();
        self.intern(DebugKind::Struct, name, None, bytes, DebugReach::Near, members)
    }

    /// A struct or union `name`, `bytes` long, whose members come with
    /// [`define_aggregate`](Self::define_aggregate): one a member can point
    /// to before it has them. Not shared with another of its name and size: two
    /// that read alike are two until each is defined.
    pub fn declare_aggregate(
        &mut self,
        kind: DebugKind,
        name: &str,
        bytes: i64,
    ) -> i64 {
        let id = self.debug.types.len() as i64 + 1;
        self.debug
            .types
            .push(
                DebugType {
                    id,
                    kind,
                    name: name.to_owned(),
                    target: None,
                    size: bytes,
                    reach: DebugReach::Near,
                    members: Vec::new(),
                    spelling: None,
                },
            );
        id
    }

    /// The members of the aggregate `id` declared: (field, type, offset, bit
    /// field's start and width).
    pub fn define_aggregate(
        &mut self,
        id: i64,
        fields: &[(&str, i64, i64, Option<(i64, i64)>)],
    ) {
        let members = fields
            .iter()
            .map(|&(name, r#type, offset, bits)| DebugMember {
                name: name.to_owned(),
                r#type,
                offset,
                bit_start: bits.map(|one| one.0),
                bit_width: bits.map(|one| one.1),
            })
            .collect();
        if let Some(one) = self.debug.types.iter_mut().find(|one| one.id == id) {
            one.members = members;
        }
    }

    pub fn pointer(
        &mut self,
        target: i64,
        reach: DebugReach,
    ) -> i64 {
        self.intern(DebugKind::Pointer, "", Some(target), 0, reach, Vec::new())
    }

    /// A parameter passed by reference to a `target`.
    pub fn reference(
        &mut self,
        target: i64,
    ) -> i64 {
        self.intern(DebugKind::Reference, "", Some(target), 0, DebugReach::Near, Vec::new())
    }

    /// The function's `argument`th parameter, hidden ones counted.
    pub fn parameter(
        &mut self,
        argument: i64,
        name: &str,
        r#type: i64,
    ) {
        self.parameters.push(DebugParameter { argument, name: name.to_owned(), r#type });
    }

    /// A variable of the function, held in `place`.
    pub fn variable(
        &mut self,
        place: i64,
        name: &str,
        r#type: i64,
        parameter: bool,
    ) {
        self.variables.push(DebugVariable { place, name: name.to_owned(), r#type, parameter, argument: None });
    }

    /// A parameter's home, which holds the value of the function's `argument`th
    /// argument once the function has stored it: until then the argument is
    /// where the convention passes it.
    pub fn parameter_home(
        &mut self,
        place: i64,
        name: &str,
        r#type: i64,
        argument: i64,
    ) {
        self.variables.push(DebugVariable {
            place,
            name: name.to_owned(),
            r#type,
            parameter: true,
            argument: Some(argument),
        });
    }

    /// A variable of the module, `offset` bytes into data object `object`.
    pub fn global(
        &mut self,
        object: i64,
        offset: i64,
        name: &str,
        r#type: i64,
    ) {
        self.debug.globals.push(DebugGlobal { function: None, object, offset, name: name.to_owned(), r#type });
    }

    /// A variable of the function, in data: `offset` bytes into `object`.
    pub fn local_static(
        &mut self,
        object: i64,
        offset: i64,
        name: &str,
        r#type: i64,
    ) {
        self.statics.push(DebugGlobal { function: None, object, offset, name: name.to_owned(), r#type });
    }

    /// `place` holds no variable after all.
    pub fn forget(
        &mut self,
        place: i64,
    ) {
        self.variables.retain(|one| one.place != place);
    }

    /// A procedure returning `result`, None nothing, of `parameters`.
    pub fn procedure(
        &mut self,
        result: Option<i64>,
        parameters: &[i64],
    ) -> i64 {
        let members = parameters
            .iter()
            .map(|&r#type| DebugMember { name: String::new(), r#type, offset: 0, bit_start: None, bit_width: None })
            .collect();
        self.intern(DebugKind::Procedure, "", result, 0, DebugReach::Near, members)
    }

    /// The function `function` just compiled, returning `result`: a
    /// procedure of the parameters and variables declared since the last.
    pub fn function(
        &mut self,
        function: i64,
        name: &str,
        result: Option<i64>,
    ) {
        let parameters: Vec<i64> = self.parameters.iter().map(|one| one.r#type).collect();
        let r#type = self.procedure(result, &parameters);
        self.finished(function, false, name, r#type);
    }

    /// [`function`](Self::function), of the procedure type `r#type` the
    /// frontend states.
    pub fn typed_function(
        &mut self,
        function: i64,
        name: &str,
        r#type: i64,
    ) {
        self.finished(function, false, name, r#type);
    }

    /// The module's own code just compiled, `function`: its variables are
    /// the module's.
    pub fn module_code(
        &mut self,
        function: i64,
    ) {
        let r#type = self.procedure(None, &[]);
        self.finished(function, true, "", r#type);
    }

    fn finished(
        &mut self,
        function: i64,
        module: bool,
        name: &str,
        r#type: i64,
    ) {
        let parameters = std::mem::take(&mut self.parameters);
        let statics = std::mem::take(&mut self.statics);
        self.debug.globals.extend(statics.into_iter().map(|one| DebugGlobal { function: Some(function), ..one }));
        let variables = std::mem::take(&mut self.variables);
        self.debug
            .functions
            .push(
                DebugFunction {
                    function,
                    module,
                    name: name.to_owned(),
                    r#type,
                    parameters,
                    variables,
                },
            );
    }

    pub fn built(&self) -> &Debug {
        &self.debug
    }

    pub fn finish(self) -> Debug {
        self.debug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `int` and `long` are both four bytes and signed on a 32-bit target, but
    /// two types: the source's spelling is part of a scalar's identity, and
    /// is shared by every use of it.
    #[test]
    fn scalars_of_one_width_with_two_spellings_are_two_types() {
        let mut builder = Builder::default();
        let (int, long, again) = (
            builder.spelled_scalar(DebugScalar::Int32, "int"),
            builder.spelled_scalar(DebugScalar::Int32, "long"),
            builder.spelled_scalar(DebugScalar::Int32, "int"),
        );
        assert_ne!(int, long);
        assert_eq!(int, again);
        assert_ne!(int, builder.scalar(DebugScalar::Int32), "no spelling is its own type");
    }

    /// Two structs of one name and size that are declared are two until each is
    /// defined, and one that holds a pointer to itself names its own id:
    /// `structure` made it after its members, so that was never possible.
    #[test]
    fn a_declared_aggregate_is_not_shared_and_a_member_may_point_to_it() {
        let mut builder = Builder::default();
        let int = builder.scalar(DebugScalar::Int16);
        let (a, b) = (
            builder.declare_aggregate(DebugKind::Struct, "node", 4),
            builder.declare_aggregate(DebugKind::Struct, "node", 4),
        );
        assert_ne!(a, b);
        let next = builder.pointer(a, DebugReach::Near);
        builder.define_aggregate(a, &[("next", next, 0, None), ("v", int, 2, None)]);
        let made = builder.built();
        let node = made.types.iter().find(|one| one.id == a).expect("the struct");
        assert_eq!(
            node.members.iter().map(|one| (one.name.as_str(), one.r#type)).collect::<Vec<_>>(),
            [("next", next), ("v", int)]
        );
        assert_eq!(made.types.iter().find(|one| one.id == next).and_then(|one| one.target), Some(a));
        assert!(made.types.iter().find(|one| one.id == b).expect("the other").members.is_empty());
    }
}
