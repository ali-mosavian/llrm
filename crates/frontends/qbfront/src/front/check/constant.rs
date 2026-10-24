//! CONST: which expressions BC reads as constants.

use super::*;

impl Checker {
    pub(super) fn constant_expr(
        &mut self,
        expr: &Expr,
    ) -> Option<Typed> {
        match expr {
            Expr::Literal(..) => self.expr(expr),
            Expr::Name(name, span) => match self.consts.get(stem(name)) {
                Some(constant) => Some(constant.clone()),
                None => {
                    self.err(*span, "Invalid constant");
                    None
                }
            },
            // `UGL.8BIT`: a constant whose name has a dot is read as a field
            // access
            Expr::Field { .. } if dotted(expr).is_some_and(|name| self.consts.contains_key(stem(&name))) => {
                dotted(expr).and_then(|name| self.consts.get(stem(&name)).cloned())
            }
            Expr::Unary { op, operand, .. } if !matches!(op, Unary::ByValue | Unary::Segmented) => {
                self.constant_expr(operand)?;
                self.expr(expr)
            }
            // CONST takes arithmetic and the logical operators, but not `^`, a
            // comparison or `+` on strings
            Expr::Binary { op, left, right, span } => {
                let l = self.constant_expr(left)?;
                let r = self.constant_expr(right)?;
                let comparison = matches!(
                    op,
                    Binary::Power
                        | Binary::Eq
                        | Binary::NotEqual
                        | Binary::Less
                        | Binary::LessEqual
                        | Binary::Greater
                        | Binary::GreaterEqual
                );
                if comparison || (*op == Binary::Add && l.ty.string() && r.ty.string()) {
                    self.err(*span, "Syntax error");
                    return None;
                }
                self.expr(expr)
            }
            // and the conversions CINT, CLNG, CSNG (not in VBDOS) and CDBL, and
            // the size LEN of a number
            Expr::Apply { name, arguments, span } if arguments.len() == 1 => {
                let function = name.to_ascii_uppercase();
                let allowed = match function.as_str() {
                    "CINT" | "CLNG" | "CDBL" => true,
                    "CSNG" => self.dialect != Dialect::VbDos,
                    "LEN" => true,
                    _ => false,
                };
                if !allowed {
                    self.err(*span, "Syntax error");
                    return None;
                }
                let argument = self.constant_expr(&arguments[0])?;
                if function == "LEN" {
                    if argument.ty.string() {
                        self.err(*span, "Syntax error");
                        return None;
                    }
                    let size = self.size_of(&argument.ty);
                    return Some(Typed { constant: Some(size as f64), ..Typed::value(Ty::Int) });
                }
                self.expr(expr)
            }
            other => {
                self.err(other.span(), "Invalid constant");
                None
            }
        }
    }

    pub(super) fn constant(
        &mut self,
        name: &str,
        value: &Expr,
        span: Span,
    ) {
        if self.reserved_name(name, span) {
            return;
        }
        let Some(typed) = self.constant_expr(value) else { return };
        let key = stem(name).to_owned();
        // arrays have a namespace of their own: a CONST clashes with scalars
        // (and with another CONST)
        let scalar_in_use = self.scope.vars.get(&key).is_some_and(|vars| vars.iter().any(|v| v.rank.is_none()))
            || (self.procedure.is_some()
                && self
                    .module_scope
                    .vars
                    .get(&key)
                    .is_some_and(|vars| vars.iter().any(|v| v.rank.is_none() && v.shared)));
        if self.consts.contains_key(&key) {
            self.err(span, "Duplicate definition");
            return;
        }
        if scalar_in_use {
            self.err(span, "Syntax error");
            return;
        }
        let ty = match Ty::of_suffix(name) {
            Some(declared) => {
                if declared.string() != typed.ty.string() {
                    self.err(span, "Type mismatch");
                    return;
                }
                declared
            }
            None => typed.ty.clone(),
        };
        self.consts.insert(
            key,
            Typed { ty, variable: false, constant: typed.constant, unfolded: typed.unfolded, array: None },
        );
    }
}
