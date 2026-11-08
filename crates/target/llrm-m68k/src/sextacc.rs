//! `s - (short)x` with the sum in an address register: `suba.w %d3, %a1` for
//! `ext.l %d3; sub.l %d3, %d0`, 2 B and 4 clk less on the 68000.
//!
//! # How it works
//! [`sign_extended_words`] runs before allocation. A `sub` or `add` whose
//! second source is a `MOVSXd32d16` used only there becomes `SUBA16r` or
//! `ADDA16r` of the word, when the sum and its tied source can be address
//! registers (every other definition and use is a copy, a phi or an operand
//! the pin already gives an address-capable class) and nothing reads the
//! codes the old add left (`adda` leaves them alone).
//!
//! DEVIATE: see [`crate::dagisel::adda`]; the pin has no `ADDA`.
//!
//! # See also
//! [`crate::addrarith`], which makes a pointer's adds address-register adds.

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::dagisel::local;
use crate::dagisel::hooks::Cpu as SelectCpu;
use crate::mfunc::{Class, MFunction, op_inst, operand_class};
use crate::timing::Price;

const ADDS: [(&str, &str, bool); 3] =
    [("ADD32dd", "ADDA16r", true), ("ADD32dr", "ADDA16r", true), ("SUB32dd", "SUBA16r", false)];

fn row_of(name: &str) -> Option<(&'static str, bool)> {
    ADDS.iter().find(|(from, ..)| *from == name).map(|&(_, to, commutes)| (to, commutes))
}

/// What `texts` cost on `cpu`: bytes (2 each) and clocks.
fn price_of(
    texts: &[&str],
    cpu: SelectCpu,
) -> Option<(usize, u32)> {
    let mut sum = (0usize, 0u32);
    for text in texts {
        sum.0 += 2;
        sum.1 += crate::timing::cost(cpu, text, 2, false).ok()?.clocks;
    }
    Some(sum)
}

/// Whether the `adda.w` form is cheaper than extending to a long and adding:
/// from a word, `ext.l` and the add go; from a byte, `ext.w` stays and
/// `ext.l` (a single `extb.l` on the 68020 and up) goes.
fn pays(
    byte: bool,
    subtract: bool,
    cpu: SelectCpu,
    price: Price,
) -> bool {
    let (add, adda) = if subtract { ("sub.l\t%d1, %d0", "suba.w\t%d1, %a0") } else { ("add.l\t%d1, %d0", "adda.w\t%d1, %a0") };
    let wide = cpu.at_least(SelectCpu::M68020);
    let (old, new): (Vec<&str>, Vec<&str>) = match (byte, wide) {
        (false, _) => (vec!["ext.l\t%d1", add], vec![adda]),
        (true, false) => (vec!["ext.w\t%d1", "ext.l\t%d1", add], vec!["ext.w\t%d1", adda]),
        (true, true) => (vec!["extb.l\t%d1", add], vec!["ext.w\t%d1", adda]),
    };
    match (price_of(&old, cpu), price_of(&new, cpu)) {
        (Some(old), Some(new)) => price.beats(cpu, new, old),
        _ => false,
    }
}

pub fn sign_extended_words(
    function: &mut MFunction,
    cpu: SelectCpu,
    price: Price,
) {
    let count = function.classes.len();
    let mut defs = vec![0u32; count];
    let mut uses = vec![0u32; count];
    // each sign extension to a long: its place, its source, whether a byte
    let mut word_of: Vec<Option<(usize, usize, Reg, bool)>> = vec![None; count];
    for (b, block) in function.blocks.iter().enumerate() {
        for phi in &block.phis {
            if let Reg::Virt(v) = phi.result {
                defs[v as usize] += 2;
            }
            for (_, incoming) in &phi.incoming {
                if let Reg::Virt(v) = incoming {
                    uses[*v as usize] += 2;
                }
            }
        }
        for (at, inst) in block.insts.iter().enumerate() {
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(v)) => defs[*v as usize] += 1,
                    Operand::Use(Reg::Virt(v)) => uses[*v as usize] += 1,
                    _ => {}
                }
            }
            if let (Kind::Op(name @ ("MOVSXd32d16" | "MOVSXd32d8")), [Operand::Def(Reg::Virt(x)), Operand::Use(source)]) =
                (inst.kind, &inst.operands[..])
            {
                word_of[*x as usize] = Some((b, at, *source, name == "MOVSXd32d8"));
            }
        }
    }
    // the registers that may be address registers: defined and used by copies,
    // phis and operands the pin gives an address-capable class
    let flexible = |v: usize, function: &MFunction| -> bool {
        function.blocks.iter().all(|block| {
            block.insts.iter().all(|inst| {
                inst.operands.iter().enumerate().all(|(position, operand)| match operand {
                    Operand::Def(Reg::Virt(r)) | Operand::Use(Reg::Virt(r)) if *r as usize == v => match inst.kind {
                        Kind::Copy => true,
                        Kind::Op(name) => {
                            matches!(operand_class(name, position), Some(Class::Xr32 | Class::Ar32))
                                || row_of(name).is_some()
                        }
                        _ => false,
                    },
                    _ => true,
                })
            })
        })
    };
    let mut made: Vec<(usize, usize)> = Vec::new();
    for b in 0..function.blocks.len() {
        for at in 0..function.blocks[b].insts.len() {
            let inst = &function.blocks[b].insts[at];
            let Kind::Op(name) = inst.kind else { continue };
            let Some((row, commutes)) = row_of(name) else { continue };
            let [Operand::Def(Reg::Virt(sum)), Operand::Use(Reg::Virt(first)), Operand::Use(Reg::Virt(second))] =
                inst.operands[..]
            else {
                continue;
            };
            let pick = |x: u32| {
                word_of[x as usize]
                    .filter(|&(wb, wat, ..)| wb == b && wat < at && defs[x as usize] == 1 && uses[x as usize] == 1)
            };
            let (acc, word) = match (pick(second), commutes.then(|| pick(first)).flatten()) {
                (Some(word), _) => (first, word),
                (None, Some(word)) => (second, word),
                _ => continue,
            };
            if acc == sum || !matches!(function.classes[acc as usize], Class::Dr32 | Class::Xr32) {
                continue;
            }
            // the codes the add left must not be read
            let rest = &function.blocks[b].insts[at + 1..];
            let settled = rest
                .iter()
                .find(|one| one.sets_flags || one.reads_flags || one.call)
                .is_some_and(|one| !one.reads_flags || one.call);
            if !settled || !flexible(acc as usize, function) || !flexible(sum as usize, function) {
                continue;
            }
            let Some(desc) = local::code_of(row).and_then(local::desc) else { continue };
            let (_, wat, source, byte) = word;
            if !pays(byte, row == "SUBA16r", cpu, price) {
                continue;
            }
            // a byte is widened to a word first (`ext.w`), the long step is gone
            let used = if byte {
                let Some(widen) = local::code_of("MOVSXd16d8").and_then(local::desc) else { continue };
                let wide = function.classes.len() as u32;
                function.classes.push(Class::Dr16);
                function.blocks[b].insts[wat] =
                    op_inst("MOVSXd16d8", widen, vec![Operand::Def(Reg::Virt(wide)), Operand::Use(source)]);
                Reg::Virt(wide)
            } else {
                made.push((b, wat));
                source
            };
            function.blocks[b].insts[at] = op_inst(
                if row == "ADDA16r" { "ADDA16r" } else { "SUBA16r" },
                desc,
                vec![Operand::Def(Reg::Virt(sum)), Operand::Use(Reg::Virt(acc)), Operand::Use(used)],
            );
            function.classes[acc as usize] = Class::Ar32;
            function.classes[sum as usize] = Class::Ar32;
        }
    }
    made.sort_unstable_by(|a, b| b.cmp(a));
    for (b, at) in made {
        function.blocks[b].insts.remove(at);
    }
}
