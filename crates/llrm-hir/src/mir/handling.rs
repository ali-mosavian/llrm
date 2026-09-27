//! ON ERROR: the function's handlers are entered at its landing pad
//! (`crate::onerror`), which goes to the one ON ERROR GOTO last named. Each
//! call that may raise outside the handlers is an invoke, after storing its
//! statement's number; RESUME NEXT and RESUME switch on that number to the
//! statement after or the statement itself, RESUME label goes to the label.

use std::collections::{BTreeSet, HashMap};

use llrm_mir::{BlockId, Constant, ConstantKind, Operand as Value, TypeId};

use super::{Body, Emit};
use crate::model::{self, Statement};
use crate::onerror::Handled;

const REGISTER: &str = "$QB$OERG:";
const LABEL: &str = "$QB$RESA:";
const NEXT: &str = "B$RESN";
const AGAIN: &str = "B$RES0";
const ERR: &str = "B$FERR";
const ERL: &str = "B$FERL";

/// Whether this owns what a call of `callee` means.
pub(super) fn owns(callee: &str) -> bool {
    callee.starts_with(REGISTER) || callee.starts_with(LABEL) || [NEXT, AGAIN, ERL].contains(&callee)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Resume {
    Next,
    Again,
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
    /// Whether the block being emitted is a handler's.
    pub(super) handling: bool,
}

/// `function`'s statements, in source order: a site is an index.
pub(super) fn numbered(statements: &[Statement], function: i64) -> Vec<Statement> {
    let mut numbered: Vec<Statement> = statements.iter().copied().filter(|one| one.function == function).collect();
    numbered.sort_by_key(|one| one.instruction);
    numbered
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
    /// Makes the pad; the entry block is current.
    pub(super) fn handle(&mut self, handled: Handled, statements: &[Statement]) -> Emit<()> {
        let mut handlers: Vec<i64> = self.function.error_handler.into_iter().collect();
        for instruction in self.function.blocks.iter().flat_map(|block| &block.instructions) {
            let Some((handler, _)) = instruction.callee.as_deref().and_then(|one| one.strip_prefix(REGISTER)).and_then(|one| one.split_once(':')) else { continue };
            let handler: i64 = handler.parse().map_err(|_| "an ON ERROR marker without its handler")?;
            if handler != 0 && !handlers.contains(&handler) {
                handlers.push(handler);
            }
        }
        let inside = reached(self.function, handlers.iter().copied());
        let body = reached(self.function, std::iter::once(self.function.entry).chain(self.function.external_entries.iter().copied().filter(|one| !inside.contains(one))));
        if let Some(shared) = inside.intersection(&body).next() {
            return Err(format!("block {shared}, which both the body and its error handler run"));
        }
        let word = self.b.context.types.int(16);
        let site = self.b.alloca(word, "site");
        let active = crate::onerror::active(self.b);
        let statements = numbered(statements, self.function.id);
        let pad = self.b.block("landing");
        self.b.position(pad);
        let null = self.b.context.types.ptr(0);
        let null = Value::Constant(self.b.context.constant(Constant { ty: null, kind: ConstantKind::Null }));
        let selector = self.b.landing_pad(handled.pad, &[null], "landed");
        crate::onerror::landed(self.b, &handled);
        match handlers[..] {
            [only] => self.b.br(self.block(only)),
            _ => {
                let number = self.b.load(word, active, false, "");
                let cases: Vec<(Value, BlockId)> = handlers.iter().enumerate().map(|(at, &one)| (self.b.int(16, at as i128 + 1), self.block(one))).collect();
                let nowhere = self.b.block("");
                self.b.switch(number, nowhere, &cases);
                self.b.position(nowhere);
                self.b.unreachable();
            }
        }
        self.handling = Some(Handling { handled, handlers, active, pad, selector, site, statements, inside, raising: BTreeSet::new(), resumes: Vec::new(), handling: false });
        Ok(())
    }

    /// Notes whether `block` is the handler's.
    pub(super) fn enter_block(&mut self, block: i64) {
        if let Some(handling) = &mut self.handling {
            handling.handling = handling.inside.contains(&block);
        }
    }

    /// A call of `callee` this owns, or `None`.
    pub(super) fn handling_call(&mut self, callee: &str, returns: TypeId) -> Emit<Option<Option<Value>>> {
        if callee == ERL && !self.handling.as_ref().is_some_and(|one| one.handling) {
            // The runtime's ERL knows no line of the recompiled code.
            return Err("ERL outside the error handler".to_owned());
        }
        if let Some(registered) = callee.strip_prefix(REGISTER) {
            let (handler, scope) = registered.split_once(':').ok_or("an ON ERROR marker without its scope")?;
            if scope != "G" {
                return Err("ON LOCAL ERROR, whose handler is a procedure's, which is not selected yet".to_owned());
            }
            let handler: i64 = handler.parse().map_err(|_| "an ON ERROR marker without its handler")?;
            let handling = self.handling.as_ref().ok_or("an ON ERROR GOTO in a SUB, whose errors are not selected yet")?;
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
            return Err("a RESUME outside the error handler".to_owned());
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

    /// A call, an invoke to the pad where it may raise outside the handler.
    pub(super) fn raising_call(&mut self, instruction: i64, raises: bool, convention: u32, ty: TypeId, callee: Value, arguments: &[Value]) -> Emit<Option<Value>> {
        let Some(handling) = self.handling.as_mut().filter(|one| raises && !one.handling) else {
            return Ok(self.b.call_as(convention, ty, callee, arguments, ""));
        };
        let site = handling.statements.partition_point(|one| one.instruction <= instruction).checked_sub(1).ok_or("a call before the first statement")?;
        handling.raising.insert(site);
        let (pad, pointer) = (handling.pad, handling.site);
        let number = self.b.int(16, site as i128);
        self.b.store(number, pointer, false);
        let next = self.b.block("");
        let answered = self.b.invoke_as(convention, ty, callee, arguments, next, pad, "");
        self.b.position(next);
        Ok(answered)
    }

    /// Ends each RESUME NEXT and RESUME with its switch on the site.
    pub(super) fn close_handling(&mut self) -> Emit<()> {
        let Some(handling) = self.handling.take() else { return Ok(()) };
        let word = self.b.context.types.int(16);
        for &(block, form) in &handling.resumes {
            let placeholder = self.b.function.terminator(block).ok_or("a RESUME's block without its unreachable")?;
            self.b.function.erase(placeholder)?;
            self.b.position(block);
            let site = self.b.load(word, handling.site, false, "");
            let mut cases = Vec::new();
            for &raising in &handling.raising {
                let index = if form == Resume::Next { raising + 1 } else { raising };
                let statement = handling.statements.get(index).ok_or("a RESUME NEXT past the last statement")?;
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
