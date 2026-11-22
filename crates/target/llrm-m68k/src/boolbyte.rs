//! A byte that holds a boolean made by `scc` is zero or `0xFF` (`scc` sets the
//! whole byte), and `and.b #1` makes it zero or one. The pass finds the bytes
//! whose every definition is `scc`, a small constant or an `or`, `and` or copy
//! of such bytes, and drops the mask where the value set says it does nothing
//! that is read.
//!
//! # How it works
//! [`byte_values`] gives each virtual register the set of low-byte values its
//! definitions can write, out of `{0, 1, 0xFF}`; any other definition makes it
//! unknown. [`scc_booleans`] then rewrites each `and.b #1` of such a byte:
//! - values in `{0, 1}`: the mask is a copy;
//! - values in `{0, 0xFF}`: `neg.b`, which also gives zero or one;
//! - values in `{0, 1, 0xFF}` read only by a branch on `Z`: `tst.b`, since the
//!   byte is zero exactly when its low bit is.
//!
//! The codes the replacement leaves must be those the readers test
//! ([`crate::cmpelim::flags_read_after`]): `tst.b` of `0xFF` sets `N`, so it
//! serves a reader of `Z` only.
//!
//! DEVIATE: the pin keeps `and.b #1` (`seq; and.b #1; cmpi.b #0; beq`), and
//! GCC's m68k.md `scc` pattern (`STORE_FLAG_VALUE` is -1) uses `neg.b` for a
//! value and tests the byte for a branch. Reason: best of gcc and clang is the
//! goal. Owner: w9. Added 2026-11-22; exit 2026-12-20, when the pin gains it.
//!
//! # See also
//! `m68k.md` `scc` and `cbranch`; `M68kISelLowering::LowerSETCC`.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::ccr::flag_bit;
use crate::cmpelim::flags_read_after;
use crate::dagisel::generated::instrinfo;
use crate::dagisel::local;
use crate::mfunc::{MFunction, op_inst};

const ZERO: u8 = 1;
const ONE: u8 = 2;
const MINUS: u8 = 4;
/// Any other value, or a definition the pass does not read.
const OTHER: u8 = 8;

/// `or` and `and` of two sets of byte values, closed over `{0, 1, 0xFF}`.
fn combine(
    a: u8,
    b: u8,
    or: bool,
) -> u8 {
    if a & OTHER != 0 || b & OTHER != 0 {
        return OTHER;
    }
    let value = |bit: u8| match bit {
        ZERO => 0u8,
        ONE => 1,
        _ => 0xFF,
    };
    let mut out = 0;
    for x in [ZERO, ONE, MINUS].into_iter().filter(|x| a & x != 0) {
        for y in [ZERO, ONE, MINUS].into_iter().filter(|y| b & y != 0) {
            let r = if or { value(x) | value(y) } else { value(x) & value(y) };
            out |= match r {
                0 => ZERO,
                1 => ONE,
                _ => MINUS,
            };
        }
    }
    out
}

/// What one definition of `inst` writes into the register it defines.
fn written(
    inst: &Inst,
    sets: &[u8],
) -> u8 {
    let set_of = |operand: &Operand| match operand {
        Operand::Use(Reg::Virt(v)) => sets[*v as usize],
        _ => OTHER,
    };
    match (inst.kind, &inst.operands[..]) {
        (Kind::Copy, [_, source]) => set_of(source),
        (Kind::Op(name), [_]) if name.starts_with("SETd8") => ZERO | MINUS,
        (Kind::Op("MOVQ"), [_, Operand::Imm(value)]) => match value & 0xFF {
            0 => ZERO,
            1 => ONE,
            0xFF => MINUS,
            _ => OTHER,
        },
        (Kind::Op("OR8dd"), [_, a, b]) => combine(set_of(a), set_of(b), true),
        (Kind::Op("AND8dd"), [_, a, b]) => combine(set_of(a), set_of(b), false),
        (Kind::Op("AND8di"), [_, source, Operand::Imm(1)]) => combine(set_of(source), ONE, false),
        _ => OTHER,
    }
}

/// The values the low byte of each virtual register can hold, as a bit set; a
/// register no instruction defines is `OTHER`.
fn byte_values(function: &MFunction) -> Vec<u8> {
    let mut sets = vec![0u8; function.classes.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for inst in function.blocks.iter().flat_map(|block| &block.insts) {
            for (at, operand) in inst.operands.iter().enumerate() {
                let Operand::Def(Reg::Virt(v)) = operand else { continue };
                // only the first operand of the instructions read is a result
                let value = if at == 0 { written(inst, &sets) } else { OTHER };
                let merged = sets[*v as usize] | value;
                if merged != sets[*v as usize] {
                    sets[*v as usize] = merged;
                    changed = true;
                }
            }
        }
    }
    for set in &mut sets {
        if *set == 0 {
            *set = OTHER;
        }
    }
    sets
}

fn uses_of(
    function: &MFunction,
    v: u32,
) -> usize {
    let in_insts = function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .flat_map(|inst| &inst.operands)
        .filter(|operand| matches!(operand, Operand::Use(Reg::Virt(u)) if *u == v))
        .count();
    let in_phis = function
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .flat_map(|phi| &phi.incoming)
        .filter(|(_, register)| matches!(register, Reg::Virt(u) if *u == v))
        .count();
    in_insts + in_phis
}

pub fn scc_booleans(function: &mut MFunction) {
    let sets = byte_values(function);
    let (z, n) = (flag_bit("Z"), flag_bit("N"));
    let neg = crate::dagisel::opcodes::code_of("NEG8d").and_then(instrinfo::desc);
    let tst = local::code_of("TST8d").and_then(local::desc);
    let (Some(neg), Some(tst)) = (neg, tst) else { return };
    for block in 0..function.blocks.len() {
        let mut at = 0;
        while at < function.blocks[block].insts.len() {
            let inst = &function.blocks[block].insts[at];
            let (Kind::Op("AND8di"), [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(s)), Operand::Imm(1)]) =
                (inst.kind, &inst.operands[..])
            else {
                at += 1;
                continue;
            };
            let (d, s) = (*d, *s);
            let values = sets[s as usize];
            let Some(needed) = flags_read_after(&function.blocks[block].insts, at) else {
                at += 1;
                continue;
            };
            let unused = uses_of(function, d) == 0;
            let replacement: Vec<Inst> = if values & OTHER != 0 {
                Vec::new()
            } else if values & MINUS == 0 && needed & !(z | n) == 0 {
                // `{0, 1}`: the mask is a copy, and `tst.b` gives its codes
                let mut out = Vec::new();
                if !unused {
                    out.push(Inst::copy(Reg::Virt(d), Reg::Virt(s)));
                }
                if needed != 0 {
                    out.push(op_inst("TST8d", tst, vec![Operand::Use(Reg::Virt(s))]));
                }
                out
            } else if values & ONE == 0 && !unused && needed & !(z | n) == 0 {
                vec![op_inst("NEG8d", neg, vec![Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(s))])]
            } else if unused && needed & !z == 0 {
                vec![op_inst("TST8d", tst, vec![Operand::Use(Reg::Virt(s))])]
            } else {
                Vec::new()
            };
            if replacement.is_empty() {
                at += 1;
                continue;
            }
            let count = replacement.len();
            function.blocks[block].insts.splice(at..=at, replacement);
            at += count;
        }
    }
}

#[cfg(test)]
mod tests {
    use llrm_spike_mf::{Block, Phi};

    use super::*;
    use crate::mfunc::Class;

    /// `and.b #1` of an `scc` that nothing but a phi reads was dropped as
    /// unused when only instruction operands were counted, so the phi read
    /// a register nothing defined: QuickrBasic `ast.c` m68k Os panicked in
    /// live-range calculation ("a unique value"), 10 of 93 modules.
    #[test]
    fn the_mask_of_a_byte_a_phi_reads_stays_a_definition() {
        let mut function = MFunction::new("f");
        function.classes = vec![Class::Dr8; 3];
        let scc = Inst::new(Kind::Op("SETd8eq"), vec![Operand::Def(Reg::Virt(0))]);
        let mut mask = Inst::new(
            Kind::Op("AND8di"),
            vec![Operand::Def(Reg::Virt(1)), Operand::Use(Reg::Virt(0)), Operand::Imm(1)],
        );
        mask.sets_flags = true;
        let mut branch = Inst::new(Kind::Op("Beq8"), vec![Operand::Block(1)]);
        branch.terminator = true;
        branch.reads_flags = true;
        let first = Block { insts: vec![scc, mask, branch], ..Block::default() };
        let second =
            Block { phis: vec![Phi { result: Reg::Virt(2), incoming: vec![(0, Reg::Virt(1))] }], ..Block::default() };
        function.blocks = vec![first, second];
        scc_booleans(&mut function);
        let defined = function.blocks[0]
            .insts
            .iter()
            .any(|inst| inst.operands.iter().any(|one| matches!(one, Operand::Def(Reg::Virt(1)))));
        assert!(defined, "{:?}", function.blocks[0].insts);
    }
}
