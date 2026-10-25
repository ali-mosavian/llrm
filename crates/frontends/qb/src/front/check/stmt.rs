//! What each statement may take.

use super::*;

// ---- statements
// ----------------------------------------------------------------------------------------------

impl Checker {
    pub(super) fn unwrap_step<'a>(
        &self,
        expr: &'a Expr,
    ) -> &'a Expr {
        match expr {
            Expr::Apply { name, arguments, .. } if name == crate::front::ast::STEP_MARK && arguments.len() == 1 => {
                &arguments[0]
            }
            other => other,
        }
    }

    /// Arguments against a spec of space-separated classes: n s x v t a l m, a
    /// `?` making one optional.
    pub(super) fn spec(
        &mut self,
        arguments: &[Expr],
        spec: &str,
        span: Span,
    ) -> bool {
        let parts: Vec<&str> = spec.split_whitespace().collect();
        let required = parts.iter().filter(|p| !p.ends_with('?')).count();
        if arguments.len() < required || arguments.len() > parts.len() {
            self.err(span, if arguments.len() > parts.len() { "Syntax error" } else { "Argument-count mismatch" });
            return false;
        }
        let mut ok = true;
        for (argument, part) in arguments.iter().zip(&parts) {
            let optional = part.ends_with('?');
            if matches!(argument, Expr::Omitted(_)) {
                if !optional {
                    self.err(argument.span(), "Syntax error");
                    ok = false;
                }
                continue;
            }
            let argument = self.unwrap_step(argument);
            let letter = part.chars().next().unwrap_or('n');
            if letter == 'r' {
                // a label anywhere in the module or in this procedure (RESTORE)
                if let Expr::Name(label, at) = argument {
                    if !self.labels.contains(label)
                        && !self.module_labels.contains(label)
                        && !self.all_labels.contains(label)
                    {
                        self.err_about(*at, "Label not defined", label);
                    }
                } else {
                    self.err(argument.span(), "Syntax error");
                }
                continue;
            }
            if letter == 'k' {
                // a keyword the statement spells (ON, KEY, a device name):
                // nothing to evaluate
                continue;
            }
            let class = match letter {
                'n' => Class::Num,
                'i' => Class::Int,
                'q' => Class::Long,
                's' => Class::Str,
                'x' => Class::Any,
                'v' => Class::Var,
                't' => Class::StrVar,
                'a' => Class::Array,
                'e' => Class::ArrayStart,
                'l' => Class::Label,
                'm' => {
                    // B or BF
                    if !matches!(argument, Expr::Name(name, _) if name == "B" || name == "BF") {
                        self.err(argument.span(), "Syntax error");
                        ok = false;
                    }
                    continue;
                }
                'p' => {
                    // a PUT action verb
                    if !matches!(
                        argument,
                        Expr::Name(name, _) if matches!(name.as_str(), "AND" | "OR" | "PRESET" | "PSET" | "XOR")
                    ) {
                        self.err(argument.span(), "Syntax error");
                        ok = false;
                    }
                    continue;
                }
                other => panic!("spec letter {other}"),
            };
            ok &= self.argument_of_class(class, argument).is_some();
        }
        ok
    }

    pub(super) fn statement(
        &mut self,
        statement: &Statement,
    ) {
        self.span = statement.span();
        match statement {
            Statement::DefType { type_name, ranges, .. } => {
                if let Some(ty) = self.type_of_name(type_name) {
                    for (from, to) in ranges {
                        for letter in *from..=*to {
                            self.def_types[usize::from(letter as u8 - b'A')] = ty.clone();
                        }
                    }
                }
            }
            Statement::TypeDecl { name, fields, span } => self.type_declaration(name, fields, *span),
            Statement::Dim(items) | Statement::Static(items) => {
                if matches!(statement, Statement::Static(_)) && self.procedure.is_none() {
                    self.err_because(
                        self.span,
                        "Subprogram error",
                        Reason::headline("STATIC can only be used inside a SUB or FUNCTION")
                            .help("use DIM SHARED, or move the statement into a procedure"),
                    );
                }
                // `DIM a()` names no bounds: VBDOS's
                if matches!(statement, Statement::Dim(_))
                    && !matches!(self.dialect, Dialect::VbDos | Dialect::Quickr)
                    && items.iter().any(|item| item.array && item.bounds.is_empty())
                {
                    self.err(self.span, "Syntax error");
                }
                if matches!(statement, Statement::Dim(_))
                    && self.procedure.is_some()
                    && items.iter().any(|item| item.shared)
                {
                    self.err_because(
                        self.span,
                        "Simple or array variable expected",
                        Reason::headline("DIM SHARED cannot be used inside a SUB or FUNCTION")
                            .help("declare it SHARED at module level, or write `SHARED name` inside the procedure to use a module variable"),
                    );
                }
                let keep = self.static_scope;
                self.static_scope |= matches!(statement, Statement::Static(_));
                self.static_statement = matches!(statement, Statement::Static(_));
                for item in items {
                    self.declare(item, false);
                }
                self.static_statement = false;
                self.static_scope = keep;
            }
            Statement::Redim(items) | Statement::RedimPreserve(items) => {
                for item in items {
                    self.declare(item, true);
                }
            }
            Statement::Shared(items) => {
                if self.procedure.is_none() {
                    self.err(self.span, "Illegal outside of SUB or FUNCTION");
                }
                for item in items {
                    self.share(item);
                }
            }
            Statement::Common { declarations, .. } => {
                for item in declarations {
                    // `COMMON x()` names an array that may already exist, or be
                    // dimensioned later
                    if item.array && item.bounds.is_empty() && self.array_var(&item.name).is_some() {
                        continue;
                    }
                    self.declare(item, false);
                }
            }
            Statement::Erase(names) => {
                for name in names {
                    self.argument_of_class(Class::Array, name);
                }
            }
            Statement::Const { name, value, span } => self.constant(name, value, *span),
            Statement::Assign { target, value, span } => self.assign(target, value, *span),
            Statement::Label(name, span) => {
                if crate::front::reserved::is_reserved_name(self.dialect, name) {
                    self.err(*span, "Statement unrecognizable");
                }
            }
            Statement::Comment(_) | Statement::OptionExplicit(_) => {}
            Statement::OptionBase(base, span) => {
                self.option_base = *base;
                if std::mem::replace(&mut self.option_base_seen, true) {
                    self.err_because(
                        *span,
                        "Syntax error",
                        Reason::headline("OPTION BASE appears twice")
                            .help("a program has one OPTION BASE: remove the second"),
                    );
                }
                let dimensioned = |scope: &Scope| scope.vars.values().flatten().any(|v| v.rank.is_some());
                if dimensioned(&self.scope) || dimensioned(&self.module_scope) {
                    self.err_because(
                        *span,
                        "Array already dimensioned",
                        Reason::headline("OPTION BASE comes after an array was dimensioned")
                            .help("move OPTION BASE above the first DIM of an array"),
                    );
                }
            }
            Statement::Goto(label, span) | Statement::CallOrGoto(label, span) => {
                // VBDOS takes `GOTO 0`
                let vbdos_zero = label == "0" && matches!(self.dialect, Dialect::VbDos | Dialect::Quickr);
                if !vbdos_zero && !self.labels.contains(label) {
                    self.err_about(*span, "Label not defined", label);
                }
            }
            Statement::If { condition, then_branch, else_branch, .. } => {
                self.numeric(condition);
                for s in then_branch.iter().chain(else_branch) {
                    self.statement(s);
                }
            }
            Statement::For { counter, start, end, step, body, next_variable, .. } => {
                if !matches!(counter, Expr::Name(..)) {
                    self.err(counter.span(), "FOR index variable illegal");
                }
                if let (Expr::Name(counter, counter_span), Some((next, span))) = (counter, next_variable) {
                    let counter_span = *counter_span;
                    // `i` and `i%` are two variables unless DEFINT makes them
                    // one
                    let same = stem(counter).eq_ignore_ascii_case(stem(next))
                        && self.default_type(counter).plain() == self.default_type(next).plain();
                    if !same {
                        self.err_because(
                            *span,
                            "NEXT without FOR",
                            Reason::headline(
                                "this NEXT names `{text}`, but the FOR loop it closes counts with `{other}`",
                            )
                            .help("write the same variable after NEXT as after FOR, or leave it out")
                            .related(counter_span),
                        );
                    }
                }
                self.declaring = true;
                let typed = self.expr(counter);
                self.declaring = false;
                if let Some(typed) = &typed {
                    if !typed.variable || !typed.ty.numeric() {
                        let headline = if typed.variable {
                            format!("a FOR counter must be a number, but `{{text}}` is {}", typed.ty.label())
                        } else {
                            "a FOR counter must be a variable, but `{text}` is not".to_owned()
                        };
                        self.err_because(
                            counter.span(),
                            "Type mismatch",
                            Reason::headline(headline).help("use a numeric variable such as `i%`"),
                        );
                    }
                }
                // an INTEGER counter takes constants of 16 bits
                let integer_counter = typed.as_ref().is_some_and(|t| t.ty == Ty::Int);
                for bound in [Some(start), Some(end), step.as_ref()].into_iter().flatten() {
                    if integer_counter {
                        self.integer(bound);
                    } else {
                        self.numeric(bound);
                    }
                }
                let counter_key = match counter {
                    Expr::Name(name, _) => Some(format!("{}{}", stem(name), self.type_key(name))),
                    _ => None,
                };
                if let Some(key) = &counter_key {
                    if self.for_counters.contains(key) {
                        self.err(counter.span(), "FOR index variable already in use");
                    }
                    self.for_counters.push(key.clone());
                }
                self.in_loop.push("FOR");
                for s in body {
                    self.statement(s);
                }
                self.in_loop.pop();
                if counter_key.is_some() {
                    self.for_counters.pop();
                }
            }
            Statement::While { condition, body, .. } => {
                self.numeric(condition);
                self.in_loop.push("WHILE");
                for s in body {
                    self.statement(s);
                }
                self.in_loop.pop();
            }
            Statement::Do { pre, post, body, .. } => {
                for (_, condition) in pre.iter().chain(post) {
                    self.numeric(condition);
                }
                self.in_loop.push("DO");
                for s in body {
                    self.statement(s);
                }
                self.in_loop.pop();
            }
            Statement::Select { selector, arms, otherwise, .. } => self.select(selector, arms, otherwise),
            Statement::Call { name, arguments, explicit, span } => self.call(name, arguments, *explicit, *span),
            Statement::Exit(target, span) => {
                use crate::syntax::ExitTarget;
                let ok = match target {
                    ExitTarget::Sub => matches!(self.procedure, Some((ProcedureKind::Sub, _))),
                    ExitTarget::Function => matches!(self.procedure, Some((ProcedureKind::Function, _))),
                    ExitTarget::Do => self.in_loop.contains(&"DO"),
                    ExitTarget::For => self.in_loop.contains(&"FOR"),
                };
                if !ok {
                    self.err(*span, "EXIT not within matching block");
                }
            }
            Statement::DefSeg { value, .. } => {
                if let Some(value) = value {
                    self.numeric(value);
                }
            }
            Statement::OnError { label, span, local } => {
                if *local {
                    self.on_local_error(*span);
                }
                // an error handler is module-level code, even when the
                // statement stands in a procedure ON ERROR names a module-level
                // handler; ON LOCAL ERROR one of the procedure
                let known = if self.procedure.is_some() && !*local {
                    self.module_labels.contains(label)
                } else {
                    self.labels.contains(label)
                };
                if label != "0" && !known {
                    self.err_about(*span, "Label not defined", label);
                }
            }
            Statement::OnErrorResumeNext { local, span } => {
                if *local {
                    self.on_local_error(*span);
                } else if matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45) {
                    self.err(*span, "GOTO missing");
                }
            }
            Statement::Resume { target, span } => {
                if let ResumeTarget::Label(label) = target {
                    if label != "0" && !self.labels.contains(label) {
                        self.err_about(*span, "Label not defined", label);
                    }
                }
            }
            Statement::OpenIsam { database, record_type, table, file, span } => {
                self.string_expr(database);
                if !self.records.contains_key(record_type) {
                    self.err(*span, "User-defined type not defined");
                }
                self.string_expr(table);
                self.integer(file);
            }
            Statement::Open { path, file, record_length, .. } => {
                self.string_expr(path);
                self.integer(file);
                if let Some(length) = record_length {
                    self.integer(length);
                }
            }
            Statement::OpenClassic { mode, file, path, record_length, .. } => {
                self.string_expr(mode);
                self.integer(file);
                self.string_expr(path);
                if let Some(length) = record_length {
                    self.numeric(length);
                }
            }
            Statement::Close { files, .. } => {
                for file in files {
                    self.integer(file);
                }
            }
            Statement::LineInput { file, prompt, destination, .. } => {
                if let Some(file) = file {
                    self.integer(file);
                }
                if let Some(prompt) = prompt {
                    self.string_expr(prompt);
                }
                if let Some(typed) = self.expr(destination) {
                    if !typed.variable || !typed.ty.string() {
                        self.err(
                            destination.span(),
                            if typed.variable { "String expression required" } else { "Variable required" },
                        );
                    }
                }
            }
            Statement::FileTransfer { file, position, target, .. } => {
                self.integer(file);
                if let Some(position) = position {
                    self.numeric(position);
                }
                if let Some(target) = target {
                    if let Some(typed) = self.expr(target) {
                        if self.whole_array_field(&typed, target) {
                        } else if !typed.variable {
                            self.err(target.span(), "Variable required");
                        }
                    }
                }
            }
            Statement::Seek { file, position, .. } => {
                self.integer(file);
                self.numeric(position);
            }
            Statement::Print { kind, file, using, items, .. } => {
                if let Some(file) = file {
                    self.integer(file);
                }
                if let Some(using) = using {
                    self.string_expr(using);
                }
                for item in items {
                    self.print_item(*kind, &item.value);
                }
            }
            Statement::Input { file, prompt, destinations, .. } => {
                if let Some(file) = file {
                    self.integer(file);
                }
                if let Some(prompt) = prompt {
                    self.string_expr(prompt);
                }
                for destination in destinations {
                    self.declaring = true;
                    let typed = self.expr(destination);
                    self.declaring = false;
                    if let Some(typed) = typed {
                        if !typed.variable || matches!(typed.ty, Ty::Record(_)) {
                            self.err(destination.span(), "Variable required");
                        }
                    }
                }
            }
            Statement::Data { .. } => {}
            Statement::Read { destinations, .. } => {
                for destination in destinations {
                    self.declaring = true;
                    let typed = self.expr(destination);
                    self.declaring = false;
                    if let Some(typed) = typed {
                        if !typed.variable || matches!(typed.ty, Ty::Record(_)) {
                            self.err(destination.span(), "Variable required");
                        }
                    }
                }
            }
            Statement::Runtime { name, arguments, span } => self.runtime(name, arguments, *span),
            Statement::MethodCall { object, arguments, .. } => {
                self.expr(object);
                for argument in arguments {
                    self.expr(argument);
                }
            }
            // '$FORM name: the form is a variable of the form type, seen
            // everywhere
            Statement::FormDeclaration(name, span) => {
                // VBDOS's BC refuses one inside a SUB or FUNCTION (corpus group
                // `meta`)
                if self.procedure.is_some() {
                    self.err(*span, "Must precede executable statements");
                }
                let var = Var {
                    ty: Ty::Record("FORM".into()),
                    rank: None,
                    declared: true,
                    shared: true,
                    dynamic: false,
                    explicit: true,
                    suffixed: false,
                    redimmed: false,
                };
                self.scope.vars.entry(stem(name).to_ascii_uppercase()).or_default().push(var);
            }
        }
    }

    pub(super) fn print_item(
        &mut self,
        kind: PrintKind,
        value: &Expr,
    ) {
        match value {
            Expr::Omitted(_) => {}
            Expr::Apply { name, arguments, .. } if (name == "TAB" || name == "SPC") && arguments.len() == 1 => {
                self.numeric(&arguments[0]);
            }
            other => {
                let typed = self.expr(other);
                let _ = kind;
                self.scalar(typed, other.span());
            }
        }
    }

    pub(super) fn select(
        &mut self,
        selector: &Expr,
        arms: &[(Vec<CaseItem>, Vec<Statement>)],
        otherwise: &[Statement],
    ) {
        let typed = self.expr(selector);
        let typed = self.scalar(typed, selector.span());
        for (items, body) in arms {
            for item in items {
                let (values, relation) = match item {
                    CaseItem::Value(a) => (vec![a], false),
                    CaseItem::Range(a, b) => (vec![a, b], false),
                    CaseItem::Relation(_, a) => (vec![a], true),
                };
                for value in values {
                    let integer_selector = typed.as_ref().is_some_and(|t| t.ty == Ty::Int);
                    let case = if integer_selector
                        && matches!(
                            value,
                            Expr::Literal(Literal::Integer(..), _) | Expr::Unary { .. }
                        )
                    {
                        self.integer(value)
                    } else {
                        self.expr(value)
                    };
                    let Some(case) = case else { continue };
                    if let Some(selector) = &typed {
                        if selector.ty.string() != case.ty.string() || matches!(case.ty, Ty::Record(_)) {
                            let headline = format!(
                                "the CASE value `{{text}}` is {}, but the SELECT CASE expression is {}",
                                case.ty.label(),
                                selector.ty.label()
                            );
                            self.err_because(
                                value.span(),
                                "Type mismatch",
                                Reason::headline(headline)
                                    .help("CASE values must be the same kind (number or string) as the expression"),
                            );
                        } else if relation && selector.ty.string() {
                            // CASE IS < "a" is fine
                        }
                    }
                }
            }
            for s in body {
                self.statement(s);
            }
        }
        for s in otherwise {
            self.statement(s);
        }
    }

    pub(super) fn assign(
        &mut self,
        target: &Expr,
        value: &Expr,
        span: Span,
    ) {
        let value_typed = self.expr(value);
        // PDS and VBDOS: ERR = n
        if matches!(target, Expr::Name(name, _) if name == "ERR") {
            if matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45) {
                self.err(span, "Statement unrecognizable");
            } else {
                self.numeric(value);
            }
            return;
        }
        // MID$(s$, n, n) = x
        if let Expr::Apply { name, arguments, .. } = target {
            if name == "MID$" {
                if arguments.len() < 2 || arguments.len() > 3 {
                    self.err(span, "Argument-count mismatch");
                    return;
                }
                self.argument_of_class(Class::StrVar, &arguments[0]);
                self.numeric(&arguments[1]);
                if let Some(third) = arguments.get(2) {
                    self.numeric(third);
                }
                if let Some(typed) = value_typed {
                    if !typed.ty.string() {
                        let headline = format!("`{{text}}` is {}, but a string is needed here", typed.ty.label());
                        self.err_because(
                            value.span(),
                            "Type mismatch",
                            Reason::headline(headline).help("convert the number with STR$(...)"),
                        );
                    }
                }
                return;
            }
        }
        // a FUNCTION assigns its result through its own name
        if let (Expr::Name(name, _), Some((ProcedureKind::Function, own))) = (target, &self.procedure) {
            if stem(name) == stem(own) {
                let ty =
                    self.procs.get(stem(own)).or_else(|| self.procs.get(own.as_str())).and_then(|p| p.result.clone());
                // `f$ = …` in FUNCTION f% names another variable than the
                // result
                if let (Some(suffix), Some(result)) = (Ty::of_suffix(name), &ty) {
                    if suffix.plain() != result.plain() {
                        self.err_because(
                            span,
                            "Duplicate definition",
                            Reason::headline(
                                "`{name}` has a different type character than the FUNCTION it is in, so it is another variable, not the result",
                            )
                            .help("assign the result to the function's own name"),
                        );
                        return;
                    }
                }
                if let (Some(ty), Some(v)) = (ty, value_typed) {
                    self.assignable(&ty, &v, value.span());
                }
                return;
            }
        }
        let target_typed = match target {
            Expr::Name(name, _) if self.consts.contains_key(stem(name)) => {
                self.err_because(
                    span,
                    "Duplicate definition",
                    Reason::headline("`{name}` is a CONST and cannot be assigned to")
                        .help("use a variable, or change the CONST"),
                );
                return;
            }
            _ => {
                self.assigning = true;
                let typed = self.expr(target);
                self.assigning = false;
                typed
            }
        };
        let (Some(t), Some(v)) = (target_typed, value_typed) else { return };
        if !t.variable {
            self.err(span, "Variable required");
            return;
        }
        if t.array.is_some() {
            self.err_because(
                span,
                "Syntax error",
                Reason::headline("`{text}` is a whole array and cannot be assigned to")
                    .help("assign to one element, as in `name(1) = value`"),
            );
            return;
        }
        self.assignable(&t.ty, &v, value.span());
    }

    pub(super) fn assignable(
        &mut self,
        target: &Ty,
        value: &Typed,
        span: Span,
    ) {
        if value.array.is_some() {
            self.err_because(
                span,
                "Syntax error",
                Reason::headline("`{text}` is a whole array, but a single value is needed here")
                    .help("write an index, as in `name(1)`"),
            );
            return;
        }
        let ok = match (target, &value.ty) {
            // a property of a form or control: its file says what it is
            (Ty::Variant, _) | (_, Ty::Variant) => true,
            (Ty::Record(a), Ty::Record(b)) => a == b,
            (Ty::Str(_), Ty::Str(_)) => true,
            (a, b) if a.numeric() && b.numeric() => {
                // a constant that cannot fit; PDS and VBDOS fold only integer
                // literals into integers
                if let (Some(c), true) = (value.constant, !value.variable) {
                    let classic = matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45);
                    let integer_literal = matches!(b, Ty::Int | Ty::Long);
                    let fits = match a {
                        Ty::Int => c.round() >= -32768.0 && c.round() <= 32767.0,
                        Ty::Long => c.round() >= -2147483648.0 && c.round() <= 2147483647.0,
                        Ty::Single => c.abs() <= f64::from(f32::MAX),
                        _ => true,
                    };
                    if !fits && (classic || integer_literal || matches!(a, Ty::Single)) {
                        self.overflow(span, &a, "variable");
                        return;
                    }
                }
                true
            }
            _ => false,
        };
        if !ok {
            let help = match (target.string(), value.ty.string()) {
                (true, false) if value.ty.numeric() => "convert the number with STR$(...)",
                (false, true) if target.numeric() => "convert the string with VAL(...)",
                _ => "give the value the same type as the place it is stored in",
            };
            let headline = format!("`{{text}}` is {}, but this place needs {}", value.ty.label(), target.label());
            self.err_because(span, "Type mismatch", Reason::headline(headline).help(help));
        }
    }
}
