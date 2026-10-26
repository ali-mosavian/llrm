//! CALL and the statements BC reads as calls with a shape of their own.

use super::*;

impl Checker {
    pub(super) fn call(
        &mut self,
        name: &str,
        arguments: &[Expr],
        explicit: bool,
        span: Span,
    ) {
        if explicit && Ty::of_suffix(name).is_some() {
            self.err(span, "Invalid character");
            return;
        }
        if explicit && !name.contains('.') && crate::front::reserved::is_reserved_name(self.dialect, name) {
            self.err(span, "Name of subprogram illegal");
            return;
        }
        // the statements the syntax stores as implicit calls
        if !explicit {
            let handled = match name {
                "GOSUB" => Some("l"),
                "RETURN" => Some("l?"),
                "RANDOMIZE" => Some("n?"),
                "SHELL" => Some("s?"),
                "STOP" | "RESET" => Some(""),
                "CLEAR" => Some("n? n? n?"),
                "ENVIRON" | "KILL" | "MKDIR" | "CHDIR" | "RMDIR" => Some("s"),
                "BLOAD" => Some("s n?"),
                "BSAVE" => Some("s n n"),
                _ => None,
            };
            if let Some(spec) = handled {
                if self.procs.get(name).is_none() {
                    // a procedure's RETURN takes no label: it cannot leave a
                    // procedure
                    if name == "RETURN" && !arguments.is_empty() && self.procedure.is_some() {
                        self.err_because(
                            span,
                            "Subprogram error",
                            Reason::headline(
                                "`RETURN label` cannot be used inside a SUB or FUNCTION: it would leave the procedure",
                            )
                            .help("use a plain `RETURN`, or move the GOSUB and its label into the procedure"),
                        );
                        return;
                    }
                    self.spec(arguments, spec, span);
                    return;
                }
            }
            if matches!(name, "LSET" | "RSET") {
                if let [Expr::Binary { left, right, .. }] = arguments {
                    // LSET moves a string into a string variable, or one record
                    // into another
                    if let Some(target) = self.argument_of_class(Class::Var, left) {
                        match &target.ty {
                            Ty::Str(_) => {
                                self.string_expr(right);
                            }
                            Ty::Record(_) if name == "LSET" => {
                                if let Some(value) = self.expr(right) {
                                    if !matches!(value.ty, Ty::Record(_)) {
                                        let headline = format!(
                                            "LSET of a record needs a record on the right, but `{{text}}` is {}",
                                            value.ty.label()
                                        );
                                        self.err_because(
                                            right.span(),
                                            "Type mismatch",
                                            Reason::headline(headline).help("use a variable of the same TYPE"),
                                        );
                                    }
                                }
                            }
                            Ty::Record(_) => self.err(left.span(), "String assignment required"),
                            _ => self.err(left.span(), "Record/String assignment required"),
                        }
                    }
                } else {
                    self.err(span, "Syntax error");
                }
                return;
            }
        }
        // `frmMain.cmdOK.Move 1, 2`, `SCREEN.HIDE`: a method of a VBDOS form,
        // control or object
        if let Some((root, _)) = name.split_once('.') {
            let object = matches!(self.dialect, Dialect::VbDos | Dialect::Quickr)
                && (matches!(
                    root.to_ascii_uppercase().as_str(),
                    "SCREEN" | "PRINTER" | "CLIPBOARD"
                )
                    || self.lookup_record_var(root));
            if object {
                for argument in arguments {
                    let inner = match argument {
                        Expr::Unary { op: Unary::ByValue | Unary::Segmented, operand, .. } => &**operand,
                        other => other,
                    };
                    self.expr(inner);
                }
                return;
            }
        }
        let key = stem(name);
        match self.procs.get(key).cloned() {
            // a SUB named only below its first use is no statement yet: BC
            // takes the name for an assignment
            Some(procedure)
                if procedure.kind == ProcedureKind::Sub && !explicit && span.line < procedure.first_line =>
            {
                self.err(span, "Equal sign missing");
            }
            Some(procedure) if procedure.kind == ProcedureKind::Sub => {
                self.check_arguments(name, &procedure, arguments, span, true);
            }
            Some(_) => self.err(span, if explicit { "Name of subprogram illegal" } else { "Variable required" }),
            None if sigs::function(name, self.dialect).is_some_and(|sig| sig.statement) => {
                if let Some(sig) = sigs::function(name, self.dialect) {
                    self.intrinsic(sig, name, arguments, span);
                }
            }
            None => {
                if !explicit {
                    // a statement that starts with an unknown name is taken for
                    // an assignment
                    self.err(span, if arguments.is_empty() { "Equal sign missing" } else { "DECLARE required" });
                    return;
                }
                for argument in arguments {
                    if let Some(typed) = self.expr(argument) {
                        self.whole_array_field(&typed, argument);
                    }
                }
            }
        }
    }

    /// PAINT: a background needs a string tile; empty trailing slots only
    /// follow a tile, and not in VBDOS.
    pub(super) fn paint(
        &mut self,
        arguments: &[Expr],
        span: Span,
    ) {
        let mut stripped = arguments;
        while matches!(stripped.last(), Some(Expr::Omitted(_))) {
            stripped = &stripped[..stripped.len() - 1];
        }
        let trailing = stripped.len() != arguments.len();
        let before = self.errors.len();
        if !self.spec(stripped, "n n x? i? s?", span) {
            return;
        }
        let tile = match stripped.get(2) {
            Some(Expr::Omitted(_)) | None => false,
            Some(paint) => {
                let paint = self.unwrap_step(paint).clone();
                let typed = self.expr(&paint);
                self.errors.truncate(before);
                typed.is_some_and(|typed| typed.ty.string())
            }
        };
        let vbdos = matches!(self.dialect, Dialect::VbDos | Dialect::Quickr);
        let dangling = trailing && (!tile || vbdos || arguments.len() < 5);
        if (stripped.len() > 4 && !tile) || dangling {
            self.err(span, "Syntax error");
        }
    }

    /// The ISAM statements whose argument lists have a shape of their own.
    pub(super) fn isam_list(
        &mut self,
        name: &str,
        arguments: &[Expr],
        span: Span,
    ) {
        let [table, rest @ ..] = arguments else {
            self.err(span, "Syntax error");
            return;
        };
        self.numeric(table);
        match name {
            // INSERT #table, record
            "INSERT" | "RETRIEVE" | "UPDATE" => {
                if let [record] = rest {
                    if let Some(typed) = self.argument_of_class(Class::Var, record) {
                        if !matches!(typed.ty, Ty::Record(_)) {
                            self.err(record.span(), "Record variable required");
                        }
                    }
                } else {
                    self.err(span, "Syntax error");
                }
            }
            // CREATEINDEX #table, name$, unique, column$...
            "CREATEINDEX" => {
                if rest.len() < 3 {
                    self.err(span, "Syntax error");
                    return;
                }
                self.string_expr(&rest[0]);
                self.numeric(&rest[1]);
                for column in &rest[2..] {
                    self.string_expr(column);
                }
            }
            // SEEKEQ #table, key...
            _ => {
                for key in rest {
                    let typed = self.expr(key);
                    self.scalar(typed, key.span());
                }
            }
        }
    }

    /// FIELD #n, width AS string-variable, ...: the variables are
    /// variable-length strings.
    pub(super) fn field_statement(
        &mut self,
        arguments: &[Expr],
        span: Span,
    ) {
        let [file, pairs @ ..] = arguments else {
            self.err(span, "Syntax error");
            return;
        };
        self.integer(file);
        for pair in pairs.chunks(2) {
            let [width, variable] = pair else {
                self.err(span, "Syntax error");
                return;
            };
            self.integer(width);
            if let Some(typed) = self.expr(variable) {
                if !typed.variable || !matches!(typed.ty, Ty::Str(None)) {
                    self.err(variable.span(), "Variable-length string required");
                }
            }
        }
    }

    /// LOCAL ERROR belongs to PDS and VBDOS, and to procedures.
    pub(super) fn on_local_error(
        &mut self,
        span: Span,
    ) {
        if matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45) {
            self.err(span, "Syntax error");
        } else if self.procedure.is_none() {
            self.err(span, "Illegal outside of SUB or FUNCTION");
        }
    }

    pub(super) fn runtime(
        &mut self,
        name: &str,
        arguments: &[Expr],
        span: Span,
    ) {
        let spec = match name {
            "SCREEN" => "i? i? i? i?",
            "COLOR" => "i? i? i?",
            "CLS" => "i?",
            "SLEEP" => "n?",
            "LOCATE" => "i? i? i? i? i?",
            "WIDTH" => "x n?",
            "WIDTH_FILE" => "i i",
            "WIDTH_LPRINT" => "i",
            "PSET" | "PRESET" => "n n i?",
            "LINE" => "n n n n i? m? i?",
            "CIRCLE" => "n n n i? n? n? n?",
            "PAINT" => "n n x? i? s?",
            "DRAW" | "PLAY" => "s",
            "GET" => "n n n n e",
            "PUT" => "n n e p?",
            "VIEW" | "VIEW_SCREEN" => "i? i? i? i? i? i?",
            "VIEW_PRINT" => "i? i?",
            "WINDOW" | "WINDOW_SCREEN" => "n? n? n? n?",
            "PALETTE" => "i? n?",
            "PCOPY" => "i i",
            "SOUND" => "i n",
            "POKE" | "OUT" => "n i",
            "WAIT" => "n i i?",
            "BEEP" | "TRON" | "TROFF" => "",
            "SYSTEM" | "END" => "i?",
            "FILES" | "SHELL" => "s?",
            "RUN" => "x?",
            "CHAIN" => "s",
            "ERROR" => "n",
            "NAME" => "s s",
            "SWAP" => "v v",
            "RESTORE" => "r?",
            "DATE$" | "TIME$" => "s",
            "KEY"
                if matches!(
                    arguments,
                    [Expr::Name(word, _)] if matches!(word.as_str(), "ON" | "OFF" | "LIST")
                ) =>
            {
                "k"
            }
            "KEY" => "i s",
            "EVENT" => "k i? k l?",
            "LOCK" | "UNLOCK" => "n n? n?",
            "IOCTL" => "i s",
            "FIELD" | "PALETTE_USING" | "CREATEINDEX" | "INSERT" | "RETRIEVE" | "UPDATE" | "SEEKEQ" | "SEEKGE"
            | "SEEKGT" => "",
            "BEGINTRANS" | "COMMITTRANS" | "CHECKPOINT" => "",
            "CHDRIVE" => "s",
            "LOAD" | "UNLOAD" => "",
            "STACK" => "n?",
            "EVENT_SWITCH" => "k",
            "ROLLBACK" => "n?",
            "ROLLBACK_ALL" => "",
            "DELETE" | "MOVEFIRST" | "MOVELAST" | "MOVENEXT" | "MOVEPREVIOUS" => "n",
            "DELETEINDEX" => "n s",
            "DELETETABLE" => "s s",
            "SETINDEX" => "n s?",
            "ENDIF" => "",
            _ => {
                self.err(span, "Syntax error");
                return;
            }
        };
        if matches!(
            name,
            "CREATEINDEX" | "INSERT" | "RETRIEVE" | "UPDATE" | "SEEKEQ" | "SEEKGE" | "SEEKGT"
        ) {
            self.isam_list(name, arguments, span);
            return;
        }
        if matches!(name, "LOAD" | "UNLOAD") {
            // the form to load or unload is a variable of the form type
            // (a control array element too: LOAD frmFont.chkFonts(i%))
            let mut root = arguments.first();
            while let Some(expr) = root {
                root = match expr {
                    Expr::Field { base, .. } => Some(&**base),
                    Expr::Index { base, .. } => Some(&**base),
                    Expr::Apply { name, .. } => {
                        if !self.lookup_record_var(name.split('.').next().unwrap_or(name)) {
                            self.err(span, "Form variable already in use or illegal");
                        }
                        break;
                    }
                    Expr::Name(form, at) => {
                        if !self.lookup_record_var(form) {
                            self.err(*at, "Form variable already in use or illegal");
                        }
                        break;
                    }
                    _ => {
                        self.err(span, "Syntax error");
                        break;
                    }
                };
            }
            if arguments.len() != 1 {
                self.err(span, "Syntax error");
            }
            return;
        }
        if name == "FIELD" {
            self.field_statement(arguments, span);
            return;
        }
        if name == "RUN" {
            // RUN "file", RUN line-number
            if let [argument] = arguments {
                if let Expr::Literal(Literal::Integer(number, _), at) = argument {
                    if !self.labels.contains(&number.to_string()) {
                        self.err_about(*at, "Label not defined", &number.to_string());
                    }
                } else {
                    self.string_expr(argument);
                }
            }
            return;
        }
        if name == "EVENT"
            && matches!(
                arguments.first(),
                Some(Expr::Name(device, _)) if device == "SIGNAL"
            )
        {
            self.err(span, "Feature unavailable");
            return;
        }
        if name == "PALETTE" && arguments.len() == 1 {
            self.err(span, "Syntax error");
            return;
        }
        if name == "PALETTE_USING" {
            // an INTEGER or LONG array, or an element of one to start from
            if let [array] = arguments {
                let typed = match array {
                    Expr::Apply { arguments, .. } if !arguments.is_empty() => {
                        self.expr(array).filter(|typed| typed.variable)
                    }
                    _ => self.array_argument(array),
                };
                if let Some(typed) = typed {
                    if !matches!(typed.ty.plain(), Ty::Int | Ty::Long) {
                        self.err(span, "Syntax error");
                    }
                } else if !matches!(array, Expr::Apply { .. } | Expr::Name(..)) {
                    self.err(span, "Syntax error");
                }
            }
            return;
        }
        if name == "PAINT" {
            self.paint(arguments, span);
            return;
        }
        if name == "LINE" {
            // `,color,` and `,,` are BC's; VBDOS refuses the dangling comma
            if matches!(arguments.last(), Some(Expr::Omitted(_)))
                && arguments.len() == 6
                && matches!(self.dialect, Dialect::VbDos | Dialect::Quickr)
            {
                self.err(span, "Syntax error");
                return;
            }
        }
        if name == "SWAP" {
            if let [a, b] = arguments {
                let (a_typed, b_typed) = (self.argument_of_class(Class::Var, a), self.argument_of_class(Class::Var, b));
                if let (Some(a), Some(b)) = (a_typed, b_typed) {
                    if a.ty.plain() != b.ty.plain() {
                        self.err_because(
                            span,
                            "Type mismatch",
                            Reason::headline(format!(
                                "SWAP needs two variables of the same type, but the first is {} and the second is {}",
                                a.ty.label(),
                                b.ty.label()
                            ))
                            .help("convert one of them into a variable of the other's type first"),
                        );
                    }
                }
            } else {
                self.err_because(
                    span,
                    "Argument-count mismatch",
                    Reason::headline("SWAP needs exactly two variables")
                        .about("SWAP")
                        .help("write `SWAP a, b`; both must be variables of the same type"),
                );
            }
            return;
        }
        // ON TIMER takes seconds, up to a day's worth
        let spec = if name == "EVENT"
            && matches!(
                arguments.first(),
                Some(Expr::Name(device, _)) if device == "TIMER"
            ) {
            "k n? k l?"
        } else {
            spec
        };
        self.spec(arguments, spec, span);
    }
}
