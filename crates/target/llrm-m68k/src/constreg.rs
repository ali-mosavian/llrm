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

/// A `move.l #n, mem` through a base register, with `n` a `moveq` makes and
/// not zero (`clr.l` is 2 bytes already): the register store that replaces it
/// and its operands, the immediate still last.
fn immediate_store(
    name: &str,
    operands: &[Operand],
) -> Option<(&'static str, Vec<Operand>)> {
    let ok = |value: i64| value != 0 && (-128..=127).contains(&value);
    match (name, operands) {
        ("MOV32ji", [Operand::Use(_), Operand::Imm(value)]) if ok(*value) => Some(("MOV32jr", operands.to_vec())),
        ("MOV32pi", [Operand::Imm(_), Operand::Use(_), Operand::Imm(value)]) if ok(*value) => {
            Some(("MOV32pr", operands.to_vec()))
        }
        _ => None,
    }
}

/// `and.l #15, %d2` where a data register is dead is `moveq #15, %dN; and.l
/// %dN, %d2`: 4 bytes and 12 clocks on the 68000 for 6 and 16. Likewise `or`,
/// `sub`, `cmp`, a store to memory (`move.l #5, (4,%sp)` is `moveq #5, %dN;
/// move.l %dN, (4,%sp)`, 6 bytes for 8, 20 clocks for 24) and an `add` past `addq`'s 1 to 8, with an immediate a `moveq`
/// makes (-128 to 127). The register is one nothing reads from there on, and
/// the callee-saved ones only where the function already uses them, so no
/// spill, copy or save is added. The register row leaves the condition codes
/// the immediate row did (`moveq` sets N and Z, the operation sets all again).
///
/// DEVIATE: the pin's patterns take the immediate form, as
/// `constant_into_destination`. Reason: best of gcc and clang is the goal; gcc
/// keeps the constant in a register (`moveq #9,%d2; cmp.l %d0,%d2`). Exit:
/// 2026-11-20. Off for `Options::PINNED`.
pub fn immediate_from_free_register(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    use llrm_spike_mf::livephys::PhysLiveness;

    use crate::regalloc::M68kRegisters;
    let data = [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7];
    let root = |register: Reg| match register {
        Reg::Virt(v) => REGISTER_INFO.root(assignment.0[v as usize]),
        Reg::Phys(p) => REGISTER_INFO.root(p),
    };
    let used: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| block.insts.iter())
        .flat_map(|inst| inst.operands.iter())
        .filter_map(|operand| match operand {
            Operand::Def(register) | Operand::Use(register) => Some(root(*register)),
            _ => None,
        })
        .collect();
    let candidates: Vec<_> = data
        .iter()
        .copied()
        .enumerate()
        .filter(|&(index, one)| index < 2 || used.contains(&one))
        .map(|(_, one)| one)
        .collect();
    let Some(move_desc) = local::code_of("MOVI32ri").and_then(local::desc) else { return };
    let liveness = PhysLiveness::of::<M68kRegisters>(function, assignment, &REGISTER_INFO);
    for b in 0..function.blocks.len() {
        // from the end, so the indices of the instructions still to visit hold
        for at in (0..function.blocks[b].insts.len()).rev() {
            let inst = &function.blocks[b].insts[at];
            let Kind::Op(name) = inst.kind else { continue };
            if let Some((store, operands)) = immediate_store(name, &inst.operands)
                // a store into the call area is `callpush`'s: it pushes the
                // constant with `pea`, 4 bytes
                .filter(|(_, operands)| {
                    !operands.iter().any(|one| matches!(one, Operand::Use(base) if root(*base) == reg::SP))
                })
            {
                let before = liveness.live_before::<M68kRegisters>(function, assignment, &REGISTER_INFO, b, at);
                let touched: Vec<_> = inst
                    .operands
                    .iter()
                    .filter_map(|operand| match operand {
                        Operand::Def(register) | Operand::Use(register) => Some(root(*register)),
                        _ => None,
                    })
                    .collect();
                let Some(&free) =
                    candidates.iter().find(|&&one| !before.is_live(&REGISTER_INFO, one) && !touched.contains(&one))
                else {
                    continue;
                };
                let (Some(store_desc), Some(value)) =
                    (local::code_of(store).and_then(local::desc), operands.last().cloned())
                else {
                    continue;
                };
                let Operand::Imm(value) = value else { continue };
                let mut stored = operands;
                *stored.last_mut().expect("a store has a source") = Operand::Use(Reg::Phys(free));
                let load = op_inst("MOVI32ri", move_desc, vec![Operand::Def(Reg::Phys(free)), Operand::Imm(value)]);
                function.blocks[b].insts[at] = op_inst(store, store_desc, stored);
                function.blocks[b].insts.insert(at, load);
                continue;
            }
            let (row, uses_dest) = match (name, &inst.operands[..]) {
                ("AND32di", [Operand::Def(_), Operand::Use(_), Operand::Imm(value)]) => ("AND32dd", *value),
                ("OR32di", [Operand::Def(_), Operand::Use(_), Operand::Imm(value)]) => ("OR32dd", *value),
                ("SUB32di", [Operand::Def(_), Operand::Use(_), Operand::Imm(value)]) => ("SUB32dd", *value),
                ("ADD32di", [Operand::Def(_), Operand::Use(_), Operand::Imm(value)]) if !(-8..=8).contains(value) => {
                    ("ADD32dd", *value)
                }
                ("CMP32di", [Operand::Imm(value), Operand::Use(_)]) => ("CMP32dr", *value),
                _ => continue,
            };
            let value = uses_dest;
            if !(-128..=127).contains(&value) || (name == "CMP32di" && value == 0) {
                continue;
            }
            let before = liveness.live_before::<M68kRegisters>(function, assignment, &REGISTER_INFO, b, at);
            let touched: Vec<_> = function.blocks[b].insts[at]
                .operands
                .iter()
                .filter_map(|operand| match operand {
                    Operand::Def(register) | Operand::Use(register) => Some(root(*register)),
                    _ => None,
                })
                .collect();
            let Some(&free) =
                candidates.iter().find(|&&one| !before.is_live(&REGISTER_INFO, one) && !touched.contains(&one))
            else {
                continue;
            };
            let Some(row_desc) = local::code_of(row).and_then(local::desc) else { continue };
            let load = op_inst("MOVI32ri", move_desc, vec![Operand::Def(Reg::Phys(free)), Operand::Imm(value)]);
            let operands = match &function.blocks[b].insts[at].operands[..] {
                [Operand::Def(def), Operand::Use(base), Operand::Imm(_)] => {
                    vec![Operand::Def(*def), Operand::Use(*base), Operand::Use(Reg::Phys(free))]
                }
                [Operand::Imm(_), Operand::Use(dest)] => vec![Operand::Use(Reg::Phys(free)), Operand::Use(*dest)],
                _ => continue,
            };
            function.blocks[b].insts[at] = op_inst(row, row_desc, operands);
            function.blocks[b].insts.insert(at, load);
        }
    }
}
