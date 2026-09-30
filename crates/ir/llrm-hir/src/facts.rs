//! How a frontend states what its language promises: one call, `state`.
//!
//! A fact is a promise a pass may use and may ignore; what changes meaning
//! is an IR field, not a fact (see `llrm_mir::facts`). A `Subject` is an HIR
//! thing, named by HIR ids; MIR has no subjects, only the carriers `lower`
//! makes.

use llrm_mir::facts::{Fact, Kind};

/// What a fact is stated of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Subject {
    Function(i64),
    Param { function: i64, index: i64 },
    Return(i64),
    Instruction { function: i64, id: i64 },
    Block { function: i64, id: i64 },
    Place { function: i64, id: i64 },
    Object(i64),
    Program,
}

impl Subject {
    pub fn kind(self) -> Kind {
        match self {
            Subject::Function(_) => Kind::Function,
            Subject::Param { .. } => Kind::Param,
            Subject::Return(_) => Kind::Return,
            Subject::Instruction { .. } => Kind::Instruction,
            Subject::Block { .. } => Kind::Block,
            Subject::Place { .. } => Kind::Place,
            Subject::Object(_) => Kind::Object,
            Subject::Program => Kind::Program,
        }
    }

    /// The wire name of a kind.
    pub fn kind_key(kind: Kind) -> &'static str {
        match kind {
            Kind::Function => "function",
            Kind::Param => "param",
            Kind::Return => "return",
            Kind::Instruction => "instruction",
            Kind::Block => "block",
            Kind::Place => "place",
            Kind::Object => "object",
            Kind::Program => "program",
        }
    }

    pub fn kind_named(key: &str) -> Option<Kind> {
        [Kind::Function, Kind::Param, Kind::Return, Kind::Instruction, Kind::Block, Kind::Place, Kind::Object, Kind::Program]
            .into_iter()
            .find(|&kind| Self::kind_key(kind) == key)
    }

    /// The function a subject belongs to, and its own id in it.
    pub fn fields(self) -> (Option<i64>, Option<i64>) {
        match self {
            Subject::Function(function) => (None, Some(function)),
            Subject::Param { function, index } => (Some(function), Some(index)),
            Subject::Return(function) => (Some(function), None),
            Subject::Instruction { function, id } | Subject::Block { function, id } | Subject::Place { function, id } => (Some(function), Some(id)),
            Subject::Object(id) => (None, Some(id)),
            Subject::Program => (None, None),
        }
    }

    /// The function it belongs to; a function is its own.
    pub fn function(self) -> Option<i64> {
        match self {
            Subject::Function(id) => Some(id),
            other => other.fields().0,
        }
    }

    /// The subject of `kind` with these fields; none where one is missing.
    pub fn of(kind: Kind, function: Option<i64>, id: Option<i64>) -> Option<Subject> {
        Some(match kind {
            Kind::Function => Subject::Function(id?),
            Kind::Param => Subject::Param { function: function?, index: id? },
            Kind::Return => Subject::Return(function?),
            Kind::Instruction => Subject::Instruction { function: function?, id: id? },
            Kind::Block => Subject::Block { function: function?, id: id? },
            Kind::Place => Subject::Place { function: function?, id: id? },
            Kind::Object => Subject::Object(id?),
            Kind::Program => Subject::Program,
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
        let all = [
            Subject::Function(3),
            Subject::Param { function: 1, index: 2 },
            Subject::Return(1),
            Subject::Instruction { function: 1, id: 5 },
            Subject::Block { function: 1, id: 4 },
            Subject::Place { function: 1, id: 6 },
            Subject::Object(9),
            Subject::Program,
        ];
        for subject in all {
            let (function, id) = subject.fields();
            assert_eq!(Subject::of(subject.kind(), function, id), Some(subject));
            assert_eq!(Subject::kind_named(Subject::kind_key(subject.kind())), Some(subject.kind()));
        }
    }
}
