//! A compare of a value with a constant, where the same value minus (nearly)
//! that constant is made just before it: the subtract's flags are the
//! compare's. Translated from `X86TargetLowering::EmitCmp`
//! ([X86ISelLowering.cpp#L24179](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L24179)):
//! "Else use SUB instead of CMP to enable CSE between SUB and CMP", after
//! which the DAG holds one `X86ISD::SUB` for the difference and the branch.
//!
//! # How it works
//! In a block that ends `cmp x, K; jcc`, a `d := add x, -J` or `d := sub x, J`
//! before the compare, with `x` the same value and `J` equal to `K` (any
//! condition) or to `K + 1` (`ja` is then `jae`, `jbe` `jb`, `jg` `jge`, `jle`
//! `jl`: the pin canonicalises the setcc to the second form), is made
//! `d := sub x, J` and the compare goes. Nothing between the two may write the
//! flags. The subtract's own flags are not read by anything else, which is why
//! the pin could not sink it past the branch (x_adler: `x >= 65521 ? x - 65521
//! : x`, `sub; jae` against `cmp; jbe` around an `add`).
//!
//! # Example
//! `unsigned h(unsigned x) { return x >= 65521u ? x - 65521u : x; }` on
//! `-m32 -march=i486 -O2`. The compare goes; the subtract's carry is the
//! branch (the pin also makes the difference once, then selects with `cmovb`):
//!
//! ```text
//!   mov ecx, eax                     mov ecx, eax
//!   add ecx, -65521                  sub ecx, 65521
//!   cmp eax, 65520          =>       jb  L_keep
//!   jbe L_keep
//! ```
//!
//! # Runs
//! A machine phase ahead of the first SSA phase (`flow::machine_with`), at
//! every -O level but -O0.

use std::sync::Arc;

use crate::model::ir::{Imm, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;

pub struct CmpAsSub;

impl LIRTransform for CmpAsSub {
    fn class_name(&self) -> &'static str {
        "CmpAsSub"
    }

    fn name(&self) -> &str {
        "cmpsub"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(fused(&body))
    }
}

fn mask(
    value: i64,
    bytes: u32,
) -> u64 {
    if bytes >= 8 { value as u64 } else { (value as u64) & ((1u64 << (bytes * 8)) - 1) }
}

/// The condition `name` becomes when the constant compared is one more, or
/// none where it does not.
fn shifted(name: &str) -> Option<&'static str> {
    Some(match name {
        "ja" | "jnbe" => "jae",
        "jbe" | "jna" => "jb",
        "jg" | "jnle" => "jge",
        "jle" | "jng" => "jl",
        _ => return None,
    })
}

/// `body` with each compare that a subtract before it covers dropped.
pub fn fused(body: &LirBody) -> LirBody {
    let mut changed = false;
    let blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|block| {
            let insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
            let real: Vec<usize> = (0..insns.len()).filter(|&at| !insns[at].is_meta()).collect();
            let Some(&branch) = real.last() else { return block.clone() };
            let Some(&compare) = real.iter().rev().nth(1) else { return block.clone() };
            let (Some(cmp), Some(jcc)) = (&insns[compare].what, &insns[branch].what) else { return block.clone() };
            let name = jcc.name.as_deref().unwrap_or("");
            if cmp.op != Operation::Compare
                || cmp.name.as_deref() != Some("cmp")
                || jcc.op != Operation::Branch
                || jcc.indirect
            {
                return block.clone();
            }
            let [Loc::Held(x), Loc::Imm(Imm { value: k, width, address: None })] = cmp.sources[..] else {
                return block.clone();
            };
            let bytes = x.width.max(1);
            let _ = width;
            // The subtract before it, with nothing between that writes flags.
            for &at in real.iter().rev().skip(2) {
                let one = &insns[at];
                let Some(what) = &one.what else { return block.clone() };
                let is_arith = what.op == Operation::Binary && matches!(what.name.as_deref(), Some("add" | "sub"));
                if !is_arith {
                    if crate::backend::liveness::effect(body.regs(), body.bits, one).is_none_or(|effect| {
                        effect.writes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None)
                    }) {
                        return block.clone();
                    }
                    continue;
                }
                let [Loc::Held(source), Loc::Imm(Imm { value: j, address: None, .. })] = what.sources[..] else {
                    return block.clone();
                };
                let [Loc::Held(result)] = what.dests[..] else { return block.clone() };
                // The flags are the compare's only at its width.
                if source.value != x.value || one.uses.len() != 1 || source.width != x.width || result.width != x.width
                {
                    return block.clone();
                }
                let subtracted =
                    if what.name.as_deref() == Some("add") { mask(j.wrapping_neg(), bytes) } else { mask(j, bytes) };
                let condition = if subtracted == mask(k, bytes) {
                    name
                } else if subtracted == mask(k.wrapping_add(1), bytes) && mask(k.wrapping_add(1), bytes) != 0 {
                    match shifted(name) {
                        Some(next) => next,
                        None => return block.clone(),
                    }
                } else {
                    return block.clone();
                };
                let sub = Semantics {
                    name: Some("sub".into()),
                    sources: vec![
                        Loc::Held(source),
                        Loc::Imm(Imm { value: subtracted as i64, width: source.width, address: None }),
                    ],
                    ..what.clone()
                };
                let mut out: Vec<Arc<Insn>> = Vec::new();
                for (position, insn) in insns.iter().enumerate() {
                    if position == at {
                        out.push(Arc::new(Insn { what: Some(sub.clone()), ..(**insn).clone() }));
                    } else if position == compare {
                        continue;
                    } else if position == branch {
                        let renamed = Semantics { name: Some(condition.into()), ..jcc.clone() };
                        out.push(Arc::new(Insn { what: Some(renamed), ..(**insn).clone() }));
                    } else {
                        out.push(Arc::clone(insn));
                    }
                }
                changed = true;
                return block.with_insns(out);
            }
            block.clone()
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Held, Semantics};
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 4, address: None })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    /// `x >= 65521 ? x - 65521 : x` (nhstutil-style reduction, bench x_adler):
    /// the add and the compare each set flags, and the branch read the
    /// compare's. The pin has the subtract's (`sub; jae`), so the add cannot
    /// be sunk past the branch and there is no compare.
    #[test]
    fn test_a_compare_of_the_value_just_subtracted_by_the_constant_is_the_subtracts_flags() {
        let block = LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(
                0,
                vec![
                    insn(0, Operation::Binary, "add", vec![held(2)], vec![held(1), imm(-65521)], vec![2], vec![1]),
                    insn(1, Operation::Compare, "cmp", vec![], vec![held(1), imm(65520)], vec![], vec![1]),
                    insn(2, Operation::Branch, "ja", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let mut body = LirBody::new("f", 0, vec![block.into()], IndexMap::default(), IndexMap::default());
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let done = fused(&body);
        let names: Vec<String> = done.blocks[0]
            .insns
            .iter()
            .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
            .collect();
        assert_eq!(names, ["sub", "jae"]);
    }

    /// A word compare after a dword `add` of the same value became `sub`, `ja`
    /// on the whole register: jpeg_single at -m32 left `length` stale in the
    /// upper half and printed "16-bit quantization tables not supported".
    /// Also with a constant of 32768 or more, and a negative one.
    #[test]
    fn test_a_compare_narrower_than_the_subtract_is_kept() {
        let word = |value| Loc::Held(Held { value, width: 2 });
        for (added, compared) in [(-65, 65), (-40000, 40000), (2, -2)] {
            let block = LirBlock {
                succ: vec![1, 2].into(),
                ..LirBlock::new(
                    0,
                    vec![
                        insn(0, Operation::Binary, "add", vec![held(2)], vec![held(1), imm(added)], vec![2], vec![1]),
                        insn(1, Operation::Compare, "cmp", vec![], vec![word(1), imm(compared)], vec![], vec![1]),
                        insn(2, Operation::Branch, "ja", vec![], vec![], vec![], vec![]),
                    ],
                )
            };
            let mut body = LirBody::new("f", 0, vec![block.into()], IndexMap::default(), IndexMap::default());
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            let done = fused(&body);
            let names: Vec<String> = done.blocks[0]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect();
            assert_eq!(names, ["add", "cmp", "ja"], "add {added}, cmp {compared}");
        }
    }
}
