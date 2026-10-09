//! Port of `qbopt/backend/peephole.py`: simplifications that depend on the
//! final physical register assignment.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

use iced_x86::{FlowControl, Register, RflagsBits};
use llrm_x86_m16::instructions;

use crate::analysis::dataflow::{self, Direction};
use crate::backend::classes::RegisterClasses;
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::frame::Frame;
pub use crate::backend::lanes::{Lane, Lanes};
use crate::backend::peep::{self, walk::Facts};
use crate::backend::{
    affine, copyprop, copysink, liveness, machinecse, machinedce, phielim, regthrash, select, sharedstores,
    spillforward, storecombine, target,
};
use crate::model::ir::{self, Address, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet};
use crate::support::hash::{IndexMap, IndexSet};
/// Python's `dict[int, set]` keyed by `id(one)`.
pub type DeadAfter = HashMap<usize, Lanes>;
/// Python's `Counter[int]`: read with a 0 default.
pub type Counter = IndexMap<u32, i64>;

/// Python `id(one)` on an LIR instruction.
pub fn id(one: &Arc<Insn>) -> usize {
    Arc::as_ptr(one) as usize
}

fn count(
    counter: &Counter,
    value: u32,
) -> i64 {
    counter.get(&value).copied().unwrap_or(0)
}

fn emit(
    bits: u32,
    what: &Semantics,
) -> Option<select::Emitted> {
    select::priced_in(bits, what, 0, None, false, false, None)
}

fn semantics(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

fn with_what(
    one: &Insn,
    what: Semantics,
) -> Insn {
    Insn { what: Some(what), ..one.clone() }
}

fn deduped<T: Clone + Eq + std::hash::Hash>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    items.into_iter().collect::<IndexSet<T>>().into_iter().collect()
}

fn imm(
    value: i64,
    width: u32,
) -> Imm {
    Imm { value, width, address: None }
}

fn reg(
    register: Register,
    width: u32,
) -> Reg {
    Reg { register, width }
}

fn full32(register: Register) -> Register {
    register.full_register32()
}

pub struct Peephole {
    /// One frame, shared with the phases before this one, as Python shares it.
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub cpu: Profile,
    /// The rule groups of the target, which its driver binds.
    pub rules: &'static peep::Rules,
    /// The registers a callee keeps for its caller, whole: a run of stores may
    /// not share them.
    pub saved: Vec<Register>,
    pub classes: Rc<RegisterClasses>,
}

impl Peephole {
    /// With the 16-bit x86 rules: what the tests of this crate are written for.
    #[cfg(test)]
    pub fn new<'a>(
        frame: Option<Rc<RefCell<Frame>>>,
        cpu: impl Into<ProfileOrName<'a>>,
    ) -> Result<Self, String> {
        Self::with_rules(
            frame,
            cpu,
            &peep::targets::x86_m16::RULES,
            llrm_x86_m16::PRESERVED.iter().map(|(whole, _)| *whole).collect(),
            RegisterClasses::m16(),
        )
    }

    pub fn with_rules<'a>(
        frame: Option<Rc<RefCell<Frame>>>,
        cpu: impl Into<ProfileOrName<'a>>,
        rules: &'static peep::Rules,
        saved: Vec<Register>,
        classes: Rc<RegisterClasses>,
    ) -> Result<Self, String> {
        Ok(Self { frame, cpu: targets::profile(cpu)?.clone(), rules, saved, classes })
    }

    /// Drop only synthetic reservations when no added stack storage remains.
    fn _frame(
        &self,
        body: LirBody,
    ) -> LirBody {
        let Some(frame) = &self.frame else {
            return body;
        };
        let frame = frame.borrow();
        if frame.size() == 0 {
            return body;
        }
        for one in body.insns() {
            let Some(what) = &one.what else {
                return body;
            };
            if what.op == Operation::Barrier {
                return body;
            }
            for arg in what.sources.iter().chain(&what.dests) {
                let (address, through) = match arg {
                    Loc::Mem(cell) => (cell.addr, Some(cell.through)),
                    Loc::Address(cell) => (cell.addr, Some(cell.through)),
                    Loc::Imm(value) => (value.address, None),
                    _ => continue,
                };
                if address.is_none() && through.is_some() {
                    return body;
                }
                if through.is_some_and(|through| {
                    [Register::BP, Register::EBP, Register::SP, Register::ESP].contains(&through)
                }) && address.is_none_or(|address| address.space != Space::Frame)
                {
                    return body;
                }
                if address.is_some_and(|address| address.space == Space::Frame && frame.spills_at(address.disp)) {
                    return body;
                }
            }
        }
        let blocks = body
            .blocks
            .iter()
            .map(|block| {
                block.with_insns(lir::without(
                    &block.insns,
                    |one| one.frame_adjust,
                    None::<fn(&Arc<Insn>) -> Arc<Insn>>,
                ))
            })
            .collect();
        LirBody { blocks, ..body }
    }
}

impl LIRTransform for Peephole {
    fn class_name(&self) -> &'static str {
        "Peephole"
    }

    fn name(&self) -> &str {
        "peephole"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        self.once(body)
    }
}

impl Peephole {
    fn once(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        // Before the rest: a thrashed copy is one fewer instruction for
        // everything below to reason about, and it is the only pass here
        // that can remove a copy the coalescer refused on colourability.
        let body = regthrash::thrashed(phielim::unsplit(&body));
        let body = frame_copies(&body, &self.cpu, &self.classes)?;
        let body = copyprop::forwarded(&body);
        let body = extensions(self.rules, &body);
        let body = copysink::sunk(&body);
        let body = crate::backend::postrasink::sunk(&body);
        let body = spillforward::forwarded(&body);
        let body = storecombine::combined(&body);
        let body = pushed_constants(self.rules, &body);
        let body = far_loads(
            self.rules,
            &fused(
                self.rules,
                &overwritten(&shuttles(
                    self.rules,
                    &restored_copies(
                        self.rules,
                        &high_extracts(
                            &transferred(
                                self.rules,
                                &commuted(self.rules, &constants(&pushes(self.rules, &body, &self.cpu))),
                            ),
                            &self.cpu,
                        )?,
                    ),
                )),
            ),
        );
        let body = crate::backend::exactaddress::exact_addresses(&body, &self.cpu)?;
        let body = addresses(&body, &self.cpu)?;
        let body = secondary_bases(&body, &self.cpu)?;
        let body = borrows(self.rules, &increments(self.rules, &body));
        let body = doubled(self.rules, &body, &self.cpu)?;
        let body = narrowed_arithmetic(self.rules, &body);
        // A zero upper half proven by the first is what the second reads, and
        // the copies it makes are forwarded.
        let body = if self.rules.zero_extensions.is_some() {
            copyprop::forwarded(&zero_extensions(self.rules, &widened_moves(self.rules, &body)))
        } else {
            body
        };
        let body = sharedstores::shared(&body, &self.cpu, &self.saved, &self.classes);
        let body = machinecse::eliminated(&body)?;
        let body = waits(&zero_compares(self.rules, &tested(self.rules, &zeroes(&narrowed_moves(self.rules, &body)))));
        let body = popped_arguments(&machinedce::eliminated(body), &self.cpu)?;
        // Last: EBP zeroed above for each cell reading it 32 bits wide, where a
        // 32-bit address is the prefixed form of a 16-bit one. A native
        // one reads whole registers.
        let body = self._frame(body);
        let native = self.cpu.dword_address_form().is_some_and(|form| !form.secondary);
        Ok(if native { body } else { crate::backend::upperzero::established(&body) })
    }
}

/// A call's `add sp,2*n` as `n` pops into a dead scratch register, as BCC
/// -Os cleans them, where the CPU prices it (`Profile::pops_arguments`).
/// The add's flags must be dead: a pop sets none.
pub fn popped_arguments(
    body: &LirBody,
    cpu: &Profile,
) -> Result<LirBody, String> {
    const SCRATCH: [Register; 4] = [Register::CX, Register::DX, Register::BX, Register::AX];
    let arithmetic =
        RflagsBits::OF | RflagsBits::SF | RflagsBits::ZF | RflagsBits::AF | RflagsBits::CF | RflagsBits::PF;
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(body.bits, block, exits[&block.at].clone());
        let mut insns = Vec::with_capacity(block.insns.len());
        for one in &block.insns {
            let sp = Loc::Reg(reg(Register::SP, 2));
            let words = match &one.what {
                Some(what)
                    if what.op == Operation::Binary
                        && what.name.as_deref() == Some("add")
                        && what.dests == [sp.clone()] =>
                {
                    match what.sources.as_slice() {
                        [first, Loc::Imm(Imm { value, address: None, .. })]
                            if *first == sp && value % 2 == 0 && *value > 0 =>
                        {
                            value / 2
                        }
                        _ => 0,
                    }
                }
                _ => 0,
            };
            let dead = &dead_after[&id(one)];
            let scratch = SCRATCH.into_iter().find(|register| _lanes(*register).is_subset(dead));
            match scratch {
                Some(register)
                    if words > 0
                        && Lanes::flags(arithmetic).is_subset(dead)
                        && !one.frame_adjust
                        && cpu.pops_arguments(words)? =>
                {
                    let pop = || {
                        Arc::new(with_what(
                            one,
                            semantics(Operation::Pop, "pop", vec![Loc::Reg(reg(register, 2))], vec![]),
                        ))
                    };
                    insns.extend((0..words).map(|_| pop()));
                }
                _ => insns.push(Arc::clone(one)),
            }
        }
        blocks.push(block.with_insns(insns));
    }
    Ok(LirBody { blocks, ..body.clone() })
}

/// Use a dead GPR for an allocated frame-to-frame parallel copy.
///
/// Parallel-copy scheduling has to work even when every GPR is live, so its
/// unconditional spelling is a balanced PUSH-memory/POP-memory pair.  After
/// allocation, physical liveness can prove that a same-width register is
/// dead across a particular pair.  Two MOVs then avoid the temporary stack
/// traffic without changing flags, stack depth, or either frame address.
pub fn frame_copies<'a>(
    body: &LirBody,
    cpu: impl Into<ProfileOrName<'a>>,
    classes: &RegisterClasses,
) -> Result<LirBody, String> {
    let profile = targets::profile(cpu)?;
    let forms = ["push_m", "pop_m", "mov_rm", "mov_mr"];
    if !forms.iter().all(|form| profile.prices(form)) {
        return Ok(body.clone());
    }
    if profile.cost("mov_rm")? + profile.cost("mov_mr")? >= profile.cost("push_m")? + profile.cost("pop_m")? {
        return Ok(body.clone());
    }
    if !body.blocks.iter().any(|block| {
        block
            .insns
            .iter()
            .zip(block.insns.iter().skip(1))
            .any(
                |(first, second)| first.what.as_ref().is_some_and(|what| what.op == Operation::Push)
                    && second.what.as_ref().is_some_and(|what| what.op == Operation::Pop),
            )
    }) {
        // avoid whole-body liveness when no stack shuttle exists
        return Ok(body.clone());
    }

    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(body.bits, block, exits[&block.at].clone());
        let mut insns = block.insns.to_vec();
        for index in 0..insns.len().saturating_sub(1) {
            let (pushed, popped) = (Arc::clone(&insns[index]), Arc::clone(&insns[index + 1]));
            if [&pushed, &popped].into_iter().any(|one| {
                one.what.is_none()
                    || !one.clobbers.is_empty()
                    || !one.clobbers_high.is_empty()
                    || !one.requires.is_empty()
                    || !one.delivers.is_empty()
                    || !one.spread.is_empty()
                    || one.group.is_some()
                    || one.symbol == Some(true)
                    || one.frame_adjust
                    || one.spill_reload
                    || one.spill_store
            }) {
                continue;
            }
            if pushed.at != popped.at
                || popped.covers != Some((pushed.at, pushed.at))
                || popped.call.is_some()
                || !pushed.defines.is_empty()
                || !pushed.uses.is_empty()
                || !popped.defines.is_empty()
                || !popped.uses.is_empty()
            {
                continue; // only parcopy's synthetic balanced transfer
            }
            let (Some(first), Some(second)) = (&pushed.what, &popped.what) else {
                continue;
            };
            let (source, destination) = match (
                (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
                (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
            ) {
                (
                    (Operation::Push, Some("push"), [], [Loc::Mem(source)]),
                    (Operation::Pop, Some("pop"), [Loc::Mem(destination)], []),
                ) => {
                    if source.width != destination.width
                        || ![2, 4].contains(&source.width)
                        || !_frame_cell(source)
                        || !_frame_cell(destination)
                    {
                        continue;
                    }
                    (source.clone(), destination.clone())
                }
                _ => continue,
            };
            let scratch = classes
                .available
                .iter()
                .copied()
                .filter(|register| {
                    _lanes(target::named(*register, i64::from(source.width))).is_subset(&dead_after[&id(&popped)])
                })
                .map(|register| target::named(register, i64::from(source.width)))
                .next();
            let Some(scratch) = scratch else {
                continue;
            };
            let temporary = reg(scratch, source.width);
            let load = with_what(
                &pushed,
                semantics(Operation::Move, "mov", vec![Loc::Reg(temporary)], vec![Loc::Mem(source)]),
            );
            let store = with_what(
                &popped,
                semantics(Operation::Move, "mov", vec![Loc::Mem(destination)], vec![Loc::Reg(temporary)]),
            );
            if emit(body.bits, load.what.as_ref().expect("set above")).is_none()
                || emit(body.bits, store.what.as_ref().expect("set above")).is_none()
            {
                continue;
            }
            insns[index] = Arc::new(load);
            insns[index + 1] = Arc::new(store);
        }
        blocks.push(block.with_insns(insns));
    }
    Ok(body.with_blocks(blocks))
}

/// Fold a load or transitive extension into one widening instruction
/// (`peephole.peep`).
pub fn extensions(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.extensions, body, &Facts::new(body, None))
}

/// Materialize a call's literal once when both stack and register need it
/// (`peephole.peep`).
pub fn pushed_constants(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.pushed_constants, body, &Facts::new(body, None))
}

/// `insns` with `rewrite` offered each run of up to `width` instructions,
/// meta instructions skipped (LLVM's walk without debug instructions): a
/// marker between two instructions never stops a rewrite. `rewrite` answers
/// how many it consumed and what replaces them; markers inside a consumed
/// run follow the replacement.
fn _code_windows<E>(
    insns: &[Arc<Insn>],
    width: usize,
    mut rewrite: impl FnMut(&[Arc<Insn>]) -> Result<Option<(usize, Vec<Arc<Insn>>)>, E>,
) -> Result<Vec<Arc<Insn>>, E> {
    let code: Vec<usize> = (0..insns.len()).filter(|&at| !insns[at].is_meta()).collect();
    let only: Vec<Arc<Insn>> = code.iter().map(|&at| Arc::clone(&insns[at])).collect();
    let mut out = Vec::with_capacity(insns.len());
    let (mut next, mut at) = (0, 0);
    while at < code.len() {
        out.extend(insns[next..code[at]].iter().cloned());
        match rewrite(&only[at..(at + width).min(code.len())])? {
            Some((consumed, replacement)) => {
                let last = code[at + consumed - 1];
                out.extend(replacement);
                out.extend(insns[code[at] + 1..last].iter().filter(|one| one.is_meta()).cloned());
                next = last + 1;
                at += consumed;
            }
            None => {
                out.push(Arc::clone(&insns[code[at]]));
                next = code[at] + 1;
                at += 1;
            }
        }
    }
    out.extend(insns[next..].iter().cloned());
    Ok(out)
}

/// Two adjacent immediate word pushes have one dword's stack layout
/// (`peephole.peep`).
pub fn pushes(
    rules: &peep::Rules,
    body: &LirBody,
    cpu: &Profile,
) -> LirBody {
    peep::rewritten(rules.pushes, body, &Facts::new(body, Some(cpu)))
}

pub fn _lanes(register: Register) -> Lanes {
    if target::SEGMENTS.contains(&register) {
        let width = target::width_of(register).expect("a segment register has a width");
        return (0..width).map(|byte| (register, byte as u32)).collect();
    }
    let full = full32(register);
    if ![Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI, Register::EBP]
        .contains(&full)
    {
        return Lanes::new();
    }
    let start = u32::from([Register::AH, Register::BH, Register::CH, Register::DH].contains(&register));
    (start..start + register.size() as u32).map(|byte| (full, byte)).collect()
}

/// Write only the live low word of a register-only dword move
/// (`peephole.peep`).
pub fn narrowed_moves(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.narrowed_moves, body, &Facts::new(body, None))
}

/// Use a saved accumulator in place for commutative two-address operations
/// (`peephole.peep`).
pub fn commuted(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.commuted, body, &Facts::new(body, None))
}

/// Write a commutative result directly into its copied destination
/// (`peephole.peep`).
pub fn transferred(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.transferred, body, &Facts::new(body, None))
}

/// Select direct register or memory forms for a dword's high word.
///
/// A spilled value's high-half extraction can reach allocated LIR as
/// `mov R32,[slot]; shr R32,16`.  When only R16 survives, reading
/// `word [slot+2]` computes the same value without the shift.  The proof is
/// necessarily physical: the preserved upper register lanes and every flag
/// the shift would write must both be dead.
///
/// The portable register form is `push R32; pop dead16; pop high16`.  Once
/// registers are allocated, one `shld high32,R32,16` avoids all three stack
/// accesses when its wider destination lanes and flags are dead.  An already
/// selected `mov`/`shr` pair converges to that same final form.
pub fn high_extracts<'a>(
    body: &LirBody,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<LirBody, String> {
    let profile = targets::profile(cpu)?;
    let mut virtual_uses = Counter::default();
    for block in &body.blocks {
        for one in &block.insns {
            for value in &one.uses {
                *virtual_uses.entry(*value).or_insert(0) += 1;
            }
        }
    }
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(body.bits, block, exits[&block.at].clone());
        let insns = _code_windows(&block.insns, 3, |run| -> Result<_, String> {
            if run.len() == 3 {
                if let Some(changed) = _register_high_extract(body.bits, run, &dead_after, &virtual_uses, profile)? {
                    return Ok(Some((3, changed)));
                }
            }
            if run.len() < 2 {
                return Ok(None);
            }
            let pair = &run[..2];
            Ok(match _selected_register_high_extract(body.bits, pair, &dead_after, &virtual_uses, profile)? {
                Some(changed) => Some((2, changed)),
                None => _high_extract(body.bits, pair, &dead_after).map(|changed| (2, changed)),
            })
        })?;
        blocks.push(block.with_insns(insns));
    }
    Ok(body.with_blocks(blocks))
}

fn _register_high_extract(
    bits: u32,
    parts: &[Arc<Insn>],
    dead_after: &DeadAfter,
    virtual_uses: &Counter,
    cpu: &Profile,
) -> Result<Option<Vec<Arc<Insn>>>, String> {
    let (pushed, discarded, kept) = (&parts[0], &parts[1], &parts[2]);
    if parts.iter().any(|one| {
        one.what.is_none()
            || !one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
            || one.frame_adjust
            || one.spill_reload
            || one.spill_store
    }) {
        return Ok(None);
    }
    let (Some(first), Some(second), Some(third)) = (&pushed.what, &discarded.what, &kept.what) else {
        return Ok(None);
    };
    let (source, high) = match (
        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
        (third.op, third.name.as_deref(), third.dests.as_slice(), third.sources.as_slice()),
    ) {
        (
            (Operation::Push, Some("push"), [], [Loc::Reg(source @ Reg { width: 4, .. })]),
            (Operation::Pop, Some("pop"), [Loc::Reg(Reg { width: 2, .. })], []),
            (Operation::Pop, Some("pop"), [Loc::Reg(high @ Reg { width: 2, .. })], []),
        ) => (*source, *high),
        _ => return Ok(None),
    };
    if pushed.uses.len() != 1
        || discarded.defines.len() != 1
        || kept.defines.len() != 1
        || count(virtual_uses, discarded.defines[0]) != 0
    {
        return Ok(None);
    }
    let wide_high = reg(target::named(high.register, 4), 4);
    let count = Loc::Imm(imm(16, 1));
    let shift =
        semantics(Operation::Binary, "shr", vec![Loc::Reg(wide_high)], vec![Loc::Reg(wide_high), count.clone()]);

    let Some(effects) = _register_effects(bits, &with_what(kept, shift.clone()), false, true) else {
        return Ok(None);
    };
    let upper: Lanes = _lanes(wide_high.register).minus(&_lanes(high.register));
    let flags: Lanes = effects.1.and(&_flag_lanes(0xFFFF_FFFF));
    if !upper.or(&flags).is_subset(&dead_after[&id(kept)]) {
        return Ok(None);
    }
    let old_cost = cpu.cost("push_r")? + 2 * cpu.cost("pop_r")?;
    if ir::root(source.register) == ir::root(high.register) {
        // The low-half extraction has already copied the return value away.
        // Reusing the dying source root for its own high half needs no move.
        // Lanes outside HIGH must die here because SHR writes the whole root.
        if _lanes(source.register)
            .difference(&_lanes(high.register))
            .copied()
            .collect::<Lanes>()
            .is_subset(&dead_after[&id(kept)])
            && cpu.cost("shift_ri")? <= old_cost
            && emit(bits, &shift).is_some()
        {
            return Ok(Some(vec![Arc::new(Insn {
                what: Some(shift),
                defines: kept.defines.clone(),
                ..(**pushed).clone()
            })]));
        }
        return Ok(None);
    }
    // Only DX is part of the medium-model return ABI.  With EDX's upper lanes
    // dead, SHLD's otherwise-observable old-destination contribution is dead
    // too: EDX <- EDX<<16 | SOURCE>>16 puts SOURCE.high16 directly in DX.
    // SHRD would put SOURCE.low16 in EDX.high16 and leave DX unrelated.
    let extract = semantics(
        Operation::Funnel,
        "shld",
        vec![Loc::Reg(wide_high)],
        vec![Loc::Reg(wide_high), Loc::Reg(source), count],
    );
    if cpu.cost("shift_ri")? > old_cost || emit(bits, &extract).is_none() {
        return Ok(None);
    }
    Ok(Some(vec![Arc::new(Insn { what: Some(extract), defines: kept.defines.clone(), ..(**pushed).clone() })]))
}

/// Collapse an allocated `mov R32,S32; shr R32,16` to one SHLD.
///
/// Lowering may select this shape directly while a portable half extraction
/// reaches the same point as PUSH/POP/POP.  Both spellings implement the same
/// operation when only the destination's low word survives, so final machine
/// selection must canonicalize both independently of frontend provenance.
fn _selected_register_high_extract(
    bits: u32,
    parts: &[Arc<Insn>],
    dead_after: &DeadAfter,
    virtual_uses: &Counter,
    cpu: &Profile,
) -> Result<Option<Vec<Arc<Insn>>>, String> {
    let (moved, shift) = (&parts[0], &parts[1]);
    if parts.iter().any(|one| {
        one.what.is_none()
            || !one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
            || one.frame_adjust
            || one.spill_reload
            || one.spill_store
    }) {
        return Ok(None);
    }
    let (Some(first), Some(second)) = (&moved.what, &shift.what) else {
        return Ok(None);
    };
    let (destination, source) = match (
        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
    ) {
        (
            (
                Operation::Move,
                Some("mov"),
                [Loc::Reg(destination @ Reg { width: 4, .. })],
                [Loc::Reg(source @ Reg { width: 4, .. })],
            ),
            (
                Operation::Binary,
                Some("shr"),
                [Loc::Reg(shifted @ Reg { width: 4, .. })],
                [Loc::Reg(read @ Reg { width: 4, .. }), Loc::Imm(Imm { value: 16, width: 1, .. })],
            ),
        ) => {
            if destination != shifted || shifted != read {
                return Ok(None);
            }
            (*destination, *source)
        }
        _ => return Ok(None),
    };
    if ir::root(destination.register) == ir::root(source.register)
        || moved.defines.len() != 1
        || moved.uses.len() != 1
        || shift.defines.len() != 1
        || shift.uses.len() != 1
    {
        return Ok(None);
    }
    let shared_two_address = moved.defines == shift.defines && shift.defines == shift.uses;
    let temporary_chain = shift.uses == moved.defines && count(virtual_uses, moved.defines[0]) == 1;
    if !(shared_two_address || temporary_chain) {
        return Ok(None);
    }

    let count = Loc::Imm(imm(16, 1));
    let extract = semantics(
        Operation::Funnel,
        "shld",
        vec![Loc::Reg(destination)],
        vec![Loc::Reg(destination), Loc::Reg(source), count],
    );
    let old_effects = _register_effects(bits, shift, false, true);
    let new_effects = _register_effects(bits, &with_what(moved, extract.clone()), false, true);
    let (Some(old_effects), Some(new_effects)) = (old_effects, new_effects) else {
        return Ok(None);
    };
    let low = target::named(destination.register, 2);
    let upper: Lanes = _lanes(destination.register).minus(&_lanes(low));
    let flags: Lanes = old_effects
        .1
        .union(&new_effects.1)
        .copied()
        .collect::<Lanes>()
        .intersection(&_flag_lanes(0xFFFF_FFFF))
        .copied()
        .collect();
    if !upper.or(&flags).is_subset(&dead_after[&id(shift)]) {
        return Ok(None);
    }
    let new_cost = cpu.cost("shift_ri")?;
    let old_cost = cpu.cost("mov_rr")? + cpu.cost("shift_ri")?;
    if new_cost > old_cost {
        return Ok(None);
    }

    let combined = Arc::new(Insn { what: Some(extract), defines: shift.defines.clone(), ..(**moved).clone() });
    let folded = lir::without(
        &[combined, Arc::clone(shift)],
        |one| Arc::ptr_eq(one, shift),
        None::<fn(&Arc<Insn>) -> Arc<Insn>>,
    );
    Ok(Some(folded))
}

fn _high_extract(
    bits: u32,
    parts: &[Arc<Insn>],
    dead_after: &DeadAfter,
) -> Option<Vec<Arc<Insn>>> {
    let (load, shift) = (&parts[0], &parts[1]);
    if parts.iter().any(|one| {
        one.what.is_none()
            || !one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
            || one.frame_adjust
            || one.spill_store
    }) {
        return None;
    }
    // Only a synthetic reload may be narrowed.  A source memory operation can
    // be volatile, fault on bytes no longer read, or otherwise make its full
    // access width observable.
    if !load.inserted()
        || load.symbol != Some(false)
        || load.call.as_ref().is_none_or(|call| call.effects.reads || call.effects.writes)
    {
        return None;
    }
    let (first, second) = (load.what.as_ref()?, shift.what.as_ref()?);
    let (wide, cell) = match (
        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
    ) {
        (
            (Operation::Move, Some("mov"), [Loc::Reg(wide @ Reg { width: 4, .. })], [Loc::Mem(cell)]),
            (
                Operation::Binary,
                Some("shr"),
                [Loc::Reg(destination @ Reg { width: 4, .. })],
                [Loc::Reg(shifted @ Reg { width: 4, .. }), Loc::Imm(Imm { value: 16, .. })],
            ),
        ) if cell.width == 4 => {
            if destination != wide
                || shifted != wide
                || !_frame_cell(cell)
                || cell.addr.is_none()
                || cell.index.is_some()
                || cell.selector.is_some()
            {
                return None;
            }
            (*wide, cell.clone())
        }
        _ => return None,
    };

    let low = reg(target::named(wide.register, 2), 2);
    let preserved: Lanes = _lanes(wide.register).minus(&_lanes(low.register));
    let effects = _register_effects(bits, shift, false, true)?;
    let flags: Lanes = effects.1.and(&_flag_lanes(0xFFFF_FFFF));
    if !preserved.or(&flags).is_subset(&dead_after[&id(shift)]) {
        return None;
    }

    let address = cell.addr.expect("checked above");
    let high =
        Mem { addr: Some(ir::Addr { disp: address.disp + 2, ..address }), width: 2, offset: cell.offset + 2, ..cell };
    let what = semantics(Operation::Move, "mov", vec![Loc::Reg(low)], vec![Loc::Mem(high)]);

    emit(bits, &what)?;
    Some(vec![Arc::new(with_what(load, what)), lir::anchor(Arc::clone(shift))])
}

/// Do tied work in its source register when a copy restores the result
/// (`peephole.peep`).
pub fn shuttles(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.shuttles, body, &Facts::new(body, None))
}

/// Remove a synthetic save/restore when the source survives between them
/// (`peephole.peep`).
pub fn restored_copies(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.restored_copies, body, &Facts::new(body, None))
}

/// One allocated operand with aliases of `before` renamed to `after`.
pub fn _register_operand(
    one: &Loc,
    before: Register,
    after: Register,
) -> Loc {
    // `target.named(after, target.width_of(x))`: a width the target does not
    // know leaves `after` as named.
    let named = |register: Register| target::width_of(register).map_or(after, |width| target::named(after, width));
    match one {
        Loc::Reg(one) if ir::root(one.register) == ir::root(before) => {
            Loc::Reg(Reg { register: target::named(after, i64::from(one.width)), ..*one })
        }
        Loc::Mem(one) if [ir::root(one.through), ir::root(one.index_through)].contains(&ir::root(before)) => {
            Loc::Mem(Mem {
                through: if ir::root(one.through) == ir::root(before) { named(one.through) } else { one.through },
                index_through: if ir::root(one.index_through) == ir::root(before) {
                    named(one.index_through)
                } else {
                    one.index_through
                },
                ..one.clone()
            })
        }
        Loc::Address(one) => {
            let through = if ir::root(one.through) == ir::root(before) { named(one.through) } else { one.through };
            let index = if ir::root(one.index) == ir::root(before) { named(one.index) } else { one.index };
            Loc::Address(Address { through, index, ..one.clone() })
        }
        _ => one.clone(),
    }
}

/// The bytes a constant register shift carries, as `(written, source)`, and
/// the lanes of its shifted operands.
///
/// Each written byte reads only its sources; the rest of the operands are not
/// read at all. `shld edx, eax, 16` moves DX into the upper half of EDX, so it
/// reads DX only where that half is live. None for any other instruction.
pub fn _moved_lanes(
    bits: u32,
    one: &Insn,
) -> Option<(Vec<(Lane, Lane)>, Lanes)> {
    let _ = bits;
    _moved_by(one.what.as_ref().and_then(constant_shift)?)
}

/// A shift by a constant: whether it is left, its destination, the register
/// shifted in, and the count.
pub type Shift = (bool, Option<Register>, Option<Register>, u8);

/// `what` as a shift by a constant, from its operands.
fn constant_shift(what: &Semantics) -> Option<Shift> {
    let left = matches!(what.name.as_deref()?, "shl" | "shld");
    match (what.name.as_deref()?, what.dests.as_slice(), what.sources.as_slice()) {
        ("shl" | "shr", [Loc::Reg(dest)], [Loc::Reg(_), Loc::Imm(count)]) => {
            Some((left, Some(dest.register), None, count.value as u8))
        }
        ("shld" | "shrd", [Loc::Reg(dest)], [Loc::Reg(_), Loc::Reg(source), Loc::Imm(count)]) => {
            Some((left, Some(dest.register), Some(source.register), count.value as u8))
        }
        _ => None,
    }
}

pub fn _moved_by((left, destination, source, count): Shift) -> Option<(Vec<(Lane, Lane)>, Lanes)> {
    let destination = destination.filter(|one| matches!(one.size(), 2 | 4) && !_lanes(*one).is_empty())?;
    if source.is_some_and(|one| one.size() != destination.size() || _lanes(one).is_empty()) {
        return None;
    }
    let bits = destination.size() * 8;
    let count = usize::from(count & 31);
    if count == 0 || count >= bits {
        return None;
    }
    // Result bit `b` comes from the destination shifted by `count`, and what
    // the shift empties from the source's other end, or zero.
    let origin = |bit: usize| -> Option<(Register, usize)> {
        if left {
            if bit >= count { Some((destination, bit - count)) } else { source.map(|one| (one, bits - count + bit)) }
        } else if bit + count < bits {
            Some((destination, bit + count))
        } else {
            source.map(|one| (one, bit + count - bits))
        }
    };
    let byte = |register: Register, bit: usize| (full32(register), u32::try_from(bit / 8).expect("a byte"));
    let mut moved = Vec::new();
    for bit in 0..bits {
        if let Some((register, from)) = origin(bit) {
            let pair = (byte(destination, bit), byte(register, from));
            if !moved.contains(&pair) {
                moved.push(pair);
            }
        }
    }
    Some((moved, _lanes(destination).or(&source.map(_lanes).unwrap_or_default())))
}

/// `_register_effects` of an instruction that says only what it does: no
/// required or delivered register, no clobber and no symbol. Asked of a
/// proposed rewrite of one, which needs no instruction made of it to ask (copy
/// propagation made and dropped two per register it tried to forward).
pub fn _register_effects_of_what(
    bits: u32,
    what: &Semantics,
    may_write: bool,
    flags: bool,
) -> Option<(Lanes, Lanes)> {
    if what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty) {
        return Some((Lanes::new(), Lanes::new()));
    }
    if what.op == Operation::Barrier {
        return None;
    }
    let key = EffectKey { bits, may_write, flags, what, requires: &[], delivers: &[] };
    if let Some(found) = EFFECTS.with(|held| held.borrow().find(&key)) {
        return found;
    }
    EFFECTS_COMPUTED.with(|count| count.set(count.get() + 1));
    let answer = _register_effects_of(bits, &Insn::new(0, None, None, vec![], vec![]), what, may_write, flags);
    EFFECTS.with(|held| held.borrow_mut().remember(&key, answer.clone()));
    answer
}

pub fn _register_effects(
    bits: u32,
    one: &Insn,
    may_write: bool,
    flags: bool,
) -> Option<(Lanes, Lanes)> {
    // A symbol/source anchor is a placement fact, not an unknown machine
    // instruction.  Complete unrolling can leave many of these between a
    // definition and its ABI use; clearing liveness at each one kept dead
    // upper register lanes alive and defeated width selection.  A NOTHING
    // carrying a real clobber mask remains a barrier below.
    if one
        .what
        .as_ref()
        .is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty))
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
    {
        return Some((Lanes::new(), Lanes::new()));
    }
    if !one.clobbers.is_empty() || one.symbol == Some(true) {
        return None;
    }
    let what = match &one.what {
        Some(what) if what.op != Operation::Barrier => what,
        // `getattr(one.op, "node", None)`: a `mir.Op` has no `node`, so the
        // decoded-opaque fallback never applies and this is unknown.
        _ => return None,
    };
    // Every pass of the peephole, and the liveness and copy propagation around
    // it, asks of the same instructions again and again (50 asks for each
    // of an `-O2` module's 16,000, #924): the answer depends on these fields
    // alone, and is remembered by their value.
    let key = EffectKey { bits, may_write, flags, what, requires: &one.requires, delivers: &one.delivers };
    if let Some(found) = EFFECTS.with(|held| held.borrow().find(&key)) {
        return found;
    }
    EFFECTS_COMPUTED.with(|count| count.set(count.get() + 1));
    let answer = _register_effects_of(bits, one, what, may_write, flags);
    EFFECTS.with(|held| held.borrow_mut().remember(&key, answer.clone()));
    answer
}

/// The inputs of `_register_effects` after the cases it answers at once: the
/// same ones, the same answer.
struct EffectKey<'a> {
    bits: u32,
    may_write: bool,
    flags: bool,
    what: &'a Semantics,
    requires: &'a [(Held, Register)],
    delivers: &'a [(Held, Register)],
}

impl EffectKey<'_> {
    fn hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        (self.bits, self.may_write, self.flags, self.what, self.requires, self.delivers).hash(&mut hasher);
        hasher.finish()
    }
}

type Effects = Option<(Lanes, Lanes)>;

/// What `_register_effects` answered, by the hash of what it was asked, and the
/// asked kept to tell a hash that is another's.
#[derive(Default)]
struct EffectsHeld {
    by_hash: HashMap<u64, Vec<(u32, bool, bool, Semantics, Vec<(Held, Register)>, Vec<(Held, Register)>, Effects)>>,
    count: usize,
}

impl EffectsHeld {
    fn find(
        &self,
        key: &EffectKey,
    ) -> Option<Effects> {
        self.by_hash
            .get(&key.hash())?
            .iter()
            .find(|(bits, may_write, flags, what, requires, delivers, _)| {
                *bits == key.bits
                    && *may_write == key.may_write
                    && *flags == key.flags
                    && what == key.what
                    && requires == key.requires
                    && delivers == key.delivers
            })
            .map(|(.., answer)| answer.clone())
    }

    fn remember(
        &mut self,
        key: &EffectKey,
        answer: Effects,
    ) {
        // A function's instructions, not the run's.
        if self.count > 200_000 {
            *self = Self::default();
        }
        self.count += 1;
        self.by_hash
            .entry(key.hash())
            .or_default()
            .push(
                (
                    key.bits,
                    key.may_write,
                    key.flags,
                    key.what.clone(),
                    key.requires.to_vec(),
                    key.delivers.to_vec(),
                    answer,
                ),
            );
    }
}

thread_local! {
    static EFFECTS: RefCell<EffectsHeld> = RefCell::new(EffectsHeld::default());
    static EFFECTS_COMPUTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has worked out an instruction's register effects,
/// for a test that asking again of one does not.
pub fn effects_computed() -> usize {
    EFFECTS_COMPUTED.with(std::cell::Cell::get)
}

/// What `x86.instr` says of `what`; None where it has no row, which is not
/// knowing.
fn _register_effects_of(
    bits: u32,
    one: &Insn,
    what: &Semantics,
    may_write: bool,
    flags: bool,
) -> Effects {
    _effects_by_table(bits, one, what, may_write, flags).flatten()
}

/// `_register_effects_of` as `x86.instr` states it: None where it has no row.
pub fn _effects_by_table(
    bits: u32,
    one: &Insn,
    what: &Semantics,
    may_write: bool,
    flags: bool,
) -> Option<Effects> {
    let found = match crate::backend::effects::served(bits, what)? {
        crate::backend::effects::Served::Unknown => return Some(None),
        crate::backend::effects::Served::Known(found) => found,
    };
    let mut reads: Lanes = one
        .requires
        .iter()
        .flat_map(|(held, register)| _lanes(target::named(*register, i64::from(held.width))))
        .collect();
    let mut writes: Lanes = one
        .delivers
        .iter()
        .flat_map(|(held, register)| _lanes(target::named(*register, i64::from(held.width))))
        .collect();
    if flags {
        reads.extend(_flag_lanes(found.flags_read).minus(&writes));
        writes.extend(_flag_lanes(found.flags_written));
    }
    for register in &found.reads {
        reads.extend(_lanes(*register).minus(&writes));
    }
    for register in &found.writes {
        writes.extend(_lanes(*register));
    }
    if may_write {
        for register in &found.maybe_writes {
            writes.extend(_lanes(*register));
        }
    }
    Some(Some((reads, writes)))
}

pub fn _flag_lanes(mask: u32) -> Lanes {
    Lanes::flags(mask)
}

/// A bp-relative slot whose bytes the displacement alone names.
pub fn _frame_cell(cell: &Mem) -> bool {
    cell.addr.is_some_and(|addr| addr.space == Space::Frame) && cell.through == Register::BP && cell.base.is_none()
}

/// Whether two such slots share a byte -- arithmetic on the displacements.
pub fn _overlapping(
    a: &Mem,
    b: &Mem,
) -> bool {
    if !_frame_cell(a) || !_frame_cell(b) {
        return true;
    }
    let (a_addr, b_addr) = (a.addr.expect("a frame cell"), b.addr.expect("a frame cell"));
    a_addr.disp < b_addr.disp + i64::from(b.width) && b_addr.disp < a_addr.disp + i64::from(a.width)
}

/// The one frame slot this instruction writes, if that is all it writes.
///
/// The allocator's own store says so itself. Any other instruction has to
/// prove it from the operation it came from, because an inserted store
/// carries the `op` of whatever it stands beside and that one's stores are
/// not its own.
pub fn _frame_written(one: &Insn) -> Option<Mem> {
    let what = one.what.as_ref()?;
    let cells: Vec<&Mem> = what
        .dests
        .iter()
        .filter_map(|dest| match dest {
            Loc::Mem(cell) => Some(cell),
            _ => None,
        })
        .collect();
    if cells.len() != 1 || !_frame_cell(cells[0]) {
        return None;
    }
    // A call that may write names no frame cell.
    if !one.spill_store && one.call.as_ref().is_some_and(|call| call.writes()) {
        return None;
    }
    Some(cells[0].clone())
}

/// Remove overwritten moves, pure arithmetic and owned spill reloads.
///
/// What is dead on exit from the block is a fact about the whole body, not
/// something a backward walk of one block can assume away: a phi's parallel
/// copy is written as the last instruction there, which is exactly where
/// "assume everything live" refuses to look.
pub fn overwritten(body: &LirBody) -> LirBody {
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut dead = exits[&block.at].clone();
        let mut redundant: HashSet<usize> = HashSet::default();
        for one in block.insns.iter().rev() {
            let Some(effect) = liveness::effect(body.bits, one) else {
                dead.clear();
                continue;
            };
            if let Some(what) = &one.what {
                match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
                    (
                        Operation::Move,
                        Some("mov"),
                        [Loc::Reg(dest)],
                        [source @ (Loc::Reg(_) | Loc::Imm(_) | Loc::Mem(_))],
                    ) => {
                        let lanes = _lanes(dest.register);
                        let width = match source {
                            Loc::Reg(one) => one.width,
                            Loc::Imm(one) => one.width,
                            Loc::Mem(one) => one.width,
                            _ => unreachable!(),
                        };
                        if !lanes.is_empty()
                            && lanes.is_subset(&dead)
                            && dest.width == width
                            && one.requires.is_empty()
                            && one.delivers.is_empty()
                            && (matches!(source, Loc::Reg(_))
                                || matches!(source, Loc::Imm(value) if value.address.is_none())
                                || matches!(source, Loc::Mem(_)) && one.spill_reload)
                        {
                            redundant.insert(id(one));
                            continue;
                        }
                    }
                    (Operation::Extend, Some("movsx" | "movzx"), [Loc::Reg(dest)], [Loc::Reg(_)]) => {
                        // Pure, and nothing else it writes. Lowering a divide's
                        // sign word as `cwd` leaves the widening it replaced
                        // behind, with a register and an instruction to its
                        // name.
                        if !effect.writes.is_empty()
                            && effect.writes.is_subset(&dead)
                            && !_lanes(dest.register).is_empty()
                            && one.requires.is_empty()
                            && one.delivers.is_empty()
                            && one.spread.is_empty()
                            && one.group.is_none()
                            && one.symbol != Some(true)
                        {
                            redundant.insert(id(one));
                            continue;
                        }
                    }
                    (Operation::Binary, Some("add" | "sub" | "and" | "or" | "xor"), [Loc::Reg(dest)], sources) => {
                        if !effect.writes.is_empty()
                            && effect.writes.is_subset(&dead)
                            && !_lanes(dest.register).is_empty()
                            && one.requires.is_empty()
                            && one.delivers.is_empty()
                            && one.spread.is_empty()
                            && one.group.is_none()
                            && sources.iter().all(|source| {
                                matches!(source, Loc::Reg(_))
                                    || matches!(source, Loc::Imm(value) if value.address.is_none())
                            })
                        {
                            redundant.insert(id(one));
                            continue;
                        }
                    }
                    _ => {}
                }
            }
            dead = effect.dead_before(&dead);
        }
        let insns = block
            .insns
            .iter()
            .map(|one| if redundant.contains(&id(one)) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
            .collect();
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

/// Fold a load, an operation and a store into one memory operation
/// (`peephole.peep`).
pub fn fused(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.fused, body, &Facts::new(body, None))
}

/// Load a far pointer's two words with one les (lds, lfs, lgs)
/// (`peephole.peep`).
pub fn far_loads(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.far_loads, body, &Facts::new(body, None))
}

/// Replace a dead temporary's shift with a scaled 67h LEA.
///
/// The load remains a load.  Only the register-only `shl; add` tail is
/// selected differently, after virtual use counts prove that no later use
/// expects the temporary to contain its shifted value.
fn _loaded_scaled_add<'a>(
    parts: &[Arc<Insn>],
    uses: &Counter,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<Option<Arc<Insn>>, String> {
    if parts.len() != 3
        || parts.iter().any(|one| {
            one.what.is_none()
                || !one.clobbers.is_empty()
                || !one.clobbers_high.is_empty()
                || !one.spread.is_empty()
                || one.group.is_some()
                || one.frame_adjust
        })
    {
        return Ok(None);
    }
    let (load, shift, addition) = (&parts[0], &parts[1], &parts[2]);
    let (first, second, third) = (
        load.what.as_ref().expect("checked above"),
        shift.what.as_ref().expect("checked above"),
        addition.what.as_ref().expect("checked above"),
    );
    let (temporary, total, amount) = match (
        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
        (third.op, third.name.as_deref(), third.dests.as_slice(), third.sources.as_slice()),
    ) {
        (
            (Operation::Move, Some("mov"), [Loc::Reg(temporary)], [Loc::Mem(_)]),
            (
                Operation::Binary,
                Some("shl"),
                [shift_dest],
                [shift_source, Loc::Imm(Imm { value: amount, address: None, .. })],
            ),
            (Operation::Binary, Some("add"), [Loc::Reg(total)], [add_left, add_right]),
        ) => {
            if *shift_dest != Loc::Reg(*temporary)
                || *shift_source != Loc::Reg(*temporary)
                || *add_left != Loc::Reg(*total)
                || *add_right != Loc::Reg(*temporary)
                || !(1..=3).contains(amount)
            {
                return Ok(None);
            }
            (*temporary, *total, *amount)
        }
        _ => return Ok(None),
    };
    if temporary.width != total.width
        || ![2, 4].contains(&temporary.width)
        || !target::WIDTHS.contains_key(&temporary.register)
        || !target::WIDTHS.contains_key(&total.register)
        || ir::root(temporary.register) == ir::root(total.register)
        || ir::root(temporary.register) == Register::ESP
        || load.defines.len() != 1
        || shift.defines != load.defines
        || shift.uses != load.defines
        || !addition.uses.contains(&load.defines[0])
        || count(uses, load.defines[0]) != 2
    {
        return Ok(None);
    }
    let target_cpu = targets::profile(cpu)?;
    let old = target_cpu.operations.shift + target_cpu.operations.add;
    let mut new = target_cpu.operations.address + target_cpu.operations.prefix;
    if temporary.width < 4 {
        new += target_cpu.partial_register_stall;
    }
    if new > old {
        return Ok(None);
    }
    let what = semantics(
        Operation::Address,
        "lea",
        vec![Loc::Reg(total)],
        vec![Loc::Address(Address {
            through: full32(total.register),
            index: full32(temporary.register),
            scale: 1 << amount,
            ..Address::new(None)
        })],
    );
    Ok(Some(Arc::new(with_what(addition, what))))
}

/// Select physically adjacent load/scale/add tails across inert anchors.
fn _loaded_addresses(
    bits: u32,
    block: &LirBlock,
    flags_dead_out: bool,
    uses: &Counter,
    cpu: &Profile,
) -> Result<LirBlock, String> {
    let dead = _flags_dead_after(bits, block, flags_dead_out);
    let mut insns = block.insns.to_vec();
    let work: Vec<usize> =
        insns.iter().enumerate().filter(|(_, one)| !_skippable_nothing(one)).map(|(index, _)| index).collect();
    let mut removed: HashSet<usize> = HashSet::default();
    let mut at = 0;
    while at + 2 < work.len() {
        let indexes = &work[at..at + 3];
        let parts: Vec<Arc<Insn>> = indexes.iter().map(|index| Arc::clone(&insns[*index])).collect();
        let combined = if dead.contains(&id(&parts[2])) { _loaded_scaled_add(&parts, uses, cpu)? } else { None };
        let Some(combined) = combined else {
            at += 1;
            continue;
        };
        let shift = Arc::clone(&parts[1]);
        if shift.symbol == Some(true) {
            insns[indexes[1]] = lir::anchor(Arc::clone(&shift));
            insns[indexes[2]] = combined;
            at += 3;
            continue;
        }
        let folded = lir::without(
            &[Arc::clone(&shift), combined],
            |one| Arc::ptr_eq(one, &shift),
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        // The shift's bytes join the combined instruction, or stay on an
        // anchor where the shift was.
        match &folded[..] {
            [joined] => {
                insns[indexes[2]] = Arc::clone(joined);
                removed.insert(indexes[1]);
            }
            [anchor, joined] => {
                insns[indexes[1]] = Arc::clone(anchor);
                insns[indexes[2]] = Arc::clone(joined);
            }
            _ => unreachable!("without keeps the combined instruction and at most one anchor"),
        }
        at += 3;
    }
    let insns: Vec<_> =
        insns.into_iter().enumerate().filter(|(index, _)| !removed.contains(index)).map(|(_, one)| one).collect();
    Ok(block.with_insns(insns))
}

/// The instructions of `block` after which no arithmetic flag is read,
/// given whether any is read after its end.
pub(crate) fn _flags_dead_after(
    bits: u32,
    block: &LirBlock,
    dead_out: bool,
) -> HashSet<usize> {
    let mut dead: HashSet<usize> = HashSet::default();
    let mut flags_dead = dead_out;
    for one in block.insns.iter().rev() {
        if flags_dead {
            dead.insert(id(one));
        }
        flags_dead = _flags_before(bits, one, flags_dead);
    }
    dead
}

/// Whether replacing `parts`, followed in their block by `after`, drops a
/// definition something still reads.
///
/// Allocated instructions carry virtual identities that need not follow
/// their physical two-address spelling, and coalescing gives one identity
/// several definitions. A definition is lost only where a read reaches it:
/// later in the block before the identity is defined again, or past the
/// block's end (`live_out`).
pub fn _loses_live_definition(
    parts: &[Arc<Insn>],
    combined: &Insn,
    after: &[Arc<Insn>],
    live_out: &BTreeSet<u32>,
) -> bool {
    let eliminated: BTreeSet<u32> = parts
        .iter()
        .flat_map(|one| one.defines.iter().copied())
        .filter(|value| !combined.defines.contains(value))
        .collect();
    _read_before_redefined(&eliminated, after, live_out)
}

/// Whether `after`, or the block's successors past it, read any of
/// `eliminated` before redefining it.
pub fn _read_before_redefined(
    eliminated: &BTreeSet<u32>,
    after: &[Arc<Insn>],
    live_out: &BTreeSet<u32>,
) -> bool {
    let reads =
        |one: &Insn, value: u32| one.uses.contains(&value) || one.requires.iter().any(|(held, _)| held.value == value);
    eliminated
        .iter()
        .any(
            |value| {
                let mut redefined: Option<Option<i64>> = None;
                for one in after {
                    match redefined {
                        // A group reads before any member writes.
                        Some(group) if group.is_some() && one.group == group => {
                            if reads(one, *value) {
                                return true;
                            }
                            continue;
                        }
                        Some(_) => return false,
                        None => {}
                    }
                    if reads(one, *value) {
                        return true;
                    }
                    if one.defines.contains(value) {
                        redefined = Some(one.group);
                    }
                }
                redefined.is_none() && live_out.contains(value)
            },
        )
}

/// Select LEA for allocated arithmetic when the replaced flags are dead.
pub fn addresses<'a>(
    body: &LirBody,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<LirBody, String> {
    let target_cpu = targets::profile(cpu)?;
    let mut virtual_uses = Counter::default();
    for block in &body.blocks {
        for one in &block.insns {
            for value in &one.uses {
                *virtual_uses.entry(*value).or_insert(0) += 1;
            }
        }
    }
    for block in &body.blocks {
        for one in &block.insns {
            for (held, _register) in &one.requires {
                *virtual_uses.entry(held.value).or_insert(0) += 1;
            }
        }
    }
    for block in &body.blocks {
        for phi in &block.phis {
            for (_source, value) in &phi.incoming {
                *virtual_uses.entry(*value).or_insert(0) += 1;
            }
        }
    }
    let (_, live_out) = crate::backend::allocate::live(body);
    let flags_out = _flags_live_out(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let flags_dead_out = flags_out.get(&block.at).is_some_and(Lanes::is_empty);
        let block = _loaded_addresses(body.bits, block, flags_dead_out, &virtual_uses, target_cpu)?;
        let dead = _flags_dead_after(body.bits, &block, flags_dead_out);
        let live_out = live_out.get(&block.at).cloned().unwrap_or_default();
        let insns = _code_windows(&block.insns, block.insns.len(), |rest| -> Result<_, String> {
            Ok(match _affine_address(body.bits, rest, &dead, target_cpu)? {
                Some((taken, made)) if !_loses_live_definition(&rest[..taken], &made[0], &rest[taken..], &live_out) => {
                    Some((taken, made))
                }
                _ => None,
            })
        })?;
        blocks.push(block.with_insns(insns));
    }
    Ok(body.with_blocks(blocks))
}

/// `ir.ROOT.get(register)`: `ROOT` itself is private to `ir`, and its keys
/// are exactly the registers `root` renames plus the eight 32-bit roots.
fn _root_get(register: Register) -> Option<Register> {
    let rooted = ir::root(register);
    let key = rooted != register
        || [
            Register::EAX,
            Register::EBX,
            Register::ECX,
            Register::EDX,
            Register::ESI,
            Register::EDI,
            Register::EBP,
            Register::ESP,
        ]
        .contains(&register);
    key.then_some(rooted)
}

/// Replace repeated allocated address shuttles with one clean 67h base.
///
/// Constrained-occurrence splitting keeps a long-lived word value in any GPR
/// and creates short BX/SI/DI copies at native memory uses.  Once allocation
/// has chosen physical registers, several such copies may be dearer than
/// zero-extending the owner once and addressing through its 32-bit root.  The
/// latter is the medium model's legal address-size-prefixed fallback.
///
/// This is deliberately post-allocation: it recognizes only copies created
/// by allocation, changes no program algebra, checks every virtual consumer,
/// and compares both the selected CPU cost and exact encoded byte totals.
pub fn secondary_bases<'a>(
    body: &LirBody,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<LirBody, String> {
    let profile = targets::profile(cpu)?;
    let Some(secondary) = profile.dword_address_form() else {
        return Ok(body.clone());
    };

    let mut definitions: IndexMap<u32, Option<Arc<Insn>>> = IndexMap::default();
    let mut uses: IndexMap<u32, Vec<Arc<Insn>>> = IndexMap::default();
    for one in body.insns() {
        for value in &one.defines {
            if definitions.contains_key(value) {
                definitions.insert(*value, None);
            } else {
                definitions.insert(*value, Some(Arc::clone(&one)));
            }
        }
        for value in one.uses.iter().copied().collect::<BTreeSet<u32>>() {
            uses.entry(value).or_default().push(Arc::clone(&one));
        }
    }

    let source_register = |one: &Insn, value: u32| -> Option<Reg> {
        let what = one.what.as_ref()?;
        for (named, destination) in one.defines.iter().zip(&what.dests) {
            if let Loc::Reg(destination) = destination {
                if *named == value && destination.width == 2 {
                    return Some(*destination);
                }
            }
        }
        None
    };

    let derived = |value: u32, original: u32, source: &Reg| -> Option<(Arc<Insn>, Vec<Arc<Insn>>)> {
        let definition = definitions.get(&value).cloned().flatten();
        let consumers = uses.get(&value).cloned().unwrap_or_default();
        let definition = definition?;
        if !definition.inserted() || definition.uses != [original] || consumers.len() != 1 {
            return None;
        }
        if let Some(what) = &definition.what {
            if what.op != Operation::Nothing {
                match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
                    (Operation::Move, Some("mov"), [Loc::Reg(destination)], [Loc::Reg(copied)]) => {
                        if destination.width != 2 || copied != source {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
        }
        let consumer = &consumers[0];
        if Arc::ptr_eq(consumer, &definition) {
            return None;
        }
        let consumed = consumer.what.as_ref()?;
        let mut seen = 0;
        for operand in consumed.dests.iter().chain(&consumed.sources) {
            let selector_alias = matches!(
                operand,
                Loc::Mem(cell) if cell.selector.is_some_and(|selector| selector.value == value)
            );
            for held in ir::values(operand) {
                if held.value == value {
                    seen += 1;
                    if !matches!(operand, Loc::Mem(cell)
                        if cell.base == Some(held)
                            && cell.index.is_none()
                            && !selector_alias
                            // A frame cell has two independent address
                            // components: fixed BP and the allocated dynamic
                            // index.  This rewrite widens one value only, so it
                            // cannot legally turn the pair into a 32-bit
                            // address.
                            && !cell.addr.is_some_and(|addr| addr.space == Space::Frame)
                            && target::width_of(cell.through) == Some(2))
                    {
                        return None;
                    }
                }
            }
        }
        if seen != 0 { Some((definition, consumers)) } else { None }
    };

    let rewritten = |one: &Insn, substitutions: &IndexMap<u32, (u32, Register)>| -> Arc<Insn> {
        let operand = |where_: &Loc| -> Loc {
            let Loc::Mem(cell) = where_ else {
                return where_.clone();
            };
            let Some(base) = cell.base else {
                return where_.clone();
            };
            let Some((original, register)) = substitutions.get(&base.value) else {
                return where_.clone();
            };
            Loc::Mem(Mem { base: Some(Held { value: *original, width: 4 }), through: *register, ..cell.clone() })
        };
        let what = one.what.as_ref().expect("a rewritten consumer has semantics");
        Arc::new(Insn {
            what: Some(Semantics {
                dests: what.dests.iter().map(operand).collect(),
                sources: what.sources.iter().map(operand).collect(),
                ..what.clone()
            }),
            uses: one
                .uses
                .iter()
                .map(|value| substitutions.get(value).map_or(*value, |substituted| substituted.0))
                .collect(),
            ..one.clone()
        })
    };

    let mut substitutions: IndexMap<u32, (u32, Register)> = IndexMap::default();
    let mut remove: HashSet<usize> = HashSet::default();
    let mut insert_after: HashMap<usize, Arc<Insn>> = HashMap::default();
    for (candidate, definition) in definitions.clone() {
        let Some(definition) = definition else {
            continue;
        };
        let Some(source) = source_register(&definition, candidate) else {
            continue;
        };
        let Some(root) = _root_get(source.register) else {
            continue;
        };
        if target::width_of(root) != Some(4) {
            continue;
        }
        let mut group = Vec::new();
        for (value, made) in &definitions {
            if *value == candidate || made.is_none() {
                continue;
            }
            if let Some((made, found)) = derived(*value, candidate, &source) {
                group.push((*value, made, found));
            }
        }
        if group.is_empty() {
            continue;
        }
        let trial_substitutions: IndexMap<u32, (u32, Register)> =
            group.iter().map(|(value, _made, _consumers)| (*value, (candidate, root))).collect();
        let mut consumers: IndexMap<usize, Arc<Insn>> = IndexMap::default();
        for (_v, _d, found) in &group {
            for one in found {
                consumers.insert(id(one), rewritten(one, &trial_substitutions));
            }
        }
        let extension = Arc::new(Insn {
            call: definition.call.clone(),
            ..Insn::new(
                definition.at,
                Some((definition.at, definition.at)),
                Some(semantics(Operation::Extend, "movzx", vec![Loc::Reg(reg(root, 4))], vec![Loc::Reg(source)])),
                vec![candidate],
                vec![candidate],
            )
        });
        let copies: Vec<Semantics> = group
            .iter()
            .filter_map(|(_value, made, _found)| made.what.clone())
            .filter(|what| what.name.as_deref() == Some("mov"))
            .collect();
        let before_parts: Vec<Semantics> = copies
            .iter()
            .cloned()
            .chain(
                group
                    .iter()
                    .flat_map(|(_value, _made, found)| found.iter().map(|one| one.what.clone().expect("a consumer"))),
            )
            .collect();
        let after_parts: Vec<Semantics> = std::iter::once(extension.what.clone().expect("set above"))
            .chain(consumers.values().map(|one| one.what.clone().expect("a consumer")))
            .collect();
        let before_encoded: Vec<Option<select::Emitted>> =
            before_parts.iter().map(|what| emit(body.bits, what)).collect();
        let after_encoded: Vec<Option<select::Emitted>> =
            after_parts.iter().map(|what| emit(body.bits, what)).collect();
        if before_encoded.iter().chain(&after_encoded).any(Option::is_none) {
            continue;
        }
        let before_cost = copies.len() as i64 * profile.cost("mov_rr")?;
        // The defining word write is immediately read as a full register by
        // MOVZX.  This is exactly the partial-register transition charged by
        // the final scorer; omitting it made P6/Core select a locally smaller
        // sequence that was substantially slower under their own profiles.
        let after_cost =
            profile.cost("movzx")? + profile.partial_register_stall + consumers.len() as i64 * secondary.use_cost;
        let before_bytes: usize = before_encoded.iter().flatten().map(|one| one.code.len()).sum();
        let after_bytes: usize = after_encoded.iter().flatten().map(|one| one.code.len()).sum();
        if after_cost >= before_cost || after_bytes > before_bytes {
            continue;
        }
        substitutions.extend(trial_substitutions);
        remove.extend(group.iter().map(|(_value, made, _found)| id(made)));
        insert_after.insert(id(&definition), extension);
    }

    if substitutions.is_empty() {
        return Ok(body.clone());
    }
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        for one in &block.insns {
            if remove.contains(&id(one)) {
                continue;
            }
            insns.push(if one.uses.iter().any(|value| substitutions.contains_key(value)) {
                rewritten(one, &substitutions)
            } else {
                Arc::clone(one)
            });
            if let Some(extension) = insert_after.get(&id(one)) {
                insns.push(Arc::clone(extension));
            }
        }
        blocks.push(block.with_insns(insns));
    }
    Ok(body.with_blocks(blocks))
}

/// `mov z,y` and the arithmetic after it that only rewrites z, as one 67h LEA.
///
/// Each step keeps z an affine sum of registers (`affine::step`). The
/// longest prefix whose flags are dead (`dead`), whose sum an address names,
/// and which the target prices no dearer in cycles or bytes becomes one LEA.
/// A word result keeps only the low sixteen bits, which the upper halves of
/// the registers read cannot reach. Returns the instructions consumed.
fn _affine_address(
    mode: u32,
    parts: &[Arc<Insn>],
    dead: &HashSet<usize>,
    cpu: &Profile,
) -> Result<Option<(usize, Vec<Arc<Insn>>)>, String> {
    let Some(copy) = parts.first() else {
        return Ok(None);
    };
    let Some(wide) = cpu.dword_address_form() else {
        return Ok(None);
    };
    let Some((dest, affine::Step::Copy(source), mut old)) = affine::step(copy, cpu) else {
        return Ok(None);
    };
    let z = full32(dest.register);
    if full32(source.register) == z {
        return Ok(None);
    }
    let bits = i64::from(dest.width) * 8;
    let wrapped = |value: i64| (value + (1 << (bits - 1))).rem_euclid(1 << bits) - (1 << (bits - 1));
    let (mut terms, mut disp): (affine::Terms, i64) = (vec![(full32(source.register), 1)], 0);
    let mut best: Option<(usize, Address, i64, bool)> = None;
    for (at, one) in parts.iter().enumerate().skip(1) {
        let Some((written, step, cost)) = affine::step(one, cpu) else {
            break;
        };
        if written != dest {
            break;
        }
        match step {
            affine::Step::Add(value) => disp = wrapped(disp + value),
            affine::Step::Scale(factor) => {
                terms.iter_mut().for_each(|term| term.1 *= factor);
                disp = wrapped(disp * factor);
            }
            affine::Step::AddRegister(other) => {
                let root = full32(other.register);
                match terms.iter_mut().find(|term| term.0 == root) {
                    Some(term) => term.1 += 1,
                    None => terms.push((root, 1)),
                }
            }
            affine::Step::Copy(_) => break,
        }
        old += cost;
        if terms.len() > 2 || terms.iter().any(|term| term.1 > 9) {
            break;
        }
        if !dead.contains(&id(one)) {
            continue;
        }
        if dest.width == 2
            && let Some(address) = affine::word_form(&terms, disp)
        {
            best = Some((at, address, old, true));
        } else if let Some(address) = affine::form(&terms, disp, &wide.scales) {
            best = Some((at, address, old, false));
        }
    }
    let Some((last, address, old, word)) = best else {
        return Ok(None);
    };
    let replaced = &parts[..=last];
    let partial: HashSet<Register> = terms.iter().map(|term| term.0).collect();
    let stalls = if dest.width < 4 { partial.len() as i64 * cpu.partial_register_stall } else { 0 };
    // The target's own price of the form (`three_operand`): a word address has
    // no prefix and reads no dword register.
    let scale = address.scale;
    let width = i64::from(dest.width);
    let Some(form) = llrm_mir::target::three_operand(
        &cpu.operations,
        &cpu.address_forms,
        width,
        i64::from(cpu.operand_bytes),
        scale,
        word,
    ) else {
        return Ok(None);
    };
    let price = form + if word { 0 } else { stalls };
    if price > old {
        return Ok(None);
    }
    let what = semantics(Operation::Address, "lea", vec![Loc::Reg(dest)], vec![Loc::Address(address)]);
    let Some(new) = emit(mode, &what) else {
        return Ok(None);
    };
    // What the parts are in bytes once `increments` has run: a unit add whose
    // carry is dead is the one-byte INC or DEC.
    let mut before = 0;
    for one in replaced {
        let part = one.what.as_ref().expect("checked plain");
        let Some(plain) = emit(mode, part) else {
            return Ok(None);
        };
        let unit = match (part.name.as_deref(), part.dests.as_slice(), part.sources.as_slice()) {
            (Some(name @ ("add" | "sub")), [Loc::Reg(register)], [_, Loc::Imm(Imm { value, .. })])
                if dead.contains(&id(one)) =>
            {
                // By the register's width: 1 and -1 (0xFFFF in a word) are the
                // unit.
                let bits = i64::from(register.width) * 8;
                let step = match (*value + (1 << (bits - 1))).rem_euclid(1 << bits) - (1 << (bits - 1)) {
                    1 => Some(name == "add"),
                    -1 => Some(name != "add"),
                    _ => None,
                };
                step.map(|up| {
                    semantics(
                        Operation::Unary,
                        if up { "inc" } else { "dec" },
                        vec![Loc::Reg(*register)],
                        vec![Loc::Reg(*register)],
                    )
                })
            }
            _ => None,
        };
        before += unit
            .and_then(|unit| emit(mode, &unit))
            .map_or(plain.code.len(), |short| short.code.len().min(plain.code.len()));
    }
    if new.code.len() > before {
        return Ok(None);
    }
    // The LEA takes its first owner's bytes; the other parts' join it where
    // they meet, else stay on anchors. Bytes never veto the LEA.
    let owned: Vec<&Arc<Insn>> =
        replaced.iter().filter(|one| one.covers.is_some_and(|(start, end)| start != end)).collect();
    let owner = owned.first().copied().unwrap_or(copy);
    let symbolic: Vec<&Arc<Insn>> = replaced.iter().filter(|one| one.symbol == Some(true)).collect();
    if !symbolic.is_empty() && (symbolic.len() != 1 || !Arc::ptr_eq(symbolic[0], owner)) {
        return Ok(None);
    }
    let result = &replaced[last];
    let intermediate: HashSet<u32> = replaced[..last].iter().flat_map(|one| one.defines.iter().copied()).collect();
    let uses = deduped(copy.uses.iter().copied().chain(
        replaced[1..].iter().flat_map(|one| one.uses.iter().copied()).filter(|value| !intermediate.contains(value)),
    ));
    let live: HashSet<u32> = uses.iter().chain(&result.defines).copied().collect();
    let widths =
        deduped(replaced.iter().flat_map(|one| one.widths.iter().copied()).filter(|pair| live.contains(&pair.0)));
    let combined = Arc::new(Insn {
        what: Some(what),
        defines: result.defines.clone(),
        uses,
        widths,
        requires: deduped(replaced.iter().flat_map(|one| one.requires.iter().copied())),
        delivers: deduped(replaced.iter().flat_map(|one| one.delivers.iter().copied())),
        ..(**owner).clone()
    });
    let others: Vec<Arc<Insn>> =
        owned.iter().filter(|one| !Arc::ptr_eq(one, &owner)).map(|one| Arc::clone(one)).collect();
    let sequence: Vec<Arc<Insn>> = std::iter::once(combined).chain(others.iter().cloned()).collect();
    let made = lir::without(
        &sequence,
        |one| others.iter().any(|other| Arc::ptr_eq(other, one)),
        None::<fn(&Arc<Insn>) -> Arc<Insn>>,
    );
    Ok(Some((last + 1, made)))
}

/// Select compact INC/DEC for a unit add whose carry result is dead
/// (`peephole.peep`).
pub fn increments(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.increments, body, &Facts::new(body, None))
}

/// `mov r,0 ; sbb r,0` as `sbb r,r` (`peephole.peep`).
pub fn borrows(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.borrows, body, &Facts::new(body, None))
}

/// `shl r,1` as `add r,r` where the target prices the add lower
/// (`peephole.peep`).
pub fn doubled(
    rules: &peep::Rules,
    body: &LirBody,
    cpu: &Profile,
) -> Result<LirBody, String> {
    cpu.doubling()?;
    Ok(peep::rewritten(rules.doubled, body, &Facts::new(body, Some(cpu))))
}

/// A dword operation and the `movzx` of its word, as the word operation where
/// the register's upper half is zero (`peephole.peep`).
pub fn narrowed_arithmetic(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.narrowed_arithmetic, body, &Facts::new(body, None))
}

/// A word copy of a register zero above it as the dword copy (`peephole.peep`).
pub fn widened_moves(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.widened_moves, body, &Facts::new(body, None))
}

/// The extension of a word whose register is zero above it as a copy of the
/// register (`peephole.peep`).
pub fn zero_extensions(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.zero_extensions, body, &Facts::new(body, None))
}

fn _flags_before(
    bits: u32,
    one: &Insn,
    flags_dead: bool,
) -> bool {
    let (reads, writes) = _flag_effect(bits, one);
    let dead = if flags_dead { _ARITHMETIC_LANES.clone() } else { Lanes::new() };
    _ARITHMETIC_LANES.is_subset(&dead.or(&writes).minus(&reads))
}

/// What each conditional jump reads: the condition's own flags, which is x86's
/// and the same for every target and operand size, so iced states it from the
/// mnemonic, not from any target's rows.
static _BRANCH_READS: LazyLock<HashMap<String, u32>> = LazyLock::new(|| {
    iced_x86::Code::values()
        .filter(|code| code.flow_control() == FlowControl::ConditionalBranch)
        .filter_map(|code| {
            let name = format!("{:?}", code.mnemonic()).to_lowercase();
            let condition = name.strip_prefix('j')?;
            instructions::parse::CONDITIONS.contains(&condition).then_some(())?;
            Some((name, iced_x86::Instruction::with_branch(code, 0).ok()?.rflags_read()))
        })
        .collect()
});

/// The flags a conditional jump reads, or all where the description does not
/// know it.
pub fn _branch_reads(what: &Semantics) -> Lanes {
    _flag_lanes(_BRANCH_READS.get(what.name.as_deref().unwrap_or("")).copied().unwrap_or(0xFFFF_FFFF))
}

const _ARITHMETIC: u32 =
    RflagsBits::OF | RflagsBits::SF | RflagsBits::ZF | RflagsBits::AF | RflagsBits::CF | RflagsBits::PF;
// What `cmp r,0` leaves that the instruction computing r may not: inc keeps
// the carry, add and subtract set carry and overflow from their operands.
static _DIFFERING: LazyLock<Lanes> = LazyLock::new(|| _flag_lanes(RflagsBits::OF | RflagsBits::CF | RflagsBits::AF));
// What `xor r,r` writes: a direction flag read later, as a string fill reads
// it, is no objection.
static _ARITHMETIC_LANES: LazyLock<Lanes> = LazyLock::new(|| _flag_lanes(_ARITHMETIC));

/// `inc edi; cmp edi,0; jne` is `inc edi; jne`.
///
/// An add, subtract, logic or unary operation sets ZF and SF from its result
/// exactly as a zero test of that result does. The test goes where only its
/// branch reads those two, and no later instruction reads the carry,
/// overflow or adjust flags it would have cleared. Work independent of the
/// computation may stand between them, in the block or in a sole predecessor
/// that only falls into it: the computation sinks to just before the test.
pub fn tested(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    let live = _flags_live_out(body);
    let mut predecessors = HashMap::<i64, Vec<usize>>::default();
    for (at, block) in body.blocks.iter().enumerate() {
        for to in &block.succ {
            predecessors.entry(*to).or_default().push(at);
        }
    }
    let mut blocks = body.blocks.iter().map(|block| block.insns.to_vec()).collect::<Vec<_>>();
    for (block_index, block) in body.blocks.iter().enumerate() {
        let insns = &blocks[block_index];
        // Moves change no flag, so the three may have a phi's copies between
        // them.
        let work: Vec<usize> =
            insns.iter().enumerate().filter(|(_, one)| !_skippable_nothing(one)).map(|(index, _)| index).collect();
        let at = |position: isize| work[usize::try_from(position).expect("a non-negative position")];
        // A two-way branch may end in a jump to its other side.
        let jumps =
            work.last().is_some_and(|&last| insns[last].what.as_ref().is_some_and(|what| what.op == Operation::Jump));
        let branch_at = work.len() as isize - 1 - isize::from(jumps);
        let mut test_at = branch_at - 1;
        // Moves and anchors change no flag.
        while test_at >= 0 && (_moves(&insns[at(test_at)], None) || _nothing(&insns[at(test_at)])) {
            test_at -= 1;
        }
        if test_at < 0 {
            continue;
        }
        let branch = &insns[at(branch_at)];
        if !branch.what.as_ref().is_some_and(|what| {
            what.op == Operation::Branch && what.name.as_deref().is_some_and(|name| rules.zero_jcc.contains(name))
        }) || !live[&block.at].is_disjoint(&_DIFFERING)
        {
            continue;
        }
        // A cell's step sets the flags its zero test would, where only
        // anchors stand between them.
        if let Some(cell) = _zero_tested_cell(&insns[at(test_at)]) {
            let mut step_at = test_at - 1;
            while step_at >= 0 && _nothing(&insns[at(step_at)]) {
                step_at -= 1;
            }
            if step_at >= 0 && _sets_cell(&insns[at(step_at)], &cell) {
                let test = Arc::clone(&insns[at(test_at)]);
                blocks[block_index][at(test_at)] = Arc::new(Insn {
                    what: Some(semantics(Operation::Nothing, "", vec![], vec![])),
                    defines: Vec::new(),
                    uses: Vec::new(),
                    widths: Vec::new(),
                    ..(*test).clone()
                });
            }
            continue;
        }
        let Some(register) = _zero_tested(&insns[at(test_at)]) else {
            continue;
        };
        // The straight line into the test: a sole predecessor's work, then the
        // block's.
        let sole = match predecessors.get(&block.at).map(Vec::as_slice) {
            Some([one]) if *one != block_index && body.blocks[*one].succ == [block.at] => Some(*one),
            _ => None,
        };
        let mut line = Vec::new();
        if let Some(one) = sole {
            let before = &blocks[one];
            let jumps =
                before.last().and_then(|last| last.what.as_ref()).is_some_and(|what| what.op == Operation::Jump);
            line.extend((0..before.len() - usize::from(jumps)).map(|index| (one, index)));
        }
        line.extend((0..at(test_at)).map(|index| (block_index, index)));
        let Some(source) = _flag_source(body.bits, &blocks, &line, &register) else {
            continue;
        };
        let (from, from_index) = line[source];
        let setter = blocks[from].remove(from_index);
        let test_index = at(test_at) - usize::from(from == block_index);
        let test = Arc::clone(&blocks[block_index][test_index]);
        blocks[block_index][test_index] = Arc::new(Insn {
            what: Some(semantics(Operation::Nothing, "", vec![], vec![])),
            defines: Vec::new(),
            uses: Vec::new(),
            widths: Vec::new(),
            ..(*test).clone()
        });
        blocks[block_index].insert(test_index, setter);
    }
    body.with_blocks(body.blocks.iter().zip(blocks).map(|(block, insns)| block.with_insns(insns)).collect())
}

/// The position in `line` of what set `register` last, when nothing after it
/// reads flags, touches its operands or its result, or accesses a register it
/// reads: it may then run last, and its flags are `register`'s zero test.
fn _flag_source(
    bits: u32,
    blocks: &[Vec<Arc<Insn>>],
    line: &[(usize, usize)],
    register: &Reg,
) -> Option<usize> {
    let registers = |lanes: &Lanes| {
        let mut lanes = *lanes;
        lanes.retain(|lane| lane.0 != Register::None);
        lanes
    };
    let mut crossed = Vec::new();
    for (position, (block, index)) in line.iter().enumerate().rev() {
        let one = &blocks[*block][*index];
        if _skippable_nothing(one) {
            continue;
        }
        let effect = liveness::effect(bits, one)?;
        if _sets_from(one, register) {
            let operands_in_registers = one
                .what
                .as_ref()
                .is_some_and(|what| what.sources.iter().all(|source| matches!(source, Loc::Reg(_) | Loc::Imm(_))));
            let (reads, writes) = (registers(&effect.reads), registers(&effect.writes));
            return (operands_in_registers
                && crossed.iter().all(|other: &liveness::Effect| {
                    other.writes.is_disjoint(&reads)
                        && other.reads.is_disjoint(&writes)
                        && other.writes.is_disjoint(&writes)
                }))
            .then_some(position);
        }
        let transfers = one
            .what
            .as_ref()
            .is_none_or(
                |what| [Operation::Branch, Operation::Jump, Operation::Call, Operation::Return].contains(&what.op),
            );
        if transfers || !one.clobbers.is_empty() || effect.reads.iter().any(|lane| lane.0 == Register::None) {
            return None;
        }
        crossed.push(effect);
    }
    None
}

/// The flag lanes `one` reads and writes: the one answer flags liveness has,
/// forwards and back. What a callee, a caller after a return, and whatever runs
/// after the body leaves may read: no calling convention passes the adjust flag
/// in or out, so only an instruction here that reads AF reads it.
fn _exit_flags() -> Lanes {
    _flag_lanes(_ARITHMETIC).minus(&_ADJUST)
}

pub fn _flag_effect(
    bits: u32,
    one: &Insn,
) -> (Lanes, Lanes) {
    let every = _flag_lanes(_ARITHMETIC);
    if _nothing(one) {
        // One that clobbers is an opaque barrier, which may observe any flag.
        return if one.clobbers.is_empty() { (Lanes::new(), Lanes::new()) } else { (every, Lanes::new()) };
    }
    if let Some(effect) = liveness::effect(bits, one) {
        let flags = |lanes: &Lanes| lanes.iter().copied().filter(|lane| lane.0 == Register::None).collect::<Lanes>();
        return (flags(&effect.reads), flags(&effect.writes));
    }
    if one.what.as_ref().is_some_and(|what| [Operation::Call, Operation::Return].contains(&what.op)) {
        return (_exit_flags(), Lanes::new());
    }
    // Bytes this cannot encode -- a relocated operand, an x87 form --
    // still name their instruction, and only a few instructions read
    // a flag. Writes stay unknown, which only keeps flags live longer.
    if let Some(name) = one.what.as_ref().and_then(|what| what.name.as_deref()) {
        if !name.is_empty() && !_reads_flags(name) {
            let written = match name {
                "add" | "sub" | "and" | "or" | "xor" | "cmp" | "test" | "neg" => every,
                // They keep the carry.
                "inc" | "dec" => every.minus(&_flag_lanes(RflagsBits::CF)),
                _ => Lanes::new(),
            };
            return (Lanes::new(), written);
        }
    }
    (every, Lanes::new())
}

static _ADJUST: LazyLock<Lanes> = LazyLock::new(|| _flag_lanes(RflagsBits::AF));
/// Every mnemonic iced-x86 knows to read an arithmetic flag, and every
/// interrupt: it hands the flags to the handler.
static _FLAG_READERS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    iced_x86::Code::values()
        .filter(|code| {
            let mut one = iced_x86::Instruction::default();
            one.set_code(*code);
            one.rflags_read() & _ARITHMETIC != 0 || one.flow_control() == FlowControl::Interrupt
        })
        .map(|code| format!("{:?}", code.mnemonic()).to_lowercase())
        .collect()
});

pub(crate) fn _reads_flags(name: &str) -> bool {
    _FLAG_READERS.contains(name)
}

/// `cmp r,0; jcc` is `or r,r; jcc`, a byte shorter (`peephole.peep`).
pub fn zero_compares(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.zero_compares, body, &Facts::new(body, None))
}

/// A plain move, writing nothing that shares a root with `register`.
fn _moves(
    one: &Insn,
    register: Option<&Reg>,
) -> bool {
    let Some(what) = &one.what else {
        return false;
    };
    if what.op != Operation::Move || what.name.as_deref() != Some("mov") || !one.clobbers.is_empty() {
        return false;
    }
    register.is_none_or(|register| {
        what.dests
            .iter()
            .all(
                |dest| match dest {
                    Loc::Reg(dest) => ir::root(dest.register) != ir::root(register.register),
                    _ => true,
                },
            )
    })
}

pub fn _nothing(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty))
}

/// A no-op with no virtual edge, safe to skip for physical adjacency.
pub fn _skippable_nothing(one: &Insn) -> bool {
    _nothing(one) && one.defines.is_empty() && one.uses.is_empty()
}

fn _zero_tested(one: &Insn) -> Option<Reg> {
    if !one.clobbers.is_empty() || one.symbol == Some(true) {
        return None;
    }
    let what = one.what.as_ref()?;
    match (what.op, what.name.as_deref(), what.sources.as_slice()) {
        (Operation::Compare, Some("cmp"), [Loc::Reg(register), Loc::Imm(Imm { value: 0, address: None, .. })]) => {
            Some(*register)
        }
        (Operation::Compare, Some("test"), [Loc::Reg(register), Loc::Reg(other)]) if other == register => {
            Some(*register)
        }
        _ => None,
    }
}

/// The memory cell `cmp [m],0` tests.
fn _zero_tested_cell(one: &Insn) -> Option<Mem> {
    if !one.clobbers.is_empty() || one.symbol == Some(true) {
        return None;
    }
    match (one.what.as_ref()?.op, one.what.as_ref()?.name.as_deref(), one.what.as_ref()?.sources.as_slice()) {
        (Operation::Compare, Some("cmp"), [Loc::Mem(cell), Loc::Imm(Imm { value: 0, address: None, .. })]) => {
            Some(cell.clone())
        }
        _ => None,
    }
}

fn _sets_cell(
    one: &Insn,
    cell: &Mem,
) -> bool {
    if !one.clobbers.is_empty() {
        return false;
    }
    let Some(what) = &one.what else {
        return false;
    };
    match (what.op, what.name.as_deref(), what.dests.as_slice()) {
        (Operation::Binary, Some("add" | "sub" | "and" | "or" | "xor"), [Loc::Mem(dest)])
        | (Operation::Unary, Some("inc" | "dec" | "neg"), [Loc::Mem(dest)]) => dest == cell,
        _ => false,
    }
}

fn _sets_from(
    one: &Insn,
    register: &Reg,
) -> bool {
    if !one.clobbers.is_empty() {
        return false;
    }
    let Some(what) = &one.what else {
        return false;
    };
    match (what.op, what.name.as_deref(), what.dests.as_slice()) {
        (Operation::Binary, Some("add" | "sub" | "and" | "or" | "xor"), [Loc::Reg(dest)]) => dest == register,
        (Operation::Unary, Some("inc" | "dec" | "neg"), [Loc::Reg(dest)]) => dest == register,
        _ => false,
    }
}

/// Which flag lanes something may read after each block's last instruction.
pub fn _flags_live_out(body: &LirBody) -> HashMap<i64, Lanes> {
    let exits = _exit_flags();
    let effects = |one: &Insn| _flag_effect(body.bits, one);

    let steps: HashMap<i64, Vec<(Lanes, Lanes)>> =
        body.blocks.iter().map(|block| (block.at, block.insns.iter().map(|one| effects(one)).collect())).collect();
    let nodes: Vec<&LirBlock> = body.blocks.iter().collect();
    let blocks: HashMap<i64, &LirBlock> = nodes.iter().map(|block| (block.at, *block)).collect();
    // The solution's output is what is live on entry; what a block's last
    // instruction leaves is its input.
    let solved = dataflow::solve(
        &nodes,
        Direction::Backward,
        |_| Lanes::new(),
        |at, live_in| {
            let block = blocks[&at];
            if block.succ.is_empty() {
                exits.clone()
            } else {
                block.succ.iter().flat_map(|at| live_in.get(at).unwrap_or(&exits).iter().copied()).collect()
            }
        },
        |at, after| steps[&at].iter().rev().fold(after.clone(), |live, (reads, writes)| live.minus(writes).or(reads)),
    );
    solved.input.into_iter().collect()
}

/// Use XOR for zero only when later integer work replaces every arithmetic
/// flag.
pub fn zeroes(body: &LirBody) -> LirBody {
    let live = _flags_live_out(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        // What the block's successors read, not a guess: `mov bx,0; jmp` to a
        // block that sets its own flags first zeroes with xor too.
        let mut flags_dead = live[&block.at].is_disjoint(&_ARITHMETIC_LANES);
        let mut insns = Vec::new();
        for one in block.insns.iter().rev() {
            let mut one = Arc::clone(one);
            let previous = _flags_before(body.bits, &one, flags_dead);
            if let Some(what) = &one.what {
                if one.clobbers.is_empty() {
                    if let (
                        Operation::Move,
                        Some("mov"),
                        [Loc::Reg(dest)],
                        [Loc::Imm(Imm { value: 0, width, address: None })],
                    ) = (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
                    {
                        if flags_dead
                            && dest.width == *width
                            && [2, 4].contains(width)
                            && target::WIDTHS.contains_key(&dest.register)
                            && one.symbol != Some(true)
                        {
                            let dest = *dest;
                            one = Arc::new(with_what(
                                &one,
                                semantics(
                                    Operation::Binary,
                                    "xor",
                                    vec![Loc::Reg(dest)],
                                    vec![Loc::Reg(dest), Loc::Reg(dest)],
                                ),
                            ));
                        }
                    }
                }
            }
            flags_dead = previous;
            insns.push(one);
        }
        insns.reverse();
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

const _WAITING: [&str; 36] = [
    "wait", "fwait", "fld", "fild", "fst", "fstp", "fist", "fistp", "fadd", "faddp", "fiadd", "fsub", "fsubp", "fsubr",
    "fsubrp", "fisub", "fisubr", "fmul", "fmulp", "fimul", "fdiv", "fdivp", "fdivr", "fdivrp", "fidiv", "fidivr",
    "fchs", "fabs", "fsqrt", "fxch", "fcom", "fcomp", "fcompp", "fucom", "fucomp", "fucompp",
];

/// An immediately following waiting instruction already checks pending FP
/// exceptions.
///
/// Intel SDM Vol. 1 section 8.3.12. Never cross integer work, an unknown
/// instruction, a non-waiting control instruction, or a block boundary.
pub fn waits(body: &LirBody) -> LirBody {
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut following: Option<&Semantics> = None;
        let mut redundant: HashSet<usize> = HashSet::default();
        for one in block.insns.iter().rev() {
            let what = one.what.as_ref();
            if what.is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty))
            {
                continue;
            }
            if what.is_some_and(|what| matches!(what.name.as_deref(), Some("wait" | "fwait")))
                && following
                    .is_some_and(|following| following.name.as_deref().is_some_and(|name| _WAITING.contains(&name)))
            {
                redundant.insert(id(one));
            } else {
                following = what;
            }
        }
        let insns = lir::without(&block.insns, |one| redundant.contains(&id(one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>);
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

/// What `constants` believes a register holds: a literal, or a fresh
/// `object()` standing for another register's unknown contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Known {
    Value(i64),
    Object(usize),
}

/// Reuse identical scalar register contents until an instruction overwrites
/// them.
pub fn constants(body: &LirBody) -> LirBody {
    let mut objects = 0usize;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut held: IndexMap<Reg, Known> = IndexMap::default();
        let mut redundant: HashSet<usize> = HashSet::default();
        for one in &block.insns {
            let what = one.what.as_ref();
            if what.is_some_and(|what| {
                what.op == Operation::Nothing
                    && what.name.as_deref().is_none_or(str::is_empty)
                    && what.dests.is_empty()
                    && what.sources.is_empty()
                    && what.target.is_none()
            }) && one.clobbers.is_empty()
            {
                continue;
            }
            let moved = what.is_some_and(|what| what.op == Operation::Move && what.name.as_deref() == Some("mov"));
            let extend = what.is_some_and(|what| {
                what.op == Operation::Extend
                    && matches!(what.name.as_deref(), Some("cwd" | "cdq" | "movsx"))
                    && what.dests.len() == 1
                    && what.sources.len() == 1
                    && what.dests.iter().chain(&what.sources).all(|arg| matches!(arg, Loc::Reg(_)))
            });
            if !moved && !extend {
                match _register_effects(body.bits, one, true, false) {
                    None => held.clear(),
                    Some((_reads, writes)) => held.retain(|dest, _| _lanes(dest.register).is_disjoint(&writes)),
                }
                continue;
            }
            let what = what.expect("a move or extension has semantics");
            let mut candidate = None;
            if moved && what.dests.len() == 1 && what.sources.len() == 1 {
                let (dest, source) = (&what.dests[0], &what.sources[0]);
                if let Loc::Reg(dest) = dest {
                    let source_width = match source {
                        Loc::Reg(source) => Some(source.width),
                        Loc::Imm(source) => Some(source.width),
                        _ => None,
                    };
                    if target::WIDTHS.contains_key(&dest.register) && source_width == Some(dest.width) {
                        match source {
                            Loc::Imm(source) if source.address.is_none() => {
                                candidate =
                                    Some((*dest, Known::Value(source.value & ((1i64 << (dest.width * 8)) - 1))));
                            }
                            Loc::Reg(source) if target::WIDTHS.contains_key(&source.register) => {
                                let value = *held
                                    .entry(*source)
                                    .or_insert_with(
                                        || {
                                            objects += 1;
                                            Known::Object(objects)
                                        },
                                    );
                                candidate = Some((*dest, value));
                            }
                            _ => {}
                        }
                    }
                }
            }
            if let Some((dest, value)) = &candidate {
                if one.clobbers.is_empty() && held.get(dest) == Some(value) {
                    redundant.insert(id(one));
                    continue;
                }
            }
            let mut written: HashSet<Register> =
                one.clobbers.iter().chain(&one.clobbers_high).map(|register| full32(*register)).collect();
            written.extend(what.dests.iter().filter_map(|dest| match dest {
                Loc::Reg(dest) => Some(full32(dest.register)),
                _ => None,
            }));
            held.retain(|dest, _| !written.contains(&full32(dest.register)));
            if let Some((dest, value)) = candidate {
                if one.clobbers.is_empty() {
                    held.insert(dest, value);
                }
            }
        }
        let insns = block
            .insns
            .iter()
            .map(|one| if redundant.contains(&id(one)) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
            .collect();
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

#[cfg(test)]
#[path = "peephole_tests.rs"]
mod tests;
