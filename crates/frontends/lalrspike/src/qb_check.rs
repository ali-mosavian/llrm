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
    /// A string; a fixed one has its length.
    Str(Option<usize>),
    Record(String),
}

impl Ty {
    fn numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Long | Ty::Single | Ty::Double)
    }

    fn string(&self) -> bool {
        matches!(self, Ty::Str(_))
    }

    fn rank(&self) -> u8 {
        match self {
            Ty::Int => 1,
            Ty::Long => 2,
            Ty::Single => 3,
            Ty::Double => 4,
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
}

#[derive(Clone, Debug, Default)]
struct Scope {
    /// by the name's stem (suffix removed), upper case
    vars: HashMap<String, Vec<Var>>,
}

#[derive(Clone, Debug)]
struct Record {
    fields: Vec<(String, Ty, Option<usize>)>,
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
    labels: HashSet<String>,
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
            labels: HashSet::new(),
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
        // procedures first: a call may precede its definition
        for procedure in &module.procedures {
            self.declare_procedure(procedure);
        }
        self.collect_labels(&module.statements);
        let statements = module.statements.clone();
        for statement in &statements {
            self.statement(statement);
        }
        self.module_scope = self.scope.clone();
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
            TypeName::String => Some(Ty::Str(None)),
            TypeName::Named(name) if name == "ANY" => None,
            TypeName::Named(name) => self.records.contains_key(name).then(|| Ty::Record(name.clone())),
            _ => None,
        }
    }

    fn declare_procedure(&mut self, procedure: &Procedure) {
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
        };
        match self.procs.get(&key) {
            Some(existing) if existing.defined && info.defined => self.err(procedure.span, "Duplicate definition"),
            Some(existing) if existing.kind != info.kind || existing.params.len() != info.params.len() => {
                self.err(procedure.span, "Argument-count mismatch")
            }
            Some(existing) => {
                info.declared |= existing.declared;
                info.defined |= existing.defined;
            }
            None => {}
        }
        self.procs.insert(key, info);
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
        let var = Var { ty, rank: None, declared: false, shared: false, dynamic: false };
        self.scope.vars.entry(key).or_default().push(var.clone());
        Some(var)
    }

    // ---- procedures ---------------------------------------------------------------------------------

    fn procedure_body(&mut self, procedure: &Procedure) {
        let saved_scope = std::mem::take(&mut self.scope);
        let saved_labels = std::mem::take(&mut self.labels);
        self.procedure = Some((procedure.kind, procedure.name.clone()));
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
        let ty = declaration
            .type_name
            .as_ref()
            .and_then(|name| self.type_of_name(name))
            .unwrap_or_else(|| self.default_type(&declaration.name));
        let var = Var { ty, rank: declaration.array.then_some(1), declared: declaration.type_name.is_some() && Ty::of_suffix(&declaration.name).is_none(), shared: false, dynamic: true };
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
            Expr::Index { base, indices, span } => {
                let base_typed = self.expr(base)?;
                let _ = (base_typed, indices, span);
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
        let Ty::Record(record) = &base_typed.ty else {
            self.err(span, "Type mismatch");
            return None;
        };
        let record = self.records.get(record)?.clone();
        match record.fields.iter().find(|(field, ..)| field == name) {
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
            return Some(constant.clone());
        }
        if let Some(sig) = qb_sigs::function(name, self.dialect) {
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
                self.numeric(argument);
            }
            return Some(Typed::value(Ty::Int));
        }
        // an array element
        let array = self.array_var(name);
        if let Some(var) = array {
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
        if let Some(procedure) = self.procs.get(key).cloned() {
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
        if procedure.params.len() != arguments.len() {
            self.err(span, "Argument-count mismatch");
            return;
        }
        for (parameter, argument) in procedure.params.iter().zip(arguments) {
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
            let class = match letter {
                'n' => Class::Num,
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
                for item in items {
                    self.declare(item, false);
                }
            }
            Statement::Redim(items) => {
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
            Statement::Label(..) | Statement::Comment(_) | Statement::OptionExplicit(_) => {}
            Statement::OptionBase(_, span) => {
                let dimensioned = |scope: &Scope| scope.vars.values().flatten().any(|v| v.rank.is_some());
                if dimensioned(&self.scope) || dimensioned(&self.module_scope) {
                    self.err(*span, "Array already dimensioned");
                }
            }
            Statement::Goto(label, span) | Statement::CallOrGoto(label, span) => {
                if !self.labels.contains(label) && !label.bytes().all(|b| b.is_ascii_digit()) {
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
                if let Some(typed) = typed {
                    if !typed.variable || !typed.ty.numeric() {
                        self.err(counter.span(), "Type mismatch");
                    }
                }
                self.numeric(start);
                self.numeric(end);
                if let Some(step) = step {
                    self.numeric(step);
                }
                self.in_loop.push("FOR");
                for s in body {
                    self.statement(s);
                }
                self.in_loop.pop();
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
                if label != "0" && !self.labels.contains(label) {
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
            Statement::Open { path, file, record_length, .. } => {
                self.string_expr(path);
                self.numeric(file);
                if let Some(length) = record_length {
                    self.numeric(length);
                }
            }
            Statement::OpenClassic { mode, file, path, record_length, .. } => {
                self.string_expr(mode);
                self.numeric(file);
                self.string_expr(path);
                if let Some(length) = record_length {
                    self.numeric(length);
                }
            }
            Statement::Close { files, .. } => {
                for file in files {
                    self.numeric(file);
                }
            }
            Statement::LineInput { file, prompt, destination, .. } => {
                if let Some(file) = file {
                    self.numeric(file);
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
                self.numeric(file);
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
                self.numeric(file);
                self.numeric(position);
            }
            Statement::Print { kind, file, using, items, .. } => {
                if let Some(file) = file {
                    self.numeric(file);
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
                    self.numeric(file);
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
                    let Some(case) = self.expr(value) else { continue };
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
                let ty = self.procs.get(stem(own)).and_then(|p| p.result.clone());
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
            _ => self.expr(target),
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
        let Some(typed) = self.constant_expr(value) else { return };
        let key = stem(name).to_owned();
        if self.consts.contains_key(&key) || self.scope.vars.contains_key(&key) {
            self.err(span, "Duplicate definition");
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
        if self.records.contains_key(name) {
            self.err(span, "Duplicate definition");
            return;
        }
        let mut out: Vec<(String, Ty, Option<usize>)> = Vec::new();
        for field in fields {
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
        self.records.insert(name.to_owned(), Record { fields: out });
    }

    fn declare(&mut self, d: &Declaration, redim: bool) {
        let span = d.span;
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
        for bound in &d.bounds {
            if let Some(lower) = &bound.lower {
                self.numeric(lower);
            }
            self.numeric(&bound.upper);
        }
        let key = stem(&d.name).to_owned();
        let as_declared = d.type_name.is_some() && Ty::of_suffix(&d.name).is_none();
        if self.consts.contains_key(&key) {
            self.err(span, "Duplicate definition");
            return;
        }
        let existing: Vec<Var> = self.scope.vars.get(&key).cloned().unwrap_or_default();
        if d.array {
            if let Some(old) = existing.iter().find(|v| v.rank.is_some()) {
                if redim && old.dynamic {
                    // REDIM resizes a dynamic array
                    if old.ty.plain() != ty.plain() {
                        self.err(span, "Duplicate definition");
                    }
                    return;
                }
                self.err(span, "Array already dimensioned");
                return;
            }
        } else if existing.iter().any(|v| v.rank.is_none() && (v.ty.plain() == ty.plain() || v.declared || as_declared)) {
            self.err(span, "Duplicate definition");
            return;
        }
        // a name bound by AS to another type, or a different suffix, is taken
        if existing.iter().any(|v| v.declared && v.ty.plain() != ty.plain() && v.rank.is_some() == d.array) {
            self.err(span, "Duplicate definition");
            return;
        }
        let var = Var {
            ty,
            rank: d.array.then_some(if d.bounds.is_empty() { usize::MAX } else { d.bounds.len() }),
            declared: as_declared,
            shared: d.shared,
            dynamic: redim || d.dynamic,
        };
        self.scope.vars.entry(key).or_default().push(var);
    }

    fn share(&mut self, d: &Declaration) {
        let key = stem(&d.name).to_owned();
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
                let var = Var { ty, rank: d.array.then_some(usize::MAX), declared: wanted.is_some() && Ty::of_suffix(&d.name).is_none(), shared: true, dynamic: false };
                self.scope.vars.entry(key.clone()).or_default().push(var.clone());
                self.module_scope.vars.entry(key).or_default().push(var);
            }
        }
    }

    fn call(&mut self, name: &str, arguments: &[Expr], explicit: bool, span: Span) {
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
                    self.argument_of_class(Class::StrVar, left);
                    self.string_expr(right);
                } else {
                    self.err(span, "Syntax error");
                }
                return;
            }
        }
        let key = stem(name);
        match self.procs.get(key).cloned() {
            Some(procedure) if procedure.kind == ProcedureKind::Sub => {
                self.check_arguments(&procedure, arguments, span, true);
            }
            Some(_) => self.err(span, if explicit { "Name of subprogram illegal" } else { "Variable required" }),
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
        if !self.spec(stripped, "n n x? n? s?", span) {
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

    /// FIELD #n, width AS string-variable, ...: the variables are variable-length strings.
    fn field_statement(&mut self, arguments: &[Expr], span: Span) {
        let [file, pairs @ ..] = arguments else {
            self.err(span, "Syntax error");
            return;
        };
        self.numeric(file);
        for pair in pairs.chunks(2) {
            let [width, variable] = pair else {
                self.err(span, "Syntax error");
                return;
            };
            self.numeric(width);
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
            "SCREEN" => "n? n? n? n?",
            "COLOR" => "n? n? n?",
            "CLS" | "SLEEP" => "n?",
            "LOCATE" => "n? n? n? n? n?",
            "WIDTH" => "x n?",
            "WIDTH_FILE" => "n n",
            "WIDTH_LPRINT" => "n",
            "PSET" | "PRESET" => "n n n?",
            "LINE" => "n n n n n? m? n?",
            "CIRCLE" => "n n n n? n? n? n?",
            "PAINT" => "n n x? n? s?",
            "DRAW" | "PLAY" => "s",
            "GET" => "n n n n e",
            "PUT" => "n n e m?",
            "VIEW" | "VIEW_SCREEN" => "n? n? n? n? n? n?",
            "VIEW_PRINT" => "n? n?",
            "WINDOW" => "n? n? n? n?",
            "PALETTE" => "n? n?",
            "PCOPY" | "SOUND" | "POKE" | "OUT" => "n n",
            "WAIT" => "n n n?",
            "BEEP" | "TRON" | "TROFF" | "SYSTEM" | "END" => "",
            "FILES" => "s?",
            "RUN" => "x?",
            "CHAIN" => "s",
            "ERROR" => "n",
            "NAME" => "s s",
            "SWAP" => "v v",
            "RESTORE" => "l?",
            "DATE$" | "TIME$" => "s",
            "KEY" if matches!(arguments, [Expr::Name(word, _)] if matches!(word.as_str(), "ON" | "OFF" | "LIST")) => "x",
            "KEY" => "n s",
            "EVENT" => "x n? x x?",
            "LOCK" | "UNLOCK" => "n n? n?",
            "IOCTL" => "n s",
            "FIELD" => "",
            "ENDIF" => "",
            _ => {
                self.err(span, "Syntax error");
                return;
            }
        };
        if name == "FIELD" {
            self.field_statement(arguments, span);
            return;
        }
        if name == "RUN" {
            // RUN "file", RUN line-number
            if let [argument] = arguments {
                if !matches!(argument, Expr::Literal(Literal::Integer(..), _)) {
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
        self.spec(arguments, spec, span);
    }
}
