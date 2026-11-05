//! DEVIATE LG3-T: the encoded size of an LSR solution, for a target whose code
//! bytes the `Tti` counts exactly (`Tti::lsr_byte_model`, wasm). The pin's cost
//! has no byte axis: LG3's solution and the pin's can tie on instructions plus
//! setup while one spends more bytes on constants and unfolded offsets
//! (wasm32 quicksort, +10 B). Exit: 2026-11-20, or when the pin's order ranks
//! LG3's formulae.

use std::collections::BTreeSet;

use llrm_analysis::scev::expr::{Kind as ScevKind, Scev};
use llrm_analysis::scev::ScalarEvolution;
use llrm_mir::module::Operand;
use llrm_mir::tti::LsrByteModel;

use crate::lsr_collect::Collection;
use crate::lsr_use::Kind;

/// The bytes of a signed LEB128.
fn sleb(mut v: i128) -> u32 {
    let mut bytes = 1;
    while !(-64..64).contains(&v) {
        v >>= 7;
        bytes += 1;
    }
    bytes
}

/// The bytes of an unsigned LEB128.
fn uleb(v: u64) -> u32 {
    ((64 - v.leading_zeros()).max(1)).div_ceil(7)
}

/// Whether `s` is a global's address plus constants: the linker resolves it,
/// one relocatable `i32.const` with an addend.
fn link_time_constant(
    se: &ScalarEvolution,
    s: Scev,
) -> bool {
    match &se.node(s).kind {
        ScevKind::Unknown(Operand::Constant(_)) => true,
        ScevKind::Add(ops) => {
            let mut globals = 0;
            for op in ops {
                match &se.node(op.scev).kind {
                    ScevKind::Constant(_) => {}
                    ScevKind::Unknown(Operand::Constant(_)) => globals += 1,
                    _ => return false,
                }
            }
            globals == 1
        }
        _ => false,
    }
}

/// Reading `s` where it is used: a local, or a global's address constant.
fn read_bytes(
    se: &ScalarEvolution,
    m: &LsrByteModel,
    s: Scev,
) -> u32 {
    if link_time_constant(se, s) { m.const_op + m.reloc_leb } else { m.local_get }
}

/// The instructions that compute `s` from locals.
fn expr_bytes(
    se: &ScalarEvolution,
    m: &LsrByteModel,
    s: Scev,
) -> u32 {
    if link_time_constant(se, s) {
        return m.const_op + m.reloc_leb;
    }
    match &se.node(s).kind {
        ScevKind::Constant(c) => m.const_op + sleb(c.sext_value()),
        ScevKind::Add(ops) | ScevKind::Mul(ops) => {
            ops.iter().map(|op| expr_bytes(se, m, op.scev)).sum::<u32>() + (ops.len() as u32 - 1) * m.binop
        }
        _ => read_bytes(se, m, s),
    }
}

impl Collection<'_, '_> {
    /// The bytes of `solution` (one formula index per use): each use's reads
    /// and adds, the offsets and compares it ends in, and, once for each
    /// register, the update it takes every trip and the setup before the loop.
    pub fn solution_bytes(
        &self,
        se: &ScalarEvolution,
        m: &LsrByteModel,
        solution: &[usize],
    ) -> u32 {
        let mut bytes = 0;
        let mut regs = BTreeSet::new();
        for (lu, &chosen) in self.uses.iter().zip(solution) {
            let f = &lu.formulae[chosen];
            let used = f.base_regs.len() as u32 + u32::from(f.scaled_reg.is_some());
            bytes += f.base_regs.iter().copied().chain(f.scaled_reg).map(|reg| read_bytes(se, m, reg)).sum::<u32>();
            bytes += used.saturating_sub(1) * m.binop;
            if f.base_gv.is_some() {
                bytes += m.const_op + m.reloc_leb + m.binop;
            }
            if f.unfolded_offset.is_non_zero() {
                bytes += m.const_op + sleb(i128::from(f.unfolded_offset.0)) + m.binop;
            }
            for fixup in &lu.fixups {
                let offset = fixup.offset.add_unsigned(f.base_offset).0;
                match lu.kind {
                    Kind::Address => bytes += uleb(offset as u64 & u64::from(u32::MAX)),
                    Kind::ICmpZero if offset != 0 => bytes += m.const_op + sleb(i128::from(offset)) + m.binop,
                    // The exit branches on the value itself.
                    Kind::ICmpZero => {}
                    Kind::Basic | Kind::Special => {}
                }
            }
            regs.extend(f.base_regs.iter().copied().chain(f.scaled_reg));
        }
        for reg in regs {
            bytes += self.register_bytes(se, m, reg);
        }
        bytes
    }

    fn register_bytes(
        &self,
        se: &ScalarEvolution,
        m: &LsrByteModel,
        reg: Scev,
    ) -> u32 {
        if link_time_constant(se, reg) {
            return 0;
        }
        match &se.node(reg).kind {
            ScevKind::Constant(_) | ScevKind::Unknown(_) => 0,
            ScevKind::AddRec(ops, l) if *l == self.l && ops.len() == 2 => {
                let step = match &se.node(ops[1].scev).kind {
                    ScevKind::Constant(c) => m.const_op + sleb(c.sext_value()),
                    _ => m.local_get,
                };
                m.local_get + step + m.binop + m.local_set + expr_bytes(se, m, ops[0].scev) + m.local_set
            }
            ScevKind::AddRec(..) => 0,
            _ => expr_bytes(se, m, reg) + m.local_set,
        }
    }
}
