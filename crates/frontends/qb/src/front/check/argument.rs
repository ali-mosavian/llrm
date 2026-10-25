//! What an argument must be: the intrinsics' classes, a procedure's parameters
//! and whole arrays.

use super::*;

impl Checker {
    pub(super) fn intrinsic(
        &mut self,
        sig: &'static sigs::FnSig,
        name: &str,
        arguments: &[Expr],
        span: Span,
    ) -> Option<Typed> {
        let candidates: Vec<&Vec<(Class, bool)>> = sig
            .alternatives
            .iter()
            .filter(|alt| {
                alt.len() >= arguments.len() && alt.iter().take_while(|(_, o)| !*o).count() <= arguments.len()
            })
            .collect();
        if candidates.is_empty() {
            let given = arguments.len();
            let headline = format!(
                "`{{name}}` is called with {given} argument{}, which this function does not take",
                if given == 1 { "" } else { "s" }
            );
            self.err_because(
                span,
                "Argument-count mismatch",
                Reason::headline(headline).about(name).help("check the argument list of this function in the manual"),
            );
            return None;
        }
        for (n, alternative) in candidates.iter().enumerate() {
            let mark = self.errors.len();
            let mut typed_args = Vec::new();
            let mut ok = true;
            for (index, argument) in arguments.iter().enumerate() {
                match self.argument_of_class(alternative[index].0, argument) {
                    Some(typed) => typed_args.push(typed),
                    None => ok = false,
                }
            }
            if ok {
                let result = Typed::value(self.result_type(sig.ret, &typed_args));
                // CINT, CLNG, CSNG and CDBL of a folded constant are folded
                // (VBDOS's BC does not fold CSNG: `CALL pp(CSNG(32768))` to an
                // INTEGER parameter compiles and wraps)
                let function = name.to_ascii_uppercase();
                let folds = !(function == "CSNG" && self.dialect == Dialect::VbDos);
                let value =
                    typed_args.first().filter(|_| folds).and_then(|argument| fold::conversion(&function, argument));
                return Some(Typed { constant: value, ..result });
            }
            if n + 1 < candidates.len() {
                self.errors.truncate(mark);
            }
        }
        None
    }

    /// One argument against its class: the typed argument, or an error
    /// reported.
    pub(super) fn argument_of_class(
        &mut self,
        class: Class,
        argument: &Expr,
    ) -> Option<Typed> {
        match class {
            Class::Num => self.numeric(argument),
            Class::Int => {
                let typed = self.numeric(argument)?;
                if let (Some(c), false) = (typed.constant, typed.variable) {
                    if !(-32768.0..=32767.0).contains(&c.round()) {
                        self.overflow(argument.span(), &Ty::Int, "argument");
                        return None;
                    }
                }
                Some(typed)
            }
            Class::Long => {
                let typed = self.numeric(argument)?;
                if let (Some(c), false) = (typed.constant, typed.variable) {
                    if !(-2147483648.0..=2147483647.0).contains(&c.round()) {
                        self.overflow(argument.span(), &Ty::Long, "argument");
                        return None;
                    }
                }
                Some(typed)
            }
            Class::Single => {
                let typed = self.numeric(argument)?;
                if let (Some(c), false) = (typed.constant, typed.variable) {
                    if c.abs() > f64::from(f32::MAX) {
                        self.overflow(argument.span(), &Ty::Single, "argument");
                        return None;
                    }
                }
                Some(typed)
            }
            Class::Str => self.string_expr(argument),
            Class::Any => {
                let typed = self.expr(argument);
                self.scalar(typed, argument.span())
            }
            Class::Var => {
                let typed = self.expr(argument)?;
                if self.whole_array_field(&typed, argument) {
                    return None;
                }
                if !typed.variable {
                    self.err(argument.span(), "Variable required");
                    return None;
                }
                Some(typed)
            }
            Class::StrVar => {
                let typed = self.expr(argument)?;
                if !typed.variable {
                    self.err(argument.span(), "Variable required");
                    return None;
                }
                if !typed.ty.string() {
                    self.err(argument.span(), "String expression required");
                    return None;
                }
                Some(typed)
            }
            Class::Whole => self.expr(argument),
            Class::VarLenString => {
                let typed = self.argument_of_class(Class::StrVar, argument)?;
                // (QB 4.5 takes the address of a fixed-length string; PDS and
                // VBDOS refuse it)
                if matches!(typed.ty, Ty::Str(Some(_)))
                    && !matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45)
                {
                    self.err(argument.span(), "Fixed-length string illegal");
                    return None;
                }
                Some(typed)
            }
            Class::ArrayStart => match argument {
                Expr::Apply { name, arguments, span } if !arguments.is_empty() => match self.array_var(name) {
                    Some(var) => Some(Typed {
                        ty: var.ty,
                        variable: true,
                        constant: None,
                        unfolded: None,
                        array: Some(var.rank.unwrap_or(1)),
                    }),
                    None => {
                        self.err(*span, "Array not defined");
                        None
                    }
                },
                other => self.array_argument(other),
            },
            Class::Array => self.array_argument(argument),
            Class::Label => {
                if let Expr::Name(name, span) = argument {
                    if !self.labels.contains(name) {
                        self.err_about(*span, "Label not defined", name);
                    }
                    return Some(Typed::value(Ty::Int));
                }
                self.err(argument.span(), "Syntax error");
                None
            }
        }
    }

    /// An array named as an argument: `a()` or a bare name of an array.
    pub(super) fn array_argument(
        &mut self,
        argument: &Expr,
    ) -> Option<Typed> {
        let (name, span) = match argument {
            Expr::Name(name, span) => (name.clone(), *span),
            // an array field of a record: `r.a`, or `r.a()` as an argument
            Expr::Field { .. } if self.record_array(argument).is_some() => {
                return self.record_array(argument);
            }
            other => {
                if matches!(
                    other,
                    Expr::Apply { arguments, .. } | Expr::Index { indices: arguments, .. } if arguments.is_empty()
                ) {
                    self.err_because(
                        other.span(),
                        "Syntax error",
                        Reason::headline("`{text}` is not an array that can be passed here")
                            .help("a whole array is passed by its name; an array inside a record only as an argument of a CALL"),
                    );
                } else {
                    self.err(other.span(), "Array not defined");
                }
                return None;
            }
        };
        match self.array_var(&name) {
            Some(var) => Some(Typed {
                ty: var.ty,
                variable: true,
                constant: None,
                unfolded: None,
                array: Some(var.rank.unwrap_or(1)),
            }),
            None => {
                self.err(span, "Array not defined");
                None
            }
        }
    }

    pub(super) fn check_arguments(
        &mut self,
        name: &str,
        procedure: &ProcInfo,
        arguments: &[Expr],
        span: Span,
        is_call: bool,
    ) {
        let _ = is_call;
        // BC reads top down: a call above the first DECLARE or definition has
        // nothing to be checked against
        if procedure.unchecked || span.line < procedure.first_line {
            for argument in arguments {
                let inner = match argument {
                    Expr::Unary { op: Unary::ByValue | Unary::Segmented, operand, .. } => operand,
                    other => other,
                };
                // `a()` passes a whole array
                if matches!(inner, Expr::Apply { arguments, .. } if arguments.is_empty()) {
                    if let Expr::Apply { name, .. } = inner {
                        if self.array_var(name).is_some() {
                            continue;
                        }
                    }
                }
                if let Some(typed) = self.expr(inner) {
                    if typed.array.is_some() && matches!(inner, Expr::Field { .. }) {
                        self.err(inner.span(), "Syntax error");
                    }
                }
            }
            return;
        }
        if procedure.params.len() != arguments.len() {
            let count = |n: usize| match n {
                0 => "no arguments".to_owned(),
                1 => "1 argument".to_owned(),
                n => format!("{n} arguments"),
            };
            let headline = format!(
                "`{{name}}` is called with {}, but takes {}",
                count(arguments.len()),
                count(procedure.params.len())
            );
            self.err_because(
                span,
                "Argument-count mismatch",
                Reason::headline(headline)
                    .about(name)
                    .help("give the call exactly the arguments the SUB or FUNCTION declares"),
            );
            return;
        }
        for (index, (parameter, argument)) in procedure.params.iter().zip(arguments).enumerate() {
            // BYVAL and SEG at the call mark how the value goes; the value is
            // what is typed
            let argument = match argument {
                Expr::Unary { op: Unary::ByValue | Unary::Segmented, operand, .. } => &**operand,
                other => other,
            };
            if parameter.array {
                match self.array_argument_for(argument) {
                    Some(typed) => {
                        if !parameter.any && typed.ty.plain() != parameter.ty.plain() {
                            let headline = format!(
                                "argument {} of `{{other}}` is an array of {}, but the parameter is an array of {}",
                                index + 1,
                                typed.ty.label(),
                                parameter.ty.label()
                            );
                            self.err_because(
                                argument.span(),
                                "Parameter type mismatch",
                                Reason::headline(headline)
                                    .related(span)
                                    .help("pass an array of exactly the declared type"),
                            );
                        }
                    }
                    None => {
                        let headline = format!("argument {} of `{{other}}` must be an array", index + 1);
                        self.err_because(
                            argument.span(),
                            "Parameter type mismatch",
                            Reason::headline(headline).related(span).help("pass the array's name followed by `()`"),
                        );
                    }
                }
                continue;
            }
            let Some(typed) = self.expr(argument) else { continue };
            if typed.array.is_some() {
                let headline = format!(
                    "argument {} of `{{other}}` is an array, but the parameter is {}",
                    index + 1,
                    parameter.ty.label()
                );
                self.err_because(
                    argument.span(),
                    "Parameter type mismatch",
                    Reason::headline(headline).related(span).help("pass one element, such as `name(1)`"),
                );
                continue;
            }
            if parameter.any {
                continue;
            }
            let compatible = match (&typed.ty, &parameter.ty) {
                (Ty::Record(a), Ty::Record(b)) => a == b,
                (Ty::Str(_), Ty::Str(_)) => true,
                (a, b) if a.numeric() && b.numeric() => {
                    // a value converts to the parameter's type, but a variable
                    // or the result of a DEF FN is passed as it is
                    !(typed.variable || (self.dialect != Dialect::VbDos && self.is_def_fn_call(argument)))
                        || a == b
                        || parameter.by_value
                }
                _ => false,
            };
            if !compatible {
                let by_reference = typed.variable && !matches!(argument, Expr::Unary { op: Unary::Grouped, .. });
                let numeric_pair = typed.ty.numeric() && parameter.ty.numeric();
                let code = if by_reference || numeric_pair { "Parameter type mismatch" } else { "Type mismatch" };
                let headline = format!(
                    "argument {} of `{{other}}` is {}, but the parameter is declared as {}",
                    index + 1,
                    typed.ty.label(),
                    parameter.ty.label()
                );
                let help = if by_reference {
                    "BC does not convert a variable passed by reference: pass a variable of exactly that type, or wrap it in parentheses to pass its value"
                } else {
                    "convert the value (VAL, STR$, CINT...) or pass one of the declared type"
                };
                self.err_because(argument.span(), code, Reason::headline(headline).related(span).help(help));
            } else if let (Some(c), false) = (typed.constant, typed.variable) {
                // a constant is converted to the parameter's type where the
                // call stands
                let (low, high) = match parameter.ty {
                    Ty::Int => (-32768.0, 32767.0),
                    Ty::Long => (-2147483648.0, 2147483647.0),
                    Ty::Single => (-f64::from(f32::MAX), f64::from(f32::MAX)),
                    _ => (f64::MIN, f64::MAX),
                };
                let value = if parameter.ty == Ty::Single { c } else { c.round() };
                if !(low..=high).contains(&value) {
                    self.overflow(argument.span(), &parameter.ty, "parameter");
                }
            }
        }
    }

    /// `r.a` for an array field `a` is no value: BC takes it only where a whole
    /// array is asked for.
    pub(super) fn whole_array_field(
        &mut self,
        typed: &Typed,
        argument: &Expr,
    ) -> bool {
        let whole = typed.array.is_some() && matches!(argument, Expr::Field { .. });
        if whole {
            self.err_because(
                argument.span(),
                "Syntax error",
                Reason::headline("`{text}` is a whole array inside a record, which BC does not accept here")
                    .help("pass one element, `name(1)`; `name()` passes the whole array in a CALL"),
            );
        }
        whole
    }

    /// The whole array that `r.a` or `r.a()` names, when `a` is an array field
    /// of the record `r`.
    pub(super) fn record_array(
        &mut self,
        expr: &Expr,
    ) -> Option<Typed> {
        let field = match expr {
            Expr::Field { .. } => expr,
            Expr::Index { base, indices, .. } if indices.is_empty() && matches!(**base, Expr::Field { .. }) => &**base,
            _ => return None,
        };
        let errors = self.errors.len();
        let typed = self.expr(field);
        self.errors.truncate(errors);
        typed.filter(|typed| typed.array.is_some())
    }

    /// Whether `expr` calls a DEF FN function.
    pub(super) fn is_def_fn_call(
        &self,
        expr: &Expr,
    ) -> bool {
        let name = match expr {
            Expr::Name(name, _) | Expr::Apply { name, .. } => name,
            _ => return false,
        };
        self.procs.get(stem(name)).or_else(|| self.procs.get(name.as_str())).is_some_and(|procedure| procedure.def_fn)
    }

    pub(super) fn array_argument_for(
        &mut self,
        argument: &Expr,
    ) -> Option<Typed> {
        let (name, span) = match argument {
            Expr::Apply { name, arguments, span } if arguments.is_empty() => (name.clone(), *span),
            Expr::Index { indices, .. } if indices.is_empty() => return self.record_array(argument),
            _ => return None,
        };
        let var = self.array_var(&name);
        let _ = span;
        var.map(|var| Typed {
            ty: var.ty,
            variable: true,
            constant: None,
            unfolded: None,
            array: Some(var.rank.unwrap_or(1)),
        })
    }
}
