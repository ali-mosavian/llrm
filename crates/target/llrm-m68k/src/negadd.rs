//! `add #-1, %d1` is `subq #1, %d1` when the branch after it reads only N and
//! Z: the result is the same, and the carry and overflow differ.
//!
//! # How it works
//! [`negative_adds_to_subq`] turns an add of -8 to -1 into the `subq` of the
//! same size where the readers of the codes after it test only flags the two
//! forms leave alike ([`crate::cmpelim::flags_read_after`]).
//!
//! DEVIATE: the pin has no `SUBQ`, and its DAG combine canonicalizes `x - 1` to
//! `x + -1`. Reason: best of gcc and clang is the goal. Exit: none.

use llrm_spike_mf::{Kind, Operand};

use crate::ccr::flag_bit;
use crate::cmpelim::flags_read_after;
use crate::dagisel::local;
use crate::mfunc::{MFunction, op_inst};

/// The `subq` an `add` of an immediate is the other way round to.
fn subq_for(name: &str) -> Option<&'static str> {
    let rest = name.strip_prefix("ADD")?;
    let sized = rest.strip_suffix("di")?;
    let target =
        ["SUBQ8di", "SUBQ16di", "SUBQ32di"].into_iter().find(|one| one[4..].strip_suffix("di") == Some(sized))?;
    local::code_of(target).map(|_| target)
}

pub fn negative_adds_to_subq(function: &mut MFunction) {
    let alike = flag_bit("N") | flag_bit("Z");
    for block in &mut function.blocks {
        for at in 0..block.insts.len() {
            let Kind::Op(name) = block.insts[at].kind else { continue };
            let Some(target) = subq_for(name) else { continue };
            let [Operand::Def(def), Operand::Use(base), Operand::Imm(value)] = block.insts[at].operands[..] else {
                continue;
            };
            if !(-8..=-1).contains(&value) {
                continue;
            }
            if flags_read_after(&block.insts, at).is_none_or(|needed| needed & !alike != 0) {
                continue;
            }
            let Some(desc) = local::code_of(target).and_then(local::desc) else { continue };
            block.insts[at] = op_inst(target, desc, vec![Operand::Def(def), Operand::Use(base), Operand::Imm(-value)]);
        }
    }
}
