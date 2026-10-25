//! The check's values: types, variables, scopes, records and what an expression
//! is.

use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagnostic {
    pub span: Span,
    pub message: String,
    /// The name the error is about, when it is one word of a longer span (a
    /// label in a GOTO).
    pub subject: Option<String>,
    /// A sentence in place of the usual one for this error, where the checker
    /// knows more (what the two sides were, which call).
    pub headline: Option<String>,
    /// What to do about it, in words, ahead of the usual advice.
    pub detail: Option<String>,
    /// Another place in the source the message talks about (`{other}`).
    pub related: Option<Span>,
}

/// What the checker knows about an error beyond its BC name.
#[derive(Default)]
pub struct Reason {
    pub headline: Option<String>,
    pub help: Option<String>,
    pub subject: Option<String>,
    pub related: Option<Span>,
}

impl Reason {
    pub fn headline(text: impl Into<String>) -> Self {
        Self { headline: Some(text.into()), ..Self::default() }
    }

    pub fn help(
        mut self,
        text: impl Into<String>,
    ) -> Self {
        self.help = Some(text.into());
        self
    }

    pub fn about(
        mut self,
        word: &str,
    ) -> Self {
        self.subject = Some(word.to_owned());
        self
    }

    pub fn related(
        mut self,
        span: Span,
    ) -> Self {
        self.related = Some(span);
        self
    }
}

/// A BASIC type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Int,
    Long,
    Single,
    Double,
    /// PDS and VBDOS's CURRENCY.
    Currency,
    /// A property of a VBDOS form or control, whose type the form's file would
    /// say.
    Variant,
    /// A string; a fixed one has its length.
    Str(Option<usize>),
    Record(String),
}

impl Ty {
    pub(super) fn numeric(&self) -> bool {
        matches!(
            self,
            Ty::Int | Ty::Long | Ty::Currency | Ty::Single | Ty::Double | Ty::Variant
        )
    }

    /// The type by name, for a message: "an INTEGER", "a string".
    pub(super) fn label(&self) -> String {
        match self {
            Ty::Int => "an INTEGER".into(),
            Ty::Long => "a LONG".into(),
            Ty::Single => "a SINGLE".into(),
            Ty::Double => "a DOUBLE".into(),
            Ty::Currency => "a CURRENCY".into(),
            Ty::Variant => "a form property".into(),
            Ty::Str(Some(n)) => format!("a string of {n} characters"),
            Ty::Str(None) => "a string".into(),
            Ty::Record(name) => format!("a record of type {name}"),
        }
    }

    /// What a value of this type is, for a message: "a number", "a string".
    pub(super) fn describe(&self) -> &'static str {
        match self {
            Ty::Str(_) => "a string",
            Ty::Record(_) => "a record",
            Ty::Variant => "a form property",
            _ => "a number",
        }
    }

    pub(super) fn string(&self) -> bool {
        matches!(self, Ty::Str(_) | Ty::Variant)
    }

    pub(super) fn rank(&self) -> u8 {
        match self {
            Ty::Int => 1,
            Ty::Long => 2,
            Ty::Currency => 3,
            Ty::Single => 4,
            Ty::Double => 5,
            _ => 0,
        }
    }

    /// The same type, with a fixed string made a plain one.
    pub(super) fn plain(&self) -> Ty {
        match self {
            Ty::Str(_) => Ty::Str(None),
            other => other.clone(),
        }
    }

    pub(super) fn of_suffix(name: &str) -> Option<Ty> {
        match name.as_bytes().last()? {
            b'%' => Some(Ty::Int),
            b'&' => Some(Ty::Long),
            b'!' => Some(Ty::Single),
            b'#' => Some(Ty::Double),
            b'@' => Some(Ty::Currency),
            b'$' => Some(Ty::Str(None)),
            _ => None,
        }
    }
}

pub(super) fn promote(
    a: &Ty,
    b: &Ty,
) -> Ty {
    if a.rank() >= b.rank() { a.clone() } else { b.clone() }
}

#[derive(Clone, Debug)]
pub(super) struct Var {
    pub(super) ty: Ty,
    /// An array's rank; scalars have none.
    pub(super) rank: Option<usize>,
    /// Declared with AS: its stem is bound to this type.
    pub(super) declared: bool,
    /// DIM SHARED: procedures see it.
    pub(super) shared: bool,
    /// Dimensioned as a dynamic array (REDIM, or under $DYNAMIC): REDIM may
    /// resize it.
    pub(super) dynamic: bool,
    /// Named by a declaration (DIM, a parameter, SHARED), not made by a first
    /// use.
    pub(super) explicit: bool,
    /// The name carried a type character.
    pub(super) suffixed: bool,
    /// The array was made by a REDIM, so a later DIM is not a second
    /// dimensioning.
    pub(super) redimmed: bool,
}

#[derive(Clone, Debug)]
pub(super) struct ParamInfo {
    pub(super) ty: Ty,
    pub(super) array: bool,
    /// A parameter written AS ANY.
    pub(super) any: bool,
    pub(super) by_value: bool,
}

#[derive(Clone, Debug)]
pub(super) struct ProcInfo {
    pub(super) kind: ProcedureKind,
    pub(super) params: Vec<ParamInfo>,
    pub(super) result: Option<Ty>,
    pub(super) defined: bool,
    pub(super) declared: bool,
    pub(super) def_fn: bool,
    /// `DECLARE SUB name` with no parameter list: its calls are not checked.
    pub(super) unchecked: bool,
    /// The line where the name first appears (a DECLARE or the definition):
    /// calls before it are unchecked.
    pub(super) first_line: usize,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Scope {
    /// by the name's stem (suffix removed), upper case
    pub(super) vars: HashMap<String, Vec<Var>>,
}

#[derive(Clone, Debug)]
pub(super) struct Record {
    /// name, type, and the number of dimensions of an array field
    pub(super) fields: Vec<(String, Ty, Option<usize>)>,
    /// the record's size in bytes
    pub(super) bytes: usize,
    /// VBDOS's FORM and CONTROL: any property name goes.
    pub(super) opaque: bool,
}

/// What an expression is: its type, and whether it names a variable that can be
/// assigned or passed by reference.
#[derive(Clone, Debug)]
pub(super) struct Typed {
    pub(super) ty: Ty,
    pub(super) variable: bool,
    /// The value BC folds at compile time: a literal, a sign or NOT of one, a
    /// conversion (CINT ...) of one, and arithmetic on those where an operand
    /// is wider than INTEGER. A folded value outside its target's range is
    /// an error where it is converted; the same arithmetic on two INTEGERs is
    /// left to run time.
    pub(super) constant: Option<f64>,
    /// The value of a constant expression BC does not fold (two INTEGERs added,
    /// a power): known, so it can size an array, but converted without a check.
    pub(super) unfolded: Option<f64>,
    /// An array named without subscripts (`a()`), for whole-array arguments.
    pub(super) array: Option<usize>,
}

impl Typed {
    /// The value of the expression when it is a constant, folded or not.
    pub(super) fn known(&self) -> Option<f64> {
        self.constant.or(self.unfolded)
    }

    pub(super) fn value(ty: Ty) -> Self {
        Self { ty, variable: false, constant: None, unfolded: None, array: None }
    }
}

impl Ty {
    pub(super) fn min_int(
        &self,
        _other: &Ty,
    ) -> Ty {
        Ty::Long
    }
}
