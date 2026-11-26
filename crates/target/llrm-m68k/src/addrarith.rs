//! A pointer moved by a constant is added to in an address register: `lea`,
//! then `addq` or `adda` where that costs less on the CPU, in place of `move
//! %a0, %d2; addq #6, %d2; move %d2, %a0`. The opposite round trip, a data
//! value added to through an address register, is [`refine_lea_round_trips`].
//!
//! # How it works
//! The pin's add of an immediate takes and gives a data register, so a pointer
//! that loads from its base goes through two copies around it. [`address_adds`]
//! finds an add whose input is a copy of an address register (used only by the
//! add) and whose result reaches nothing but address registers, through copies
//! and phis, and makes it `lea (d16,An)` for a displacement that fits 16 bits,
//! `adda` otherwise. Before allocation the form must not depend on the CPU:
//! `addq` and `adda` take their result in the base's register and `lea` does
//! not, so choosing them early moved the allocation (the 040 seeds s6, s242,
//! s256, s277 grew in bytes and clocks). [`refine_address_adds`] runs after
//! allocation and turns a `lea` whose result is its base into `addq`/`subq`
//! for 1 to 8 or `adda`, whichever the timing tables price lower; on a smaller
//! size they are ordered by bytes first. None of the three change the codes,
//! so the add must not be one whose codes a branch reads.
//!
//! The copy the add reads may be read by addresses besides the add (a byte
//! load that folded the step as a displacement, `move.b (1,%a2)`): those read
//! the base, and a copy after the add whose users are displaced addresses in
//! the block reads the add's result with the displacement lowered by the step,
//! so `*++p` is `addq.l #1, %a0; move.b (%a0)` without the old pointer kept
//! live beside the new one (QB `kw_scan`: two copies and a register a trip).
//!
//! A second, wide run ([`address_adds`] with `wide`,
//! `Options::address_data_uses`) also takes a result that an either-class
//! operand (a store's source) reads, that a compare reads where the value is
//! also loaded through, or that is returned in an address register; the
//! function is built both ways and the cheaper kept ([`crate::trial`]), since
//! one more address register live can cost a spill.
//!
//! DEVIATE: the pin keeps the add in a data register and leaves the class to
//! its selector, which narrows a value's class to its uses and copies for the
//! use that needs the other (`constrainRegClass`); the allocator splits live
//! ranges, never classes. Reason: gcc chooses the class of a pointer family by
//! the cost of its uses (`regclass`), and best of gcc and clang is the goal.
//! Owner m68k. Exit: none.
//!
//! # See also
//! `M68kInstrInfo::copyPhysReg`, which is what the copies become; gcc's
//! `addsi3` (`m68k.md`) with its `a` constraint, which adds in the address
//! register.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};
use llrm_support::hash::HashSet;

use crate::REGISTER_INFO;
use crate::dagisel::hooks::Cpu;
use crate::dagisel::{cmpa, indexed, local};
use crate::expand::is_address;
use crate::mc;
use crate::mfunc::{Class, MFunction, op_inst, operand_class};
use crate::reg;
use crate::regalloc::Assignment;

/// A use of a virtual register: an instruction operand, or a phi's incoming.
enum Use {
    /// A copy into this register.
    Copy(u32),
    Phi(u32),
    /// A copy into the input `x` of a pointer step (`x = copy y; d = x + k`),
    /// with its result `d`: the copy costs `y` nothing in a data register if
    /// `d` reaches addresses. Without it the steps of one pointer, each a data
    /// copy of `y` to the others, kept each other in data registers (gcc
    /// `regclass` takes the class of a pointer family by its uses' costs).
    Step(u32, u32),
    /// An operand the instruction's description gives an address register.
    Address,
    /// An operand of either class (a store's source, a compare's other side):
    /// an address register fills it with no copy.
    Either,
    Other,
}

/// What operand `index` of `name` asks of the register it reads.
fn operand_use(
    name: &str,
    operands: &[Operand],
    index: usize,
    wide: bool,
) -> Use {
    match operand_class(name, index) {
        Some(Class::Ar32) => Use::Address,
        _ if indexed::is_base_operand(name, index, operands.len()) => Use::Address,
        Some(Class::Xr32) if wide => Use::Either,
        _ => Use::Other,
    }
}

/// Every use of every virtual register, in one pass over the function. A
/// rewrite only removes the use of the copy it deletes and defines a register
/// no one else uses, so the table stays right for the registers it is asked
/// about.
fn uses_by_register(
    function: &MFunction,
    compare_in_address: bool,
    wide: bool,
) -> Vec<Vec<Use>> {
    let mut all: Vec<Vec<Use>> = (0..function.classes.len()).map(|_| Vec::new()).collect();
    let mut deferred: Vec<(usize, usize, Vec<u32>)> = Vec::new();
    let mut copy_of: Vec<Option<u32>> = vec![None; function.classes.len()];
    let mut steps: llrm_support::hash::HashMap<u32, u32> = Default::default();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        if let (
            Kind::Op("ADD32di" | "ADDQ32di" | "SUB32di" | "SUBQ32di"),
            [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(x)), Operand::Imm(_)],
        ) = (&inst.kind, &inst.operands[..])
        {
            steps.insert(*x, *d);
        }
    }
    for block in &function.blocks {
        for phi in &block.phis {
            for (_, incoming) in &phi.incoming {
                if let Reg::Virt(v) = incoming {
                    all[*v as usize].push(match phi.result {
                        Reg::Virt(result) => Use::Phi(result),
                        Reg::Phys(_) => Use::Other,
                    });
                }
            }
        }
        for inst in &block.insts {
            if let (Kind::Copy, [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(c))]) =
                (&inst.kind, &inst.operands[..])
            {
                copy_of[*d as usize] = Some(*c);
            }
            for (index, operand) in inst.operands.iter().enumerate() {
                if let Operand::Use(Reg::Virt(v)) = operand {
                    let slot = all[*v as usize].len();
                    all[*v as usize].push(match (&inst.kind, &inst.operands[..]) {
                        (Kind::Copy, [Operand::Def(Reg::Virt(dst)), _]) if let Some(result) = steps.get(dst) => {
                            Use::Step(*dst, *result)
                        }
                        (Kind::Copy, [Operand::Def(Reg::Virt(dst)), _]) => Use::Copy(*dst),
                        // a pointer returned or passed in an address register
                        (Kind::Copy, [Operand::Def(Reg::Phys(to)), _]) if wide && is_address(*to) => Use::Address,
                        (Kind::Op(name), operands) => match operand_use(name, operands, index, wide) {
                            // a compare's destination: `cmpa.l` follows
                            // allocation
                            // (`addrcmp::compare_in_address_register`); it
                            // counts as an address use when the compare's other
                            // register is a pointer, settled below once every
                            // use is known
                            Use::Other if compare_in_address && cmpa::dest_of(name) == Some(index) => {
                                let others: Vec<u32> = operands
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(at, o)| match o {
                                        Operand::Use(Reg::Virt(w)) if at != index => Some(*w),
                                        _ => None,
                                    })
                                    .collect();
                                deferred.push((*v as usize, slot, others));
                                Use::Other
                            }
                            asked => asked,
                        },
                        _ => Use::Other,
                    });
                }
            }
        }
    }
    // a compare of a value with a pointer compares pointers: `cmpa.l`. In the
    // wide run a pointer compared with anything takes either class; a counter,
    // which reaches no address, does not
    let loaded_through = if wide && !deferred.is_empty() { flows_to_addresses(&all) } else { Vec::new() };
    for (v, slot, others) in deferred {
        if others.iter().any(|w| pointer_like(&all, &copy_of, function, *w, 3)) {
            all[v][slot] = Use::Address;
        } else if loaded_through.get(v) == Some(&true) {
            all[v][slot] = Use::Either;
        }
    }
    all
}

/// For each register, whether copies, phis and pointer steps take its value to
/// an address operand.
fn flows_to_addresses(uses: &[Vec<Use>]) -> Vec<bool> {
    let mut from: Vec<Vec<usize>> = vec![Vec::new(); uses.len()];
    let mut flows = vec![false; uses.len()];
    let mut work = Vec::new();
    for (v, ones) in uses.iter().enumerate() {
        for one in ones {
            match one {
                Use::Copy(to) | Use::Phi(to) | Use::Step(to, _) => from[*to as usize].push(v),
                Use::Address if !flows[v] => {
                    flows[v] = true;
                    work.push(v);
                }
                _ => {}
            }
        }
    }
    while let Some(to) = work.pop() {
        for &v in &from[to] {
            if !flows[v] {
                flows[v] = true;
                work.push(v);
            }
        }
    }
    flows
}

/// Whether `v` is a pointer: it is dereferenced, or copied to or from a
/// register that is.
fn pointer_like(
    uses: &[Vec<Use>],
    copy_of: &[Option<u32>],
    function: &MFunction,
    v: u32,
    depth: u32,
) -> bool {
    let through_uses = uses[v as usize]
        .iter()
        .any(
            |one| match one {
                Use::Address => true,
                Use::Copy(to) => {
                    function.classes[*to as usize] == Class::Ar32
                        || (depth > 0 && pointer_like(uses, copy_of, function, *to, depth - 1))
                }
                _ => false,
            },
        );
    through_uses
        || copy_of[v as usize].is_some_and(|from| {
            function.classes[from as usize] == Class::Ar32
                || (depth > 0 && pointer_like(uses, copy_of, function, from, depth - 1))
        })
}

/// Whether everything `v` reaches, through copies and phis, ends in an address
/// register; a copy into `skip` is the caller's own and does not count.
/// Marks a walk that crossed a [`Use::Step`].
const STEPPED: u32 = u32::MAX;

/// [`reaches_addresses_only`], and where it crossed a step, with an address
/// use somewhere on it: steps of a counter that only feed each other reach
/// nothing.
fn reaches_addresses(
    function: &MFunction,
    uses: &[Vec<Use>],
    v: u32,
    skip: Option<u32>,
) -> bool {
    let mut seen = HashSet::default();
    reaches_addresses_only(function, uses, v, skip, &mut seen)
        && (!seen.contains(&STEPPED)
            || seen.iter().any(|one| {
                uses.get(*one as usize).is_some_and(|own| {
                    own.iter().any(|u| match u {
                        Use::Address | Use::Either => true,
                        Use::Copy(to) | Use::Phi(to) => function.classes[*to as usize] == Class::Ar32,
                        _ => false,
                    })
                })
            }))
}

fn reaches_addresses_only(
    function: &MFunction,
    uses: &[Vec<Use>],
    v: u32,
    skip: Option<u32>,
    seen: &mut HashSet<u32>,
) -> bool {
    if !seen.insert(v) {
        return true;
    }
    let own = &uses[v as usize];
    !own.is_empty()
        && own.iter().all(|one| match one {
            Use::Copy(to) if Some(*to) == skip => true,
            Use::Copy(to) | Use::Phi(to) => match function.classes[*to as usize] {
                Class::Ar32 => true,
                Class::Xr32 => reaches_addresses_only(function, uses, *to, skip, seen),
                _ => false,
            },
            Use::Address | Use::Either => true,
            // the step's own result decides; the walk is marked so that the
            // caller asks for one real address use among everything it crossed
            Use::Step(_, result) => {
                seen.insert(STEPPED);
                reaches_addresses_only(function, uses, *result, skip, seen)
            }
            Use::Other => false,
        })
}

/// The instruction that adds `value` to an address register by `name`'s form,
/// as machine operands `[result, base, ...]`.
struct Candidate {
    opcode: &'static str,
    /// The operands for a result `a` and a base `b`.
    operands: fn(Reg, Reg, i64) -> Vec<Operand>,
    value: i64,
}

fn tied(
    result: Reg,
    base: Reg,
    value: i64,
) -> Vec<Operand> {
    vec![Operand::Def(result), Operand::Use(base), Operand::Imm(value)]
}

fn displaced(
    result: Reg,
    base: Reg,
    value: i64,
) -> Vec<Operand> {
    vec![Operand::Def(result), Operand::Imm(value), Operand::Use(base)]
}

fn candidates(value: i64) -> Vec<Candidate> {
    let mut all = Vec::new();
    if (1..=8).contains(&value) {
        all.push(Candidate { opcode: "ADDQ32ai", operands: tied, value });
    } else if (-8..=-1).contains(&value) {
        all.push(Candidate { opcode: "SUBQ32ai", operands: tied, value: -value });
    }
    if (-32768..=32767).contains(&value) {
        all.push(Candidate { opcode: "LEA32p", operands: displaced, value });
    }
    all.push(Candidate { opcode: "ADD32ai", operands: tied, value });
    all
}

/// The clocks and bytes of `one` on `cpu`, with a fixed pair of registers.
fn price(
    one: &Candidate,
    cpu: Cpu,
) -> Option<(u32, u32)> {
    let (a, b) = (llrm_lir::registers::RegId::from(reg::A1), llrm_lir::registers::RegId::from(reg::A0));
    let operands = (one.operands)(Reg::Phys(a), Reg::Phys(b), one.value)
        .into_iter()
        .map(|operand| match operand {
            Operand::Def(Reg::Phys(r)) | Operand::Use(Reg::Phys(r)) => mc::Operand::Reg(r),
            Operand::Imm(value) => mc::Operand::Imm(value),
            _ => unreachable!("a candidate has registers and an immediate"),
        })
        .collect::<Vec<_>>();
    // a tied form names its register once as a result and once as the base: the
    // same register
    let operands = if one.opcode == "LEA32p" {
        operands
    } else {
        vec![operands[0].clone(), operands[0].clone(), operands[2].clone()]
    };
    let inst = mc::Inst { opcode: one.opcode, operands };
    let bytes = crate::encode::encode(&inst).ok()?.bytes.len() as u32;
    let clocks = crate::timing::cost(cpu, &mc::print(&inst).ok()?, bytes, false).ok()?.clocks;
    Some((clocks, bytes))
}

fn cheapest(
    value: i64,
    cpu: Cpu,
    size: bool,
) -> Option<Candidate> {
    candidates(value)
        .into_iter()
        .filter_map(|one| price(&one, cpu).map(|(clocks, bytes)| (crate::timing::order(cpu, size, clocks, bytes), one)))
        .min_by_key(|(key, _)| *key)
        .map(|(_, one)| one)
}

/// The base of the add at `at` that reads `x`: `x` is a copy of an address
/// register (or of one of either class whose other uses are all addresses
/// too), read by the add alone. The copy's place and its source.
fn address_copy(
    function: &MFunction,
    uses: &[Vec<Use>],
    b: usize,
    at: usize,
    x: u32,
    wide: bool,
    size: bool,
) -> Option<(usize, Reg)> {
    let insts = &function.blocks[b].insts;
    let copy = insts[..at]
        .iter()
        .rposition(
            |one| matches!(
                &one.operands[..],
                [Operand::Def(Reg::Virt(def)), _] if *def == x
            ),
        )?;
    let Kind::Copy = insts[copy].kind else { return None };
    let Operand::Use(base @ Reg::Virt(y)) = insts[copy].operands[1] else { return None };
    // read by the add and, besides, by addresses alone (the byte load that
    // folded the add's `+ 1` as a displacement): those read `y` instead
    let own = &uses[x as usize];
    let others = own.iter().filter(|one| matches!(one, Use::Other)).count();
    // those of the add itself, which the rewrite replaces. None where it reads
    // `x` as an operand of either class: a data read counted in its place was a
    // store, which then took `y` and its address register
    let adds_own = match insts[at].kind {
        Kind::Op(name) => insts[at]
            .operands
            .iter()
            .enumerate()
            .filter(|(index, operand)| {
                matches!(operand, Operand::Use(Reg::Virt(v)) if *v == x)
                    && matches!(
                        operand_use(name, &insts[at].operands, *index, wide),
                        Use::Other
                    )
            })
            .count(),
        _ => 0,
    };
    let addresses_only = own
        .iter()
        .all(
            |one| match one {
                Use::Other | Use::Address | Use::Either => true,
                Use::Copy(to) => function.classes[*to as usize] == Class::Ar32,
                Use::Phi(_) | Use::Step(..) => false,
            },
        );
    // only in the wide run of a size build, which the trial keeps where it is
    // smaller: one more address register live can cost a spill (-O1's bench
    // cells grew 4 to 16 bytes with it)
    if others != adds_own || !addresses_only || (own.len() > 1 && !(wide && size)) {
        return None;
    }
    match function.classes[y as usize] {
        Class::Ar32 => {}
        Class::Xr32 if reaches_addresses(function, uses, y, Some(x)) => {}
        _ => return None,
    }
    Some((copy, base))
}

/// Whether no instruction reads the codes of the one at `at`.
fn codes_unread(
    insts: &[Inst],
    at: usize,
) -> bool {
    for after in &insts[at + 1..] {
        if after.call {
            break;
        }
        // read first: `addx` reads the codes it sets
        if after.reads_flags() {
            return false;
        }
        if after.sets_flags() {
            break;
        }
    }
    true
}

/// Makes each pointer add that qualifies an address-register instruction, and
/// says how many it made. With `wide`, an operand that takes either class (a
/// store's source) and a copy into a fixed address register (a pointer
/// returned) are no data use of the result: the value stays in an address
/// register and they read it there, which holds one more address register
/// where the function has few ([`crate::trial`] keeps the cheaper build).
pub fn address_adds(
    function: &mut MFunction,
    cpu: Cpu,
    size: bool,
    compare_in_address: bool,
    wide: bool,
) -> usize {
    let mut made = 0;
    let uses = uses_by_register(function, compare_in_address, wide);
    for b in 0..function.blocks.len() {
        let mut at = 0;
        while at < function.blocks[b].insts.len() {
            if let Some(done) = rewrite_one(function, &uses, b, at, cpu, size, wide) {
                // the copy before the add is gone
                made += 1;
                at = done;
            } else {
                at += 1;
            }
        }
    }
    made
}

fn rewrite_one(
    function: &mut MFunction,
    uses: &[Vec<Use>],
    b: usize,
    at: usize,
    cpu: Cpu,
    size: bool,
    wide: bool,
) -> Option<usize> {
    let insts = &function.blocks[b].insts;
    let Kind::Op(name) = insts[at].kind else { return None };
    let sign = match name {
        "ADD32di" | "ADDQ32di" => 1,
        "SUB32di" | "SUBQ32di" => -1,
        "ADD32dr" | "ADD32dd" => return rewrite_indexed(function, uses, b, at, wide, size),
        _ => return None,
    };
    let [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(x)), Operand::Imm(value)] = insts[at].operands[..] else {
        return None;
    };
    let (copy, base) = address_copy(function, uses, b, at, x, wide, size)?;
    if !codes_unread(&function.blocks[b].insts, at) || !reaches_addresses(function, uses, d, Some(x)) {
        return None;
    }
    // the form, priced on the CPU's selection class so that the allocation
    // does not depend on the CPU; `refine_address_adds` prices the real one
    let one = cheapest(sign * value, selection_class(cpu), size)?;
    let desc = local::desc(local::code_of(one.opcode)?)?;
    let mut inst: Inst = op_inst(one.opcode, desc, (one.operands)(Reg::Virt(d), base, one.value));
    // none of the three changes the codes; the pin's `adda` says it does
    inst.sets_flags = false;
    function.classes[d as usize] = Class::Ar32;
    if let Reg::Virt(y) = base {
        function.classes[y as usize] = Class::Ar32;
    }
    let insts = &mut function.blocks[b].insts;
    insts[at] = inst;
    insts.remove(copy);
    read_base_instead(function, b, at - 1, x, base, d, Some(sign * value));
    Some(at)
}

/// What read the copy `x` of `base` besides the add (`d = x + step`, now at
/// `at` of block `b`) reads `base`. A copy of `x` after the add whose every
/// user is a displaced address in this block reads the add's result instead,
/// the displacement lowered by the step: `addq.l #1, %a1` then `(0,%a1)`, not
/// `(1,%a2)` with the old pointer kept live beside the new one.
fn read_base_instead(
    function: &mut MFunction,
    b: usize,
    at: usize,
    x: u32,
    base: Reg,
    d: u32,
    step: Option<i64>,
) {
    let copies: Vec<(usize, u32)> = if step.is_some() {
        function.blocks[b]
            .insts
            .iter()
            .enumerate()
            .skip(at + 1)
            .filter_map(|(index, inst)| match (&inst.kind, &inst.operands[..]) {
                (Kind::Copy, [Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from))])
                    if *from == x && function.classes[*to as usize] == Class::Ar32 =>
                {
                    Some((index, *to))
                }
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    };
    let step = step.unwrap_or(0);
    for (index, to) in copies {
        if displaced_in_block(function, b, index, to, step) {
            function.blocks[b].insts[index].operands[1] = Operand::Use(Reg::Virt(d));
            for inst in function.blocks[b].insts[index + 1..].iter_mut() {
                lower_displacement(inst, to, step);
            }
        }
    }
    for block in &mut function.blocks {
        for inst in &mut block.insts {
            for operand in &mut inst.operands {
                if matches!(operand, Operand::Use(Reg::Virt(v)) if *v == x) {
                    *operand = Operand::Use(base);
                }
            }
        }
    }
}

/// The `(Imm(displacement), Use(register))` pair of the address operand of
/// `inst` that reads `register`.
fn displacement_of(
    inst: &Inst,
    register: u32,
) -> Option<usize> {
    let Kind::Op(name) = inst.kind else { return None };
    (1..inst.operands.len()).find(|&at| {
        matches!(inst.operands[at], Operand::Use(Reg::Virt(v)) if v == register)
            && matches!(inst.operands[at - 1], Operand::Imm(_))
            && operand_class(name, at) == Some(Class::Ar32)
    })
}

/// Whether every use of `to`, which the copy at `copy` of block `b` defines,
/// is a displaced address of this block, after the copy, that `step` can be
/// taken from.
fn displaced_in_block(
    function: &MFunction,
    b: usize,
    copy: usize,
    to: u32,
    step: i64,
) -> bool {
    let reads = |inst: &Inst| inst.operands.iter().any(|o| matches!(o, Operand::Use(Reg::Virt(v)) if *v == to));
    let elsewhere = function.blocks.iter().enumerate().any(|(index, block)| {
        block.phis.iter().any(|phi| phi.incoming.iter().any(|(_, r)| matches!(r, Reg::Virt(v) if *v == to)))
            || (index != b && block.insts.iter().any(reads))
    });
    if elsewhere {
        return false;
    }
    let insts = &function.blocks[b].insts;
    insts[..=copy].iter().all(|inst| !reads(inst))
        && insts[copy + 1..].iter().filter(|inst| reads(inst)).all(|inst| {
            displacement_of(inst, to).is_some_and(|at| {
                matches!(
                    inst.operands[at - 1],
                    Operand::Imm(k) if (-32768..=32767).contains(&(k - step))
                )
            }) && inst.operands.iter().filter(|o| matches!(o, Operand::Use(Reg::Virt(v)) if *v == to)).count() == 1
        })
}

fn lower_displacement(
    inst: &mut Inst,
    register: u32,
    step: i64,
) {
    if let Some(at) = displacement_of(inst, register) {
        if let Operand::Imm(k) = inst.operands[at - 1] {
            inst.operands[at - 1] = Operand::Imm(k - step);
        }
    }
}

/// A pointer plus a register, used only as an address, is `lea (0,An,Dn)`:
/// `move.l %a0, %d0; add.l %d2, %d0; move.l %d0, %a1` has the same bytes and
/// no copy to an address register after it.
fn rewrite_indexed(
    function: &mut MFunction,
    uses: &[Vec<Use>],
    b: usize,
    at: usize,
    wide: bool,
    size: bool,
) -> Option<usize> {
    let insts = &function.blocks[b].insts;
    let [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(first)), Operand::Use(Reg::Virt(second))] =
        insts[at].operands[..]
    else {
        return None;
    };
    let (copy, base, x, index) = [(first, second), (second, first)]
        .into_iter()
        .find_map(
            |(x, index)| address_copy(function, uses, b, at, x, wide, size).map(|(copy, base)| (copy, base, x, index)),
        )?;
    if !codes_unread(insts, at) || !reaches_addresses(function, uses, d, Some(x)) {
        return None;
    }
    let desc = local::desc(local::code_of("LEA32f")?)?;
    let operands =
        vec![Operand::Def(Reg::Virt(d)), Operand::Imm(0), Operand::Use(base), Operand::Use(Reg::Virt(index))];
    let mut inst: Inst = op_inst("LEA32f", desc, operands);
    inst.sets_flags = false;
    function.classes[d as usize] = Class::Ar32;
    if let Reg::Virt(y) = base {
        function.classes[y as usize] = Class::Ar32;
    }
    let insts = &mut function.blocks[b].insts;
    insts[at] = inst;
    insts.remove(copy);
    read_base_instead(function, b, at - 1, x, base, d, None);
    Some(at)
}

pub(crate) fn is_data(register: llrm_lir::registers::RegId) -> bool {
    [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7].contains(&register)
}

/// The clocks and bytes of the data-register add `opcode` of `value` on `cpu`.
fn data_add_price(
    opcode: &'static str,
    value: i64,
    cpu: Cpu,
) -> Option<(u32, u32)> {
    let d0 = mc::Operand::Reg(llrm_lir::registers::RegId::from(reg::D0));
    let inst = mc::Inst { opcode, operands: vec![d0.clone(), d0, mc::Operand::Imm(value)] };
    let bytes = crate::encode::encode(&inst).ok()?.bytes.len() as u32;
    let clocks = crate::timing::cost(cpu, &mc::print(&inst).ok()?, bytes, false).ok()?.clocks;
    Some((clocks, bytes))
}

/// The clocks of the two-byte register copy `text` on `cpu`.
fn copy_clocks(
    cpu: Cpu,
    text: &str,
) -> Option<u32> {
    Some(crate::timing::cost(cpu, text, 2, false).ok()?.clocks)
}

/// DEVIATE: the pin keeps the `lea` its selector makes for an add of an
/// immediate to a data value, and its allocator leaves the copies around it:
/// `move.l %d0,%a0; lea (48,%a0),%a0; move.l %a0,%d0`. This is X86's
/// LEA-to-ADD (`FixupLEAs`), after allocation so that the registers stay as
/// allocated (making the add before allocation moved values from address
/// registers into the scarcer data registers: 14 of 300 seeds grew). Exit:
/// none.
///
/// Three adjacent instructions that copy a data register to an address
/// register, `lea` from it, and copy the result to a data register, those
/// two address registers read nowhere else, are `addq`/`subq`/`add` on the
/// data register (with a copy when the two data registers differ), where the
/// timing tables price that lower on `cpu` (bytes first at a smaller size).
/// The add sets the codes the `lea` leaves, so one whose codes a later
/// instruction would read stays.
pub fn refine_lea_round_trips(
    function: &mut MFunction,
    assignment: &Assignment,
    cpu: Cpu,
    size: bool,
) {
    let uses = uses_by_register(function, false, false);
    let single = |v: u32| uses[v as usize].len() == 1;
    let phys = |r: &Reg| match r {
        Reg::Virt(v) => assignment.0[*v as usize],
        Reg::Phys(p) => *p,
    };
    for block in &mut function.blocks {
        let mut at = 0;
        while at + 2 < block.insts.len() {
            let rewrite = (|| {
                let insts = &block.insts[at..at + 3];
                let ([Operand::Def(Reg::Virt(base)), Operand::Use(x)], Kind::Copy) =
                    (&insts[0].operands[..], &insts[0].kind)
                else {
                    return None;
                };
                let (Kind::Op("LEA32p"), [Operand::Def(Reg::Virt(d)), Operand::Imm(value), Operand::Use(Reg::Virt(b))]) =
                    (&insts[1].kind, &insts[1].operands[..])
                else {
                    return None;
                };
                let ([Operand::Def(w), Operand::Use(Reg::Virt(c))], Kind::Copy) =
                    (&insts[2].operands[..], &insts[2].kind)
                else {
                    return None;
                };
                if b != base || c != d || !single(*base) || !single(*d) || !is_data(phys(x)) || !is_data(phys(w)) {
                    return None;
                }
                if is_data(phys(&Reg::Virt(*base))) || is_data(phys(&Reg::Virt(*d))) {
                    return None;
                }
                for after in &block.insts[at + 3..] {
                    if after.call {
                        break;
                    }
                    // read first: `addx` reads the codes it sets
                    if after.reads_flags() {
                        return None;
                    }
                    if after.sets_flags() {
                        break;
                    }
                }
                let (opcode, immediate) = match *value {
                    1..=8 => ("ADDQ32di", *value),
                    -8..=-1 => ("SUBQ32di", -*value),
                    _ => ("ADD32di", *value),
                };
                // the three as they are, against the add (and a copy)
                let same = phys(x) == phys(w);
                let lea = price(&Candidate { opcode: "LEA32p", operands: displaced, value: *value }, cpu)?;
                let (clocks, bytes) = data_add_price(opcode, immediate, cpu)?;
                let old = (
                    copy_clocks(cpu, "move.l\t%d0, %a0")? + lea.0 + copy_clocks(cpu, "move.l\t%a0, %d0")?,
                    2 + lea.1 + 2,
                );
                let copy_cost = if same { 0 } else { copy_clocks(cpu, "move.l\t%d0, %d1")? };
                let new = (clocks + copy_cost, bytes + if same { 0 } else { 2 });
                if crate::timing::order(cpu, size, new.0, new.1) >= crate::timing::order(cpu, size, old.0, old.1) {
                    return None;
                }
                let desc = local::desc(local::code_of(opcode)?)?;
                let add = |source: Reg| op_inst(opcode, desc, tied(*w, source, immediate));
                Some(if same {
                    vec![add(*x)]
                } else {
                    let mut copy = insts[2].clone();
                    copy.operands = vec![Operand::Def(*w), Operand::Use(*x)];
                    vec![copy, add(*w)]
                })
            })();
            match rewrite {
                Some(new) => {
                    let count = new.len();
                    block.insts.splice(at..at + 3, new);
                    at += count;
                }
                None => at += 1,
            }
        }
    }
}

/// The CPU whose tables the pin selects from: the 68000 for it and the 68010,
/// the 68020 for the rest.
fn selection_class(cpu: Cpu) -> Cpu {
    if cpu.at_least(Cpu::M68020) { Cpu::M68020 } else { Cpu::M68000 }
}

/// DEVIATE: the pin has no such pass on m68k; this is X86FixupLEAs's `lea` to
/// `add` (and back) for a result that is its base, priced on the CPU's tables.
/// Exit: none.
///
/// After allocation, an add of a constant to an address register that stays in
/// it (`lea (d,An), An`, `addq`/`subq`, `adda`) is the cheapest of the three on
/// `cpu`. One whose result is another register stays `lea`, for the tied forms
/// would need a copy.
pub fn refine_address_adds(
    function: &mut MFunction,
    assignment: &Assignment,
    cpu: Cpu,
    size: bool,
) {
    for block in &mut function.blocks {
        for inst in &mut block.insts {
            let Kind::Op(name) = inst.kind else { continue };
            let (d, b, value) = match (name, &inst.operands[..]) {
                ("LEA32p", [Operand::Def(Reg::Virt(d)), Operand::Imm(value), Operand::Use(Reg::Virt(b))]) => {
                    (*d, *b, *value)
                }
                (
                    "ADDQ32ai" | "ADD32ai",
                    [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(b)), Operand::Imm(value)],
                ) => (*d, *b, *value),
                ("SUBQ32ai", [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(b)), Operand::Imm(value)]) => {
                    (*d, *b, -*value)
                }
                _ => continue,
            };
            if REGISTER_INFO.root(assignment.0[d as usize]) != REGISTER_INFO.root(assignment.0[b as usize]) {
                continue;
            }
            let Some(one) = cheapest(value, cpu, size) else { continue };
            if one.opcode == name {
                continue;
            }
            let Some(desc) = local::code_of(one.opcode).and_then(local::desc) else { continue };
            let mut new: Inst = op_inst(one.opcode, desc, (one.operands)(Reg::Virt(d), Reg::Virt(b), one.value));
            // none of the three change the codes; the pin's `adda` says it does
            new.sets_flags = false;
            *inst = new;
        }
    }
}
