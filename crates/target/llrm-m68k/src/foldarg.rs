//! A stack argument that one instruction reads is read in place: `move.l
//! 4(%sp), %a1` before a loop and `cmp.l %a1, %d2` in it are `cmp.l 4(%sp),
//! %d2`.
//!
//! # How it works
//! [`fold_argument_loads`] finds a load of an incoming-argument slot that
//! nothing stores to or takes the address of, follows its value through
//! copies, and when it reaches one instruction that has a memory-source twin
//! (`ADD32dr` is `ADD32dp`) makes that instruction read the slot. The load and
//! the copies go, and so does the register that held the argument. A user
//! that is a compare of the value with another reads the codes the other way
//! round, so the conditions after it are swapped as in [`foldload`]. A user
//! that stores the value (not to the call area) stores from the slot instead,
//! the bytes of the long that its width names (`move.b 11(%sp), (%a1)+`).
//!
//! DEVIATE: the pin folds a load only into a user in its own block
//! (`PeepholeOptimizer::optimizeLoadInstr`), so an argument read in a loop
//! stays in a register for the whole of it. Reason: best of gcc and clang is
//! the goal, and gcc compares against the slot. At speed the memory source
//! costs clocks in the loop, so only a function built for size folds. Exit:
//! review 2026-11-20.
//!
//! # See also
//! `TargetInstrInfo::foldMemoryOperand`.
//!
//! [`foldload`]: crate::foldload

use llrm_spike_mf::{Kind, Operand, Reg};
use llrm_support::hash::{HashMap, HashSet};

use crate::dagisel::{bfffo, indexed, local};
use crate::foldload::{commutes, swapped_readers};
use crate::mfunc::{MFunction, op_inst};

/// Where an instruction is: block and index.
type At = (usize, usize);

/// The load `MOV32rp` makes of an argument slot: result, displacement, slot.
fn argument_load(
    function: &MFunction,
    inst: &llrm_spike_mf::Inst,
) -> Option<(u32, i64, i32)> {
    let Kind::Op("MOV32rp") = inst.kind else { return None };
    let [Operand::Def(Reg::Virt(v)), Operand::Imm(displacement), Operand::Frame(slot)] = inst.operands[..] else {
        return None;
    };
    function.fixed.contains_key(&slot).then_some((v, displacement, slot))
}

pub fn fold_argument_loads(function: &mut MFunction) {
    let mut loads = Vec::new();
    // slots something other than one of these loads names
    let mut taken = HashSet::default();
    let mut users: HashMap<u32, Vec<(At, usize)>> = HashMap::default();
    let mut in_phis = HashSet::default();
    for (b, block) in function.blocks.iter().enumerate() {
        for phi in &block.phis {
            in_phis.extend(phi.incoming.iter().filter_map(|(_, r)| if let Reg::Virt(v) = r { Some(*v) } else { None }));
        }
        for (i, inst) in block.insts.iter().enumerate() {
            let load = argument_load(function, inst);
            if let Some(found) = load {
                loads.push(((b, i), found));
            }
            for (index, operand) in inst.operands.iter().enumerate() {
                match operand {
                    Operand::Frame(slot) if load.is_none() => {
                        taken.insert(*slot);
                    }
                    Operand::Use(Reg::Virt(v)) => users.entry(*v).or_default().push(((b, i), index)),
                    _ => {}
                }
            }
        }
    }
    let mut gone: HashSet<At> = HashSet::default();
    let mut rewritten: HashSet<At> = HashSet::default();
    for (load_at, (value, displacement, slot)) in loads {
        if taken.contains(&slot) {
            continue;
        }
        // the value through copies to what reads it
        let (mut copies, mut readers, mut pending) = (Vec::new(), Vec::new(), vec![value]);
        let mut reaches_phi = false;
        while let Some(register) = pending.pop() {
            reaches_phi |= in_phis.contains(&register);
            for &(at, index) in users.get(&register).into_iter().flatten() {
                let inst = &function.blocks[at.0].insts[at.1];
                match (&inst.kind, &inst.operands[..]) {
                    (Kind::Copy, [Operand::Def(Reg::Virt(to)), _]) => {
                        copies.push(at);
                        pending.push(*to);
                    }
                    _ => readers.push((at, index)),
                }
            }
        }
        let [(user, index)] = readers[..] else { continue };
        // a branch after the load may read the codes it set (`cmpelim` took
        // its `tst` for them); the folded user does not set them there
        if codes_are_read(&function.blocks[load_at.0].insts, load_at.1) {
            continue;
        }
        if reaches_phi || rewritten.contains(&user) {
            continue;
        }
        let Some(folded) = fold_user(function, user, index, displacement, slot) else { continue };
        let (insts, readers_after) = folded;
        let block = &mut function.blocks[user.0];
        block.insts[user.1] = insts;
        for (offset, reader) in readers_after {
            block.insts[user.1 + 1 + offset] = reader;
        }
        rewritten.insert(user);
        gone.insert(load_at);
        gone.extend(copies);
    }
    for (b, block) in function.blocks.iter_mut().enumerate() {
        let mut i = 0;
        block
            .insts
            .retain(
                |_| {
                    i += 1;
                    !gone.contains(&(b, i - 1))
                },
            );
    }
}

/// Whether an instruction after the one at `at` reads the codes it set before
/// anything sets them again.
fn codes_are_read(
    insts: &[llrm_spike_mf::Inst],
    at: usize,
) -> bool {
    if !insts[at].sets_flags() {
        return false;
    }
    for inst in &insts[at + 1..] {
        if inst.reads_flags() {
            return true;
        }
        if inst.sets_flags() || inst.call {
            return false;
        }
    }
    false
}

/// The store that writes what a slot holds as `store` writes a register:
/// `MOV8od_WB` is `MOV8op_WB`, `MOV8fd` is `MOV8fp`.
fn store_twin(store: &str) -> Option<String> {
    if let Some(twin) = indexed::slot_store_twin(store) {
        return Some(twin.to_owned());
    }
    let stem = store.strip_prefix("MOV")?;
    let (bits, rest) = stem.split_at(stem.find(|c: char| !c.is_ascii_digit())?);
    let [to, from] = rest.as_bytes() else { return None };
    (matches!(to, b'j' | b'p' | b'f') && matches!(from, b'd' | b'r')).then(|| format!("MOV{bits}{}p", *to as char))
}

/// `user` with its register operand `index` read from the slot, and the
/// conditions after it as they read once a compare's operands change places.
fn fold_user(
    function: &MFunction,
    user: At,
    index: usize,
    displacement: i64,
    slot: i32,
) -> Option<(llrm_spike_mf::Inst, Vec<(usize, llrm_spike_mf::Inst)>)> {
    let insts = &function.blocks[user.0].insts;
    let inst = &insts[user.1];
    let Kind::Op(name) = inst.kind else { return None };
    if let Some(twin) = store_twin(name) {
        // a store of the slot's bytes: the low ones of a big-endian long
        let bits: i64 = name.strip_prefix("MOV")?.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()?;
        // a store to the call area is for `callpush`, which pushes from the
        // slot
        let to_stack = inst
            .operands
            .iter()
            .any(
                |operand| match operand {
                    Operand::Use(Reg::Virt(base)) => insts[..user.1].iter().any(|before| {
                        matches!(
                            (before.kind, &before.operands[..]),
                            (Kind::Copy, [Operand::Def(Reg::Virt(copy)), Operand::Use(Reg::Phys(_))]) if copy == base
                        )
                    }),
                    _ => false,
                },
            );
        if index + 1 != inst.operands.len() || to_stack {
            return None;
        }
        let mut operands = inst.operands.clone();
        operands.pop();
        operands.extend([Operand::Imm(displacement + 4 - bits / 8), Operand::Frame(slot)]);
        let desc = local::desc(local::code_of(&twin)?)?;
        let mut folded = op_inst(local::name_of(u32::from(local::code_of(&twin)?))?, desc, operands);
        folded.origin = inst.origin;
        return Some((folded, Vec::new()));
    }
    let twin = match bfffo::memory_twin(name) {
        Some(twin) => twin.to_owned(),
        None => name.strip_suffix("32dr").map(|stem| format!("{stem}32dp"))?,
    };
    let twin = local::name_of(u32::from(local::code_of(&twin)?))?;
    let source = usize::from(local::text(name)?.2.first()?.at);
    let mut operands = inst.operands.clone();
    let mut readers = Vec::new();
    if index != source {
        if !operands.iter().any(|one| matches!(one, Operand::Def(_))) {
            // a compare: the other way round, and the conditions with it
            readers = swapped_readers(&insts[user.1 + 1..])?;
            operands.swap(0, 1);
        } else if commutes(name, inst.commutable) {
            operands.swap(index, source);
        } else {
            return None;
        }
    }
    operands.remove(source);
    let memory = usize::from(local::text(twin)?.2.first()?.at);
    operands.splice(memory..memory, [Operand::Imm(displacement), Operand::Frame(slot)]);
    let desc = local::desc(local::code_of(twin)?)?;
    let mut folded = op_inst(twin, desc, operands);
    folded.origin = inst.origin;
    Some((folded, readers))
}
