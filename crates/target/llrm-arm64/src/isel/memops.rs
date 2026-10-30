//! `memcpy`, `memmove` and `memset` of a constant length that fits the pin's
//! budget are expanded in place (`AArch64TargetLowering`'s `MaxStoresPerMem*`,
//! `TargetLowering::findOptimalMemOpLowering`): 16-byte `q` accesses, then the
//! next smaller power of two for the remainder. A remainder that a smaller
//! access would not cover is covered by the last size again, overlapping the
//! bytes before it.
//!
//! Copies load a batch before storing it, so `pairs` makes `ldp`/`stp q` of
//! them. A `memmove` loads all before it stores.

use llrm_mir::Operand;

use super::{Selected, Selector, Src};
use crate::mc::inst::*;

/// Accesses a copy of up to this many may use, and a fill; half of each under
/// `minsize`.
const COPY_OPS: usize = 16;
const FILL_OPS: usize = 32;
/// Loads a copy has in flight before it stores them.
const BATCH: usize = 4;

/// The `(bytes, offset)` accesses that cover `total` bytes.
fn pieces(total: u64) -> Vec<(u8, u64)> {
    let mut ops: Vec<(u8, u64)> = Vec::new();
    let (mut done, mut size) = (0, 16u64);
    while done < total {
        let left = total - done;
        if size > left {
            let mut fits = size;
            while fits > left {
                fits /= 2;
            }
            if !ops.is_empty() && fits < left {
                ops.push((size as u8, total - size));
                break;
            }
            size = fits;
        }
        ops.push((size as u8, done));
        done += size;
    }
    ops
}

/// The `q` accesses that cover `total >= 16` bytes, the last overlapping the
/// one before it.
fn wide_pieces(total: u64) -> Vec<(u8, u64)> {
    (0..total.div_ceil(16)).map(|at| (16, (at * 16).min(total - 16))).collect()
}

impl Selector<'_> {
    /// The pointer `operand`, a global's address made in a register: a
    /// page-offset relocation of an access wider than a byte needs an
    /// offset that is a multiple of its size, and these start anywhere.
    fn in_register(
        &mut self,
        operand: Operand,
    ) -> Selected<super::val::AddrExpr> {
        let address = self.pointer_operand(operand)?;
        if matches!(address.base, super::val::Base::Global { .. }) {
            let reg = self.pointer_register(&address)?;
            return Ok(super::val::AddrExpr::of(super::val::Base::Reg(reg)));
        }
        Ok(address)
    }

    fn minsize(&self) -> bool {
        self.function.attrs.iter().any(|one| matches!(one, llrm_mir::Attribute::Flag(name) if name == "minsize"))
    }

    fn constant_length(
        &mut self,
        operand: Operand,
    ) -> Selected<Option<u64>> {
        Ok(match self.src(operand)? {
            Src::Int { value, .. } => u64::try_from(value).ok(),
            _ => None,
        })
    }

    /// Expands `base` (`memcpy`, `memmove` or `memset`) when its length is
    /// constant and within budget; whether it did.
    pub(super) fn inline_memory(
        &mut self,
        base: &str,
        arguments: &[Operand],
    ) -> Selected<bool> {
        let Some(total) = self.constant_length(arguments[2])? else { return Ok(false) };
        if matches!(self.constant_length(arguments[3])?, Some(1)) || total == 0 {
            return Ok(false);
        }
        let divisor = if self.minsize() { 2 } else { 1 };
        if base == "memset" {
            return self.inline_fill(arguments, total, FILL_OPS / divisor);
        }
        let ops = pieces(total);
        if ops.len() > COPY_OPS / divisor {
            return Ok(false);
        }
        let destination = self.in_register(arguments[0])?;
        let source = self.in_register(arguments[1])?;
        let batch = if base == "memmove" { ops.len() } else { BATCH };
        for chunk in ops.chunks(batch) {
            let mut loaded = Vec::new();
            for &(bytes, offset) in chunk {
                let mem = self.piece(bytes);
                let at = super::val::AddrExpr { offset: source.offset + offset as i64, ..source.clone() };
                let addr = self.addressing(&at, bytes)?;
                self.inst(Inst::Load { mem, addr, mmo: super::mem::memory_operand(&at) });
                loaded.push(mem);
            }
            for (&(bytes, offset), mem) in chunk.iter().zip(loaded) {
                let at = super::val::AddrExpr { offset: destination.offset + offset as i64, ..destination.clone() };
                let addr = self.addressing(&at, bytes)?;
                self.inst(Inst::Store { mem, addr, mmo: super::mem::memory_operand(&at) });
            }
        }
        Ok(true)
    }

    fn inline_fill(
        &mut self,
        arguments: &[Operand],
        total: u64,
        budget: usize,
    ) -> Selected<bool> {
        let byte = match self.src(arguments[1])? {
            Src::Int { value, .. } => Some(value as u8),
            _ => None,
        };
        // A byte that is not constant fills `q` registers whole, which needs a
        // first 16 bytes.
        let ops = if byte.is_none() {
            if total < 16 {
                return Ok(false);
            }
            wide_pieces(total)
        } else {
            pieces(total)
        };
        if ops.len() > budget {
            return Ok(false);
        }
        let destination = self.in_register(arguments[0])?;
        let wide = ops.iter().any(|&(bytes, _)| bytes == 16);
        let vector = if wide {
            let rd = self.mf.fpr();
            let fill = match byte {
                Some(0) => Fill::Zero,
                Some(byte) => Fill::Byte(byte),
                None => Fill::Gpr(self.operand_g(arguments[1])?.reg),
            };
            self.inst(Inst::VectorFill { rd, fill });
            Some(rd)
        } else {
            None
        };
        let scalar = match byte {
            Some(0) => Gpr::R31,
            Some(byte) if ops.iter().any(|&(bytes, _)| bytes != 16) => {
                let reg = self.mf.gpr();
                self.load_immediate(reg, 64, u64::from(byte) * 0x0101_0101_0101_0101);
                reg
            }
            _ => Gpr::R31,
        };
        for (bytes, offset) in ops {
            let mem = match (bytes, vector) {
                (16, Some(reg)) => Memory::Fp { bytes, reg },
                _ => {
                    Memory::Int { width: if bytes == 8 { Width::X } else { Width::W }, bytes, sign: false, reg: scalar }
                }
            };
            let at = super::val::AddrExpr { offset: destination.offset + offset as i64, ..destination.clone() };
            let addr = self.addressing(&at, bytes)?;
            self.inst(Inst::Store { mem, addr, mmo: super::mem::memory_operand(&at) });
        }
        Ok(true)
    }

    /// A fresh register of `bytes` to carry one access of a copy.
    fn piece(
        &mut self,
        bytes: u8,
    ) -> Memory {
        if bytes == 16 {
            Memory::Fp { bytes, reg: self.mf.fpr() }
        } else {
            Memory::Int { width: if bytes == 8 { Width::X } else { Width::W }, bytes, sign: false, reg: self.mf.gpr() }
        }
    }
}
