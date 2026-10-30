//! TYPE: the fields of a record and the bytes they take.

use super::*;

impl Checker {
    pub(super) fn type_declaration(
        &mut self,
        name: &str,
        fields: &[Declaration],
        span: Span,
    ) {
        if self.procedure.is_some() {
            self.err(span, "Illegal inside SUB or FUNCTION");
        }
        if crate::front::reserved::is_reserved_type(self.dialect, name) {
            self.err(span, "Identifier expected");
            return;
        }
        if Ty::of_suffix(name).is_some() {
            self.err_because(
                span,
                "Invalid character",
                Reason::headline("the TYPE name `{name}` carries a type character")
                    .help("write the name without `%`, `&`, `!`, `#`, `$` or `@`"),
            );
            return;
        }
        if self.records.contains_key(name) {
            self.err_because(
                span,
                "Duplicate definition",
                Reason::headline("the TYPE `{name}` is defined twice")
                    .help("give the second one another name, or remove it"),
            );
            return;
        }
        if fields.is_empty() {
            self.err_because(
                span,
                "Element not defined",
                Reason::headline("the TYPE `{name}` has no fields")
                    .help("add at least one `field AS type` line before END TYPE"),
            );
        }
        let mut out: Vec<(String, Ty, Option<usize>)> = Vec::new();
        let mut bytes = 0usize;
        for field in fields {
            // array fields are PDS's and VBDOS's, and need bounds
            if field.array
                && (field.bounds.is_empty() || matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45))
            {
                self.err(
                    field.span,
                    if matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45) {
                        "AS missing"
                    } else {
                        "Syntax error"
                    },
                );
                continue;
            }
            if matches!(field.type_name, Some(TypeName::String)) {
                match &field.fixed_length {
                    None => {
                        self.err(field.span, "Asterisk missing");
                        continue;
                    }
                    Some(length) => {
                        let ok = self
                            .constant_expr(length)
                            .and_then(|typed| typed.known())
                            .is_some_and(|c| (1.0..=32767.0).contains(&c));
                        if !ok {
                            self.err(field.span, "Integer between 1 and 32767 required");
                            continue;
                        }
                    }
                }
            }
            if Ty::of_suffix(&field.name).is_some() {
                self.err(field.span, "Invalid character");
                continue;
            }
            if crate::front::reserved::is_reserved_field(self.dialect, &field.name) {
                self.err(field.span, "Identifier expected");
                continue;
            }
            if out.iter().any(|(existing, ..)| *existing == field.name) {
                self.err_because(
                    field.span,
                    "Duplicate definition",
                    Reason::headline("the field `{name}` appears twice in this TYPE").help("rename one of the fields"),
                );
                continue;
            }
            let mut ty = match field.type_name.as_ref().and_then(|t| self.type_of_name(t)) {
                Some(ty) => ty,
                None => {
                    self.err(field.span, "User-defined type not defined");
                    continue;
                }
            };
            if let (Ty::Str(_), Some(length)) = (&ty, &field.fixed_length) {
                if let Some(typed) = self.constant_expr(length) {
                    ty = Ty::Str(typed.known().map(|c| c as usize));
                }
            }
            let mut size = self.size_of(&ty);
            if field.array {
                let Some(elements) = self.field_elements(&field.bounds) else { continue };
                size = size.saturating_mul(elements);
                if size > 65535 {
                    self.err(field.span, "Array too big");
                    continue;
                }
            }
            bytes += size;
            out.push((field.name.clone(), ty, field.array.then(|| field.bounds.len().max(1))));
        }
        if bytes > 65535 {
            self.err(span, "TYPE more than 65535 bytes");
        }
        self.records.insert(name.to_owned(), Record { fields: out, opaque: false, bytes });
    }

    /// The bytes one value of `ty` takes in a record.
    pub(super) fn size_of(
        &self,
        ty: &Ty,
    ) -> usize {
        match ty {
            Ty::Int => 2,
            Ty::Long | Ty::Single => 4,
            Ty::Double | Ty::Currency => 8,
            Ty::Str(length) => length.unwrap_or(0),
            Ty::Record(name) => self.records.get(name).map_or(0, |record| record.bytes),
            Ty::Variant => 0,
        }
    }

    /// The element count of an array field's bounds, which must be constants BC
    /// can hold in 16 bits.
    pub(super) fn field_elements(
        &mut self,
        bounds: &[crate::syntax::Bound],
    ) -> Option<usize> {
        let mut elements = 1usize;
        for bound in bounds {
            let upper = self.field_bound(&bound.upper)?;
            let lower = match &bound.lower {
                Some(lower) => self.field_bound(lower)?,
                None => 0,
            };
            if lower > upper {
                self.err(bound.upper.span(), "Lower bound exceeds upper bound");
                return None;
            }
            elements = elements.saturating_mul((upper - lower + 1) as usize);
        }
        Some(elements)
    }

    pub(super) fn field_bound(
        &mut self,
        bound: &Expr,
    ) -> Option<i64> {
        let mut names = Vec::new();
        free_names(bound, &mut names);
        if names.iter().any(|name| !self.consts.contains_key(stem(name))) {
            self.err(bound.span(), "Dynamic array illegal");
            return None;
        }
        let typed = self.constant_expr(bound)?;
        if typed.ty.string() {
            self.err_because(
                bound.span(),
                "Type mismatch",
                Reason::headline("an array bound must be a number, but `{text}` is a string")
                    .help("use a number or a CONST"),
            );
            return None;
        }
        let value = typed.known()? as i64;
        if !(-32768..=32767).contains(&value) {
            self.overflow(bound.span(), &Ty::Int, "bound");
            return None;
        }
        Some(value)
    }
}
