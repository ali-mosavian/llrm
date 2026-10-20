//! A memory intrinsic through a huge pointer: a fill, a copy or a move whose
//! bytes can lie in more than one 64K segment.
//!
//! # How it works
//! A `rep` instruction wraps its 16-bit offset at 64K, so a fill or copy
//! through a pointer that carries into its selector cannot be one string
//! instruction. The lowering is a routine of the compiler's
//! (`llrm_x86::helpers`) laid down inline with its operands in fixed registers:
//! each run is the fewest of the bytes left and the bytes to the end of the
//! segment of each pointer, done by `rep stosd`/`movsd` and a tail, and the
//! selector of a pointer whose offset wrapped steps by the segment stride. The
//! count is 32 bits, any value; the offset and selector are whatever the
//! pointer holds, normalized or not. A `memmove` the passes proved descending
//! is copied from the last byte.
//!
//! The intrinsic carries the address space, so the passes that make fills and
//! copies (`fill`) name nothing about segments; this lowering is where they
//! are.
//!
//! DEVIATE:
//! 1. LLVM expands such a sequence with a custom inserter that creates basic
//!    blocks. isel here emits straight-line code per instruction, so the loop
//!    is inline machine code with fixed registers, as the i64 divide's is
//!    (`divided_by_helper`). Exit: isel emitting extra LIR blocks for a
//!    lowering, #2284.

use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_mir::module::Operand;
use llrm_mir::types::Type;

use super::{Selector, Unselected, insn, refuse, semantics};
use crate::abi::runtime;
use crate::backend::callregs::{call_clobbered_high, call_clobbers};
use crate::backend::lower_int64::{_helper, runtime_registers};
use crate::model::ir::{Held, Imm, Loc, Operation};
use crate::model::lir::Insn;

impl Selector<'_, '_, '_> {
    /// Whether `operand` is a pointer that carries into its selector.
    pub(super) fn is_huge(
        &self,
        operand: Operand,
    ) -> bool {
        self.function.operand_type(&self.module.context, operand).is_some_and(|ty| self.carries(ty))
    }

    /// A pointer operand as the offset and the selector the routines take.
    fn offset_and_selector(
        &mut self,
        operand: Operand,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(Held, Held), Unselected> {
        let ty = self.function.operand_type(&self.module.context, operand).expect("a typed pointer");
        let Type::Pointer(space) = *self.types().get(ty) else { return refuse("a memory intrinsic of a non-pointer") };
        if space != self.spaces.near {
            return self.far(operand, at, out);
        }
        let pointer = self.pointer(operand)?;
        let offset = self.fresh_held(2);
        out.push(insn(at, self.address(pointer, offset)));
        let Loc::Held(selector) = self.near_selector(pointer, false, at, out) else { unreachable!("a held selector") };
        Ok((offset, selector))
    }

    /// A length or count as a dword in a register.
    fn dword_count(
        &mut self,
        operand: Operand,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<Held, Unselected> {
        let ty = self.function.operand_type(&self.module.context, operand).expect("a typed length");
        if let Some(length) = self.length(operand) {
            let held = self.fresh_held(4);
            let value = Loc::Imm(Imm { value: length, width: 4, address: None });
            out.push(insn(at, semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![value])));
            return Ok(held);
        }
        let held = self.held(operand, ty, at, out)?;
        if held.width == 4 {
            return Ok(held);
        }
        let wide = self.fresh_held(4);
        out.push(insn(at, semantics(Operation::Extend, "movzx", vec![Loc::Held(wide)], vec![Loc::Held(held)])));
        Ok(wide)
    }

    /// `llvm.memset`, or `llvm.experimental.memset.pattern` (a `count` of cells
    /// of the value's width), through a huge pointer.
    pub(super) fn huge_fill(
        &mut self,
        destination: Operand,
        value: Operand,
        count: Operand,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let ty = self.function.operand_type(&self.module.context, value).expect("a typed value");
        let cell = match self.types().int_bits(ty) {
            Some(bits @ (8 | 16 | 32)) => bits / 8,
            _ => return refuse("a huge fill of a cell that is not a byte, a word or a dword"),
        };
        let (offset, selector) = self.offset_and_selector(destination, at, out)?;
        let count = self.dword_count(count, at, out)?;
        let pattern = self.fresh_held(4);
        match self.constant(value, cell) {
            Some(bits) => {
                let imm = Loc::Imm(Imm { value: bits & 0xFFFF_FFFF, width: 4, address: None });
                out.push(insn(at, semantics(Operation::Move, "mov", vec![Loc::Held(pattern)], vec![imm])));
            }
            None => {
                let held = self.held(value, ty, at, out)?;
                let into = if held.width == 4 { "mov" } else { "movzx" };
                let operation = if held.width == 4 { Operation::Move } else { Operation::Extend };
                out.push(insn(at, semantics(operation, into, vec![Loc::Held(pattern)], vec![Loc::Held(held)])));
            }
        }
        let routine =
            self.arch.huge_fill_routine(cell).map_err(|refused| Unselected(format!("a huge fill: {refused}")))?;
        self.huge_call(routine, vec![pattern, count, selector, offset], at, out);
        Ok(())
    }

    /// `llvm.memcpy`, or `llvm.memmove` whose direction the passes proved,
    /// where either pointer is huge.
    pub(super) fn huge_copy(
        &mut self,
        destination: Operand,
        source: Operand,
        length: Operand,
        backward: bool,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let (to_offset, to_selector) = self.offset_and_selector(destination, at, out)?;
        let (from_offset, from_selector) = self.offset_and_selector(source, at, out)?;
        let count = self.dword_count(length, at, out)?;
        let routine =
            self.arch.huge_copy_routine(backward).map_err(|refused| Unselected(format!("a huge copy: {refused}")))?;
        self.huge_call(routine, vec![count, from_selector, from_offset, to_selector, to_offset], at, out);
        Ok(())
    }

    /// The routine's call: its operands in the registers it reads, memory read
    /// and written, and nothing delivered.
    fn huge_call(
        &mut self,
        routine: llrm_target::routines::HugeString,
        operands: Vec<Held>,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
    ) {
        let requires: Vec<(Held, RegId)> = operands.into_iter().zip(routine.reads.iter().copied()).collect();
        let contract = runtime::Contract {
            writes: runtime::Memory::Any,
            reads: runtime::Memory::Any,
            .._helper(
                routine.name,
                runtime_registers(&self.regs, &routine.reads, false),
                runtime_registers(&self.regs, &routine.clobbers, true),
            )
        };
        out.push(Arc::new(Insn {
            clobbers: call_clobbers(&contract, self.segments, &self.cpu.general),
            clobbers_high: call_clobbered_high(&contract, self.segments, &self.cpu.general),
            uses: requires.iter().map(|(held, _)| held.value).collect(),
            requires,
            call: Some(self.listed(llrm_mir::memory::Effects::ANY, Default::default())),
            ..Insn::new(at, Some((at, at)), Some(semantics(Operation::Call, "call", vec![], vec![])), vec![], vec![])
        }));
        self.calls.insert(at, routine.name.to_owned());
        self.inline.insert(at, routine.code);
    }
}
