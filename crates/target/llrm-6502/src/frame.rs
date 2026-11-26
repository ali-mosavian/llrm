//! The 6502 frame: where the frame objects go, the prologue and epilogue, and
//! the address of an object. The soft stack is `rs0` (`rc0:rc1`) and grows
//! down; the hardware stack holds only return addresses, so there is no return
//! slot and no alignment.
//!
//! # How it works
//! The layout is the generic `calculate_frame_object_offsets`: fixed objects
//! (the stack arguments, at offsets from the stack pointer at entry), the saved
//! registers, the locals, then the outgoing call area the frame reserves. The
//! prologue is one `ADJSP -size`; objects are addressed from `rs0` at `offset +
//! size`, which the `(zp),Y` forms take as a byte (at most 255). The call
//! sequence is reserved in the frame, so `CALLSEQ_START` and `CALLSEQ_END`
//! expand to nothing; a frame with variable-sized objects adjusts `rs0` around
//! each call instead.
//!
//! An object aligned past [`REALIGN_ABOVE`] realigns the soft stack
//! (`needsStackRealignment`): the prologue is one `ALIGNSP`, `rs0 = (rs0 -
//! size) & -align`, which also stores the old `rs0` in a frame slot; the
//! epilogue reloads `rs0` from it (`RESTORESP`), and a stack argument is read
//! from the old `rs0` (`LEAOLD`) where the frame has no pointer register to
//! keep it in.
//!
//! DEVIATE: not translated are a frame pointer, a dynamic alloca in a
//! realigned frame, an object past offset 255 (it needs the address in a
//! pair), tail calls and shrink wrapping. Exit: when a program needs them.
//!
//! # See also
//! `crates/target/llrm-m68k/src/frame.rs`, the same shape for m68k.

use llrm_codegen::backend::frameobjects::{Frame, Object, calculate_frame_object_offsets};

use crate::dagisel::generated::ids::opcode as op;
use crate::emit::{MInst, Operand, spill};

/// What the frame depends on.
#[derive(Clone, Debug, Default)]
pub struct Function {
    /// Fixed objects (incoming arguments) first, then the locals; an offset is
    /// given for the fixed ones only.
    pub objects: Vec<Object>,
    /// How many of `objects` are fixed.
    pub fixed: usize,
    /// The callee-saved pairs the allocator used (`rs4`.. in `calling.toml`'s
    /// `preserved`).
    pub saved: Vec<String>,
    pub max_call_frame: i64,
    pub adjusts_stack: bool,
    pub has_var_sized: bool,
}

/// The laid-out frame.
#[derive(Clone, Debug)]
pub struct Layout {
    pub stack_size: i64,
    /// `objects` with every offset set (from the stack pointer at entry), the
    /// saved-pair slots last.
    pub objects: Vec<Object>,
    saved: Vec<String>,
    first_save: usize,
    fixed: usize,
    reserved_call_frame: bool,
    realign: Option<Realign>,
}

/// A frame that realigns the stack pointer: to `align`, the old pointer kept
/// in the object `slot`.
#[derive(Clone, Copy, Debug)]
struct Realign {
    align: i64,
    slot: usize,
}

/// The alignment the soft stack keeps, in bytes: frames are any size. The
/// datalayout's `S` is this (`tests/stack_facts.rs`).
pub const STACK_ALIGN: i64 = 1;

pub fn layout(function: &Function) -> Layout {
    let mut objects = function.objects.clone();
    let max_align = objects.iter().map(|object| object.align).max().unwrap_or(1);
    let realign = (max_align > STACK_ALIGN).then(|| {
        objects.push(Object { size: 2, align: 1, offset: 0, callee_saved: false, dead: false });
        Realign { align: max_align, slot: objects.len() - 1 }
    });
    let first_save = objects.len();
    for _ in &function.saved {
        objects.push(Object { size: 2, align: 1, offset: 0, callee_saved: true, dead: false });
    }
    let reserved_call_frame = !function.has_var_sized;
    let frame = Frame {
        local_area: 0,
        max_align: 1,
        stack_align: STACK_ALIGN,
        transient_align: 1,
        adjusts_stack: function.adjusts_stack,
        has_var_sized: function.has_var_sized,
        realigned: realign.is_some(),
        reserved_call_frame,
        max_call_frame_size: function.max_call_frame,
    };
    let stack_size = calculate_frame_object_offsets(&mut objects, function.fixed, frame);
    Layout {
        stack_size,
        objects,
        saved: function.saved.clone(),
        first_save,
        fixed: function.fixed,
        reserved_call_frame,
        realign,
    }
}

/// The widest scalar (clang gives a fixed-point `long _Accum` `align 8`). An
/// `alloca` aligned to no more is clamped to what the frame keeps
/// ([`STACK_ALIGN`]); a larger one realigns the stack
/// (`Machine::realigned_above`).
pub const REALIGN_ABOVE: u64 = 8;

fn adjust(bytes: i64) -> MInst {
    MInst::new(op::ADJSP, vec![Operand::Imm(bytes)])
}

impl Layout {
    /// `getFrameIndexReference`: the byte offset of object `index` from `rs0`.
    pub fn address(
        &self,
        index: usize,
    ) -> i64 {
        self.objects[index].offset + self.stack_size
    }

    /// Whether the frame realigns `rs0`.
    pub fn realigned(&self) -> bool {
        self.realign.is_some()
    }

    /// The byte offset of fixed object `index` from `rs0` at entry, which a
    /// realigned frame no longer is the frame's `rs0` plus a distance.
    pub fn entry_offset(
        &self,
        index: usize,
    ) -> i64 {
        self.objects[index].offset
    }

    /// Where a realigned frame keeps the `rs0` it had at entry.
    pub fn old_stack_pointer(&self) -> i64 {
        self.realign.map_or(0, |realign| self.address(realign.slot))
    }

    fn save_slots(&self) -> impl Iterator<Item = (&String, i64)> {
        self.saved.iter().enumerate().map(|(n, name)| (name, self.address(self.first_save + n)))
    }

    /// `emitPrologue` with the register saves: the frame opened, then each pair
    /// stored.
    pub fn prologue(&self) -> Vec<MInst> {
        let mut insts = Vec::new();
        if let Some(realign) = self.realign {
            insts.push(MInst::new(
                spill::ALIGNSP,
                vec![
                    Operand::Imm(self.stack_size),
                    Operand::Imm(realign.align),
                    Operand::Imm(self.old_stack_pointer()),
                ],
            ));
        } else if self.stack_size != 0 {
            insts.push(adjust(-self.stack_size));
        }
        for (name, offset) in self.save_slots() {
            insts.push(MInst::new(op::STP16, vec![Operand::reg(name), Operand::reg("rs0"), Operand::Imm(offset)]));
        }
        insts
    }

    /// `emitEpilogue`: the pairs reloaded, then the frame closed.
    pub fn epilogue(&self) -> Vec<MInst> {
        let mut insts = Vec::new();
        for (name, offset) in self.save_slots() {
            insts.push(MInst::new(op::LDP16, vec![Operand::reg(name), Operand::reg("rs0"), Operand::Imm(offset)]));
        }
        if self.realign.is_some() {
            insts.push(MInst::new(spill::RESTORESP, vec![Operand::Imm(self.old_stack_pointer())]));
        } else if self.stack_size != 0 {
            insts.push(adjust(self.stack_size));
        }
        insts
    }

    /// `eliminateCallFramePseudoInstr`: nothing when the call frame is
    /// reserved, else `rs0` moves by the bytes of the call sequence (`down`
    /// opens it).
    pub fn call_sequence(
        &self,
        down: bool,
        bytes: i64,
    ) -> Vec<MInst> {
        if self.reserved_call_frame || bytes == 0 {
            return Vec::new();
        }
        vec![adjust(if down { -bytes } else { bytes })]
    }

    /// `eliminateFrameIndex`: a frame object in the pointer operand of a memory
    /// instruction, or in a `LEASP16`, becomes `rs0` and the offset from
    /// it.
    pub fn eliminate(
        &self,
        inst: &MInst,
    ) -> Result<MInst, String> {
        let at = match inst.opcode {
            op::LDP8 | op::LDP16 | op::STP8 | op::STP16 => inst.operands.len() - 2,
            op::STZP => 0,
            op::LEASP16 => 1,
            code if crate::pointer_alu::pointer_alu(code).is_some_and(|fold| !fold.indexed) => inst.operands.len() - 2,
            _ => return Ok(inst.clone()),
        };
        let Operand::FrameIndex(index) = inst.operands[at] else { return Ok(inst.clone()) };
        if self.realigned() && index < self.fixed {
            return Err(format!("stack argument {index} of a realigned frame is read from the old rs0"));
        }
        let base = self.address(index);
        let mut operands = inst.operands.clone();
        if inst.opcode == op::LEASP16 {
            operands[1] = Operand::Imm(base);
            return Ok(MInst::new(inst.opcode, operands));
        }
        let Operand::Imm(extra) = inst.operands[at + 1] else { return Err("the offset is not an immediate".into()) };
        let offset = base + extra;
        if !(0..=255).contains(&offset) {
            return Err(format!("frame object {index} is {offset} bytes from rs0: past the 255 a (zp),Y reaches"));
        }
        operands[at] = Operand::reg("rs0");
        operands[at + 1] = Operand::Imm(offset);
        Ok(MInst::new(inst.opcode, operands))
    }
}

#[cfg(test)]
mod stack_facts {
    /// The datalayout named no stack alignment, so a pass raising a local to
    /// its type's preferred alignment had no bound.
    #[test]
    fn the_datalayout_states_the_stack_alignment_the_frames_keep() {
        let layout = crate::LAYOUT
            .lines()
            .find_map(|line| line.strip_prefix("datalayout = \"")?.strip_suffix('"'))
            .expect("a datalayout line");
        llrm_target::testing::assert_stack_facts("6502", layout, super::STACK_ALIGN as u64, None);
    }
}
