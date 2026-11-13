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

pub(crate) fn is_address(register: RegId) -> bool {
    mask_bit(REGISTER_INFO.root(register)).is_some_and(|bit| bit >= 8)
}

/// `%sp` moved by `bytes`: the pin's `adda.l`/`suba.l` (6 bytes), or where
/// `short` `addq` (1 to 8, 2 bytes) or `lea (d16,%sp), %sp` (4 bytes). No
/// `subq` to an address register has a row yet (`SUBQ32ai` is not built).
pub(crate) fn sp_adjust(
    bytes: i64,
    short: bool,
) -> mc::Inst {
    use mc::Operand::{Imm, Reg as R};
    let sp = || R(reg::SP);
    match bytes {
        1..=8 if short => mc::Inst { opcode: "ADDQ32ai", operands: vec![sp(), sp(), Imm(bytes)] },
        -32768..=32767 if short => mc::Inst { opcode: "LEA32p", operands: vec![sp(), Imm(bytes), sp()] },
        0.. => mc::Inst { opcode: "ADD32ai", operands: vec![sp(), sp(), Imm(bytes)] },
        _ => mc::Inst { opcode: "SUB32ai", operands: vec![sp(), sp(), Imm(-bytes)] },
    }
}

fn frame_op(
    op: &FrameOp,
    out: &mut Vec<mc::Inst>,
    short: bool,
) {
    use mc::Operand::{Imm, Reg as R};
    let base = |base: &Base| R(if *base == Base::Sp { reg::SP } else { reg::A6 });
    let mask = |registers: &[RegId]| {
        registers.iter().filter_map(|one| mask_bit(REGISTER_INFO.root(*one))).fold(0i64, |all, bit| all | 1 << bit)
    };
    // one `move.l` a register, in `movem`'s memory order: the registers of the
    // highest address are pushed first, and popped last
    let order = |registers: &[RegId]| {
        let mut ranked: Vec<(u32, RegId)> =
            registers.iter().filter_map(|one| mask_bit(REGISTER_INFO.root(*one)).map(|bit| (bit, *one))).collect();
        ranked.sort_by_key(|(bit, _)| *bit);
        ranked
    };
    match op {
        FrameOp::PushEach { registers } => {
            for (_, one) in order(registers).into_iter().rev() {
                let opcode = if is_address(one) { "MOV32ea" } else { "MOV32ed" };
                out.push(mc::Inst { opcode, operands: vec![R(reg::SP), R(one)] });
            }
            return;
        }
        FrameOp::PopEach { registers } => {
            for (_, one) in order(registers) {
                let opcode = if is_address(one) { "MOV32ao" } else { "MOV32do" };
                out.push(mc::Inst { opcode, operands: vec![R(one), R(reg::SP)] });
            }
            return;
        }
        // `BuildStackAlignAND`: `and` takes no address register, so
        // through `%d0`
        FrameOp::AlignSp(align) => {
            out.push(mc::Inst { opcode: "MOV32rr", operands: vec![R(reg::D0), R(reg::SP)] });
            out.push(mc::Inst { opcode: "AND32di", operands: vec![R(reg::D0), R(reg::D0), Imm(-align)] });
            out.push(mc::Inst { opcode: "MOV32rr", operands: vec![R(reg::SP), R(reg::D0)] });
            return;
        }
        _ => {}
    }
    out.push(match op {
        FrameOp::PushEach { .. } | FrameOp::PopEach { .. } => unreachable!("handled above"),
        FrameOp::Link { bytes } => mc::Inst { opcode: "LINK16", operands: vec![R(reg::A6), Imm(-bytes)] },
        FrameOp::Unlink => mc::Inst { opcode: "UNLK", operands: vec![R(reg::A6)] },
        FrameOp::AlignSp(_) => unreachable!("handled above"),
        FrameOp::SubSp(bytes) => sp_adjust(-bytes, short),
        FrameOp::AddSp(bytes) => sp_adjust(*bytes, short),
        FrameOp::Save { registers, base: at, offset } => {
            mc::Inst { opcode: "MOVM32pm", operands: vec![Imm(*offset), base(at), Imm(mask(registers))] }
        }
        FrameOp::Restore { registers, base: at, offset } => {
            mc::Inst { opcode: "MOVM32mp", operands: vec![Imm(mask(registers)), Imm(*offset), base(at)] }
        }
        FrameOp::Push { registers } => match registers[..] {
            [one] => {
                let opcode = if is_address(one) { "MOV32ea" } else { "MOV32ed" };
                mc::Inst { opcode, operands: vec![R(reg::SP), R(one)] }
            }
            _ => mc::Inst { opcode: "MOVM32em", operands: vec![R(reg::SP), Imm(mask(registers))] },
        },
        FrameOp::Pop { registers } => match registers[..] {
            [one] => {
                let opcode = if is_address(one) { "MOV32ao" } else { "MOV32do" };
                mc::Inst { opcode, operands: vec![R(one), R(reg::SP)] }
            }
            _ => mc::Inst { opcode: "MOVM32mo", operands: vec![Imm(mask(registers)), R(reg::SP)] },
        },
    });
}

/// Bytes and clocks of `ops` on `cpu`, from the encoder and the manuals'
/// tables.
pub fn frame_cost(
    ops: &[FrameOp],
    cpu: crate::dagisel::hooks::Cpu,
) -> Result<(usize, u32), String> {
    frame_cost_with(ops, cpu, false)
}

/// [`frame_cost`] with the short stack-pointer adjusts where `short`.
pub fn frame_cost_with(
    ops: &[FrameOp],
    cpu: crate::dagisel::hooks::Cpu,
    short: bool,
) -> Result<(usize, u32), String> {
    let mut insts = Vec::new();
    ops.iter().for_each(|op| frame_op(op, &mut insts, short));
    let (mut bytes, mut clocks) = (0, 0);
    for inst in &insts {
        let size = crate::encode::encode(inst)?.bytes.len();
        bytes += size;
        clocks += crate::timing::cost(cpu, &mc::print(inst)?, size as u32, false)?.clocks;
    }
    Ok((bytes, clocks))
}

/// The prologue and the epilogue: the pin's (`suba.l`, then `movem` into
/// slots), or the saved registers pushed and popped, whichever of the two is
/// no larger and no slower on `cpu` in each; the pin's otherwise.
fn frame_ops(
    layout: &frame::Layout,
    cpu: crate::dagisel::hooks::Cpu,
    push_frame: bool,
    short: bool,
    size: bool,
) -> Result<(Vec<FrameOp>, Vec<FrameOp>), String> {
    frame_ops_choosing(
        layout,
        push_frame,
        true,
        |ops| frame_cost_with(ops, cpu, short),
        |bytes, clocks| crate::timing::order(cpu, size, clocks, bytes as u32),
    )
}

/// [`frame_ops`] with the price of a sequence, `(bytes, clocks)`, given: a test
/// makes the push dearer to see the pin's frame chosen.
pub fn frame_ops_priced(
    layout: &frame::Layout,
    push_frame: bool,
    cost: impl Fn(&[FrameOp]) -> Result<(usize, u32), String>,
) -> Result<(Vec<FrameOp>, Vec<FrameOp>), String> {
    frame_ops_choosing(layout, push_frame, false, cost, |_, clocks| (0, i64::from(clocks)))
}

/// [`frame_ops_priced`] that also weighs a `move.l` a register (`each`) beside
/// the one `movem`: of the frames no larger and no slower than the pin's, the
/// one `key(bytes, clocks)` of its two halves ranks lowest, the `movem` push
/// on a tie.
///
/// DEVIATE from the pin, which has neither: gcc pushes each register. At two
/// registers the bytes are equal and the clocks lower from the 68020 on
/// (68020: `movem` pair 34, four `move.l` 22; 68040 9 against 4). From three
/// registers `movem` is smaller and stays (fnptr +4 B, partition 040 +8 B in
/// the first A/B). Exit: none.
pub fn frame_ops_choosing(
    layout: &frame::Layout,
    push_frame: bool,
    each: bool,
    cost: impl Fn(&[FrameOp]) -> Result<(usize, u32), String>,
    key: impl Fn(usize, u32) -> (i64, i64),
) -> Result<(Vec<FrameOp>, Vec<FrameOp>), String> {
    let (pin_opening, pin_closing) = (layout.prologue(), layout.epilogue());
    let mut pushed = Vec::new();
    if push_frame {
        pushed.extend(layout.push_prologue().zip(layout.push_epilogue()));
        if each {
            pushed.extend(layout.push_each_prologue().zip(layout.push_each_epilogue()));
        }
    }
    let no_worse = |ours: &[FrameOp], pin: &[FrameOp]| -> Result<bool, String> {
        let ((ours_bytes, ours_clocks), (pin_bytes, pin_clocks)) = (cost(ours)?, cost(pin)?);
        Ok(ours_bytes <= pin_bytes && ours_clocks <= pin_clocks)
    };
    let mut best: Option<((i64, i64), (Vec<FrameOp>, Vec<FrameOp>))> = None;
    // the first candidate that passes is the `movem` push; a later one must not
    // be larger than it (strict on bytes: no m68k byte rate is derived)
    let mut bytes_limit = usize::MAX;
    for (opening, closing) in pushed {
        if !(no_worse(&opening, &pin_opening)? && no_worse(&closing, &pin_closing)?) {
            continue;
        }
        let ((ob, oc), (cb, cc)) = (cost(&opening)?, cost(&closing)?);
        if ob + cb > bytes_limit {
            continue;
        }
        bytes_limit = bytes_limit.min(ob + cb);
        let ranked = key(ob + cb, oc + cc);
        if best.as_ref().is_none_or(|(held, _)| ranked < *held) {
            best = Some((ranked, (opening, closing)));
        }
    }
    Ok(best.map_or((pin_opening, pin_closing), |(_, frame)| frame))
}

/// A line of the function: a block's label (its layout index) or an
/// instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Line {
    Label(usize),
    Inst(mc::Inst),
    /// The source place of the instructions that follow, until the next one.
    Loc(Row),
}

/// A line-table row: the place and the flags the pin sets on it
/// (`DwarfDebug::beginInstruction`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Row {
    pub place: llrm_codegen::model::lir::SrcLoc,
    /// The first row after the frame is made.
    pub prologue_end: bool,
    /// The row of the instruction that takes the frame down.
    pub epilogue_begin: bool,
    /// The first row of a block: a statement even on the line before it.
    pub block_top: bool,
}

/// `out` and the places that start in it (`(index into out, place)`) as
/// lines, a place ahead of the instruction it starts at.
fn flush(
    out: &mut Vec<mc::Inst>,
    marks: &mut Vec<(usize, Row)>,
    lines: &mut Vec<Line>,
) {
    let mut marks = std::mem::take(marks).into_iter().peekable();
    for (index, inst) in out.drain(..).enumerate() {
        while let Some((_, place)) = marks.next_if(|(at, _)| *at <= index) {
            lines.push(Line::Loc(place));
        }
        lines.push(Line::Inst(inst));
    }
    lines.extend(marks.map(|(_, place)| Line::Loc(place)));
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
    cpu: crate::dagisel::hooks::Cpu,
    push_frame: bool,
) -> Result<Vec<Line>, String> {
    finish_with(function, assignment, number, slots, cpu, push_frame, false, None)
}

/// What the frame is laid out from, and so what a frame index is at.
pub(crate) struct FrameInputs {
    fixed: Vec<i32>,
    objects: Vec<Object>,
    used: Vec<RegId>,
    has_calls: bool,
    short: bool,
    /// `MachineFrameInfo::hasVarSizedObjects`: the body writes the stack
    /// pointer (a dynamic `alloca`).
    has_var_sized: bool,
    /// The frame indices, in the order of `objects`.
    pub(crate) frame_indices: Vec<i32>,
}

impl FrameInputs {
    pub(crate) fn of(
        insts: &[&crate::mfunc::Inst],
        function: &MFunction,
        assignment: &Assignment,
        slots: &[i32],
        short: bool,
    ) -> Self {
        // a tail call has no call sequence: it leaves the frame it is in
        let has_calls = insts.iter().any(|inst| inst.call && inst.kind != Kind::Op("TCRETURNq"));
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
        let frame_indices: Vec<i32> = fixed
            .iter()
            .copied()
            .chain(function.locals.iter().map(|local| local.0))
            .chain(slots.iter().copied())
            .collect();
        // the pin sets it where a dynamic alloca is built (`visitAlloca`); here
        // the live body shows it
        let has_var_sized = insts
            .iter()
            .any(
                |inst| inst.kind == Kind::Copy
                    && matches!(
                        inst.operands.first(),
                        Some(Operand::Def(r)) if physical(*r, assignment) == reg::SP
                    ),
            );
        FrameInputs { fixed, objects, used, has_calls, short, has_var_sized, frame_indices }
    }

    /// The laid-out frame for a call area of `max_call_frame` bytes.
    pub(crate) fn layout(
        &self,
        max_call_frame: i64,
    ) -> frame::Layout {
        frame::layout(&Function {
            fixed: self.fixed.len(),
            objects: self.objects.clone(),
            saved: self.used.clone(),
            max_call_frame,
            stack_align: if self.short { frame::SHIPPED_STACK_ALIGN } else { 0 },
            adjusts_stack: self.has_calls,
            has_var_sized: self.has_var_sized,
            ..Function::default()
        })
    }
}

/// The largest `ADJCALLSTACKDOWN` among `insts` whose sequence `kept` keeps.
pub(crate) fn call_area(
    insts: &[&crate::mfunc::Inst],
    kept: &dyn Fn(usize) -> bool,
) -> i64 {
    insts
        .iter()
        .enumerate()
        .filter(|(at, inst)| inst.kind == Kind::Op("ADJCALLSTACKDOWN") && kept(*at))
        .filter_map(|(_, inst)| match inst.operands.first() {
            Some(Operand::Imm(bytes)) => Some(*bytes),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// [`finish`], with the outgoing arguments pushed where [`crate::callpush`]
/// finds that cheaper: `pushes` is `Some(price)` to allow it.
pub fn finish_with(
    function: &MFunction,
    assignment: &Assignment,
    number: usize,
    slots: &[i32],
    cpu: crate::dagisel::hooks::Cpu,
    push_frame: bool,
    short: bool,
    pushes: Option<crate::timing::Price>,
) -> Result<Vec<Line>, String> {
    if let Some(folded) = crate::frameindexed::fold(function, assignment, slots, short) {
        // the fold proved its range for the stack depth it could foresee; call
        // cleanups left owed grow the depth, which only emission knows:
        // a function that overflows keeps its `lea`s
        match finish_with(&folded, assignment, number, slots, cpu, push_frame, short, pushes) {
            Err(why) if why == crate::frameindexed::OUT_OF_RANGE => {}
            done => return done,
        }
    }
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
    let blocks: Vec<usize> = kept.iter().map(|(at, _)| *at).collect();
    let targeted: Vec<usize> = insts
        .iter()
        .flat_map(|inst| &inst.operands)
        .filter_map(|operand| match operand {
            Operand::Block(id) => function.blocks.iter().position(|block| block.id == *id),
            _ => None,
        })
        .collect();
    let inputs = FrameInputs::of(&insts, function, assignment, slots, short);
    let frame_indices = &inputs.frame_indices;
    let call_area = |kept_sequences: &dyn Fn(usize) -> bool| call_area(&insts, kept_sequences);
    let frame_of = |max_call_frame: i64| inputs.layout(max_call_frame);
    let wants_size = pushes == Some(crate::timing::Price::Size);
    let pushes = match pushes {
        Some(price) => crate::callpush::plan(&insts, &blocks, assignment, cpu, price, short, |max_call_frame| {
            let (opening, closing) = frame_ops(&frame_of(max_call_frame), cpu, push_frame, short, wants_size)?;
            frame_cost_with(&[opening, closing].concat(), cpu, short)
        })?,
        None => crate::callpush::Plan::default(),
    };
    let layout = frame_of(call_area(&|at| pushes.keeps_frame(at)));
    let mut out = Vec::new();
    let (opening, closing) = frame_ops(&layout, cpu, push_frame, short, wants_size)?;
    opening.iter().for_each(|op| frame_op(op, &mut out, short));
    let mut lines = Vec::new();
    // Where each place starts in `out`: only when it changes.
    let mut marks: Vec<(usize, Row)> = Vec::new();
    // The first row with a line ends the prologue, whether or not the opening
    // ops made a frame (the pin flags a frameless function's too; #7199 made
    // this one frameless).
    let mut frame_made = true;
    let mut last_origin = 0u32;
    let mut block_top = false;
    // the bytes a `CCR_PUSH` has on the stack: a slot named from the stack
    // pointer is that much further
    let mut pushed = 0i64;
    // the bytes of finished calls' arguments still on the stack: their cleanups
    // wait for the next instruction that needs the stack pointer as it is
    let mut owed = 0i64;
    let mut last_block = 0usize;
    // the values of the stores whose pushes wait for the call, by offset
    let mut waiting: Vec<(i64, usize)> = Vec::new();
    let blocks: Vec<usize> = kept.iter().map(|(at, _)| *at).collect();
    let mut block_of = blocks.iter().copied().peekable();
    let mut shown = 0usize;
    // the operands of `inst` as the printer has them, with `pushed` bytes
    // more on the stack than the frame accounts for
    let convert = |inst: &crate::mfunc::Inst, pushed: i64| -> Result<Vec<mc::Operand>, String> {
        let mut converted = Vec::new();
        let indexed_base = match inst.kind {
            Kind::Op(name) => crate::scaledindex::memory_at(name).map(|(base, _)| base),
            Kind::Copy => None,
        };
        let mut it = inst.operands.iter().enumerate().peekable();
        while let Some((position, operand)) = it.next() {
            converted.push(match operand {
                Operand::Def(r) | Operand::Use(r) => mc::Operand::Reg(physical(*r, assignment)),
                Operand::Imm(value) => {
                    // a frame index follows the displacement it adds to
                    if let Some((_, Operand::Frame(index))) = it.peek() {
                        let at = frame_indices.iter().position(|one| one == index).ok_or("an unknown frame index")?;
                        let (base, disp) = layout.address(at);
                        it.next();
                        let shown = value + disp + if base == Base::Sp { pushed } else { 0 };
                        // the displacement of an indexed operand is 8 bits;
                        // `pushed` is known only here
                        if indexed_base == Some(position + 1) && i8::try_from(shown).is_err() {
                            return Err(crate::frameindexed::OUT_OF_RANGE.to_owned());
                        }
                        converted.push(mc::Operand::Imm(shown));
                        converted.push(mc::Operand::Reg(if base == Base::Sp { reg::SP } else { reg::A6 }));
                        continue;
                    }
                    mc::Operand::Imm(*value)
                }
                // The table and blocks of a table jump are for the passes and
                // the emitted table; the instruction is `jmp (An)`.
                Operand::Symbol(..) | Operand::Block(_) if inst.kind == Kind::Op("JMP32j") => continue,
                Operand::Symbol(name, addend) => mc::Operand::Symbol(name.clone(), *addend),
                Operand::Block(id) => {
                    let at = function.blocks.iter().position(|block| block.id == *id).ok_or("a branch to no block")?;
                    mc::Operand::Symbol(format!(".LBB{number}_{at}"), 0)
                }
                Operand::Frame(_) => return Err("a frame index with no displacement".to_owned()),
            });
        }
        Ok(converted)
    };
    // the push of argument store `store`, `pushed` bytes down: of its value,
    // or of the memory the load folded into it reads
    let push_for = |store: usize, pushed: i64| -> Result<mc::Inst, String> {
        if let Some((base, offset)) = pushes.address_push(store) {
            return Ok(crate::callpush::address_push(base, offset));
        }
        match pushes.folded_load(store) {
            Some(load) => {
                let Kind::Op(name) = insts[load].kind else { return Err("a folded load is an instruction".to_owned()) };
                let mut operands = convert(insts[load], pushed)?;
                operands[0] = mc::Operand::Reg(reg::SP);
                let opcode = crate::callpush::memory_push(name).ok_or("a folded load has a push")?;
                Ok(mc::Inst { opcode, operands })
            }
            None if matches!(insts[store].kind, Kind::Op("CLR32p" | "CLR32j")) => {
                // a `clr.l` store has no value operand: it stores zero
                Ok(crate::callpush::push_of_sized(&mc::Operand::Imm(0), wants_size))
            }
            None => {
                let operands = convert(insts[store], pushed)?;
                Ok(crate::callpush::push_of_sized(operands.last().ok_or("a push of nothing")?, wants_size))
            }
        }
    };
    // Whether the stack pointer must be as the frame has it before the call
    // that takes the arguments pushed at `from`: a block change or such an
    // instruction comes first, and the cleanups owed would then pop these
    // arguments instead of the earlier call's.
    let pays_before_call = |from: usize| -> bool {
        for next in from..insts.len() {
            if blocks[next] != blocks[from] {
                return true;
            }
            let inst = insts[next];
            if inst.call {
                return false;
            }
            if pushes.is_dropped(next)
                || pushes.is_push(next)
                || pushes.is_down(next)
                || pushes.deferred_offset(next).is_some()
                || pushes.cleanup(next).is_some()
            {
                continue;
            }
            if inst.terminator
                || inst.barrier
                || matches!(
                    inst.kind,
                    Kind::Op("RET" | "TCRETURNq" | "UNREACHABLE" | "ADJCALLSTACKDOWN")
                )
                || crate::callpush::names_sp(inst, assignment)
            {
                return true;
            }
        }
        true
    };
    for (index, inst) in insts.iter().copied().enumerate() {
        let at = block_of.next().expect("a block per instruction");
        if owed > 0 && at != last_block {
            // a block is entered with the stack as its predecessors left it
            frame_op(&FrameOp::AddSp(owed), &mut out, short);
            pushed -= owed;
            owed = 0;
        }
        last_block = at;
        while shown <= at {
            if targeted.contains(&shown) {
                flush(&mut out, &mut marks, &mut lines);
                lines.push(Line::Label(shown));
                last_origin = 0;
                block_top = true;
            }
            shown += 1;
        }
        let place = (inst.origin != 0).then(|| function.places.get(inst.origin as usize - 1).copied()).flatten();
        let returns = inst.kind == Kind::Op("RET");
        // A block's first instruction without a place stands under line 0.
        let place = place.or(block_top.then(|| llrm_codegen::model::lir::SrcLoc::line(0)));
        if let Some(place) = place
            && (inst.origin != last_origin || returns || frame_made || block_top)
        {
            let row = Row { place, prologue_end: frame_made && place.line != 0, epilogue_begin: returns, block_top };
            marks.push((out.len(), row));
            frame_made = frame_made && place.line == 0;
        }
        block_top = false;
        if inst.origin != 0 {
            last_origin = inst.origin;
        }
        if owed > 0
            && (pushes.is_push(index) || (pushes.flushes(index) && !waiting.is_empty()))
            && pays_before_call(index)
        {
            frame_op(&FrameOp::AddSp(owed), &mut out, short);
            pushed -= owed;
            owed = 0;
        }
        if pushes.flushes(index) {
            waiting.sort_by(|a, b| b.0.cmp(&a.0));
            for (_, store) in waiting.drain(..) {
                out.push(push_for(store, pushed)?);
                pushed += 4;
            }
        }
        let operands = convert(inst, pushed)?;
        if pushes.is_dropped(index) {
            continue;
        }
        if let Some(offset) = pushes.deferred_offset(index) {
            waiting.push((offset, index));
            continue;
        }
        if pushes.is_push(index) {
            out.push(push_for(index, pushed)?);
            pushed += 4;
            continue;
        }
        if pushes.is_down(index) {
            continue;
        }
        if let Some(load) = pushes.fused_load(index) {
            // a kept store that moves what the load read: the store's address,
            // then the load's memory
            let (Kind::Op(from), Kind::Op(to)) = (insts[load].kind, inst.kind) else {
                return Err("a fused store and load are instructions".to_owned());
            };
            let opcode = crate::callpush::memory_store(from, to).ok_or("a fused load has a store")?;
            let mut moved = operands;
            moved.pop();
            moved.extend(convert(insts[load], pushed)?.into_iter().skip(1));
            out.push(mc::Inst { opcode, operands: moved });
            continue;
        }
        if let Some(bytes) = pushes.cleanup(index) {
            if function.returns_twice {
                frame_op(&FrameOp::AddSp(bytes), &mut out, short);
                pushed -= bytes;
            } else {
                owed += bytes;
            }
            continue;
        }
        // an instruction that reads the stack pointer as the frame has it, or
        // leaves the block, finds the cleanups done
        if owed > 0
            && (inst.terminator
                || inst.barrier
                || matches!(
                    inst.kind,
                    Kind::Op("RET" | "TCRETURNq" | "UNREACHABLE" | "ADJCALLSTACKDOWN")
                )
                || crate::callpush::names_sp(inst, assignment))
        {
            frame_op(&FrameOp::AddSp(owed), &mut out, short);
            pushed -= owed;
            owed = 0;
        }
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
            // `copyPhysReg` of `CCR`: the 68000 reads the codes with `move from
            // SR`, which is privileged from the 68010 on, where `move from CCR`
            // takes its place
            Kind::Op("CCR_SAVE") => {
                let [mc::Operand::Reg(dst), _] = operands[..] else { return Err("CCR_SAVE operands".to_owned()) };
                let word = REGISTER_INFO.view(REGISTER_INFO.root(dst), 16).ok_or("no word view")?;
                let (opcode, from) = match cpu {
                    crate::dagisel::hooks::Cpu::M68000 => ("MOV16ds", mc::Operand::Symbol("%sr".to_owned(), 0)),
                    _ => ("MOV16dc", mc::Operand::Reg(reg::CCR)),
                };
                out.push(mc::Inst { opcode, operands: vec![mc::Operand::Reg(word), from] });
            }
            Kind::Op("CCR_PUSH") => {
                let (opcode, from) = match cpu {
                    crate::dagisel::hooks::Cpu::M68000 => ("MOV16es", mc::Operand::Symbol("%sr".to_owned(), 0)),
                    _ => ("MOV16ec", mc::Operand::Reg(reg::CCR)),
                };
                out.push(mc::Inst { opcode, operands: vec![mc::Operand::Reg(reg::SP), from] });
                pushed += 2;
            }
            Kind::Op("CCR_POP") => {
                out.push(mc::Inst {
                    opcode: "MOV16co",
                    operands: vec![mc::Operand::Reg(reg::CCR), mc::Operand::Reg(reg::SP)],
                });
                pushed -= 2;
            }
            Kind::Op("CCR_RESTORE") => {
                let [_, mc::Operand::Reg(src)] = operands[..] else { return Err("CCR_RESTORE operands".to_owned()) };
                let word = REGISTER_INFO.view(REGISTER_INFO.root(src), 16).ok_or("no word view")?;
                out.push(mc::Inst {
                    opcode: "MOV16cd",
                    operands: vec![mc::Operand::Reg(reg::CCR), mc::Operand::Reg(word)],
                });
            }
            Kind::Op("ADJCALLSTACKDOWN" | "ADJCALLSTACKUP") => {}
            Kind::Op("UNREACHABLE") => {}
            Kind::Op("RET") => {
                closing.iter().for_each(|op| frame_op(op, &mut out, short));
                out.push(mc::Inst { opcode: "RTS", operands: vec![] });
            }
            Kind::Op("TCRETURNq") => {
                let [target, _] = &operands[..] else { return Err("TCRETURNq operands".to_owned()) };
                closing.iter().for_each(|op| frame_op(op, &mut out, short));
                out.push(mc::Inst { opcode: "BRA16", operands: vec![target.clone()] });
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
            Kind::Op(name)
                if extension(name).is_some()
                    || crate::dagisel::scaled::base_of(name).is_some_and(|base| extension(base).is_some()) =>
            {
                out.extend(expand_extension(name, &operands, cpu)?);
            }
            Kind::Op(name) if crate::dagisel::indexed::extension_of(name).is_some() => {
                let (_, def, count) = crate::dagisel::indexed::printed_as(name).ok_or("a writeback")?;
                let mut operands = operands;
                for _ in 0..count {
                    operands.remove(def);
                }
                let plain = crate::dagisel::indexed::extension_of(name).ok_or("an extension writeback")?;
                out.extend(expand_extension(plain, &operands, cpu)?);
            }
            Kind::Op(name) if crate::dagisel::indexed::printed_as(name).is_some() => {
                let (opcode, def, count) = crate::dagisel::indexed::printed_as(name).ok_or("a writeback")?;
                let mut operands = operands;
                for _ in 0..count {
                    operands.remove(def);
                }
                out.push(mc::Inst { opcode, operands });
            }
            Kind::Op(name) => out.push(mc::Inst { opcode: name, operands }),
        }
    }
    flush(&mut out, &mut marks, &mut lines);
    Ok(lines)
}

/// `ExpandMOVI` for a long: a clear, a `moveq` where the register is a data
/// one and the value fits, `moveq` and `not.b` for 128 to 255, `movea.w` for
/// an address register and a value under 32 KiB, else a `move.l`.
/// DEVIATE from the pin, whose test is `isUInt<16>`: `movea.w` sign-extends, so
/// 32768 to 65535 came out as 0xFFFF8000 to 0xFFFFFFFF. Exit: the pin tests
/// `isInt<16>` (as a signed value).
pub fn expand_movi(
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
    } else if address && (0..=32767).contains(&value) {
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
pub(crate) fn extension(name: &str) -> Option<(bool, u32, char, u32)> {
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

/// The plain load of `from` bits (8 or 16) through addressing mode `mode`:
/// the row an extending load of that mode moves with.
pub(crate) fn plain_load(
    from: u32,
    mode: char,
) -> Option<&'static str> {
    Some(match (from, mode) {
        (8, 'o') => "MOV8do",
        (8, 'j') => "MOV8dj",
        (8, 'p') => "MOV8dp",
        (8, 'f') => "MOV8df",
        (8, 'q') => "MOV8dq",
        (8, 'k') => "MOV8dk",
        (16, 'o') => "MOV16do",
        (16, 'j') => "MOV16rj",
        (16, 'p') => "MOV16rp",
        (16, 'f') => "MOV16rf",
        (16, 'q') => "MOV16rq",
        (16, 'k') => "MOV16rk",
        _ => return None,
    })
}

/// `ExpandMOVSZX_RR` and `ExpandMOVSZX_RM`: clear the destination and move
/// into its low part, or move and then extend (`AddSExt`, `AddZExt`).
///
/// DEVIATE: 16-bit destinations are not translated. Reason: nothing selected
/// them yet. Exit: when a kernel needs them.
fn expand_extension(
    name: &str,
    operands: &[mc::Operand],
    cpu: crate::dagisel::hooks::Cpu,
) -> Result<Vec<mc::Inst>, String> {
    use mc::Operand::{Imm, Reg as R};
    // `MOVZXd32f16x2`: the load is the scaled sibling of `MOV16rf`
    let (name, scale) = match crate::dagisel::scaled::base_of(name) {
        Some(base) => (base, crate::dagisel::scaled::scale_of(name)),
        None => (name, None),
    };
    let (signed, to, mode, from) = extension(name).ok_or("not an extension")?;
    let R(dst) = operands[0] else { return Err(format!("{name}: operands")) };
    let root = REGISTER_INFO.root(dst);
    let view = |bits| REGISTER_INFO.view(root, bits).ok_or_else(|| format!("{name}: no {bits}-bit view"));
    let mut out = Vec::new();
    let extend = |out: &mut Vec<mc::Inst>| -> Result<(), String> {
        if signed {
            // DEVIATE: `extb.l` from the 68020 on, where the pin has `ext.w`
            // then `ext.l`; gcc -m68020 uses it. Exit: beyond pin.
            if from == 8 && to == 32 && cpu.at_least(crate::dagisel::hooks::Cpu::M68020) {
                out.push(inst("EXTB32", vec![R(view(32)?), R(view(32)?)]));
                return Ok(());
            }
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
    let load = plain_load(from, mode).ok_or_else(|| format!("{name}: addressing mode {mode} is not translated"))?;
    let load = match scale {
        Some(scale) => {
            crate::dagisel::scaled::scaled_name(load, scale).ok_or_else(|| format!("{name}: no scaled {load}"))?
        }
        None => load,
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
