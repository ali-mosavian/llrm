//! Flags kept across an instruction that clobbers them: `X86FlagsCopyLowering`.
//! Translated from
//! [X86FlagsCopyLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp)
//! (`runOnMachineFunction`).
//!
//! # How it works
//! A copy of the flags is two pseudo instructions: `v := flags_copy` where the
//! flags are live, and `flags_restore v` where they are wanted back. Each
//! condition read after the restore (up to the next instruction that writes
//! the flags) gets a byte, `setcc`, made where the copy was; the reader is
//! preceded by `test byte, byte` and reads `ne` (`e` for the inverse
//! condition, which shares the byte). `adc`/`sbb` read the carry: it is saved
//! as `setb` and made again by `add byte, 0xff`. A reader right after another
//! of the same byte reuses the `test`. The pseudo instructions go.
//!
//! DEVIATE R1 (no number yet): a copy and its restore must be in one block (the
//! pin follows the copy across blocks by dominance); else the pass fails.
//! Exit: the first isel producer of a cross-block copy. Nothing in isel makes
//! a copy yet, so the pass does nothing on the corpus.
//!
//! # Example
//! ```lir
//! v1 := flags_copy
//! v2 := add v3, 0x1
//! flags_restore v1
//! jl
//! ```
//! becomes
//! ```lir
//! v4 := setl
//! v2 := add v3, 0x1
//! test v4, v4
//! jne
//! ```
//!
//! # Runs
//! After `FixupSetCC`, before the float phases. Every level (the pin's pass is
//! not gated), so it is not in `UNOPTIMIZED`.

use std::collections::HashMap;
use std::sync::Arc;

use crate::backend::liveness;
use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;

pub struct FlagsCopy;

impl LIRTransform for FlagsCopy {
    fn class_name(&self) -> &'static str {
        "FlagsCopy"
    }

    fn name(&self) -> &str {
        "flagscopy"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        lowered(&body)
    }
}

/// The saved conditions and their inverses; a condition and its inverse share
/// one byte, saved as the first.
const PAIRS: [(&str, &str); 8] =
    [("e", "ne"), ("l", "ge"), ("le", "g"), ("b", "ae"), ("be", "a"), ("s", "ns"), ("o", "no"), ("p", "np")];

const ALIASES: [(&str, &str); 14] = [
    ("z", "e"),
    ("nz", "ne"),
    ("c", "b"),
    ("nc", "ae"),
    ("nae", "b"),
    ("nb", "ae"),
    ("na", "be"),
    ("nbe", "a"),
    ("nl", "ge"),
    ("nge", "l"),
    ("ng", "le"),
    ("nle", "g"),
    ("pe", "p"),
    ("po", "np"),
];

/// What an instruction reads of the flags.
enum Read {
    /// A condition: the saved one, and whether the reader wants it inverted.
    Cond { saved: &'static str, inverted: bool, stem: &'static str },
    Carry,
}

fn reading(one: &Insn) -> Option<Read> {
    let name = one.what.as_ref()?.name.as_deref()?;
    if name == "adc" || name == "sbb" {
        return Some(Read::Carry);
    }
    let (stem, cc): (&'static str, &str) = ["j", "set", "cmov"].iter().find_map(|&s| name.strip_prefix(s).map(|c| (s, c)))?;
    if stem == "j" && cc == "mp" {
        return None;
    }
    let cc = ALIASES.iter().find(|(from, _)| *from == cc).map_or(cc, |(_, to)| to);
    PAIRS.iter().find_map(|&(saved, inverse)| {
        (cc == saved || cc == inverse).then_some(Read::Cond { saved, inverted: cc == inverse, stem })
    })
}

fn is(
    one: &Insn,
    name: &str,
) -> bool {
    one.what.as_ref().and_then(|what| what.name.as_deref()) == Some(name)
}

fn make(
    at: i64,
    name: &str,
    op: Operation,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
    defines: Vec<u32>,
    uses: Vec<u32>,
) -> Arc<Insn> {
    let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
}

fn byte(value: u32) -> Loc {
    Loc::Held(Held { value, width: 1 })
}

/// `body` with its flag copies lowered.
pub fn lowered(body: &LirBody) -> Result<LirBody, String> {
    let (regs, bits) = (body.regs(), body.bits);
    let mut next = body.blocks.iter().flat_map(|b| b.insns.iter()).flat_map(|one| one.defines.iter().chain(&one.uses)).copied().max().unwrap_or(0) + 1;
    let flag_lane = |lanes: &crate::backend::lanes::Lanes| lanes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None);
    let mut out = body.clone();
    for block in &mut out.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
        while let Some(copy_at) = insns.iter().position(|one| is(one, "flags_copy")) {
            let saved = insns[copy_at].defines[0];
            let restore_at = insns[copy_at..]
                .iter()
                .position(|one| is(one, "flags_restore") && one.uses == [saved])
                .map(|at| at + copy_at)
                .ok_or_else(|| format!("flags copy v{saved} is restored in another block"))?;
            // Readers after the restore, up to the first writer of the flags.
            let mut readers = Vec::new();
            for index in restore_at + 1..insns.len() {
                let reads = reading(&insns[index]);
                let effect = liveness::effect(regs, bits, &insns[index]);
                let writes = effect.as_ref().map_or(true, |e| flag_lane(&e.writes));
                if reads.is_some() {
                    readers.push((index, reads.unwrap()));
                }
                if writes {
                    break;
                }
            }
            // A byte for each saved condition, made at the copy.
            let mut bytes: HashMap<&str, u32> = HashMap::new();
            let mut made = Vec::new();
            for (_, read) in &readers {
                let (key, name) = match read {
                    Read::Cond { saved, .. } => (*saved, format!("set{saved}")),
                    Read::Carry => ("b", "setb".to_owned()),
                };
                if !bytes.contains_key(key) {
                    let value = next;
                    next += 1;
                    bytes.insert(key, value);
                    made.push(make(insns[copy_at].at, &name, Operation::Unary, vec![byte(value)], vec![], vec![value], vec![]));
                }
            }
            let mut tested: Option<u32> = None;
            let mut edits: Vec<(usize, Vec<Arc<Insn>>)> = Vec::new();
            for (index, read) in &readers {
                let one = &insns[*index];
                match read {
                    Read::Cond { saved, inverted, stem } => {
                        let value = bytes[saved];
                        let mut before = Vec::new();
                        if tested != Some(value) {
                            before.push(make(
                                one.at,
                                "test",
                                Operation::Compare,
                                vec![],
                                vec![byte(value), byte(value)],
                                vec![],
                                vec![value],
                            ));
                            tested = Some(value);
                        }
                        let mut changed = (**one).clone();
                        if let Some(what) = changed.what.as_mut() {
                            what.name = Some(format!("{stem}{}", if *inverted { "e" } else { "ne" }));
                        }
                        before.push(Arc::new(changed));
                        edits.push((*index, before));
                    }
                    Read::Carry => {
                        let value = bytes["b"];
                        let again = next;
                        next += 1;
                        let add = make(
                            one.at,
                            "add",
                            Operation::Binary,
                            vec![byte(again)],
                            vec![byte(value), Loc::Imm(Imm { value: 0xff, width: 1, address: None })],
                            vec![again],
                            vec![value],
                        );
                        edits.push((*index, vec![add, Arc::clone(one)]));
                        tested = None;
                    }
                }
            }
            for (index, replacement) in edits.into_iter().rev() {
                insns.splice(index..=index, replacement);
            }
            insns.remove(restore_at);
            insns.splice(copy_at..=copy_at, made);
        }
        *block = block.with_insns(insns);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn pseudo(
        name: &str,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let dests = defines.iter().map(|&v| held(v)).collect();
        let sources = uses.iter().map(|&v| held(v)).collect();
        make(0, name, Operation::Move, dests, sources, defines, uses)
    }

    fn reader(
        name: &str,
        value: u32,
    ) -> Arc<Insn> {
        if name.starts_with('j') {
            return make(0, name, Operation::Branch, vec![], vec![], vec![], vec![]);
        }
        make(0, name, Operation::Unary, vec![byte(value)], vec![], vec![value], vec![])
    }

    /// The flags are copied before `add` clobbers them, restored, then read.
    fn after(readers: &[&str]) -> Vec<String> {
        let mut insns = vec![
            pseudo("flags_copy", vec![1], vec![]),
            make(0, "add", Operation::Binary, vec![held(2)], vec![held(3), held(3)], vec![2], vec![3]),
            pseudo("flags_restore", vec![], vec![1]),
        ];
        insns.extend(readers.iter().zip(10..).map(|(&name, value)| reader(name, value)));
        let body = LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        let done = lowered(&body).unwrap();
        done.blocks[0].insns.iter().map(|one| one.what.as_ref().unwrap().name.clone().unwrap()).collect()
    }

    #[test]
    fn a_condition_read_after_a_clobber_is_a_saved_byte_tested() {
        assert_eq!(after(&["jl"]), ["setl", "add", "test", "jne"]);
    }

    #[test]
    fn the_inverse_condition_reads_the_same_byte_for_equal() {
        assert_eq!(after(&["setl", "setge"]), ["setl", "add", "test", "setne", "sete"]);
    }

    #[test]
    fn a_second_condition_gets_its_own_byte_and_test() {
        assert_eq!(after(&["setl", "sete"]), ["setl", "sete", "add", "test", "setne", "test", "setne"]);
    }

    #[test]
    fn the_carry_is_saved_as_setb_and_made_again_by_an_add() {
        assert_eq!(after(&["adc"]), ["setb", "add", "add", "adc"]);
    }

    #[test]
    fn a_restore_in_another_block_is_refused() {
        let insns = vec![pseudo("flags_copy", vec![1], vec![])];
        let body = LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        assert!(lowered(&body).is_err());
    }
}
