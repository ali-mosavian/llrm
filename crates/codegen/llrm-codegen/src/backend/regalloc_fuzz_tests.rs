//! A fuzz lane for the register allocator: generated bodies at high pressure,
//! every constraint kind, and the result must be an allocation (the allocator
//! is total), no longer name a value, and store what the generated body stores.
//!
//! `FUZZ_SEEDS=N` runs N seeds (default 40); `FUZZ_SEED=S` replays one and
//! prints its body; `FUZZ_CPU` names the profile (default: each of 386, 486,
//! Core, P5).

use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::allocate::RegAlloc;
use crate::backend::cpu::ProfileOrName;
use crate::backend::parcopy::ParallelCopy;
use crate::backend::phielim::PhiElimination;
use crate::backend::twoaddr::TwoAddress;
use crate::backend::{target, verify};
use crate::coalescer::pass::Coalescer;
use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;
use crate::support::pyrepr::Repr;

/// SplitMix64: the same sequence on every host.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(
        &mut self,
        count: usize,
    ) -> usize {
        (self.next() % count as u64) as usize
    }

    fn pick(
        &mut self,
        from: &[u32],
    ) -> u32 {
        from[self.below(from.len())]
    }
}

const ENTRY: i64 = 0x10;
const LOOP: i64 = 0x1000;
const EXIT: i64 = 0x2000;
/// Where the exit stores the values: arguments' side of the frame, which
/// no spill slot is.
const EXIT_CELL: i64 = 100;

fn held(value: u32) -> Loc {
    Loc::Held(Held { value, width: 2 })
}

fn imm(value: i64) -> Loc {
    Loc::Imm(Imm { value, width: 2, address: None })
}

fn frame(disp: i64) -> Mem {
    Mem { through: RegId::BP, disp_width: 2, ..Mem::new(Some(Addr::new(Space::Frame, disp)), 2) }
}

/// What a generated body is made of.
struct Shape {
    pool: usize,
    ops: usize,
}

struct Builder {
    rng: Rng,
    at: i64,
    next: u32,
    notes: Vec<String>,
}

impl Builder {
    fn fresh(&mut self) -> u32 {
        self.next += 1;
        self.next
    }

    /// A copy of `value` for one role of an address: isel copies a value into
    /// a fresh one for each, so no value is a base and an index.
    fn role_copy(
        &mut self,
        value: u32,
        out: &mut Vec<Insn>,
    ) -> u32 {
        let copy = self.fresh();
        out.push(self.mov(copy, held(value), &[value]));
        copy
    }

    fn insn(
        &mut self,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: &[u32],
        uses: &[u32],
    ) -> Insn {
        self.at += 4;
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Insn::new(self.at, Some((self.at, self.at + 2)), Some(what), defines.to_vec(), uses.to_vec())
    }

    fn jump(
        &mut self,
        name: &str,
        op: Operation,
        to: i64,
    ) -> Insn {
        self.at += 4;
        let what = Semantics { name: Some(name.into()), target: Some(to), ..Semantics::new(op) };
        Insn::new(self.at, Some((self.at, self.at + 2)), Some(what), vec![], vec![])
    }

    fn mov(
        &mut self,
        into: u32,
        out_of: Loc,
        uses: &[u32],
    ) -> Insn {
        self.insn(Operation::Move, "mov", vec![held(into)], vec![out_of], &[into], uses)
    }

    /// One random operation over `pool`, in the order of a loop body.
    fn operation(
        &mut self,
        pool: &[u32],
        out: &mut Vec<Insn>,
    ) {
        let (d, s, t) = (self.rng.pick(pool), self.rng.pick(pool), self.rng.pick(pool));
        let kind = match std::env::var("FUZZ_KINDS") {
            Ok(only) => {
                let only: Vec<usize> = only.split(',').filter_map(|one| one.parse().ok()).collect();
                only[self.rng.below(13) % only.len()]
            }
            Err(_) => self.rng.below(13),
        };
        self.notes.push(format!("op {kind} on {d} {s} {t}"));
        match kind {
            0 => {
                let value = self.rng.below(100) as i64;
                out.push(self.mov(d, imm(value), &[]));
            }
            1 => out.push(self.mov(d, held(s), &[s])),
            2 | 3 => {
                out.push(self.insn(Operation::Binary, "add", vec![held(d)], vec![held(d), held(s)], &[d], &[d, s]))
            }
            // A base and an index, each a copy made for its role, as isel
            // makes them: a value used as a base and as an index in one body
            // would be joined into a class neither role has.
            4 => {
                let (base, index) = (self.role_copy(s, out), self.role_copy(t, out));
                let cell = Mem {
                    base: Some(Held { value: base, width: 2 }),
                    index: Some(Held { value: index, width: 2 }),
                    ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2)
                };
                out.push(self.insn(Operation::Move, "mov", vec![held(d)], vec![Loc::Mem(cell)], &[d], &[base, index]));
            }
            5 => {
                let base = self.role_copy(s, out);
                let cell = Mem {
                    base: Some(Held { value: base, width: 2 }),
                    ..Mem::new(Some(Addr::new(Space::Literal, 2)), 2)
                };
                out.push(self.insn(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![held(d)], &[], &[base, d]));
            }
            // dx:ax product, the high half redefined.
            6 if d != s && s != t && d != t => out.push(self.insn(
                Operation::Multiply,
                "imul",
                vec![held(d), held(s)],
                vec![held(d), held(t)],
                &[d, s],
                &[d, t],
            )),
            // dx:ax by a divisor; the remainder replaces the dividend.
            7 if d != t => {
                let (high, quotient) = (self.fresh(), self.fresh());
                out.push(self.insn(Operation::Extend, "cwd", vec![held(high)], vec![held(d)], &[high], &[d]));
                out.push(self.insn(
                    Operation::Divide,
                    "idiv",
                    vec![held(quotient), held(d)],
                    vec![held(high), held(d), held(t)],
                    &[quotient, d],
                    &[high, d, t],
                ));
            }
            // A count in cl.
            8 if d != s => {
                out.push(self.insn(Operation::Binary, "shl", vec![held(d)], vec![held(d), held(s)], &[d], &[d, s]))
            }
            // A call: arguments pushed, everything clobbered, the answer in ax.
            9 => {
                let result = self.fresh();
                let mut call = self.insn(Operation::Call, "call", vec![], vec![], &[result], &[s]);
                call.requires = vec![(Held { value: s, width: 2 }, RegId::AX)];
                call.delivers = vec![(Held { value: result, width: 2 }, RegId::AX)];
                call.clobbers = [RegId::EAX, RegId::ECX, RegId::EDX, RegId::EBX, RegId::ES].into_iter().collect();
                out.push(call);
                out.push(self.mov(d, held(result), &[result]));
            }
            // A far access: a selector and a base.
            10 if s != t => {
                let (base, selector) = (self.role_copy(s, out), self.role_copy(t, out));
                let cell = Mem {
                    base: Some(Held { value: base, width: 2 }),
                    selector: Some(Held { value: selector, width: 2 }),
                    ..Mem::new(Some(Addr::new(Space::Far, 0)), 2)
                };
                out.push(self.insn(
                    Operation::Move,
                    "mov",
                    vec![held(d)],
                    vec![Loc::Mem(cell)],
                    &[d],
                    &[base, selector],
                ));
            }
            // A far pointer loaded whole, then read through: the segment half
            // is a selector and nothing else.
            12 => {
                let (offset, segment) = (self.fresh(), self.fresh());
                let at = 40 + 4 * self.rng.below(4) as i64;
                let cell = Mem { width: 4, ..frame(at) };
                out.push(self.insn(
                    Operation::Move,
                    "les",
                    vec![held(offset), held(segment)],
                    vec![Loc::Mem(cell)],
                    &[offset, segment],
                    &[],
                ));
                let far = Mem {
                    base: Some(Held { value: offset, width: 2 }),
                    selector: Some(Held { value: segment, width: 2 }),
                    ..Mem::new(Some(Addr::new(Space::Far, 0)), 2)
                };
                out.push(self.insn(
                    Operation::Move,
                    "mov",
                    vec![held(d)],
                    vec![Loc::Mem(far)],
                    &[d],
                    &[offset, segment],
                ));
            }
            // A frame cell.
            _ => {
                let at = 40 + 2 * self.rng.below(8) as i64;
                out.push(self.insn(Operation::Move, "mov", vec![held(d)], vec![Loc::Mem(frame(at))], &[d], &[]));
            }
        }
    }
}

/// A body: values defined, a loop of random operations ending in a parallel
/// copy that permutes some of them, and every value read at the exit.
fn body(
    seed: u64,
    shape: &Shape,
) -> (LirBody, Vec<String>) {
    let mut build = Builder { rng: Rng(seed), at: 0, next: shape.pool as u32, notes: Vec::new() };
    let pool: Vec<u32> = (1..=shape.pool as u32).collect();

    let mut entry: Vec<Insn> = Vec::new();
    for (at, value) in pool.iter().enumerate() {
        entry.push(if at % 3 == 0 {
            build.insn(
                Operation::Move,
                "mov",
                vec![held(*value)],
                vec![Loc::Mem(frame(6 + 2 * at as i64))],
                &[*value],
                &[],
            )
        } else {
            build.mov(*value, imm(at as i64), &[])
        });
    }
    entry.push(build.jump("jmp", Operation::Jump, LOOP));

    let mut inside: Vec<Insn> = Vec::new();
    let skipped: Vec<usize> = std::env::var("FUZZ_SKIP")
        .map(|one| one.split(',').filter_map(|at| at.parse().ok()).collect())
        .unwrap_or_default();
    for at in 0..shape.ops {
        let kept = inside.len();
        build.operation(&pool, &mut inside);
        if skipped.contains(&at) {
            inside.truncate(kept);
        }
    }
    // The latch: a permutation of some values, as one parallel copy. Its
    // moves read every source before any is written, cycles included.
    let mut moved: Vec<u32> = pool.iter().copied().filter(|_| build.rng.below(3) != 0).collect();
    if moved.len() > 1 {
        let before = moved.clone();
        let count = moved.len();
        for at in (1..count).rev() {
            moved.swap(at, build.rng.below(at + 1));
        }
        for (into, out_of) in before.iter().zip(&moved) {
            let mut copy = build.mov(*into, held(*out_of), &[*out_of]);
            // A copy made by phi elimination claims no bytes of its own.
            copy.covers = Some((copy.at, copy.at));
            copy.group = Some(1);
            inside.push(copy);
        }
    }
    inside.push(build.insn(Operation::Compare, "cmp", vec![], vec![held(pool[0]), imm(100)], &[], &[pool[0]]));
    inside.push(build.jump("jne", Operation::Branch, LOOP));

    let mut exit: Vec<Insn> = Vec::new();
    for (at, value) in pool.iter().enumerate() {
        exit.push(build.insn(
            Operation::Move,
            "mov",
            vec![Loc::Mem(frame(EXIT_CELL + 2 * at as i64))],
            vec![held(*value)],
            &[],
            &[*value],
        ));
    }

    let block = |at: i64, insns: Vec<Insn>, succ: Vec<i64>| {
        let mut block = LirBlock::new(at, insns.into_iter().map(Arc::new).collect());
        block.succ = succ.into();
        block
    };
    let blocks =
        vec![block(ENTRY, entry, vec![LOOP]), block(LOOP, inside, vec![LOOP, EXIT]), block(EXIT, exit, vec![])];
    (
        LirBody::new(
            format!("fuzz{seed}"),
            ENTRY,
            crate::model::lir::shared(blocks),
            IndexMap::default(),
            IndexMap::default(),
        ),
        build.notes,
    )
}

/// `place` with every value it reads renamed by `map`.
fn read_as(
    place: &Loc,
    map: &dyn Fn(u32) -> u32,
) -> Loc {
    let held = |one: Option<Held>| one.map(|one| Held { value: map(one.value), ..one });
    match place {
        Loc::Held(one) => Loc::Held(Held { value: map(one.value), ..*one }),
        Loc::Mem(cell) => Loc::Mem(Mem {
            base: held(cell.base),
            index: held(cell.index),
            selector: held(cell.selector),
            ..cell.clone()
        }),
        other => other.clone(),
    }
}

/// The generated body in SSA: the loop's variables become phis at its header,
/// every definition a fresh name, and the exit reads the names the loop ends
/// with. Values an instruction defines that are not variables (a quotient, a
/// call's answer) are already defined once and keep their names.
fn in_ssa(body: &LirBody) -> LirBody {
    let all: Vec<u32> = body.insns().iter().flat_map(|one| one.defines.iter().chain(&one.uses).copied()).collect();
    let mut next = all.iter().copied().max().unwrap_or(0) + 1;
    let mut fresh = || {
        next += 1;
        next - 1
    };
    let variables: Vec<u32> = body.blocks[0].insns.iter().flat_map(|one| one.defines.iter().copied()).collect();
    let mut current: IndexMap<u32, u32> = variables.iter().map(|value| (*value, fresh())).collect();
    let headers: Vec<(u32, u32)> = variables.iter().map(|value| (*value, current[value])).collect();
    let mut inside: Vec<Arc<Insn>> = Vec::new();
    // A parallel copy reads what stood before it.
    let mut before: Option<IndexMap<u32, u32>> = None;
    for one in &body.blocks[1].insns {
        if one.group.is_none() {
            before = None;
        } else if before.is_none() {
            before = Some(current.clone());
        }
        let seen = before.clone().unwrap_or_else(|| current.clone());
        let read = |value: u32| seen.get(&value).copied().unwrap_or(value);
        let made: IndexMap<u32, u32> = one
            .defines
            .iter()
            .map(|value| (*value, if variables.contains(value) { fresh() } else { *value }))
            .collect();
        let written = |value: u32| made.get(&value).copied().unwrap_or(value);
        let mut changed = (**one).clone();
        if let Some(what) = &one.what {
            let place = |place: &Loc| match place {
                Loc::Held(one) => Loc::Held(Held { value: written(one.value), ..*one }),
                other => read_as(other, &read),
            };
            changed.what = Some(Semantics {
                dests: what.dests.iter().map(place).collect(),
                sources: what.sources.iter().map(|source| read_as(source, &read)).collect(),
                ..what.clone()
            });
        }
        // What a tied instruction reads of its destination is the old name.
        changed.defines = one.defines.iter().map(|value| written(*value)).collect();
        changed.uses = one.uses.iter().map(|value| read(*value)).collect();
        changed.requires = one
            .requires
            .iter()
            .map(|(held, register)| (Held { value: read(held.value), ..*held }, *register))
            .collect();
        changed.delivers = one
            .delivers
            .iter()
            .map(|(held, register)| (Held { value: written(held.value), ..*held }, *register))
            .collect();
        for (value, name) in &made {
            if variables.contains(value) {
                current.insert(*value, *name);
            }
        }
        inside.push(Arc::new(changed));
    }
    let latch = |value: u32| current.get(&value).copied().unwrap_or(value);
    let exit: Vec<Arc<Insn>> = body.blocks[2]
        .insns
        .iter()
        .map(|one| {
            let mut changed = (**one).clone();
            if let Some(what) = &one.what {
                changed.what = Some(Semantics {
                    dests: what.dests.iter().map(|place| read_as(place, &latch)).collect(),
                    sources: what.sources.iter().map(|place| read_as(place, &latch)).collect(),
                    ..what.clone()
                });
            }
            changed.uses = one.uses.iter().map(|value| latch(*value)).collect();
            Arc::new(changed)
        })
        .collect();
    let mut blocks = body.blocks.clone();
    blocks[1] = LirBlock {
        phis: headers
            .iter()
            .map(|(value, result)| Phi { result: *result, incoming: vec![(ENTRY, *value), (LOOP, latch(*value))] })
            .collect(),
        ..blocks[1].with_insns(inside).into_inner()
    }
    .into();
    blocks[2] = blocks[2].with_insns(exit);
    body.with_blocks(blocks)
}

/// What a finished body must be: every value placed, nothing a later phase
/// cannot schedule or the machine cannot name.
fn complaints(done: &LirBody) -> Vec<String> {
    let regs = done.regs();
    let mut out = verify::verify(done, false);
    for one in done.insns() {
        let Some(what) = &one.what else { continue };
        for place in what.dests.iter().chain(&what.sources) {
            let unplaced = match place {
                Loc::Held(_) => true,
                Loc::Mem(cell) => {
                    (cell.base.is_some() && cell.through == RegId::None)
                        || (cell.index.is_some() && cell.index_through == RegId::None)
                        || (cell.selector.is_some()
                            && cell
                                .addr
                                .is_some_and(|addr| addr.space == Space::Far && addr.segment == (RegId::None).iced()))
                }
                _ => false,
            };
            if unplaced {
                out.push(format!("{:#06x}: {} is not placed", one.at, place.repr()));
            }
            if let Loc::Reg(Reg { register, width }) = place {
                if regs.width_of(*register).is_some_and(|got| got != i64::from(*width)) {
                    out.push(format!("{:#06x}: register {register:?} at width {width}", one.at));
                }
            }
        }
    }
    out
}

/// What a body does, run: the generator's body in values, and the allocator's
/// in registers and frame cells, must store the same things.
mod run {
    use llrm_lir::registers::RegId;

    use crate::model::ir::{self, Loc, Mem, Operation, Space};
    use crate::model::lir::LirBody;
    use crate::support::hash::HashMap;

    pub struct Machine {
        virtual_: bool,
        vals: HashMap<u32, u32>,
        regs: HashMap<RegId, u32>,
        mem: HashMap<(i64, i64, i64, i64), u32>,
        stack: Vec<u32>,
        poison: u32,
        /// Every store that leaves the frame, in order.
        pub log: Vec<((i64, i64, i64, i64), u32)>,
        taken: usize,
    }

    fn hashed(key: (i64, i64, i64, i64)) -> u32 {
        let mut z =
            (key.0 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (key.1 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z ^= (key.2 as u64).wrapping_mul(0x94D0_49BB_1331_11EB) ^ (key.3 as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93);
        ((z ^ (z >> 29)) & 0xFFFF) as u32
    }

    impl Machine {
        pub fn new(virtual_: bool) -> Self {
            Self {
                virtual_,
                vals: HashMap::default(),
                regs: HashMap::default(),
                mem: HashMap::default(),
                stack: Vec::new(),
                poison: 0,
                log: Vec::new(),
                taken: 0,
            }
        }

        /// Nothing a body computes: the low word is hashed, so arithmetic that
        /// masks to a word cannot land on a small constant by luck.
        fn poisoned(count: u32) -> u32 {
            0x8000_0000 | hashed((i64::from(count), 7, 7, 7))
        }

        fn register(
            &mut self,
            register: RegId,
        ) -> u32 {
            let poison = &mut self.poison;
            *self
                .regs
                .entry(ir::root(register))
                .or_insert_with(
                    || {
                        *poison += 1;
                        Self::poisoned(*poison)
                    },
                )
        }

        fn key(
            &mut self,
            cell: &Mem,
        ) -> Result<(i64, i64, i64, i64), String> {
            let space = cell.addr.map_or(-1, |addr| addr.space as i64);
            let disp = cell.addr.map_or(cell.offset, |addr| addr.disp);
            if cell.addr.is_some_and(|addr| addr.space == Space::Frame) {
                return Ok((space, disp + cell.offset, 0, 0));
            }
            let placed = |held: Option<ir::Held>, through: RegId, this: &mut Self| -> Result<i64, String> {
                match held {
                    None => Ok(0),
                    Some(_) if through != RegId::None => Ok(i64::from(this.register(through))),
                    Some(held) => this
                        .vals
                        .get(&held.value)
                        .map(|got| i64::from(*got))
                        .ok_or(format!("value#{} is read before it is set", held.value)),
                }
            };
            let base = placed(cell.base, cell.through, self)?;
            let index = placed(cell.index, cell.index_through, self)? * cell.scale;
            let segment = match (cell.addr.map(|addr| addr.segment), cell.selector) {
                (Some(segment), _) if segment != (RegId::None).iced() => i64::from(self.register(RegId::from(segment))),
                (_, Some(selector)) => self
                    .vals
                    .get(&selector.value)
                    .map(|got| i64::from(*got))
                    .ok_or("a selector is read before it is set")?,
                _ => 0,
            };
            // Sixteen-bit addressing wraps.
            Ok((space, segment, (disp + base + index) & 0xFFFF, 0))
        }

        fn read(
            &mut self,
            place: &Loc,
        ) -> Result<u32, String> {
            Ok(match place {
                Loc::Imm(imm) => (imm.value & 0xFFFF) as u32,
                Loc::Held(held) => {
                    *self.vals.get(&held.value).ok_or(format!("value#{} is read before it is set", held.value))?
                }
                Loc::Reg(reg) => self.register(reg.register),
                Loc::Mem(cell) => {
                    let key = self.key(cell)?;
                    *self.mem.get(&key).unwrap_or(&hashed(key))
                }
                other => return Err(format!("cannot read {other:?}")),
            })
        }

        fn write(
            &mut self,
            place: &Loc,
            value: u32,
        ) -> Result<(), String> {
            match place {
                Loc::Held(held) => {
                    self.vals.insert(held.value, value);
                }
                Loc::Reg(reg) => {
                    self.regs.insert(ir::root(reg.register), value);
                }
                Loc::Mem(cell) => {
                    let key = self.key(cell)?;
                    if key.0 != Space::Frame as i64 {
                        self.log.push((key, value));
                    }
                    self.mem.insert(key, value);
                }
                other => return Err(format!("cannot write {other:?}")),
            }
            Ok(())
        }

        pub fn frame(
            &mut self,
            disp: i64,
        ) -> u32 {
            let key = (Space::Frame as i64, disp, 0, 0);
            *self.mem.get(&key).unwrap_or(&hashed(key))
        }

        /// The body from its entry, its loop run three times.
        pub fn execute(
            &mut self,
            body: &LirBody,
            again: i64,
        ) -> Result<(), String> {
            let regs = body.regs();
            let mut at = body.entry;
            for _ in 0..10_000 {
                let block = body.blocks.iter().find(|block| block.at == at).ok_or(format!("no block {at:#x}"))?;
                if at == again {
                    self.taken += 1;
                }
                let mut next: Option<i64> = None;
                let mut branches: Option<i64> = None;
                let mut position = 0;
                while position < block.insns.len() {
                    let one = &block.insns[position];
                    position += 1;
                    // A parallel copy reads every source before it writes.
                    if let Some(group) = one.group {
                        let mut moves = vec![one];
                        while block.insns.get(position).is_some_and(|more| more.group == Some(group)) {
                            moves.push(&block.insns[position]);
                            position += 1;
                        }
                        let mut read = Vec::new();
                        for each in &moves {
                            let what = each.what.as_ref().ok_or("a copy with no move")?;
                            read.push(self.read(&what.sources[0])?);
                        }
                        for (each, value) in moves.iter().zip(read) {
                            self.write(&each.what.as_ref().expect("checked").dests[0], value)?;
                        }
                        continue;
                    }
                    let Some(what) = &one.what else { continue };
                    match what.op {
                        Operation::Nothing | Operation::Compare => {}
                        Operation::Move => {
                            if crate::backend::target::far_load(regs, what) {
                                // A far pointer: its offset, then its segment,
                                // a word on.
                                let Loc::Mem(cell) = &what.sources[0] else {
                                    return Err("les from a register".to_owned());
                                };
                                let next = Mem {
                                    addr: cell.addr.map(|addr| ir::Addr { disp: addr.disp + 2, ..addr }),
                                    ..cell.clone()
                                };
                                let (offset, segment) = (
                                    self.read(&Loc::Mem(Mem { width: 2, ..cell.clone() }))?,
                                    self.read(&Loc::Mem(Mem { width: 2, ..next }))?,
                                );
                                self.write(&what.dests[0], offset)?;
                                self.write(&what.dests[1], segment)?;
                                continue;
                            }
                            let value = self.read(&what.sources[0])?;
                            self.write(&what.dests[0], value)?;
                        }
                        Operation::Binary => {
                            let (a, b) = (self.read(&what.sources[0])?, self.read(&what.sources[1])?);
                            let result = match what.name.as_deref() {
                                Some("add") => a.wrapping_add(b),
                                Some("shl") => a << (b & 15),
                                other => return Err(format!("binary {other:?}")),
                            };
                            self.write(&what.dests[0], result & 0xFFFF)?;
                        }
                        Operation::Multiply => {
                            let (a, b) = (self.read(&what.sources[0])? & 0xFFFF, self.read(&what.sources[1])? & 0xFFFF);
                            let product = a * b;
                            self.write(&what.dests[0], product & 0xFFFF)?;
                            self.write(&what.dests[1], product >> 16)?;
                        }
                        Operation::Extend => {
                            let value = self.read(&what.sources[0])?;
                            self.write(&what.dests[0], if value & 0x8000 != 0 { 0xFFFF } else { 0 })?;
                        }
                        Operation::Divide => {
                            let (hi, lo, divisor) = (
                                self.read(&what.sources[0])? & 0xFFFF,
                                self.read(&what.sources[1])? & 0xFFFF,
                                self.read(&what.sources[2])? & 0xFFFF,
                            );
                            let dividend = ((hi << 16) | lo) as i32;
                            let divisor = i32::from(divisor as u16 as i16);
                            let (quotient, remainder) = if divisor == 0 {
                                (0, 0)
                            } else {
                                (dividend.wrapping_div(divisor), dividend.wrapping_rem(divisor))
                            };
                            self.write(&what.dests[0], quotient as u32 & 0xFFFF)?;
                            self.write(&what.dests[1], remainder as u32 & 0xFFFF)?;
                        }
                        Operation::Push => {
                            let value = self.read(&what.sources[0])?;
                            self.stack.push(value);
                        }
                        Operation::Pop => {
                            let value = self.stack.pop().ok_or("pop of an empty stack")?;
                            self.write(&what.dests[0], value)?;
                        }
                        Operation::Exchange => {
                            let (a, b) = (self.read(&what.dests[0])?, self.read(&what.dests[1])?);
                            self.write(&what.dests[0], b)?;
                            self.write(&what.dests[1], a)?;
                        }
                        Operation::Call => {
                            let argument = if self.virtual_ {
                                let (held, _) = one.requires.first().ok_or("a call with no argument")?;
                                *self.vals.get(&held.value).ok_or("the argument is read before it is set")?
                            } else {
                                self.register(RegId::AX)
                            };
                            let answer = (argument.wrapping_mul(31).wrapping_add(7)) & 0xFFFF;
                            if self.virtual_ {
                                let (held, _) = one.delivers.first().ok_or("a call with no result")?;
                                self.vals.insert(held.value, answer);
                            } else {
                                for register in &one.clobbers {
                                    self.poison += 1;
                                    self.regs.insert(ir::root(*register), Self::poisoned(self.poison));
                                }
                                self.regs.insert(RegId::EAX, answer);
                            }
                        }
                        Operation::Jump => next = what.target,
                        Operation::Branch => {
                            // The loop runs three times, whatever the branch is
                            // spelt.
                            branches = what.target;
                            if self.taken < 3 {
                                next = what.target;
                            }
                        }
                        other => return Err(format!("cannot run {other:?}")),
                    }
                    if next.is_some() {
                        break;
                    }
                }
                at = match next {
                    Some(to) => to,
                    None => match block.succ.as_slice() {
                        [] => return Ok(()),
                        [only] => *only,
                        many => {
                            *many.iter().find(|to| Some(**to) != branches).ok_or("a branch with nowhere to fall to")?
                        }
                    },
                };
            }
            Err(format!("did not finish, in {at:#x}"))
        }
    }
}

/// The same stores, from the body as generated and from the body allocated.
fn behaves_alike(
    generated: &LirBody,
    allocated: &LirBody,
    pool: usize,
) -> Result<(), String> {
    let (mut before, mut after) = (run::Machine::new(true), run::Machine::new(false));
    before.execute(generated, LOOP).map_err(|why| format!("generated: {why}"))?;
    after.execute(allocated, LOOP).map_err(|why| format!("allocated: {why}"))?;
    if before.log != after.log {
        return Err(format!("stores differ: {:?} against {:?}", before.log, after.log));
    }
    for at in 0..pool as i64 {
        let cell = EXIT_CELL + 2 * at;
        let (want, got) = (before.frame(cell), after.frame(cell));
        if want != got {
            return Err(format!("value#{} left as {got:#x}, not {want:#x}", at + 1));
        }
    }
    Ok(())
}

fn allocated(
    seed: u64,
    shape: &Shape,
    cpu: &str,
) -> Result<(), String> {
    let (body, notes) = body(seed, shape);
    let generated = body.clone();
    let segments = &*target::BUILT_IN;
    let mut phases: Vec<Box<dyn LIRTransform>> = vec![Box::new(RegAlloc::new(
        None,
        None,
        ProfileOrName::Name(cpu),
        segments,
        &crate::backend::classes::RegisterClasses::m16(),
    )?)];
    if std::env::var_os("FUZZ_NO_PARCOPY").is_none() {
        phases.push(Box::new(ParallelCopy::default()));
    }
    if std::env::var_os("FUZZ_SHOW").is_some() {
        eprintln!("{}", crate::backend::lirtext::lir_stage("input", &[(body.name.clone(), body.clone())]));
    }
    let mut now = body;
    for phase in &mut phases {
        now = phase.transform(now).map_err(|error| format!("{}: {error}\n{}", phase.name(), notes.join("\n")))?;
    }
    if std::env::var_os("FUZZ_SHOW").is_some() {
        eprintln!("{}", crate::backend::lirtext::lir_stage("allocated", &[(now.name.clone(), now.clone())]));
        for block in &now.blocks {
            eprintln!(
                "BLOCK {:#x} succ {:x?} last {:?}",
                block.at,
                block.succ,
                block.insns.last().and_then(|one| one.what.as_ref()).map(|what| (&what.name, what.target))
            );
        }
    }
    let mut bad = complaints(&now);
    if bad.is_empty() {
        bad.extend(behaves_alike(&generated, &now, shape.pool).err());
    }
    if bad.is_empty() { Ok(()) } else { Err(format!("{}\n{}", bad.join("\n"), notes.join("\n"))) }
}

/// The generated body in SSA through the production phases from phi elimination
/// on, and the result must store what the generated body stores.
fn spilled_and_allocated(
    seed: u64,
    shape: &Shape,
    cpu: &str,
) -> Result<(), String> {
    let (body, notes) = body(seed, shape);
    let generated = body.clone();
    let segments = &*target::BUILT_IN;
    let frame = std::rc::Rc::new(std::cell::RefCell::new(crate::backend::frame::Frame::new(0)));
    let mut phases: Vec<Box<dyn LIRTransform>> = vec![
        Box::new(PhiElimination::default()),
        Box::new(TwoAddress::default()),
        Box::new(Coalescer::new(None, segments, &crate::backend::classes::RegisterClasses::m16())),
        Box::new(RegAlloc::new(
            None,
            Some(frame),
            ProfileOrName::Name(cpu),
            segments,
            &crate::backend::classes::RegisterClasses::m16(),
        )?),
        Box::new(ParallelCopy::default()),
    ];
    if std::env::var_os("FUZZ_NO_PARCOPY").is_some() {
        phases.pop();
    }
    let mut now = in_ssa(&body);
    for phase in &mut phases {
        if std::env::var_os("FUZZ_SHOW").is_some() {
            eprintln!(
                "{}",
                crate::backend::lirtext::lir_stage(
                    &format!("before {}", phase.name()),
                    &[(now.name.clone(), now.clone())]
                )
            );
        }
        now = phase.transform(now).map_err(|error| format!("{}: {error}\n{}", phase.name(), notes.join("\n")))?;
    }
    let mut bad = complaints(&now);
    if bad.is_empty() {
        bad.extend(behaves_alike(&generated, &now, shape.pool).err());
    }
    if bad.is_empty() { Ok(()) } else { Err(format!("{}\n{}", bad.join("\n"), notes.join("\n"))) }
}

/// Seeds the lane found wrong results for: the allocator spilled a value whose
/// number SsaSpill had already slotted and was handed that slot, whose contents
/// were another live range's (seed 17: value#2 left as 0x69df, not 0x6ffc).
#[test]
fn test_the_allocator_is_not_handed_the_spillers_slots() {
    for seed in [17, 28] {
        for cpu in ["386", "486", "Core", "P5"] {
            let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
            spilled_and_allocated(seed, &shape, cpu).unwrap_or_else(|why| panic!("seed {seed} on {cpu}: {why}"));
        }
    }
}

/// Seeds the lane found wrong code for: the coalescer took a member out of a
/// parallel copy to make it a move of the constant its source holds, and that
/// move wrote before the other members read (seed 91: value#5 left as 0x45, not
/// 0x0), or claimed the bytes its source's definition still claimed (seeds 130,
/// 163, 233).
#[test]
fn test_a_copy_made_a_move_of_its_constant_keeps_its_place_and_its_bytes() {
    for seed in [91, 130, 163, 233] {
        for cpu in ["386", "486", "Core", "P5"] {
            let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
            spilled_and_allocated(seed, &shape, cpu).unwrap_or_else(|why| panic!("seed {seed} on {cpu}: {why}"));
        }
    }
}

#[test]
fn test_generated_ssa_bodies_spill_and_allocate() {
    let count: u64 = std::env::var("FUZZ_SEEDS").ok().and_then(|one| one.parse().ok()).unwrap_or(40);
    let only: Option<u64> = std::env::var("FUZZ_SEED").ok().and_then(|one| one.parse().ok());
    let cpus: Vec<String> = std::env::var("FUZZ_CPU")
        .map(|one| vec![one])
        .unwrap_or_else(|_| ["386", "486", "Core", "P5"].map(str::to_owned).to_vec());
    let mut failures = Vec::new();
    for seed in only.map_or(0..count, |one| one..one + 1) {
        for cpu in &cpus {
            let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
            if let Err(why) = spilled_and_allocated(seed, &shape, cpu) {
                failures.push(format!("seed {seed} on {cpu} (pool {}, ops {}): {why}", shape.pool, shape.ops));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failed; replay with FUZZ_SEED=<seed>:\n{}",
        failures.len(),
        failures[..failures.len().min(3)].join("\n\n")
    );
}

#[test]
fn test_generated_bodies_allocate() {
    let count: u64 = std::env::var("FUZZ_SEEDS").ok().and_then(|one| one.parse().ok()).unwrap_or(40);
    let only: Option<u64> = std::env::var("FUZZ_SEED").ok().and_then(|one| one.parse().ok());
    let cpus: Vec<String> = std::env::var("FUZZ_CPU")
        .map(|one| vec![one])
        .unwrap_or_else(|_| ["386", "486", "Core", "P5"].map(str::to_owned).to_vec());
    let mut failures = Vec::new();
    for seed in only.map_or(0..count, |one| one..one + 1) {
        for cpu in &cpus {
            // Pressure from a few over the registers to far over.
            let mut shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
            if let Some(one) = std::env::var("FUZZ_OPS").ok().and_then(|one| one.parse().ok()) {
                shape.ops = one;
            }
            if let Some(one) = std::env::var("FUZZ_POOL").ok().and_then(|one| one.parse().ok()) {
                shape.pool = one;
            }
            if let Err(why) = allocated(seed, &shape, cpu) {
                failures.push(format!("seed {seed} on {cpu} (pool {}, ops {}): {why}", shape.pool, shape.ops));
            }
        }
    }
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for one in &failures {
        let why = one.lines().next().unwrap_or("").split(": ").skip(1).collect::<Vec<_>>().join(": ");
        let why: String = why.chars().filter(|c| !c.is_ascii_digit()).collect();
        *kinds.entry(why).or_default() += 1;
    }
    assert!(
        failures.is_empty(),
        "{} failed; replay with FUZZ_SEED=<seed>:\n{kinds:#?}\n{}",
        failures.len(),
        failures[..failures.len().min(3)].join("\n\n")
    );
}

/// The walk found a value's intervals by every value live in every block,
/// hashed (the homes of d_faces: 56 values, 240 blocks, 4.4M instructions a
/// call, 616 calls). From where the values occur and the blocks they are live
/// through it finds the same.
#[test]
fn test_intervals_from_occurrences_are_those_of_the_walk() {
    use crate::analysis::intervals as ranges;
    use crate::backend::postings::Postings;
    let mut compared = 0;
    for seed in 0..120_u64 {
        let shape = Shape { pool: 6 + (seed % 9) as usize, ops: 5 + (seed % 11) as usize };
        let (plain, _) = body(seed, &shape);
        let index = ranges::indexed(&plain);
        let postings = Postings::of(&plain);
        let every: Vec<u32> = plain
            .insns()
            .iter()
            .flat_map(|one| one.defines.iter().chain(&one.uses).copied())
            .collect::<std::collections::BTreeSet<u32>>()
            .into_iter()
            .collect();
        for step in [1_usize, 2, 3] {
            let values: Vec<u32> = every
                .iter()
                .copied()
                .enumerate()
                .filter(|(at, _)| at % step == (seed as usize) % step)
                .map(|(_, value)| value)
                .collect();
            let whole = ranges::_ranges_reference(&plain, &index, &|value| values.contains(&value));
            let mut places: IndexMap<u32, Vec<ranges::Occurrence>> = IndexMap::default();
            for &value in &values {
                let mut at: Vec<ranges::Place> = postings
                    .defs_at(value)
                    .chain(postings.uses_at(value))
                    .map(|(block, position)| (block as usize, position as usize))
                    .collect();
                at.sort_unstable();
                at.dedup();
                places.insert(
                    value,
                    at.into_iter()
                        .map(|(block, position)| {
                            let one = &plain.blocks[block].insns[position];
                            ((block, position), one.defines.contains(&value), one.uses.contains(&value))
                        })
                        .collect(),
                );
            }
            let found = ranges::intervals_by_occurrences(&plain, &index, &values, &places);
            assert!(
                found == whole,
                "seed {seed}, every {step}th value: the intervals from occurrences differ from the walk"
            );
            compared += whole.len();
        }
    }
    assert!(compared > 0, "premise: some value was live");
}

/// Reading the intervals of a body copied every interval of it
/// (`_existing_colors` asked for the body's own and added the homes': 0.14 G of
/// d_faces, 616 calls of 8000 segments), though a read needs the remembered
/// answer itself.
#[test]
fn test_reading_the_intervals_of_a_body_asked_of_twice_copies_none() {
    use crate::analysis::intervals as ranges;
    let (plain, _) = body(3, &Shape { pool: 9, ops: 8 });
    let first = ranges::intervals_shared(&plain, None);
    let again = ranges::intervals_shared(&plain, None);
    assert!(std::sync::Arc::ptr_eq(&first, &again), "the second ask copied the answer");
    assert!(*first == ranges::intervals(&plain, None));
}

/// A function's allocator alternates between its base body and a trial's (a
/// spill or a split of it). The facts share one manager: a trial's asks must
/// not evict the base's, or each switch worked both out again. Over six
/// alternations the base is numbered and worked out once, the trial is edited
/// from it once, and every later ask is answered from memory; each answer is
/// the whole walk's.
#[test]
fn test_a_base_and_its_trial_asked_in_turn_are_each_worked_out_once() {
    use crate::analysis::intervals::{edited, intervals, intervals_afresh, numbered, worked};
    let (base, _) = body(4, &Shape { pool: 10, ops: 12 });
    let mut trial_blocks = base.blocks.clone();
    let block = trial_blocks.iter_mut().find(|block| block.insns.len() > 3).expect("a block with instructions");
    let copy = std::sync::Arc::new((*block.insns[1]).clone());
    block.insns.edit(|insns| insns[1] = copy);
    let trial = base.with_blocks(trial_blocks);
    let before = (worked(&base), edited(&base), numbered(&base));
    for _ in 0..6 {
        for body in [&base, &trial] {
            intervals(body, None);
            crate::analysis::intervals::indexed_shared(body);
        }
    }
    let after = (worked(&base), edited(&base), numbered(&base));
    assert_eq!(
        (after.0 - before.0, after.1 - before.1, after.2 - before.2),
        (1, 1, 2),
        "(worked, edited, numbered) over six turns of base and trial: the base once, the trial edited from it"
    );
    for body in [&base, &trial] {
        assert_eq!(intervals(body, None), intervals_afresh(body));
    }
}

/// An answer edited from an earlier one worked the changed values out by taking
/// the body's liveness whole and walking every block (`intervals liveness` and
/// `walk`, 1.5 G of compiling d_faces for 882 edits that name a few values
/// each). It finds them from where they occur; the intervals and weights are
/// the walk's.
#[test]
fn test_an_edited_answer_works_its_changed_values_out_from_where_they_occur() {
    use crate::analysis::intervals::{by_occurrences, edited, intervals, intervals_afresh};
    let (base, _) = body(4, &Shape { pool: 10, ops: 12 });
    intervals(&base, None);
    let mut blocks = base.blocks.clone();
    let block = blocks.iter_mut().find(|block| block.insns.len() > 3).expect("a block with instructions");
    let copy = std::sync::Arc::new((*block.insns[1]).clone());
    block.insns.edit(|insns| insns[1] = copy);
    let trial = base.with_blocks(blocks);
    let before = (edited(&base), by_occurrences(&base));
    let scans = crate::analysis::occurrences::Occurrences::scans(&base);
    let found = intervals(&trial, None);
    assert_eq!(edited(&base) - before.0, 1, "premise: the trial's answer is an edit of the base's");
    assert_eq!(
        crate::analysis::occurrences::Occurrences::scans(&base),
        scans,
        "every instruction of the body was scanned for the changed values' occurrences (156 M of d_faces -O1)"
    );
    assert_eq!(
        by_occurrences(&base) - before.1,
        1,
        "the changed values were walked for, not found from their occurrences"
    );
    assert_eq!(found, intervals_afresh(&trial));
}

/// Two-address fixing copied a set of every live value for each block to see
/// which of a swap's sources live on (`joins` 1024: `lir twoaddr` 6.5 G, slope
/// 1.8). It reads the rows of bits, and builds no set of the live values.
#[test]
fn test_two_address_fixing_builds_no_set_of_the_live_values() {
    use crate::backend::liverows::SETS_BUILT;
    let (plain, _) = body(3, &Shape { pool: 12, ops: 14 });
    SETS_BUILT.with(|one| one.set(0));
    let _ = crate::backend::twoaddr::tied(&plain, Default::default());
    let built = SETS_BUILT.with(|one| one.get());
    assert_eq!(built, 0, "{built} sets of the live values built");
}

/// `liverows::live` was built from per-block sorted sets and converted to bit
/// rows for the fixed point: 24% of compiling QCport's `d_faces` (#559). Dense
/// rows all the way give the same sets.
#[test]
fn test_dense_liveness_is_what_the_sorted_sets_gave() {
    for seed in 0..200 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            assert_eq!(
                crate::backend::liverows::live(&body),
                crate::backend::liverows::live_reference(&body),
                "seed {seed}"
            );
            let (entering, leaving) = crate::backend::liverows::live_reference(&body);
            let rows = crate::backend::liverows::live_rows(&body);
            for block in &body.blocks {
                assert_eq!(
                    rows.entering(block.at).collect::<Vec<_>>(),
                    entering[&block.at].iter().collect::<Vec<_>>(),
                    "seed {seed} entering {}",
                    block.at
                );
                assert_eq!(
                    rows.leaving(block.at).collect::<Vec<_>>(),
                    leaving[&block.at].iter().collect::<Vec<_>>(),
                    "seed {seed} leaving {}",
                    block.at
                );
            }
        }
    }
}

/// Every question of a body's intervals was worked out afresh: a spill was
/// followed by the allocator's facts of the body it made, the spiller's steps
/// and the class check, each asking the same (58% of the asks of compiling
/// `d_faces`, #559). The same instructions are answered from memory; others are
/// not.
#[test]
fn test_the_intervals_of_the_same_instructions_are_worked_out_once() {
    use crate::analysis::intervals::{intervals, worked};
    let (generated, _) = body(3, &Shape { pool: 8, ops: 10 });
    let before = worked(&generated) + crate::analysis::intervals::by_occurrences(&generated);
    let first = intervals(&generated, None);
    let again = intervals(&generated, None);
    assert_eq!(
        worked(&generated) + crate::analysis::intervals::by_occurrences(&generated) - before,
        1,
        "the same body was worked out twice"
    );
    assert_eq!(first, again);
    // A body with one instruction made afresh is another question.
    let mut other = generated.clone();
    let block = other.blocks.iter_mut().find(|block| !block.insns.is_empty()).expect("a block with instructions");
    let copy = std::sync::Arc::new((*block.insns[0]).clone());
    block.insns.edit(|insns| insns[0] = copy);
    intervals(&other, None);
    assert_eq!(
        worked(&generated) + crate::analysis::intervals::by_occurrences(&generated) - before,
        2,
        "another body was answered from the first"
    );
}

/// A spill made a body of nearly the same instructions, and every fact of it
/// was worked out afresh (the allocator's 9784 rebuilds of `d_faces`, #944). An
/// answer is the earlier one moved to the new slots, worked out again for the
/// values the changed instructions name: the same intervals and weights.
#[test]
fn test_the_intervals_of_an_edited_body_are_the_ones_worked_out_afresh() {
    use std::sync::Arc;

    use crate::analysis::intervals::{edited, intervals, intervals_afresh};
    let mut made = 0;
    for seed in 0..120 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            intervals(&body, None);
            let mut other = body.clone();
            let (block, at) = {
                let blocks: Vec<usize> = other
                    .blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, block)| block.insns.len() > 2)
                    .map(|(at, _)| at)
                    .collect();
                let Some(&block) = blocks.get(seed as usize % blocks.len().max(1)) else { continue };
                (block, 1 + seed as usize % (other.blocks[block].insns.len() - 2))
            };
            // One instruction made afresh, and another of the block's put in
            // again beside it: as a spill's reload would be.
            let copy = Arc::new((*other.blocks[block].insns[at]).clone());
            other.blocks[block].insns.edit(|insns| insns[at] = copy);
            let inserted = Arc::new((*other.blocks[block].insns[at - 1]).clone());
            other.blocks[block].insns.edit(|insns| insns.insert(at, inserted));
            let before = edited(&other);
            let found = intervals(&other, None);
            made += edited(&other) - before;
            assert_eq!(found, intervals_afresh(&other), "seed {seed}");
        }
    }
    assert!(made > 50, "premise: bodies were answered by editing ({made})");
}

/// The class of every value was found with the body numbered, its intervals
/// found and the clobber masks built for the `[word+word]` roles, in bodies
/// with none: 3.8 s of compiling `d_faces` (#559).
#[test]
fn test_a_body_with_no_word_address_pairs_is_not_numbered_to_find_classes() {
    let (generated, _) = body(5, &Shape { pool: 8, ops: 6 });
    let before = crate::analysis::intervals::worked(&generated);
    crate::backend::regclass::classes(
        &generated,
        &std::collections::BTreeSet::new(),
        &crate::backend::target::BUILT_IN,
        &crate::backend::classes::RegisterClasses::m16(),
    );
    assert_eq!(
        crate::analysis::intervals::worked(&generated) - before,
        0,
        "intervals were worked out for a body with no word pairs"
    );
}

/// The interval walk looked each instruction's slot up in a map hashed by its
/// address, built a set per group and shifted its live map on every removal:
/// 7.6 s of compiling `d_faces` (#559). Slots counted in place, a
/// live list with gaps: the same intervals in the same order.
#[test]
fn test_the_interval_walk_is_the_references_in_every_order() {
    use crate::analysis::intervals::{_ranges_reference, _walked, indexed};
    for seed in 0..200 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            let index = indexed(&body);
            let all = |_: u32| true;
            assert!(
                _walked(&body, &index, &all).iter().eq(_ranges_reference(&body, &index, &all).iter()),
                "seed {seed}"
            );
            let some = |value: u32| value % 3 != 0;
            assert!(
                _walked(&body, &index, &some).iter().eq(_ranges_reference(&body, &index, &some).iter()),
                "seed {seed} among some"
            );
        }
    }
}

/// Values live into a block come out of its walk in the order they were first
/// read, last read first: two values read and never made in the block. The
/// order of the intervals is part of what is the same.
#[test]
fn test_values_read_and_never_made_in_a_block_come_out_in_the_order_they_were_first_walked() {
    use crate::analysis::intervals::{_ranges_reference, _walked, indexed};
    let read = |at: i64, into: u32, from: u32| {
        Insn::new(
            at,
            Some((at, 1)),
            Some(Semantics {
                name: Some("mov".into()),
                dests: vec![held(into)],
                sources: vec![held(from)],
                ..Semantics::new(Operation::Move)
            }),
            vec![into],
            vec![from],
        )
    };
    let blocks = vec![LirBlock::new(1, vec![Arc::new(read(1, 3, 1)), Arc::new(read(2, 4, 2))])];
    let body = LirBody::new("f", 1, crate::model::lir::shared(blocks), Default::default(), Default::default());
    let index = indexed(&body);
    let walked = _walked(&body, &index, &|_| true);
    assert_eq!(walked.keys().copied().collect::<Vec<_>>(), vec![4, 3, 2, 1]);
    assert!(walked.iter().eq(_ranges_reference(&body, &index, &|_| true).iter()));
}

/// Splitting a value asked of the liveness of every value in every block as
/// sets, built afresh for each split and again for the body it made (9.7% of
/// compiling `d_faces`, #559). One value at a time is asked of, and the rows
/// answer it: whether it is live at a block's entry and exit, as the sets said.
#[test]
fn test_one_values_liveness_from_rows_is_what_the_sets_said() {
    use crate::backend::liverows::{LiveAt, live_reference, live_rows, live_rows_by};
    for seed in 0..150 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            let (entering, leaving) = live_reference(&body);
            let rows = live_rows(&body);
            let values: Vec<u32> = body
                .blocks
                .iter()
                .flat_map(|block| block.insns.iter().flat_map(|one| one.defines.iter().chain(&one.uses).copied()))
                .chain([u32::MAX, 0])
                .collect();
            for &value in values.iter().take(40) {
                let only = live_rows_by(&body, |one| one == value);
                for block in &body.blocks {
                    let (live_in, live_out) =
                        (entering[&block.at].contains(&value), leaving[&block.at].contains(&value));
                    assert_eq!(
                        (rows.live_in(block.at, value), rows.live_out(block.at, value)),
                        (live_in, live_out),
                        "seed {seed} value {value}"
                    );
                    assert_eq!(
                        (only.live_in(block.at, value), only.live_out(block.at, value)),
                        (live_in, live_out),
                        "seed {seed} value {value} alone"
                    );
                }
            }
            assert!(!rows.live_in(i64::MIN, 1), "a block the body does not hold");
        }
    }
}

/// The next value was found by putting every value the body names in a set (a
/// fifth of a split's carving).
#[test]
fn test_the_next_value_is_one_past_the_largest_the_body_names() {
    for seed in 0..150 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            let mut seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::from([0]);
            for block in &body.blocks {
                seen.extend(block.arrives());
                for one in &block.insns {
                    seen.extend(one.defines.iter().chain(&one.uses).copied());
                }
            }
            assert_eq!(body.next_value(), seen.last().copied().expect("seeded") + 1, "seed {seed}");
        }
    }
}

/// The classes of every value numbered the body, found every interval and built
/// the clobber masks for the `[word+word]` roles, all of which the allocator's
/// facts had just found for the same body (2.3 s of compiling `d_faces`, #559).
/// Given them, the classes are the same, in the same order, and nothing is
/// worked out again.
#[test]
fn test_classes_given_the_intervals_and_masks_are_the_classes_found_without() {
    use crate::analysis::intervals::{indexed, intervals, worked};
    use crate::backend::classes::RegisterClasses;
    use crate::backend::masks::_masks;
    use crate::backend::regclass::{Found, classes, classes_given};
    let registers = RegisterClasses::m16();
    for seed in 0..150 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            let live = intervals(&body, None);
            let masks = _masks(&body, &indexed(&body), &crate::backend::target::BUILT_IN);
            let before = worked(&body);
            let given = classes_given(
                &body,
                &std::collections::BTreeSet::new(),
                &crate::backend::target::BUILT_IN,
                &registers,
                &Found { live: &live, masks: &masks },
            );
            assert_eq!(worked(&body) - before, 0, "seed {seed}: intervals were worked out again");
            let alone =
                classes(&body, &std::collections::BTreeSet::new(), &crate::backend::target::BUILT_IN, &registers);
            assert!(given.iter().eq(alone.iter()), "seed {seed}");
        }
    }
    // A body with a `[base+index]` access of two words: the roles are chosen
    // from the intervals and masks.
    let cell = Mem {
        base: Some(Held { value: 1, width: 2 }),
        index: Some(Held { value: 2, width: 2 }),
        scale: 1,
        ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2)
    };
    let load = Insn::new(
        3,
        Some((3, 1)),
        Some(Semantics {
            name: Some("mov".into()),
            dests: vec![held(3)],
            sources: vec![Loc::Mem(cell)],
            ..Semantics::new(Operation::Move)
        }),
        vec![3],
        vec![1, 2],
    );
    let define = |at: i64, value: u32| {
        Insn::new(
            at,
            Some((at, 1)),
            Some(Semantics {
                name: Some("mov".into()),
                dests: vec![held(value)],
                sources: vec![imm(0)],
                ..Semantics::new(Operation::Move)
            }),
            vec![value],
            vec![],
        )
    };
    let blocks = vec![LirBlock::new(1, vec![Arc::new(define(1, 1)), Arc::new(define(2, 2)), Arc::new(load)])];
    let pairs = LirBody::new("f", 1, crate::model::lir::shared(blocks), Default::default(), Default::default());
    let live = intervals(&pairs, None);
    let masks = _masks(&pairs, &indexed(&pairs), &crate::backend::target::BUILT_IN);
    let given = classes_given(
        &pairs,
        &std::collections::BTreeSet::new(),
        &crate::backend::target::BUILT_IN,
        &registers,
        &Found { live: &live, masks: &masks },
    );
    let alone = classes(&pairs, &std::collections::BTreeSet::new(), &crate::backend::target::BUILT_IN, &registers);
    assert!(given.contains_key(&1) && given.contains_key(&2), "the pair's values are confined to a base and an index");
    assert!(given.iter().eq(alone.iter()));
}

/// A mask at `slot` over registers, for the `_clobbered` tests.
fn _mask_at(
    slot: i64,
    during: &[RegId],
    high: &[RegId],
    before: &[RegId],
) -> super::masks::Mask {
    super::masks::Mask {
        slot,
        during: during.iter().copied().collect(),
        high: high.iter().copied().collect(),
        before: before.iter().copied().collect(),
    }
}

/// `_clobbered` agrees with the look at every point it replaced, on random
/// points and values.
#[test]
fn test_clobbered_agrees_with_a_look_at_every_point() {
    use super::masks::{_clobbered, _clobbered_reference, Masks};
    use crate::analysis::intervals::{Interval, Segment};
    let registers = [RegId::AX, RegId::BX, RegId::CX, RegId::DX, RegId::SI];
    let mut seed = 99_u64;
    let mut next = |modulus: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) % modulus
    };
    for _ in 0..200 {
        let pick = |next: &mut dyn FnMut(u64) -> u64| -> Vec<RegId> {
            registers.iter().copied().filter(|_| next(4) == 0).collect()
        };
        let list: Vec<_> = (0..next(12))
            .map(|at| {
                let during = pick(&mut next);
                let high = pick(&mut next);
                let before = pick(&mut next);
                _mask_at(at as i64 * 4 + next(3) as i64, &during, &high, &before)
            })
            .collect();
        let masks = Masks::new(list);
        for _ in 0..30 {
            let mut start = next(20) as i64;
            let segments = (0..1 + next(3))
                .map(|_| {
                    let end = start + 1 + next(8) as i64;
                    let seg = Segment { start, end };
                    start = end + next(5) as i64;
                    seg
                })
                .collect();
            let one = Interval::new(1, segments);
            for register in registers {
                for width in [1, 2, 4] {
                    assert_eq!(
                        _clobbered(&one, register, &masks, width),
                        _clobbered_reference(&one, register, &masks, width),
                        "{register:?} width {width} over {:?}",
                        one.segments
                    );
                }
            }
        }
    }
}

/// Every query looked at every point: `_clobbered` was 5.8% of compiling
/// `d_faces` (#559). 20,000 points and 20,000 questions took 1.2 s; they are
/// answered by bisection now.
#[test]
fn test_clobbered_does_not_look_at_every_point() {
    use super::masks::{_clobbered, Masks};
    use crate::analysis::intervals::{Interval, Segment};
    let masks = Masks::new((0..20_000).map(|at| _mask_at(at * 3, &[RegId::DX], &[], &[])).collect());
    let mut clobbered = 0;
    for at in 0..20_000 {
        let one = Interval::new(1, vec![Segment { start: at * 3 + 1, end: at * 3 + 2 }]);
        clobbered += usize::from(_clobbered(&one, RegId::DX, &masks, 2));
    }
    assert_eq!(clobbered, 0, "a value live between two points is not across either");
    // Points looked at, not milliseconds (0.2 s failed at load 200): the
    // reference walks all 20,000 for each question, the bisection none.
    assert!(masks.walked() < 20_000, "{} points looked at for 20,000 questions", masks.walked());
    let one = Interval::new(1, vec![Segment { start: 1, end: 2 }]);
    let _ = super::masks::_clobbered_reference(&one, RegId::DX, &masks, 2);
    assert!(masks.walked() >= 20_000, "the count did not see the reference walk the points");
}

/// A value live through N blocks carried N segments, one per block, so every
/// overlap test on it cost N and the allocator's cost grew with the square of
/// the blocks (branches: 6 values over N blocks). One run goes on where a block
/// begins; only a definition (an odd slot) leaves a gap.
#[test]
fn test_a_value_live_through_blocks_is_one_segment() {
    use crate::analysis::intervals::{PER_INSN, intervals};
    for seed in 0..40u64 {
        let (plain, _) = body(seed, &Shape { pool: 8 + (seed % 7) as usize, ops: 8 + (seed % 11) as usize });
        for (value, one) in intervals(&plain, None) {
            for pair in one.segments.windows(2) {
                assert!(
                    pair[0].end != pair[1].start || pair[1].start % PER_INSN != 0,
                    "seed {seed}: value {value} is cut at a block top, {:?}",
                    one.segments
                );
            }
        }
    }
}

/// Slots are spaced `GAP` apart so a rewrite can add an instruction without
/// moving the others; what the code reads of them is `uniform`, which must be
/// the consecutive numbering's, point for point.
#[test]
fn test_the_spaced_numbering_is_the_consecutive_one_under_uniform() {
    use crate::analysis::frequency::Frequency;
    use crate::analysis::intervals::{indexed, indexed_consecutive, intervals_where};
    for seed in 0..40u64 {
        let (plain, _) = body(seed, &Shape { pool: 8 + (seed % 7) as usize, ops: 8 + (seed % 11) as usize });
        let (spaced, consecutive) = (indexed(&plain), indexed_consecutive(&plain));
        for (key, slot) in &spaced.at {
            assert_eq!(spaced.uniform(*slot), consecutive.at[key], "seed {seed}: an instruction's slot");
        }
        for (block, (first, last)) in &spaced.span {
            let (was_first, was_last) = consecutive.span[block];
            assert_eq!(
                (spaced.uniform(*first), spaced.uniform(*last)),
                (was_first, was_last),
                "seed {seed}: block {block}"
            );
        }
        let busy = Frequency::of(&plain);
        let (new, old) = (
            intervals_where(&plain, &spaced, &busy, &|_| true),
            intervals_where(&plain, &consecutive, &busy, &|_| true),
        );
        assert_eq!(new.len(), old.len(), "seed {seed}");
        for (value, one) in &new {
            let was = &old[value];
            let mapped: Vec<(i64, i64)> =
                one.segments.iter().map(|s| (spaced.uniform(s.start), spaced.uniform(s.end))).collect();
            let expected: Vec<(i64, i64)> = was.segments.iter().map(|s| (s.start, s.end)).collect();
            assert_eq!(mapped, expected, "seed {seed}: value {value}");
        }
    }
}

/// The numbering a rewrite leaves is the old one with the instructions it added
/// put between their neighbours' slots (LLVM's `SlotIndexes`): under `uniform`
/// every point is where a fresh consecutive numbering of the new body puts it,
/// however many instructions went in or out; and where a gap has no
/// room left the patch says so, for a fresh numbering to answer.
#[test]
fn test_a_patched_numbering_is_the_consecutive_one_under_uniform() {
    use crate::analysis::intervals::{indexed, indexed_consecutive};
    let mut patched_any = false;
    for seed in 0..60u64 {
        let (plain, _) = body(seed, &Shape { pool: 8 + (seed % 7) as usize, ops: 8 + (seed % 11) as usize });
        let mut numbering = indexed(&plain);
        let mut current = plain.clone();
        let mut next = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut random = |bound: usize| {
            next = next.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (next >> 33) as usize % bound
        };
        for _ in 0..12 {
            let mut blocks = current.blocks.clone();
            let at = random(blocks.len());
            let mut insns: Vec<std::sync::Arc<crate::model::lir::Insn>> = blocks[at].insns.to_vec();
            for _ in 0..1 + random(3) {
                match random(3) {
                    0 if !insns.is_empty() => {
                        let one = (*insns[random(insns.len())]).clone();
                        insns.insert(random(insns.len() + 1), std::sync::Arc::new(one));
                    }
                    1 if insns.len() > 1 => {
                        insns.remove(random(insns.len()));
                    }
                    _ if !insns.is_empty() => {
                        let place = random(insns.len());
                        insns[place] = std::sync::Arc::new((*insns[place]).clone());
                    }
                    _ => {}
                }
            }
            blocks[at].insns = insns.into();
            let edited = current.with_blocks(blocks);
            let Some(patched) = numbering.patched(&current, &edited) else {
                numbering = indexed(&edited);
                current = edited;
                continue;
            };
            patched_any = true;
            let whole = indexed_consecutive(&edited);
            assert_eq!(patched.at.len(), whole.at.len(), "seed {seed}: the instructions numbered");
            for (insn, slot) in &patched.at {
                assert_eq!(patched.uniform(*slot), whole.at[insn], "seed {seed}: an instruction's slot");
            }
            for (block, (first, last)) in &patched.span {
                assert_eq!(
                    (patched.uniform(*first), patched.uniform(*last)),
                    whole.span[block],
                    "seed {seed}: block {block}"
                );
            }
            assert_eq!(patched.epoch, numbering.epoch, "seed {seed}: a patch is of the same numbering");
            numbering = patched;
            current = edited;
        }
    }
    assert!(patched_any, "no edit was patched");
}

/// A gap halves with each instruction added in it: after 20 added at one point
/// the next has no room and the numbering must be made afresh, not patched into
/// slots that collide.
#[test]
fn test_a_gap_with_no_room_left_is_answered_by_a_fresh_numbering() {
    use crate::analysis::intervals::indexed;
    let (plain, _) = body(3, &Shape { pool: 8, ops: 12 });
    let mut numbering = indexed(&plain);
    let mut current = plain;
    let mut refused = false;
    for round in 0..40 {
        let mut blocks = current.blocks.clone();
        let mut insns: Vec<std::sync::Arc<crate::model::lir::Insn>> = blocks[0].insns.to_vec();
        let copy = std::sync::Arc::new((*insns[1]).clone());
        insns.insert(1, copy);
        blocks[0].insns = insns.into();
        let edited = current.with_blocks(blocks);
        match numbering.patched(&current, &edited) {
            Some(patched) => numbering = patched,
            None => {
                refused = true;
                numbering = indexed(&edited);
                assert!(round >= 15, "a gap of 2^20 ran out after only {round} instructions");
            }
        }
        current = edited;
    }
    assert!(refused, "40 instructions in one gap were all patched");
}

/// The points that destroy registers, carried across a rewrite, are the points
/// a fresh look at the rewritten body finds, in the same order and at the same
/// slots in the registers' lists: a rewrite that removed or added a call took
/// its point out or put it in, and left every other point where it was.
#[test]
fn test_the_points_that_destroy_registers_carried_across_a_rewrite_are_those_of_a_fresh_look() {
    use super::masks::{_masks, Masks};
    use crate::analysis::intervals::{changes, indexed};
    let segments = &crate::backend::target::BUILT_IN;
    let mut carried = 0;
    for seed in 0..80u64 {
        let (plain, _) = body(seed, &Shape { pool: 8 + (seed % 7) as usize, ops: 12 + (seed % 11) as usize });
        let mut numbering = indexed(&plain);
        let mut current = plain.clone();
        let mut masks = _masks(&current, &numbering, segments);
        let _ = masks.same_points(&masks);
        let mut next = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut random = |bound: usize| {
            next = next.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (next >> 33) as usize % bound
        };
        for _ in 0..12 {
            let mut blocks = current.blocks.clone();
            let at = random(blocks.len());
            let mut insns: Vec<Arc<Insn>> = blocks[at].insns.to_vec();
            for _ in 0..1 + random(3) {
                match random(3) {
                    0 if !insns.is_empty() => {
                        let mut one = (*insns[random(insns.len())]).clone();
                        if random(2) == 0 {
                            one.clobbers = [RegId::EAX, RegId::ECX].into_iter().collect();
                        }
                        insns.insert(random(insns.len() + 1), Arc::new(one));
                    }
                    1 if insns.len() > 1 => {
                        insns.remove(random(insns.len()));
                    }
                    _ if !insns.is_empty() => {
                        let place = random(insns.len());
                        insns[place] = Arc::new((*insns[place]).clone());
                    }
                    _ => {}
                }
            }
            blocks[at].insns = insns.into();
            let edited = current.with_blocks(blocks);
            let Some(patched) = numbering.patched(&current, &edited) else {
                numbering = indexed(&edited);
                masks = _masks(&edited, &numbering, segments);
                current = edited;
                continue;
            };
            let found = changes(&current, &edited).and_then(|changes| {
                let gone: IndexMap<usize, i64> = changes
                    .gone
                    .iter()
                    .filter_map(|one| {
                        numbering
                            .at
                            .get(&crate::analysis::intervals::key(one))
                            .map(|slot| (crate::analysis::intervals::key(one), *slot))
                    })
                    .collect();
                masks.reserve_points(4096);
                Masks::after(std::mem::take(&mut masks), &gone, &patched, segments, &changes)
            });
            let fresh = _masks(&edited, &patched, segments);
            let found = found.expect("a rewrite of the same shape carries the points");
            assert!(found.same_points(&fresh), "seed {seed}: the points carried differ from a fresh look");
            // The list is edited where it stands (a copy of it was 5 M
            // instructions of d_faces -O2 over 195 rewrites): the
            // room reserved in it is kept.
            assert!(found.point_capacity() >= 4096, "seed {seed}: the list of points was built again");
            carried += 1;
            masks = found;
            numbering = patched;
            current = edited;
        }
    }
    assert!(carried > 100, "only {carried} rewrites carried their points");
}

/// The counts of what the instructions say of the values were copied whole
/// at each of the 195 rewrites of d_faces -O2 (6.3 M instructions): a scan made
/// after a rewrite is the one before it, edited where it stands.
#[test]
fn test_a_scan_after_a_rewrite_is_edited_where_it_stands() {
    use crate::backend::classes::RegisterClasses;
    use crate::backend::regclass::Scan;
    let registers = RegisterClasses::m16();
    let mut checked = 0;
    for seed in 0..40_u64 {
        let (plain, _) = body(seed, &Shape { pool: 10, ops: 14 });
        let scan = Scan::of(&plain, &registers, &crate::backend::target::BUILT_IN);
        let values: Vec<u32> = plain.insns().into_iter().flat_map(|one| one.uses.clone()).collect();
        let Some(&value) = values.first() else { continue };
        let Some(at) = scan.counts_address(value) else { continue };
        let moved = scan.after(&[], &[], &registers, &crate::backend::target::BUILT_IN);
        assert_eq!(moved.counts_address(value), Some(at), "seed {seed}: the counts were copied");
        checked += 1;
    }
    assert!(checked > 10, "premise: only {checked} bodies had counts to compare");
}

/// The live intervals built by `LiveRangeCalc` (SlotIndexes, dominator tree,
/// PHI-def insertion) are live in and out of exactly the blocks the dataflow
/// liveness says.
#[test]
fn test_live_intervals_are_live_where_the_dataflow_says() {
    use crate::backend::liverows::{LiveAt, live_rows};
    use crate::liveness::liveintervals::LiveIntervals;
    let mut checked = 0;
    let mut multi_def = 0;
    for seed in 0..120u64 {
        let (plain, _) = body(seed, &Shape { pool: 6 + (seed % 7) as usize, ops: 6 + (seed % 13) as usize });
        let rows = live_rows(&plain);
        let mut lis = LiveIntervals::new(&plain);
        let values: Vec<u32> = lis.values().collect();
        for value in values {
            let li = lis.interval(value).clone();
            multi_def += usize::from(li.get_num_val_nums() > 1);
            for (number, block) in plain.blocks.iter().enumerate() {
                let start = lis.indexes.get_mbb_start_idx(number);
                let last = lis.indexes.get_mbb_last_idx(number);
                let live_in = !li.empty() && li.live_at(&start);
                let live_out = !li.empty() && li.live_at(&last);
                assert_eq!(
                    live_in,
                    rows.live_in(block.at, value),
                    "seed {seed}: value#{value} live in to block {number}: {li}"
                );
                assert_eq!(
                    live_out,
                    rows.live_out(block.at, value),
                    "seed {seed}: value#{value} live out of block {number}: {li}"
                );
                checked += 1;
            }
            assert!(li.verify(), "seed {seed}: value#{value} {li}");
        }
    }
    assert!(
        checked > 1000 && multi_def > 0,
        "premise: bodies with values defined more than once ({checked}, {multi_def})"
    );
}

/// The generated bodies of seeds `0..count` through the allocator on the 386,
/// for tests that count what the allocation did.
pub(crate) fn allocate_generated_bodies(count: u64) {
    for seed in 0..count {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (body, _) = body(seed, &shape);
        let mut phase = RegAlloc::new(
            None,
            None,
            ProfileOrName::Name("386"),
            &*target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("a phase");
        phase.transform(body).expect("allocated");
    }
}

/// The allocator asked `constrain::required` of the same body twice (2.16 M
/// Ir on d_faces -O2): one allocation asks it once.
#[test]
fn test_one_allocation_asks_what_the_body_requires_once() {
    let shape = Shape { pool: 7, ops: 6 };
    let asked = |seed| {
        let before = crate::backend::constrain::REQUIRED_ASKED.with(|asked| asked.get());
        allocated(seed, &shape, "486").expect("allocates");
        crate::backend::constrain::REQUIRED_ASKED.with(|asked| asked.get()) - before
    };
    for seed in [0, 1, 2] {
        assert_eq!(asked(seed), 1, "seed {seed}");
    }
}
