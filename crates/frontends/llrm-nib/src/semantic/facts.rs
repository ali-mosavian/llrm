//! What the checker learns of the names a program spells, by where each
//! stands: for a tool reading the source, as a language server does.

use super::*;

/// A name spelled at `span`, and what the checker knows of it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fact {
    pub span: Span,
    pub name: String,
    pub known: Known,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Known {
    /// A local, and its type.
    Local(String),
    /// A field or method of a value, and the type declaring it.
    Member(String),
}

impl FunctionCompiler<'_> {
    /// Keeps what the name at `span` is, when the caller asked for facts.
    pub(super) fn learn(&self, span: Span, name: &str, known: impl FnOnce() -> Known) {
        if let Some(facts) = self.facts {
            facts.borrow_mut().push(Fact { span, name: name.to_owned(), known: known() });
        }
    }

    /// The member `name`, at the end of `span`, of the type `owner`.
    pub(super) fn learn_member(&self, span: Span, name: &str, owner: &str) {
        let at = Span { column: span.end_column.saturating_sub(name.len()), ..span };
        self.learn(at, name, || Known::Member(self.types.template_of(owner).to_owned()));
    }

    /// The method `name` called on `receiver`, which is of the type `owner`.
    pub(super) fn learn_method(&self, receiver: &Expr, name: &str, owner: &str) {
        let before = receiver.span();
        let at = Span { column: before.end_column + 1, end_column: before.end_column + 1 + name.len(), ..before };
        self.learn(at, name, || Known::Member(self.types.template_of(owner).to_owned()));
    }

    /// `type_` as the source spells it.
    pub(super) fn spelled(&self, type_: BindingType) -> String {
        let named = |id: u32| self.types.types.iter().find(|one| one.id == id).map_or_else(String::new, |one| one.name.clone());
        match type_ {
            BindingType::Scalar(type_name) => named(type_id(type_name)),
            BindingType::Struct(id) => named(id),
            BindingType::Slice { element, rank: 1 } => format!("&[{}]", named(element.id())),
            BindingType::Slice { element, rank } => format!("&[{}, {rank}]", named(element.id())),
            BindingType::Array { element, shape } => {
                let dims: Vec<String> = shape.dims().iter().map(u32::to_string).collect();
                format!("{}[{}]", named(element.id()), dims.join(", "))
            }
        }
    }
}
