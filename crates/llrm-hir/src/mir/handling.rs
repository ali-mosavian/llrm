//! ON ERROR: the function's handler is entered at its landing pad
//! (`crate::onerror`). Each call that may raise outside the handler is an
//! invoke, after storing its statement's number; RESUME NEXT and RESUME
//! switch on that number to the statement after or the statement itself,
//! RESUME label goes to the label.

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
    handler: i64,
    pad: BlockId,
    selector: Value,
    site: Value,
    /// The function's statements, in source order: a site is an index.
    statements: Vec<Statement>,
    /// The handler's blocks: what the pad reaches before a RESUME.
    inside: BTreeSet<i64>,
    raising: BTreeSet<usize>,
    resumes: Vec<(BlockId, Resume)>,
    /// Whether the block being emitted is the handler's.
    pub(super) handling: bool,
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
        let handler = self.function.error_handler.expect("a handler");
        let inside = reached(self.function, [handler]);
        let body = reached(self.function, std::iter::once(self.function.entry).chain(self.function.external_entries.iter().copied().filter(|one| !inside.contains(one))));
        if let Some(shared) = inside.intersection(&body).next() {
            return Err(format!("block {shared}, which both the body and its error handler run"));
        }
        let word = self.b.context.types.int(16);
        let site = self.b.alloca(word, "site");
        let mut statements: Vec<Statement> = statements.iter().copied().filter(|one| one.function == self.function.id).collect();
        statements.sort_by_key(|one| one.instruction);
        let pad = self.b.block("landing");
        self.b.position(pad);
        let null = self.b.context.types.ptr(0);
        let null = Value::Constant(self.b.context.constant(Constant { ty: null, kind: ConstantKind::Null }));
        let selector = self.b.landing_pad(handled.pad, &[null], "landed");
        self.b.br(self.block(handler));
        self.handling = Some(Handling { handled, handler, pad, selector, site, statements, inside, raising: BTreeSet::new(), resumes: Vec::new(), handling: false });
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
        if callee == ERL {
            return Err("ERL: the recompiled object keeps no line table to answer it from, which is not selected yet".to_owned());
        }
        if let Some(registered) = callee.strip_prefix(REGISTER) {
            let (handler, scope) = registered.split_once(':').ok_or("an ON ERROR marker without its scope")?;
            if scope != "G" {
                return Err("ON LOCAL ERROR, whose handler is a procedure's, which is not selected yet".to_owned());
            }
            let handler: i64 = handler.parse().map_err(|_| "an ON ERROR marker without its handler")?;
            let handling = self.handling.as_ref().ok_or("an ON ERROR GOTO in a SUB, whose errors are not selected yet")?;
            let enabled = match handler {
                0 => false,
                one if one == handling.handler => true,
                _ => return Err("a second ON ERROR GOTO handler: one per module is selected".to_owned()),
            };
            let handled = handling.handled;
            let flag = self.b.int(1, i128::from(enabled));
            self.b.call_as(llrm_mir::opcode::BASIC, handled.onerror_type, Value::Constant(handled.onerror), &[flag], "");
            return Ok(Some(None));
        }
        let Some(handling) = self.handling.as_mut() else {
            return if [NEXT, AGAIN].contains(&callee) || callee.starts_with(LABEL) { Err("a RESUME without an error handler".to_owned()) } else { Ok(None) };
        };
        if callee == ERR && handling.handling {
            let selector = self.b.extract_value(handling.selector, 1, "");
            return Ok(Some(Some(self.b.cast(llrm_mir::CastOp::Trunc, selector, returns, ""))));
        }
        if let Some(label) = callee.strip_prefix(LABEL) {
            let label: i64 = label.parse().map_err(|_| "a RESUME marker without its label")?;
            let target = self.block(label);
            self.b.br(target);
            return Ok(Some(None));
        }
        let form = match callee {
            NEXT => Resume::Next,
            AGAIN => Resume::Again,
            _ => return Ok(None),
        };
        if !handling.handling {
            return Err("a RESUME outside the error handler".to_owned());
        }
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
