//! ON ERROR: a function's error lands at its landing pad (`crate::onerror`).
//! Each call that may raise outside the handlers is an invoke, after storing
//! its statement's number; RESUME NEXT and RESUME switch on that number to
//! the statement after or the statement itself, RESUME label goes to the
//! label.
//!
//! The module's handlers, which ON ERROR GOTO names in its body, take an
//! error any procedure raises, on that procedure's frame: they run as one
//! function of their own, which every pad calls and whose answer is the
//! RESUME it ran. A procedure's own handlers, ON LOCAL ERROR's, run in it,
//! entered from its pad, and read its locals.

use std::collections::{BTreeSet, HashMap};

use llrm_mir::opcode::Attribute;
use llrm_mir::{BlockId, Constant, ConstantKind, Operand as Value, TypeId};

use super::{Body, Emit};
use crate::model::{self, Operand, Statement, Storage};
use crate::onerror::Handled;

const REGISTER: &str = "$QB$OERG:";
const LABEL: &str = "$QB$RESA:";
const NEXT: &str = "B$RESN";
const AGAIN: &str = "B$RES0";
const ERR: &str = "B$FERR";
const ERL: &str = "B$FERL";

/// What the module handler answers for RESUME and RESUME NEXT; RESUME
/// label's `LABELS + k` for its `k`th label.
const RESUMED_AGAIN: i128 = 0;
const RESUMED_NEXT: i128 = 1;
const LABELS: i128 = 2;

/// Whether this owns what a call of `callee` means.
pub(super) fn owns(callee: &str) -> bool {
    callee.starts_with(REGISTER) || callee.starts_with(LABEL) || [NEXT, AGAIN, ERL].contains(&callee)
}

/// What raises error `n`, as ERROR does: RESUME reached by falling
/// through raises RESUME_WITHOUT_ERROR with it, and the module handler run
/// to the module's end NO_RESUME.
pub(super) const RAISE: &str = "B$SERR";
pub(super) const RESUME_WITHOUT_ERROR: i128 = 20;
pub(super) const NO_RESUME: i128 = 19;

/// Whether `callee`, somewhere in `function`, is a RESUME or RESUME NEXT.
pub(super) fn resumes(callee: &str) -> bool {
    [NEXT, AGAIN].contains(&callee)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Resume {
    Next,
    Again,
}

/// The module's handlers: the body's blocks they run, run as the function
/// `outlined`, `i16 (i16 err, i16 erl)`, which answers the RESUME it ran.
/// A block the body also runs, falling through to a handler's label, is
/// emitted in both: in the body it is ordinary code.
#[derive(Clone, Debug)]
pub(super) struct ModuleHandler {
    /// The module body's function, whose ON ERROR GOTOs name them.
    pub owner: i64,
    /// Each handler, numbered from 1 in this order.
    handlers: Vec<i64>,
    /// The blocks the handlers reach.
    inside: BTreeSet<i64>,
    /// Those the body does not: its copy leaves them out.
    only: BTreeSet<i64>,
    /// The line of the last error it took, a global: ERL outside it.
    last_erl: Value,
    /// Whether RESUME clears it, as the runtime's does.
    resume_clears_erl: bool,
    /// Each RESUME label's block, in the order the answers number them.
    labels: Vec<i64>,
    /// The number of the handler ON ERROR GOTO last named, a global.
    active: Value,
    pub outlined: (Value, TypeId),
}

impl ModuleHandler {
    /// The handlers of `owner`, whose ON ERROR GOTOs name them, as the
    /// function `outlined`, keeping the one ON ERROR GOTO last named in
    /// `active` and the line of the last error it took in `erl`, with
    /// whether RESUME clears it; refused where a local both they and the
    /// body use would be two, one in each frame.
    /// `rows` are the owner's statements, `raises` whether a callee may raise.
    pub(super) fn of(owner: &model::Function, active: Value, erl: (Value, bool), rows: &[Statement], raises: &dyn Fn(&str) -> bool, outlined: (Value, TypeId)) -> Emit<Self> {
        let (last_erl, resume_clears_erl) = erl;
        let handlers = handlers(owner)?;
        // The body emits every block the handlers do not reach, however it
        // is entered (a DATA block is no entry once laid out), and what those
        // reach; and each statement RESUME may continue at from it.
        let inside = reached(owner, handlers.iter().copied());
        let ran = reached(owner, owner.blocks.iter().map(|one| one.id).filter(|one| !inside.contains(one)));
        let ran = resumed(owner, ran, &numbered(rows, owner.id), raises);
        let only: BTreeSet<i64> = inside.difference(&ran).copied().collect();
        let used = |blocks: &mut dyn Iterator<Item = &model::Block>| -> BTreeSet<i64> { blocks.flat_map(places_of).collect() };
        let theirs = used(&mut owner.blocks.iter().filter(|one| inside.contains(&one.id)));
        let body = used(&mut owner.blocks.iter().filter(|one| !only.contains(&one.id)));
        if let Some(place) = owner.places.iter().find(|one| one.storage == Storage::Local && theirs.contains(&one.id) && body.contains(&one.id)) {
            return Err(format!("{}, a local both the body and its error handler use: the handler runs on the frame of the procedure that raised", place.name));
        }
        let mut labels = Vec::new();
        for callee in owner.blocks.iter().filter(|one| inside.contains(&one.id)).flat_map(|one| &one.instructions).filter_map(|one| one.callee.as_deref()) {
            if let Some(label) = callee.strip_prefix(LABEL) {
                let label: i64 = label.parse().map_err(|_| "a RESUME marker without its label")?;
                if !labels.contains(&label) {
                    labels.push(label);
                }
            }
        }
        Ok(Self { owner: owner.id, handlers, inside, only, last_erl, resume_clears_erl, labels, active, outlined })
    }
}

pub(super) struct Handling {
    handled: Handled,
    /// Each handler ON ERROR GOTO names, numbered from 1 in this order.
    handlers: Vec<i64>,
    /// The number of the handler ON ERROR GOTO last named.
    active: Value,
    pad: BlockId,
    selector: Value,
    site: Value,
    /// The function's statements, in source order: a site is an index.
    statements: Vec<Statement>,
    /// The handlers' blocks: what the pad reaches before a RESUME.
    inside: BTreeSet<i64>,
    raising: BTreeSet<usize>,
    resumes: Vec<(BlockId, Resume)>,
    /// The module handler's RESUME labels, which only its body can take.
    labels: Vec<i64>,
    /// Whether the module handler serves this function but runs elsewhere.
    served: bool,
    /// Whether the block being emitted is a handler's.
    pub(super) handling: bool,
}

/// The module handler, being emitted as its own function: its arguments,
/// ERR and ERL.
pub(super) struct Outlined {
    handler: ModuleHandler,
    handled: Handled,
    err: Value,
    erl: Value,
}

/// `function`'s statements, in source order: a site is an index.
pub(super) fn numbered(statements: &[Statement], function: i64) -> Vec<Statement> {
    let mut numbered: Vec<Statement> = statements.iter().copied().filter(|one| one.function == function).collect();
    numbered.sort_by_key(|one| one.instruction);
    numbered
}

/// Each handler `function`'s ON ERROR GOTOs name, in the order they first do.
fn handlers(function: &model::Function) -> Emit<Vec<i64>> {
    let mut handlers: Vec<i64> = function.error_handler.into_iter().collect();
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        let Some((handler, _)) = instruction.callee.as_deref().and_then(|one| one.strip_prefix(REGISTER)).and_then(|one| one.split_once(':')) else { continue };
        let handler: i64 = handler.parse().map_err(|_| "an ON ERROR marker without its handler")?;
        if handler != 0 && !handlers.contains(&handler) {
            handlers.push(handler);
        }
    }
    Ok(handlers)
}

/// The blocks `handlers` reach, and those the body does, from its entry
/// and the statements RESUME may continue at outside them.
fn reaches(function: &model::Function, handlers: &[i64]) -> (BTreeSet<i64>, BTreeSet<i64>) {
    let inside = reached(function, handlers.iter().copied());
    let body = reached(function, std::iter::once(function.entry).chain(function.external_entries.iter().copied().filter(|one| !inside.contains(one))));
    (inside, body)
}

/// `body` grown by each statement RESUME may continue at from the body: a
/// statement of it that may raise (`raises`, of a callee), and the one
/// after, where RESUME NEXT goes though it be the handlers' code, as falling
/// through reaches it. `rows` are `function`'s statements in source order.
fn resumed(function: &model::Function, mut body: BTreeSet<i64>, rows: &[Statement], raises: &dyn Fn(&str) -> bool) -> BTreeSet<i64> {
    let mut raising = vec![false; rows.len()];
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        let Some(callee) = instruction.callee.as_deref().filter(|one| raises(one)) else { continue };
        if let Some(row) = rows.partition_point(|one| one.instruction <= instruction.id).checked_sub(1) {
            raising[row] = true;
        }
    }
    loop {
        let added: Vec<i64> = (0..rows.len())
            .filter(|&row| raising[row] && body.contains(&rows[row].block))
            .filter_map(|row| rows.get(row + 1).map(|next| next.block))
            .filter(|block| !body.contains(block))
            .collect();
        if added.is_empty() {
            return body;
        }
        body = reached(function, body.iter().copied().chain(added));
    }
}

/// The blocks `handlers` reach, which the body must not: a procedure's own
/// handler runs in it, where one block cannot be both.
fn apart(function: &model::Function, handlers: &[i64]) -> Emit<BTreeSet<i64>> {
    let (inside, body) = reaches(function, handlers);
    match inside.intersection(&body).next() {
        Some(shared) => Err(format!("block {shared}, which both the body and its error handler run")),
        None => Ok(inside),
    }
}

/// Each place `block` names.
fn places_of(block: &model::Block) -> impl Iterator<Item = i64> + '_ {
    let operands = block.instructions.iter().flat_map(|one| &one.operands).chain(&block.terminator.operands);
    operands.filter_map(|operand| match operand {
        Operand::PlaceRef(one) => Some(one.place),
        Operand::ArrayElement(one) => Some(one.place),
        Operand::ProjectedPlace(one) => Some(one.place),
        _ => None,
    })
}

/// Each block `from` reaches in `function`.
fn reached(function: &model::Function, from: impl IntoIterator<Item = i64>) -> BTreeSet<i64> {
    let blocks: HashMap<i64, &model::Block> = function.blocks.iter().map(|one| (one.id, one)).collect();
    let mut seen = BTreeSet::new();
    let mut pending: Vec<i64> = from.into_iter().collect();
    while let Some(block) = pending.pop() {
        let Some(&one) = blocks.get(&block) else { continue };
        if seen.insert(block) {
            pending.extend(one.terminator.targets.iter().chain(one.terminator.cases.iter().map(|(_, target)| target)).copied());
        }
    }
    seen
}

impl Body<'_, '_, '_> {
    /// The blocks another function runs: the module handler's, in its body.
    pub(super) fn elsewhere(&self) -> BTreeSet<i64> {
        match &self.tables.module_handler {
            Some(handler) if handler.owner == self.function.id && self.outlined.is_none() => handler.only.clone(),
            _ => BTreeSet::new(),
        }
    }

    /// Makes the pad; the entry block is current.
    pub(super) fn handle(&mut self, handled: Handled, statements: &[Statement]) -> Emit<()> {
        let word = self.b.context.types.int(16);
        let site = self.b.alloca(word, "site");
        let statements = numbered(statements, self.function.id);
        let entry = self.b.current().expect("the entry block");
        let pad = self.b.block("landing");
        let null = self.b.context.types.ptr(0);
        let null = Value::Constant(self.b.context.constant(Constant { ty: null, kind: ConstantKind::Null }));
        let module = self.tables.module_handler.clone();
        let handling = if self.function.error_handler_local {
            if module.is_some() {
                return Err("ON LOCAL ERROR in a module whose body has ON ERROR GOTO: which handler takes an error is not selected yet".to_owned());
            }
            let handlers = handlers(self.function)?;
            let inside = apart(self.function, &handlers)?;
            let active = crate::onerror::active(self.b);
            self.b.position(pad);
            let selector = self.b.landing_pad(handled.pad, &[null], "landed");
            crate::onerror::landed(self.b, &handled);
            match handlers[..] {
                [only] => self.b.br(self.block(only)),
                _ => {
                    let number = self.b.load(word, active, false, "");
                    let cases: Vec<(Value, BlockId)> = handlers.iter().enumerate().map(|(at, &one)| (self.b.int(16, at as i128 + 1), self.block(one))).collect();
                    self.dispatch(number, &cases);
                }
            }
            Handling { handled, handlers, active, pad, selector, site, statements, inside, raising: BTreeSet::new(), resumes: Vec::new(), labels: Vec::new(), served: false, handling: false }
        } else {
            let module = module.ok_or("an error handler the module body's ON ERROR GOTO does not name")?;
            self.b.position(pad);
            let selector = self.b.landing_pad(handled.pad, &[null], "landed");
            crate::onerror::landed(self.b, &handled);
            let code = self.b.extract_value(selector, 1, "");
            let err = self.b.cast(llrm_mir::CastOp::Trunc, code, word, "");
            let erl = crate::onerror::erl(self.b, &handled, site, word);
            let (outlined, ty) = module.outlined;
            let resumed = self.b.call(ty, outlined, &[err, erl], "resumed").expect("an answer");
            let again = self.b.block("again");
            let next = self.b.block("next");
            let mut cases = vec![(self.b.int(16, RESUMED_AGAIN), again), (self.b.int(16, RESUMED_NEXT), next)];
            if module.owner == self.function.id {
                cases.extend(module.labels.iter().enumerate().map(|(k, &label)| (self.b.int(16, LABELS + k as i128), self.block(label))));
            }
            self.dispatch(resumed, &cases);
            // Each switched on the site once every site is known.
            let mut resumes = Vec::new();
            for (block, form) in [(again, Resume::Again), (next, Resume::Next)] {
                self.b.position(block);
                self.b.unreachable();
                resumes.push((block, form));
            }
            let inside = if module.owner == self.function.id { module.only.clone() } else { BTreeSet::new() };
            let served = module.owner != self.function.id;
            Handling { handled, handlers: module.handlers, active: module.active, pad, selector, site, statements, inside, raising: BTreeSet::new(), resumes, labels: module.labels, served, handling: false }
        };
        self.handling = Some(handling);
        self.b.position(entry);
        Ok(())
    }

    /// A switch on `value` to `cases`, and nowhere else.
    fn dispatch(&mut self, value: Value, cases: &[(Value, BlockId)]) {
        let nowhere = self.b.block("");
        self.b.switch(value, nowhere, cases);
        self.b.position(nowhere);
        self.b.unreachable();
    }

    /// The module handler as its own function: an entry that goes to the
    /// handler ON ERROR GOTO named last, then the handlers' blocks.
    pub(super) fn outline(&mut self, handler: ModuleHandler, handled: Handled) -> Emit<()> {
        let blocks: Vec<&model::Block> = super::emission_order(self.function).into_iter().filter(|one| handler.inside.contains(&one.id)).collect();
        let entry = self.b.block("entry");
        for block in &blocks {
            let id = self.b.block(&format!("b{}", block.id));
            self.blocks.insert(block.id, id);
        }
        self.b.position(entry);
        self.allocate()?;
        let word = self.b.context.types.int(16);
        let (err, erl) = (self.b.parameter(0), self.b.parameter(1));
        self.b.store(erl, handler.last_erl, false);
        let number = self.b.load(word, handler.active, false, "");
        let cases: Vec<(Value, BlockId)> = handler.handlers.iter().enumerate().map(|(at, &one)| (self.b.int(16, at as i128 + 1), self.block(one))).collect();
        self.dispatch(number, &cases);
        self.outlined = Some(Outlined { handler, handled, err, erl });
        for block in blocks {
            self.emit_block(block)?;
        }
        Ok(())
    }

    /// Notes whether `block` is the handler's.
    pub(super) fn enter_block(&mut self, block: i64) {
        if let Some(handling) = &mut self.handling {
            handling.handling = handling.inside.contains(&block);
        }
    }

    /// A call of `callee` this owns in the module handler's own function.
    fn outlined_call(&mut self, callee: &str, returns: TypeId) -> Emit<Option<Option<Value>>> {
        let outlined = self.outlined.as_ref().expect("the module handler");
        if let Some(registered) = callee.strip_prefix(REGISTER) {
            let (handler, _) = registered.split_once(':').ok_or("an ON ERROR marker without its scope")?;
            let handler: i64 = handler.parse().map_err(|_| "an ON ERROR marker without its handler")?;
            let number = if handler == 0 { 0 } else { outlined.handler.handlers.iter().position(|&one| one == handler).ok_or("a handler its body names none")? + 1 };
            crate::onerror::goto(self.b, &outlined.handled, outlined.handler.active, number as u16, true)?;
            return Ok(Some(None));
        }
        let word = self.b.context.types.int(16);
        let widened = |b: &mut llrm_mir::build::Builder, value: Value, op| if returns == word { value } else { b.cast(op, value, returns, "") };
        let answer = match callee {
            ERR => return Ok(Some(Some(widened(self.b, outlined.err, llrm_mir::CastOp::SExt)))),
            ERL => return Ok(Some(Some(widened(self.b, outlined.erl, llrm_mir::CastOp::ZExt)))),
            NEXT => RESUMED_NEXT,
            AGAIN => RESUMED_AGAIN,
            _ => match callee.strip_prefix(LABEL) {
                Some(label) => {
                    let label: i64 = label.parse().map_err(|_| "a RESUME marker without its label")?;
                    LABELS + outlined.handler.labels.iter().position(|&one| one == label).expect("each label") as i128
                }
                None => return Ok(None),
            },
        };
        crate::onerror::resuming(self.b, &outlined.handled);
        if outlined.handler.resume_clears_erl {
            let none = self.b.int(16, 0);
            self.b.store(none, outlined.handler.last_erl, false);
        }
        let answer = self.b.int(16, answer);
        self.b.ret(Some(answer));
        // What the block holds past its RESUME runs nowhere.
        let after = self.b.block("");
        self.b.position(after);
        Ok(Some(None))
    }

    /// Whether a RESUME or RESUME NEXT of `callee` is reached by falling
    /// through into a handler's code, where no error is being handled.
    pub(super) fn fallen_resume(&self, callee: &str) -> bool {
        resumes(callee) && self.outlined.is_none() && self.handling.as_ref().is_some_and(|one| !one.handling)
    }

    /// A call of `callee` this owns, or `None`.
    pub(super) fn handling_call(&mut self, callee: &str, returns: TypeId) -> Emit<Option<Option<Value>>> {
        if self.outlined.is_some() {
            return self.outlined_call(callee, returns);
        }
        if callee == ERL && !self.handling.as_ref().is_some_and(|one| one.handling) {
            // The runtime's ERL knows no line of the recompiled code: the
            // module handler keeps the last one it took.
            let Some(module) = &self.tables.module_handler else { return Err("ERL outside the error handler".to_owned()) };
            let word = self.b.context.types.int(16);
            let line = self.b.load(word, module.last_erl, false, "erl");
            return Ok(Some(Some(if returns == word { line } else { self.b.cast(llrm_mir::CastOp::ZExt, line, returns, "") })));
        }
        if let Some(registered) = callee.strip_prefix(REGISTER) {
            let (handler, scope) = registered.split_once(':').ok_or("an ON ERROR marker without its scope")?;
            if (scope == "L") != self.function.error_handler_local {
                return Err("ON ERROR and ON LOCAL ERROR in one procedure".to_owned());
            }
            let handler: i64 = handler.parse().map_err(|_| "an ON ERROR marker without its handler")?;
            let handling = self.handling.as_ref().filter(|one| !one.served).ok_or("an ON ERROR GOTO in a SUB, whose errors are not selected yet")?;
            let number = if handler == 0 { 0 } else { handling.handlers.iter().position(|&one| one == handler).expect("each handler") + 1 };
            crate::onerror::goto(self.b, &handling.handled, handling.active, number as u16, handling.handling)?;
            return Ok(Some(None));
        }
        let Some(handling) = self.handling.as_mut() else {
            return if [NEXT, AGAIN].contains(&callee) || callee.starts_with(LABEL) { Err("a RESUME without an error handler".to_owned()) } else { Ok(None) };
        };
        if callee == ERR && handling.handling {
            let selector = self.b.extract_value(handling.selector, 1, "");
            return Ok(Some(Some(self.b.cast(llrm_mir::CastOp::Trunc, selector, returns, ""))));
        }
        if callee == ERL {
            return Ok(Some(Some(crate::onerror::erl(self.b, &handling.handled, handling.site, returns))));
        }
        let form = match callee {
            NEXT => Some(Resume::Next),
            AGAIN => Some(Resume::Again),
            _ if callee.starts_with(LABEL) => None,
            _ => return Ok(None),
        };
        if !handling.handling {
            return Err("a RESUME label outside the error handler".to_owned());
        }
        crate::onerror::resuming(self.b, &handling.handled);
        let Some(form) = form else {
            let label: i64 = callee[LABEL.len()..].parse().map_err(|_| "a RESUME marker without its label")?;
            let target = self.block(label);
            self.b.br(target);
            return Ok(Some(None));
        };
        // The block's own unreachable follows; the switch replaces it once
        // every site is known.
        handling.resumes.push((self.b.current().expect("a placed block"), form));
        Ok(Some(None))
    }

    /// A call, an invoke to the pad where it may raise outside the handler;
    /// `attributes` what it states of its arguments, by index.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn raising_call(&mut self, instruction: i64, raises: bool, convention: u32, ty: TypeId, callee: Value, arguments: &[Value], attributes: &[(usize, Attribute)]) -> Emit<Option<Value>> {
        let Some(handling) = self.handling.as_mut().filter(|one| raises && !one.handling) else {
            let answered = self.b.call_as(convention, ty, callee, arguments, "");
            self.attributed(attributes);
            return Ok(answered);
        };
        let site = handling.statements.partition_point(|one| one.instruction <= instruction).checked_sub(1).ok_or("a call before the first statement")?;
        handling.raising.insert(site);
        let (pad, pointer) = (handling.pad, handling.site);
        let number = self.b.int(16, site as i128);
        self.b.store(number, pointer, false);
        let next = self.b.block("");
        let answered = self.b.invoke_as(convention, ty, callee, arguments, next, pad, "");
        self.attributed(attributes);
        self.b.position(next);
        Ok(answered)
    }

    fn attributed(&mut self, attributes: &[(usize, Attribute)]) {
        for (index, attribute) in attributes {
            self.b.argument_attr(*index, attribute.clone());
        }
    }

    /// Ends each RESUME NEXT and RESUME with its switch on the site.
    pub(super) fn close_handling(&mut self) -> Emit<()> {
        let Some(handling) = self.handling.take() else { return Ok(()) };
        if handling.served && !handling.raising.is_empty() && !handling.labels.is_empty() {
            return Err("an error it raises, which the module handler may RESUME at a label of the module body: the runtime unwinds this frame for that, which is not selected yet".to_owned());
        }
        let word = self.b.context.types.int(16);
        for &(block, form) in &handling.resumes {
            let placeholder = self.b.function.terminator(block).ok_or("a RESUME's block without its unreachable")?;
            self.b.function.erase(placeholder)?;
            self.b.position(block);
            let site = self.b.load(word, handling.site, false, "");
            let mut cases = Vec::new();
            for &raising in &handling.raising {
                // The last statement is the body's end, where RESUME NEXT after
                // the last one continues: an error its own exit raises (a
                // string freed, an array erased) resumes there too.
                let index = if form == Resume::Next { raising + 1 } else { raising };
                let statement = handling.statements.get(index.min(handling.statements.len().saturating_sub(1))).ok_or("a RESUME in a body with no statement")?;
                if handling.inside.contains(&statement.block) {
                    return Err(format!("a RESUME into block {}, the error handler's", statement.block));
                }
                cases.push((self.b.int(16, raising as i128), self.block(statement.block)));
            }
            let nowhere = self.b.block("");
            self.b.position(nowhere);
            self.b.unreachable();
            self.b.position(block);
            self.b.switch(site, nowhere, &cases);
        }
        Ok(())
    }
}
