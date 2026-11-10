//! `lea (d,%sp), %a0; move.l %d0, (0,%a0,%d1)` is `move.l %d0, (d,%sp,%d1)`: a
//! stack array indexed by a register is addressed from the stack pointer in
//! one instruction.
//!
//! # How it works
//! [`fold`] runs after allocation, where the frame is known. A register
//! defined once by `lea` of a frame index whose every use is the base of an
//! indexed operand takes the frame index there, and the `lea` goes. The base
//! displacement of an indexed operand is 8 bits, so the fold needs the final
//! displacement (the object's, the operand's and the bytes pushed ahead of it)
//! to fit in -128..=127: [`Bounds`] gives the range it can have, from the
//! frame laid out before the fold with room for the arguments a call pushes.
//! The depth also grows by cleanups `expand` leaves owed across calls, which
//! only emission sees: it checks the final displacement ([`OUT_OF_RANGE`]) and
//! emits the function again without the fold.
//!
//! DEVIATE: the pin forms the indexed address from a register holding the
//! `lea`, and `getFrameIndexAddress` drops an index beside a frame index
//! (`a[i]` of a stack array stored to `a[0]`; see `dagisel::addr`). Reason:
//! the `lea` is re-formed per access (a call clobbers its register) and costs
//! 4 B and 8 clk each; gcc folds it. Exit: none; a 68020 base displacement
//! (`(bd,%sp,Xn)`) would lift the range, and its encoding is not translated.

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::expand::{FrameInputs, call_area};
use crate::frame::Base;
use crate::mfunc::{Inst, MFunction};
use crate::regalloc::Assignment;
use crate::scaledindex::memory_at;

/// What emission says when a folded operand's displacement left eight bits: the
/// stack pointer was further down than [`Bounds`] foresaw (call cleanups owed),
/// and the function is emitted again without the fold.
pub const OUT_OF_RANGE: &str = "an indexed displacement beyond eight bits";

/// The bytes beyond the laid-out displacement of a slot that the stack pointer
/// can be further down when it is read: a call's arguments still pushed when
/// the next call's are being pushed (`2 * calls`), a saved condition code (4),
/// and the rounding of a frame the fold shrinks (8).
pub fn margin(calls: i64) -> i64 {
    2 * calls + 4 + 8
}

/// Whether a displacement that can be anywhere in `low..=high` is an indexed
/// operand's 8-bit one.
pub fn fits(
    low: i64,
    high: i64,
) -> bool {
    i8::try_from(low).is_ok() && i8::try_from(high).is_ok()
}

/// The range of the displacement of a frame index from its frame register,
/// over every stack depth the code reaches it at.
pub struct Bounds {
    inputs: FrameInputs,
    layout: crate::frame::Layout,
    margin: i64,
}

impl Bounds {
    fn of(
        function: &MFunction,
        assignment: &Assignment,
        slots: &[i32],
        short: bool,
    ) -> Self {
        let insts: Vec<&Inst> = function.blocks.iter().flat_map(|block| block.insts.iter()).collect();
        let inputs = FrameInputs::of(&insts, function, assignment, slots, short);
        let calls = call_area(&insts, &|_| true);
        let layout = inputs.layout(calls);
        Bounds { margin: margin(calls), inputs, layout }
    }

    fn range(
        &self,
        index: i32,
    ) -> Option<(i64, i64)> {
        let at = self.inputs.frame_indices.iter().position(|one| *one == index)?;
        let (base, disp) = self.layout.address(at);
        Some(match base {
            Base::Sp => (disp, disp + self.margin),
            Base::Fp => (disp - 8, disp + 8),
        })
    }
}

/// `lea (d, frame index), v`: the register, the displacement and the index.
fn frame_lea(inst: &Inst) -> Option<(u32, i64, i32)> {
    match (&inst.kind, &inst.operands[..]) {
        (Kind::Op("LEA32p"), [Operand::Def(Reg::Virt(v)), Operand::Imm(d), Operand::Frame(f)]) => Some((*v, *d, *f)),
        _ => None,
    }
}

/// `function` with the frame leas whose every use is the base of an indexed
/// operand folded into them; none when no `lea` qualifies.
pub fn fold(
    function: &MFunction,
    assignment: &Assignment,
    slots: &[i32],
    short: bool,
) -> Option<MFunction> {
    if !function.blocks.iter().flat_map(|block| &block.insts).any(|inst| frame_lea(inst).is_some()) {
        return None;
    }
    let count = function.classes.len();
    // how often each register is defined, and by what; where it is read
    let mut defs = vec![0u32; count];
    let mut leas: Vec<Option<(i64, i32, usize)>> = vec![None; count];
    // every read as an indexed base: (block, instruction, operand); `other`:
    // any other read
    let mut reads: Vec<Vec<(usize, usize, usize)>> = vec![Vec::new(); count];
    let mut other = vec![false; count];
    for (b, block) in function.blocks.iter().enumerate() {
        for phi in &block.phis {
            if let Reg::Virt(v) = phi.result {
                defs[v as usize] += 2;
            }
            for (_, incoming) in &phi.incoming {
                if let Reg::Virt(v) = incoming {
                    other[*v as usize] = true;
                }
            }
        }
        for (i, inst) in block.insts.iter().enumerate() {
            if let Some((v, d, f)) = frame_lea(inst) {
                leas[v as usize] = Some((d, f, b));
            }
            let base = match &inst.kind {
                Kind::Op(name) => memory_at(name).map(|(base, _)| base),
                Kind::Copy => None,
            };
            for (o, operand) in inst.operands.iter().enumerate() {
                match operand {
                    Operand::Def(Reg::Virt(v)) => defs[*v as usize] += 1,
                    Operand::Use(Reg::Virt(v)) if base == Some(o) => reads[*v as usize].push((b, i, o)),
                    Operand::Use(Reg::Virt(v)) => other[*v as usize] = true,
                    _ => {}
                }
            }
        }
    }
    let bounds = Bounds::of(function, assignment, slots, short);
    let mut folded = function.clone();
    let mut removed = vec![Vec::new(); function.blocks.len()];
    for v in 0..count {
        let Some((d, f, at)) = leas[v] else { continue };
        if defs[v] != 1 || other[v] || reads[v].is_empty() {
            continue;
        }
        let Some((low, high)) = bounds.range(f) else { continue };
        let fits = reads[v]
            .iter()
            .all(
                |&(b, i, o)| {
                    let Operand::Imm(disp) = function.blocks[b].insts[i].operands[o - 1] else { return false };
                    let shift = d + disp;
                    fits(low + shift, high + shift)
                },
            );
        if !fits {
            continue;
        }
        for &(b, i, o) in &reads[v] {
            let Operand::Imm(disp) = folded.blocks[b].insts[i].operands[o - 1] else { unreachable!() };
            folded.blocks[b].insts[i].operands[o - 1] = Operand::Imm(disp + d);
            folded.blocks[b].insts[i].operands[o] = Operand::Frame(f);
        }
        removed[at].push(v as u32);
    }
    if removed.iter().all(Vec::is_empty) {
        return None;
    }
    for (block, gone) in folded.blocks.iter_mut().zip(&removed) {
        block.insts.retain(|inst| !frame_lea(inst).is_some_and(|(v, ..)| gone.contains(&v)));
    }
    Some(folded)
}
