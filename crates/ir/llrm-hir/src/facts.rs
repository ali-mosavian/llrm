//! How a frontend states what its language promises: one call, `state`.
//!
//! A fact is a promise a pass may use and may ignore; what changes meaning
//! is an IR field, not a fact (see `llrm_mir::facts`). A `Subject` is an HIR
//! thing, named by HIR ids; MIR has no subjects, only the carriers `lower`
//! makes.

pub use llrm_mir::facts::{Effect, Fact, Kind};

/// A module's functions and instructions by id, found once, for every fact
/// that names one.
pub struct Index<'m> {
    functions: llrm_support::hash::HashMap<i64, &'m crate::model::Function>,
    instructions: llrm_support::hash::HashMap<(i64, i64), &'m crate::model::Instruction>,
    value_types: llrm_support::hash::HashMap<(i64, i64), i64>,
}

impl<'m> Index<'m> {
    pub fn of(module: &'m crate::model::Module) -> Self {
        let functions = module.functions.iter().map(|one| (one.id, one)).collect();
        let instructions = module
            .functions
            .iter()
            .flat_map(|function| function.blocks.iter().flat_map(|block| &block.instructions).map(move |instruction| ((function.id, instruction.id), instruction)))
            .collect();
        let value_types = module.functions.iter().flat_map(|function| function.values.iter().map(move |value| ((function.id, value.id), value.r#type))).collect();
        Self { functions, instructions, value_types }
    }

    pub fn function(&self, id: i64) -> Option<&'m crate::model::Function> {
        self.functions.get(&id).copied()
    }

    /// The HIR type of value `id` of `function`.
    pub fn value_type(&self, function: i64, id: i64) -> Option<i64> {
        self.value_types.get(&(function, id)).copied()
    }

    pub fn instruction(&self, function: i64, id: i64) -> Option<&'m crate::model::Instruction> {
        self.instructions.get(&(function, id)).copied()
    }
}

/// What a fact is stated of, by HIR ids. A routine is one subject, whether
/// the module defines it or only calls it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Subject {
    Callable(i64),
    Param { function: i64, index: i64 },
    Instruction { function: i64, id: i64 },
    /// Operand `operand` of call `instruction`.
    Operand { function: i64, instruction: i64, operand: i64 },
    Object(i64),
    /// The terminator of block `block`.
    Terminator { function: i64, block: i64 },
    /// A place of `function`.
    Place { function: i64, place: i64 },
    /// The member at byte `offset` of aggregate type `owner`.
    Field { owner: i64, offset: i64 },
}

impl Subject {
    pub fn kind(self) -> Kind {
        match self {
            Subject::Callable(_) => Kind::Callable,
            Subject::Param { .. } => Kind::Param,
            Subject::Instruction { .. } => Kind::Instruction,
            Subject::Operand { .. } => Kind::Operand,
            Subject::Object(_) => Kind::Object,
            Subject::Terminator { .. } => Kind::Terminator,
            Subject::Place { .. } => Kind::Place,
            Subject::Field { .. } => Kind::Field,
        }
    }

    /// The wire name of a kind.
    pub fn kind_key(kind: Kind) -> &'static str {
        match kind {
            Kind::Callable => "callable",
            Kind::Param => "param",
            Kind::Instruction => "instruction",
            Kind::Operand => "operand",
            Kind::Object => "object",
            Kind::Terminator => "terminator",
            Kind::Place => "place",
            Kind::Field => "field",
        }
    }

    pub fn kind_named(key: &str) -> Option<Kind> {
        [Kind::Callable, Kind::Param, Kind::Instruction, Kind::Operand, Kind::Object, Kind::Terminator, Kind::Place, Kind::Field].into_iter().find(|&kind| Self::kind_key(kind) == key)
    }

    /// The function a subject belongs to, its own id in it, and, for what
    /// is a part of one thing, which part.
    pub fn fields(self) -> (Option<i64>, Option<i64>, Option<i64>) {
        match self {
            Subject::Callable(id) | Subject::Object(id) => (None, Some(id), None),
            Subject::Param { function, index } => (Some(function), Some(index), None),
            Subject::Instruction { function, id } => (Some(function), Some(id), None),
            Subject::Operand { function, instruction, operand } => (Some(function), Some(instruction), Some(operand)),
            Subject::Terminator { function, block } => (Some(function), Some(block), None),
            Subject::Place { function, place } => (Some(function), Some(place), None),
            Subject::Field { owner, offset } => (None, Some(owner), Some(offset)),
        }
    }

    /// The subject of `kind` with these fields; none where one is missing.
    pub fn of(kind: Kind, function: Option<i64>, id: Option<i64>, part: Option<i64>) -> Option<Subject> {
        Some(match kind {
            Kind::Callable => Subject::Callable(id?),
            Kind::Param => Subject::Param { function: function?, index: id? },
            Kind::Instruction => Subject::Instruction { function: function?, id: id? },
            Kind::Operand => Subject::Operand { function: function?, instruction: id?, operand: part? },
            Kind::Object => Subject::Object(id?),
            Kind::Terminator => Subject::Terminator { function: function?, block: id? },
            Kind::Place => Subject::Place { function: function?, place: id? },
            Kind::Field => Subject::Field { owner: id?, offset: part? },
        })
    }
}

/// A fact, of what, and who said so: the frontend and, where it knows, the
/// source line. Provenance stays in HIR, for bisecting a wrong fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stated {
    pub subject: Subject,
    pub fact: Fact,
    pub source: Option<String>,
}

/// A frontend's facts, stated as it meets them.
#[derive(Clone, Debug, Default)]
pub struct Builder {
    frontend: String,
    stated: Vec<Stated>,
}

impl Builder {
    pub fn new(frontend: &str) -> Self {
        Self { frontend: frontend.to_owned(), stated: Vec::new() }
    }

    /// The language promises `fact` of `subject`.
    pub fn state(&mut self, subject: Subject, fact: Fact) -> &mut Self {
        self.say(subject, fact, self.frontend.clone())
    }

    /// The same, from source line `line`.
    pub fn state_at(&mut self, subject: Subject, fact: Fact, line: i64) -> &mut Self {
        self.say(subject, fact, format!("{}:{line}", self.frontend))
    }

    fn say(&mut self, subject: Subject, fact: Fact, source: String) -> &mut Self {
        if !self.stated.iter().any(|one| one.subject == subject && one.fact == fact) {
            self.stated.push(Stated { subject, fact, source: Some(source) });
        }
        self
    }

    /// Adds facts stated elsewhere, as they were.
    pub fn extend(&mut self, stated: impl IntoIterator<Item = Stated>) {
        for one in stated {
            if !self.stated.iter().any(|have| have.subject == one.subject && have.fact == one.fact) {
                self.stated.push(one);
            }
        }
    }

    /// Keeps the facts `keep` says to: a frontend that drops code drops what
    /// was stated of it.
    pub fn retain(&mut self, keep: impl Fn(&Stated) -> bool) {
        self.stated.retain(keep);
    }

    pub fn finish(self) -> Vec<Stated> {
        self.stated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fact stated twice of one subject is one fact, and the first says where.
    #[test]
    fn a_fact_stated_twice_is_one() {
        let mut builder = Builder::new("c");
        let subject = Subject::Param { function: 1, index: 0 };
        builder.state_at(subject, Fact::NoAlias, 7).state(subject, Fact::NoAlias);
        let stated = builder.finish();
        assert_eq!(stated.len(), 1);
        assert_eq!(stated[0].source.as_deref(), Some("c:7"));
    }

    /// A subject's wire fields name it back.
    #[test]
    fn every_subject_round_trips_through_its_fields() {
        let all = [Subject::Callable(4), Subject::Param { function: 1, index: 2 }, Subject::Instruction { function: 1, id: 5 }, Subject::Operand { function: 1, instruction: 5, operand: 2 }, Subject::Object(7)];
        for subject in all {
            let (function, id, part) = subject.fields();
            assert_eq!(Subject::of(subject.kind(), function, id, part), Some(subject));
            assert_eq!(Subject::kind_named(Subject::kind_key(subject.kind())), Some(subject.kind()));
        }
    }
}
