//! ON ERROR: the module's handler runs in the main body's function, entered
//! at its landing pad (`llrm_hir::onerror`). Each call that may raise is an
//! invoke, after storing its statement's number; each statement starts a
//! block, where RESUME continues: RESUME NEXT at the statement after the
//! faulting one, RESUME at that statement, RESUME label at the label.

use super::*;

/// The ON ERROR routines whose meaning this owns.
pub const REGISTER: &str = "B$OEGA";
pub const ERR: &str = "B$FERR";
pub const ERL: &str = "B$FERL";
pub const NEXT: &str = "B$RESN";
pub const AGAIN: &str = "B$RES0";
pub const LABEL: &str = "B$RESA";

pub fn owns(name: &str) -> bool {
    [REGISTER, ERR, ERL, NEXT, AGAIN, LABEL].contains(&name)
}

/// Where RESUME continues, relative to the faulting statement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Resume {
    Next,
    Again,
}

pub(super) struct Handling {
    handled: Handled,
    /// Whether ON ERROR GOTO last named the handler: 1, or 0 for none.
    active: Operand,
    pad: BlockId,
    selector: Operand,
    site: Operand,
    /// Every statement's start, in address order: a site is an index.
    statements: Vec<usize>,
    /// The block each statement or RESUME label of the body proper starts,
    /// by its start.
    rows: BTreeMap<usize, BlockId>,
    placed: BTreeSet<usize>,
    /// The sites a call may raise at.
    raising: BTreeSet<usize>,
    /// Each RESUME's block and form, switched once every site is known.
    resumes: Vec<(BlockId, Resume)>,
    /// Statements a RESUME label continues at.
    labels: BTreeSet<usize>,
    /// Whether the block being emitted is the handler's.
    pub(super) inside: bool,
}

impl Emitter<'_, '_, '_> {
    /// Makes the pad and a block per statement; the entry block is current.
    pub(super) fn handle(&mut self, handled: Handled) {
        let handler = self.body.handler.as_ref().expect("a handler");
        let word = self.b.context.types.int(16);
        let site = self.b.alloca(word, "site");
        let active = llrm_hir::onerror::active(&mut self.b);
        let statements: Vec<usize> = self.unit.facts.statements.iter().map(|&(at, _)| at).collect();
        let starts: BTreeSet<usize> =
            self.body.blocks.iter().filter(|block| !handler.blocks.contains(&block.at)).flat_map(|block| block.insns.iter().map(|insn| insn.at)).collect();
        // RESUME label continues at its label, which starts no statement where
        // /V polls for events there first.
        let labels = self.unit.facts.resumptions.values().map(|&label| label as usize);
        let resumed: BTreeSet<usize> = statements.iter().copied().chain(labels).filter(|at| starts.contains(at)).collect();
        let rows = resumed.into_iter().map(|at| (at, self.b.block(&format!("s{at:04x}")))).collect();
        let pad = self.b.block("landing");
        self.b.position(pad);
        let null = self.b.context.types.ptr(0);
        let null = Operand::Constant(self.b.context.constant(Constant { ty: null, kind: ConstantKind::Null }));
        let selector = self.b.landing_pad(handled.pad, &[null], "landed");
        llrm_hir::onerror::landed(&mut self.b, &handled);
        self.handling = Some(Handling {
            handled,
            active,
            pad,
            selector,
            site,
            statements,
            rows,
            placed: BTreeSet::new(),
            raising: BTreeSet::new(),
            resumes: Vec::new(),
            labels: BTreeSet::new(),
            inside: false,
        });
    }

    /// The pad goes to the handler's seed, with nothing the body left in a
    /// register: the runtime enters it from wherever the error was raised.
    pub(super) fn enter_handler(&mut self) -> Emit<()> {
        let (Some(handling), Some(handler)) = (&self.handling, &self.body.handler) else { return Ok(()) };
        let (pad, seed) = (handling.pad, handler.seed);
        self.block = pad;
        self.b.position(pad);
        self.forget("the runtime enters the handler, which leaves it undefined");
        let target = self.enter(seed)?;
        self.b.position(pad);
        self.b.br(target);
        Ok(())
    }

    /// Every register, ES and flag undefined, as this block's end.
    fn forget(&mut self, why: &str) {
        self.current.clear();
        self.bits.clear();
        self.clobber(&TRACKED, why);
        let ty = self.var_type(Var::Es);
        let es = self.sentinel(ty, format!("reads es after {why}"));
        self.current.insert(Var::Es, es);
        for (bit, _) in BITS {
            self.bits.insert(bit, BitState::Unknown(format!("reads the flags after {why}")));
        }
        self.ends.insert(self.block, (self.current.clone(), self.bits.clone()));
    }

    /// Starts the statement at `at`'s block, if one starts there: RESUME
    /// may enter it, so it reads nothing the code before left in a register.
    pub(super) fn statement(&mut self, at: usize) -> Emit<()> {
        let Some(handling) = self.handling.as_mut().filter(|one| !one.inside) else { return Ok(()) };
        let Some(&block) = handling.rows.get(&at) else { return Ok(()) };
        handling.placed.insert(at);
        if self.depth != 0 || self.floats != 0 {
            return Err(format!("the statement at {at:#06x} starts with bytes pushed or values on the x87 stack"));
        }
        self.ends.insert(self.block, (self.current.clone(), self.bits.clone()));
        self.b.position(self.block);
        self.b.br(block);
        self.block = block;
        self.b.position(block);
        self.current.clear();
        self.bits.clear();
        self.pushes.clear();
        Ok(())
    }

    /// A call, an invoke to the pad where it may raise outside the handler,
    /// after storing its statement's number.
    pub fn call_as(&mut self, convention: u32, ty: TypeId, callee: ConstantId, arguments: &[Operand]) -> Emit<Option<Operand>> {
        let raises = self.unit.raising.contains(&callee);
        let Some(handling) = self.handling.as_mut().filter(|one| raises && !one.inside) else {
            return Ok(self.b.call_as(convention, ty, Operand::Constant(callee), arguments, ""));
        };
        let at = self.insn.as_ref().map(|insn| insn.at).ok_or("a call outside an instruction")?;
        let site = handling.statements.partition_point(|&start| start <= at).checked_sub(1).ok_or_else(|| format!("a call at {at:#06x} that may raise before the first statement"))?;
        handling.raising.insert(site);
        let (pad, pointer) = (handling.pad, handling.site);
        let number = self.b.int(16, site as i128);
        self.b.store(number, pointer, false);
        let next = self.b.block("");
        let answered = self.b.invoke_as(convention, ty, Operand::Constant(callee), arguments, next, pad, "");
        self.ends.insert(self.block, (self.current.clone(), self.bits.clone()));
        self.block = next;
        self.b.position(next);
        Ok(answered)
    }

    /// B$OEGA, B$FERR, B$FERL and the RESUMEs.
    pub(super) fn handling_call(&mut self, name: &str, at: usize) -> Emit<()> {
        let why = format!("{name} clobbers it");
        let disturbed = || -> Emit<Vec<Register>> {
            let contract = self.unit.facts.contract(at).filter(|one| one.established).ok_or_else(|| format!("{name}'s contract is not established"))?;
            Ok(llrm_bcmachine::abi::runtime::disturbs(contract).into_iter().filter_map(crate::machine::from_contract).filter(|&one| one != crate::machine::FLAGS).collect())
        };
        match name {
            REGISTER => {
                let handler = self.unit.facts.registrations.get(&(at as i64)).copied();
                let offset = self.stack_word(self.depth, 2)?;
                let enabled = match (handler, self.constant(offset)) {
                    (Some(entry), _) if self.body.handler.as_ref().is_some_and(|one| one.seed as i64 == entry) => true,
                    (None, Some(0)) => false,
                    _ => return Err("an ON ERROR GOTO whose handler is not the module's one".to_owned()),
                };
                let handling = self.handling.as_ref().ok_or("an ON ERROR GOTO in a SUB, whose errors are not selected yet")?;
                let (handled, active, inside) = (handling.handled, handling.active, handling.inside);
                self.popped(4)?;
                llrm_hir::onerror::goto(&mut self.b, &handled, active, u16::from(enabled), inside)?;
                let disturbed = disturbed()?;
                self.clobber(&disturbed, &why);
                for (bit, _) in BITS {
                    self.bits.insert(bit, BitState::Unknown(format!("reads the flags {name} leaves")));
                }
                Ok(())
            }
            ERR => {
                let code = match self.handling.as_ref().filter(|one| one.inside).map(|one| one.selector) {
                    Some(selector) => {
                        let selector = self.b.extract_value(selector, 1, "");
                        let word = self.b.context.types.int(16);
                        self.cast(CastOp::Trunc, selector, word)
                    }
                    None => {
                        let (err, ty) = self.unit.err.ok_or("ERR undeclared")?;
                        self.b.call_as(llrm_mir::opcode::BASIC, ty, Operand::Constant(err), &[], "").expect("ERR")
                    }
                };
                let disturbed = disturbed()?;
                self.clobber(&disturbed, &why);
                self.set_register(Register::AX, code)
            }
            ERL => {
                // The runtime's ERL knows no line of the recompiled code.
                let handling = self.handling.as_ref().filter(|one| one.inside).ok_or("ERL outside the error handler")?;
                let (handled, site) = (handling.handled, handling.site);
                let word = self.b.context.types.int(16);
                // A LONG in DX:AX, whose high word a line never reaches.
                let line = llrm_hir::onerror::erl(&mut self.b, &handled, site, word);
                self.clobber(&disturbed()?, &why);
                let zero = self.b.int(16, 0);
                self.set_register(Register::AX, line)?;
                self.set_register(Register::DX, zero)
            }
            NEXT | AGAIN | LABEL => {
                let handling = self.handling.as_mut().filter(|one| one.inside).ok_or("a RESUME outside the error handler")?;
                llrm_hir::onerror::resuming(&mut self.b, &handling.handled);
                let target = if name == LABEL {
                    let label = *self.unit.facts.resumptions.get(&(at as i64)).ok_or("a RESUME whose label is not a constant")? as usize;
                    let target = *handling.rows.get(&label).ok_or_else(|| format!("a RESUME to {label:#06x}, which starts no statement of the body"))?;
                    handling.labels.insert(label);
                    Some(target)
                } else {
                    handling.resumes.push((self.block, if name == NEXT { Resume::Next } else { Resume::Again }));
                    None
                };
                self.forget("the runtime resumes, which leaves it undefined");
                match target {
                    Some(target) => self.b.br(target),
                    // Switched on the site once every site is known.
                    None => self.b.unreachable(),
                }
                Ok(())
            }
            _ => unreachable!("an ON ERROR routine"),
        }
    }

    /// Ends each RESUME with its switch on the site, and refuses a statement
    /// RESUME must reach that the raise did not place.
    pub(super) fn close_handling(&mut self) -> Emit<()> {
        let Some(handling) = self.handling.take() else { return Ok(()) };
        let word = self.b.context.types.int(16);
        let mut wanted: BTreeSet<usize> = handling.labels.clone();
        for &(block, form) in &handling.resumes {
            let placeholder = self.b.function.terminator(block).expect("a RESUME's unreachable");
            self.b.function.erase(placeholder)?;
            self.b.position(block);
            let site = self.b.load(word, handling.site, false, "");
            let mut cases = Vec::new();
            for &raising in &handling.raising {
                let index = if form == Resume::Next { raising + 1 } else { raising };
                let &start = handling.statements.get(index).ok_or("a RESUME NEXT past the last statement")?;
                let &target = handling.rows.get(&start).ok_or_else(|| format!("a RESUME to {start:#06x}, which starts no statement of the body"))?;
                wanted.insert(start);
                cases.push((self.b.int(16, raising as i128), target));
            }
            let nowhere = self.b.block("");
            self.b.position(nowhere);
            self.b.unreachable();
            self.b.position(block);
            self.b.switch(site, nowhere, &cases);
        }
        if let Some(start) = wanted.difference(&handling.placed).next() {
            return Err(format!("the statement at {start:#06x} RESUME continues at starts inside what the raise reads as one operation"));
        }
        for (start, &block) in &handling.rows {
            if !handling.placed.contains(start) {
                self.b.position(block);
                self.b.unreachable();
            }
        }
        Ok(())
    }
}
