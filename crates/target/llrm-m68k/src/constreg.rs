//! `move.l %d1, %d2; and.l #1, %d2` is `moveq #1, %d2; and.l %d1, %d2`: the
//! same value and codes in 4 bytes, not 8, and 12 clocks, not 20, on the
//! 68000. Likewise `or`, and `add` of a constant past `addq`'s 1 to 8 (an
//! address register may be the source of an `add`). `x & 1` of a copy is
//! popcount's loop.
//!
//! # How it works
//! [`constant_into_destination`] runs after allocation. A copy into a data
//! register, then the immediate row of `and`, `or` or `add` on that register
//! with a constant a `moveq` makes (-128 to 127), becomes the `moveq` and the
//! register row with the copy's source. Both leave N and Z from the result and
//! V and C clear.
//!
//! DEVIATE: the pin's patterns take the immediate form. Reason: best of gcc
//! and clang is the goal; gcc builds the constant in the destination. The
//! pin has no pass for it (its `AND32di` pattern takes the immediate). Exit:
//! 2026-11-12, the shared flow's peephole owns it (convergence step 5) or the
//! pin's choice stands. Off for `Options::PINNED`.
//!
//! # See also
//! `consts.rs`.

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::dagisel::local;
use crate::mfunc::{MFunction, op_inst};
use crate::regalloc::Assignment;
use crate::{REGISTER_INFO, reg};

/// Rewrites each copy and immediate row it feeds, as described above.
pub fn constant_into_destination(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let data = [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7];
    let root = |register: Reg| match register {
        Reg::Virt(v) => REGISTER_INFO.root(assignment.0[v as usize]),
        Reg::Phys(p) => REGISTER_INFO.root(p),
    };
    for block in &mut function.blocks {
        let mut at = 0;
        while at + 1 < block.insts.len() {
            let here = at;
            at += 1;
            let (Kind::Copy, [Operand::Def(to), Operand::Use(from)]) =
                (block.insts[here].kind, &block.insts[here].operands[..])
            else {
                continue;
            };
            let (to, from) = (*to, *from);
            let Kind::Op(name) = block.insts[here + 1].kind else { continue };
            let [Operand::Def(def), Operand::Use(base), Operand::Imm(value)] = block.insts[here + 1].operands[..]
            else {
                continue;
            };
            if root(def) != root(to) || root(base) != root(to) || root(from) == root(to) || !data.contains(&root(to)) {
                continue;
            }
            if !(-128..=127).contains(&value) {
                continue;
            }
            let from_data = data.contains(&root(from));
            let row = match (name, from_data) {
                ("AND32di", true) => "AND32dd",
                ("OR32di", true) => "OR32dd",
                // 1 to 8 is an `addq`, 2 bytes already
                ("ADD32di", true) if !(-8..=8).contains(&value) => "ADD32dd",
                ("ADD32di", false) if !(-8..=8).contains(&value) => "ADD32dr",
                _ => continue,
            };
            let Some(move_desc) = local::code_of("MOVI32ri").and_then(local::desc) else { continue };
            let Some(row_desc) = local::code_of(row).and_then(local::desc) else { continue };
            let load = op_inst("MOVI32ri", move_desc, vec![Operand::Def(def), Operand::Imm(value)]);
            let rewritten = op_inst(row, row_desc, vec![Operand::Def(def), Operand::Use(base), Operand::Use(from)]);
            block.insts[here] = load;
            block.insts[here + 1] = rewritten;
            at += 1;
        }
    }
}
