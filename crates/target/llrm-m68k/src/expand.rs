//! After allocation: the pseudo-instructions expanded, frame indices turned
//! into stack addresses, the prologue and epilogue added, and the machine
//! function lowered to printable instructions.
//!
//! The expansions are `M68kExpandPseudo`'s and `M68kInstrInfo`'s:
//! `MOVI32ri` is `moveq` (`ExpandMOVI`), `MOVZXd32p8` clears the register and
//! loads the byte (`ExpandMOVSZX_RM`), `RET` is `rts`, a register copy is
//! `move.l` (`copyPhysReg`), and the call-stack pseudos go (the frame reserves
//! the call area).
//!
//! DEVIATE: only 32-bit copies, `moveq` immediates and the zero-extending byte
//! load are expanded. Reason: the slice does not reach the rest. Exit: M4c.

use llrm_codegen::backend::frameobjects::Object;
use llrm_lir::registers::RegId;

use crate::frame::{self, Base, FrameOp, Function};
use crate::mc;
use crate::mfunc::{Kind, MFunction, Operand, Reg};
use crate::regalloc::Assignment;
use crate::{REGISTER_INFO, reg};

fn physical(
    register: Reg,
    assignment: &Assignment,
) -> RegId {
    match register {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(p) => p,
    }
}

pub(crate) fn mask_bit(root: RegId) -> Option<u32> {
    let index = |all: [RegId; 8]| all.iter().position(|one| *one == root);
    index([reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7])
        .map(|at| at as u32)
        .or_else(
            || index([reg::A0, reg::A1, reg::A2, reg::A3, reg::A4, reg::A5, reg::A6, reg::SP]).map(|at| 8 + at as u32),
        )
}

fn frame_op(
    op: &FrameOp,
    out: &mut Vec<mc::Inst>,
) {
    use mc::Operand::{Imm, Reg as R};
    let base = |base: &Base| R(if *base == Base::Sp { reg::SP } else { reg::A6 });
    let mask = |registers: &[RegId]| {
        registers.iter().filter_map(|one| mask_bit(REGISTER_INFO.root(*one))).fold(0i64, |all, bit| all | 1 << bit)
    };
    out.push(match op {
        FrameOp::Link { bytes } => mc::Inst { opcode: "LINK16", operands: vec![R(reg::A6), Imm(-bytes)] },
        FrameOp::Unlink => mc::Inst { opcode: "UNLK", operands: vec![R(reg::A6)] },
        FrameOp::SubSp(bytes) => mc::Inst { opcode: "SUB32ai", operands: vec![R(reg::SP), R(reg::SP), Imm(*bytes)] },
        FrameOp::AddSp(bytes) => mc::Inst { opcode: "ADD32ai", operands: vec![R(reg::SP), R(reg::SP), Imm(*bytes)] },
        FrameOp::Save { registers, base: at, offset } => {
            mc::Inst { opcode: "MOVM32pm", operands: vec![Imm(*offset), base(at), Imm(mask(registers))] }
        }
        FrameOp::Push(registers) | FrameOp::Pop(registers) => {
            let pushing = matches!(op, FrameOp::Push(_));
            let mut sorted: Vec<RegId> = registers.clone();
            sorted.sort_by_key(|one| mask_bit(REGISTER_INFO.root(*one)));
            match (&sorted[..], pushing) {
                ([one], true) => mc::Inst { opcode: "PUSH32", operands: vec![R(*one)] },
                ([one], false) => mc::Inst { opcode: "POP32", operands: vec![R(*one)] },
                (_, true) => mc::Inst { opcode: "PUSHM32", operands: vec![Imm(mask(registers))] },
                (_, false) => mc::Inst { opcode: "POPM32", operands: vec![Imm(mask(registers))] },
            }
        }
        FrameOp::Restore { registers, base: at, offset } => {
            mc::Inst { opcode: "MOVM32mp", operands: vec![Imm(mask(registers)), Imm(*offset), base(at)] }
        }
    });
}

/// A line of the function: a block's label (its layout index) or an
/// instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Line {
    Label(usize),
    Inst(mc::Inst),
}

/// A branch to the block laid out next is not kept (`BranchFolding`'s
/// fall-through, which the builder leaves to the emitter).
fn falls_through(
    function: &MFunction,
    at: usize,
    inst: &crate::mfunc::Inst,
) -> bool {
    matches!(inst.kind, Kind::Op(name) if name.starts_with("BRA"))
        && matches!(
            inst.operands.first(),
            Some(Operand::Block(id)) if function.blocks.get(at + 1).is_some_and(|next| next.id == *id)
        )
}

/// The function as lines; `number` is the function's number in the module,
/// which names the labels.
pub fn finish(
    function: &MFunction,
    assignment: &Assignment,
    number: usize,
    slots: &[i32],
    overlay: bool,
) -> Result<Vec<Line>, String> {
    let kept: Vec<(usize, &crate::mfunc::Inst)> = function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(at, block)| block.insts.iter().map(move |inst| (at, inst)))
        .filter(|(at, inst)| {
            let last = function.blocks[*at].insts.last().is_some_and(|last| std::ptr::eq(last, *inst));
            !(last && falls_through(function, *at, inst))
        })
        .collect();
    let insts: Vec<_> = kept.iter().map(|(_, inst)| *inst).collect();
    let targeted: Vec<usize> = insts
        .iter()
        .flat_map(|inst| &inst.operands)
        .filter_map(|operand| match operand {
            Operand::Block(id) => function.blocks.iter().position(|block| block.id == *id),
            _ => None,
        })
        .collect();
    let has_calls = insts.iter().any(|inst| inst.call);
    let max_call_frame = insts
        .iter()
        .filter(|inst| inst.kind == Kind::Op("ADJCALLSTACKDOWN"))
        .filter_map(|inst| match inst.operands.first() {
            Some(Operand::Imm(bytes)) => Some(*bytes),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    // the callee-saved registers the allocation used, in `CSR_STD` order
    let used: Vec<RegId> = frame::callee_saved()
        .into_iter()
        .filter(|saved| {
            insts
                .iter()
                .flat_map(|inst| &inst.operands)
                .any(
                    |operand| match operand {
                        Operand::Def(r) | Operand::Use(r) => REGISTER_INFO.root(physical(*r, assignment)) == *saved,
                        _ => false,
                    },
                )
        })
        .collect();
    let fixed: Vec<i32> = function.fixed.keys().rev().copied().collect();
    let mut objects: Vec<Object> = fixed
        .iter()
        .map(|index| Object { size: 4, align: 4, offset: function.fixed[index], callee_saved: false, dead: false })
        .collect();
    // the static allocas, then the spill slots, are locals after the fixed
    // objects (`getStackSlotRange`: one long each)
    objects.extend(function.locals.iter().map(|&(_, size, align)| Object {
        size,
        align,
        offset: 0,
        callee_saved: false,
        dead: false,
    }));
    objects.extend(slots.iter().map(|_| Object { size: 4, align: 4, offset: 0, callee_saved: false, dead: false }));
    let frame_indices: Vec<i32> =
        fixed.iter().copied().chain(function.locals.iter().map(|local| local.0)).chain(slots.iter().copied()).collect();
    let layout = frame::layout(&Function {
        fixed: fixed.len(),
        objects,
        saved: used,
        max_call_frame,
        adjusts_stack: has_calls,
        ..Function::default()
    });
    let mut out = Vec::new();
    let pushes = overlay && layout.pushes();
    let opening = if pushes { layout.push_prologue() } else { layout.prologue() };
    opening.iter().for_each(|op| frame_op(op, &mut out));
    let mut lines = Vec::new();
    let mut block_of = kept.iter().map(|(at, _)| *at).peekable();
    let mut shown = 0usize;
    for inst in insts.iter().copied() {
        let at = block_of.next().expect("a block per instruction");
        while shown <= at {
            if targeted.contains(&shown) {
                lines.extend(out.drain(..).map(Line::Inst));
                lines.push(Line::Label(shown));
            }
            shown += 1;
        }
        let operands: Vec<mc::Operand> = {
            let mut converted = Vec::new();
            let mut it = inst.operands.iter().peekable();
            while let Some(operand) = it.next() {
                converted.push(match operand {
                    Operand::Def(r) | Operand::Use(r) => mc::Operand::Reg(physical(*r, assignment)),
                    Operand::Imm(value) => {
                        // a frame index follows the displacement it adds to
                        if let Some(Operand::Frame(index)) = it.peek() {
                            let at =
                                frame_indices.iter().position(|one| one == index).ok_or("an unknown frame index")?;
                            let (base, disp) = layout.address(at);
                            it.next();
                            converted.push(mc::Operand::Imm(value + disp));
                            converted.push(mc::Operand::Reg(if base == Base::Sp { reg::SP } else { reg::A6 }));
                            continue;
                        }
                        mc::Operand::Imm(*value)
                    }
                    Operand::Symbol(name, addend) => mc::Operand::Symbol(name.clone(), *addend),
                    Operand::Block(id) => {
                        let at =
                            function.blocks.iter().position(|block| block.id == *id).ok_or("a branch to no block")?;
                        mc::Operand::Symbol(format!(".LBB{number}_{at}"), 0)
                    }
                    Operand::Frame(_) => return Err("a frame index with no displacement".to_owned()),
                });
            }
            converted
        };
        match inst.kind {
            Kind::Copy => {
                let [mc::Operand::Reg(dst), mc::Operand::Reg(src)] = operands[..] else {
                    return Err("a copy of non-registers".to_owned());
                };
                if REGISTER_INFO.root(dst) != REGISTER_INFO.root(src) {
                    out.push(mc::Inst {
                        opcode: "MOV32rr",
                        operands: vec![mc::Operand::Reg(dst), mc::Operand::Reg(src)],
                    });
                }
            }
            Kind::Op("ADJCALLSTACKDOWN" | "ADJCALLSTACKUP") => {}
            Kind::Op("RET") => {
                let closing = if pushes { layout.pop_epilogue() } else { layout.epilogue() };
                closing.iter().for_each(|op| frame_op(op, &mut out));
                out.push(mc::Inst { opcode: "RTS", operands: vec![] });
            }
            Kind::Op("MOVM32pm_P") => {
                let [mc::Operand::Imm(disp), ref base, mc::Operand::Reg(src)] = operands[..] else {
                    return Err("MOVM32pm_P operands".to_owned());
                };
                let mask = mask_bit(REGISTER_INFO.root(src)).ok_or("a spill of a register no MOVEM names")?;
                out.push(mc::Inst {
                    opcode: "MOVM32pm",
                    operands: vec![mc::Operand::Imm(disp), base.clone(), mc::Operand::Imm(1 << mask)],
                });
            }
            Kind::Op("MOVM32mp_P") => {
                let [mc::Operand::Reg(dst), mc::Operand::Imm(disp), ref base] = operands[..] else {
                    return Err("MOVM32mp_P operands".to_owned());
                };
                let mask = mask_bit(REGISTER_INFO.root(dst)).ok_or("a reload into a register no MOVEM names")?;
                out.push(mc::Inst {
                    opcode: "MOVM32mp",
                    operands: vec![mc::Operand::Imm(1 << mask), mc::Operand::Imm(disp), base.clone()],
                });
            }
            Kind::Op("MOVI32ri") => {
                let [mc::Operand::Reg(dst), mc::Operand::Imm(value)] = operands[..] else {
                    return Err("MOVI32ri operands".to_owned());
                };
                out.extend(expand_movi(dst, value)?);
            }
            Kind::Op(name @ ("MOVI8di" | "MOVI16di")) => {
                let [mc::Operand::Reg(dst), mc::Operand::Imm(value)] = operands[..] else {
                    return Err(format!("{name} operands"));
                };
                out.extend(expand_small_movi(dst, value, name == "MOVI8di")?);
            }
            Kind::Op("MOVXd16d8" | "MOVXd32d8" | "MOVXd32d16") => {
                let [mc::Operand::Reg(dst), mc::Operand::Reg(src)] = operands[..] else {
                    return Err("MOVX operands".to_owned());
                };
                out.extend(expand_any_extension(dst, src)?);
            }
            Kind::Op(name) if extension(name).is_some() => {
                out.extend(expand_extension(name, &operands)?);
            }
            Kind::Op(name) => out.push(mc::Inst { opcode: name, operands }),
        }
    }
    lines.extend(out.into_iter().map(Line::Inst));
    // the registers a `ret` reads: the result, live at the `rts`
    let returned: Vec<RegId> = insts
        .iter()
        .filter(|inst| inst.kind == Kind::Op("RET"))
        .flat_map(|inst| &inst.operands)
        .filter_map(|operand| match operand {
            Operand::Use(Reg::Phys(register)) => Some(*register),
            _ => None,
        })
        .collect();
    if overlay {
        lines = lines
            .into_iter()
            .map(|line| match line {
                Line::Inst(inst) => Line::Inst(crate::overlay::rewrite(inst)),
                label => label,
            })
            .collect();
        lines = crate::overlay::merge_masks(lines);
        lines = crate::overlay::constant_into_destination(lines);
        lines = crate::overlay::boolean_by_negation(lines);
        lines = crate::overlay::narrow_immediates(lines);
        lines = crate::overlay::drop_redundant_tests(lines);
        lines = crate::overlay::drop_repeated_compares(lines);
        lines = crate::overlay::fold_post_increment(lines);
        lines = crate::chain::chain_post_increment(lines, &returned);
        lines = crate::chain::loop_post_increment(lines);
    }
    Ok(lines)
}

/// `ExpandMOVI` for a long: a clear, a `moveq` where the register is a data
/// one and the value fits, `moveq` and `not.b` for 128 to 255, `movea.w` for
/// an address register and a value under 64 KiB, else a `move.l`.
/// DEVIATE: in a data register `moveq` and `not.w` for 0xFF80 to 0xFFFF and
/// `moveq` and `swap` for a `moveq` value shifted up 16, 4 bytes and 8 clocks
/// where `move.l #imm` is 6 and 12.
fn expand_movi(
    dst: RegId,
    value: i64,
) -> Result<Vec<mc::Inst>, String> {
    use mc::Operand::{Imm, Reg as R};
    let address = mask_bit(REGISTER_INFO.root(dst)).ok_or("MOVI32ri into a register no move names")? >= 8;
    Ok(if value == 0 {
        if address {
            vec![inst("SUB32ar", vec![R(dst), R(dst), R(dst)])]
        } else {
            vec![inst("MOVQ", vec![R(dst), Imm(0)])]
        }
    } else if !address && (-128..=127).contains(&value) {
        vec![inst("MOVQ", vec![R(dst), Imm(value)])]
    } else if !address && (0..=255).contains(&value) {
        let low = REGISTER_INFO.view(REGISTER_INFO.root(dst), 8).ok_or("no byte view")?;
        vec![inst("MOVQ", vec![R(dst), Imm(!value & 0xFF)]), inst("NOT8d", vec![R(low), R(low)])]
    } else if !address && (0xFF80..=0xFFFF).contains(&value) {
        let word = REGISTER_INFO.view(REGISTER_INFO.root(dst), 16).ok_or("no word view")?;
        vec![inst("MOVQ", vec![R(dst), Imm(!value & 0xFFFF)]), inst("NOT16d", vec![R(word), R(word)])]
    } else if !address && value % 65536 == 0 && (1..=127).contains(&(value / 65536)) {
        vec![inst("MOVQ", vec![R(dst), Imm(value / 65536)]), inst("SWAP", vec![R(dst), R(dst)])]
    } else if address && (0..=65535).contains(&value) {
        let word = REGISTER_INFO.view(REGISTER_INFO.root(dst), 16).ok_or("no word view")?;
        vec![inst("MOV16ai", vec![R(word), Imm(value)])]
    } else {
        vec![inst("MOV32ri", vec![R(dst), Imm(value)])]
    })
}

/// `ExpandMOVI` for a byte or a word, in a data register: a clear for 0, a
/// `moveq` for a byte (the high bits do not matter) or a word that fits,
/// else `move.w #imm`.
fn expand_small_movi(
    dst: RegId,
    value: i64,
    byte: bool,
) -> Result<Vec<mc::Inst>, String> {
    use mc::Operand::{Imm, Reg as R};
    let root = REGISTER_INFO.root(dst);
    let long = REGISTER_INFO.view(root, 32).ok_or("no long view")?;
    let value = if byte { i64::from(value as i8) } else { i64::from(value as i16) };
    Ok(if value == 0 {
        vec![inst("MOVQ", vec![R(long), Imm(0)])]
    } else if byte || (-128..=127).contains(&value) {
        vec![inst("MOVQ", vec![R(long), Imm(value)])]
    } else {
        vec![inst("MOV16ri", vec![R(dst), Imm(value)])]
    })
}

/// `ExpandMOVX_RR`: the wider view of the source's register, moved unless it
/// is the destination already.
fn expand_any_extension(
    dst: RegId,
    src: RegId,
) -> Result<Vec<mc::Inst>, String> {
    use mc::Operand::Reg as R;
    let bits = if matches!(REGISTER_INFO.name(dst), Some(name) if name.starts_with("wd")) { 16 } else { 32 };
    let wide = REGISTER_INFO.view(REGISTER_INFO.root(src), bits).ok_or("no wide view of the source")?;
    Ok(if wide == dst {
        vec![]
    } else if bits == 16 {
        vec![inst("MOV16rr", vec![R(dst), R(wide)])]
    } else {
        vec![inst("MOV32rr", vec![R(dst), R(wide)])]
    })
}

/// `MOVZXd32p8`, `MOVSXd16d8`..: a zero or sign extending move as
/// `(signed, destination bits, addressing mode, source bits)`.
fn extension(name: &str) -> Option<(bool, u32, char, u32)> {
    let rest = name
        .strip_prefix("MOVZX")
        .map(|rest| (false, rest))
        .or_else(|| name.strip_prefix("MOVSX").map(|rest| (true, rest)))?;
    let (signed, rest) = rest;
    let rest = rest.strip_prefix('d')?;
    let split = rest.find(|c: char| c.is_ascii_alphabetic())?;
    let destination: u32 = rest[..split].parse().ok()?;
    let mode = rest[split..].chars().next()?;
    let source: u32 = rest[split + 1..].parse().ok()?;
    Some((signed, destination, mode, source))
}

fn inst(
    opcode: &'static str,
    operands: Vec<mc::Operand>,
) -> mc::Inst {
    mc::Inst { opcode, operands }
}

/// `ExpandMOVSZX_RR` and `ExpandMOVSZX_RM`: clear the destination and move
/// into its low part, or move and then extend (`AddSExt`, `AddZExt`).
///
/// DEVIATE: the `q` and `k` modes and 16-bit destinations are not translated.
/// Reason: nothing selected them yet. Exit: when a kernel needs them.
fn expand_extension(
    name: &str,
    operands: &[mc::Operand],
) -> Result<Vec<mc::Inst>, String> {
    use mc::Operand::{Imm, Reg as R};
    let (signed, to, mode, from) = extension(name).ok_or("not an extension")?;
    let R(dst) = operands[0] else { return Err(format!("{name}: operands")) };
    let root = REGISTER_INFO.root(dst);
    let view = |bits| REGISTER_INFO.view(root, bits).ok_or_else(|| format!("{name}: no {bits}-bit view"));
    let mut out = Vec::new();
    let extend = |out: &mut Vec<mc::Inst>| -> Result<(), String> {
        if signed {
            if from == 8 {
                let word = view(16)?;
                out.push(inst("EXT16", vec![R(word), R(word)]));
            }
            if to == 32 {
                out.push(inst("EXT32", vec![R(view(32)?), R(view(32)?)]));
            }
        } else {
            let mask = if from == 8 { 0xFF } else { 0xFFFF };
            let opcode = if to == 16 { "AND16di" } else { "AND32di" };
            out.push(inst(opcode, vec![R(dst), R(dst), Imm(mask)]));
        }
        Ok(())
    };
    if mode == 'd' {
        let R(source) = operands[1] else { return Err(format!("{name}: operands")) };
        if REGISTER_INFO.root(source) == root {
            extend(&mut out)?;
        } else if signed {
            out.push(inst(
                if to == 16 { "MOV16dd" } else { "MOV32dd" },
                vec![R(dst), R(REGISTER_INFO.view(REGISTER_INFO.root(source), to).ok_or("no view")?)],
            ));
            extend(&mut out)?;
        } else {
            out.push(inst("MOVQ", vec![R(dst), Imm(0)]));
            out.push(inst(if from == 8 { "MOV8dd" } else { "MOV16dd" }, vec![R(view(from)?), R(source)]));
        }
        return Ok(out);
    }
    let load: &'static str = match (from, mode) {
        (8, 'j') => "MOV8dj",
        (8, 'p') => "MOV8dp",
        (8, 'f') => "MOV8df",
        (16, 'j') => "MOV16rj",
        (16, 'p') => "MOV16rp",
        (16, 'f') => "MOV16rf",
        _ => return Err(format!("{name}: addressing mode {mode} is not translated")),
    };
    let reads_dst = operands[1..].iter().any(|one| matches!(one, R(reg) if REGISTER_INFO.root(*reg) == root));
    let mut moved = operands.to_vec();
    moved[0] = R(view(from)?);
    if !signed && !reads_dst {
        out.push(inst("MOVQ", vec![R(dst), Imm(0)]));
        out.push(inst(load, moved));
    } else {
        out.push(inst(load, moved));
        extend(&mut out)?;
    }
    Ok(out)
}
