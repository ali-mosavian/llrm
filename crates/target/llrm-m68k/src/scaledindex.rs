//! `lsl.l #2, %d1; move.l (0,%a0,%d1), %d0` is `move.l (%a0,%d1.l*4), %d0` on
//! the 68020 and later: the shift is part of the address.
//!
//! # How it works
//! [`scale_indexes`] runs after selection, before allocation. A register
//! defined once by `lsl.l #1..3, Rx` whose every use is the index of an
//! indexed operand that has a scaled sibling ([`scaled`]) of that scale is
//! replaced by `Rx` in those operands, which take the sibling's row, and the
//! shift goes. Nothing else may read the shifted register or the codes the
//! shift set. A shifted base (selection put the shifted value in the base
//! slot) swaps with the index; the other operand then becomes the base, taken
//! from the `lea` it was copied from when its class is data-only.
//!
//! DEVIATE: the pin has no scaled index, so it keeps the shift and the
//! register it needs. Reason: best of gcc and clang is the goal; gcc
//! -m68020 scales. Exit: review 2026-11-20. 68020 and later only, from the caller.

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::cmpelim::flags_read_after;
use crate::dagisel::{local, scaled};
use crate::mfunc::{Class, MFunction};

/// The positions of the base and the index in an operand list of row `name`,
/// when it has exactly one indexed operand.
pub(crate) fn memory_at(name: &str) -> Option<(usize, usize)> {
    // an extending load has no printed operands: `dst, disp, base, index`
    if scaled::base_of(name).is_none() && crate::expand::extension(name).is_some_and(|(_, _, mode, _)| mode == 'f') {
        return Some((2, 3));
    }
    let &(_, _, operands) = local::text(name)?;
    let mut found = operands.iter().filter(|one| one.method.contains("ARII"));
    let first = found.next()?;
    found.next().is_none().then(|| (usize::from(first.at) + 1, usize::from(first.at) + 2))
}

/// A register that holds `other`'s value and may be an address register: itself
/// when its class allows, else the source of the one copy that defines it (a
/// `lea` result copied to the index slot of an extending load).
fn address_source(
    function: &MFunction,
    accounts: &[Account],
    other: u32,
) -> Option<u32> {
    if function.classes[other as usize].meet(Class::Ar32).is_some() {
        return Some(other);
    }
    let account = &accounts[other as usize];
    let (b, at) = account.at;
    match (account.defs, function.blocks[b].insts[at].kind, &function.blocks[b].insts[at].operands[..]) {
        (1, Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Virt(from))])
            if accounts[*from as usize].defs <= 1 && function.classes[*from as usize].meet(Class::Ar32).is_some() =>
        {
            Some(*from)
        }
        _ => None,
    }
}

/// Where a virtual register is defined and read.
#[derive(Clone, Default)]
struct Account {
    defs: u32,
    /// `(block, instruction)` of the one definition.
    at: (usize, usize),
    /// Reads other than as an index of a row with one: any phi, any other
    /// operand.
    others: u32,
    /// `(block, instruction, position)` of each read as the base or the index
    /// of an indexed operand.
    indexes: Vec<(usize, usize, usize)>,
    /// The registers copied from this one, with the copy's `(block,
    /// instruction)`.
    copies: Vec<(u32, (usize, usize))>,
}

pub fn scale_indexes(function: &mut MFunction) {
    let mut accounts = vec![Account::default(); function.classes.len()];
    for (b, block) in function.blocks.iter().enumerate() {
        for phi in &block.phis {
            for (_, incoming) in &phi.incoming {
                if let Reg::Virt(v) = incoming {
                    accounts[*v as usize].others += 1;
                }
            }
        }
        for (at, inst) in block.insts.iter().enumerate() {
            let memory = match inst.kind {
                Kind::Op(name) => memory_at(name),
                _ => None,
            };
            if let (Kind::Copy, [Operand::Def(Reg::Virt(copy)), Operand::Use(Reg::Virt(from))]) =
                (inst.kind, &inst.operands[..])
            {
                // a copy of the shifted register is the shifted register
                accounts[*copy as usize].defs += 1;
                accounts[*copy as usize].at = (b, at);
                accounts[*from as usize].copies.push((*copy, (b, at)));
                continue;
            }
            for (position, operand) in inst.operands.iter().enumerate() {
                match operand {
                    Operand::Def(Reg::Virt(v)) => {
                        let account = &mut accounts[*v as usize];
                        account.defs += 1;
                        account.at = (b, at);
                    }
                    Operand::Use(Reg::Virt(v))
                        if memory.is_some_and(|(base, index)| position == base || position == index) =>
                    {
                        accounts[*v as usize].indexes.push((b, at, position));
                    }
                    Operand::Use(Reg::Virt(v)) => accounts[*v as usize].others += 1,
                    _ => {}
                }
            }
        }
    }
    // the scaled copies of each shifted register's reads, decided before any
    // edit
    let mut edits: Vec<(u32, u32, u8)> = Vec::new();
    for (shifted, account) in accounts.iter().enumerate() {
        if account.defs != 1 || account.others != 0 || (account.indexes.is_empty() && account.copies.is_empty()) {
            continue;
        }
        let (b, at) = account.at;
        let inst = &function.blocks[b].insts[at];
        // `lsl.l #n, Rx`, or `add.l Rx, Rx` for 2
        let (def, source, count) = match (inst.kind, &inst.operands[..]) {
            (
                Kind::Op("SHL32di"),
                [Operand::Def(Reg::Virt(def)), Operand::Use(Reg::Virt(source)), Operand::Imm(count @ 1..=3)],
            ) => (*def, *source, *count),
            (
                Kind::Op("ADD32dd"),
                [Operand::Def(Reg::Virt(def)), Operand::Use(Reg::Virt(one)), Operand::Use(Reg::Virt(other))],
            ) if one == other => (*def, *one, 1),
            _ => continue,
        };
        let scale = 1u8 << count;
        // the source is read where the shift was: it must have one definition
        if def as usize != shifted || accounts[source as usize].defs > 1 {
            continue;
        }
        if flags_read_after(&function.blocks[b].insts, at) != Some(0) {
            continue;
        }
        let scales = |reads: &[(usize, usize, usize)]| {
            reads
                .iter()
                .all(
                    |&(rb, ri, position)| {
                        let inst = &function.blocks[rb].insts[ri];
                        let Kind::Op(name) = inst.kind else { return false };
                        let Some((base, index)) = memory_at(name) else { return false };
                        // the scaled register is the index: the other operand
                        // is the base, and must be able to be an address
                        // register
                        let other = if position == index { base } else { index };
                        let Operand::Use(Reg::Virt(other)) = inst.operands[other] else { return false };
                        scaled::scaled_name(name, scale).is_some()
                            && (position == index || address_source(function, &accounts, other).is_some())
                            && inst.operands[position] != inst.operands[if position == index { base } else { index }]
                    },
                )
        };
        // each copy is read only as a scaled index too
        let copies_scale = account
            .copies
            .iter()
            .all(
                |&(copy, _)| {
                    let copied = &accounts[copy as usize];
                    copied.defs == 1
                        && copied.others == 0
                        && copied.copies.is_empty()
                        && !copied.indexes.is_empty()
                        && scales(&copied.indexes)
                },
            );
        if scales(&account.indexes) && copies_scale {
            edits.push((def, source, scale));
        }
    }
    let mut gone: Vec<(usize, usize)> = Vec::new();
    // the copies that fed a base: dead if nothing else reads them
    let mut fed: Vec<(u32, (usize, usize))> = Vec::new();
    for (shifted, source, scale) in edits {
        let account = accounts[shifted as usize].clone();
        let mut rewrite = |reads: &[(usize, usize, usize)], function: &mut MFunction| {
            for &(rb, ri, position) in reads {
                let inst = &mut function.blocks[rb].insts[ri];
                let Kind::Op(name) = inst.kind else { continue };
                let Some((base, index)) = memory_at(name) else { continue };
                inst.kind = Kind::Op(scaled::scaled_name(name, scale).expect("checked"));
                if position == base {
                    inst.operands.swap(base, index);
                    let Operand::Use(Reg::Virt(other)) = inst.operands[base] else { continue };
                    let held = address_source(function, &accounts, other).expect("checked");
                    let class = &mut function.classes[held as usize];
                    *class = class.meet(Class::Ar32).expect("checked");
                    function.blocks[rb].insts[ri].operands[base] = Operand::Use(Reg::Virt(held));
                    if held != other {
                        fed.push((other, accounts[other as usize].at));
                    }
                }
                function.blocks[rb].insts[ri].operands[index] = Operand::Use(Reg::Virt(source));
            }
        };
        rewrite(&account.indexes, function);
        for &(copy, at) in &account.copies {
            rewrite(&accounts[copy as usize].indexes, function);
            gone.push(at);
        }
        gone.push(account.at);
    }
    let mut read = vec![false; function.classes.len()];
    for block in &function.blocks {
        for (_, incoming) in block.phis.iter().flat_map(|phi| phi.incoming.iter()) {
            if let Reg::Virt(v) = incoming {
                read[*v as usize] = true;
            }
        }
        for operand in block.insts.iter().flat_map(|inst| inst.operands.iter()) {
            if let Operand::Use(Reg::Virt(v)) = operand {
                read[*v as usize] = true;
            }
        }
    }
    gone.extend(fed.into_iter().filter(|(copy, _)| !read[*copy as usize]).map(|(_, at)| at));
    gone.sort_unstable_by(|one, other| other.cmp(one));
    gone.dedup();
    for (b, at) in gone {
        function.blocks[b].insts.remove(at);
    }
}
