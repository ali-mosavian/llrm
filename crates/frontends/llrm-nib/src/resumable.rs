//! A generator body as a resumable `next` (section 12). Its locals that live
//! across a `yield` are fields of a state, its frame, read and written
//! through `self`; the body becomes states, each running from one resume
//! point to the next `yield`, `return`, or jump, and `next` is one loop over
//! a `match` of the state. Control flow stays structured, so no loop is
//! entered at its middle.
//!
//! A block is split into states when it holds a `yield`, and then so is
//! every block of the statement holding it. The names bound in a split block
//! are the frame's fields; those bound in a block left whole are its own.
//! A field is zero while its name is out of scope: where a scope ends, on
//! every path, each owning name it bound is reset to zero, which drops its
//! value. Dropping the frame then drops exactly what is live.

use std::collections::{BTreeMap, BTreeSet};

use super::error::Diagnostic;
use super::scopes;
use super::syntax::{AssignTarget, BinaryOp, Expr, MatchArm, Pattern, Span, Statement, TypeSpec};

/// The state's field holding the resume point.
pub const RESUME: &str = "$resume";
/// How a generator's state type is named: the stem, then a number.
pub const STATE: &str = "$state";

/// What the frame keeps, by name.
pub struct Kept {
    /// Every name kept: the parameters and the split blocks' bindings.
    pub fields: BTreeSet<String>,
    /// Those whose value is dropped where their scope ends.
    pub owning: BTreeSet<String>,
    /// Those that are iterators, which a `for` steps in place.
    pub iterators: BTreeSet<String>,
    /// The views the body binds, read by name and assigned through `self`.
    pub views: BTreeSet<String>,
}

/// Where a split loop's `break` and `continue` go, and the scopes they leave.
struct Targets {
    exit: usize,
    next: usize,
    depth: usize,
}

pub struct Lowering<'a> {
    kept: &'a Kept,
    states: Vec<Vec<Statement>>,
    /// The state that has ended: every later `next` is `.none`.
    done: usize,
    loops: Vec<Targets>,
    /// The owning names in scope, a list per open split block, each in binding order.
    scopes: Vec<Vec<String>>,
    /// Each split loop's item that is an element of the sequence it steps.
    elements: BTreeMap<String, Expr>,
    /// Fields the lowering adds: a loop's counter, bound or iterator.
    pub hidden: Vec<(String, HiddenType)>,
    span: Span,
}

/// A hidden field's type: a counter's, another field's, or one named.
pub enum HiddenType {
    Counter,
    SameAs(String),
    Named(TypeSpec),
}

impl<'a> Lowering<'a> {
    pub fn new(kept: &'a Kept, span: Span) -> Self {
        Self { kept, states: vec![Vec::new(), Vec::new()], done: 1, loops: Vec::new(), scopes: Vec::new(), elements: BTreeMap::new(), hidden: Vec::new(), span }
    }

    /// `body`, whose scope opens with `parameters`, as the arms of `match
    /// self.$resume`, the last ending it.
    pub fn arms(mut self, mut body: Vec<Statement>, parameters: &[String]) -> Result<(Vec<MatchArm>, Vec<(String, HiddenType)>), Diagnostic> {
        self.block(&mut body, 0, self.done, parameters)?;
        self.states[self.done] = self.finish();
        let span = self.span;
        let done = std::mem::take(&mut self.states[self.done]);
        let mut arms: Vec<MatchArm> = std::mem::take(&mut self.states)
            .into_iter()
            .enumerate()
            .filter(|(index, _)| *index != self.done)
            .map(|(index, body)| MatchArm { pattern: Pattern::Literal(Expr::Integer(index as i64, span)), body, span })
            .collect();
        arms.push(MatchArm { pattern: Pattern::Wildcard(span), body: done, span });
        Ok((arms, self.hidden))
    }

    fn reserve(&mut self) -> usize {
        self.states.push(Vec::new());
        self.states.len() - 1
    }

    fn this(&self) -> Expr {
        Expr::Name("self".into(), self.span)
    }

    /// `self.field`.
    fn field(&self, name: &str) -> Expr {
        Expr::Member { base: Box::new(self.this()), field: name.into(), span: self.span }
    }

    fn set(&self, name: &str, value: Expr) -> Statement {
        Statement::Assign {
            target: AssignTarget::Member { base: self.this(), field: name.into() },
            operation: None,
            value,
            span: self.span,
        }
    }

    /// Resumes at `state`.
    fn goto(&self, state: usize) -> Vec<Statement> {
        vec![self.set(RESUME, Expr::Integer(state as i64, self.span)), Statement::Continue(self.span)]
    }

    /// `.none` from here on: the generator has ended.
    fn finish(&self) -> Vec<Statement> {
        let none = Expr::Variant { enum_name: None, name: "none".into(), arguments: Vec::new(), span: self.span };
        vec![self.set(RESUME, Expr::Integer(self.done as i64, self.span)), Statement::Return { value: Some(none), span: self.span }]
    }

    fn hidden_field(&mut self, stem: &str, type_: HiddenType) -> String {
        let name = format!("${stem}{}", self.hidden.len());
        self.hidden.push((name.clone(), type_));
        name
    }

    /// Opens a scope holding `names`.
    fn open(&mut self, names: &[String]) {
        let owning = names.iter().filter(|one| self.kept.owning.contains(*one)).cloned().collect();
        self.scopes.push(owning);
    }

    /// `name`, just bound, is in the innermost scope.
    fn bound(&mut self, name: &str) {
        if self.kept.owning.contains(name) {
            self.scopes.last_mut().expect("a scope").push(name.into());
        }
    }

    /// Resets, innermost last binding first, what the scopes from `depth` on hold.
    fn leave(&self, depth: usize) -> Vec<Statement> {
        let names = self.scopes[depth..].iter().rev().flat_map(|scope| scope.iter().rev());
        names.map(|name| self.set(name, Expr::Zero(self.span))).collect()
    }

    /// Closes the innermost scope where `state` ends it, unless it has left.
    fn close(&mut self, state: usize) {
        if !self.left(state) {
            let resets = self.leave(self.scopes.len() - 1);
            self.states[state].extend(resets);
        }
        self.scopes.pop();
    }

    /// Whether `state` has already jumped or returned.
    fn left(&self, state: usize) -> bool {
        fn leaves(statement: Option<&Statement>) -> bool {
            match statement {
                Some(Statement::Continue(_) | Statement::Return { .. }) => true,
                Some(Statement::Unsafe { body, .. }) => leaves(body.last()),
                _ => false,
            }
        }
        leaves(self.states[state].last())
    }

    /// Lowers `body`, a scope opening with `names`, from `state`, then
    /// resumes at `then`.
    fn block(&mut self, body: &mut [Statement], state: usize, then: usize, names: &[String]) -> Result<usize, Diagnostic> {
        self.open(names);
        let state = self.inline(body, state)?;
        self.close(state);
        if !self.left(state) {
            let jump = self.goto(then);
            self.states[state].extend(jump);
        }
        Ok(then)
    }

    /// Lowers `body` from `state` in the scope open now; the state it ends in.
    fn inline(&mut self, body: &mut [Statement], mut state: usize) -> Result<usize, Diagnostic> {
        for statement in body {
            state = self.statement(statement, state)?;
        }
        Ok(state)
    }

    fn statement(&mut self, statement: &mut Statement, state: usize) -> Result<usize, Diagnostic> {
        let span = statement.span();
        match statement {
            // Every name a split block binds is a field.
            Statement::Bind { name, value, .. } => {
                let mut value = value.clone();
                self.expression(&mut value);
                let made = self.set(name, value);
                self.states[state].push(made);
                self.bound(name);
                return Ok(state);
            }
            Statement::Destructure { pattern, value, otherwise, .. } => return self.destructure(pattern, value, otherwise, state, span),
            _ if !holds_yield(statement) => {
                let lowered = self.whole(statement.clone(), false)?;
                self.states[state].extend(lowered);
                return Ok(state);
            }
            _ => {}
        }
        match statement {
            Statement::Yield { value, .. } => {
                let mut value = value.clone();
                self.expression(&mut value);
                let resume = self.reserve();
                let some = Expr::Variant { enum_name: None, name: "some".into(), arguments: vec![value], span };
                let made = self.set(RESUME, Expr::Integer(resume as i64, span));
                self.states[state].push(made);
                self.states[state].push(Statement::Return { value: Some(some), span });
                Ok(resume)
            }
            Statement::If { condition, then_branch, else_branch, .. } => {
                let (then, otherwise, after) = (self.reserve(), self.reserve(), self.reserve());
                let mut condition = condition.clone();
                self.expression(&mut condition);
                let branch = Statement::If { condition, then_branch: self.goto(then), else_branch: self.goto(otherwise), span };
                self.states[state].push(branch);
                self.block(then_branch, then, after, &[])?;
                self.block(else_branch, otherwise, after, &[])?;
                Ok(after)
            }
            Statement::While { condition, body, .. } => {
                let (head, inside, after) = (self.reserve(), self.reserve(), self.reserve());
                let made = self.goto(head);
                self.states[state].extend(made);
                let mut condition = condition.clone();
                self.expression(&mut condition);
                let made = Statement::If { condition, then_branch: self.goto(inside), else_branch: self.goto(after), span };
                self.states[head].push(made);
                self.looped(body, inside, head, after, head, &[])?;
                Ok(after)
            }
            Statement::ForRange { name, start, end, body, .. } => {
                let bound = self.hidden_field("end", HiddenType::SameAs(name.clone()));
                let (mut start, mut end) = (start.clone(), end.clone());
                self.expression(&mut start);
                self.expression(&mut end);
                let made = self.set(name, start);
                self.states[state].push(made);
                let made = self.set(&bound, end);
                self.states[state].push(made);
                let below = Expr::Binary { op: BinaryOp::Less, left: Box::new(self.field(name)), right: Box::new(self.field(&bound)), span };
                let step = self.stepped(name);
                self.counted(state, below, None, step, body, name)
            }
            Statement::For { name, iterable, body, .. } => {
                let mut iterable = iterable.clone();
                self.expression(&mut iterable);
                if let Some(state_type) = generator_state(&iterable) {
                    // Another generator, kept in this one's state while the loop runs.
                    let iterator = self.hidden_field("iterator", HiddenType::Named(TypeSpec::Named(state_type)));
                    let made = self.set(&iterator, iterable);
                    self.states[state].push(made);
                    self.scopes.push(vec![iterator.clone()]);
                    let after = self.stepped_iterator(state, &iterator, name, body)?;
                    self.close(after);
                    return Ok(after);
                }
                if let Expr::Member { base, field, .. } = &iterable {
                    if matches!(base.as_ref(), Expr::Name(this, _) if this == "self") && self.kept.iterators.contains(field) {
                        let field = field.clone();
                        return self.stepped_iterator(state, &field, name, body);
                    }
                }
                // A sequence, by index; the item is the element, a place.
                let counter = self.hidden_field("index", HiddenType::Counter);
                let made = self.set(&counter, Expr::Integer(0, span));
                self.states[state].push(made);
                let length = Expr::Member { base: Box::new(iterable.clone()), field: "len".into(), span };
                let below = Expr::Binary { op: BinaryOp::Less, left: Box::new(self.field(&counter)), right: Box::new(length), span };
                let element = Expr::Index { base: Box::new(iterable), indices: vec![self.field(&counter)], span };
                self.elements.insert(name.clone(), element);
                let step = self.stepped(&counter);
                self.counted(state, below, None, step, body, name)
            }
            Statement::Match { subject, arms, .. } => {
                let mut subject = subject.clone();
                self.expression(&mut subject);
                let after = self.reserve();
                let mut dispatched = Vec::new();
                for arm in arms.iter_mut() {
                    let inside = self.reserve();
                    // The arm's names are fields; the pattern binds them afresh.
                    let (pattern, names) = rename_bindings(&arm.pattern);
                    let mut taken = self.taken(&names);
                    taken.extend(self.goto(inside));
                    dispatched.push(MatchArm { pattern, body: taken, span: arm.span });
                    self.block(&mut arm.body, inside, after, &names)?;
                }
                self.states[state].push(Statement::Match { subject, arms: dispatched, span });
                Ok(after)
            }
            Statement::With { name, value, body, .. } => {
                let mut value = value.clone();
                self.expression(&mut value);
                let made = self.set(name, value);
                self.states[state].push(made);
                self.open(std::slice::from_ref(name));
                let state = self.inline(body, state)?;
                self.close(state);
                Ok(state)
            }
            Statement::Unsafe { body, .. } => {
                // Each piece of the body, whichever state runs it, stays unsafe.
                let marks: Vec<usize> = self.states.iter().map(Vec::len).collect();
                self.open(&[]);
                let end = self.inline(body, state)?;
                self.close(end);
                for (index, statements) in self.states.iter_mut().enumerate() {
                    let from = marks.get(index).copied().unwrap_or(0);
                    if statements.len() > from {
                        let body = statements.split_off(from);
                        statements.push(Statement::Unsafe { body, span });
                    }
                }
                Ok(end)
            }
            _ => unreachable!("only a statement with a block holds a yield"),
        }
    }

    /// `let pattern = value else: otherwise`: the pattern binds its names
    /// afresh, and each moves to its field.
    fn destructure(&mut self, pattern: &Pattern, value: &Expr, otherwise: &mut Option<Vec<Statement>>, state: usize, span: Span) -> Result<usize, Diagnostic> {
        let mut value = value.clone();
        self.expression(&mut value);
        let (pattern, names) = rename_bindings(pattern);
        let otherwise = match otherwise {
            None => None,
            Some(block) if !block.iter().any(holds_yield) => {
                let mut whole = Vec::new();
                for one in block.iter() {
                    whole.extend(self.whole(one.clone(), false)?);
                }
                Some(whole)
            }
            Some(block) => {
                let other = self.reserve();
                self.block(block, other, self.done, &[])?;
                Some(self.goto(other))
            }
        };
        self.states[state].push(Statement::Destructure { pattern, value, otherwise, span });
        let taken = self.taken(&names);
        self.states[state].extend(taken);
        for name in &names {
            self.bound(name);
        }
        Ok(state)
    }

    /// Moves each renamed binding of `names` to its field.
    fn taken(&self, names: &[String]) -> Vec<Statement> {
        names.iter().map(|name| self.set(name, Expr::Name(bound_name(name), self.span))).collect()
    }

    /// `name += 1`.
    fn stepped(&self, name: &str) -> Statement {
        Statement::Assign {
            target: AssignTarget::Member { base: self.this(), field: name.into() },
            operation: Some(BinaryOp::Add),
            value: Expr::Integer(1, self.span),
            span: self.span,
        }
    }

    /// A loop from `state` over the iterator field `iterator`, binding
    /// `name` to each item; the state after it.
    fn stepped_iterator(&mut self, state: usize, iterator: &str, name: &str, body: &mut [Statement]) -> Result<usize, Diagnostic> {
        let span = self.span;
        let (head, inside, after) = (self.reserve(), self.reserve(), self.reserve());
        let made = self.goto(head);
        self.states[state].extend(made);
        let next = Expr::MethodCall { receiver: Box::new(self.field(iterator)), name: "next".into(), type_arguments: Vec::new(), arguments: Vec::new(), span };
        let mut taken = self.taken(&[name.to_string()]);
        taken.extend(self.goto(inside));
        let arms = vec![
            MatchArm { pattern: Pattern::Variant { enum_name: None, name: "some".into(), fields: vec![Pattern::Binding(bound_name(name), span)], span }, body: taken, span },
            MatchArm { pattern: Pattern::Variant { enum_name: None, name: "none".into(), fields: Vec::new(), span }, body: self.goto(after), span },
        ];
        self.states[head].push(Statement::Match { subject: next, arms, span });
        self.looped(body, inside, head, after, head, &[name.to_string()])?;
        Ok(after)
    }

    /// A counted loop from `state`: while `below`, `body` with `name` in
    /// scope, then `step`.
    fn counted(&mut self, state: usize, below: Expr, take: Option<Statement>, step: Statement, body: &mut [Statement], name: &str) -> Result<usize, Diagnostic> {
        let (head, inside, next, after) = (self.reserve(), self.reserve(), self.reserve(), self.reserve());
        let made = self.goto(head);
        self.states[state].extend(made);
        let mut entered: Vec<Statement> = take.into_iter().collect();
        entered.extend(self.goto(inside));
        let made = Statement::If { condition: below, then_branch: entered, else_branch: self.goto(after), span: self.span };
        self.states[head].push(made);
        self.states[next].push(step);
        let made = self.goto(head);
        self.states[next].extend(made);
        self.looped(body, inside, next, after, next, &[name.to_string()])?;
        Ok(after)
    }

    /// A loop's body from `inside`, a scope opening with `names`, its
    /// `continue` going to `next`.
    fn looped(&mut self, body: &mut [Statement], inside: usize, next: usize, exit: usize, then: usize, names: &[String]) -> Result<(), Diagnostic> {
        self.loops.push(Targets { exit, next, depth: self.scopes.len() });
        let result = self.block(body, inside, then, names);
        self.loops.pop();
        result.map(|_| ())
    }

    /// A statement holding no `yield`, kept whole: its names that are fields
    /// read `self`, and its `break`, `continue` and `return` that leave the
    /// split code end the scopes they leave and jump. Inside a loop it keeps,
    /// `break` and `continue` are that loop's.
    fn whole(&mut self, mut statement: Statement, in_loop: bool) -> Result<Vec<Statement>, Diagnostic> {
        match &mut statement {
            Statement::Break(_) if !in_loop => {
                let target = self.loops.last().expect("a split loop");
                let mut left = self.leave(target.depth);
                left.extend(self.goto(target.exit));
                return Ok(left);
            }
            Statement::Continue(_) if !in_loop => {
                let target = self.loops.last().expect("a split loop");
                let mut left = self.leave(target.depth);
                left.extend(self.goto(target.next));
                return Ok(left);
            }
            Statement::Return { value, span } => {
                if value.is_some() {
                    return Err(Diagnostic::new(*span, "a generator returns no value"));
                }
                let mut left = self.leave(0);
                left.extend(self.finish());
                return Ok(left);
            }
            Statement::Assign { target: target @ AssignTarget::Name(_), .. } => {
                let AssignTarget::Name(name) = &*target else { unreachable!() };
                if let Some(element) = self.elements.get(name) {
                    *target = AssignTarget::of(element.clone()).expect("an element is a place");
                } else if self.kept.fields.contains(name.as_str()) || self.kept.views.contains(name.as_str()) {
                    *target = AssignTarget::Member { base: self.this(), field: name.clone() };
                }
            }
            _ => {}
        }
        for expression in statement.own_expressions_mut() {
            self.expression(expression);
        }
        let inner = in_loop || matches!(statement, Statement::While { .. } | Statement::For { .. } | Statement::ForRange { .. });
        for block in statement.blocks_mut() {
            let kept = std::mem::take(block);
            for one in kept {
                block.extend(self.whole(one, inner)?);
            }
        }
        Ok(vec![statement])
    }

    /// Each field `expression` names, read through `self`, and each loop
    /// item, its element.
    fn expression(&self, expression: &mut Expr) {
        let Ok(()) = expression.walk_mut(&mut |one| -> Result<(), std::convert::Infallible> {
            if let Expr::Name(name, _) = one {
                if let Some(element) = self.elements.get(name) {
                    *one = element.clone();
                    return Ok(());
                }
            }
            if let Expr::Name(name, span) = one {
                if self.kept.fields.contains(name.as_str()) {
                    *one = Expr::Member { base: Box::new(Expr::Name("self".into(), *span)), field: name.clone(), span: *span };
                }
            }
            Ok(())
        });
    }
}

/// The state type of a generator `expression` starts, if it starts one.
pub fn generator_state(expression: &Expr) -> Option<String> {
    match expression {
        Expr::StructLiteral { name, .. } if name.starts_with(STATE) => Some(name.clone()),
        _ => None,
    }
}

/// Whether `statement` holds a `yield`, at any depth.
pub fn holds_yield(statement: &Statement) -> bool {
    let mut statement = statement.clone();
    matches!(statement, Statement::Yield { .. }) || statement.blocks_mut().into_iter().any(|block| block.iter().any(holds_yield))
}

/// The names a split `body` binds, in order: its fields besides the parameters.
pub fn split_bindings(body: &[Statement]) -> Vec<(String, Span)> {
    let mut found = Vec::new();
    for statement in body {
        let split = holds_yield(statement);
        match statement {
            Statement::Bind { name, span, .. } => found.push((name.clone(), *span)),
            Statement::Destructure { pattern, span, .. } => found.extend(pattern.names().into_iter().map(|name| (name.to_string(), *span))),
            Statement::For { name, span, .. } | Statement::ForRange { name, span, .. } | Statement::With { name, span, .. } if split => {
                found.push((name.clone(), *span));
            }
            Statement::Match { arms, .. } if split => {
                for arm in arms {
                    found.extend(arm.pattern.names().into_iter().map(|name| (name.to_string(), arm.span)));
                }
            }
            _ => {}
        }
        if split {
            let mut statement = statement.clone();
            for block in statement.blocks_mut() {
                found.extend(split_bindings(block));
            }
        }
    }
    found
}

/// The name a pattern binds in place of `name`, before it moves to its field.
fn bound_name(name: &str) -> String {
    format!("{name}$bound")
}

/// `pattern` binding each of its names under its bound name, and the names.
fn rename_bindings(pattern: &Pattern) -> (Pattern, Vec<String>) {
    let mut renamed = pattern.clone();
    let mut names = Vec::new();
    for name in pattern_names_mut(&mut renamed) {
        names.push(name.clone());
        *name = bound_name(name);
    }
    (renamed, names)
}

/// Each name `pattern` binds, to rename.
fn pattern_names_mut(pattern: &mut Pattern) -> Vec<&mut String> {
    match pattern {
        Pattern::Binding(name, _) => vec![name],
        Pattern::Variant { fields, .. } | Pattern::Struct { fields, .. } | Pattern::Tuple(fields, _) => {
            fields.iter_mut().flat_map(pattern_names_mut).collect()
        }
        Pattern::Sequence { before, rest, after, .. } => {
            before.iter_mut().chain(rest.as_deref_mut()).chain(after).flat_map(pattern_names_mut).collect()
        }
        Pattern::Wildcard(_) | Pattern::Literal(_) => Vec::new(),
    }
}

/// Each name `statement` itself binds, to rename.
fn binding_names_mut(statement: &mut Statement) -> Vec<&mut String> {
    match statement {
        Statement::Bind { name, .. } | Statement::For { name, .. } | Statement::ForRange { name, .. } | Statement::With { name, .. } => vec![name],
        Statement::Destructure { pattern, .. } => pattern_names_mut(pattern),
        Statement::Match { arms, .. } => arms.iter_mut().flat_map(|arm| pattern_names_mut(&mut arm.pattern)).collect(),
        _ => Vec::new(),
    }
}

/// `body` with each binding of a name bound before -- a parameter
/// included -- given a name of its own, and each use renamed to the binding
/// it sees. A frame keeps one field per name, so each binding needs one.
pub fn unique_bindings(body: &mut [Statement], parameters: &[String]) {
    let mut seen: BTreeSet<String> = parameters.iter().cloned().collect();
    let mut original: BTreeMap<String, String> = BTreeMap::new();
    for statement in body.iter_mut() {
        statement.each_mut(&mut |one| {
            for name in binding_names_mut(one) {
                if !seen.insert(name.clone()) {
                    let renamed = format!("{name}$again{}", original.len() + 1);
                    original.insert(renamed.clone(), std::mem::replace(name, renamed));
                }
            }
        });
    }
    let mut locals = parameters.to_vec();
    let Ok(()) = scopes::walk_mut(body, &mut locals, &mut |expression, locals| -> Result<(), std::convert::Infallible> {
        if let Expr::Name(name, _) = expression {
            let sees = locals.iter().rev().find(|local| original.get(*local).unwrap_or(local) == name);
            if let Some(local) = sees {
                *name = local.clone();
            }
        }
        Ok(())
    });
}
