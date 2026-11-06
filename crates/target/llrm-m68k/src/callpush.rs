//! Outgoing arguments as pushes: `move.l %d0, -(%sp)` for a store into the
//! frame's call area, and the stack pointer restored after the call.
//!
//! # How it works
//! A call sequence is `ADJCALLSTACKDOWN n`, stores of the arguments to
//! `(k, %sp)` for every `k` from 0 up to `n - 4`, the call and
//! `ADJCALLSTACKUP`. [`plan`] finds the sequences whose stores fill the area
//! with nothing else naming the stack pointer in between. Stores that come
//! from `n - 4` down to 0 are pushed where they stand; in any other order the
//! pushes go right before the call, which needs the stored registers to keep
//! their values until then. Pushing is priced against
//! the stores (`encode` and `timing::cost` on the cpu, the cleanup `adda`
//! included) three ways: no sequence, every sequence, and the sequences that
//! are cheaper alone. The frame is priced with each choice's largest call area
//! left: when no store remains the prologue and epilogue lose their `suba` and
//! `adda`, which is where a pair of two-word calls gains.
//!
//! A pushed register that a scratch-register load in the same block filled,
//! and nothing else reads, is pushed from the load's memory instead
//! (`move.l (4,%a0), -(%sp)`) when that is cheaper than the load and the push;
//! no store or call may sit between the load and the push.
//!
//! DEVIATE: the pin keeps the reserved call frame and stores
//! (`M68kFrameLowering` has no `X86CallFrameOptimization`); gcc pushes. Reason:
//! best of gcc and clang is the goal. Exit: none.
//!
//! # See also
//! `X86CallFrameOptimization::runOnMachineFunction` (`isProfitable`).

use std::collections::{HashMap, HashSet};

use crate::dagisel::hooks::Cpu;
use crate::mfunc::{Inst, Kind, Operand, Reg};
use crate::regalloc::Assignment;
use crate::{REGISTER_INFO, mc, reg};

/// Which instructions of a function are rewritten.
#[derive(Default)]
pub struct Plan {
    /// Argument stores that become pushes.
    pushes: HashSet<usize>,
    /// Argument stores that wait for the call and become pushes there, with
    /// their offsets.
    deferred: HashMap<usize, i64>,
    /// Argument stores whose push reads memory, with the load that read it.
    folded: HashMap<usize, usize>,
    /// Loads that vanish into a push.
    dropped: HashSet<usize>,
    /// `ADJCALLSTACKDOWN`s that vanish.
    downs: HashSet<usize>,
    /// `ADJCALLSTACKUP`s that become `adda.l #bytes, %sp`.
    ups: HashMap<usize, i64>,
}

impl Plan {
    pub fn is_push(
        &self,
        at: usize,
    ) -> bool {
        self.pushes.contains(&at)
    }

    /// The call-area offset of a store whose push goes before the call.
    pub fn deferred_offset(
        &self,
        at: usize,
    ) -> Option<i64> {
        self.deferred.get(&at).copied()
    }

    /// The load whose memory operand the push of store `at` reads.
    pub fn folded_load(
        &self,
        at: usize,
    ) -> Option<usize> {
        self.folded.get(&at).copied()
    }

    /// Whether the load `at` is part of a push.
    pub fn is_dropped(
        &self,
        at: usize,
    ) -> bool {
        self.dropped.contains(&at)
    }

    pub fn is_down(
        &self,
        at: usize,
    ) -> bool {
        self.downs.contains(&at)
    }

    pub fn cleanup(
        &self,
        at: usize,
    ) -> Option<i64> {
        self.ups.get(&at).copied()
    }

    /// Whether the call sequence starting at `down` stays in the frame.
    pub fn keeps_frame(
        &self,
        down: usize,
    ) -> bool {
        !self.downs.contains(&down)
    }
}

/// One convertible sequence: where it is, how big, and the two prices.
struct Sequence {
    down: usize,
    up: usize,
    bytes: i64,
    stores: Vec<usize>,
    offsets: Vec<i64>,
    /// The stores are not in push order: the pushes wait for the call.
    deferred: bool,
    /// Stores that push what a load read, and the loads.
    folds: Vec<(usize, usize)>,
    kept: (usize, u32),
    pushed: (usize, u32),
}

fn physical(
    register: Reg,
    assignment: &Assignment,
) -> llrm_lir::registers::RegId {
    match register {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(p) => p,
    }
}

fn names_sp(
    inst: &Inst,
    assignment: &Assignment,
) -> bool {
    inst.operands
        .iter()
        .any(
            |operand| match operand {
                Operand::Def(r) | Operand::Use(r) => REGISTER_INFO.root(physical(*r, assignment)) == reg::SP,
                _ => false,
            },
        )
}

fn defines(
    inst: &Inst,
    register: llrm_lir::registers::RegId,
    assignment: &Assignment,
) -> bool {
    inst.operands
        .iter()
        .any(
            |operand| matches!(
                operand,
                Operand::Def(r) if REGISTER_INFO.root(physical(*r, assignment)) == REGISTER_INFO.root(register)
            ),
        )
}

/// What a call-area store writes.
#[derive(Clone, Copy)]
enum Source {
    Register(llrm_lir::registers::RegId),
    Immediate(i64),
}

/// The call-area store `inst` is: its offset and what it stores.
fn argument_store(
    inst: &Inst,
    assignment: &Assignment,
) -> Option<(i64, Source)> {
    let (offset, base, source) = match (inst.kind, &inst.operands[..]) {
        (Kind::Op("MOV32pr"), [Operand::Imm(k), Operand::Use(base), Operand::Use(src)]) => {
            (*k, base, Source::Register(physical(*src, assignment)))
        }
        (Kind::Op("MOV32jr"), [Operand::Use(base), Operand::Use(src)]) => {
            (0, base, Source::Register(physical(*src, assignment)))
        }
        (Kind::Op("MOV32pi"), [Operand::Imm(k), Operand::Use(base), Operand::Imm(v)]) => {
            (*k, base, Source::Immediate(*v))
        }
        (Kind::Op("MOV32ji"), [Operand::Use(base), Operand::Imm(v)]) => (0, base, Source::Immediate(*v)),
        _ => return None,
    };
    (REGISTER_INFO.root(physical(*base, assignment)) == reg::SP).then_some((offset, source))
}

/// `move.l value, -(%sp)`: `value` is a register or an immediate.
pub fn push_of(value: &mc::Operand) -> mc::Inst {
    let opcode = match value {
        mc::Operand::Reg(src) if crate::expand::is_address(*src) => "MOV32ea",
        mc::Operand::Reg(_) => "MOV32ed",
        mc::Operand::Imm(v) if crate::dagisel::pea::fits(*v) => {
            return mc::Inst { opcode: crate::dagisel::pea::NAME, operands: vec![value.clone()] };
        }
        _ => "MOV32ei",
    };
    mc::Inst { opcode, operands: vec![mc::Operand::Reg(reg::SP), value.clone()] }
}

/// The push that reads what the load `load` reads: `move.l (4,%a0), %d1` is
/// `move.l (4,%a0), -(%sp)`.
pub fn memory_push(load: &str) -> Option<&'static str> {
    let [dst, src] = load.strip_prefix("MOV32")?.as_bytes() else { return None };
    if !matches!(dst, b'd' | b'a' | b'r') {
        return None;
    }
    Some(match src {
        b'j' => "MOV32ej",
        b'p' => "MOV32ep",
        b'f' => "MOV32ef",
        b'b' => "MOV32eb",
        b'k' => "MOV32ek",
        b'q' => "MOV32eq",
        _ => return None,
    })
}

/// The operands of `inst` as the printer has them, a frame index at
/// displacement 0 from `%sp`: right for a price, since the form fixes the size.
fn priced_operands(
    inst: &Inst,
    assignment: &Assignment,
) -> Option<Vec<mc::Operand>> {
    let mut out = Vec::new();
    let mut it = inst.operands.iter().peekable();
    while let Some(operand) = it.next() {
        match operand {
            Operand::Def(r) | Operand::Use(r) => out.push(mc::Operand::Reg(physical(*r, assignment))),
            Operand::Imm(value) => {
                out.push(mc::Operand::Imm(*value));
                if matches!(it.peek(), Some(Operand::Frame(_))) {
                    it.next();
                    out.push(mc::Operand::Reg(reg::SP));
                }
            }
            Operand::Symbol(name, addend) => out.push(mc::Operand::Symbol(name.clone(), *addend)),
            _ => return None,
        }
    }
    Some(out)
}

/// The registers `inst` reads besides its result.
fn reads(
    inst: &Inst,
    assignment: &Assignment,
) -> Vec<llrm_lir::registers::RegId> {
    inst.operands
        .iter()
        .filter_map(|operand| match operand {
            Operand::Use(r) => Some(REGISTER_INFO.root(physical(*r, assignment))),
            _ => None,
        })
        .collect()
}

/// The load that can be part of the push of store `store` (a register source
/// `src`), and what it costs with the load: when the register is a scratch one
/// only the push uses, the push can read the load's memory itself. `point` is
/// where the push happens: the store, or the call when the pushes wait for it.
fn foldable(
    insts: &[&Inst],
    assignment: &Assignment,
    blocks: &[usize],
    store: usize,
    point: usize,
    src: llrm_lir::registers::RegId,
    stores: &[usize],
) -> Option<usize> {
    let root = REGISTER_INFO.root(src);
    // the call clobbers these: nothing reads the value after it
    if ![reg::D0, reg::D1, reg::A0, reg::A1].iter().any(|scratch| REGISTER_INFO.root(*scratch) == root) {
        return None;
    }
    // the nearest definition in the store's block, with no call between
    let load = (0..store)
        .rev()
        .take_while(|&i| blocks[i] == blocks[store] && !insts[i].call && insts[i].kind != Kind::Op("ADJCALLSTACKUP"))
        .find(|&i| defines(insts[i], src, assignment))?;
    let inst = insts[load];
    let Kind::Op(name) = inst.kind else { return None };
    memory_push(name)?;
    if !inst.may_load || inst.may_store || inst.side_effects {
        return None;
    }
    let address = reads(inst, assignment);
    for i in load + 1..=point {
        if i == store {
            continue;
        }
        let other = insts[i];
        if matches!(other.kind, Kind::Op("ADJCALLSTACKDOWN")) {
            continue;
        }
        let own_store = stores.contains(&i);
        let names_root = other
            .operands
            .iter()
            .any(
                |operand| match operand {
                    Operand::Def(r) | Operand::Use(r) => REGISTER_INFO.root(physical(*r, assignment)) == root,
                    _ => false,
                },
            );
        let defines_address = other
            .operands
            .iter()
            .any(
                |operand| match operand {
                    Operand::Def(r) => address.contains(&REGISTER_INFO.root(physical(*r, assignment))),
                    _ => false,
                },
            );
        // the call the pushes wait for may write memory: they are made first
        let writes = !own_store && !(other.call && i == point) && (other.may_store || other.side_effects || other.call);
        if names_root || defines_address || writes {
            return None;
        }
    }
    // the load goes with its push: a later store of the same register still
    // needs the value (`a + a`: both arguments are the one load)
    for other in &insts[point + 1..] {
        let reads_root = other.operands.iter().any(
            |operand| matches!(operand, Operand::Use(r) if REGISTER_INFO.root(physical(*r, assignment)) == root),
        );
        if reads_root {
            return None;
        }
        if other.call || defines(other, src, assignment) {
            break;
        }
    }
    Some(load)
}

fn cost(
    inst: &mc::Inst,
    cpu: Cpu,
) -> Result<(usize, u32), String> {
    let size = crate::encode::encode(inst)?.bytes.len();
    Ok((size, crate::timing::cost(cpu, &mc::print(inst)?, size as u32, false)?.clocks))
}

fn add(
    a: (usize, u32),
    b: (usize, u32),
) -> (usize, u32) {
    (a.0 + b.0, a.1 + b.1)
}

/// Whether `a` is the cheaper: bytes then clocks at the size level, clocks
/// then bytes otherwise.
fn cheaper(
    a: (usize, u32),
    b: (usize, u32),
    size: bool,
) -> bool {
    if size { a < b } else { (a.1, a.0) < (b.1, b.0) }
}

/// The sequences of `insts` whose stores can be pushes, with their prices.
fn sequences(
    insts: &[&Inst],
    blocks: &[usize],
    assignment: &Assignment,
    cpu: Cpu,
    size: bool,
    short: bool,
) -> Result<(Vec<Sequence>, Vec<i64>), String> {
    let mut found = Vec::new();
    // the call areas of the sequences that cannot be pushed
    let mut others = Vec::new();
    let mut at = 0;
    while at < insts.len() {
        let Kind::Op("ADJCALLSTACKDOWN") = insts[at].kind else {
            at += 1;
            continue;
        };
        let down = at;
        let bytes = match insts[down].operands.first() {
            Some(Operand::Imm(bytes)) => *bytes,
            _ => 0,
        };
        let call = (down + 1..insts.len()).find(|&i| insts[i].call);
        let up = call
            .and_then(|call| (call + 1..insts.len()).find(|&i| matches!(insts[i].kind, Kind::Op("ADJCALLSTACKUP"))));
        // A cleanup belongs to the call before it in its own block, with no
        // other call or call area between: a tail shared by two paths leaves
        // one `up` after two calls, and the one in another block is not this
        // call's. The arguments may be pushed before a diamond the call is
        // after.
        let own_up = call.zip(up).filter(|&(call, up)| {
            blocks[up] == blocks[call]
                && insts[call + 1..up].iter().all(|inst| !inst.call && !matches!(inst.kind, Kind::Op("ADJCALLSTACKDOWN")))
        });
        let Some((call, up)) = own_up else {
            others.push(bytes);
            at += 1;
            continue;
        };
        at = up + 1;
        // each argument store: where, its offset, its source, its price kept
        // and pushed
        let mut parts: Vec<(usize, i64, Source, (usize, u32), (usize, u32))> = Vec::new();
        let mut convertible = bytes > 0 && bytes % 4 == 0;
        for i in down + 1..call {
            if let Some((offset, source)) = argument_store(insts[i], assignment) {
                let sp = mc::Operand::Reg(reg::SP);
                let (value, store_rows) = match source {
                    Source::Register(src) => (mc::Operand::Reg(src), ("MOV32jr", "MOV32pr")),
                    Source::Immediate(v) => (mc::Operand::Imm(v), ("MOV32ji", "MOV32pi")),
                };
                let store = if offset == 0 {
                    mc::Inst { opcode: store_rows.0, operands: vec![sp.clone(), value.clone()] }
                } else {
                    mc::Inst {
                        opcode: store_rows.1,
                        operands: vec![mc::Operand::Imm(offset), sp.clone(), value.clone()],
                    }
                };
                parts.push((i, offset, source, cost(&store, cpu)?, cost(&push_of(&value), cpu)?));
            } else if names_sp(insts[i], assignment) || matches!(insts[i].kind, Kind::Op("ADJCALLSTACKDOWN")) {
                convertible = false;
            }
        }
        let stores: Vec<usize> = parts.iter().map(|part| part.0).collect();
        let offsets: Vec<i64> = parts.iter().map(|part| part.1).collect();
        let wanted: Vec<i64> = (0..bytes / 4).map(|n| bytes - 4 - 4 * n).collect();
        let in_order = offsets == wanted;
        let mut sorted = offsets.clone();
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        let mut priced = ((0usize, 0u32), (0usize, 0u32));
        let mut folds = Vec::new();
        let mut keeps_values = true;
        for &(store, _, source, kept, pushed) in &parts {
            let (mut kept, mut pushed) = (kept, pushed);
            if let Source::Register(src) = source {
                let point = if in_order { store } else { call };
                // a push that reads memory replaces the load and the push
                let memory = foldable(insts, assignment, blocks, store, point, src, &stores).and_then(|load| {
                    let Kind::Op(name) = insts[load].kind else { return None };
                    let mut operands = priced_operands(insts[load], assignment)?;
                    operands[0] = mc::Operand::Reg(reg::SP);
                    let read = cost(&mc::Inst { opcode: memory_push(name)?, operands }, cpu).ok()?;
                    let own = mc::Inst { opcode: name, operands: priced_operands(insts[load], assignment)? };
                    let load_price = cost(&own, cpu).ok()?;
                    Some((load, read, load_price))
                });
                match memory {
                    Some((load, read, load_price)) if cheaper(read, add(load_price, pushed), size) => {
                        folds.push((store, load));
                        kept = add(kept, load_price);
                        pushed = read;
                    }
                    // a push that waits for the call reads its register there
                    _ if !in_order => {
                        keeps_values &= !insts[store + 1..call].iter().any(|inst| defines(inst, src, assignment));
                    }
                    _ => {}
                }
            }
            priced.0 = add(priced.0, kept);
            priced.1 = add(priced.1, pushed);
        }
        if !convertible || sorted != wanted || !keeps_values {
            others.push(bytes);
            continue;
        }
        let cleanup = crate::expand::sp_adjust(bytes, short);
        found.push(Sequence {
            down,
            up,
            bytes,
            stores,
            offsets,
            deferred: !in_order,
            folds,
            kept: priced.0,
            pushed: add(priced.1, cost(&cleanup, cpu)?),
        });
    }
    Ok((found, others))
}

/// The pushes to make in `insts`. `frame_price(n)` is the price of the
/// prologue and epilogue when the largest call area is `n` bytes.
pub fn plan(
    insts: &[&Inst],
    blocks: &[usize],
    assignment: &Assignment,
    cpu: Cpu,
    size: bool,
    short: bool,
    frame_price: impl Fn(i64) -> Result<(usize, u32), String>,
) -> Result<Plan, String> {
    let (found, others) = sequences(insts, blocks, assignment, cpu, size, short)?;
    if found.is_empty() {
        return Ok(Plan::default());
    }
    let widest = |chosen: &[bool]| {
        found
            .iter()
            .zip(chosen)
            .filter(|(_, pushed)| !**pushed)
            .map(|(one, _)| one.bytes)
            .chain(others.iter().copied())
            .max()
            .unwrap_or(0)
    };
    let total = |chosen: &[bool]| -> Result<(usize, u32), String> {
        let mut sum = frame_price(widest(chosen))?;
        for (one, pushed) in found.iter().zip(chosen) {
            sum = add(sum, if *pushed { one.pushed } else { one.kept });
        }
        Ok(sum)
    };
    let none = vec![false; found.len()];
    let all = vec![true; found.len()];
    let alone: Vec<bool> = found.iter().map(|one| cheaper(one.pushed, one.kept, size)).collect();
    let mut best = (none.clone(), total(&none)?);
    for choice in [all, alone] {
        let price = total(&choice)?;
        if cheaper(price, best.1, size) {
            best = (choice, price);
        }
    }
    let mut plan = Plan::default();
    for (one, pushed) in found.iter().zip(&best.0) {
        if *pushed {
            for &(store, load) in &one.folds {
                plan.folded.insert(store, load);
                plan.dropped.insert(load);
            }
            if one.deferred {
                plan.deferred.extend(one.stores.iter().copied().zip(one.offsets.iter().copied()));
            } else {
                plan.pushes.extend(one.stores.iter().copied());
            }
            plan.downs.insert(one.down);
            plan.ups.insert(one.up, one.bytes);
        }
    }
    Ok(plan)
}
