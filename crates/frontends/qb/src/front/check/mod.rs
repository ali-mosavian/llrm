//! The validity check: what BC refuses after the syntax is right.
//!
//! A symbol table over the typed syntax (`syntax::Module`): types and
//! constants, variables by the name's stem and type, procedures and their
//! signatures, labels. It types every expression by BC's rules and checks each
//! statement's arguments against its signature. Diagnostics carry
//! BC's own wording where BC's is known; what matters, and what the sweeps
//! against BC compare, is whether a program has any.

use llrm_support::hash::{HashMap, HashSet};

use crate::Dialect;
use crate::front::sigs::{self, Class, Ret};
use crate::syntax::{
    Binary, CaseItem, Declaration, Expr, Literal, Module, Parameter, PrintKind, Procedure, ProcedureKind, ResumeTarget,
    Span, Statement, TypeName, Unary,
};

mod argument;
mod call;
mod constant;
mod declare;
mod expr;
mod fold;
mod record;
mod stmt;
mod types;

use types::*;
pub use types::{Diagnostic, Reason};

pub fn check(
    module: &Module,
    dialect: Dialect,
) -> Vec<Diagnostic> {
    let mut checker = Checker::new(dialect);
    checker.module(module);
    checker.errors
}

struct Checker {
    dialect: Dialect,
    errors: Vec<Diagnostic>,
    def_types: [Ty; 26],
    records: HashMap<String, Record>,
    consts: HashMap<String, Typed>,
    procs: HashMap<String, ProcInfo>,
    module_scope: Scope,
    scope: Scope,
    /// Inside a procedure: its kind and name.
    procedure: Option<(ProcedureKind, String)>,
    /// The procedure is STATIC, or a STATIC statement is being declared: its
    /// arrays are fixed.
    static_scope: bool,
    /// An OPTION BASE has been read.
    option_base_seen: bool,
    /// The lower bound an array gets when it names none (OPTION BASE).
    option_base: i64,
    /// The bytes of the static arrays declared so far: BC has 64 KB of data for
    /// them and the rest.
    static_bytes: usize,
    /// A STATIC statement is being declared.
    static_statement: bool,
    /// The names being read are made by their statement (FOR, INPUT, READ), as
    /// DIM would.
    declaring: bool,
    /// The target of an assignment is being read.
    assigning: bool,
    /// The counters of the FOR loops being read.
    for_counters: Vec<String>,
    labels: HashSet<String>,
    /// The numeric labels that stand on a line with no statement: defined, but
    /// never a duplicate of another.
    weak_labels: HashSet<String>,
    /// `ON ERROR GOTO 0` or `RESUME 0` came before a line number 0: BC reads
    /// the 0 as a label reference, and the later definition is a Duplicate
    /// label.
    zero_referenced: bool,
    /// The module level's labels, kept while a procedure's are the ones in use.
    module_labels: HashSet<String>,
    /// Every label of the file, module and procedures (RESTORE reaches any).
    all_labels: HashSet<String>,
    span: Span,
    in_loop: Vec<&'static str>,
    /// The dimensions of the array field the last `field` resolved, if it was
    /// one.
    field_rank: Option<usize>,
}

impl Checker {
    fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            errors: Vec::new(),
            def_types: std::array::from_fn(|_| Ty::Single),
            records: HashMap::default(),
            consts: HashMap::default(),
            procs: HashMap::default(),
            module_scope: Scope::default(),
            scope: Scope::default(),
            procedure: None,
            static_scope: false,
            option_base_seen: false,
            option_base: 0,
            static_bytes: 0,
            static_statement: false,
            declaring: false,
            assigning: false,
            for_counters: Vec::new(),
            labels: HashSet::default(),
            weak_labels: HashSet::default(),
            zero_referenced: false,
            module_labels: HashSet::default(),
            all_labels: HashSet::default(),
            span: Span { line: 1, start: 0, end: 0 },
            in_loop: Vec::new(),
            field_rank: None,
        }
    }

    fn err(
        &mut self,
        span: Span,
        message: &str,
    ) {
        self.errors.push(Diagnostic { span, message: message.into(), ..Diagnostic::default() });
    }

    /// `err`, naming the word of the span at fault.
    fn err_about(
        &mut self,
        span: Span,
        message: &str,
        subject: &str,
    ) {
        self.errors.push(Diagnostic {
            span,
            message: message.into(),
            subject: Some(subject.to_owned()),
            ..Diagnostic::default()
        });
    }

    /// `err` with the reason in words, and the word of the span at fault when
    /// there is one.
    fn err_because(
        &mut self,
        span: Span,
        message: &str,
        reason: Reason,
    ) {
        self.errors.push(Diagnostic {
            span,
            message: message.into(),
            subject: reason.subject,
            headline: reason.headline,
            detail: reason.help,
            related: reason.related,
        });
    }

    /// "Wrong number of dimensions", with both counts.
    fn dimensions(
        &mut self,
        span: Span,
        dimensioned: usize,
        given: usize,
    ) {
        let word = |n: usize| if n == 1 { "subscript" } else { "subscripts" };
        let headline = format!(
            "`{{name}}` has {dimensioned} dimension{} but is indexed with {given} {}",
            if dimensioned == 1 { "" } else { "s" },
            word(given)
        );
        self.err_because(
            span,
            "Wrong number of dimensions",
            Reason::headline(headline).help("give as many subscripts as the array was dimensioned with"),
        );
    }

    /// "Math overflow" for a constant that does not fit `ty`, naming the type
    /// and its range; `place` says where it goes ("an argument", "a bound").
    fn overflow(
        &mut self,
        span: Span,
        ty: &Ty,
        place: &str,
    ) {
        let (name, range) = match ty {
            Ty::Int => ("an INTEGER", "-32768 to 32767"),
            Ty::Long => ("a LONG", "-2147483648 to 2147483647"),
            _ => ("a SINGLE", "about -3.4E38 to 3.4E38"),
        };
        let help = match ty {
            Ty::Int => "use a smaller value, or a LONG (`&`) where the statement accepts one",
            Ty::Long => "use a smaller value, or a double (`#`) where the statement accepts one",
            _ => "use a smaller value, or a double (`#`)",
        };
        self.err_because(
            span,
            "Math overflow",
            Reason::headline(format!("`{{text}}` does not fit {name} {place} ({range})")).help(help),
        );
    }

    // ---- the module
    // ----------------------------------------------------------------------------

    fn module(
        &mut self,
        module: &Module,
    ) {
        // A procedure's signature depends on what precedes it: DEFtype ranges,
        // CONSTs, record types. They are read in order here, on a scratch state
        // that the statement pass starts over from.
        let before = self.errors.len();
        let mut pending: Vec<&Procedure> = module.procedures.iter().collect();
        pending.sort_by_key(|procedure| procedure.span.line);
        let mut next = 0;
        for statement in &module.statements {
            while next < pending.len() && pending[next].span.line < statement.span().line {
                self.declare_procedure(pending[next]);
                next += 1;
            }
            let mark = self.errors.len();
            match statement {
                Statement::DefType { .. } => self.statement(statement),
                Statement::Const { name, value, span } => self.constant(name, value, *span),
                Statement::TypeDecl { name, fields, span } => self.type_declaration(name, fields, *span),
                _ => {}
            }
            self.errors.truncate(mark);
        }
        for procedure in &pending[next..] {
            self.declare_procedure(procedure);
        }
        let _ = before;
        self.consts.clear();
        self.records.clear();
        self.scope = Scope::default();
        self.def_types = std::array::from_fn(|_| Ty::Single);
        // COMMON and DECLARE come before the first executable statement
        let first_executable = module
            .statements
            .iter()
            .find(|statement| {
                !matches!(
                    statement,
                    Statement::Dim(_)
                        | Statement::Const { .. }
                        | Statement::TypeDecl { .. }
                        | Statement::DefType { .. }
                        | Statement::Comment(_)
                        | Statement::OptionBase(..)
                        | Statement::OptionExplicit(_)
                        | Statement::Common { .. }
                        | Statement::Static(_)
                        | Statement::Shared(_)
                        | Statement::Label(..)
                        | Statement::FormDeclaration(..)
                )
            })
            .map(|statement| statement.span().line);
        if let Some(line) = first_executable {
            for statement in &module.statements {
                if let Statement::Common { span, .. } = statement {
                    if span.line > line {
                        self.err(*span, "COMMON and DECLARE must precede executable statements");
                    }
                }
            }
            for procedure in module.procedures.iter().filter(|procedure| procedure.declaration) {
                if procedure.span.line > line {
                    self.err(procedure.span, "COMMON and DECLARE must precede executable statements");
                }
            }
        }
        self.collect_labels(&module.statements);
        self.all_labels = self.labels.clone();
        for procedure in &module.procedures {
            collect_label_names(&procedure.body, &mut self.all_labels);
        }
        let statements = module.statements.clone();
        for statement in &statements {
            self.statement(statement);
        }
        self.module_scope = self.scope.clone();
        self.module_labels = self.labels.clone();
        for procedure in &module.procedures {
            if procedure.declaration {
                continue;
            }
            self.procedure_body(procedure);
        }
    }

    fn collect_labels(
        &mut self,
        statements: &[Statement],
    ) {
        for (index, statement) in statements.iter().enumerate() {
            if matches!(statement, Statement::OnError { label, .. } if label == "0")
                || matches!(
                    statement,
                    Statement::Resume { target: ResumeTarget::Label(label), .. } if label == "0"
                )
            {
                self.zero_referenced = true;
            }
            if let Statement::Label(name, span) = statement {
                if name == "0" && self.zero_referenced {
                    self.err(*span, "Duplicate label");
                }
                // a line number alone on its line (or before a comment) is a
                // target, and no duplicate
                let numeric = name.chars().all(|c| c.is_ascii_digit() || c == '.');
                let alone = statements.get(index + 1).is_none_or(|next| next.span().line != span.line);
                // (the module's only: a procedure's line numbers all count)
                if numeric && alone && self.procedure.is_none() {
                    self.labels.insert(name.clone());
                    self.weak_labels.insert(name.clone());
                } else {
                    // a label in a procedure may not repeat one of the module's
                    if self.procedure.is_some() && self.module_labels.contains(name) {
                        self.err(*span, "Duplicate label");
                    }
                    if !self.labels.insert(name.clone()) && !self.weak_labels.remove(name) {
                        self.err(*span, "Duplicate label");
                    }
                }
            }
            for body in statement.bodies() {
                self.collect_labels(body);
            }
        }
    }

    fn type_of_name(
        &self,
        name: &TypeName,
    ) -> Option<Ty> {
        match name {
            TypeName::Integer => Some(Ty::Int),
            TypeName::Long => Some(Ty::Long),
            TypeName::Single => Some(Ty::Single),
            TypeName::Double => Some(Ty::Double),
            TypeName::Currency => Some(Ty::Currency),
            TypeName::String => Some(Ty::Str(None)),
            TypeName::Named(name) if name == "ANY" => None,
            TypeName::Named(name) => {
                (self.records.contains_key(name) || self.builtin_object(name)).then(|| Ty::Record(name.clone()))
            }
            _ => None,
        }
    }

    /// VBDOS's own record types: a FORM and a CONTROL.
    fn builtin_object(
        &self,
        name: &str,
    ) -> bool {
        matches!(self.dialect, Dialect::VbDos | Dialect::Quickr) && matches!(name, "FORM" | "CONTROL")
    }

    /// A word BC reserves cannot name anything; dotted names are checked as the
    /// variables they are.
    fn reserved_name(
        &mut self,
        name: &str,
        span: Span,
    ) -> bool {
        // DATA, REM, TRON and TROFF swallow the rest of their line; a suffixed
        // spelling is no use of the word
        let word_with_suffix = Ty::of_suffix(name).is_some()
            && matches!(
                stem(name).to_ascii_uppercase().as_str(),
                "DATA" | "REM" | "TRON" | "TROFF"
            );
        if name.contains('.') || word_with_suffix || !crate::front::reserved::is_reserved_name(self.dialect, name) {
            return false;
        }
        self.err_because(
            span,
            "Identifier expected",
            Reason::headline("`{text}` is a reserved word and cannot be used as a name")
                .about(name)
                .help(
                    "rename it; the manual lists the words each dialect reserves, such as COLOR, NAME, SECOND or POINT",
                ),
        );
        true
    }

    fn declare_procedure(
        &mut self,
        procedure: &Procedure,
    ) {
        if !procedure.module_scope && self.reserved_name(&procedure.name, procedure.span) {
            return;
        }
        if procedure.kind == ProcedureKind::Function
            && !procedure.module_scope
            && procedure.name.to_ascii_uppercase().starts_with("FN")
        {
            self.err(
                procedure.span,
                if procedure.name.eq_ignore_ascii_case("FN") {
                    "Function name illegal"
                } else {
                    "Name of subprogram illegal"
                },
            );
        }
        // `SUB p ()` is VBDOS's; QB 4.5 and PDS take `SUB p`
        if procedure.empty_list
            && !procedure.declaration
            && !procedure.module_scope
            && !matches!(self.dialect, Dialect::VbDos | Dialect::Quickr)
        {
            self.err(procedure.span, "Formal parameter specification illegal");
        }
        // the parameter list: names unique, types known, no fixed-length
        // strings, BYVAL only where the family has it
        let mut seen: Vec<String> = Vec::new();
        for parameter in &procedure.parameters {
            let declaration = &parameter.declaration;
            let upper = declaration.name.to_ascii_uppercase();
            if seen.contains(&upper) {
                self.err(procedure.span, "Formal parameters not unique");
            }
            seen.push(upper);
            if declaration.fixed_length.is_some() {
                self.err(procedure.span, "Fixed-length string illegal");
            }
            match &declaration.type_name {
                Some(TypeName::Named(name)) if !procedure.declaration || name != "ANY" => {
                    if !self.records.contains_key(name) && !self.builtin_object(name) {
                        self.err(procedure.span, "TYPE not defined");
                    }
                }
                _ => {}
            }
            if parameter.by_value
                && !procedure.declaration
                && matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45)
            {
                self.err(procedure.span, "Formal parameter specification illegal");
            }
        }
        // an AS clause after FUNCTION's parameters is VBDOS's
        if procedure.kind == ProcedureKind::Function
            && !procedure.module_scope
            && procedure.result.is_some()
            && Ty::of_suffix(&procedure.name).is_none()
            && !matches!(self.dialect, Dialect::VbDos | Dialect::Quickr)
        {
            self.err_because(
                procedure.span,
                "Syntax error",
                Reason::headline("`{name}` gives its result type with AS, which this dialect does not allow")
                    .help("name the type with a type character, as in `name%` or `name$`; AS results need VBDOS"),
            );
        }
        let params: Vec<ParamInfo> = procedure
            .parameters
            .iter()
            .map(|parameter| {
                let declaration = &parameter.declaration;
                let any = matches!(
                    &declaration.type_name,
                    Some(TypeName::Named(name)) if name == "ANY"
                );
                let ty = declaration
                    .type_name
                    .as_ref()
                    .and_then(|name| self.type_of_name(name))
                    .unwrap_or_else(|| self.default_type(&declaration.name));
                ParamInfo { ty, array: declaration.array, any, by_value: parameter.by_value }
            })
            .collect();
        let result = match procedure.kind {
            ProcedureKind::Function => Some(
                procedure
                    .result
                    .as_ref()
                    .and_then(|name| self.type_of_name(name))
                    .unwrap_or_else(|| self.default_type(&procedure.name)),
            ),
            ProcedureKind::Sub => None,
        };
        let def_fn = procedure.module_scope;
        let key = if def_fn { procedure.name.clone() } else { stem(&procedure.name).to_owned() };
        let mut info = ProcInfo {
            kind: procedure.kind,
            params,
            result,
            defined: !procedure.declaration,
            declared: procedure.declaration,
            def_fn,
            unchecked: procedure.unchecked,
            first_line: procedure.span.line,
        };
        let previous = self.procs.get(&key).cloned();
        match previous {
            Some(existing) if existing.defined && info.defined => self.err_because(
                procedure.span,
                "Duplicate definition",
                Reason::headline("`{name}` is defined twice")
                    .help("give the second definition another name, or remove it"),
            ),
            Some(existing)
                if existing.kind != info.kind
                    || (!existing.unchecked && !info.unchecked && existing.params.len() != info.params.len())
                    || (existing.unchecked
                        && existing.declared
                        && !existing.defined
                        && info.defined
                        && matches!(
                            self.dialect,
                            Dialect::QuickBasic45 | Dialect::Pds71 | Dialect::VbDos
                        )) =>
            {
                let headline = if existing.kind != info.kind {
                    "`{name}` is declared as one kind of procedure (SUB or FUNCTION) and defined as the other"
                        .to_owned()
                } else if existing.unchecked {
                    // all three BCs: a DECLARE with no parentheses matches no
                    // definition, not even one without
                    // parameters
                    "`{name}` is declared without a parameter list, which BC never matches to its definition".to_owned()
                } else {
                    let count = |n: usize| match n {
                        0 => "no parameters".to_owned(),
                        1 => "1 parameter".to_owned(),
                        n => format!("{n} parameters"),
                    };
                    format!(
                        "`{{name}}` is declared with {} but defined with {}",
                        count(existing.params.len()),
                        count(info.params.len())
                    )
                };
                self.err_because(
                    procedure.span,
                    "Argument-count mismatch",
                    Reason::headline(headline).help("write the DECLARE with its parentheses, `DECLARE SUB name ()` when it takes nothing, and make it agree with the definition"),
                )
            }
            Some(existing)
                if existing.kind == ProcedureKind::Function
                    && matches!(
                        (&existing.result, &info.result),
                        (Some(a), Some(b)) if a.plain() != b.plain()
                    ) =>
            {
                self.err_because(
                    procedure.span,
                    "Duplicate definition",
                    Reason::headline("`{name}` is declared with one result type and defined with another").help(
                        "give the DECLARE and the FUNCTION the same type character (`f%` both times, or `AS INTEGER`)",
                    ),
                );
            }
            Some(existing) => {
                if !existing.unchecked
                    && !info.unchecked
                    && existing
                        .params
                        .iter()
                        .zip(&info.params)
                        .any(|(a, b)| !a.any && !b.any && (a.ty.plain() != b.ty.plain() || a.array != b.array))
                {
                    self.err_because(
                        procedure.span,
                        if info.kind == ProcedureKind::Function {
                            "Duplicate definition"
                        } else {
                            "Parameter type mismatch"
                        },
                        Reason::headline(
                            "the parameters of `{name}` differ from its DECLARE: a parameter has another type",
                        )
                        .help("make the DECLARE and the definition list the same types"),
                    );
                }
                info.declared |= existing.declared;
                info.defined |= existing.defined;
                info.unchecked |= existing.unchecked;
                info.first_line = info.first_line.min(existing.first_line);
                if existing.unchecked && !info.unchecked {
                    info.params = existing.params.clone();
                }
            }
            None => {}
        }
        self.procs.insert(key, info);
    }

    /// The type a name stands for, spelled: its suffix or its DEFtype's.
    fn type_key(
        &self,
        name: &str,
    ) -> String {
        format!("{:?}", Ty::of_suffix(name).unwrap_or_else(|| self.default_type(name)))
    }

    fn default_type(
        &self,
        name: &str,
    ) -> Ty {
        if let Some(ty) = Ty::of_suffix(name) {
            return ty;
        }
        let first = name.bytes().next().map_or(b'A', |b| b.to_ascii_uppercase());
        if first.is_ascii_uppercase() { self.def_types[usize::from(first - b'A')].clone() } else { Ty::Single }
    }

    // ---- scopes
    // --------------------------------------------------------------------------------

    /// The variable `name` names in the current scope: found, or implicitly
    /// created.
    fn variable(
        &mut self,
        name: &str,
        array: bool,
        span: Span,
    ) -> Option<Var> {
        if self.reserved_name(name, span) {
            return None;
        }
        // a scalar cannot share a procedure's name
        if !array
            && self.procs.get(stem(name)).is_some_and(|p| !p.def_fn)
            && !self.procedure.as_ref().is_some_and(|(_, own)| stem(own) == stem(name))
        {
            self.err(span, "Syntax error");
            return None;
        }
        let key = stem(name).to_owned();
        let suffix = Ty::of_suffix(name);
        let in_procedure = self.procedure.is_some();
        let shared_from_module = |scope: &Scope| scope.vars.get(&key).cloned();
        let mut candidates: Vec<Var> = self.scope.vars.get(&key).cloned().unwrap_or_default();
        if self.procedure.is_some() {
            if let Some(shared) = self.module_scope.vars.get(&key) {
                candidates.extend(shared.iter().filter(|v| v.shared && !v.rank.is_some() == !array).cloned());
            }
        }
        let _ = shared_from_module;
        let _ = in_procedure;
        // a name bound with AS answers only to its own type
        if let Some(bound) = candidates.iter().find(|v| v.declared && v.rank.is_some() == array) {
            if suffix.as_ref().is_some_and(|s| *s != bound.ty.plain()) {
                self.err_because(
                    span,
                    "Duplicate definition",
                    Reason::headline("`{text}` is bound to another type by AS, so a different type character names a different variable")
                        .help("write the name the way it was declared"),
                );
                return None;
            }
            return Some(bound.clone());
        }
        let ty = suffix.clone().unwrap_or_else(|| self.default_type(name));
        if let Some(found) = candidates.iter().find(|v| v.ty.plain() == ty.plain() && v.rank.is_some() == array) {
            return Some(found.clone());
        }
        if array {
            // an array must have been dimensioned
            self.err(span, "Array not dimensioned");
            return None;
        }
        let var = Var {
            ty,
            rank: None,
            declared: false,
            shared: false,
            dynamic: false,
            explicit: self.declaring,
            suffixed: suffix.is_some(),
            redimmed: false,
        };
        self.scope.vars.entry(key).or_default().push(var.clone());
        Some(var)
    }

    // ---- procedures
    // ---------------------------------------------------------------------------------

    fn procedure_body(
        &mut self,
        procedure: &Procedure,
    ) {
        let saved_scope = std::mem::take(&mut self.scope);
        let saved_labels = std::mem::take(&mut self.labels);
        let saved_weak = std::mem::take(&mut self.weak_labels);
        // a CONST in a procedure is local to it
        let saved_consts = self.consts.clone();
        self.procedure = Some((procedure.kind, procedure.name.clone()));
        self.static_scope = procedure.is_static;
        if procedure.module_scope {
            // DEF FN sees the module's variables
            self.scope = self.module_scope.clone();
        }
        for parameter in &procedure.parameters {
            self.declare_parameter(parameter);
        }
        self.collect_labels(&procedure.body);
        for statement in &procedure.body.clone() {
            self.statement(statement);
        }
        self.procedure = None;
        self.consts = saved_consts;
        self.scope = saved_scope;
        self.labels = saved_labels;
        self.weak_labels = saved_weak;
    }

    fn declare_parameter(
        &mut self,
        parameter: &Parameter,
    ) {
        let declaration = &parameter.declaration;
        if self.reserved_name(&declaration.name, declaration.span) {
            return;
        }
        let ty = declaration
            .type_name
            .as_ref()
            .and_then(|name| self.type_of_name(name))
            .unwrap_or_else(|| self.default_type(&declaration.name));
        let var = Var {
            ty,
            rank: declaration.array.then_some(usize::MAX),
            declared: declaration.type_name.is_some() && Ty::of_suffix(&declaration.name).is_none(),
            shared: false,
            dynamic: true,
            explicit: true,
            suffixed: Ty::of_suffix(&declaration.name).is_some(),
            redimmed: false,
        };
        self.scope.vars.entry(stem(&declaration.name).to_owned()).or_default().push(var);
    }
}

/// The value of a real literal's text (`D` and `E` both mark the exponent).
fn parse_real(text: &str) -> Option<f64> {
    text.replace(['D', 'd'], "E").parse().ok()
}

/// A name without its type suffix, upper case.
fn stem(name: &str) -> &str {
    name.trim_end_matches(['%', '&', '!', '#', '$', '@'])
}

/// The labels a statement list defines, nested bodies included.
fn collect_label_names(
    statements: &[Statement],
    out: &mut HashSet<String>,
) {
    for statement in statements {
        if let Statement::Label(name, _) = statement {
            out.insert(name.clone());
        }
        for body in statement.bodies() {
            collect_label_names(body, out);
        }
    }
}

/// The names an expression reads.
fn free_names(
    expr: &Expr,
    out: &mut Vec<String>,
) {
    match expr {
        Expr::Name(name, _) => out.push(name.clone()),
        Expr::Apply { name, arguments, .. } => {
            out.push(name.clone());
            arguments.iter().for_each(|one| free_names(one, out));
        }
        Expr::Unary { operand, .. } => free_names(operand, out),
        Expr::Binary { left, right, .. } => {
            free_names(left, out);
            free_names(right, out);
        }
        _ => {}
    }
}

fn dotted(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name, _) => Some(name.clone()),
        Expr::Field { base, name, .. } => Some(format!("{}.{name}", dotted(base)?)),
        _ => None,
    }
}
