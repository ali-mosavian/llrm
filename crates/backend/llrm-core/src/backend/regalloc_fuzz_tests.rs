//! A fuzz lane for the register allocator: generated bodies at high pressure,
//! every constraint kind, and the result must be an allocation (the allocator
//! is total), no longer name a value, and store what the generated body stores.
//!
//! `FUZZ_SEEDS=N` runs N seeds (default 40); `FUZZ_SEED=S` replays one and
//! prints its body; `FUZZ_CPU` names the profile (default: each of 386, 486, Core, P5).

use std::sync::Arc;

use iced_x86::Register;

use crate::backend::allocate::RegAlloc;
use crate::backend::coalesce::Coalescer;
use crate::backend::phielim::PhiElimination;
use crate::backend::ssaspill::SsaSpill;
use crate::backend::twoaddr::TwoAddress;
use crate::backend::cpu::ProfileOrName;
use crate::backend::parcopy::ParallelCopy;
use crate::backend::{target, verify};
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

    fn below(&mut self, count: usize) -> usize {
        (self.next() % count as u64) as usize
    }

    fn pick(&mut self, from: &[u32]) -> u32 {
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
    Mem { through: Register::BP, disp_width: 2, ..Mem::new(Some(Addr::new(Space::Frame, disp)), 2) }
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

    fn insn(&mut self, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, defines: &[u32], uses: &[u32]) -> Insn {
        self.at += 4;
        let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
        Insn::new(self.at, Some((self.at, self.at + 2)), Some(what), defines.to_vec(), uses.to_vec())
    }

    fn jump(&mut self, name: &str, op: Operation, to: i64) -> Insn {
        self.at += 4;
        let what = Semantics { name: Some(name.to_owned()), target: Some(to), ..Semantics::new(op) };
        Insn::new(self.at, Some((self.at, self.at + 2)), Some(what), vec![], vec![])
    }

    fn mov(&mut self, into: u32, out_of: Loc, uses: &[u32]) -> Insn {
        self.insn(Operation::Move, "mov", vec![held(into)], vec![out_of], &[into], uses)
    }

    /// One random operation over `pool`, in the order of a loop body.
    fn operation(&mut self, pool: &[u32], out: &mut Vec<Insn>) {
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
            2 | 3 => out.push(self.insn(Operation::Binary, "add", vec![held(d)], vec![held(d), held(s)], &[d], &[d, s])),
            // A base and an index.
            4 => {
                let cell = Mem { base: Some(Held { value: s, width: 2 }), index: Some(Held { value: t, width: 2 }), ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2) };
                out.push(self.insn(Operation::Move, "mov", vec![held(d)], vec![Loc::Mem(cell)], &[d], &[s, t]));
            }
            5 => {
                let cell = Mem { base: Some(Held { value: s, width: 2 }), ..Mem::new(Some(Addr::new(Space::Literal, 2)), 2) };
                out.push(self.insn(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![held(d)], &[], &[s, d]));
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
            8 if d != s => out.push(self.insn(Operation::Binary, "shl", vec![held(d)], vec![held(d), held(s)], &[d], &[d, s])),
            // A call: arguments pushed, everything clobbered, the answer in ax.
            9 => {
                let result = self.fresh();
                let mut call = self.insn(Operation::Call, "call", vec![], vec![], &[result], &[s]);
                call.requires = vec![(Held { value: s, width: 2 }, Register::AX)];
                call.delivers = vec![(Held { value: result, width: 2 }, Register::AX)];
                call.clobbers = [Register::EAX, Register::ECX, Register::EDX, Register::EBX, Register::ES].into_iter().collect();
                out.push(call);
                out.push(self.mov(d, held(result), &[result]));
            }
            // A far access: a selector and a base.
            10 if s != t => {
                let cell = Mem {
                    base: Some(Held { value: s, width: 2 }),
                    selector: Some(Held { value: t, width: 2 }),
                    ..Mem::new(Some(Addr::new(Space::Far, 0)), 2)
                };
                out.push(self.insn(Operation::Move, "mov", vec![held(d)], vec![Loc::Mem(cell)], &[d], &[s, t]));
            }
            // A far pointer loaded whole, then read through: the segment half
            // is a selector and nothing else.
            12 => {
                let (offset, segment) = (self.fresh(), self.fresh());
                let at = 40 + 4 * self.rng.below(4) as i64;
                let cell = Mem { width: 4, ..frame(at) };
                out.push(self.insn(Operation::Move, "les", vec![held(offset), held(segment)], vec![Loc::Mem(cell)], &[offset, segment], &[]));
                let far = Mem {
                    base: Some(Held { value: offset, width: 2 }),
                    selector: Some(Held { value: segment, width: 2 }),
                    ..Mem::new(Some(Addr::new(Space::Far, 0)), 2)
                };
                out.push(self.insn(Operation::Move, "mov", vec![held(d)], vec![Loc::Mem(far)], &[d], &[offset, segment]));
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
fn body(seed: u64, shape: &Shape) -> (LirBody, Vec<String>) {
    let mut build = Builder { rng: Rng(seed), at: 0, next: shape.pool as u32, notes: Vec::new() };
    let pool: Vec<u32> = (1..=shape.pool as u32).collect();

    let mut entry: Vec<Insn> = Vec::new();
    for (at, value) in pool.iter().enumerate() {
        entry.push(if at % 3 == 0 { build.insn(Operation::Move, "mov", vec![held(*value)], vec![Loc::Mem(frame(6 + 2 * at as i64))], &[*value], &[]) } else { build.mov(*value, imm(at as i64), &[]) });
    }
    entry.push(build.jump("jmp", Operation::Jump, LOOP));

    let mut inside: Vec<Insn> = Vec::new();
    let skipped: Vec<usize> = std::env::var("FUZZ_SKIP").map(|one| one.split(',').filter_map(|at| at.parse().ok()).collect()).unwrap_or_default();
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
        exit.push(build.insn(Operation::Move, "mov", vec![Loc::Mem(frame(EXIT_CELL + 2 * at as i64))], vec![held(*value)], &[], &[*value]));
    }

    let block = |at: i64, insns: Vec<Insn>, succ: Vec<i64>| {
        let mut block = LirBlock::new(at, insns.into_iter().map(Arc::new).collect());
        block.succ = succ;
        block
    };
    let blocks = vec![block(ENTRY, entry, vec![LOOP]), block(LOOP, inside, vec![LOOP, EXIT]), block(EXIT, exit, vec![])];
    (LirBody::new(format!("fuzz{seed}"), ENTRY, blocks, IndexMap::default(), IndexMap::default()), build.notes)
}

/// `place` with every value it reads renamed by `map`.
fn read_as(place: &Loc, map: &dyn Fn(u32) -> u32) -> Loc {
    let held = |one: Option<Held>| one.map(|one| Held { value: map(one.value), ..one });
    match place {
        Loc::Held(one) => Loc::Held(Held { value: map(one.value), ..*one }),
        Loc::Mem(cell) => Loc::Mem(Mem { base: held(cell.base), index: held(cell.index), selector: held(cell.selector), ..cell.clone() }),
        other => other.clone(),
    }
}

/// The generated body in SSA: the loop's variables become phis at its header, every
/// definition a fresh name, and the exit reads the names the loop ends with. Values
/// an instruction defines that are not variables (a quotient, a call's answer) are
/// already defined once and keep their names.
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
        let made: IndexMap<u32, u32> = one.defines.iter().map(|value| (*value, if variables.contains(value) { fresh() } else { *value })).collect();
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
        changed.requires = one.requires.iter().map(|(held, register)| (Held { value: read(held.value), ..*held }, *register)).collect();
        changed.delivers = one.delivers.iter().map(|(held, register)| (Held { value: written(held.value), ..*held }, *register)).collect();
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
                changed.what = Some(Semantics { dests: what.dests.iter().map(|place| read_as(place, &latch)).collect(), sources: what.sources.iter().map(|place| read_as(place, &latch)).collect(), ..what.clone() });
            }
            changed.uses = one.uses.iter().map(|value| latch(*value)).collect();
            Arc::new(changed)
        })
        .collect();
    let mut blocks = body.blocks.clone();
    blocks[1] = LirBlock { phis: headers.iter().map(|(value, result)| Phi { result: *result, incoming: vec![(ENTRY, *value), (LOOP, latch(*value))] }).collect(), ..blocks[1].with_insns(inside) };
    blocks[2] = blocks[2].with_insns(exit);
    body.with_blocks(blocks)
}

/// What a finished body must be: every value placed, nothing a later phase
/// cannot schedule or the machine cannot name.
fn complaints(done: &LirBody) -> Vec<String> {
    let mut out = verify::verify(done, false);
    for one in done.insns() {
        let Some(what) = &one.what else { continue };
        for place in what.dests.iter().chain(&what.sources) {
            let unplaced = match place {
                Loc::Held(_) => true,
                Loc::Mem(cell) => {
                    (cell.base.is_some() && cell.through == Register::None)
                        || (cell.index.is_some() && cell.index_through == Register::None)
                        || (cell.selector.is_some() && cell.addr.is_some_and(|addr| addr.space == Space::Far && addr.segment == Register::None))
                }
                _ => false,
            };
            if unplaced {
                out.push(format!("{:#06x}: {} is not placed", one.at, place.repr()));
            }
            if let Loc::Reg(Reg { register, width }) = place {
                if target::width_of(*register).is_some_and(|got| got != i64::from(*width)) {
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
    use crate::support::hash::HashMap;

    use iced_x86::Register;

    use crate::model::ir::{self, Loc, Mem, Operation, Space};
    use crate::model::lir::LirBody;

    pub struct Machine {
        virtual_: bool,
        vals: HashMap<u32, u32>,
        regs: HashMap<Register, u32>,
        mem: HashMap<(i64, i64, i64, i64), u32>,
        stack: Vec<u32>,
        poison: u32,
        /// Every store that leaves the frame, in order.
        pub log: Vec<((i64, i64, i64, i64), u32)>,
        taken: usize,
    }

    fn hashed(key: (i64, i64, i64, i64)) -> u32 {
        let mut z = (key.0 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (key.1 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z ^= (key.2 as u64).wrapping_mul(0x94D0_49BB_1331_11EB) ^ (key.3 as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93);
        ((z ^ (z >> 29)) & 0xFFFF) as u32
    }

    impl Machine {
        pub fn new(virtual_: bool) -> Self {
            Self { virtual_, vals: HashMap::default(), regs: HashMap::default(), mem: HashMap::default(), stack: Vec::new(), poison: 0, log: Vec::new(), taken: 0 }
        }

        /// Nothing a body computes: the low word is hashed, so arithmetic that
        /// masks to a word cannot land on a small constant by luck.
        fn poisoned(count: u32) -> u32 {
            0x8000_0000 | hashed((i64::from(count), 7, 7, 7))
        }

        fn register(&mut self, register: Register) -> u32 {
            let poison = &mut self.poison;
            *self.regs.entry(ir::root(register)).or_insert_with(|| {
                *poison += 1;
                Self::poisoned(*poison)
            })
        }

        fn key(&mut self, cell: &Mem) -> Result<(i64, i64, i64, i64), String> {
            let space = cell.addr.map_or(-1, |addr| addr.space as i64);
            let disp = cell.addr.map_or(cell.offset, |addr| addr.disp);
            if cell.addr.is_some_and(|addr| addr.space == Space::Frame) {
                return Ok((space, disp + cell.offset, 0, 0));
            }
            let placed = |held: Option<ir::Held>, through: Register, this: &mut Self| -> Result<i64, String> {
                match held {
                    None => Ok(0),
                    Some(_) if through != Register::None => Ok(i64::from(this.register(through))),
                    Some(held) => this.vals.get(&held.value).map(|got| i64::from(*got)).ok_or(format!("value#{} is read before it is set", held.value)),
                }
            };
            let base = placed(cell.base, cell.through, self)?;
            let index = placed(cell.index, cell.index_through, self)? * cell.scale;
            let segment = match (cell.addr.map(|addr| addr.segment), cell.selector) {
                (Some(segment), _) if segment != Register::None => i64::from(self.register(segment)),
                (_, Some(selector)) => self.vals.get(&selector.value).map(|got| i64::from(*got)).ok_or("a selector is read before it is set")?,
                _ => 0,
            };
            // Sixteen-bit addressing wraps.
            Ok((space, segment, (disp + base + index) & 0xFFFF, 0))
        }

        fn read(&mut self, place: &Loc) -> Result<u32, String> {
            Ok(match place {
                Loc::Imm(imm) => (imm.value & 0xFFFF) as u32,
                Loc::Held(held) => *self.vals.get(&held.value).ok_or(format!("value#{} is read before it is set", held.value))?,
                Loc::Reg(reg) => self.register(reg.register),
                Loc::Mem(cell) => {
                    let key = self.key(cell)?;
                    *self.mem.get(&key).unwrap_or(&hashed(key))
                }
                other => return Err(format!("cannot read {other:?}")),
            })
        }

        fn write(&mut self, place: &Loc, value: u32) -> Result<(), String> {
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

        pub fn frame(&mut self, disp: i64) -> u32 {
            let key = (Space::Frame as i64, disp, 0, 0);
            *self.mem.get(&key).unwrap_or(&hashed(key))
        }

        /// The body from its entry, its loop run three times.
        pub fn execute(&mut self, body: &LirBody, again: i64) -> Result<(), String> {
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
                            if crate::backend::target::far_load(what) {
                                // A far pointer: its offset, then its segment, a word on.
                                let Loc::Mem(cell) = &what.sources[0] else { return Err("les from a register".to_owned()) };
                                let next = Mem { addr: cell.addr.map(|addr| ir::Addr { disp: addr.disp + 2, ..addr }), ..cell.clone() };
                                let (offset, segment) = (self.read(&Loc::Mem(Mem { width: 2, ..cell.clone() }))?, self.read(&Loc::Mem(Mem { width: 2, ..next }))?);
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
                            let (hi, lo, divisor) = (self.read(&what.sources[0])? & 0xFFFF, self.read(&what.sources[1])? & 0xFFFF, self.read(&what.sources[2])? & 0xFFFF);
                            let dividend = ((hi << 16) | lo) as i32;
                            let divisor = i32::from(divisor as u16 as i16);
                            let (quotient, remainder) = if divisor == 0 { (0, 0) } else { (dividend.wrapping_div(divisor), dividend.wrapping_rem(divisor)) };
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
                                self.register(Register::AX)
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
                                self.regs.insert(Register::EAX, answer);
                            }
                        }
                        Operation::Jump => next = what.target,
                        Operation::Branch => {
                            // The loop runs three times, whatever the branch is spelt.
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
                        many => *many.iter().find(|to| Some(**to) != branches).ok_or("a branch with nowhere to fall to")?,
                    },
                };
            }
            Err(format!("did not finish, in {at:#x}"))
        }
    }
}

/// The same stores, from the body as generated and from the body allocated.
fn behaves_alike(generated: &LirBody, allocated: &LirBody, pool: usize) -> Result<(), String> {
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

fn allocated(seed: u64, shape: &Shape, cpu: &str) -> Result<(), String> {
    let (body, notes) = body(seed, shape);
    let generated = body.clone();
    let segments = &*target::BUILT_IN;
    let mut phases: Vec<Box<dyn LIRTransform>> = vec![Box::new(RegAlloc::new(None, None, ProfileOrName::Name(cpu), segments, &crate::backend::classes::RegisterClasses::m16())?)];
    if std::env::var_os("FUZZ_NO_PARCOPY").is_none() {
        phases.push(Box::new(ParallelCopy));
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
            eprintln!("BLOCK {:#x} succ {:x?} last {:?}", block.at, block.succ, block.insns.last().and_then(|one| one.what.as_ref()).map(|what| (&what.name, what.target)));
        }
    }
    let mut bad = complaints(&now);
    if bad.is_empty() {
        bad.extend(behaves_alike(&generated, &now, shape.pool).err());
    }
    if bad.is_empty() { Ok(()) } else { Err(format!("{}\n{}", bad.join("\n"), notes.join("\n"))) }
}

/// The generated body in SSA through the production phases from the spiller on, and the
/// result must store what the generated body stores.
fn spilled_and_allocated(seed: u64, shape: &Shape, cpu: &str) -> Result<(), String> {
    let (body, notes) = body(seed, shape);
    let generated = body.clone();
    let segments = &*target::BUILT_IN;
    let frame = std::rc::Rc::new(std::cell::RefCell::new(crate::backend::frame::Frame::new(0)));
    let mut phases: Vec<Box<dyn LIRTransform>> = vec![
        Box::new(SsaSpill { frame: frame.clone(), segments: segments.clone(), classes: crate::backend::classes::RegisterClasses::m16(), prices: crate::backend::ssaspill::Prices::clocks(), run: Default::default() }),
        Box::new(PhiElimination),
        Box::new(TwoAddress),
        Box::new(Coalescer::new(None, segments, &crate::backend::classes::RegisterClasses::m16())),
        Box::new(RegAlloc::new(None, Some(frame), ProfileOrName::Name(cpu), segments, &crate::backend::classes::RegisterClasses::m16())?),
        Box::new(ParallelCopy),
    ];
    if std::env::var_os("FUZZ_NO_PARCOPY").is_some() {
        phases.pop();
    }
    let mut now = in_ssa(&body);
    if std::env::var_os("FUZZ_NO_SPILL").is_some() {
        phases.remove(0);
    }
    for phase in &mut phases {
        if std::env::var_os("FUZZ_SHOW").is_some() {
            eprintln!("{}", crate::backend::lirtext::lir_stage(&format!("before {}", phase.name()), &[(now.name.clone(), now.clone())]));
        }
        now = phase.transform(now).map_err(|error| format!("{}: {error}\n{}", phase.name(), notes.join("\n")))?;
    }
    let mut bad = complaints(&now);
    if bad.is_empty() {
        bad.extend(behaves_alike(&generated, &now, shape.pool).err());
    }
    if bad.is_empty() { Ok(()) } else { Err(format!("{}\n{}", bad.join("\n"), notes.join("\n"))) }
}

/// Seeds the lane found wrong results for: the allocator spilled a value whose number
/// SsaSpill had already slotted and was handed that slot, whose contents were another
/// live range's (seed 17: value#2 left as 0x69df, not 0x6ffc).
#[test]
fn test_the_allocator_is_not_handed_the_spillers_slots() {
    for seed in [17, 28] {
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
    let cpus: Vec<String> = std::env::var("FUZZ_CPU").map(|one| vec![one]).unwrap_or_else(|_| ["386", "486", "Core", "P5"].map(str::to_owned).to_vec());
    let mut failures = Vec::new();
    for seed in only.map_or(0..count, |one| one..one + 1) {
        for cpu in &cpus {
            let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
            if let Err(why) = spilled_and_allocated(seed, &shape, cpu) {
                failures.push(format!("seed {seed} on {cpu} (pool {}, ops {}): {why}", shape.pool, shape.ops));
            }
        }
    }
    assert!(failures.is_empty(), "{} failed; replay with FUZZ_SEED=<seed>:\n{}", failures.len(), failures[..failures.len().min(3)].join("\n\n"));
}

#[test]
fn test_generated_bodies_allocate() {
    let count: u64 = std::env::var("FUZZ_SEEDS").ok().and_then(|one| one.parse().ok()).unwrap_or(40);
    let only: Option<u64> = std::env::var("FUZZ_SEED").ok().and_then(|one| one.parse().ok());
    let cpus: Vec<String> = std::env::var("FUZZ_CPU").map(|one| vec![one]).unwrap_or_else(|_| ["386", "486", "Core", "P5"].map(str::to_owned).to_vec());
    let mut failures = Vec::new();
    let before = crate::backend::allocate::last_resorts();
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
    eprintln!("last resorts: {}", crate::backend::allocate::last_resorts() - before);
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

/// Seed 4 on 386, twelve values over six registers with calls and far
/// accesses: a reload no register is free for was refused ("value#52 cannot
/// be spilled and no register is free for it", #134, seed 5 before block
/// frequencies moved the spill weights). Its holders cannot all stay, so the
/// last resort evicts the cheapest and spills them.
#[test]
fn test_a_value_that_cannot_be_spilled_takes_a_register_by_force() {
    let before = crate::backend::allocate::last_resorts();
    let shape = Shape { pool: 12, ops: 11 };
    let done = allocated(4, &shape, "386");
    assert!(done.is_ok(), "{done:?}");
    assert!(crate::backend::allocate::last_resorts() > before, "premise: the allocation needed the last resort");
}

/// LLVM and GCC allocate a function once. The allocator here also allocated the body in each other shape its spills suggested,
/// twice each, and kept the cheapest: 85 allocations of `d_faces`'s big function (#944). A profile that does not search makes
/// the one allocation, and the same output where no other shape was cheaper.
#[test]
fn test_an_allocator_that_does_not_search_allocates_a_body_once() {
    use crate::backend::allocate::trials;
    let shape = Shape { pool: 12, ops: 11 };
    let mut searched = 0;
    for seed in 0..12 {
        let run = |search: bool| {
            let (body, _) = body(seed, &shape);
            let cpu = crate::backend::cpu::tuned_searching(&llrm_x86_m16::M16, "386", false, search).expect("a profile");
            let mut phase = RegAlloc::new(None, None, ProfileOrName::Profile(cpu), &*target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16()).expect("a phase");
            let before = trials();
            let out = phase.transform(body).expect("allocated");
            (trials() - before, out)
        };
        let (with, _) = run(true);
        let (without, alone) = run(false);
        searched += with;
        assert_eq!(without, 0, "seed {seed}: allocated again by a profile that does not search");
        assert!(complaints(&alone).is_empty(), "seed {seed}: {:?}", complaints(&alone));
    }
    assert!(searched > 0, "premise: some body had a shape to try");
}

/// The search tried every shape its spills suggested, twice each: up to 12 allocations of one body, and 80% of compiling
/// `d_faces`. Unless it is exhaustive it allocates the shape the spills suggest and the body without splitting: at most 2
/// more, and never a worse output than the first allocation's.
#[test]
fn test_a_search_that_is_not_exhaustive_makes_at_most_two_more_allocations() {
    use crate::backend::allocate::trials;
    let shape = Shape { pool: 12, ops: 11 };
    let mut most_all = 0;
    for seed in 0..40 {
        let run = |exhaustive: bool| {
            let (body, _) = body(seed, &shape);
            let cpu = crate::backend::cpu::tuned_exhaustive(&llrm_x86_m16::M16, "386", false, exhaustive).expect("a profile");
            let mut phase = RegAlloc::new(None, None, ProfileOrName::Profile(cpu), &*target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16()).expect("a phase");
            let before = trials();
            let out = phase.transform(body).expect("allocated");
            (trials() - before, out)
        };
        let (directed, out) = run(false);
        let (all, _) = run(true);
        assert!(directed <= 2, "seed {seed}: {directed} more allocations");
        assert!(complaints(&out).is_empty(), "seed {seed}: {:?}", complaints(&out));
        most_all = most_all.max(all);
    }
    assert!(most_all > 2, "premise: some body has more than two shapes to try (most: {most_all})");
}

/// The walk found a value's intervals by every value live in every block, hashed (the homes of d_faces: 56 values, 240 blocks, 4.4M
/// instructions a call, 616 calls). From where the values occur and the blocks they are live through it finds the same.
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
        let every: Vec<u32> = plain.insns().iter().flat_map(|one| one.defines.iter().chain(&one.uses).copied()).collect::<std::collections::BTreeSet<u32>>().into_iter().collect();
        for step in [1_usize, 2, 3] {
            let values: Vec<u32> = every.iter().copied().enumerate().filter(|(at, _)| at % step == (seed as usize) % step).map(|(_, value)| value).collect();
            let whole = ranges::_ranges_reference(&plain, &index, &|value| values.contains(&value));
            let mut places: IndexMap<u32, Vec<ranges::Occurrence>> = IndexMap::default();
            for &value in &values {
                let mut at: Vec<ranges::Place> = postings.defs(value).iter().chain(postings.uses(value)).map(|(block, position)| (*block as usize, *position as usize)).collect();
                at.sort_unstable();
                at.dedup();
                places.insert(value, at.into_iter().map(|(block, position)| {
                    let one = &plain.blocks[block].insns[position];
                    ((block, position), one.defines.contains(&value), one.uses.contains(&value))
                }).collect());
            }
            let found = ranges::intervals_by_occurrences(&plain, &index, &values, &places);
            assert!(found == whole, "seed {seed}, every {step}th value: the intervals from occurrences differ from the walk");
            compared += whole.len();
        }
    }
    assert!(compared > 0, "premise: some value was live");
}

/// `allocate::live` was built from per-block sorted sets and converted to bit rows for the fixed point:
/// 24% of compiling QCport's `d_faces` (#559). Dense rows all the way give the same sets.
#[test]
fn test_dense_liveness_is_what_the_sorted_sets_gave() {
    for seed in 0..200 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            assert_eq!(crate::backend::allocate::live(&body), crate::backend::allocate::live_reference(&body), "seed {seed}");
            let (entering, leaving) = crate::backend::allocate::live_reference(&body);
            let rows = crate::backend::allocate::live_rows(&body);
            for block in &body.blocks {
                assert_eq!(rows.entering(block.at).collect::<Vec<_>>(), entering[&block.at].iter().copied().collect::<Vec<_>>(), "seed {seed} entering {}", block.at);
                assert_eq!(rows.leaving(block.at).collect::<Vec<_>>(), leaving[&block.at].iter().copied().collect::<Vec<_>>(), "seed {seed} leaving {}", block.at);
            }
        }
    }
}

/// Every question of a body's intervals was worked out afresh: a spill was followed by the allocator's
/// facts of the body it made, the spiller's steps and the class check, each asking the same (58% of the
/// asks of compiling `d_faces`, #559). The same instructions are answered from memory; others are not.
#[test]
fn test_the_intervals_of_the_same_instructions_are_worked_out_once() {
    use crate::analysis::intervals::{intervals, worked};
    let (generated, _) = body(3, &Shape { pool: 8, ops: 10 });
    let before = worked();
    let first = intervals(&generated, None);
    let again = intervals(&generated, None);
    assert_eq!(worked() - before, 1, "the same body was worked out twice");
    assert_eq!(first, again);
    // A body with one instruction made afresh is another question.
    let mut other = generated.clone();
    let block = other.blocks.iter_mut().find(|block| !block.insns.is_empty()).expect("a block with instructions");
    let copy = std::sync::Arc::new((*block.insns[0]).clone());
    block.insns[0] = copy;
    intervals(&other, None);
    assert_eq!(worked() - before, 2, "another body was answered from the first");
}

/// A spill made a body of nearly the same instructions, and every fact of it was worked out afresh (the allocator's
/// 9784 rebuilds of `d_faces`, #944). An answer is the earlier one moved to the new slots, worked out again for the values
/// the changed instructions name: the same intervals and weights.
#[test]
fn test_the_intervals_of_an_edited_body_are_the_ones_worked_out_afresh() {
    use crate::analysis::intervals::{edited, intervals, intervals_afresh};
    use std::sync::Arc;
    let mut made = 0;
    for seed in 0..120 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            intervals(&body, None);
            let mut other = body.clone();
            let (block, at) = {
                let blocks: Vec<usize> = other.blocks.iter().enumerate().filter(|(_, block)| block.insns.len() > 2).map(|(at, _)| at).collect();
                let Some(&block) = blocks.get(seed as usize % blocks.len().max(1)) else { continue };
                (block, 1 + seed as usize % (other.blocks[block].insns.len() - 2))
            };
            // One instruction made afresh, and another of the block's put in again beside it: as a spill's reload would be.
            let copy = Arc::new((*other.blocks[block].insns[at]).clone());
            other.blocks[block].insns[at] = copy;
            let inserted = Arc::new((*other.blocks[block].insns[at - 1]).clone());
            other.blocks[block].insns.insert(at, inserted);
            let before = edited();
            let found = intervals(&other, None);
            made += edited() - before;
            assert_eq!(found, intervals_afresh(&other), "seed {seed}");
        }
    }
    assert!(made > 50, "premise: bodies were answered by editing ({made})");
}

/// `spiller::siblings` built the interference of every value live together (13.8% of compiling
/// `d_faces`, #559) to ask of the pairs among the values a plain move relates. The graph of those
/// values alone has the edges among them that the whole graph has.
#[test]
fn test_interference_among_some_values_is_the_whole_graphs_among_them() {
    use crate::backend::coalesce::{_interference, _interference_among};
    for seed in 0..120 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            let whole = _interference(&body);
            let mut some: std::collections::BTreeSet<u32> = whole.keys().copied().filter(|one| (one + seed as u32) % 3 != 0).collect();
            some.insert(1);
            let among = _interference_among(&body, Some(&some));
            for value in &some {
                let expected: std::collections::BTreeSet<u32> = whole.get(value).map(|near| near.intersection(&some).copied().collect()).unwrap_or_default();
                let found = among.get(value).cloned().unwrap_or_default();
                assert_eq!(found, expected, "seed {seed} value {value}");
            }
            assert!(among.keys().all(|one| some.contains(one)), "seed {seed}: a value outside the set has an entry");
        }
    }
}

/// The class of every value was found with the body numbered, its intervals found and the clobber masks
/// built for the `[word+word]` roles, in bodies with none: 3.8 s of compiling `d_faces` (#559).
#[test]
fn test_a_body_with_no_word_address_pairs_is_not_numbered_to_find_classes() {
    let (generated, _) = body(5, &Shape { pool: 8, ops: 6 });
    let before = crate::analysis::intervals::worked();
    crate::backend::regclass::classes(&generated, &std::collections::BTreeSet::new(), &crate::backend::target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
    assert_eq!(crate::analysis::intervals::worked() - before, 0, "intervals were worked out for a body with no word pairs");
}

/// The interval walk looked each instruction's slot up in a map hashed by its address, built a set per group
/// and shifted its live map on every removal: 7.6 s of compiling `d_faces` (#559). Slots counted in place, a
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
            assert!(_walked(&body, &index, &all).iter().eq(_ranges_reference(&body, &index, &all).iter()), "seed {seed}");
            let some = |value: u32| value % 3 != 0;
            assert!(_walked(&body, &index, &some).iter().eq(_ranges_reference(&body, &index, &some).iter()), "seed {seed} among some");
        }
    }
}

/// Values live into a block come out of its walk in the order they were first read, last read first: two
/// values read and never made in the block. The order of the intervals is part of what is the same.
#[test]
fn test_values_read_and_never_made_in_a_block_come_out_in_the_order_they_were_first_walked() {
    use crate::analysis::intervals::{_ranges_reference, _walked, indexed};
    let read = |at: i64, into: u32, from: u32| Insn::new(at, Some((at, 1)), Some(Semantics { name: Some("mov".to_owned()), dests: vec![held(into)], sources: vec![held(from)], ..Semantics::new(Operation::Move) }), vec![into], vec![from]);
    let blocks = vec![LirBlock::new(1, vec![Arc::new(read(1, 3, 1)), Arc::new(read(2, 4, 2))])];
    let body = LirBody::new("f", 1, blocks, Default::default(), Default::default());
    let index = indexed(&body);
    let walked = _walked(&body, &index, &|_| true);
    assert_eq!(walked.keys().copied().collect::<Vec<_>>(), vec![4, 3, 2, 1]);
    assert!(walked.iter().eq(_ranges_reference(&body, &index, &|_| true).iter()));
}

/// A spilled read's saving was looked up in the profile by form name, four times, for every instruction, at
/// every rebuild of the allocator's facts, and each use asked the instruction's pattern afresh (5.6 s of
/// compiling `d_faces`, #559). The saving is found once per form and the pattern once per instruction: the
/// same weights.
#[test]
fn test_fold_prices_are_what_the_per_instruction_lookup_gave() {
    use crate::analysis::frequency::Frequency;
    use crate::analysis::intervals::intervals;
    use crate::backend::allocate::{_fold_discount, _fold_priced};
    use crate::backend::cpu::ProfileOrName;
    use crate::backend::spiller::folded_source;
    for cpu in ["386", "486", "Core", "P5"] {
        let profile = crate::backend::cpu::profile(ProfileOrName::from(cpu)).expect("a profile");
        for seed in 0..150 {
            let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
            let (plain, _) = body(seed, &shape);
            for body in [in_ssa(&plain), plain] {
                let busy = Frequency::of(&body);
                let live = intervals(&body, None);
                let mut free: crate::support::hash::IndexMap<u32, f64> = Default::default();
                for block in &body.blocks {
                    let each = busy.block(block.at);
                    for one in &block.insns {
                        let discount = _fold_discount(one, profile);
                        if discount == 0.0 {
                            continue;
                        }
                        for value in &one.uses {
                            if folded_source(one, &std::collections::BTreeSet::from([*value])).is_some() {
                                *free.entry(*value).or_insert(0.0) += each * discount;
                            }
                        }
                    }
                }
                let mut expected = live.clone();
                for (value, found) in &free {
                    if let Some(one) = expected.get_mut(value).filter(|one| one.weight != f64::INFINITY) {
                        one.weight = (one.weight - found / (one.size() + crate::analysis::intervals::GRACE) as f64).max(0.0);
                    }
                }
                let priced = _fold_priced(&body, live, profile, &busy);
                assert!(priced.iter().eq(expected.iter()), "seed {seed} on {cpu}");
            }
        }
    }
}

/// The last reads of a body were found from the sets of every block's entry and exit, of which only the
/// exits are read (5.7% of compiling `d_faces`, #559). The exits as rows give the same reads.
#[test]
fn test_the_last_reads_are_what_the_sets_of_every_block_gave() {
    use crate::analysis::intervals::key;
    use std::collections::BTreeSet;
    for seed in 0..150 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            let (_, live_out) = crate::backend::allocate::live_reference(&body);
            let mut expected: BTreeSet<(usize, u32)> = BTreeSet::new();
            for block in &body.blocks {
                let mut alive: BTreeSet<u32> = live_out[&block.at].clone();
                let mut index = block.insns.len() as i64 - 1;
                while index >= 0 {
                    let one = &block.insns[index as usize];
                    let mut first = index as usize;
                    if one.group.is_some() {
                        while first > 0 && block.insns[first - 1].group == one.group {
                            first -= 1;
                        }
                    }
                    let group = &block.insns[first..=index as usize];
                    for item in group {
                        for value in &item.defines {
                            alive.remove(value);
                        }
                    }
                    for item in group {
                        expected.extend(item.uses.iter().filter(|value| !alive.contains(value)).map(|value| (key(item), *value)));
                    }
                    alive.extend(group.iter().flat_map(|item| item.uses.iter().copied()));
                    index = first as i64 - 1;
                }
            }
            assert_eq!(crate::backend::spiller::_final_uses(&body), expected, "seed {seed}");
        }
    }
}

/// Splitting a value asked of the liveness of every value in every block as sets, built afresh for each
/// split and again for the body it made (9.7% of compiling `d_faces`, #559). One value at a time is asked
/// of, and the rows answer it: whether it is live at a block's entry and exit, as the sets said.
#[test]
fn test_one_values_liveness_from_rows_is_what_the_sets_said() {
    use crate::backend::allocate::{LiveAt, live_reference, live_rows, live_rows_by};
    for seed in 0..150 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            let (entering, leaving) = live_reference(&body);
            let rows = live_rows(&body);
            let values: Vec<u32> = body.blocks.iter().flat_map(|block| block.insns.iter().flat_map(|one| one.defines.iter().chain(&one.uses).copied())).chain([u32::MAX, 0]).collect();
            for &value in values.iter().take(40) {
                let only = live_rows_by(&body, |one| one == value);
                for block in &body.blocks {
                    let (live_in, live_out) = (entering[&block.at].contains(&value), leaving[&block.at].contains(&value));
                    assert_eq!((rows.live_in(block.at, value), rows.live_out(block.at, value)), (live_in, live_out), "seed {seed} value {value}");
                    assert_eq!((only.live_in(block.at, value), only.live_out(block.at, value)), (live_in, live_out), "seed {seed} value {value} alone");
                }
            }
            assert!(!rows.live_in(i64::MIN, 1), "a block the body does not hold");
        }
    }
}

/// The next value was found by putting every value the body names in a set (a fifth of a split's carving).
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
            assert_eq!(crate::backend::splitkit::_next_value(&body), seen.last().copied().expect("seeded") + 1, "seed {seed}");
        }
    }
}

/// The classes of every value numbered the body, found every interval and built the clobber masks for the
/// `[word+word]` roles, all of which the allocator's facts had just found for the same body (2.3 s of
/// compiling `d_faces`, #559). Given them, the classes are the same, in the same order, and nothing is worked
/// out again.
#[test]
fn test_classes_given_the_intervals_and_masks_are_the_classes_found_without() {
    use crate::analysis::intervals::{indexed, intervals, worked};
    use crate::backend::allocate::_masks;
    use crate::backend::classes::RegisterClasses;
    use crate::backend::regclass::{Found, classes, classes_given};
    let registers = RegisterClasses::m16();
    for seed in 0..150 {
        let shape = Shape { pool: 7 + (seed % 9) as usize, ops: 6 + (seed % 17) as usize };
        let (plain, _) = body(seed, &shape);
        for body in [in_ssa(&plain), plain] {
            let live = intervals(&body, None);
            let masks = _masks(&body, &indexed(&body), &crate::backend::target::BUILT_IN);
            let before = worked();
            let given = classes_given(&body, &std::collections::BTreeSet::new(), &crate::backend::target::BUILT_IN, &registers, &Found { live: &live, masks: &masks });
            assert_eq!(worked() - before, 0, "seed {seed}: intervals were worked out again");
            let alone = classes(&body, &std::collections::BTreeSet::new(), &crate::backend::target::BUILT_IN, &registers);
            assert!(given.iter().eq(alone.iter()), "seed {seed}");
        }
    }
    // A body with a `[base+index]` access of two words: the roles are chosen from the intervals and masks.
    let cell = Mem { base: Some(Held { value: 1, width: 2 }), index: Some(Held { value: 2, width: 2 }), scale: 1, ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2) };
    let load = Insn::new(3, Some((3, 1)), Some(Semantics { name: Some("mov".to_owned()), dests: vec![held(3)], sources: vec![Loc::Mem(cell)], ..Semantics::new(Operation::Move) }), vec![3], vec![1, 2]);
    let define = |at: i64, value: u32| Insn::new(at, Some((at, 1)), Some(Semantics { name: Some("mov".to_owned()), dests: vec![held(value)], sources: vec![imm(0)], ..Semantics::new(Operation::Move) }), vec![value], vec![]);
    let blocks = vec![LirBlock::new(1, vec![Arc::new(define(1, 1)), Arc::new(define(2, 2)), Arc::new(load)])];
    let pairs = LirBody::new("f", 1, blocks, Default::default(), Default::default());
    let live = intervals(&pairs, None);
    let masks = _masks(&pairs, &indexed(&pairs), &crate::backend::target::BUILT_IN);
    let given = classes_given(&pairs, &std::collections::BTreeSet::new(), &crate::backend::target::BUILT_IN, &registers, &Found { live: &live, masks: &masks });
    let alone = classes(&pairs, &std::collections::BTreeSet::new(), &crate::backend::target::BUILT_IN, &registers);
    assert!(given.contains_key(&1) && given.contains_key(&2), "the pair's values are confined to a base and an index");
    assert!(given.iter().eq(alone.iter()));
}

/// A mask at `slot` over registers, for the `_clobbered` tests.
fn _mask_at(slot: i64, during: &[Register], high: &[Register], before: &[Register]) -> super::allocate::Mask {
    super::allocate::Mask { slot, during: during.iter().copied().collect(), high: high.iter().copied().collect(), before: before.iter().copied().collect() }
}

/// `_clobbered` agrees with the look at every point it replaced, on random points and values.
#[test]
fn test_clobbered_agrees_with_a_look_at_every_point() {
    use super::allocate::{Masks, _clobbered, _clobbered_reference};
    use crate::analysis::intervals::{Interval, Segment};
    let registers = [Register::AX, Register::BX, Register::CX, Register::DX, Register::SI];
    let mut seed = 99_u64;
    let mut next = |modulus: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) % modulus
    };
    for _ in 0..200 {
        let pick = |next: &mut dyn FnMut(u64) -> u64| -> Vec<Register> { registers.iter().copied().filter(|_| next(4) == 0).collect() };
        let list: Vec<_> = (0..next(12)).map(|at| { let during = pick(&mut next); let high = pick(&mut next); let before = pick(&mut next); _mask_at(at as i64 * 4 + next(3) as i64, &during, &high, &before) }).collect();
        let masks = Masks::new(list);
        for _ in 0..30 {
            let mut start = next(20) as i64;
            let segments = (0..1 + next(3)).map(|_| { let end = start + 1 + next(8) as i64; let seg = Segment { start, end }; start = end + next(5) as i64; seg }).collect();
            let one = Interval::new(1, segments);
            for register in registers {
                for width in [1, 2, 4] {
                    assert_eq!(_clobbered(&one, register, &masks, width), _clobbered_reference(&one, register, &masks, width), "{register:?} width {width} over {:?}", one.segments);
                }
            }
        }
    }
}

/// Every query looked at every point: `_clobbered` was 5.8% of compiling `d_faces` (#559). 20,000 points and
/// 20,000 questions took 1.2 s; they are answered by bisection now.
#[test]
fn test_clobbered_does_not_look_at_every_point() {
    use super::allocate::{Masks, _clobbered};
    use crate::analysis::intervals::{Interval, Segment};
    let masks = Masks::new((0..20_000).map(|at| _mask_at(at * 3, &[Register::DX], &[], &[])).collect());
    let started = std::time::Instant::now();
    let mut clobbered = 0;
    for at in 0..20_000 {
        let one = Interval::new(1, vec![Segment { start: at * 3 + 1, end: at * 3 + 2 }]);
        clobbered += usize::from(_clobbered(&one, Register::DX, &masks, 2));
    }
    assert_eq!(clobbered, 0, "a value live between two points is not across either");
    assert!(started.elapsed().as_secs_f64() < 0.2, "{:?} for 20,000 questions", started.elapsed());
}
