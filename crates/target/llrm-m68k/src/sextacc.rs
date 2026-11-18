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

use crate::REGISTER_INFO;
use crate::dagisel::hooks::Cpu as SelectCpu;
use crate::dagisel::local;
use crate::mfunc::{Class, MFunction, op_inst, operand_class};
use crate::timing::Price;

const ADDS: [(&str, &str, bool); 3] =
    [("ADD32dd", "ADDA16r", true), ("ADD32dr", "ADDA16r", true), ("SUB32dd", "SUBA16r", false)];

fn row_of(name: &str) -> Option<(&'static str, bool)> {
    ADDS.iter().find(|(from, ..)| *from == name).map(|&(_, to, commutes)| (to, commutes))
}

/// The plain load an extending load (any mode, scaled or with a `(An)+`
/// writeback) is made of, and the bits it loads: `MOVSXd32o8_WB` is `MOV8do_WB`
/// and 8. Only a sign extension to a long.
fn plain_of(name: &str) -> Option<(&'static str, u32)> {
    let writeback = name.ends_with("_WB");
    let printed = if writeback { crate::dagisel::indexed::extension_of(name)? } else { name };
    let (stem, scale) = match crate::dagisel::scaled::base_of(printed) {
        Some(base) => (base, crate::dagisel::scaled::scale_of(printed)),
        None => (printed, None),
    };
    let (true, 32, mode, from) = crate::expand::extension(stem)? else { return None };
    let plain = crate::expand::plain_load(from, mode)?;
    let plain = match scale {
        Some(scale) => crate::dagisel::scaled::scaled_name(plain, scale)?,
        None => plain,
    };
    if !writeback {
        return Some((plain, from));
    }
    let twin = crate::dagisel::indexed::code_of(&format!("{plain}_WB"))?;
    Some((crate::dagisel::indexed::name_of(u32::from(twin))?, from))
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
    moves: usize,
    cpu: SelectCpu,
    price: Price,
) -> bool {
    let (add, adda) =
        if subtract { ("sub.l\t%d1, %d0", "suba.w\t%d1, %a0") } else { ("add.l\t%d1, %d0", "adda.w\t%d1, %a0") };
    let wide = cpu.at_least(SelectCpu::M68020);
    let (old, new): (Vec<&str>, Vec<&str>) = match (byte, wide) {
        (false, _) => (vec!["ext.l\t%d1", add], vec![adda]),
        (true, false) => (vec!["ext.w\t%d1", "ext.l\t%d1", add], vec!["ext.w\t%d1", adda]),
        (true, true) => (vec!["extb.l\t%d1", add], vec!["ext.w\t%d1", adda]),
    };
    // each web copy to or from a data register is a `move.l`
    let mut new = new;
    new.extend(std::iter::repeat_n("move.l\t%a0, %d0", moves));
    match (price_of(&old, cpu), price_of(&new, cpu)) {
        (Some(old), Some(new)) => price.beats(cpu, new, old),
        _ => false,
    }
}

/// The number of sums moved to an address register.
pub fn sign_extended_words(
    function: &mut MFunction,
    cpu: SelectCpu,
    price: Price,
) -> usize {
    let count = function.classes.len();
    let mut defs = vec![0u32; count];
    let mut uses = vec![0u32; count];
    // each sign extension to a long: its place, its source, whether a byte
    let mut word_of: Vec<Option<(usize, usize, Reg, bool, Option<&'static str>)>> = vec![None; count];
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
            if let (
                Kind::Op(name @ ("MOVSXd32d16" | "MOVSXd32d8")),
                [Operand::Def(Reg::Virt(x)), Operand::Use(source)],
            ) = (inst.kind, &inst.operands[..])
            {
                word_of[*x as usize] = Some((b, at, *source, name == "MOVSXd32d8", None));
            } else if let (Kind::Op(name), Some(Operand::Def(Reg::Virt(x)))) = (inst.kind, inst.operands.first()) {
                // an extending load: the load stays, narrow, and the extension
                // goes
                if let Some((plain, from)) = plain_of(name) {
                    word_of[*x as usize] = Some((b, at, Reg::Virt(*x), from == 8, Some(plain)));
                }
            }
        }
    }
    // the registers that may be address registers: defined and used by copies,
    // phis and operands the pin gives an address-capable class
    let successors = llrm_spike_mf::regalloc::successors(function);
    // reach[a][b]: whether b follows a by one step or more
    let reach: Vec<Vec<bool>> = (0..successors.len())
        .map(|start| {
            let mut seen = vec![false; successors.len()];
            let mut work = successors[start].clone();
            while let Some(next) = work.pop() {
                if !std::mem::replace(&mut seen[next], true) {
                    work.extend(&successors[next]);
                }
            }
            seen
        })
        .collect();
    // A move runs as often as the add when it is in the add's loop (or the add
    // is in no loop); one in another loop, or outside the add's, runs at its
    // own rate.
    let counts =
        |block: usize, here: usize| block == here || !reach[here][here] || (reach[block][here] && reach[here][block]);
    // How many moves to or from data registers `v` needs as an address
    // register; `None` when a use of it cannot take one.
    let free_uses = |v: usize, function: &MFunction, here: usize, folding: usize| -> Option<usize> {
        let mut moves = 0;
        for (at, index, inst) in function
            .blocks
            .iter()
            .enumerate()
            .flat_map(|(at, block)| block.insts.iter().enumerate().map(move |(index, inst)| (at, index, inst)))
        {
            for (position, operand) in inst.operands.iter().enumerate() {
                let (Operand::Def(Reg::Virt(r)) | Operand::Use(Reg::Virt(r))) = operand else { continue };
                if *r as usize != v {
                    continue;
                }
                match inst.kind {
                    // a value copied out to a data register (a result) costs a
                    // move; one copied in (a call result) is copied in the
                    // pin's code too
                    Kind::Copy => {
                        let out = matches!(operand, Operand::Use(_))
                            && inst.operands.iter().any(|other| {
                                matches!(
                                    other,
                                    Operand::Def(Reg::Phys(p)) if !crate::expand::is_address(REGISTER_INFO.root(*p))
                                )
                            });
                        moves += usize::from(out && counts(at, here));
                    }
                    Kind::Op(name) => {
                        let free = matches!(operand_class(name, position), Some(Class::Ar32))
                            || name.starts_with("MOV32")
                            || name == "MOVI32ri"
                            // only the add being folded: another add keeps its
                            // data operands, and an address register there is
                            // assembled as a data register
                            || (row_of(name).is_some() && at == here && index == folding);
                        if !free {
                            return None;
                        }
                    }
                }
            }
        }
        Some(moves)
    };
    // `v` and the registers joined to it by copies, phis and two-address adds
    // that take an address register: a source's other users are the
    // allocator's, so a register reached as a source is followed to its
    // sources, one reached as a user to its users. Each join to one that does
    // not take an address register costs a move, which counts when it runs as
    // often as the add (`counts`).
    let flexible = |v: usize, function: &MFunction, here: usize, folding: usize| -> Option<usize> {
        // (user, source, whether the move runs as often as the add)
        let mut joins: Vec<(usize, usize, bool)> = Vec::new();
        for (b, block) in function.blocks.iter().enumerate() {
            for phi in &block.phis {
                let Reg::Virt(result) = phi.result else { continue };
                for (from, incoming) in &phi.incoming {
                    if let Reg::Virt(incoming) = incoming {
                        // a phi's copies run at the end of the incoming block
                        joins.push((result as usize, *incoming as usize, counts(*from, here) && counts(b, here)));
                    }
                }
            }
            for inst in &block.insts {
                let tied = match (inst.kind, &inst.operands[..]) {
                    (Kind::Copy, [Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from))]) => Some((to, from)),
                    (Kind::Op(name), [Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from)), Operand::Use(_)])
                        if row_of(name).is_some() || matches!(name, "ADDA16r" | "SUBA16r") =>
                    {
                        Some((to, from))
                    }
                    _ => None,
                };
                if let Some((to, from)) = tied {
                    joins.push((*to as usize, *from as usize, counts(b, here)));
                }
            }
        }
        let mut moves = 0;
        // each member, with the directions it is followed in: (to its sources,
        // to its users)
        let mut web = vec![(v, true, true)];
        let mut at = 0;
        while at < web.len() {
            let (current, up, down) = web[at];
            at += 1;
            moves += free_uses(current, function, here, folding)?;
            for &(user, source, runs) in &joins {
                let (other, as_source) = if up && user == current {
                    (source, true)
                } else if down && source == current {
                    (user, false)
                } else {
                    continue;
                };
                if web.iter().any(|&(member, ..)| member == other) {
                    continue;
                }
                if free_uses(other, function, here, folding).is_some() {
                    web.push((other, as_source, !as_source));
                } else {
                    moves += usize::from(runs);
                }
            }
        }
        Some(moves)
    };
    let mut made: Vec<(usize, usize)> = Vec::new();
    let mut inserted: Vec<(usize, usize, llrm_spike_mf::Inst)> = Vec::new();
    let mut folded = 0;
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
                .find(|one| one.sets_flags() || one.reads_flags() || one.call)
                .is_some_and(|one| !one.reads_flags() || one.call);
            if !settled {
                continue;
            }
            let (Some(into), Some(out)) =
                (flexible(acc as usize, function, b, at), flexible(sum as usize, function, b, at))
            else {
                continue;
            };
            let Some(desc) = local::code_of(row).and_then(local::desc) else { continue };
            let (_, wat, source, byte, load) = word;
            if !pays(byte, row == "SUBA16r", into + out, cpu, price) {
                continue;
            }
            // a byte is widened to a word first (`ext.w`), the long step is
            // gone; an extending load becomes the plain load of its width
            let used = if let Some(plain) = load {
                let Some(desc) = local::code_of(plain).and_then(local::desc) else { continue };
                let narrow = function.classes.len() as u32;
                function.classes.push(if byte { Class::Dr8 } else { Class::Dr16 });
                let mut operands = function.blocks[b].insts[wat].operands.clone();
                operands[0] = Operand::Def(Reg::Virt(narrow));
                function.blocks[b].insts[wat] = op_inst(plain, desc, operands);
                if byte {
                    let Some(widen) = local::code_of("MOVSXd16d8").and_then(local::desc) else { continue };
                    let wide = function.classes.len() as u32;
                    function.classes.push(Class::Dr16);
                    inserted.push((
                        b,
                        wat,
                        op_inst(
                            "MOVSXd16d8",
                            widen,
                            vec![Operand::Def(Reg::Virt(wide)), Operand::Use(Reg::Virt(narrow))],
                        ),
                    ));
                    Reg::Virt(wide)
                } else {
                    Reg::Virt(narrow)
                }
            } else if byte {
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
            folded += 1;
        }
    }
    // from the back, so the places still hold
    let mut edits: Vec<(usize, usize, Option<llrm_spike_mf::Inst>)> = made
        .into_iter()
        .map(|(b, at)| (b, at, None))
        .chain(inserted.into_iter().map(|(b, at, inst)| (b, at + 1, Some(inst))))
        .collect();
    edits.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));
    for (b, at, inst) in edits {
        match inst {
            Some(inst) => function.blocks[b].insts.insert(at, inst),
            None => {
                function.blocks[b].insts.remove(at);
            }
        }
    }
    folded
}
