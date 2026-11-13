//! Outgoing arguments as pushes: `move.l %d0, -(%sp)` for a store into the
//! frame's call area, and the stack pointer restored after the call.
//!
//! # How it works
//! A call sequence is `ADJCALLSTACKDOWN n`, stores of the arguments to
//! `(k, %sp)` for every `k` from 0 up to `n - 4`, the call and
//! `ADJCALLSTACKUP`. [`plan`] finds the sequences whose stores fill the area
//! with nothing else naming the stack pointer in between. Stores that come
//! from `n - 4` down to 0 are pushed where they stand; in any other order the
//! pushes go as late as the stored registers allow: before the call, or before
//! the first instruction after the last store that rewrites one of them.
//! Pushing is priced against the stores (`encode` and `timing::cost` on the
//! cpu, the cleanup `adda` included) three ways: no sequence, every sequence,
//! and the sequences that are cheaper alone. The frame is priced with each
//! choice's largest call area left: when no store remains the prologue and
//! epilogue lose their `suba` and `adda`, which is where a pair of two-word
//! calls gains.
//!
//! A pushed register that a scratch-register load in the same block filled,
//! and nothing else reads, is pushed from the load's memory instead
//! (`move.l (4,%a0), -(%sp)`) when that is cheaper than the load and the push;
//! no store or call may sit between the load and the push.
//!
//! A pushed register that an address register plus a constant filled (`move
//! %a3, %d0; addq #1, %d0`), and nothing else reads, is pushed as `pea
//! (1,%a3)`: 4 B for the 8 of the copy, the add and the store, and no codes
//! change, so no later branch may read the ones the add left.
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
use crate::timing::Price;
use crate::{REGISTER_INFO, mc, reg};

/// Which instructions of a function are rewritten.
#[derive(Default)]
pub struct Plan {
    /// Argument stores that become pushes.
    pushes: HashSet<usize>,
    /// Argument stores that wait for the call and become pushes there, with
    /// their offsets.
    deferred: HashMap<usize, i64>,
    /// The instruction each deferred sequence's pushes are made before: the
    /// call, or the first one after the last store that rewrites a pushed
    /// register.
    flush: HashSet<usize>,
    /// Argument stores whose push reads memory, with the load that read it.
    folded: HashMap<usize, usize>,
    /// Argument stores whose push is `pea (k,An)`: the register and `k`.
    addressed: HashMap<usize, (llrm_lir::registers::RegId, i64)>,
    /// Instructions that vanish into a push: loads, and copies and adds.
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

    /// Whether the deferred pushes are made before instruction `at`.
    pub fn flushes(
        &self,
        at: usize,
    ) -> bool {
        self.flush.contains(&at)
    }

    /// The load whose memory operand the push of store `at` reads.
    pub fn folded_load(
        &self,
        at: usize,
    ) -> Option<usize> {
        self.folded.get(&at).copied()
    }

    /// The register and displacement of the `pea` that is the push of store
    /// `at`.
    pub fn address_push(
        &self,
        at: usize,
    ) -> Option<(llrm_lir::registers::RegId, i64)> {
        self.addressed.get(&at).copied()
    }

    /// Whether the instruction `at` is part of a push.
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
    /// The stores are not in push order: the pushes wait.
    deferred: bool,
    /// The instruction the waiting pushes are made before.
    flush: usize,
    /// Stores that push what a load read, and the loads.
    folds: Vec<(usize, usize)>,
    /// Stores that push an address register plus a constant: the instructions
    /// that made the value, the register and the constant.
    addresses: Vec<(usize, Vec<usize>, llrm_lir::registers::RegId, i64)>,
    kept: (usize, u32),
    pushed: (usize, u32),
    /// The cleanup `pushed` includes: owed calls merge theirs.
    cleanup: (usize, u32),
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

pub(crate) fn names_sp(
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
enum Source<'a> {
    Register(llrm_lir::registers::RegId),
    Immediate(i64),
    /// The address of a symbol, plus an addend.
    Symbol(&'a str, i64),
}

/// The call-area store `inst` is: its offset and what it stores.
fn argument_store<'a>(
    inst: &'a Inst,
    assignment: &Assignment,
) -> Option<(i64, Source<'a>)> {
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
        (Kind::Op("MOV32pi"), [Operand::Imm(k), Operand::Use(base), Operand::Symbol(name, addend)]) => {
            (*k, base, Source::Symbol(name, *addend))
        }
        (Kind::Op("MOV32ji"), [Operand::Use(base), Operand::Symbol(name, addend)]) => {
            (0, base, Source::Symbol(name, *addend))
        }
        // a store of zero selects `clr.l`
        (Kind::Op("CLR32p"), [Operand::Imm(k), Operand::Use(base)]) => (*k, base, Source::Immediate(0)),
        (Kind::Op("CLR32j"), [Operand::Use(base)]) => (0, base, Source::Immediate(0)),
        _ => return None,
    };
    (REGISTER_INFO.root(physical(*base, assignment)) == reg::SP).then_some((offset, source))
}

/// [`push_of`], where a function built for size pushes zero as `clr.l
/// -(%sp)`: 2 B against `pea (0).w`'s 4, and 6 clk slower on the 68000.
pub fn push_of_sized(
    value: &mc::Operand,
    size: bool,
) -> mc::Inst {
    if size && matches!(value, mc::Operand::Imm(0)) {
        return mc::Inst { opcode: "CLR32e", operands: vec![mc::Operand::Reg(reg::SP)] };
    }
    push_of(value)
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

/// `pea (offset,base)`.
pub fn address_push(
    base: llrm_lir::registers::RegId,
    offset: i64,
) -> mc::Inst {
    mc::Inst {
        opcode: crate::dagisel::pea::DISPLACED,
        operands: vec![mc::Operand::Imm(offset), mc::Operand::Reg(base)],
    }
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
/// where the push happens: the store, or the last store when the pushes wait
/// for it.
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
    // until another instruction sets them, the codes are the load's
    let mut load_codes = true;
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
        // a call or write before the push point may change what it reads
        let writes = !own_store && (other.may_store || other.side_effects || other.call);
        // the load's codes go with it: a reader of them (`sne`) would see older
        // ones
        let reads_codes = load_codes && other.reads_flags() && !other.call;
        if names_root || defines_address || writes || reads_codes {
            return None;
        }
        if other.sets_flags() || other.call {
            load_codes = false;
        }
    }
    // the load goes with its push: a later store of the same register still
    // needs the value (`a + a`: both arguments are the one load)
    if !unread_after(insts, assignment, point, root, src) {
        return None;
    }
    Some(load)
}

/// Whether nothing reads `root` after instruction `point` before it is
/// rewritten or a call clobbers it.
fn unread_after(
    insts: &[&Inst],
    assignment: &Assignment,
    point: usize,
    root: llrm_lir::registers::RegId,
    src: llrm_lir::registers::RegId,
) -> bool {
    for other in &insts[point + 1..] {
        let reads_root = other
            .operands
            .iter()
            .any(
                |operand| matches!(
                    operand,
                    Operand::Use(r) if REGISTER_INFO.root(physical(*r, assignment)) == root
                ),
            );
        if reads_root {
            return false;
        }
        if other.call || defines(other, src, assignment) {
            break;
        }
    }
    true
}

/// The address register and constant that make up the value of store `store`
/// (a register source `src`), and the instructions that made it: `move %a3,
/// %d0; addq #1, %d0` is `1(%a3)`, and the add alone when it reads the address
/// register itself. The push can wait for `call`, so the address register
/// must keep its value until then; nothing may read the register the value was
/// made in after the store, and the codes the copy, the add and the store left
/// must be unread, as the push leaves them alone.
fn address_of(
    insts: &[&Inst],
    assignment: &Assignment,
    blocks: &[usize],
    store: usize,
    call: usize,
    src: llrm_lir::registers::RegId,
) -> Option<(Vec<usize>, llrm_lir::registers::RegId, i64)> {
    let root = REGISTER_INFO.root(src);
    if ![reg::D0, reg::D1, reg::A0, reg::A1].iter().any(|scratch| REGISTER_INFO.root(*scratch) == root) {
        return None;
    }
    let nearest = |before: usize| {
        (0..before)
            .rev()
            .take_while(|&i| blocks[i] == blocks[store] && !insts[i].call && insts[i].kind != Kind::Op("ADJCALLSTACKUP"))
            .find(|&i| defines(insts[i], src, assignment))
    };
    let add = nearest(store)?;
    let Kind::Op(name) = insts[add].kind else { return None };
    let sign = match name {
        "ADDQ32di" | "ADD32di" => 1,
        "SUBQ32di" | "SUB32di" => -1,
        _ => return None,
    };
    let [Operand::Def(_), Operand::Use(input), Operand::Imm(value)] = insts[add].operands[..] else { return None };
    let input = physical(input, assignment);
    let usable = |register: llrm_lir::registers::RegId| {
        crate::expand::is_address(REGISTER_INFO.root(register)) && REGISTER_INFO.root(register) != reg::SP
    };
    let (producers, base) = if REGISTER_INFO.root(input) == root {
        let copy = nearest(add)?;
        let (Kind::Copy, [Operand::Def(_), Operand::Use(from)]) = (insts[copy].kind, &insts[copy].operands[..]) else {
            return None;
        };
        let from = physical(*from, assignment);
        if !usable(from) {
            return None;
        }
        (vec![copy, add], from)
    } else if usable(input) {
        (vec![add], input)
    } else {
        return None;
    };
    let value = sign * value;
    if i16::try_from(value).is_err() {
        return None;
    }
    let first = producers[0];
    for i in first + 1..call {
        if i == store || producers.contains(&i) {
            continue;
        }
        let other = insts[i];
        // the address register must hold the same value at the push
        if defines(other, base, assignment) {
            return None;
        }
        if i < store {
            let names_root = other.operands.iter().any(|operand| match operand {
                Operand::Def(r) | Operand::Use(r) => REGISTER_INFO.root(physical(*r, assignment)) == root,
                _ => false,
            });
            if names_root || (other.reads_flags() && !other.call) {
                return None;
            }
        }
    }
    // the codes after the store: the first instruction that touches them
    // must set them
    for other in &insts[store + 1..] {
        if other.call {
            break;
        }
        // read first: `addx` reads the codes it sets
        if other.reads_flags() || other.terminator {
            return None;
        }
        if other.sets_flags() {
            break;
        }
    }
    unread_after(insts, assignment, store, root, src).then_some((producers, base, value))
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

/// Whether `a` is the cheaper at `price`.
fn cheaper(
    a: (usize, u32),
    b: (usize, u32),
    price: Price,
    cpu: Cpu,
) -> bool {
    price.beats(cpu, a, b)
}

/// The sequences of `insts` whose stores can be pushes, with their prices.
fn sequences(
    insts: &[&Inst],
    blocks: &[usize],
    assignment: &Assignment,
    cpu: Cpu,
    price: Price,
    short: bool,
) -> Result<(Vec<Sequence>, Vec<i64>), String> {
    let size = price == Price::Size;
    let mut found = Vec::new();
    // the call areas of the sequences that cannot be pushed
    let mut others = Vec::new();
    let mut at = 0;
    let mut claimed = Vec::new();
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
        let own_up = call
            .zip(up)
            .filter(
                |&(call, up)| blocks[up] == blocks[call]
                    && insts[call + 1..up]
                        .iter()
                        .all(|inst| !inst.call && !matches!(inst.kind, Kind::Op("ADJCALLSTACKDOWN"))),
            );
        let Some((call, up)) = own_up else {
            others.push(bytes);
            at += 1;
            continue;
        };
        at = up + 1;
        claimed.push(up);
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
                    Source::Symbol(name, addend) => {
                        (mc::Operand::Symbol(name.to_owned(), addend), ("MOV32ji", "MOV32pi"))
                    }
                };
                let store = if matches!(source, Source::Immediate(0)) {
                    // the `clr.l` the selector chose, priced as it is
                    if offset == 0 {
                        mc::Inst { opcode: "CLR32j", operands: vec![sp.clone()] }
                    } else {
                        mc::Inst { opcode: "CLR32p", operands: vec![mc::Operand::Imm(offset), sp.clone()] }
                    }
                } else if offset == 0 {
                    mc::Inst { opcode: store_rows.0, operands: vec![sp.clone(), value.clone()] }
                } else {
                    mc::Inst {
                        opcode: store_rows.1,
                        operands: vec![mc::Operand::Imm(offset), sp.clone(), value.clone()],
                    }
                };
                parts.push((i, offset, source, cost(&store, cpu)?, cost(&push_of_sized(&value, size), cpu)?));
            } else if names_sp(insts[i], assignment) || matches!(insts[i].kind, Kind::Op("ADJCALLSTACKDOWN")) {
                convertible = false;
            }
        }
        // stores that push `pea (k,An)`: the value never exists in a register,
        // so the order of the pushes depends on the address register alone
        let mut addressed: HashMap<usize, (Vec<usize>, llrm_lir::registers::RegId, i64, (usize, u32), (usize, u32))> =
            HashMap::new();
        if convertible {
            for &(store, _, source, _, pushed) in &parts {
                let Source::Register(src) = source else { continue };
                let Some((producers, base, offset)) = address_of(insts, assignment, blocks, store, call, src) else {
                    continue;
                };
                let made = producers.iter().try_fold((0, 0), |sum, &i| {
                    let operands = priced_operands(insts[i], assignment)?;
                    let opcode = match insts[i].kind {
                        Kind::Copy => "MOV32rr",
                        Kind::Op(name) => name,
                    };
                    cost(&mc::Inst { opcode, operands }, cpu).ok().map(|one| add(sum, one))
                });
                if let (Some(made), Ok(pea)) = (made, cost(&address_push(base, offset), cpu))
                    && cheaper(pea, add(made, pushed), price, cpu)
                {
                    addressed.insert(store, (producers, base, offset, made, pea));
                }
            }
        }
        let stores: Vec<usize> = parts.iter().map(|part| part.0).collect();
        let offsets: Vec<i64> = parts.iter().map(|part| part.1).collect();
        let wanted: Vec<i64> = (0..bytes / 4).map(|n| bytes - 4 - 4 * n).collect();
        let in_order = offsets == wanted;
        let mut sorted = offsets.clone();
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        let last_store = stores.iter().copied().max().unwrap_or(call);
        // A deferred push is made as late as the pushed registers allow, so
        // the stack-pointer-relative instructions before it keep their offsets:
        // before the call, or before the first instruction after the last
        // store that rewrites one of them.
        let mut flush = call;
        let mut keeps_values = true;
        if !in_order {
            for &(store, _, source, _, _) in &parts {
                let Source::Register(src) = source else { continue };
                if addressed.contains_key(&store) {
                    continue;
                }
                match (store + 1..call).find(|&i| defines(insts[i], src, assignment)) {
                    Some(at) if at < last_store => keeps_values = false,
                    Some(at) => flush = flush.min(at),
                    None => {}
                }
            }
        }
        // a push sets the codes: made between an instruction that set them and
        // the one that reads them, it would change what the reader sees
        if !in_order && flush < call && codes_live_across(insts, blocks, flush) {
            keeps_values = false;
        }
        let mut priced = ((0usize, 0u32), (0usize, 0u32));
        let mut folds = Vec::new();
        let mut addresses = Vec::new();
        for &(store, _, source, kept, pushed) in &parts {
            let (mut kept, mut pushed) = (kept, pushed);
            if let Some((producers, base, offset, made, pea)) = addressed.remove(&store) {
                addresses.push((store, producers, base, offset));
                kept = add(kept, made);
                pushed = pea;
            } else if let Source::Register(src) = source {
                let point = if in_order { store } else { flush - 1 };
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
                if let Some((load, read, load_price)) = memory
                    && cheaper(read, add(load_price, pushed), price, cpu)
                {
                    folds.push((store, load));
                    kept = add(kept, load_price);
                    pushed = read;
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
            flush,
            folds,
            addresses,
            kept: priced.0,
            pushed: add(priced.1, cost(&cleanup, cpu)?),
            cleanup: cost(&cleanup, cpu)?,
        });
    }
    // A call area made before a diamond can serve a call on each path: the
    // second call's cleanup has no call area of its own to claim it, and its
    // arguments are stored into the area the first call's sequence would have
    // pushed away. That area stays. The call may be laid out before the area's
    // `ADJCALLSTACKDOWN` (a rotated loop), so the order of the two says nothing.
    for (up, _) in insts
        .iter()
        .enumerate()
        .filter(|(i, inst)| matches!(inst.kind, Kind::Op("ADJCALLSTACKUP")) && !claimed.contains(i))
    {
        let Some(Operand::Imm(bytes)) = insts[up].operands.first() else { continue };
        if !insts[..up].iter().any(|inst| inst.call) {
            continue;
        }
        found.retain(|one: &Sequence| {
            let shared = one.bytes == *bytes && blocks[one.down] != blocks[one.up];
            if shared {
                others.push(one.bytes);
            }
            !shared
        });
    }
    Ok((found, others))
}

/// Whether the codes an instruction before `at` set, in its block, are read by
/// the first instruction at or after `at` that touches them.
fn codes_live_across(
    insts: &[&Inst],
    blocks: &[usize],
    at: usize,
) -> bool {
    let set_before = (0..at)
        .rev()
        .take_while(|&i| blocks[i] == blocks[at])
        .find(|&i| insts[i].sets_flags() || insts[i].call)
        .is_some_and(|i| insts[i].sets_flags() && !insts[i].call);
    set_before
        && (at..insts.len())
            .take_while(|&i| blocks[i] == blocks[at])
            .find(|&i| insts[i].reads_flags() || insts[i].sets_flags() || insts[i].call)
            .is_some_and(|i| insts[i].reads_flags() && !insts[i].call)
}

/// The pushes to make in `insts`. `frame_price(n)` is the price of the
/// prologue and epilogue when the largest call area is `n` bytes.
pub fn plan(
    insts: &[&Inst],
    blocks: &[usize],
    assignment: &Assignment,
    cpu: Cpu,
    price: Price,
    short: bool,
    frame_price: impl Fn(i64) -> Result<(usize, u32), String>,
) -> Result<Plan, String> {
    let (found, others) = sequences(insts, blocks, assignment, cpu, price, short)?;
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
    // Whether the cleanup of pushed sequence `earlier` is still owed when `later` pushes: nothing between them
    // reads the stack pointer as the frame has it (`expand`'s `owed`), so one `adda` pays for both.
    let owed_across = |earlier: &Sequence, later: &Sequence| -> bool {
        blocks[earlier.up] == blocks[later.down]
            && insts[earlier.up + 1..later.down].iter().all(|inst| {
                !(inst.terminator
                    || inst.barrier
                    || matches!(inst.kind, Kind::Op("RET" | "TCRETURNq" | "UNREACHABLE" | "ADJCALLSTACKDOWN"))
                    || names_sp(inst, assignment))
            })
    };
    let total = |chosen: &[bool]| -> Result<(usize, u32), String> {
        let mut sum = frame_price(widest(chosen))?;
        let mut run: Option<(&Sequence, i64)> = None;
        for (one, pushed) in found.iter().zip(chosen) {
            sum = add(sum, if *pushed { one.pushed } else { one.kept });
            if !*pushed {
                run = None;
                continue;
            }
            // a cleanup merged into the run's: the run's one `adda` is of the bytes together
            if let Some((last, bytes)) = run.filter(|(last, _)| owed_across(last, one)) {
                let merged = cost(&crate::expand::sp_adjust(bytes + one.bytes, short), cpu)?;
                let alone = cost(&crate::expand::sp_adjust(bytes, short), cpu)?;
                sum = add(sum, merged);
                sum = (sum.0.saturating_sub(alone.0 + one.cleanup.0), sum.1.saturating_sub(alone.1 + one.cleanup.1));
                run = Some((one, bytes + one.bytes));
                let _ = last;
            } else {
                run = Some((one, one.bytes));
            }
        }
        Ok(sum)
    };
    let none = vec![false; found.len()];
    let all = vec![true; found.len()];
    let alone: Vec<bool> = found.iter().map(|one| cheaper(one.pushed, one.kept, price, cpu)).collect();
    let mut best = (none.clone(), total(&none)?);
    for choice in [all, alone] {
        let sum = total(&choice)?;
        if cheaper(sum, best.1, price, cpu) {
            best = (choice, sum);
        }
    }
    let mut plan = Plan::default();
    for (one, pushed) in found.iter().zip(&best.0) {
        if *pushed {
            for &(store, load) in &one.folds {
                plan.folded.insert(store, load);
                plan.dropped.insert(load);
            }
            for (store, producers, base, offset) in &one.addresses {
                plan.addressed.insert(*store, (*base, *offset));
                plan.dropped.extend(producers.iter().copied());
            }
            if one.deferred {
                plan.deferred.extend(one.stores.iter().copied().zip(one.offsets.iter().copied()));
                plan.flush.insert(one.flush);
            } else {
                plan.pushes.extend(one.stores.iter().copied());
            }
            plan.downs.insert(one.down);
            plan.ups.insert(one.up, one.bytes);
        }
    }
    Ok(plan)
}
