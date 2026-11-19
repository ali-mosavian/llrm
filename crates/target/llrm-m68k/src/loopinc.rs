//! A load through `(An)` in one block and the step of `An` by its size in
//! another is one `(An)+` in the first: a loop whose pointer is read in the
//! header and stepped in the latch, with a conditional block between.
//!
//! # How it works
//! [`post_increment_across_blocks`] runs on the selected function, before
//! allocation. A step `q = p + n` (`addq`, `lea`) whose block is dominated by
//! a plain load `(p)` of `n` bytes in the same cycle, where every other use
//! of `p` is a memory address after that load, becomes the load's writeback
//! form: the load defines `q`, the step goes, and each later use of `p` reads
//! `q` and takes the displacement less `n`. One such use is allowed (an
//! `(An)` that becomes `(d16,An)`, 2 bytes), so the step's 2 bytes are never
//! paid back with a loss.
//!
//! DEVIATE: the pin's indexed combine looks inside one block, and its LSR
//! puts the increment beside a load only when nothing else reads the old
//! pointer. gcc's `auto_inc_dec` rewrites the later uses as displacements.
//! Reason: bytes and clocks on every loop that steps a pointer past a
//! conditional store (`partition`, `copy`). Owner m68k. Exit: 2026-11-14 (lands or
//! goes). Off for `Options::PINNED`.
//!
//! # See also
//! `AArch64LoadStoreOpt::findMatchingUpdateInsnForward`, `auto_inc_dec` in
//! gcc.

use llrm_spike_mf::cfg::Cfg;
use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::chain::{self, Mode};
use crate::dagisel::local;
use crate::mfunc::{Class, MFunction, op_inst, operand_class};

/// Where the value of a register is read: through the register itself or a
/// copy of it, in blocks and instruction numbers.
#[derive(Default)]
struct Reads {
    /// `(block, instruction, register read)`.
    insts: Vec<(usize, usize, u32)>,
    /// Read by a phi: the value must stay.
    phi: bool,
}

/// `(q, p, n)` of a step `q = p + n`.
fn step(inst: &Inst) -> Option<(u32, u32, i64)> {
    let Kind::Op(name) = inst.kind else { return None };
    match (name, &inst.operands[..]) {
        ("ADDQ32ai" | "ADD32ai", [Operand::Def(Reg::Virt(q)), Operand::Use(Reg::Virt(p)), Operand::Imm(n)]) => {
            Some((*q, *p, *n))
        }
        ("LEA32p", [Operand::Def(Reg::Virt(q)), Operand::Imm(n), Operand::Use(Reg::Virt(p))]) => Some((*q, *p, *n)),
        _ => None,
    }
}

/// `load` as the writeback load that also defines `updated`, when the pin has
/// one for a plain move, and the class its result must narrow to. The pin's
/// `MOV32rj` takes either kind of register; its writeback rows are `MOV32do`
/// and `MOV32ao`.
fn writeback(
    classes: &[Class],
    load: &Inst,
    updated: u32,
) -> Option<(Inst, Option<(u32, Class)>)> {
    let Kind::Op(name) = load.kind else { return None };
    if name.starts_with("MOVSX") || name.starts_with("MOVZX") {
        return None;
    }
    let slot = chain::slots(name).into_iter().find(|slot| slot.mode == Mode::Ari)?;
    let [Operand::Def(Reg::Virt(result)), ..] = load.operands[..] else { return None };
    let bits = name.strip_prefix("MOV")?.trim_end_matches(|c: char| c.is_alphabetic());
    // `MOV32rj` takes either kind of register and has a `MOV32ro` row, but the
    // writebacks are by kind: `MOV32do_WB`, `MOV32ao_WB`
    let twins: Vec<String> = if name.ends_with("rj") {
        let mut all = vec![format!("MOV{bits}do")];
        if bits == "32" {
            all.push("MOV32ao".to_owned());
        }
        all
    } else {
        vec![chain::with_mode(name, slot, 'o')?.to_owned()]
    };
    for twin in twins {
        let Some(row) = local::code_of(&format!("{twin}_WB")).and_then(|code| local::name_of(u32::from(code))) else {
            continue;
        };
        let Some(desc) = local::code_of(row).and_then(local::desc) else { continue };
        let mut operands = load.operands.clone();
        let defs = operands.iter().take_while(|operand| matches!(operand, Operand::Def(_))).count();
        if defs != 1 {
            return None;
        }
        operands.insert(defs, Operand::Def(Reg::Virt(updated)));
        let want = operand_class(row, 0);
        let have = classes[result as usize];
        let narrow = match want {
            Some(want) if have == want || have.meet(want) == Some(have) => None,
            Some(want) if have == Class::Xr32 => Some((result, want)),
            Some(_) => continue,
            None => None,
        };
        return Some((op_inst(row, desc, operands), narrow));
    }
    None
}

pub fn post_increment_across_blocks(function: &mut MFunction) {
    let count = function.classes.len();
    // A register made once, by a copy of another, is that register's value.
    let mut defs = vec![0u32; count];
    let mut copy_of: Vec<Option<(u32, usize, usize)>> = vec![None; count];
    for (b, block) in function.blocks.iter().enumerate() {
        for phi in &block.phis {
            if let Reg::Virt(v) = phi.result {
                defs[v as usize] += 2;
            }
        }
        for (at, inst) in block.insts.iter().enumerate() {
            for operand in &inst.operands {
                if let Operand::Def(Reg::Virt(v)) = operand {
                    defs[*v as usize] += 1;
                }
            }
            if let (Kind::Copy, [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(s))]) =
                (inst.kind, &inst.operands[..])
            {
                copy_of[*d as usize] = Some((*s, b, at));
            }
        }
    }
    let alias = |v: u32| copy_of[v as usize].filter(|_| defs[v as usize] == 1);
    let root = |mut v: u32| {
        for _ in 0..8 {
            match alias(v) {
                Some((s, ..)) => v = s,
                None => break,
            }
        }
        v
    };
    let mut reads: Vec<Reads> = (0..count).map(|_| Reads::default()).collect();
    let mut total = vec![0u32; count];
    for (b, block) in function.blocks.iter().enumerate() {
        for phi in &block.phis {
            for (_, incoming) in &phi.incoming {
                if let Reg::Virt(v) = incoming {
                    reads[root(*v) as usize].phi = true;
                    total[*v as usize] += 1;
                }
            }
        }
        for (at, inst) in block.insts.iter().enumerate() {
            for operand in &inst.operands {
                let Operand::Use(Reg::Virt(v)) = operand else { continue };
                total[*v as usize] += 1;
                // the copy that makes an alias is the alias, not a reader
                let is_alias_copy = matches!(inst.kind, Kind::Copy)
                    && matches!(
                        inst.operands[0],
                        Operand::Def(Reg::Virt(d)) if alias(d).is_some()
                    );
                if !is_alias_copy {
                    reads[root(*v) as usize].insts.push((b, at, *v));
                }
            }
        }
    }
    let cfg = Cfg::new(function);
    // a register a transform wrote or whose readers it moved is not looked at
    // again: one walk, each register's readers listed once
    let mut touched = vec![false; count];
    let mut gone: Vec<(usize, usize)> = Vec::new();
    for step_block in 0..function.blocks.len() {
        for at in 0..function.blocks[step_block].insts.len() {
            let walk = Walk {
                cfg: &cfg,
                reads: &reads,
                total: &total,
                root: &root,
                copy_at: &|v| alias(v).map(|(_, b, at)| (b, at)),
            };
            if let Some(dead) = try_step(function, &walk, &mut touched, step_block, at) {
                gone.push((step_block, at));
                gone.extend(dead);
            }
        }
    }
    // after the walk: the readers' places stay true until then
    gone.sort_unstable();
    gone.dedup();
    for (block, at) in gone.into_iter().rev() {
        function.blocks[block].insts.remove(at);
    }
}

/// What one pass over the function knows.
struct Walk<'a> {
    cfg: &'a Cfg,
    reads: &'a [Reads],
    /// How many operands read each register.
    total: &'a [u32],
    root: &'a dyn Fn(u32) -> u32,
    /// Where the copy that makes a register an alias is.
    copy_at: &'a dyn Fn(u32) -> Option<(usize, usize)>,
}

/// The step at `step_at` folded into a load, with the copies it leaves unread.
fn try_step(
    function: &mut MFunction,
    walk: &Walk,
    touched: &mut [bool],
    step_block: usize,
    step_at: usize,
) -> Option<Vec<(usize, usize)>> {
    let (q, p, n) = step(&function.blocks[step_block].insts[step_at])?;
    let root = (walk.root)(p);
    if !matches!(n, 1 | 2 | 4) || walk.reads[root as usize].phi || touched[root as usize] || root != p {
        return None;
    }
    let uses = &walk.reads[root as usize].insts;
    // the load: a use every other use follows
    let candidates = uses.iter().copied().filter(|&(b, at, reg)| {
        b != step_block
            && walk.cfg.dominates(b, step_block)
            && walk.cfg.cycle(b).is_some()
            && walk.cfg.cycle(b) == walk.cfg.cycle(step_block)
            && writeback(&function.classes, &function.blocks[b].insts[at], q).is_some()
            && {
                let inst = &function.blocks[b].insts[at];
                let Kind::Op(name) = inst.kind else { return false };
                chain::slots(name)
                    .iter()
                    .any(|slot| slot.mode == Mode::Ari && slot.bytes == n && chain::base_of(inst, *slot) == Some(reg))
            }
    });
    for (load_block, load_at, _) in candidates {
        let mut rewrites: Vec<(usize, usize, Inst)> = Vec::new();
        let mut rewritten: Vec<(u32, u32)> = Vec::new();
        let mut ok = true;
        let mut converted = 0;
        for &(b, at, reg) in uses {
            if (b, at) == (load_block, load_at) || (b, at) == (step_block, step_at) {
                continue;
            }
            let after = if b == load_block { at > load_at } else { walk.cfg.dominates(load_block, b) };
            let inst = &function.blocks[b].insts[at];
            let Kind::Op(name) = inst.kind else {
                ok = false;
                break;
            };
            let found: Vec<_> =
                chain::slots(name).into_iter().filter(|s| chain::base_of(inst, *s) == Some(reg)).collect();
            let [slot] = found[..] else {
                ok = false;
                break;
            };
            // the register read once, as the base, by an instruction after the
            // load
            if !after || inst.operands.iter().filter(|o| **o == Operand::Use(Reg::Virt(reg))).count() != 1 {
                ok = false;
                break;
            }
            let Some(mut moved) = chain::moved_back(inst, slot, n) else {
                ok = false;
                break;
            };
            for operand in &mut moved.operands {
                if *operand == Operand::Use(Reg::Virt(reg)) {
                    *operand = Operand::Use(Reg::Virt(q));
                }
            }
            converted += usize::from(slot.mode == Mode::Ari);
            rewrites.push((b, at, moved));
            rewritten.push((reg, 1));
        }
        if !ok || converted > 1 {
            continue;
        }
        let Some((new, narrow)) = writeback(&function.classes, &function.blocks[load_block].insts[load_at], q) else {
            continue;
        };
        if let Some((register, class)) = narrow {
            function.classes[register as usize] = class;
        }
        function.blocks[load_block].insts[load_at] = new;
        for (b, at, inst) in rewrites {
            function.blocks[b].insts[at] = inst;
        }
        // a copy of the pointer nothing reads now
        let mut dead = Vec::new();
        for (reg, _) in &rewritten {
            let left = walk.total[*reg as usize] - rewritten.iter().filter(|(r, _)| r == reg).count() as u32;
            if left == 0 {
                dead.extend((walk.copy_at)(*reg));
            }
        }
        touched[root as usize] = true;
        touched[q as usize] = true;
        return Some(dead);
    }
    None
}
