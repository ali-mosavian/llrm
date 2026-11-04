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
//! round, so the conditions after it are swapped as in [`foldload`].
//!
//! DEVIATE: the pin folds a load only into a user in its own block
//! (`PeepholeOptimizer::optimizeLoadInstr`), so an argument read in a loop
//! stays in a register for the whole of it. Reason: best of gcc and clang is
//! the goal, and gcc compares against the slot. At speed the memory source
//! costs clocks in the loop, so only a function built for size folds. Exit:
//! none.
//!
//! # See also
//! `TargetInstrInfo::foldMemoryOperand`.
//!
//! [`foldload`]: crate::foldload

use llrm_spike_mf::{Kind, Operand, Reg};
use llrm_support::hash::{HashMap, HashSet};

use crate::dagisel::local;
use crate::foldload::swapped_readers;
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
    let twin = name.strip_suffix("32dr").map(|stem| format!("{stem}32dp"))?;
    let twin = local::name_of(u32::from(local::code_of(&twin)?))?;
    let source = usize::from(local::text(name)?.2.first()?.at);
    let mut operands = inst.operands.clone();
    let mut readers = Vec::new();
    if index != source {
        if !operands.iter().any(|one| matches!(one, Operand::Def(_))) {
            // a compare: the other way round, and the conditions with it
            readers = swapped_readers(&insts[user.1 + 1..])?;
            operands.swap(0, 1);
        } else if inst.commutable {
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
