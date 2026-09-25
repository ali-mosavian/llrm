//! A module's declarations, by the name each has in it: `Point`, `Point.x`,
//! `Mode.text`, `Point.move`.

use crate::frontends::modern::syntax::{Module, Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Function,
    Method,
    Struct,
    Field,
    Enum,
    Variant,
    Const,
    Var,
    Type,
    Protocol,
}

impl Kind {
    /// Its LSP `SymbolKind`.
    pub fn symbol(self) -> u8 {
        match self {
            Self::Function => 12,
            Self::Method => 6,
            Self::Struct => 23,
            Self::Field => 8,
            Self::Enum => 10,
            Self::Variant => 22,
            Self::Const => 14,
            Self::Var => 13,
            Self::Type => 26,
            Self::Protocol => 11,
        }
    }

    /// Its LSP `CompletionItemKind`.
    pub fn completion(self) -> u8 {
        match self {
            Self::Function => 3,
            Self::Method => 2,
            Self::Struct => 22,
            Self::Field => 5,
            Self::Enum => 13,
            Self::Variant => 20,
            Self::Const => 21,
            Self::Var => 6,
            Self::Type => 25,
            Self::Protocol => 8,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Declaration {
    pub name: String,
    pub kind: Kind,
    pub span: Span,
    pub public: bool,
    /// A struct's fields, an enum's variants, a protocol's methods.
    pub members: Vec<Declaration>,
}

impl Declaration {
    /// The last part of its name, as its declaration spells it.
    pub fn short(&self) -> &str {
        self.name.rsplit('.').next().unwrap_or(&self.name)
    }
}

/// Every top-level declaration of `module`, in source order.
pub fn declarations(module: &Module) -> Vec<Declaration> {
    let declared = |name: &str, kind, span, members| Declaration {
        name: name.to_owned(),
        kind,
        span,
        public: module.public.contains(name),
        members,
    };
    let member = |owner: &str, name: &str, kind, span| declared(&format!("{owner}.{name}"), kind, span, Vec::new());
    let mut all: Vec<Declaration> = module.fixed_types.iter().map(|one| declared(&one.name, Kind::Type, one.span, Vec::new())).collect();
    all.extend(module.consts.iter().map(|one| declared(&one.name, Kind::Const, one.span, Vec::new())));
    all.extend(module.statics.iter().map(|one| declared(&one.name, Kind::Var, one.span, Vec::new())));
    all.extend(module.structs.iter().map(|one| {
        let fields = one.fields.iter().map(|field| member(&one.name, &field.name, Kind::Field, field.span)).collect();
        declared(&one.name, Kind::Struct, one.span, fields)
    }));
    all.extend(module.enums.iter().map(|one| {
        let variants = one.variants.iter().map(|variant| member(&one.name, &variant.name, Kind::Variant, variant.span)).collect();
        declared(&one.name, Kind::Enum, one.span, variants)
    }));
    all.extend(module.protocols.iter().map(|one| {
        let methods = one.methods.iter().map(|method| member(&one.name, &method.name, Kind::Method, method.span)).collect();
        declared(&one.name, Kind::Protocol, one.span, methods)
    }));
    let functions = module.functions.iter().chain(module.externs.iter().map(|one| &one.function));
    all.extend(functions.map(|one| {
        let kind = if one.name.contains('.') { Kind::Method } else { Kind::Function };
        declared(&one.name, kind, one.span, Vec::new())
    }));
    all.sort_by_key(|one| (one.span.line, one.span.column));
    all
}

/// The declaration `module` has of `name`, top-level or a member.
pub fn find(module: &Module, name: &str) -> Option<Declaration> {
    declarations(module)
        .into_iter()
        .flat_map(|one| std::iter::once(one.clone()).chain(one.members))
        .find(|one| one.name == name)
}
