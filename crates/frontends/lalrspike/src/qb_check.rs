//! The validity check: what BC refuses after the syntax is right.
//!
//! A symbol table over the typed syntax (`syntax::Module`): types and constants, variables by
//! the name's stem and type, procedures and their signatures, labels. It types every expression by
//! BC's rules and checks each statement's arguments against its signature. Diagnostics carry
//! BC's own wording where BC's is known; what matters, and what the sweeps against BC compare,
//! is whether a program has any.

use std::collections::{HashMap, HashSet};

use qbfront::syntax::{
    Binary, CaseItem, Declaration, Expr, Literal, Module, Parameter, PrintKind, Procedure, ProcedureKind, ResumeTarget,
    Span, Statement, TypeName, Unary,
};
use qbfront::Dialect;

use crate::qb_sigs::{self, Class, Ret};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub span: Span,
    pub message: String,
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
    /// A property of a VBDOS form or control, whose type the form's file would say.
    Variant,
    /// A string; a fixed one has its length.
    Str(Option<usize>),
    Record(String),
}

impl Ty {
    fn numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Long | Ty::Currency | Ty::Single | Ty::Double | Ty::Variant)
    }

    fn string(&self) -> bool {
        matches!(self, Ty::Str(_) | Ty::Variant)
    }

    fn rank(&self) -> u8 {
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
    fn plain(&self) -> Ty {
        match self {
            Ty::Str(_) => Ty::Str(None),
            other => other.clone(),
        }
    }

    fn of_suffix(name: &str) -> Option<Ty> {
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

fn promote(a: &Ty, b: &Ty) -> Ty {
    if a.rank() >= b.rank() { a.clone() } else { b.clone() }
}

#[derive(Clone, Debug)]
struct Var {
    ty: Ty,
    /// An array's rank; scalars have none.
    rank: Option<usize>,
    /// Declared with AS: its stem is bound to this type.
    declared: bool,
    /// DIM SHARED: procedures see it.
    shared: bool,
    /// Dimensioned as a dynamic array (REDIM, or under $DYNAMIC): REDIM may resize it.
    dynamic: bool,
    /// Named by a declaration (DIM, a parameter, SHARED), not made by a first use.
    explicit: bool,
    /// The name carried a type character.
    suffixed: bool,
    /// The array was made by a REDIM, so a later DIM is not a second dimensioning.
    redimmed: bool,
}

#[derive(Clone, Debug)]
struct ParamInfo {
    ty: Ty,
    array: bool,
    /// A parameter written AS ANY.
    any: bool,
    by_value: bool,
}

#[derive(Clone, Debug)]
struct ProcInfo {
    kind: ProcedureKind,
    params: Vec<ParamInfo>,
    result: Option<Ty>,
    defined: bool,
    declared: bool,
    def_fn: bool,
    /// `DECLARE SUB name` with no parameter list: its calls are not checked.
    unchecked: bool,
    /// The line where the name first appears (a DECLARE or the definition): calls before it are unchecked.
    first_line: usize,
}

#[derive(Clone, Debug, Default)]
struct Scope {
    /// by the name's stem (suffix removed), upper case
    vars: HashMap<String, Vec<Var>>,
}

#[derive(Clone, Debug)]
struct Record {
    fields: Vec<(String, Ty, Option<usize>)>,
    /// VBDOS's FORM and CONTROL: any property name goes.
    opaque: bool,
}

/// What an expression is: its type, and whether it names a variable that can be assigned or
/// passed by reference.
#[derive(Clone, Debug)]
struct Typed {
    ty: Ty,
    variable: bool,
    constant: Option<f64>,
    /// An array named without subscripts (`a()`), for whole-array arguments.
    array: Option<usize>,
}

impl Typed {
    fn value(ty: Ty) -> Self {
        Self { ty, variable: false, constant: None, array: None }
    }
}

pub fn check(module: &Module, dialect: Dialect) -> Vec<Diagnostic> {
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
    /// The procedure is STATIC, or a STATIC statement is being declared: its arrays are fixed.
    static_scope: bool,
    /// An OPTION BASE has been read.
    option_base_seen: bool,
    /// The target of an assignment is being read.
    assigning: bool,
    /// The counters of the FOR loops being read.
    for_counters: Vec<String>,
    /// CONSTs and TYPEs registered ahead of the statements that declare them (`const N`, `type N`).
    pre_done: HashSet<String>,
    labels: HashSet<String>,
    /// The module level's labels, kept while a procedure's are the ones in use.
    module_labels: HashSet<String>,
    option_base: i64,
    span: Span,
    in_loop: Vec<&'static str>,
}

impl Checker {
    fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            errors: Vec::new(),
            def_types: std::array::from_fn(|_| Ty::Single),
            records: HashMap::new(),
            consts: HashMap::new(),
            procs: HashMap::new(),
            module_scope: Scope::default(),
            scope: Scope::default(),
            procedure: None,
            static_scope: false,
            option_base_seen: false,
            assigning: false,
            for_counters: Vec::new(),
            pre_done: HashSet::new(),
            labels: HashSet::new(),
            module_labels: HashSet::new(),
            option_base: 0,
            span: Span { line: 1, start: 0, end: 0 },
            in_loop: Vec::new(),
        }
    }

    fn err(&mut self, span: Span, message: &str) {
        self.errors.push(Diagnostic { span, message: message.into() });
    }

    // ---- the module ----------------------------------------------------------------------------

    fn module(&mut self, module: &Module) {
        // A procedure's signature depends on what precedes it: DEFtype ranges, CONSTs, record types. They are
        // read in order here, on a scratch state that the statement pass starts over from.
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
            .find(|statement| !matches!(statement, Statement::Dim(_) | Statement::Const { .. } | Statement::TypeDecl { .. } | Statement::DefType { .. } | Statement::Comment(_) | Statement::OptionBase(..) | Statement::OptionExplicit(_) | Statement::Common { .. } | Statement::Static(_) | Statement::Shared(_) | Statement::Label(..) | Statement::FormDeclaration(..)))
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

    fn collect_labels(&mut self, statements: &[Statement]) {
        for statement in statements {
            if let Statement::Label(name, span) = statement {
                // a label in a procedure may not repeat one of the module's
                if self.procedure.is_some() && self.module_labels.contains(name) {
                    self.err(*span, "Duplicate label");
                }
                if !self.labels.insert(name.clone()) {
                    self.err(*span, "Duplicate label");
                }
            }
            for body in statement.bodies() {
                self.collect_labels(body);
            }
        }
    }

    fn type_of_name(&self, name: &TypeName) -> Option<Ty> {
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
    fn builtin_object(&self, name: &str) -> bool {
        matches!(self.dialect, Dialect::VbDos | Dialect::Quickr) && matches!(name, "FORM" | "CONTROL")
    }

    /// A word BC reserves cannot name anything; dotted names are checked as the variables they are.
    fn reserved_name(&mut self, name: &str, span: Span) -> bool {
        // DATA, REM, TRON and TROFF swallow the rest of their line; a suffixed spelling is no use of the word
        let word_with_suffix = Ty::of_suffix(name).is_some() && matches!(stem(name).to_ascii_uppercase().as_str(), "DATA" | "REM" | "TRON" | "TROFF");
        if name.contains('.') || word_with_suffix || !crate::qb_reserved::is_reserved_name(self.dialect, name) {
            return false;
        }
        self.err(span, "Identifier expected");
        true
    }

    fn declare_procedure(&mut self, procedure: &Procedure) {
        if !procedure.module_scope && self.reserved_name(&procedure.name, procedure.span) {
            return;
        }
        // `SUB p ()` is VBDOS's; QB 4.5 and PDS take `SUB p`
        if procedure.empty_list && !procedure.declaration && !procedure.module_scope && !matches!(self.dialect, Dialect::VbDos | Dialect::Quickr) {
            self.err(procedure.span, "Formal parameter specification illegal");
        }
        // the parameter list: names unique, types known, no fixed-length strings, BYVAL only where the family has it
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
            if parameter.by_value && !procedure.declaration && matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45) {
                self.err(procedure.span, "Formal parameter specification illegal");
            }
        }
        // an AS clause after FUNCTION's parameters is VBDOS's
        if procedure.kind == ProcedureKind::Function && !procedure.module_scope && procedure.result.is_some() && Ty::of_suffix(&procedure.name).is_none() && !matches!(self.dialect, Dialect::VbDos | Dialect::Quickr) {
            self.err(procedure.span, "Syntax error");
        }
        let params: Vec<ParamInfo> = procedure
            .parameters
            .iter()
            .map(|parameter| {
                let declaration = &parameter.declaration;
                let any = matches!(&declaration.type_name, Some(TypeName::Named(name)) if name == "ANY");
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
            Some(existing) if existing.defined && info.defined => self.err(procedure.span, "Duplicate definition"),
            Some(existing) if existing.kind != info.kind || (!existing.unchecked && !info.unchecked && existing.params.len() != info.params.len()) => {
                self.err(procedure.span, "Argument-count mismatch")
            }
            Some(existing) => {
                if !existing.unchecked && !info.unchecked && existing.params.iter().zip(&info.params).any(|(a, b)| !a.any && !b.any && (a.ty.plain() != b.ty.plain() || a.array != b.array)) {
                    self.err(procedure.span, if info.kind == ProcedureKind::Function { "Duplicate definition" } else { "Parameter type mismatch" });
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
    fn type_key(&self, name: &str) -> String {
        format!("{:?}", Ty::of_suffix(name).unwrap_or_else(|| self.default_type(name)))
    }

    fn default_type(&self, name: &str) -> Ty {
        if let Some(ty) = Ty::of_suffix(name) {
            return ty;
        }
        let first = name.bytes().next().map_or(b'A', |b| b.to_ascii_uppercase());
        if first.is_ascii_uppercase() { self.def_types[usize::from(first - b'A')].clone() } else { Ty::Single }
    }

    // ---- scopes --------------------------------------------------------------------------------

    /// The variable `name` names in the current scope: found, or implicitly created.
    fn variable(&mut self, name: &str, array: bool, span: Span) -> Option<Var> {
        if self.reserved_name(name, span) {
            return None;
        }
        // a scalar cannot share a procedure's name
        if !array && self.procs.get(stem(name)).is_some_and(|p| !p.def_fn) && !self.procedure.as_ref().is_some_and(|(_, own)| stem(own) == stem(name)) {
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
                self.err(span, "Duplicate definition");
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
        let var = Var { ty, rank: None, declared: false, shared: false, dynamic: false, explicit: false, suffixed: suffix.is_some(), redimmed: false };
        self.scope.vars.entry(key).or_default().push(var.clone());
        Some(var)
    }

    // ---- procedures ---------------------------------------------------------------------------------

    fn procedure_body(&mut self, procedure: &Procedure) {
        let saved_scope = std::mem::take(&mut self.scope);
        let saved_labels = std::mem::take(&mut self.labels);
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
        self.scope = saved_scope;
        self.labels = saved_labels;
    }

    fn declare_parameter(&mut self, parameter: &Parameter) {
        let declaration = &parameter.declaration;
        if self.reserved_name(&declaration.name, declaration.span) {
            return;
        }
        let ty = declaration
            .type_name
            .as_ref()
            .and_then(|name| self.type_of_name(name))
            .unwrap_or_else(|| self.default_type(&declaration.name));
        let var = Var { ty, rank: declaration.array.then_some(usize::MAX), declared: declaration.type_name.is_some() && Ty::of_suffix(&declaration.name).is_none(), shared: false, dynamic: true, explicit: true, suffixed: Ty::of_suffix(&declaration.name).is_some(), redimmed: false };
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

// ---- expressions ---------------------------------------------------------------------------------------

fn dotted(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name, _) => Some(name.clone()),
        Expr::Field { base, name, .. } => Some(format!("{}.{name}", dotted(base)?)),
        _ => None,
    }
}

impl Checker {
    fn scalar(&mut self, typed: Option<Typed>, span: Span) -> Option<Typed> {
        let typed = typed?;
        if matches!(typed.ty, Ty::Record(_)) {
            self.err(span, "Typed variable not allowed in expression");
            return None;
        }
        if typed.array.is_some() {
            self.err(span, "Syntax error");
            return None;
        }
        Some(typed)
    }

    /// An expression that must be numeric, and a 16-bit integer where it is a constant.
    fn integer(&mut self, expr: &Expr) -> Option<Typed> {
        self.argument_of_class(Class::Int, expr)
    }

    /// An expression that must be numeric.
    fn numeric(&mut self, expr: &Expr) -> Option<Typed> {
        let typed = self.expr(expr);
        let typed = self.scalar(typed, expr.span())?;
        if !typed.ty.numeric() {
            self.err(expr.span(), "Type mismatch");
            return None;
        }
        Some(typed)
    }

    fn string_expr(&mut self, expr: &Expr) -> Option<Typed> {
        let typed = self.expr(expr);
        let typed = self.scalar(typed, expr.span())?;
        if !typed.ty.string() {
            self.err(expr.span(), "String expression required");
            return None;
        }
        Some(typed)
    }

    fn expr(&mut self, expr: &Expr) -> Option<Typed> {
        match expr {
            Expr::Omitted(span) => {
                self.err(*span, "Syntax error");
                None
            }
            Expr::Literal(literal, _) => Some(match literal {
                Literal::Integer(value, TypeName::Long) => Typed { constant: Some(*value as f64), ..Typed::value(Ty::Long) },
                Literal::Integer(value, _) => Typed { constant: Some(*value as f64), ..Typed::value(Ty::Int) },
                Literal::Real(text, TypeName::Currency) => Typed { constant: parse_real(text), ..Typed::value(Ty::Currency) },
                Literal::Real(text, TypeName::Double) => {
                    Typed { constant: parse_real(text), ..Typed::value(Ty::Double) }
                }
                Literal::Real(text, _) => {
                    let constant = parse_real(text);
                    if constant.is_some_and(|c| c.abs() > f64::from(f32::MAX)) {
                        self.err(expr.span(), "Math overflow");
                    }
                    Typed { constant, ..Typed::value(Ty::Single) }
                }
                Literal::String(_) => Typed::value(Ty::Str(None)),
            }),
            Expr::Name(name, span) => self.name_expr(name, *span),
            Expr::Apply { name, arguments, span } => self.apply(name, arguments, *span),
            // `Math.Add(1, 2)`: a procedure or array whose name has a dot is not a field of anything
            Expr::Index { base, indices, span } if dotted(base).is_some_and(|name| !self.lookup_record_var(name.split('.').next().unwrap_or(""))) => {
                let name = dotted(base).unwrap_or_default();
                self.apply(&name, indices, *span)
            }
            Expr::Index { base, indices, span } => {
                let base_typed = self.expr(base)?;
                // an indexed property of a VBDOS form or control
                if base_typed.ty == Ty::Variant {
                    for index in indices {
                        self.expr(index);
                    }
                    return Some(Typed { ty: Ty::Variant, variable: true, constant: None, array: None });
                }
                self.err(*span, "Syntax error");
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
                    self.err(*span, "Type mismatch");
                    return None;
                }
                let ty = match op {
                    Unary::Not => if typed.ty == Ty::Long { Ty::Long } else if typed.ty.rank() > 2 { Ty::Long.min_int(&typed.ty) } else { Ty::Int },
                    _ => typed.ty.clone(),
                };
                let constant = typed.constant.map(|c| match op {
                    Unary::Negative => -c,
                    Unary::Not => !(c as i64) as f64,
                    _ => c,
                });
                Some(Typed { ty, variable: false, constant, array: None })
            }
            Expr::Binary { op, left, right, span } => {
                let l = self.expr(left);
                let r = self.expr(right);
                let l = self.scalar(l, left.span());
                let r = self.scalar(r, right.span());
                let (l, r) = (l?, r?);
                self.binary(*op, &l, &r, right, *span)
            }
            _ => {
                self.err(expr.span(), "Syntax error");
                None
            }
        }
    }

    fn binary(&mut self, op: Binary, l: &Typed, r: &Typed, right: &Expr, span: Span) -> Option<Typed> {
        use Binary::*;
        // a property of a form or control may be either a number or text
        if l.ty == Ty::Variant || r.ty == Ty::Variant {
            return Some(Typed::value(if matches!(op, Eq | NotEqual | Less | LessEqual | Greater | GreaterEqual) { Ty::Int } else { Ty::Variant }));
        }
        let both_strings = l.ty.string() && r.ty.string();
        match op {
            Add if both_strings => return Some(Typed::value(Ty::Str(None))),
            Eq | NotEqual | Less | LessEqual | Greater | GreaterEqual if both_strings => return Some(Typed::value(Ty::Int)),
            _ => {}
        }
        if !(l.ty.numeric() && r.ty.numeric()) {
            self.err(span, "Type mismatch");
            return None;
        }
        let promoted = promote(&l.ty, &r.ty);
        let fold = |f: &dyn Fn(f64, f64) -> f64| match (l.constant, r.constant) {
            (Some(a), Some(b)) => Some(f(a, b)),
            _ => None,
        };
        let zero_divisor = matches!(r.constant, Some(c) if c == 0.0) && matches!(right, Expr::Literal(..) | Expr::Unary { .. } | Expr::Binary { .. } | Expr::Name(..));
        Some(match op {
            Add => Typed { constant: fold(&|a, b| a + b), ..Typed::value(promoted) },
            Subtract => Typed { constant: fold(&|a, b| a - b), ..Typed::value(promoted) },
            Multiply => Typed { constant: fold(&|a, b| a * b), ..Typed::value(promoted) },
            Divide => {
                if zero_divisor {
                    self.err(span, "Division by zero");
                    return None;
                }
                let ty = if promoted == Ty::Double { Ty::Double } else { Ty::Single };
                Typed { constant: fold(&|a, b| a / b), ..Typed::value(ty) }
            }
            IntegerDivide | Modulo => {
                if zero_divisor {
                    self.err(span, "Division by zero");
                    return None;
                }
                let ty = if l.ty == Ty::Long || r.ty == Ty::Long || l.ty.rank() > 2 || r.ty.rank() > 2 { Ty::Long } else { Ty::Int };
                Typed::value(ty)
            }
            Power => Typed::value(Ty::Double),
            Eq | NotEqual | Less | LessEqual | Greater | GreaterEqual => Typed::value(Ty::Int),
            And | Or | Xor | Eqv | Imp => {
                let ty = if l.ty == Ty::Long || r.ty == Ty::Long || l.ty.rank() > 2 || r.ty.rank() > 2 { Ty::Long } else { Ty::Int };
                Typed::value(ty)
            }
        })
    }

    fn field(&mut self, whole: &Expr, base: &Expr, name: &str, span: Span) -> Option<Typed> {
        // a dotted name that no record variable owns is one variable's name
        if let Some(chain) = dotted(whole) {
            let head = dotted(base).unwrap_or_default();
            let head_is_record = self.lookup_record_var(head.split('.').next().unwrap_or(""));
            if !head_is_record {
                let var = self.variable(&chain, false, span)?;
                return Some(Typed { ty: var.ty, variable: true, constant: None, array: None });
            }
        }
        let base_typed = self.expr(base)?;
        // a property of a property of a form: nothing here says its type
        if base_typed.ty == Ty::Variant {
            return Some(Typed { ty: Ty::Variant, variable: true, constant: None, array: None });
        }
        let Ty::Record(record) = &base_typed.ty else {
            self.err(span, "Type mismatch");
            return None;
        };
        let record = self.records.get(record).cloned().unwrap_or(Record { fields: Vec::new(), opaque: true });
        if record.opaque {
            return Some(Typed { ty: Ty::Variant, variable: true, constant: None, array: None });
        }
        // a field may be named with its type character (`menu.text$`)
        let wanted = Ty::of_suffix(name);
        match record.fields.iter().find(|(field, ty, _)| stem(field).eq_ignore_ascii_case(stem(name)) && wanted.as_ref().is_none_or(|w| ty.plain() == *w)) {
            Some((_, ty, _)) => Some(Typed { ty: ty.clone(), variable: true, constant: None, array: None }),
            None => {
                self.err(span, "Element not defined");
                None
            }
        }
    }

    fn lookup_record_var(&self, name: &str) -> bool {
        let key = stem(name);
        let in_scope = |scope: &Scope| scope.vars.get(key).is_some_and(|vars| vars.iter().any(|v| matches!(v.ty, Ty::Record(_))));
        in_scope(&self.scope) || in_scope(&self.module_scope)
    }

    fn name_expr(&mut self, name: &str, span: Span) -> Option<Typed> {
        let key = stem(name);
        if let Some(constant) = self.consts.get(key) {
            // `CONST x = 1` then `x$`: the suffix names a string where the constant is a number
            if Ty::of_suffix(name).is_some_and(|suffix| suffix.string() != constant.ty.string()) {
                self.err(span, "Type mismatch");
                return None;
            }
            return Some(constant.clone());
        }
        if let Some(sig) = qb_sigs::function(name, self.dialect) {
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
                self.err(span, "Argument-count mismatch");
                return None;
            }
        }
        // a FUNCTION's own name reads its result
        if self.procedure.as_ref().is_some_and(|(kind, own)| *kind == ProcedureKind::Function && stem(own) == key) {
            let ty = self.procs.get(key).and_then(|p| p.result.clone()).unwrap_or(Ty::Single);
            return Some(Typed { ty, variable: true, constant: None, array: None });
        }
        let var = self.variable(name, false, span)?;
        Some(Typed { ty: var.ty, variable: true, constant: None, array: None })
    }

    fn result_type(&self, ret: Ret, args: &[Typed]) -> Ty {
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

    fn apply(&mut self, name: &str, arguments: &[Expr], span: Span) -> Option<Typed> {
        let key = stem(name);
        if name == crate::qb_ast::ON_SELECTOR {
            for argument in arguments {
                self.integer(argument);
            }
            return Some(Typed::value(Ty::Int));
        }
        // an array element
        let array = self.array_var(name);
        if let Some(var) = array {
            // each subscript that is not a constant holds a temporary: past six when read, seven when assigned, BC gives up
            let variable_subscripts = arguments.iter().filter(|index| !matches!(index, Expr::Literal(..))).count();
            if variable_subscripts > if self.assigning { 7 } else { 6 } {
                self.err(span, "Expression too complex");
                return None;
            }
            let rank = var.rank.unwrap_or(1);
            let mut ok = true;
            for index in arguments {
                ok &= self.numeric(index).is_some();
            }
            if ok && var.rank.is_some() && rank != arguments.len() && rank != usize::MAX {
                self.err(span, "Wrong number of dimensions");
                return None;
            }
            return ok.then_some(Typed { ty: var.ty, variable: true, constant: None, array: None });
        }
        if let Some(sig) = qb_sigs::function(name, self.dialect) {
            return self.intrinsic(sig, name, arguments, span);
        }
        // `SCREEN.ControlPanel(i)`, `frm.List(i)`: an indexed property of a VBDOS object
        if let Some((root, _)) = name.split_once('.') {
            if matches!(self.dialect, Dialect::VbDos | Dialect::Quickr) && (matches!(root.to_ascii_uppercase().as_str(), "SCREEN" | "PRINTER" | "CLIPBOARD") || self.lookup_record_var(root)) {
                for argument in arguments {
                    self.expr(argument);
                }
                return Some(Typed { ty: Ty::Variant, variable: true, constant: None, array: None });
            }
        }
        if let Some(procedure) = self.procs.get(key).or_else(|| self.procs.get(name)).cloned() {
            if procedure.kind == ProcedureKind::Function && span.line < procedure.first_line {
                // above its DECLARE or definition the name is an array that was never dimensioned
                self.err(span, if procedure.def_fn { "Function not defined" } else { "Array not dimensioned" });
                return None;
            }
            if procedure.kind == ProcedureKind::Function {
                self.check_arguments(&procedure, arguments, span, false);
                return procedure.result.map(Typed::value);
            }
            self.err(span, "Type mismatch");
            return None;
        }
        if qb_sigs::function_unknown_but_named(name) {
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

    fn array_var(&self, name: &str) -> Option<Var> {
        let key = stem(name);
        let suffix = Ty::of_suffix(name);
        let find = |scope: &Scope| {
            scope.vars.get(key).and_then(|vars| {
                vars.iter()
                    .find(|v| {
                        v.rank.is_some()
                            && match &suffix {
                                Some(s) => v.ty.plain() == *s,
                                None => v.declared || v.ty.plain() == self.default_type(name),
                            }
                    })
                    .cloned()
            })
        };
        find(&self.scope).or_else(|| find(&self.module_scope))
    }

    fn intrinsic(&mut self, sig: &'static qb_sigs::FnSig, name: &str, arguments: &[Expr], span: Span) -> Option<Typed> {
        let _ = name;
        let candidates: Vec<&Vec<(Class, bool)>> = sig
            .alternatives
            .iter()
            .filter(|alt| alt.len() >= arguments.len() && alt.iter().take_while(|(_, o)| !*o).count() <= arguments.len())
            .collect();
        if candidates.is_empty() {
            self.err(span, "Argument-count mismatch");
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
                return Some(Typed::value(self.result_type(sig.ret, &typed_args)));
            }
            if n + 1 < candidates.len() {
                self.errors.truncate(mark);
            }
        }
        None
    }

    /// One argument against its class: the typed argument, or an error reported.
    fn argument_of_class(&mut self, class: Class, argument: &Expr) -> Option<Typed> {
        match class {
            Class::Num => self.numeric(argument),
            Class::Int => {
                let typed = self.numeric(argument)?;
                if let (Some(c), false) = (typed.constant, typed.variable) {
                    if !(-32768.0..=32767.0).contains(&c.round()) {
                        self.err(argument.span(), "Math overflow");
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
                if matches!(typed.ty, Ty::Str(Some(_))) {
                    self.err(argument.span(), "Fixed-length string illegal");
                    return None;
                }
                Some(typed)
            }
            Class::ArrayStart => match argument {
                Expr::Apply { name, arguments, span } if !arguments.is_empty() => match self.array_var(name) {
                    Some(var) => Some(Typed { ty: var.ty, variable: true, constant: None, array: Some(var.rank.unwrap_or(1)) }),
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
                        self.err(*span, "Label not defined");
                    }
                    return Some(Typed::value(Ty::Int));
                }
                self.err(argument.span(), "Syntax error");
                None
            }
        }
    }

    /// An array named as an argument: `a()` or a bare name of an array.
    fn array_argument(&mut self, argument: &Expr) -> Option<Typed> {
        let (name, span) = match argument {
            Expr::Name(name, span) => (name.clone(), *span),
            other => {
                self.err(other.span(), if matches!(other, Expr::Apply { arguments, .. } if arguments.is_empty()) { "Syntax error" } else { "Array not defined" });
                return None;
            }
        };
        match self.array_var(&name) {
            Some(var) => Some(Typed { ty: var.ty, variable: true, constant: None, array: Some(var.rank.unwrap_or(1)) }),
            None => {
                self.err(span, "Array not defined");
                None
            }
        }
    }

    fn check_arguments(&mut self, procedure: &ProcInfo, arguments: &[Expr], span: Span, is_call: bool) {
        let _ = is_call;
        // BC reads top down: a call above the first DECLARE or definition has nothing to be checked against
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
                self.expr(inner);
            }
            return;
        }
        if procedure.params.len() != arguments.len() {
            self.err(span, "Argument-count mismatch");
            return;
        }
        for (parameter, argument) in procedure.params.iter().zip(arguments) {
            // BYVAL and SEG at the call mark how the value goes; the value is what is typed
            let argument = match argument {
                Expr::Unary { op: Unary::ByValue | Unary::Segmented, operand, .. } => &**operand,
                other => other,
            };
            if parameter.array {
                match self.array_argument_for(argument) {
                    Some(typed) => {
                        if !parameter.any && typed.ty.plain() != parameter.ty.plain() {
                            self.err(argument.span(), "Parameter type mismatch");
                        }
                    }
                    None => self.err(argument.span(), "Parameter type mismatch"),
                }
                continue;
            }
            let Some(typed) = self.expr(argument) else { continue };
            if typed.array.is_some() {
                self.err(argument.span(), "Parameter type mismatch");
                continue;
            }
            if parameter.any {
                continue;
            }
            let compatible = match (&typed.ty, &parameter.ty) {
                (Ty::Record(a), Ty::Record(b)) => a == b,
                (Ty::Str(_), Ty::Str(_)) => true,
                (a, b) if a.numeric() && b.numeric() => !typed.variable || a == b || parameter.by_value,
                _ => false,
            };
            if !compatible {
                let by_reference = typed.variable && !matches!(argument, Expr::Unary { op: Unary::Grouped, .. });
                let numeric_pair = typed.ty.numeric() && parameter.ty.numeric();
                self.err(argument.span(), if by_reference || numeric_pair { "Parameter type mismatch" } else { "Type mismatch" });
            }
        }
    }

    fn array_argument_for(&mut self, argument: &Expr) -> Option<Typed> {
        let (name, span) = match argument {
            Expr::Apply { name, arguments, span } if arguments.is_empty() => (name.clone(), *span),
            _ => return None,
        };
        let var = self.array_var(&name);
        let _ = span;
        var.map(|var| Typed { ty: var.ty, variable: true, constant: None, array: Some(var.rank.unwrap_or(1)) })
    }
}

impl Ty {
    fn min_int(&self, _other: &Ty) -> Ty {
        Ty::Long
    }
}

// ---- statements ----------------------------------------------------------------------------------------------

impl Checker {
    fn unwrap_step<'a>(&self, expr: &'a Expr) -> &'a Expr {
        match expr {
            Expr::Apply { name, arguments, .. } if name == crate::qb_ast::STEP_MARK && arguments.len() == 1 => &arguments[0],
            other => other,
        }
    }

    /// Arguments against a spec of space-separated classes: n s x v t a l m, a `?` making one optional.
    fn spec(&mut self, arguments: &[Expr], spec: &str, span: Span) -> bool {
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
                    if !self.labels.contains(label) && !self.module_labels.contains(label) {
                        self.err(*at, "Label not defined");
                    }
                } else {
                    self.err(argument.span(), "Syntax error");
                }
                continue;
            }
            if letter == 'k' {
                // a keyword the statement spells (ON, KEY, a device name): nothing to evaluate
                continue;
            }
            let class = match letter {
                'n' => Class::Num,
                'i' => Class::Int,
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
                    if !matches!(argument, Expr::Name(name, _) if matches!(name.as_str(), "AND" | "OR" | "PRESET" | "PSET" | "XOR")) {
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

    fn statement(&mut self, statement: &Statement) {
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
                    self.err(self.span, "Subprogram error");
                }
                // `DIM a()` names no bounds: VBDOS's
                if matches!(statement, Statement::Dim(_)) && !matches!(self.dialect, Dialect::VbDos | Dialect::Quickr) && items.iter().any(|item| item.array && item.bounds.is_empty()) {
                    self.err(self.span, "Syntax error");
                }
                if matches!(statement, Statement::Dim(_)) && self.procedure.is_some() && items.iter().any(|item| item.shared) {
                    self.err(self.span, "Simple or array variable expected");
                }
                let keep = self.static_scope;
                self.static_scope |= matches!(statement, Statement::Static(_));
                for item in items {
                    self.declare(item, false);
                }
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
                    // `COMMON x()` names an array that may already exist, or be dimensioned later
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
                if crate::qb_reserved::is_reserved_name(self.dialect, name) {
                    self.err(*span, "Statement unrecognizable");
                }
            }
            Statement::Comment(_) | Statement::OptionExplicit(_) => {}
            Statement::OptionBase(_, span) => {
                if std::mem::replace(&mut self.option_base_seen, true) {
                    self.err(*span, "Syntax error");
                }
                let dimensioned = |scope: &Scope| scope.vars.values().flatten().any(|v| v.rank.is_some());
                if dimensioned(&self.scope) || dimensioned(&self.module_scope) {
                    self.err(*span, "Array already dimensioned");
                }
            }
            Statement::Goto(label, span) | Statement::CallOrGoto(label, span) => {
                // VBDOS takes `GOTO 0`
                let vbdos_zero = label == "0" && matches!(self.dialect, Dialect::VbDos | Dialect::Quickr);
                if !vbdos_zero && !self.labels.contains(label) {
                    self.err(*span, "Label not defined");
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
                if let (Expr::Name(counter, _), Some((next, span))) = (counter, next_variable) {
                    let same = counter.eq_ignore_ascii_case(next)
                        || (stem(counter).eq_ignore_ascii_case(stem(next)) && (Ty::of_suffix(counter).is_none() || Ty::of_suffix(next).is_none()));
                    if !same {
                        self.err(*span, "NEXT without FOR");
                    }
                }
                let typed = self.expr(counter);
                if let Some(typed) = &typed {
                    if !typed.variable || !typed.ty.numeric() {
                        self.err(counter.span(), "Type mismatch");
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
                use qbfront::syntax::ExitTarget;
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
                // an error handler is module-level code, even when the statement stands in a procedure
                // ON ERROR names a module-level handler; ON LOCAL ERROR one of the procedure
                let known = if self.procedure.is_some() && !*local { self.module_labels.contains(label) } else { self.labels.contains(label) };
                if label != "0" && !known {
                    self.err(*span, "Label not defined");
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
                        self.err(*span, "Label not defined");
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
                        self.err(destination.span(), if typed.variable { "String expression required" } else { "Variable required" });
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
                        if !typed.variable {
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
                    if let Some(typed) = self.expr(destination) {
                        if !typed.variable || matches!(typed.ty, Ty::Record(_)) {
                            self.err(destination.span(), "Variable required");
                        }
                    }
                }
            }
            Statement::Data { .. } => {}
            Statement::Read { destinations, .. } => {
                for destination in destinations {
                    if let Some(typed) = self.expr(destination) {
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
            // '$FORM name: the form is a variable of the form type, seen everywhere
            Statement::FormDeclaration(name, _) => {
                let var = Var { ty: Ty::Record("FORM".into()), rank: None, declared: true, shared: true, dynamic: false, explicit: true, suffixed: false, redimmed: false };
                self.scope.vars.entry(stem(name).to_ascii_uppercase()).or_default().push(var);
            }
        }
    }

    fn print_item(&mut self, kind: PrintKind, value: &Expr) {
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

    fn select(&mut self, selector: &Expr, arms: &[(Vec<CaseItem>, Vec<Statement>)], otherwise: &[Statement]) {
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
                    let case = if integer_selector && matches!(value, Expr::Literal(Literal::Integer(..), _) | Expr::Unary { .. }) { self.integer(value) } else { self.expr(value) };
                    let Some(case) = case else { continue };
                    if let Some(selector) = &typed {
                        if selector.ty.string() != case.ty.string() || matches!(case.ty, Ty::Record(_)) {
                            self.err(value.span(), "Type mismatch");
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

    fn assign(&mut self, target: &Expr, value: &Expr, span: Span) {
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
                        self.err(value.span(), "Type mismatch");
                    }
                }
                return;
            }
        }
        // a FUNCTION assigns its result through its own name
        if let (Expr::Name(name, _), Some((ProcedureKind::Function, own))) = (target, &self.procedure) {
            if stem(name) == stem(own) {
                let ty = self.procs.get(stem(own)).or_else(|| self.procs.get(own.as_str())).and_then(|p| p.result.clone());
                // `f$ = …` in FUNCTION f% names another variable than the result
                if let (Some(suffix), Some(result)) = (Ty::of_suffix(name), &ty) {
                    if suffix.plain() != result.plain() {
                        self.err(span, "Duplicate definition");
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
                self.err(span, "Duplicate definition");
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
            self.err(span, "Syntax error");
            return;
        }
        self.assignable(&t.ty, &v, value.span());
    }

    fn assignable(&mut self, target: &Ty, value: &Typed, span: Span) {
        if value.array.is_some() {
            self.err(span, "Syntax error");
            return;
        }
        let ok = match (target, &value.ty) {
            // a property of a form or control: its file says what it is
            (Ty::Variant, _) | (_, Ty::Variant) => true,
            (Ty::Record(a), Ty::Record(b)) => a == b,
            (Ty::Str(_), Ty::Str(_)) => true,
            (a, b) if a.numeric() && b.numeric() => {
                // a constant that cannot fit; PDS and VBDOS fold only integer literals into integers
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
                        self.err(span, "Math overflow");
                        return;
                    }
                }
                true
            }
            _ => false,
        };
        if !ok {
            self.err(span, "Type mismatch");
        }
    }
}

// ---- declarations, calls, the signature table of statements -----------------------------------------------------

impl Checker {
    fn constant_expr(&mut self, expr: &Expr) -> Option<Typed> {
        match expr {
            Expr::Literal(..) => self.expr(expr),
            Expr::Name(name, span) => match self.consts.get(stem(name)) {
                Some(constant) => Some(constant.clone()),
                None => {
                    self.err(*span, "Invalid constant");
                    None
                }
            },
            // `UGL.8BIT`: a constant whose name has a dot is read as a field access
            Expr::Field { .. } if dotted(expr).is_some_and(|name| self.consts.contains_key(stem(&name))) => {
                dotted(expr).and_then(|name| self.consts.get(stem(&name)).cloned())
            }
            Expr::Unary { op, operand, .. } if !matches!(op, Unary::ByValue | Unary::Segmented) => {
                self.constant_expr(operand)?;
                self.expr(expr)
            }
            Expr::Binary { left, right, .. } => {
                self.constant_expr(left)?;
                self.constant_expr(right)?;
                self.expr(expr)
            }
            other => {
                self.err(other.span(), "Invalid constant");
                None
            }
        }
    }

    fn constant(&mut self, name: &str, value: &Expr, span: Span) {
        if self.reserved_name(name, span) {
            return;
        }
        let Some(typed) = self.constant_expr(value) else { return };
        let key = stem(name).to_owned();
        // arrays have a namespace of their own: a CONST clashes with scalars (and with another CONST)
        let scalar_in_use = self.scope.vars.get(&key).is_some_and(|vars| vars.iter().any(|v| v.rank.is_none()));
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
        self.consts.insert(key, Typed { ty, variable: false, constant: typed.constant, array: None });
    }

    fn type_declaration(&mut self, name: &str, fields: &[Declaration], span: Span) {
        if self.procedure.is_some() {
            self.err(span, "Illegal inside SUB or FUNCTION");
        }
        if crate::qb_reserved::is_reserved_type(self.dialect, name) {
            self.err(span, "Identifier expected");
            return;
        }
        if Ty::of_suffix(name).is_some() {
            self.err(span, "Invalid character");
            return;
        }
        if self.records.contains_key(name) {
            self.err(span, "Duplicate definition");
            return;
        }
        if fields.is_empty() {
            self.err(span, "Element not defined");
        }
        let mut out: Vec<(String, Ty, Option<usize>)> = Vec::new();
        for field in fields {
            // array fields are PDS's and VBDOS's, and need bounds
            if field.array && (field.bounds.is_empty() || matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45)) {
                self.err(field.span, if matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45) { "AS missing" } else { "Syntax error" });
                continue;
            }
            if matches!(field.type_name, Some(TypeName::String)) {
                match &field.fixed_length {
                    None => {
                        self.err(field.span, "Asterisk missing");
                        continue;
                    }
                    Some(length) => {
                        let ok = self.constant_expr(length).and_then(|typed| typed.constant).is_some_and(|c| (1.0..=32767.0).contains(&c));
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
            if crate::qb_reserved::is_reserved_field(self.dialect, &field.name) {
                self.err(field.span, "Identifier expected");
                continue;
            }
            if out.iter().any(|(existing, ..)| *existing == field.name) {
                self.err(field.span, "Duplicate definition");
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
                    ty = Ty::Str(typed.constant.map(|c| c as usize));
                }
            }
            out.push((field.name.clone(), ty, None));
        }
        self.records.insert(name.to_owned(), Record { fields: out, opaque: false });
    }

    fn declare(&mut self, d: &Declaration, redim: bool) {
        let span = d.span;
        if self.reserved_name(&d.name, span) {
            return;
        }
        if let Some((prefix, _)) = d.name.split_once('.') {
            // `r.a` names a field of the record variable `r`; a record variable's dotted names are not free
            let record = |scope: &Scope| scope.vars.get(stem(prefix)).is_some_and(|vars| vars.iter().any(|v| matches!(v.ty, Ty::Record(_))));
            if record(&self.scope) || record(&self.module_scope) {
                self.err(span, "Duplicate definition");
                return;
            }
            if d.type_name.is_none() && matches!(self.dialect, Dialect::Pds71 | Dialect::VbDos | Dialect::Quickr) {
                self.err(span, "AS clause required");
                return;
            }
        }
        let mut ty = match d.type_name.as_ref() {
            Some(name) => match self.type_of_name(name) {
                Some(ty) => ty,
                None => {
                    self.err(span, "User-defined type not defined");
                    return;
                }
            },
            None => self.default_type(&d.name),
        };
        if let (Ty::Str(_), Some(length)) = (&ty, &d.fixed_length) {
            if let Some(typed) = self.constant_expr(length) {
                ty = Ty::Str(typed.constant.map(|c| c as usize));
            }
        }
        if d.bounds.len() > 60 {
            self.err(span, "Too many dimensions");
            return;
        }
        let mut constant_bounds = true;
        for bound in &d.bounds {
            if let Some(lower) = &bound.lower {
                constant_bounds &= self.integer(lower).is_some_and(|typed| typed.constant.is_some());
            }
            constant_bounds &= self.integer(&bound.upper).is_some_and(|typed| typed.constant.is_some());
        }
        let key = stem(&d.name).to_owned();
        let as_declared = d.type_name.is_some() && Ty::of_suffix(&d.name).is_none();
        if !d.array && self.consts.contains_key(&key) {
            self.err(span, "Duplicate definition");
            return;
        }
        let existing: Vec<Var> = self.scope.vars.get(&key).cloned().unwrap_or_default();
        if d.array {
            // an AS-bound array, a suffixed one and a bare one bind the stem as scalars' names do
            if !existing.iter().any(|v| v.rank.is_some() && v.ty.plain() == ty.plain() && v.declared == as_declared) {
                if let Some(message) = self.scalar_conflict(&existing, &ty, &d.name, as_declared, true) {
                    self.err(span, message);
                    return;
                }
            }
            if let Some(old) = existing.iter().find(|v| v.rank.is_some() && v.ty.plain() == ty.plain() && v.declared == as_declared) {
                // `COMMON a() AS T` named the array; its DIM gives the bounds
                if old.rank == Some(usize::MAX) && !d.bounds.is_empty() {
                    if let Some(vars) = self.scope.vars.get_mut(&key) {
                        vars.retain(|v| v.rank != Some(usize::MAX));
                    }
                } else {
                    if (redim && old.dynamic) || (!redim && old.redimmed) {
                        // REDIM resizes a dynamic array
                        if old.ty.plain() != ty.plain() {
                            self.err(span, "Duplicate definition");
                        }
                        return;
                    }
                    self.err(span, "Array already dimensioned");
                    return;
                }
            }
        } else if let Some(message) = self.scalar_conflict(&existing, &ty, &d.name, as_declared, false) {
            self.err(span, message);
            return;
        }
        // a name bound by AS to another type, or a different suffix, is taken
        if !d.array && existing.iter().any(|v| v.declared && v.ty.plain() != ty.plain() && v.rank.is_none()) {
            self.err(span, "Duplicate definition");
            return;
        }
        let var = Var {
            ty,
            rank: d.array.then_some(if d.bounds.is_empty() { usize::MAX } else { d.bounds.len() }),
            declared: as_declared,
            shared: d.shared,
            dynamic: redim
                || d.dynamic
                || (d.array && d.bounds.is_empty())
                // variable bounds, or an automatic array in a procedure that is not STATIC
                || (d.array && !constant_bounds)
                || (d.array && self.procedure.is_some() && !self.static_scope),
            explicit: true,
            suffixed: Ty::of_suffix(&d.name).is_some(),
            redimmed: redim,
        };
        self.scope.vars.entry(key).or_default().push(var);
    }

    /// Why declaring a scalar `name` of type `ty` clashes with the variables already named by its stem (BC's rules:
    /// `q` and `q%` are two variables, `q AS T` binds the stem, and PDS and VBDOS refuse to mix a suffix with AS).
    fn scalar_conflict(&self, existing: &[Var], ty: &Ty, name: &str, as_declared: bool, arrays: bool) -> Option<&'static str> {
        let suffixed = Ty::of_suffix(name).is_some();
        let mixes = !matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45);
        for v in existing.iter().filter(|v| v.rank.is_some() == arrays) {
            let bare = !v.declared && !v.suffixed;
            if as_declared {
                if mixes && v.suffixed && v.ty.plain() == ty.plain() {
                    return Some("AS clause required on first declaration");
                }
                if v.explicit && (v.declared || bare) {
                    return Some("Duplicate definition");
                }
            } else if suffixed {
                if mixes && v.declared && v.explicit && v.ty.plain() == ty.plain() {
                    return Some("AS clause required");
                }
                if v.explicit && (v.declared || (v.suffixed && v.ty.plain() == ty.plain()) || (bare && v.ty.plain() == ty.plain())) {
                    return Some("Duplicate definition");
                }
            } else {
                if v.explicit && v.declared {
                    return Some("AS clause required");
                }
                if v.explicit && (bare || (v.suffixed && v.ty.plain() == ty.plain())) {
                    return Some("Duplicate definition");
                }
            }
        }
        None
    }

    fn share(&mut self, d: &Declaration) {
        let key = stem(&d.name).to_owned();
        if self.scope.vars.get(&key).is_some_and(|vars| vars.iter().any(|v| v.explicit && v.rank.is_none() == !d.array)) {
            self.err(d.span, "Variable required");
            return;
        }
        let found = self.module_scope.vars.get(&key).cloned().unwrap_or_default();
        let wanted = d.type_name.as_ref().and_then(|name| self.type_of_name(name));
        let pick = found.into_iter().find(|v| v.rank.is_some() == d.array);
        match pick {
            Some(var) => {
                if wanted.as_ref().is_some_and(|w| w.plain() != var.ty.plain()) {
                    self.err(d.span, "Duplicate definition");
                    return;
                }
                self.scope.vars.entry(key).or_default().push(var);
            }
            None => {
                let ty = wanted.clone().unwrap_or_else(|| self.default_type(&d.name));
                let var = Var { ty, rank: d.array.then_some(usize::MAX), declared: wanted.is_some() && Ty::of_suffix(&d.name).is_none(), shared: true, dynamic: false, explicit: true, suffixed: Ty::of_suffix(&d.name).is_some(), redimmed: false };
                self.scope.vars.entry(key.clone()).or_default().push(var.clone());
                self.module_scope.vars.entry(key).or_default().push(var);
            }
        }
    }

    fn call(&mut self, name: &str, arguments: &[Expr], explicit: bool, span: Span) {
        if explicit && Ty::of_suffix(name).is_some() {
            self.err(span, "Invalid character");
            return;
        }
        if explicit && !name.contains('.') && crate::qb_reserved::is_reserved_name(self.dialect, name) {
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
                    self.spec(arguments, spec, span);
                    return;
                }
            }
            if matches!(name, "LSET" | "RSET") {
                if let [Expr::Binary { left, right, .. }] = arguments {
                    // LSET moves a string into a string variable, or one record into another
                    if let Some(target) = self.argument_of_class(Class::Var, left) {
                        match &target.ty {
                            Ty::Str(_) => {
                                self.string_expr(right);
                            }
                            Ty::Record(_) if name == "LSET" => {
                                if let Some(value) = self.expr(right) {
                                    if !matches!(value.ty, Ty::Record(_)) {
                                        self.err(right.span(), "Type mismatch");
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
        // `frmMain.cmdOK.Move 1, 2`, `SCREEN.HIDE`: a method of a VBDOS form, control or object
        if let Some((root, _)) = name.split_once('.') {
            let object = matches!(self.dialect, Dialect::VbDos | Dialect::Quickr)
                && (matches!(root.to_ascii_uppercase().as_str(), "SCREEN" | "PRINTER" | "CLIPBOARD") || self.lookup_record_var(root));
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
            // a SUB named only below its first use is no statement yet: BC takes the name for an assignment
            Some(procedure) if procedure.kind == ProcedureKind::Sub && !explicit && span.line < procedure.first_line => {
                self.err(span, "Equal sign missing");
            }
            Some(procedure) if procedure.kind == ProcedureKind::Sub => {
                self.check_arguments(&procedure, arguments, span, true);
            }
            Some(_) => self.err(span, if explicit { "Name of subprogram illegal" } else { "Variable required" }),
            None if qb_sigs::function(name, self.dialect).is_some_and(|sig| sig.statement) => {
                if let Some(sig) = qb_sigs::function(name, self.dialect) {
                    self.intrinsic(sig, name, arguments, span);
                }
            }
            None => {
                if !explicit {
                    // a statement that starts with an unknown name is taken for an assignment
                    self.err(span, if arguments.is_empty() { "Equal sign missing" } else { "DECLARE required" });
                    return;
                }
                for argument in arguments {
                    let typed = self.expr(argument);
                    let _ = typed;
                }
            }
        }
    }

    /// PAINT: a background needs a string tile; empty trailing slots only follow a tile, and not in VBDOS.
    fn paint(&mut self, arguments: &[Expr], span: Span) {
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
    fn isam_list(&mut self, name: &str, arguments: &[Expr], span: Span) {
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

    /// FIELD #n, width AS string-variable, ...: the variables are variable-length strings.
    fn field_statement(&mut self, arguments: &[Expr], span: Span) {
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
    fn on_local_error(&mut self, span: Span) {
        if matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45) {
            self.err(span, "Syntax error");
        } else if self.procedure.is_none() {
            self.err(span, "Illegal outside of SUB or FUNCTION");
        }
    }

    fn runtime(&mut self, name: &str, arguments: &[Expr], span: Span) {
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
            "WINDOW" => "n? n? n? n?",
            "PALETTE" => "i? n?",
            "PCOPY" => "i i",
            "SOUND" => "i n",
            "POKE" | "OUT" => "n i",
            "WAIT" => "n i i?",
            "BEEP" | "TRON" | "TROFF" | "SYSTEM" | "END" => "",
            "FILES" => "s?",
            "RUN" => "x?",
            "CHAIN" => "s",
            "ERROR" => "n",
            "NAME" => "s s",
            "SWAP" => "v v",
            "RESTORE" => "r?",
            "DATE$" | "TIME$" => "s",
            "KEY" if matches!(arguments, [Expr::Name(word, _)] if matches!(word.as_str(), "ON" | "OFF" | "LIST")) => "k",
            "KEY" => "i s",
            "EVENT" => "k i? k l?",
            "LOCK" | "UNLOCK" => "n n? n?",
            "IOCTL" => "i s",
            "FIELD" | "PALETTE_USING" | "CREATEINDEX" | "INSERT" | "RETRIEVE" | "UPDATE" | "SEEKEQ" | "SEEKGE" | "SEEKGT" => "",
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
        if matches!(name, "CREATEINDEX" | "INSERT" | "RETRIEVE" | "UPDATE" | "SEEKEQ" | "SEEKGE" | "SEEKGT") {
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
                        self.err(*at, "Label not defined");
                    }
                } else {
                    self.string_expr(argument);
                }
            }
            return;
        }
        if name == "EVENT" && matches!(arguments.first(), Some(Expr::Name(device, _)) if device == "SIGNAL") {
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
                    Expr::Apply { arguments, .. } if !arguments.is_empty() => self.expr(array).filter(|typed| typed.variable),
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
            if matches!(arguments.last(), Some(Expr::Omitted(_))) && arguments.len() == 6 && matches!(self.dialect, Dialect::VbDos | Dialect::Quickr) {
                self.err(span, "Syntax error");
                return;
            }
        }
        if name == "SWAP" {
            if let [a, b] = arguments {
                let (a_typed, b_typed) = (self.argument_of_class(Class::Var, a), self.argument_of_class(Class::Var, b));
                if let (Some(a), Some(b)) = (a_typed, b_typed) {
                    if a.ty.plain() != b.ty.plain() {
                        self.err(span, "Type mismatch");
                    }
                }
            } else {
                self.err(span, "Argument-count mismatch");
            }
            return;
        }
        // ON TIMER takes seconds, up to a day's worth
        let spec = if name == "EVENT" && matches!(arguments.first(), Some(Expr::Name(device, _)) if device == "TIMER") { "k n? k l?" } else { spec };
        self.spec(arguments, spec, span);
    }
}
