//! `setcc` into a register zeroed before the compare, not a `movzx` of a byte
//! after it: `X86FixupSetCC`. Translated from
//! [X86FixupSetCC.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupSetCC.cpp#L68)
//! (`fixupSetCC`).
//!
//! # How it works
//! In each block the latest instruction that writes the flags is remembered.
//! A `setcc` after one, whose byte is read by one `movzx` and by nothing else,
//! becomes: a zero made just before the flag-writing instruction (a `mov` of 0,
//! `xor` after allocation), and the `setcc` writing the low byte of that
//! register; the `movzx` goes and its readers read the register. The pin
//! refuses when the flag-writing instruction also reads the flags (the zero
//! would have to clobber them there, which the `xor` does).
//!
//! `xor eax,eax; cmp; setl al` is shorter than `cmp; setl al; movzx eax,al`
//! and has no partial-register write to stall on.
//!
//! DEVIATE F1 (no number yet): the pin keeps the `setcc` result and inserts it
//! into the zeroed register as a sub-register (`INSERT_SUBREG`), which its
//! coalescer then joins. LIR has no sub-register insert: the `setcc` here
//! names the register's own value as written and read (one value defined
//! twice, as phi elimination leaves them), so the allocator gives both the
//! same register. Only a byte read once by the `movzx` is rewritten. Exit:
//! sub-register values in LIR.
//!
//! # Example
//! `int f(int a, int b) { return a < b; }` at `-m32`, `ISEL_DUMP=1`.
//! Before:
//!
//! ```lir
//! v1 := mov [[bp+0x8]]
//! cmp v1, [[bp+0xc]]
//! v3 := setl
//! v4 := movzx v3
//! ```
//!
//! After:
//!
//! ```lir
//! v5 := mov 0
//! v1 := mov [[bp+0x8]]
//! cmp v1, [[bp+0xc]]
//! v5 := setl          ; the low byte of v5
//! ```
//!
//! # Runs
//! After `MachineScheduler`, before the float phases. Optimizing levels only.

use std::sync::Arc;

use crate::backend::liveness;
use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;

pub struct FixupSetCc;

impl LIRTransform for FixupSetCc {
    fn class_name(&self) -> &'static str {
        "FixupSetCC"
    }

    fn name(&self) -> &str {
        "fixupsetcc"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(fixed(&body))
    }
}

/// Whether `one` is a `setcc` into a byte value: that value.
fn set_byte(one: &Insn) -> Option<u32> {
    let what = one.what.as_ref()?;
    let name = what.name.as_deref()?;
    let ([Loc::Held(into)], []) = (what.dests.as_slice(), what.sources.as_slice()) else { return None };
    (what.op == Operation::Unary && name.starts_with("set") && into.width == 1).then_some(into.value)
}

/// `movzx into, byte`: `into`.
fn zero_extend_of(
    one: &Insn,
    byte: u32,
) -> Option<Held> {
    let what = one.what.as_ref()?;
    let ([Loc::Held(into)], [Loc::Held(from)]) = (what.dests.as_slice(), what.sources.as_slice()) else { return None };
    (what.name.as_deref() == Some("movzx") && from.value == byte && from.width == 1 && into.width == 4).then_some(*into)
}

/// `body` with its `setcc`-and-`movzx` pairs made zero-then-`setcc`.
pub fn fixed(body: &LirBody) -> LirBody {
    let regs = body.regs();
    let bits = body.bits;
    // Every reader of each value, to find a byte read once.
    let mut readers: llrm_mir::hash::HashMap<u32, usize> = Default::default();
    let mut definers: llrm_mir::hash::HashMap<u32, usize> = Default::default();
    for block in &body.blocks {
        for phi in block.phis.iter() {
            for &(_, value) in &phi.incoming {
                *readers.entry(value).or_default() += 1;
            }
            *definers.entry(phi.result).or_default() += 1;
        }
        for one in block.insns.iter() {
            for &value in &one.uses {
                *readers.entry(value).or_default() += 1;
            }
            for &value in &one.defines {
                *definers.entry(value).or_default() += 1;
            }
        }
    }
    let mut next = body
        .blocks
        .iter()
        .flat_map(|b| b.insns.iter())
        .flat_map(|one| one.defines.iter().chain(&one.uses))
        .copied()
        .max()
        .unwrap_or(0)
        + 1;
    let flag_lane = |lanes: &crate::backend::lanes::Lanes| {
        lanes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None)
    };

    let every_flag = crate::backend::lanes::Lanes::read_flags();

    let mut out = body.clone();
    for block in &mut out.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
        let mut flags_def: Option<usize> = None;
        let mut index = 0;
        while index < insns.len() {
            let one = Arc::clone(&insns[index]);
            let effect = liveness::effect(regs, bits, &one);
            let writes_flags = effect.as_ref().is_some_and(|e| flag_lane(&e.writes));
            // An instruction of unknown effect might write the flags: the
            // instruction before is no longer the latest definition we know.
            if effect.is_none() {
                flags_def = None;
            }
            let candidate = (|| {
                let byte = set_byte(&one)?;
                let flags_at = flags_def?;
                if readers.get(&byte).copied() != Some(1) || definers.get(&byte).copied() != Some(1) {
                    return None;
                }
                // The flag-writing instruction must not read the flags.
                let def_effect = liveness::effect(regs, bits, &insns[flags_at])?;
                if flag_lane(&def_effect.all_reads()) {
                    return None;
                }
                let at = (index + 1..insns.len()).find(|&later| zero_extend_of(&insns[later], byte).is_some())?;
                let into = zero_extend_of(&insns[at], byte)?;
                // Nothing between the set and the movzx may touch the result
                // value.
                if insns[index + 1..at]
                    .iter()
                    .any(|between| between.defines.contains(&into.value) || between.uses.contains(&into.value))
                {
                    return None;
                }
                Some((flags_at, at, into))
            })();
            if let Some((flags_at, at, into)) = candidate {
                // The value the zero and the byte share is the movzx's result.
                let zero = into.value;
                let _ = &mut next;
                let at_flags = insns[flags_at].at;
                let make_zero = Arc::new(Insn::new(
                    at_flags,
                    Some((at_flags, at_flags)),
                    Some(Semantics {
                        name: Some("mov".into()),
                        dests: vec![Loc::Held(Held { value: zero, width: 4 })],
                        sources: vec![Loc::Imm(Imm { value: 0, width: 4, address: None })],
                        ..Semantics::new(Operation::Move)
                    }),
                    vec![zero],
                    vec![],
                ));
                let mut set = (*one).clone();
                if let Some(what) = set.what.as_mut() {
                    what.dests = vec![Loc::Held(Held { value: zero, width: 1 })];
                }
                set.defines = vec![zero];
                set.uses = vec![zero];
                insns[index] = Arc::new(set);
                insns.remove(at);
                insns.insert(flags_at, make_zero);
                // The zero is before `flags_at`, so every later index moved by
                // one.
                flags_def = Some(flags_at + 1);
                index += 2;
                continue;
            }
            if writes_flags {
                // `inc` leaves CF: a `setb` after it reads an earlier carry,
                // which a zero put before `inc` would clear.
                let all = effect.as_ref().is_some_and(|e| every_flag.is_subset(&e.writes));
                flags_def = all.then_some(index);
            }
            index += 1;
        }
        *block = block.with_insns(insns);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    fn held(
        value: u32,
        width: u32,
    ) -> Loc {
        Loc::Held(Held { value, width })
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

    /// The names of `cmp; [inc]; setb; movzx` after the pass.
    fn names(between: Option<Arc<Insn>>) -> Vec<String> {
        let mut insns =
            vec![insn(0, Operation::Compare, "cmp", vec![], vec![held(1, 4), held(2, 4)], vec![], vec![1, 2])];
        insns.extend(between);
        insns.push(insn(2, Operation::Unary, "setb", vec![held(3, 1)], vec![], vec![3], vec![]));
        insns.push(insn(3, Operation::Extend, "movzx", vec![held(6, 4)], vec![held(3, 1)], vec![6], vec![3]));
        let body = LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        let done = fixed(&body);
        done.blocks[0].insns.iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
    }

    /// `cmp; inc; setb`: `inc` leaves CF, so `setb` reads the compare's carry.
    /// The zero went before the `inc` (an `xor` after allocation, which clears
    /// CF), and `setb` read zero's carry.
    #[test]
    fn test_a_setcc_reading_a_carry_an_inc_left_keeps_its_movzx() {
        assert_eq!(names(None), ["mov", "cmp", "setb"], "the control: nothing between");
        let inc = insn(1, Operation::Unary, "inc", vec![held(4, 4)], vec![held(5, 4)], vec![4], vec![5]);
        assert_eq!(names(Some(inc)), ["cmp", "inc", "setb", "movzx"]);
    }
}
