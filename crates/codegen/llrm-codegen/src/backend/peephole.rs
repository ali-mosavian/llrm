//! Local rewrites of allocated LIR that need the final physical registers.
//!
//! # How it works
//! The phase is one fixed sequence of sub-passes over the body. Most are
//! generated rule groups: `peephole.peep` of the target (`llrm-x86`, plus
//! `llrm-x86-m16` or `llrm-x86-m32`) lists the groups, each with a window walk,
//! `match` patterns, `when` guards and a `rewrite`; `build.rs` turns them into
//! the matchers under `peep/`, bound to the target as `peep::Rules`. The rest
//! are hand-written functions in this file or their own modules. A window skips
//! meta instructions, so a source-map marker between two instructions never
//! stops a rewrite.
//!
//! What the passes know is physical: liveness of register lanes and flag lanes
//! (`lanes.rs`, `effects.rs`) and the CPU's prices (`Profile`). A rewrite fires
//! only when the lanes and flags it changes are dead, and, where the target has
//! a choice, when the profile prices it no dearer in cycles or bytes.
//!
//! The order is deliberate. Register renaming and copy propagation come first,
//! so every later pass sees fewer copies. Sinking of copies and spill stores
//! follows the reload forwarding, because a reload gone from the other path is
//! what lets the store leave it. Operand folding and address selection come
//! next, then the flag passes, then dead-code elimination, and last the repair
//! of what the 16-bit target's 32-bit addresses need.
//!
//! ```text
//!  frame copies -> machinecp -> extensions -> copysink
//!    -> postrasink -> forwarded reloads -> forwarded copies
//!    -> storecombine -> pushed constants
//!    -> pushes, constants, commuted, transferred, high extracts, restored
//!       copies, shuttles, overwritten, fused, fused calls, far loads,
//!       loaded segments
//!    -> exact addresses -> addresses (LEA) -> stepped after compare
//!    -> secondary bases -> increments, borrows, doubled, narrowed
//!    -> widened moves, zero extensions -> zeroed loads -> shared stores
//!    -> machine CSE -> narrowed moves, zeroes, tested, compares again,
//!       zero compares, waits -> machine DCE -> popped arguments
//!    -> frame adjust cleanup -> upper zero (non-native targets)
//! ```
//!
//! Typical rewrites, before and after:
//!
//! ```text
//!  inc edi ; cmp edi,0 ; jne L      inc edi ; jne L        (tested)
//!  cmp r,0 ; jcc                    or r,r ; jcc           (zero_compares)
//!  mov r,0 ; sbb r,0                sbb r,r                (borrows)
//!  mov z,y ; add z,x  (flags dead)  lea z,[y+x]            (addresses)
//!  mov r,[m] ; mov es,[m+2]         les r,[m]              (far_loads)
//! ```
//!
//! A sub-pass whose subject the body lacks is skipped. `needs.rs` keeps one bit
//! per candidate shape (a register copy, a cell read through an unheld 32-bit
//! register, an exact cell) per block,
//! and `gated` runs the pass only if some block has its bit. `LLRM_CHECK_NEEDS`
//! runs the skipped pass anyway and asserts it changed nothing.
//!
//! When the frame has a size but the body reads or writes no spill cell, no
//! barrier and no stack-register operand outside the frame, the synthetic frame
//! reservations (`frame_adjust`) are dropped. A native target returns before
//! the last pass: its addresses read whole registers, where a 16-bit target's
//! 32-bit address is the prefixed form of a 16-bit one and needs the upper half
//! proven zero (`upperzero.rs`).
//!
//! # Example
//! A loop counting `n` down, after allocation:
//!
//! Before:
//!
//! ```lir
//! block 0x0000
//!   0x0000  AX := mov 0x1  ; inserted
//!   0x0000  BX := mov 0x0  ; inserted
//!   0x0000  jmp   ; inserted
//! block 0x0001
//!   0x0004  CX := mov DX  ; inserted
//!   0x0004  CX := sub CX, 0x1  ; inserted
//!   0x0006  cmp DX, 0x0  ; inserted
//!   0x0006  jne   ; inserted
//! ```
//!
//! After:
//!
//! ```lir
//! block 0x0000
//!   0x0000  AX := mov 0x1  ; inserted
//!   0x0000  BX := xor BX, BX  ; inserted
//!   0x0000  jmp   ; inserted
//! block 0x0001
//!   0x0004  CX := mov DX  ; inserted
//!   0x0004  CX := dec CX  ; inserted
//!   0x0006  DX := or DX, DX  ; inserted
//!   0x0006  jne   ; inserted
//! ```
//!
//! The zero load became `xor`, the subtraction of one `dec`, and the compare
//! with zero `or`, which sets the flags `jne` reads as `cmp` did.
//!
//! From `ISEL_DUMP=1` of a C countdown loop compiled with `llrm-c -O2 -m16`:
//! the `ParallelCopy` and `Peephole` stages.
//!
//! # Runs
//! Once per body, as the machine phase after `prologue` and before `loopslots`
//! (`flow::machine_with`), in every machine of every candidate allocation.
//! There is no flag to turn it off; at -O1 and up it runs the same rounds, as
//! gcc's `-fpeephole2` does.
//!
//! # Needs
//! Allocated, two-address LIR with the frame laid out
//! ([`crate::backend::allocate`], [`crate::backend::prologue`]). Liveness comes
//! from [`crate::backend::liveness`].
//!
//! # See also
//! - gcc `-fpeephole2`:
//!   <https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/opts.cc#L638>

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

use iced_x86::{FlowControl, RflagsBits};
use llrm_lir::registers::RegId;
use llrm_lir::registers::Regs;

use crate::analysis::dataflow::{self, Direction};
use crate::backend::classes::RegisterClasses;
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::frame::Frame;
use crate::backend::intbits::{low_mask, wrap_to};
pub use crate::backend::lanes::{Lane, Lanes};
use crate::backend::needs::{self, gated};
use crate::backend::peep::{self, walk::Facts};
use crate::backend::{affine, copysink, liveness, machinedce, phielim, regthrash, select, sharedstores, storecombine};
use crate::model::ir::{self, Addr, AddressRef, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{self, Block, Insn, LirBlock, LirBody};
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
    Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
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
    register: RegId,
    width: u32,
) -> Reg {
    Reg { register, width }
}

/// The root `register` is a view of, in the target's register file.
fn full32(
    regs: Regs,
    register: RegId,
) -> RegId {
    regs.root(register)
}

pub struct Peephole {
    /// One frame, shared with the phases before this one, as Python shares it.
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub cpu: Profile,
    /// The rule groups of the target, which its driver binds.
    pub rules: &'static peep::Rules,
    /// The registers a callee keeps for its caller, whole: a run of stores may
    /// not share them.
    pub saved: Vec<RegId>,
    pub classes: Rc<RegisterClasses>,
    /// DEVIATE ZL1 applies: `movzx` byte loads become the register cleared and
    /// its low byte loaded, where the target prices the pair lower.
    pub zeroed_loads: bool,
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
            &crate::backend::targets::x86_m16::RULES,
            llrm_x86_m16::PRESERVED.iter().map(|(whole, _)| *whole).collect(),
            RegisterClasses::m16(),
            crate::backend::shadow::Selector::Old,
        )
    }

    pub fn with_rules<'a>(
        frame: Option<Rc<RefCell<Frame>>>,
        cpu: impl Into<ProfileOrName<'a>>,
        rules: &'static peep::Rules,
        saved: Vec<RegId>,
        classes: Rc<RegisterClasses>,
        _selector: crate::backend::shadow::Selector,
    ) -> Result<Self, String> {
        Ok(Self { frame, cpu: targets::profile(cpu)?.clone(), rules, saved, classes, zeroed_loads: true })
    }

    /// The frame without the spill slots no cell names, and the cells where
    /// they moved to.
    fn released(
        &self,
        body: LirBody,
    ) -> LirBody {
        match &self.frame {
            Some(frame) => crate::backend::slots::released(&body, &mut frame.borrow_mut()),
            None => body,
        }
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
        // The reserved call area is stack storage no cell names a spill of.
        if frame.size() == 0 || frame.call_area != 0 {
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
                let (address, through) = match (arg.address(), arg) {
                    (Some(one), _) => (one.addr, Some(one.through)),
                    (None, Loc::Imm(value)) => (value.address, None),
                    _ => continue,
                };
                if address.is_none() && through.is_some() {
                    return body;
                }
                if through.is_some_and(|through| [RegId::BP, RegId::EBP, RegId::SP, RegId::ESP].contains(&through))
                    && address.is_none_or(|address| address.space != Space::Frame)
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
        let mut seen = needs::Contents::default();
        let body = phielim::unsplit(&body);
        let body = gated(&mut seen, needs::COPY, body, |body| Ok(crate::backend::machinecp::propagated(&body)))?;
        let body = extensions(self.rules, &body);
        let body = copysink::sunk(&body);
        // After the forwarding: a reload on the other path is gone, and the
        // store need not run there.
        let body = crate::backend::postrasink::latched(&crate::backend::postrasink::sunk(&body));
        let body = match crate::backend::slotcopies::merged(&body) {
            Some(merged) => self.released(merged),
            None => body,
        };
        let body = forwarded_reloads(self.rules, &body);
        let body = forwarded_copies(self.rules, &body);
        let body = unread_spill_stores(&body);
        let body = crate::backend::deadspills::dead_spill_stores(&body);
        let body = storecombine::combined(&body);
        let body = pushed_constants(self.rules, &body);
        let body = far_loads(
            self.rules,
            &fused_calls(
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
            ),
        );
        let body = loaded_segments(self.rules, &body);
        let body = gated(&mut seen, needs::EXACT_CELL, body, |body| {
            crate::backend::exactaddress::exact_addresses(&body, &self.cpu)
        })?;
        let body = addresses(&body, &self.cpu)?;
        let body = stepped_after_compare(self.rules, &body);
        let body = borrows(self.rules, &increments(self.rules, &body));
        let body = doubled(self.rules, &body, &self.cpu)?;
        let body = narrowed_arithmetic(self.rules, &body);
        // A zero upper half proven by the first is what the second reads, and
        // the copies it makes are forwarded.
        let body = if self.rules.zero_extensions.is_some() {
            let body =
                zero_extensions(self.rules, &widened_moves(self.rules, &crate::backend::upperzero::hoisted(&body)));
            gated(&mut seen, needs::COPY, body, |body| Ok(crate::backend::machinecp::propagated(&body)))?
        } else {
            body
        };
        let body = if self.zeroed_loads { zeroed_loads(self.rules, &body, &self.cpu)? } else { body };
        let body = sharedstores::shared(&body, &self.cpu, &self.saved, &self.classes);
        let body = zero_compares(
            self.rules,
            &compares_again(&tested(self.rules, &zeroes(&narrowed_moves(self.rules, &body)))),
        );
        let body = popped_arguments(&machinedce::eliminated(body), &self.cpu)?;
        let body = crate::backend::repbsf::marked(&body, &self.cpu);
        // Last: EBP zeroed above for each cell reading it 32 bits wide, where a
        // 32-bit address is the prefixed form of a 16-bit one. A native
        // one reads whole registers.
        let body = self._frame(body);
        let native = self.cpu.dword_address_form().is_some_and(|form| !form.secondary);
        if native {
            return Ok(body);
        }
        gated(&mut seen, needs::UNHELD, body, |body| Ok(crate::backend::upperzero::established(&body)))
    }
}

/// A call's `add sp,unit*n` as `n` pops into a dead scratch register, as BCC
/// -Os cleans them, where the CPU prices it (`Profile::pops_arguments`). A
/// pop takes back the mode's word: 2 bytes in 16-bit code, 4 in 32-bit. The
/// add's flags must be dead: a pop sets none.
pub fn popped_arguments(
    body: &LirBody,
    cpu: &Profile,
) -> Result<LirBody, String> {
    let regs = body.regs();
    let unit = i64::from(body.bits / 8);
    let arithmetic =
        RflagsBits::OF | RflagsBits::SF | RflagsBits::ZF | RflagsBits::AF | RflagsBits::CF | RflagsBits::PF;
    // DEVIATE: the pin pops only at -Oz; the legacy -Os pop keeps -Os m16 <=
    // main. The target's stack register: `add sp, n` at 16 bits,
    // selection's `add esp, n` at 32.
    let sp = Loc::Reg(reg(regs.named(regs.stack, unit), unit as u32));
    // The words an `add sp, n` takes back, or 0.
    let words_of = |one: &Insn| match &one.what {
        Some(what)
            if what.op == Operation::Binary && what.name.as_deref() == Some("add") && what.dests == [sp.clone()] =>
        {
            match what.sources.as_slice() {
                [first, Loc::Imm(Imm { value, address: None, .. })]
                    if *first == sp && value % unit == 0 && *value > 0 =>
                {
                    value / unit
                }
                _ => 0,
            }
        }
        _ => 0,
    };
    // Solved for the first block that has a cleanup, and not at all for a
    // body that has none.
    let mut exits = None;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        if block.insns.iter().all(|one| words_of(one) == 0) {
            blocks.push(block.clone());
            continue;
        }
        let exits = exits.get_or_insert_with(|| liveness::dead_at_exit(body));
        let dead_after = regthrash::_dead_after(regs, body.bits, block, exits[&block.at].clone());
        let mut insns = Vec::with_capacity(block.insns.len());
        for one in &block.insns {
            let words = words_of(one);
            let dead = &dead_after[&id(one)];
            let scratch = regs.scratch.iter().copied().find(|register| _lanes(regs, *register).is_subset(dead));
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
                            semantics(
                                Operation::Pop,
                                "pop",
                                vec![Loc::Reg(reg(regs.named(register, unit), unit as u32))],
                                vec![],
                            ),
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

/// Fold a load or transitive extension into one widening instruction
/// (`peephole.peep`).
pub fn extensions(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.extensions, body, &Facts::new(body, None))
}

/// `movzx r,m8` as the register cleared and its low byte loaded, where the
/// target prices the pair lower (`peephole.peep`).
///
/// DEVIATE ZL1 (owner w1, w2 for the DAG path; permanent target tuning): the
/// pin keeps `movzx` on the i486 as on every x86 (X86InstrCompiler.td keeps
/// `zextloadi8` as `MOVZX32rm8`); this pair is two clocks against three there.
/// Both selectors run it since 2026-10-31 (textfill c -O2 m16, +4000 clocks
/// over main with the movzx). Gated on
/// `Profile::clears_before_byte_load`, so off for size and where the extension
/// is no slower or the partial register stalls. x_life -O2 m32 has 12 more
/// `xor` than the pin from it.
pub fn zeroed_loads(
    rules: &peep::Rules,
    body: &LirBody,
    cpu: &Profile,
) -> Result<LirBody, String> {
    cpu.clears_before_byte_load()?;
    Ok(peep::rewritten(rules.zeroed_loads, body, &Facts::new(body, Some(cpu))))
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

pub fn _lanes(
    regs: Regs,
    register: RegId,
) -> Lanes {
    if regs.is_segment(register) {
        let width = regs.width_of(register).expect("a segment register has a width");
        return Lanes::span(register, 0, width as u32);
    }
    let full = full32(regs, register);
    if !regs.in_class(full, llrm_lir::registers::class::INT) || regs.is_stack(full) {
        return Lanes::new();
    }
    let start = regs.get(register).map_or(0, |one| one.lane / 8);
    Lanes::span(full, start, register.size() as u32)
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
    let regs = body.regs();
    let profile = targets::profile(cpu)?;
    let mut virtual_uses = Counter::default();
    // The value a bare anchor (an instruction with no operation, left where a
    // coalesced copy was) hands on: its one use to its one definition.
    let mut anchors: std::collections::BTreeMap<u32, u32> = Default::default();
    for block in &body.blocks {
        for one in &block.insns {
            for value in &one.uses {
                *virtual_uses.entry(*value).or_insert(0) += 1;
            }
            if one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing)
                && let ([from], [to]) = (one.uses.as_slice(), one.defines.as_slice())
            {
                anchors.insert(*from, *to);
            }
        }
    }
    // Worked out for the first window that gets as far as asking.
    let exits = std::cell::OnceCell::new();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let dead = std::cell::OnceCell::new();
        let dead_after = |at: usize| -> Lanes {
            dead.get_or_init(|| {
                let exits = exits.get_or_init(|| liveness::dead_at_exit(body));
                regthrash::_dead_after(regs, body.bits, block, exits[&block.at].clone())
            })[&at]
                .clone()
        };
        let sunk = _copies_sunk_to_high_shifts(regs, body.bits, &block.insns);
        let insns = _code_windows(sunk.as_deref().unwrap_or(&block.insns), 3, |run| -> Result<_, String> {
            if run.len() == 3 {
                if let Some(changed) =
                    _register_high_extract(regs, body.bits, run, &dead_after, &virtual_uses, profile)?
                {
                    return Ok(Some((3, changed)));
                }
            }
            if run.len() < 2 {
                return Ok(None);
            }
            let pair = &run[..2];
            Ok(
                match _selected_register_high_extract(
                    regs,
                    body.bits,
                    pair,
                    &dead_after,
                    &virtual_uses,
                    &anchors,
                    profile,
                )? {
                    Some(changed) => Some((2, changed)),
                    None => _high_extract(regs, body.bits, pair, &dead_after).map(|changed| (2, changed)),
                },
            )
        })?;
        blocks.push(block.with_insns(insns));
    }
    Ok(body.with_blocks(blocks))
}

/// `mov dst, src` moved down to the `shr dst, 16` it feeds when the code
/// between neither reads or writes `dst` nor writes `src` (an allocation can
/// put a spill store of `src` in between), so the pair is adjacent for the high
/// extract. `None` where nothing moved.
fn _copies_sunk_to_high_shifts(
    regs: Regs,
    bits: u32,
    insns: &[Arc<Insn>],
) -> Option<Vec<Arc<Insn>>> {
    const REACH: usize = 6;
    let shifted = |one: &Insn| -> Option<Reg> {
        let what = one.what.as_ref()?;
        match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
            (
                Operation::Binary,
                Some("shr"),
                [Loc::Reg(dest @ Reg { width: 4, .. })],
                [Loc::Reg(read), Loc::Imm(count)],
            ) if dest == read && count.value == 16 => Some(*dest),
            _ => None,
        }
    };
    let mut out: Option<Vec<Arc<Insn>>> = None;
    for shift_at in 0..insns.len() {
        let list = out.as_deref().unwrap_or(insns);
        let Some(dest) = shifted(&list[shift_at]) else { continue };
        let dest_lanes = _lanes(regs, dest.register);
        let mut between: Vec<&Arc<Insn>> = Vec::new();
        for at in (0..shift_at).rev().take(REACH + 1) {
            let one = &list[at];
            if one.is_meta() {
                continue;
            }
            let Some((_, source)) = crate::backend::copysink::copy_of(one).filter(|(copied, _)| *copied == dest) else {
                between.push(one);
                continue;
            };
            if ir::root(dest.register) != ir::root(source.register) && !between.is_empty() {
                let source_lanes = _lanes(regs, source.register);
                let clear = between
                    .iter()
                    .all(
                        |other| other.what.is_some()
                            && other.clobbers.is_empty()
                            && !crate::backend::copysink::touches(regs, bits, other, &dest_lanes, true)
                            && !crate::backend::copysink::touches(regs, bits, other, &dest_lanes, false)
                            && !crate::backend::copysink::touches(regs, bits, other, &source_lanes, false),
                    );
                if clear {
                    let mut moved = list.to_vec();
                    let copy = moved.remove(at);
                    moved.insert(shift_at - 1, copy);
                    out = Some(moved);
                }
            }
            break;
        }
    }
    out
}

fn _register_high_extract(
    regs: Regs,
    bits: u32,
    parts: &[Arc<Insn>],
    dead_after: &dyn Fn(usize) -> Lanes,
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
    let wide_high = reg(regs.named(high.register, 4), 4);
    let count = Loc::Imm(imm(16, 1));
    let shift =
        semantics(Operation::Binary, "shr", vec![Loc::Reg(wide_high)], vec![Loc::Reg(wide_high), count.clone()]);

    let Some(effects) = _register_effects(regs, bits, &with_what(kept, shift.clone()), false, true) else {
        return Ok(None);
    };
    let upper: Lanes = _lanes(regs, wide_high.register).minus(&_lanes(regs, high.register));
    let flags: Lanes = effects.1.and(&_flag_lanes(0xFFFF_FFFF));
    if !upper.or(&flags).is_subset(&dead_after(id(kept))) {
        return Ok(None);
    }
    let old_cost = cpu.cost("push_r")? + 2 * cpu.cost("pop_r")?;
    if ir::root(source.register) == ir::root(high.register) {
        // The low-half extraction has already copied the return value away.
        // Reusing the dying source root for its own high half needs no move.
        // Lanes outside HIGH must die here because SHR writes the whole root.
        if _lanes(regs, source.register)
            .difference(&_lanes(regs, high.register))
            .copied()
            .collect::<Lanes>()
            .is_subset(&dead_after(id(kept)))
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
    regs: Regs,
    bits: u32,
    parts: &[Arc<Insn>],
    dead_after: &dyn Fn(usize) -> Lanes,
    virtual_uses: &Counter,
    anchors: &std::collections::BTreeMap<u32, u32>,
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
    // The copy's value reaches the shift through an anchor that is its one use.
    let anchored = shift.defines == shift.uses
        && count(virtual_uses, moved.defines[0]) == 1
        && anchors.get(&moved.defines[0]) == Some(&shift.uses[0]);
    if !(shared_two_address || temporary_chain || anchored) {
        return Ok(None);
    }

    let count = Loc::Imm(imm(16, 1));
    let extract = semantics(
        Operation::Funnel,
        "shld",
        vec![Loc::Reg(destination)],
        vec![Loc::Reg(destination), Loc::Reg(source), count],
    );
    let old_effects = _register_effects(regs, bits, shift, false, true);
    let new_effects = _register_effects(regs, bits, &with_what(moved, extract.clone()), false, true);
    let (Some(old_effects), Some(new_effects)) = (old_effects, new_effects) else {
        return Ok(None);
    };
    let low = regs.named(destination.register, 2);
    let upper: Lanes = _lanes(regs, destination.register).minus(&_lanes(regs, low));
    let flags: Lanes = old_effects
        .1
        .union(&new_effects.1)
        .copied()
        .collect::<Lanes>()
        .intersection(&_flag_lanes(0xFFFF_FFFF))
        .copied()
        .collect();
    if !upper.or(&flags).is_subset(&dead_after(id(shift))) {
        return Ok(None);
    }
    let new_cost = cpu.cost("shift_ri")?;
    let old_cost = cpu.cost("mov_rr")? + cpu.cost("shift_ri")?;
    if new_cost > old_cost {
        return Ok(None);
    }

    // The anchor still reads the copy's value: the combined instruction defines
    // it too.
    let mut defines = shift.defines.clone();
    if anchored && !(shared_two_address || temporary_chain) {
        defines.extend(moved.defines.iter().copied());
    }
    let combined = Arc::new(Insn { what: Some(extract), defines, ..(**moved).clone() });
    let folded = lir::without(
        &[combined, Arc::clone(shift)],
        |one| Arc::ptr_eq(one, shift),
        None::<fn(&Arc<Insn>) -> Arc<Insn>>,
    );
    Ok(Some(folded))
}

fn _high_extract(
    regs: Regs,
    bits: u32,
    parts: &[Arc<Insn>],
    dead_after: &dyn Fn(usize) -> Lanes,
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

    let low = reg(regs.named(wide.register, 2), 2);
    let preserved: Lanes = _lanes(regs, wide.register).minus(&_lanes(regs, low.register));
    let effects = _register_effects(regs, bits, shift, false, true)?;
    let flags: Lanes = effects.1.and(&_flag_lanes(0xFFFF_FFFF));
    if !preserved.or(&flags).is_subset(&dead_after(id(shift))) {
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

/// Drop the reload of a cell into the register that filled it
/// (`peephole.peep`, `forwarded_reloads`).
pub fn forwarded_reloads(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.forwarded_reloads, body, &Facts::new(body, None))
}

/// The frame slots that are the allocator's own: one a spill store fills, whose
/// address nothing takes and no debug variable names. No program pointer
/// reaches such a slot, so only an access that names the slot can change it.
pub fn spill_homes(body: &LirBody) -> BTreeSet<i64> {
    let home = |addr: Option<Addr>| addr.and_then(|a| a.slot_home()).filter(|home| *home < 0);
    let mut spilled: BTreeSet<i64> = BTreeSet::new();
    let mut named: BTreeSet<i64> = BTreeSet::new();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        let Some(what) = one.what.as_ref() else { continue };
        for place in what.dests.iter().chain(&what.sources) {
            match place {
                Loc::Address(a) => named.extend(home(a.addr)),
                Loc::Mem(cell) if one.spill_store => spilled.extend(home(cell.addr)),
                _ => {}
            }
        }
    }
    for variable in &body.variables {
        if let lir::DebugPlace::At(addr) = &variable.place {
            named.extend(addr.slot_home());
        }
    }
    spilled.retain(|home| !named.contains(home));
    spilled
}

/// The slot a frame cell belongs to, for a slot of the allocator's own.
pub fn frame_home(place: &Loc) -> Option<i64> {
    let Loc::Mem(cell) = place else { return None };
    cell.addr.filter(|addr| addr.space == Space::Frame).and_then(|addr| addr.slot_home()).filter(|home| *home < 0)
}

/// The slot a plain `mov [slot], reg|imm` spill store fills; nothing else
/// about the instruction (group, requirements, clobbers) may matter.
pub fn plain_spill_store(one: &Insn) -> Option<i64> {
    let what = one.what.as_ref()?;
    let plain = what.op == Operation::Move
        && what.name.as_deref() == Some("mov")
        && one.spill_store
        && one.group.is_none()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty()
        && one.spread.is_empty()
        && !one.volatile
        && !one.frame_adjust
        && one.symbol != Some(true);
    match (&what.dests[..], &what.sources[..]) {
        ([place @ Loc::Mem(cell)], [Loc::Reg(_) | Loc::Held(_) | Loc::Imm(_)])
            if plain && cell.index.is_none() && cell.index_through == RegId::None =>
        {
            frame_home(place)
        }
        _ => None,
    }
}

/// `body` without the stores to a spill slot nothing reads. The forwarding
/// above can drop the last reload of a slot and leave the store that filled
/// it. A slot is read by any operand but the destination of a plain store; a
/// slot a debug variable names stays.
pub fn unread_spill_stores(body: &LirBody) -> LirBody {
    let home = frame_home;
    let store_home = plain_spill_store;
    let mut read: BTreeSet<i64> = BTreeSet::new();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        let Some(what) = one.what.as_ref() else { continue };
        let skip = store_home(one).is_some();
        for (at, place) in what.dests.iter().chain(&what.sources).enumerate() {
            if skip && at == 0 {
                continue;
            }
            match place {
                Loc::Address(a) => {
                    read.extend(a.addr.filter(|addr| addr.space == Space::Frame).and_then(|a| a.slot_home()))
                }
                other => read.extend(home(other)),
            }
        }
    }
    for variable in &body.variables {
        if let lir::DebugPlace::At(addr) = &variable.place {
            read.extend(addr.slot_home());
        }
    }
    let dead = |one: &Arc<Insn>| store_home(one).is_some_and(|home| !read.contains(&home));
    if !body.blocks.iter().flat_map(|block| block.insns.iter()).any(dead) {
        return body.clone();
    }
    body.with_blocks(
        body.blocks
            .iter()
            .map(|block| block.with_insns(lir::without(&block.insns, dead, None::<fn(&Arc<Insn>) -> Arc<Insn>>)))
            .collect(),
    )
}

/// Copy the register that filled a cell instead of reloading the cell into
/// another register (`peephole.peep`, `forwarded_copies`).
pub fn forwarded_copies(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.forwarded_copies, body, &Facts::new(body, None))
}

/// Whether `register` in an operand at `addr` is the frame base, which the
/// frame layout resolves: not a read of the frame register, so a copy of that
/// register does not rename it.
pub fn is_frame_base(
    regs: Regs,
    addr: Option<Addr>,
    register: RegId,
) -> bool {
    addr.is_some_and(|addr| addr.space == Space::Frame) && regs.is_frame(register)
}

/// One allocated operand with aliases of `before` renamed to `after`.
pub fn _register_operand(
    regs: Regs,
    one: &Loc,
    before: RegId,
    after: RegId,
) -> Loc {
    // `target.named(after, target.width_of(x))`: a width the target does not
    // know leaves `after` as named.
    let named = |register: RegId| regs.width_of(register).map_or(after, |width| regs.named(after, width));
    let renames = |register: RegId, addr: Option<Addr>| {
        ir::root(register) == ir::root(before) && !is_frame_base(regs, addr, register)
    };
    match one {
        Loc::Reg(one) if ir::root(one.register) == ir::root(before) => {
            Loc::Reg(Reg { register: regs.named(after, i64::from(one.width)), ..*one })
        }
        Loc::Mem(one) => {
            let (through, index) = (renames(one.through, one.addr), renames(one.index_through, None));
            if !through && !index {
                return Loc::Mem(one.clone());
            }
            Loc::Mem(Mem {
                through: if through { named(one.through) } else { one.through },
                index_through: if index { named(one.index_through) } else { one.index_through },
                ..one.clone()
            })
        }
        Loc::Address(one) => {
            let through = if renames(one.through, one.addr) { named(one.through) } else { one.through };
            let index = if renames(one.index_through, None) { named(one.index_through) } else { one.index_through };
            Loc::Address(AddressRef { through, index_through: index, ..one.clone() })
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
    regs: Regs,
    bits: u32,
    one: &Insn,
) -> Option<(Vec<(Lane, Lane)>, Lanes)> {
    let _ = bits;
    _moved_by(regs, one.what.as_ref().and_then(constant_shift)?)
}

/// A shift by a constant: whether it is left, its destination, the register
/// shifted in, and the count.
pub type Shift = (bool, Option<RegId>, Option<RegId>, u8);

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

pub fn _moved_by(
    regs: Regs,
    (left, destination, source, count): Shift,
) -> Option<(Vec<(Lane, Lane)>, Lanes)> {
    let destination = destination.filter(|one| matches!(one.size(), 2 | 4) && !_lanes(regs, *one).is_empty())?;
    if source.is_some_and(|one| one.size() != destination.size() || _lanes(regs, one).is_empty()) {
        return None;
    }
    let bits = destination.size() * 8;
    let count = usize::from(count & 31);
    if count == 0 || count >= bits {
        return None;
    }
    // Result bit `b` comes from the destination shifted by `count`, and what
    // the shift empties from the source's other end, or zero.
    let origin = |bit: usize| -> Option<(RegId, usize)> {
        if left {
            if bit >= count { Some((destination, bit - count)) } else { source.map(|one| (one, bits - count + bit)) }
        } else if bit + count < bits {
            Some((destination, bit + count))
        } else {
            source.map(|one| (one, bit + count - bits))
        }
    };
    let byte = |register: RegId, bit: usize| (full32(regs, register), u32::try_from(bit / 8).expect("a byte"));
    let mut moved = Vec::new();
    for bit in 0..bits {
        if let Some((register, from)) = origin(bit) {
            let pair = (byte(destination, bit), byte(register, from));
            if !moved.contains(&pair) {
                moved.push(pair);
            }
        }
    }
    Some((moved, _lanes(regs, destination).or(&source.map(|one| _lanes(regs, one)).unwrap_or_default())))
}

/// `_register_effects` of an instruction that says only what it does: no
/// required or delivered register, no clobber and no symbol. Asked of a
/// proposed rewrite of one, which needs no instruction made of it to ask (copy
/// propagation made and dropped two per register it tried to forward).
pub fn _register_effects_of_what(
    regs: Regs,
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
    // Hashed once: a miss remembers under the hashes it looked up.
    let hashes = key.hashes();
    if let Some(found) = EFFECTS.with(|held| held.borrow().find(hashes)) {
        return found;
    }
    EFFECTS_COMPUTED.with(|count| count.set(count.get() + 1));
    let answer = _register_effects_of(regs, bits, &Insn::new(0, None, None, vec![], vec![]), what, may_write, flags);
    EFFECTS.with(|held| held.borrow_mut().remember(hashes, answer.clone()));
    answer
}

pub fn _register_effects(
    regs: Regs,
    bits: u32,
    one: &Insn,
    may_write: bool,
    flags: bool,
) -> Option<(Lanes, Lanes)> {
    // Kept on the instruction: asked again by each pass of the peephole.
    let (was, answer) = one.register_effects[usize::from(may_write) << 1 | usize::from(flags)]
        .get_or_init(|| (bits, _register_effects_worked(regs, bits, one, may_write, flags)));
    if *was == bits { *answer } else { _register_effects_worked(regs, bits, one, may_write, flags) }
}

fn _register_effects_worked(
    regs: Regs,
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
    let hashes = key.hashes();
    if let Some(found) = EFFECTS.with(|held| held.borrow().find(hashes)) {
        return found;
    }
    EFFECTS_COMPUTED.with(|count| count.set(count.get() + 1));
    let answer = _register_effects_of(regs, bits, one, what, may_write, flags);
    EFFECTS.with(|held| held.borrow_mut().remember(hashes, answer.clone()));
    answer
}

/// The inputs of `_register_effects` after the cases it answers at once: the
/// same ones, the same answer.
struct EffectKey<'a> {
    bits: u32,
    may_write: bool,
    flags: bool,
    what: &'a Semantics,
    requires: &'a [(Held, RegId)],
    delivers: &'a [(Held, RegId)],
}

impl EffectKey<'_> {
    /// Two hashes of the fields, the second seeded apart from the first.
    fn hashes(&self) -> (u64, u64) {
        use std::hash::{Hash, Hasher};
        #[cfg(test)]
        EFFECTS_HASHED.with(|count| count.set(count.get() + 1));
        let fields = (self.bits, self.may_write, self.flags, self.requires, self.delivers);
        let mut first = rustc_hash::FxHasher::default();
        fields.hash(&mut first);
        shape(self.what, &mut first);
        let mut second = rustc_hash::FxHasher::default();
        0x9e37_79b9_7f4a_7c15_u64.hash(&mut second);
        fields.hash(&mut second);
        shape(self.what, &mut second);
        (first.finish(), second.finish())
    }
}

/// What `_register_effects_of` reads of `what`: the operation, the registers
/// and widths of its operands and the registers an address is reached through.
/// Not the value numbers of unplaced values, an immediate (but whether a shift
/// count is a multiple of 32), or the displacement and symbol of a cell.
fn shape<H: std::hash::Hasher>(
    what: &Semantics,
    into: &mut H,
) {
    use std::hash::Hash;
    what.op.hash(into);
    what.name.hash(into);
    what.target.is_some().hash(into);
    what.indirect.hash(into);
    what.repeat.hash(into);
    for list in [&what.dests, &what.sources] {
        list.len().hash(into);
        for place in list {
            match place {
                Loc::Reg(one) => (0u8, one).hash(into),
                Loc::Held(one) => (1u8, one.width).hash(into),
                Loc::Imm(one) => (2u8, one.width, one.value & 31 == 0).hash(into),
                Loc::Mem(one) => (
                    3u8,
                    one.addr.map(|addr| (addr.space, addr.base, addr.segment, addr.index & 1)),
                    one.width,
                    one.through,
                    one.base.map(|held| held.width),
                    one.selector.map(|held| held.width),
                    one.index.map(|held| held.width),
                    one.scale,
                    one.index_through,
                    one.disp_width,
                    one.offset == 0,
                )
                    .hash(into),
                other => (4u8, other).hash(into),
            }
        }
    }
}

type Effects = Option<(Lanes, Lanes)>;

/// What `_register_effects` answered, by two independent 64-bit hashes of what
/// it was asked. The asked was once kept beside the answer to tell a hash that
/// is another's: a clone of its `Semantics` (a name and two operand lists) for
/// each of the 5,559 answers a d_faces -O2 compile remembers, 5.8 M Ir. Two
/// hashes are one in 2^128 a pair of different asks.
#[derive(Default)]
struct EffectsHeld {
    by_hash: HashMap<(u64, u64), Effects>,
    count: usize,
}

impl EffectsHeld {
    fn find(
        &self,
        hashes: (u64, u64),
    ) -> Option<Effects> {
        self.by_hash.get(&hashes).cloned()
    }

    fn remember(
        &mut self,
        hashes: (u64, u64),
        answer: Effects,
    ) {
        // A function's instructions, not the run's.
        if self.count > 200_000 {
            *self = Self::default();
        }
        self.count += 1;
        self.by_hash.insert(hashes, answer);
    }
}

thread_local! {
    static EFFECTS: RefCell<EffectsHeld> = RefCell::new(EffectsHeld::default());
    static EFFECTS_COMPUTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    #[cfg(test)]
    static EFFECTS_HASHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has hashed an ask, for a test that a miss hashes
/// it once.
#[cfg(test)]
pub fn effects_hashed() -> usize {
    EFFECTS_HASHED.with(std::cell::Cell::get)
}

/// How many times this thread has worked out an instruction's register effects,
/// for a test that asking again of one does not.
pub fn effects_computed() -> usize {
    EFFECTS_COMPUTED.with(std::cell::Cell::get)
}

/// What `x86.instr` says of `what`; None where it has no row, which is not
/// knowing.
fn _register_effects_of(
    regs: Regs,
    bits: u32,
    one: &Insn,
    what: &Semantics,
    may_write: bool,
    flags: bool,
) -> Effects {
    _effects_by_table(regs, bits, one, what, may_write, flags).flatten()
}

/// `_register_effects_of` as `x86.instr` states it: None where it has no row.
pub fn _effects_by_table(
    regs: Regs,
    bits: u32,
    one: &Insn,
    what: &Semantics,
    may_write: bool,
    flags: bool,
) -> Option<Effects> {
    let found = match crate::backend::effects::served(regs, bits, what)? {
        crate::backend::effects::Served::Unknown => return Some(None),
        crate::backend::effects::Served::Known(found) => found,
    };
    let mut reads: Lanes = one
        .requires
        .iter()
        .flat_map(|(held, register)| _lanes(regs, regs.named(*register, i64::from(held.width))))
        .collect();
    let mut writes: Lanes = one
        .delivers
        .iter()
        .flat_map(|(held, register)| _lanes(regs, regs.named(*register, i64::from(held.width))))
        .collect();
    if flags {
        reads.extend(_flag_lanes(found.flags_read).minus(&writes));
        writes.extend(_flag_lanes(found.flags_written));
    }
    for register in &found.reads {
        reads.extend(_lanes(regs, *register).minus(&writes));
    }
    for register in &found.writes {
        writes.extend(_lanes(regs, *register));
        // In long mode a dword write clears the upper half of the register.
        if bits == 64
            && regs.bytes(*register) == Some(4)
            && let Some(whole) = regs.view(regs.root(*register), 64)
        {
            writes.extend(_lanes(regs, whole));
        }
    }
    if may_write {
        for register in &found.maybe_writes {
            writes.extend(_lanes(regs, *register));
        }
    }
    Some(Some((reads, writes)))
}

pub fn _flag_lanes(mask: u32) -> Lanes {
    Lanes::flags(mask)
}

/// A bp-relative slot whose bytes the displacement alone names.
pub fn _frame_cell(cell: &Mem) -> bool {
    cell.addr.is_some_and(|addr| addr.space == Space::Frame) && cell.through == RegId::BP && cell.base.is_none()
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
    let regs = body.regs();
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut dead = exits[&block.at].clone();
        let mut redundant: HashSet<usize> = HashSet::default();
        for one in block.insns.iter().rev() {
            let Some(effect) = liveness::effect(regs, body.bits, one) else {
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
                        let lanes = _lanes(regs, dest.register);
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
                            && !_lanes(regs, dest.register).is_empty()
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
                            && !_lanes(regs, dest.register).is_empty()
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

/// Fold a load that only an indirect call reads into the call
/// (`peephole.peep`).
pub fn fused_calls(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.fused_calls, body, &Facts::new(body, None))
}

/// Load a far pointer's two words with one les (lds, lfs, lgs)
/// (`peephole.peep`).
pub fn far_loads(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.far_loads, body, &Facts::new(body, None))
}

/// Load a segment register straight from the cell a temporary was loaded from
/// (`peephole.peep`, `loaded_segments`).
pub fn loaded_segments(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.loaded_segments, body, &Facts::new(body, None))
}

/// Replace a dead temporary's shift with a scaled 67h LEA.
///
/// The load remains a load.  Only the register-only `shl; add` tail is
/// selected differently, after virtual use counts prove that no later use
/// expects the temporary to contain its shifted value.
fn _loaded_scaled_add<'a>(
    regs: Regs,
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
        || !regs.integer(temporary.register)
        || !regs.integer(total.register)
        || ir::root(temporary.register) == ir::root(total.register)
        || ir::root(temporary.register) == RegId::ESP
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
        vec![Loc::Address(AddressRef {
            through: full32(regs, total.register),
            index_through: full32(regs, temporary.register),
            scale: 1 << amount,
            ..AddressRef::new(None)
        })],
    );
    Ok(Some(Arc::new(with_what(addition, what))))
}

/// Select physically adjacent load/scale/add tails across inert anchors.
fn _loaded_addresses(
    regs: Regs,
    block: &LirBlock,
    dead: &dyn Fn(usize) -> bool,
    uses: &Counter,
    cpu: &Profile,
) -> Result<LirBlock, String> {
    let mut insns = block.insns.to_vec();
    let work: Vec<usize> =
        insns.iter().enumerate().filter(|(_, one)| !_skippable_nothing(one)).map(|(index, _)| index).collect();
    let mut removed: HashSet<usize> = HashSet::default();
    let mut at = 0;
    while at + 2 < work.len() {
        let indexes = &work[at..at + 3];
        let parts: Vec<Arc<Insn>> = indexes.iter().map(|index| Arc::clone(&insns[*index])).collect();
        let combined = match _loaded_scaled_add(regs, &parts, uses, cpu)? {
            Some(combined) if dead(id(&parts[2])) => Some(combined),
            _ => None,
        };
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
    Ok(block.with_insns(insns).into_inner())
}

/// The instructions of `block` after which no arithmetic flag is read,
/// given whether any is read after its end.
pub(crate) fn _flags_dead_after(
    regs: Regs,
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
        flags_dead = _flags_before(regs, bits, one, flags_dead);
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
    live_out: &llrm_mir::dense::IdSet<u32>,
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
    live_out: &llrm_mir::dense::IdSet<u32>,
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
    let regs = body.regs();
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
    // Each fact is worked out for the first window that gets as far as asking
    // for it: most bodies have none that does.
    let live_out = std::cell::OnceCell::new();
    let flags_out = std::cell::OnceCell::new();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let flags_dead_out =
            || flags_out.get_or_init(|| _flags_live_out(body)).entering.get(&block.at).is_some_and(Lanes::is_empty);
        let loaded_dead = std::cell::OnceCell::new();
        let block = _loaded_addresses(
            regs,
            block,
            &|at| loaded_dead.get_or_init(|| _flags_dead_after(regs, body.bits, block, flags_dead_out())).contains(&at),
            &virtual_uses,
            target_cpu,
        )?;
        let dead = std::cell::OnceCell::new();
        let insns = _code_windows(&block.insns, block.insns.len(), |rest| -> Result<_, String> {
            let dead_after = |at: usize| {
                dead.get_or_init(|| _flags_dead_after(regs, body.bits, &block, flags_dead_out())).contains(&at)
            };
            Ok(match _affine_address(regs, body.bits, rest, &dead_after, target_cpu)? {
                Some((taken, made)) => {
                    let live_out = live_out.get_or_init(|| crate::backend::liverows::live_out(body));
                    let live_out = live_out.get(&block.at).cloned().unwrap_or_default();
                    (!_loses_live_definition(&rest[..taken], &made[0], &rest[taken..], &live_out))
                        .then_some((taken, made))
                }
                None => None,
            })
        })?;
        blocks.push(block.with_insns(insns));
    }
    Ok(body.with_blocks(blocks))
}

/// `ir.ROOT.get(register)`: `ROOT` itself is private to `ir`, and its keys
/// are exactly the registers `root` renames plus the eight 32-bit roots.
fn _root_get(
    regs: Regs,
    register: RegId,
) -> Option<RegId> {
    let rooted = ir::root(register);
    let key = rooted != register || regs.in_class(register, llrm_lir::registers::class::INT);
    key.then_some(rooted)
}

/// `mov z,y` and the arithmetic after it that only rewrites z, as one 67h LEA.
///
/// Each step keeps z an affine sum of registers (`affine::step`). The
/// longest prefix whose flags are dead (`dead`), whose sum an address names,
/// and which the target prices no dearer in cycles or bytes becomes one LEA.
/// A word result keeps only the low sixteen bits, which the upper halves of
/// the registers read cannot reach, and a step the target ran on the whole
/// register (`promote`) goes on the chain. Returns the instructions consumed.
fn _affine_address(
    regs: Regs,
    mode: u32,
    parts: &[Arc<Insn>],
    dead: &dyn Fn(usize) -> bool,
    cpu: &Profile,
) -> Result<Option<(usize, Vec<Arc<Insn>>)>, String> {
    let Some(copy) = parts.first() else {
        return Ok(None);
    };
    let Some(wide) = cpu.dword_address_form() else {
        return Ok(None);
    };
    let (dest, mut terms, mut disp, mut old) = match affine::step(regs, copy, cpu) {
        Some((dest, affine::Step::Copy(source), old)) => {
            if full32(regs, source.register) == full32(regs, dest.register) {
                return Ok(None);
            }
            (dest, vec![(full32(regs, source.register), 1)], 0, old)
        }
        _ => match affine::lea_start(regs, copy, cpu) {
            Some(start) => start,
            None => return Ok(None),
        },
    };
    let z = full32(regs, dest.register);
    let bits = i64::from(dest.width) * 8;
    let wrapped = |value: i64| wrap_to(value, bits);
    let mut best: Option<(usize, AddressRef, i64, bool, Reg)> = None;
    for (at, one) in parts.iter().enumerate().skip(1) {
        let Some((written, step, cost)) = affine::step(regs, one, cpu) else {
            break;
        };
        // A step the target ran on the whole register (`promote`) goes on a
        // word chain, and a word step on one so run: what lies above a word
        // is nobody's.
        if written != dest && !(full32(regs, written.register) == z && written.width != dest.width) {
            break;
        }
        match step {
            affine::Step::Add(value) => disp = wrapped(disp + value),
            affine::Step::Scale(factor) => {
                terms.iter_mut().for_each(|term| term.1 *= factor);
                disp = wrapped(disp * factor);
            }
            affine::Step::AddRegister(other) => {
                let root = full32(regs, other.register);
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
        if !dead(id(one)) {
            continue;
        }
        if written.width == 2
            && let Some(address) = affine::word_form(&terms, disp)
        {
            best = Some((at, address, old, true, written));
        } else if let Some(address) = affine::form(regs, &terms, disp, &wide.scales) {
            best = Some((at, address, old, false, written));
        }
    }
    let Some((last, address, old, word, dest)) = best else {
        return Ok(None);
    };
    let replaced = &parts[..=last];
    let partial: HashSet<RegId> = terms.iter().map(|term| term.0).collect();
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
            (Some(name @ ("add" | "sub")), [Loc::Reg(register)], [_, Loc::Imm(Imm { value, .. })]) if dead(id(one)) => {
                // By the register's width: 1 and -1 (0xFFFF in a word) are the
                // unit.
                let bits = i64::from(register.width) * 8;
                let step = match wrap_to(*value, bits) {
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

/// A step made before the compare of the value it replaces, after it
/// (`peephole.peep`).
pub fn stepped_after_compare(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    peep::rewritten(rules.stepped_after_compare, body, &Facts::new(body, None))
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
    regs: Regs,
    bits: u32,
    one: &Insn,
    flags_dead: bool,
) -> bool {
    let (reads, writes) = _flag_effect(regs, bits, one);
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
            llrm_x86::instructions::dialect().conditions.iter().any(|(one, _)| one == condition).then_some(())?;
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
pub(crate) static _ARITHMETIC_LANES: LazyLock<Lanes> = LazyLock::new(|| _flag_lanes(_ARITHMETIC));

/// `inc edi; cmp edi,0; jne` is `inc edi; jne`.
///
/// An add, subtract, logic or unary operation sets ZF and SF from its result
/// exactly as a zero test of that result does. The test goes where only its
/// branch reads those two, and no later instruction reads the carry,
/// overflow or adjust flags it would have cleared. Work independent of the
/// computation may stand between them, in the block or in a sole predecessor
/// that only falls into it: the computation sinks to just before the test.
/// A computation with a memory source stays where it is, and answers a test it
/// is beside.
pub fn tested(
    rules: &peep::Rules,
    body: &LirBody,
) -> LirBody {
    let regs = body.regs();
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
        }) || !live.entering[&block.at].is_disjoint(&_DIFFERING)
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
        let Some(source) = _flag_source(regs, body.bits, &blocks, &line, &register) else {
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

/// `cmp a,b; jl X` falling into `cmp a,b; jle Y` is `cmp a,b; jl X`, `jle Y`.
///
/// A block entered from one block alone, whose first work is the compare that
/// block ended in (a compare, then its conditional jump, then perhaps a jump
/// to the other side), reads flags that are already its compare's: the jump
/// changes none. gcc's `cse_condition_code_reg` removes it in its extended
/// basic blocks, and an else-if on the same two values (a binary search's
/// `a[m] < k`, then `a[m] > k`) tests them once.
pub fn compares_again(body: &LirBody) -> LirBody {
    let mut predecessors = HashMap::<i64, Vec<usize>>::default();
    for (at, block) in body.blocks.iter().enumerate() {
        for to in &block.succ {
            predecessors.entry(*to).or_default().push(at);
        }
    }
    let work = |insns: &[Arc<Insn>]| -> Vec<usize> {
        insns.iter().enumerate().filter(|(_, one)| !_skippable_nothing(one)).map(|(index, _)| index).collect()
    };
    let compare_of = |one: &Insn| -> bool {
        one.clobbers.is_empty()
            && one.symbol != Some(true)
            && !one.volatile
            && one.what.as_ref().is_some_and(|what| what.op == Operation::Compare)
    };
    let mut blocks = body.blocks.iter().map(|block| block.insns.to_vec()).collect::<Vec<_>>();
    for (index, block) in body.blocks.iter().enumerate() {
        let Some([from]) = predecessors.get(&block.at).map(Vec::as_slice) else { continue };
        if *from == index {
            continue;
        }
        // The first work of the block, and the last of the one before: a
        // compare, a branch on it, and perhaps the jump to the other side.
        let ahead = work(&blocks[*from]);
        let mut last = ahead.len();
        let is =
            |insns: &[Arc<Insn>], at: usize, op: Operation| insns[at].what.as_ref().is_some_and(|what| what.op == op);
        if last > 0 && is(&blocks[*from], ahead[last - 1], Operation::Jump) {
            last -= 1;
        }
        if last < 2 || !is(&blocks[*from], ahead[last - 1], Operation::Branch) {
            continue;
        }
        let before = Arc::clone(&blocks[*from][ahead[last - 2]]);
        let here = work(&blocks[index]);
        let Some(&first) = here.first() else { continue };
        let again = Arc::clone(&blocks[index][first]);
        if !compare_of(&before) || !compare_of(&again) || before.widths != again.widths {
            continue;
        }
        let (Some(was), Some(is_now)) = (before.what.as_ref(), again.what.as_ref()) else { continue };
        if !was.same_meaning(is_now) {
            continue;
        }
        blocks[index][first] = Arc::new(Insn {
            what: Some(semantics(Operation::Nothing, "", vec![], vec![])),
            defines: Vec::new(),
            uses: Vec::new(),
            widths: Vec::new(),
            ..(*again).clone()
        });
    }
    body.with_blocks(body.blocks.iter().zip(blocks).map(|(block, insns)| block.with_insns(insns)).collect())
}

/// The position in `line` of what set `register` last, when nothing after it
/// reads flags, touches its operands or its result, or accesses a register it
/// reads: it may then run last, and its flags are `register`'s zero test.
fn _flag_source(
    regs: Regs,
    bits: u32,
    blocks: &[Vec<Arc<Insn>>],
    line: &[(usize, usize)],
    register: &Reg,
) -> Option<usize> {
    let registers = |lanes: &Lanes| {
        let mut lanes = *lanes;
        lanes.retain(|lane| lane.0 != RegId::None);
        lanes
    };
    let mut crossed = Vec::new();
    for (position, (block, index)) in line.iter().enumerate().rev() {
        let one = &blocks[*block][*index];
        if _skippable_nothing(one) {
            continue;
        }
        let effect = liveness::effect(regs, bits, one)?;
        if _sets_from(one, register) {
            let operands_in_registers = one
                .what
                .as_ref()
                .is_some_and(|what| what.sources.iter().all(|source| matches!(source, Loc::Reg(_) | Loc::Imm(_))));
            let (reads, writes) = (registers(&effect.reads), registers(&effect.writes));
            // A memory source is read where it is: only a setter that moves
            // down past other work must have its operands in
            // registers.
            return ((operands_in_registers || crossed.is_empty())
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
        if transfers || !one.clobbers.is_empty() || effect.reads.iter().any(|lane| lane.0 == RegId::None) {
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
    regs: Regs,
    bits: u32,
    one: &Insn,
) -> (Lanes, Lanes) {
    let every = _flag_lanes(_ARITHMETIC);
    if _nothing(one) {
        // One that clobbers is an opaque barrier, which may observe any flag.
        return if one.clobbers.is_empty() { (Lanes::new(), Lanes::new()) } else { (every, Lanes::new()) };
    }
    if let Some(effect) = liveness::effect(regs, bits, one) {
        let flags = |lanes: &Lanes| lanes.iter().copied().filter(|lane| lane.0 == RegId::None).collect::<Lanes>();
        return (flags(&effect.reads), flags(&effect.writes));
    }
    // A sibling call's `jmp` leaves the function as `ret` does: it reads no
    // flag and the caller's flags are gone (the pin's `TAILJMPd`).
    if one.what.as_ref().is_some_and(|what| what.op == Operation::Return) {
        return (Lanes::new(), every);
    }
    if one.what.as_ref().is_some_and(|what| what.op == Operation::Call) {
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
pub fn _flags_live_out(body: &LirBody) -> std::rc::Rc<crate::backend::incremental::Solution<FlagLive>> {
    let solved = FLAGS.with(|kept| kept.ask(&FlagLive { regs: body.regs(), bits: body.bits }, body));
    if llrm_support::env_flag!("LLRM_CHECK_LIVENESS") {
        assert!(
            flags_from_scratch(body) == solved.entering,
            "{}: flag liveness brought up to date differs from one solved from scratch",
            body.name
        );
    }
    solved
}

/// Flag liveness as a [`Problem`](crate::backend::incremental::Problem): the
/// flags live after a block, flowing back from the blocks that follow it.
pub struct FlagLive {
    regs: Regs,
    bits: u32,
}

impl crate::backend::incremental::Problem for FlagLive {
    /// `(read, written)`: the flags a block reads before it writes them, and
    /// those it writes.
    type Summary = (Lanes, Lanes);
    type Value = Lanes;
    const FORWARD: bool = false;

    fn summarize(
        &self,
        _body: &LirBody,
        block: &Block,
    ) -> (Lanes, Lanes) {
        block
            .insns
            .iter()
            .rev()
            .fold(
                (Lanes::new(), Lanes::new()),
                |(read, written), one| {
                    let (reads, writes) = _flag_effect(self.regs, self.bits, one);
                    (read.minus(&writes).or(&reads), written.or(&writes))
                },
            )
    }

    fn start(&self) -> Lanes {
        Lanes::new()
    }

    /// What a block with no way on leaves live, else what the blocks it goes to
    /// read (a block it names that is not there leaves what a block with no way
    /// on does).
    fn entering(
        &self,
        _at: i64,
        _entry: i64,
        present: &[Lanes],
        missing: usize,
    ) -> Lanes {
        if present.is_empty() && missing == 0 {
            return _exit_flags();
        }
        let live = present.iter().fold(Lanes::new(), |all, next| all.or(next));
        if missing > 0 { live.or(&_exit_flags()) } else { live }
    }

    fn leaving(
        &self,
        (read, written): &(Lanes, Lanes),
        after: Lanes,
    ) -> Lanes {
        after.minus(written).or(read)
    }
}

thread_local! {
    static FLAGS: crate::backend::incremental::Kept<FlagLive> = const { crate::backend::incremental::Kept::new() };
}

/// How many flag-liveness answers this thread has worked out, whole or in
/// part, for a test that a pass with nothing to fold does not ask.
pub fn flags_computed() -> usize {
    let [whole, partly, _] = FLAGS.with(crate::backend::incremental::Kept::asks);
    whole + partly
}

/// The flags live after each block of `body`, solved from nothing held: what
/// `LLRM_CHECK_LIVENESS` compares an answer brought up to date with.
fn flags_from_scratch(body: &LirBody) -> IndexMap<i64, Lanes> {
    let regs = body.regs();
    let exits = _exit_flags();
    let effects = |one: &Insn| _flag_effect(regs, body.bits, one);
    let steps: HashMap<i64, Vec<(Lanes, Lanes)>> =
        body.blocks.iter().map(|block| (block.at, block.insns.iter().map(|one| effects(one)).collect())).collect();
    let nodes: Vec<&Block> = body.blocks.iter().collect();
    let blocks: HashMap<i64, &Block> = nodes.iter().map(|block| (block.at, *block)).collect();
    let solved = dataflow::solve(
        &nodes,
        Direction::Backward,
        |_| Lanes::new(),
        |at, live_in| {
            let block = blocks[&at];
            if block.succ.is_empty() {
                exits
            } else {
                block.succ.iter().flat_map(|at| live_in.get(at).unwrap_or(&exits).iter().copied()).collect()
            }
        },
        |at, after| steps[&at].iter().rev().fold(*after, |live, (reads, writes)| live.minus(writes).or(reads)),
    );
    solved.input.into_iter().collect()
}

/// Use XOR for zero only when later integer work replaces every arithmetic
/// flag.
pub fn zeroes(body: &LirBody) -> LirBody {
    let regs = body.regs();
    let live = _flags_live_out(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        // What the block's successors read, not a guess: `mov bx,0; jmp` to a
        // block that sets its own flags first zeroes with xor too.
        let mut flags_dead = live.entering[&block.at].is_disjoint(&_ARITHMETIC_LANES);
        let mut insns = Vec::new();
        for one in block.insns.iter().rev() {
            let mut one = Arc::clone(one);
            let previous = _flags_before(regs, body.bits, &one, flags_dead);
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
                            && [2, 4, 8].contains(width)
                            && regs.integer(dest.register)
                            && one.symbol != Some(true)
                        {
                            // A quadword zero is the dword `xor`: it clears the
                            // upper half too.
                            let dest =
                                if *width == 8 { Reg { register: regs.root(dest.register), width: 4 } } else { *dest };
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
    let regs = body.regs();
    let mut objects = 0usize;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut held: IndexMap<Reg, Known> = IndexMap::default();
        let mut redundant: HashSet<usize> = HashSet::default();
        let mut folded: HashMap<usize, Arc<Insn>> = HashMap::default();
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
                    && matches!(
                        what.name.as_deref(),
                        Some("cwd" | "cdq" | "cqo" | "movsx" | "movzx")
                    )
                    && what.dests.len() == 1
                    && what.sources.len() == 1
                    && what.dests.iter().chain(&what.sources).all(|arg| matches!(arg, Loc::Reg(_)))
            });
            if !moved && !extend {
                match _register_effects(regs, body.bits, one, true, false) {
                    None => held.clear(),
                    Some((_reads, writes)) => held.retain(|dest, _| _lanes(regs, dest.register).is_disjoint(&writes)),
                }
                continue;
            }
            let what = what.expect("a move or extension has semantics");
            let mut candidate = None;
            // The extension of a register holding a constant is the extended
            // constant: a fact asked of `held`, not a pattern of
            // the bytes before it (gcc's combine does it through known values).
            if let Some(value) = extended_constant(&held, what, body.regs()) {
                if let (Some(Loc::Reg(dest)), true) = (what.dests.first(), one.clobbers.is_empty()) {
                    let made = semantics(
                        Operation::Move,
                        "mov",
                        vec![Loc::Reg(*dest)],
                        vec![Loc::Imm(imm(value, dest.width))],
                    );
                    if emit(body.bits, &made).is_some() {
                        folded.insert(id(one), Arc::new(with_what(one, made)));
                        candidate = Some((*dest, Known::Value(value & low_mask(dest.width))));
                    }
                }
            }
            if moved && what.dests.len() == 1 && what.sources.len() == 1 {
                let (dest, source) = (&what.dests[0], &what.sources[0]);
                if let Loc::Reg(dest) = dest {
                    let source_width = match source {
                        Loc::Reg(source) => Some(source.width),
                        Loc::Imm(source) => Some(source.width),
                        _ => None,
                    };
                    if regs.integer(dest.register) && source_width == Some(dest.width) {
                        match source {
                            Loc::Imm(source) if source.address.is_none() => {
                                candidate = Some((*dest, Known::Value(source.value & low_mask(dest.width))));
                            }
                            Loc::Reg(source) if regs.integer(source.register) => {
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
            let mut written: HashSet<RegId> =
                one.clobbers.iter().chain(&one.clobbers_high).map(|register| full32(regs, *register)).collect();
            written.extend(what.dests.iter().filter_map(|dest| match dest {
                Loc::Reg(dest) => Some(full32(regs, dest.register)),
                _ => None,
            }));
            held.retain(|dest, _| !written.contains(&full32(regs, dest.register)));
            if let Some((dest, value)) = candidate {
                if one.clobbers.is_empty() {
                    held.insert(dest, value);
                }
            }
        }
        let insns = block
            .insns
            .iter()
            .map(|one| {
                if redundant.contains(&id(one)) {
                    lir::anchor(Arc::clone(one))
                } else {
                    folded.get(&id(one)).map_or_else(|| Arc::clone(one), Arc::clone)
                }
            })
            .collect();
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

/// The value `movzx`/`movsx` of a register `held` holds as a constant extends
/// it to, if it does: the source register read at its own width (or the low
/// part of a wider one held whole), extended as the instruction does.
fn extended_constant(
    held: &IndexMap<Reg, Known>,
    what: &Semantics,
    regs: Regs,
) -> Option<i64> {
    if what.op != Operation::Extend || !matches!(what.name.as_deref(), Some("movzx" | "movsx")) {
        return None;
    }
    let (Some(Loc::Reg(dest)), [Loc::Reg(source)]) = (what.dests.first(), &what.sources[..]) else { return None };
    if what.dests.len() != 1
        || !regs.integer(dest.register)
        || !regs.integer(source.register)
        || source.width >= dest.width
    {
        return None;
    }
    // A byte of the high half (AH) is not the low lane of its register.
    if regs.named(full32(regs, source.register), i64::from(source.width)) != source.register {
        return None;
    }
    let value = held
        .iter()
        .find(|(one, _)| full32(regs, one.register) == full32(regs, source.register) && one.width >= source.width)
        .and_then(|(_, known)| if let Known::Value(value) = known { Some(*value) } else { None })?;
    let mask = low_mask(source.width);
    let low = value & mask;
    Some(if what.name.as_deref() == Some("movsx") && low >> (source.width * 8 - 1) & 1 == 1 {
        low | !mask
    } else {
        low
    })
}

#[cfg(test)]
#[path = "peephole_tests.rs"]
mod tests;
