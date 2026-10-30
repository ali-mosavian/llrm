//! Typing an expression by BC's rules, and folding the constants it holds.

use super::*;

impl Checker {
    pub(super) fn scalar(
        &mut self,
        typed: Option<Typed>,
        span: Span,
    ) -> Option<Typed> {
        let typed = typed?;
        if matches!(typed.ty, Ty::Record(_)) {
            self.err(span, "Typed variable not allowed in expression");
            return None;
        }
        if typed.array.is_some() {
            self.err_because(
                span,
                "Syntax error",
                Reason::headline("`{text}` is a whole array, but a single value is needed here")
                    .help("write an index, as in `name(1)`; a whole array is only passed as `name()` in a call"),
            );
            return None;
        }
        Some(typed)
    }

    /// An expression that must be numeric, and a 16-bit integer where it is a
    /// constant.
    pub(super) fn integer(
        &mut self,
        expr: &Expr,
    ) -> Option<Typed> {
        self.argument_of_class(Class::Int, expr)
    }

    /// An expression that must be numeric.
    pub(super) fn numeric(
        &mut self,
        expr: &Expr,
    ) -> Option<Typed> {
        let typed = self.expr(expr);
        let typed = self.scalar(typed, expr.span())?;
        if !typed.ty.numeric() {
            let headline = format!("`{{text}}` is {}, but a number is needed here", typed.ty.label());
            self.err_because(
                expr.span(),
                "Type mismatch",
                Reason::headline(headline).help("convert the string with VAL(...), or use a number"),
            );
            return None;
        }
        Some(typed)
    }

    pub(super) fn string_expr(
        &mut self,
        expr: &Expr,
    ) -> Option<Typed> {
        let typed = self.expr(expr);
        let typed = self.scalar(typed, expr.span())?;
        if !typed.ty.string() {
            self.err(expr.span(), "String expression required");
            return None;
        }
        Some(typed)
    }

    pub(super) fn expr(
        &mut self,
        expr: &Expr,
    ) -> Option<Typed> {
        match expr {
            Expr::Omitted(span) => {
                self.err_because(
                    *span,
                    "Syntax error",
                    Reason::headline("a value is missing here: an argument cannot be left empty")
                        .help("give the argument a value, or remove the extra comma"),
                );
                None
            }
            Expr::Literal(literal, _) => Some(match literal {
                Literal::Integer(value, TypeName::Long) => {
                    Typed { constant: Some(*value as f64), ..Typed::value(Ty::Long) }
                }
                Literal::Integer(value, _) => Typed { constant: Some(*value as f64), ..Typed::value(Ty::Int) },
                Literal::Real(text, TypeName::Currency) => {
                    Typed { constant: parse_real(text), ..Typed::value(Ty::Currency) }
                }
                Literal::Real(text, TypeName::Double) => {
                    let constant = parse_real(text);
                    if constant.is_some_and(f64::is_infinite) {
                        self.err(expr.span(), "Overflow in numeric constant");
                    }
                    Typed { constant, ..Typed::value(Ty::Double) }
                }
                Literal::Real(text, _) => {
                    let constant = parse_real(text);
                    if constant.is_some_and(|c| c.abs() > f64::from(f32::MAX)) {
                        self.overflow(expr.span(), &Ty::Single, "number");
                    }
                    Typed { constant, ..Typed::value(Ty::Single) }
                }
                Literal::String(_) => Typed::value(Ty::Str(None)),
            }),
            Expr::Name(name, span) => self.name_expr(name, *span),
            Expr::Apply { name, arguments, span } => self.apply(name, arguments, *span),
            // `Math.Add(1, 2)`: a procedure or array whose name has a dot is
            // not a field of anything
            Expr::Index { base, indices, span }
                if dotted(base).is_some_and(|name| !self.lookup_record_var(name.split('.').next().unwrap_or(""))) =>
            {
                let name = dotted(base).unwrap_or_default();
                self.apply(&name, indices, *span)
            }
            Expr::Index { base, indices, span } => {
                self.field_rank = None;
                let base_typed = self.expr(base)?;
                // an element of an array field of a record
                if let (Expr::Field { .. }, Some(rank)) = (&**base, self.field_rank.take()) {
                    // `r.a()` names the whole array
                    if indices.is_empty() {
                        return Some(base_typed);
                    }
                    let mut ok = true;
                    for index in indices {
                        ok &= self.numeric(index).is_some();
                    }
                    if ok && rank != indices.len() {
                        self.dimensions(*span, rank, indices.len());
                        return None;
                    }
                    return ok.then_some(Typed {
                        variable: true,
                        constant: None,
                        unfolded: None,
                        array: None,
                        ..base_typed
                    });
                }
                // an indexed property of a VBDOS form or control
                if base_typed.ty == Ty::Variant {
                    for index in indices {
                        self.expr(index);
                    }
                    return Some(Typed {
                        ty: Ty::Variant,
                        variable: true,
                        constant: None,
                        unfolded: None,
                        array: None,
                    });
                }
                let (headline, help) = if matches!(**base, Expr::Index { .. }) {
                    (
                        "`{text}` has two index lists in a row",
                        "an element takes one list of subscripts, as in `name(1, 2)`",
                    )
                } else {
                    (
                        "`{text}` indexes something that is not an array",
                        "only an array, or an array field of a record, can be followed by `(...)`; `name.field` selects a field",
                    )
                };
                self.err_because(*span, "Syntax error", Reason::headline(headline).help(help));
                None
            }
            Expr::Field { base, name, span } => self.field(expr, base, name, *span),
            Expr::Unary { op, operand, span } => {
                let typed = self.expr(operand);
                if *op == Unary::Grouped {
                    // a parenthesised reference is a value
                    return typed.map(|t| Typed { variable: false, ..t });
                }
                if matches!(op, Unary::ByValue | Unary::Segmented) {
                    return typed;
                }
                let typed = self.scalar(typed, *span)?;
                if !typed.ty.numeric() {
                    let headline = format!(
                        "`{{text}}` needs a number after the operator, but the operand is {}",
                        typed.ty.describe()
                    );
                    self.err_because(
                        *span,
                        "Type mismatch",
                        Reason::headline(headline).help("convert it with VAL, or use a number"),
                    );
                    return None;
                }
                let ty = match op {
                    Unary::Not => {
                        if typed.ty == Ty::Long {
                            Ty::Long
                        } else if typed.ty.rank() > 2 {
                            Ty::Long.min_int(&typed.ty)
                        } else {
                            Ty::Int
                        }
                    }
                    _ => typed.ty.clone(),
                };
                let value = fold::unary(*op, &typed);
                Some(Typed { ty, variable: false, constant: value.folded, unfolded: value.known, array: None })
            }
            Expr::Binary { op, left, right, span } => {
                let l = self.expr(left);
                let r = self.expr(right);
                let l = self.scalar(l, left.span());
                let r = self.scalar(r, right.span());
                let (l, r) = (l?, r?);
                if *op == Binary::Multiply && reassociated_overflow(left, right, self.dialect) {
                    self.err_because(
                        *span,
                        "Math overflow",
                        Reason::headline(
                            "`{text}` overflows: BC multiplies the constants out of the parentheses while compiling, and their product does not fit a LONG",
                        )
                        .help("make the constants smaller, or use a double (`#`) for one of them"),
                    );
                    return None;
                }
                self.binary(*op, &l, &r, right, *span)
            }
            _ => {
                self.err(expr.span(), "Syntax error");
                None
            }
        }
    }

    pub(super) fn binary(
        &mut self,
        op: Binary,
        l: &Typed,
        r: &Typed,
        right: &Expr,
        span: Span,
    ) -> Option<Typed> {
        use Binary::*;
        // a property of a form or control may be either a number or text
        if l.ty == Ty::Variant || r.ty == Ty::Variant {
            return Some(Typed::value(if matches!(op, Eq | NotEqual | Less | LessEqual | Greater | GreaterEqual) {
                Ty::Int
            } else {
                Ty::Variant
            }));
        }
        let both_strings = l.ty.string() && r.ty.string();
        match op {
            Add if both_strings => return Some(Typed::value(Ty::Str(None))),
            Eq | NotEqual | Less | LessEqual | Greater | GreaterEqual if both_strings => {
                return Some(Typed::value(Ty::Int));
            }
            _ => {}
        }
        if !(l.ty.numeric() && r.ty.numeric()) {
            let headline = format!(
                "`{{text}}` mixes {} with {}: this operator needs two numbers{}",
                l.ty.describe(),
                r.ty.describe(),
                if matches!(op, Add) { " (or two strings, to join them)" } else { "" }
            );
            self.err_because(
                span,
                "Type mismatch",
                Reason::headline(headline)
                    .help("convert one side (VAL turns a string into a number, STR$ a number into a string)"),
            );
            return None;
        }
        let promoted = promote(&l.ty, &r.ty);
        let value = match fold::binary(op, l, r) {
            Ok(value) => value,
            Err(()) => {
                self.err_because(
                    span,
                    "Math overflow",
                    Reason::headline(
                        "`{text}` overflows: BC works out the constant while compiling and the result does not fit",
                    )
                    .help("widen an operand: `&` makes a number a LONG, `#` a double"),
                );
                return None;
            }
        };
        let typed = |ty: Ty| Typed { constant: value.folded, unfolded: value.known, ..Typed::value(ty) };
        let divisor_is_constant =
            matches!(
                right,
                Expr::Literal(..) | Expr::Unary { .. } | Expr::Binary { .. } | Expr::Name(..)
            );
        let wide = l.ty == Ty::Long || r.ty == Ty::Long || l.ty.rank() > 2 || r.ty.rank() > 2;
        Some(match op {
            Add | Subtract | Multiply => typed(promoted),
            Divide => {
                if divisor_is_constant && r.known() == Some(0.0) {
                    self.err(span, "Division by zero");
                    return None;
                }
                typed(if promoted == Ty::Double { Ty::Double } else { Ty::Single })
            }
            IntegerDivide | Modulo => {
                // both operands are rounded to integers first: `5 MOD 0.4`
                // divides by 0
                if divisor_is_constant && r.known().is_some_and(|c| c.round_ties_even() == 0.0) {
                    self.err(span, "Division by zero");
                    return None;
                }
                typed(if wide { Ty::Long } else { Ty::Int })
            }
            Power => typed(Ty::Double),
            Eq | NotEqual | Less | LessEqual | Greater | GreaterEqual => Typed::value(Ty::Int),
            And | Or | Xor | Eqv | Imp => typed(if wide { Ty::Long } else { Ty::Int }),
        })
    }

    pub(super) fn field(
        &mut self,
        whole: &Expr,
        base: &Expr,
        name: &str,
        span: Span,
    ) -> Option<Typed> {
        // a dotted name that no record variable owns is one variable's name
        if let Some(chain) = dotted(whole) {
            let head = dotted(base).unwrap_or_default();
            let head_is_record = self.lookup_record_var(head.split('.').next().unwrap_or(""));
            if !head_is_record {
                let var = self.variable(&chain, false, span)?;
                return Some(Typed { ty: var.ty, variable: true, constant: None, unfolded: None, array: None });
            }
        }
        self.field_rank = None;
        let base_typed = self.expr(base)?;
        self.field_rank = None;
        // a property of a property of a form: nothing here says its type
        if base_typed.ty == Ty::Variant {
            return Some(Typed { ty: Ty::Variant, variable: true, constant: None, unfolded: None, array: None });
        }
        let Ty::Record(record) = &base_typed.ty else {
            let headline = format!("`{{text}}` has a `.{name}` after {}, which is not a record", base_typed.ty.label());
            self.err_because(
                span,
                "Type mismatch",
                Reason::headline(headline)
                    .help("a dot selects a field, and only a variable declared AS a TYPE has fields"),
            );
            return None;
        };
        let record_name = record.clone();
        let record = self.records.get(record).cloned().unwrap_or(Record { fields: Vec::new(), opaque: true, bytes: 0 });
        if record.opaque {
            return Some(Typed { ty: Ty::Variant, variable: true, constant: None, unfolded: None, array: None });
        }
        // a field may be named with its type character (`menu.text$`)
        let wanted = Ty::of_suffix(name);
        match record.fields.iter().find(|(field, ty, _)| {
            stem(field).eq_ignore_ascii_case(stem(name)) && wanted.as_ref().is_none_or(|w| ty.plain() == *w)
        }) {
            Some((_, ty, rank)) => {
                self.field_rank = *rank;
                Some(Typed { ty: ty.clone(), variable: true, constant: None, unfolded: None, array: *rank })
            }
            None => {
                self.err_because(
                    span,
                    "Element not defined",
                    Reason::headline(format!("`{{text}}` is not a field of the record type `{record_name}`"))
                        .help("check the field names in its TYPE statement"),
                );
                None
            }
        }
    }

    pub(super) fn lookup_record_var(
        &self,
        name: &str,
    ) -> bool {
        let key = stem(name);
        let in_scope =
            |scope: &Scope| scope.vars.get(key).is_some_and(|vars| vars.iter().any(|v| matches!(v.ty, Ty::Record(_))));
        in_scope(&self.scope) || in_scope(&self.module_scope)
    }

    pub(super) fn name_expr(
        &mut self,
        name: &str,
        span: Span,
    ) -> Option<Typed> {
        let key = stem(name);
        if let Some(constant) = self.consts.get(key) {
            // `CONST x = 1` then `x$`: the suffix names a string where the
            // constant is a number
            if Ty::of_suffix(name).is_some_and(|suffix| suffix.string() != constant.ty.string()) {
                let headline = format!(
                    "`{{text}}` has a type character that does not match the CONST of that name, which is {}",
                    constant.ty.label()
                );
                self.err_because(
                    span,
                    "Type mismatch",
                    Reason::headline(headline).help("write the name without the type character"),
                );
                return None;
            }
            return Some(constant.clone());
        }
        if let Some(sig) = sigs::function(name, self.dialect) {
            if sig.parens {
                self.err(span, "Left parenthesis missing");
                return None;
            }
            if sig.accepts(0) {
                return Some(Typed::value(self.result_type(sig.ret, &[])));
            }
        }
        if let Some(procedure) = self.procs.get(key).cloned() {
            if procedure.kind == ProcedureKind::Function {
                if procedure.params.is_empty() {
                    return procedure.result.map(Typed::value);
                }
                let n = procedure.params.len();
                let headline = format!(
                    "`{{name}}` is called with no arguments, but takes {n} argument{}",
                    if n == 1 { "" } else { "s" }
                );
                self.err_because(
                    span,
                    "Argument-count mismatch",
                    Reason::headline(headline).about(name).help("write the arguments in parentheses after the name"),
                );
                return None;
            }
        }
        // a FUNCTION's own name reads its result
        if self.procedure.as_ref().is_some_and(|(kind, own)| *kind == ProcedureKind::Function && stem(own) == key) {
            let ty = self.procs.get(key).and_then(|p| p.result.clone()).unwrap_or(Ty::Single);
            return Some(Typed { ty, variable: true, constant: None, unfolded: None, array: None });
        }
        let var = self.variable(name, false, span)?;
        Some(Typed { ty: var.ty, variable: true, constant: None, unfolded: None, array: None })
    }

    pub(super) fn result_type(
        &self,
        ret: Ret,
        args: &[Typed],
    ) -> Ty {
        match ret {
            Ret::Int => Ty::Int,
            Ret::Long => Ty::Long,
            Ret::Single => Ty::Single,
            Ret::Double => Ty::Double,
            Ret::Currency => Ty::Currency,
            Ret::Str => Ty::Str(None),
            Ret::Same => args.first().map_or(Ty::Single, |a| if a.ty.rank() == 0 { Ty::Single } else { a.ty.clone() }),
            Ret::Float => match args.first() {
                Some(a) if a.ty == Ty::Double => Ty::Double,
                _ => Ty::Single,
            },
        }
    }

    pub(super) fn apply(
        &mut self,
        name: &str,
        arguments: &[Expr],
        span: Span,
    ) -> Option<Typed> {
        let key = stem(name);
        if name == crate::front::ast::ON_SELECTOR {
            for argument in arguments {
                self.integer(argument);
            }
            return Some(Typed::value(Ty::Int));
        }
        // an array element
        let array = self.array_var(name);
        if let Some(var) = array {
            // each subscript that is not a constant holds a temporary: past six
            // when read, seven when assigned, BC gives up
            let variable_subscripts = arguments.iter().filter(|index| !matches!(index, Expr::Literal(..))).count();
            // QB 4.5 addresses a dynamic array's element with every subscript,
            // a constant too, in a temporary: past seven when read,
            // eight when assigned
            let dynamic_limit = var.dynamic
                && self.dialect == Dialect::QuickBasic45
                && arguments.len() > if self.assigning { 8 } else { 7 };
            if dynamic_limit || variable_subscripts > if self.assigning { 7 } else { 6 } {
                self.err(span, "Expression too complex");
                return None;
            }
            let rank = var.rank.unwrap_or(1);
            let mut ok = true;
            for index in arguments {
                ok &= self.numeric(index).is_some();
            }
            if ok && var.rank.is_some() && rank != arguments.len() && rank != usize::MAX {
                self.dimensions(span, rank, arguments.len());
                return None;
            }
            return ok.then_some(Typed { ty: var.ty, variable: true, constant: None, unfolded: None, array: None });
        }
        if let Some(sig) = sigs::function(name, self.dialect) {
            return self.intrinsic(sig, name, arguments, span);
        }
        // `SCREEN.ControlPanel(i)`, `frm.List(i)`: an indexed property of a
        // VBDOS object
        if let Some((root, _)) = name.split_once('.') {
            if matches!(self.dialect, Dialect::VbDos | Dialect::Quickr)
                && (matches!(
                    root.to_ascii_uppercase().as_str(),
                    "SCREEN" | "PRINTER" | "CLIPBOARD"
                )
                    || self.lookup_record_var(root))
            {
                for argument in arguments {
                    self.expr(argument);
                }
                return Some(Typed { ty: Ty::Variant, variable: true, constant: None, unfolded: None, array: None });
            }
        }
        if let Some(procedure) = self.procs.get(key).or_else(|| self.procs.get(name)).cloned() {
            if procedure.kind == ProcedureKind::Function && span.line < procedure.first_line {
                // above its DECLARE or definition the name is an array that was
                // never dimensioned
                self.err(span, if procedure.def_fn { "Function not defined" } else { "Array not dimensioned" });
                return None;
            }
            if procedure.kind == ProcedureKind::Function {
                self.check_arguments(name, &procedure, arguments, span, false);
                return procedure.result.map(Typed::value);
            }
            self.err_because(
                span,
                "Type mismatch",
                Reason::headline("`{name}` is a SUB and returns no value, so it cannot be used in an expression")
                    .help("use a FUNCTION, or call the SUB as a statement"),
            );
            return None;
        }
        if sigs::function_unknown_but_named(name) {
            self.err(span, "Syntax error");
            return None;
        }
        if name.starts_with("FN") && self.procs.get(name).is_none() {
            self.err(span, "Function not defined");
            return None;
        }
        self.err(span, "Array not dimensioned");
        None
    }

    pub(super) fn array_var(
        &self,
        name: &str,
    ) -> Option<Var> {
        let key = stem(name);
        let suffix = Ty::of_suffix(name);
        let find = |scope: &Scope| {
            scope
                .vars
                .get(key)
                .and_then(
                    |vars| vars.iter()
                        .find(|v| {
                            v.rank.is_some()
                                && match &suffix {
                                    Some(s) => v.ty.plain() == *s,
                                    None => v.declared || v.ty.plain() == self.default_type(name),
                                }
                        })
                        .cloned(),
                )
        };
        find(&self.scope).or_else(|| find(&self.module_scope))
    }
}

/// The value of a number literal, a sign of one, or those in parentheses.
fn literal_number(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Literal(Literal::Integer(value, _), _) => Some(*value as f64),
        Expr::Literal(Literal::Real(text, _), _) => text.replace('D', "E").parse().ok(),
        Expr::Unary { op: Unary::Negative, operand, .. } => literal_number(operand).map(|value| -value),
        Expr::Unary { op: Unary::Positive | Unary::Grouped, operand, .. } => literal_number(operand),
        _ => None,
    }
}

/// `(x +- c1) * c2` and `c2 * (c1 +- x)` with `x` no constant: BC distributes
/// the multiplication and folds `c1 * c2`, so a product that does not fit a
/// LONG is "Math overflow" although `x` is a variable. Two INTEGER constants
/// are combined at run time (`(i - 300) * 200`), so one must be wider than an
/// INTEGER. PDS 7.1 and VBDOS do it for `+` only (either order); QB 4.5 also
/// for `x - c1`.
fn reassociated_overflow(
    left: &Expr,
    right: &Expr,
    dialect: Dialect,
) -> bool {
    fn inner(expr: &Expr) -> Option<&Expr> {
        match expr {
            Expr::Unary { op: Unary::Grouped, operand, .. } => inner(operand),
            other => Some(other),
        }
    }
    let sum_times = |sum: &Expr, factor: &Expr| -> bool {
        let Some(Expr::Binary { op: op @ (Binary::Add | Binary::Subtract), left, right, .. }) = inner(sum) else {
            return false;
        };
        let Some(c2) = literal_number(factor) else { return false };
        let c1 = match (literal_number(left), literal_number(right), *op) {
            (None, Some(c), Binary::Add) | (Some(c), None, Binary::Add) => c,
            (None, Some(c), Binary::Subtract) if dialect == Dialect::QuickBasic45 => -c,
            _ => return false,
        };
        let wide = |c: f64| c.abs() > 32767.0 || c.fract() != 0.0;
        (wide(c1) || wide(c2)) && !(-2147483648.0..=2147483647.0).contains(&(c1 * c2))
    };
    sum_times(left, right) || sum_times(right, left)
}
