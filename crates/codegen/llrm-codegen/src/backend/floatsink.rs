//! A float load moves down to the instruction that reads it.
//!
//! # How it works
//! `fld a; fld b; fmul a, K; fadd _, b` leaves `a` below `b`, so the multiply
//! needs `fxch` to reach `a`. The load of `b` can wait until the add: `fld a;
//! fmul K; fld b; faddp` has no exchange. A load whose only reader is later in
//! its block, with a float operation between, is moved to just before that
//! reader, past nothing that may write its cell.
//!
//! The load stays where it is when
//!
//! - it is volatile, clobbers, is part of a group, or is read more than once or
//!   outside its block;
//! - between it and the reader there is a call, a barrier, an escape, a
//!   volatile access, a write to the cell, or a definition of what the load's
//!   address reads;
//! - nothing between is float arithmetic (nothing to exchange around);
//! - the target's `x87_exchange` price is under 2 clocks (P5 and later pair
//!   it), or the module is BASIC (a signalling NaN would fault elsewhere).
//!
//! DEVIATE SD-SINK: the pin has no such step; clang's `fld; fld; fxch; fmul`
//! is the same. Reason: 4 clocks per exchange on the 486 (x_ld_axpy -O3
//! 512 exchanges, 2,048 clocks). Owner llrm-target-x87, exit 2026-11-20.
//!
//! # Runs
//! After `FloatFold` (so a load that folds has folded), before `FloatAssign`.

use std::sync::Arc;

use crate::backend::floatalloc::{_loads_memory, cell_of};
use crate::backend::storedhomes::may_write;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::HashMap;

pub struct FloatSink<'a> {
    pub cpu: &'a crate::backend::cpu::Profile,
    pub basic_semantics: bool,
}

impl LIRTransform for FloatSink<'_> {
    fn class_name(&self) -> &'static str {
        "FloatSink"
    }

    fn name(&self) -> &str {
        "floatsink"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        if self.basic_semantics || self.cpu.cost("x87_exchange").unwrap_or(0) < 2 {
            return Ok(body);
        }
        Ok(sunk(&body))
    }
}

/// `body` with each float load that waits for its reader moved to it.
pub fn sunk(body: &LirBody) -> LirBody {
    let mut uses: HashMap<u32, usize> = HashMap::default();
    let mut defs: HashMap<u32, usize> = HashMap::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        for (arg, read) in one.what.iter().flat_map(|what| {
            what.sources.iter().map(|arg| (arg, true)).chain(what.dests.iter().map(|arg| (arg, false)))
        }) {
            if let Loc::Held(held) = arg
                && held.width == 10
            {
                *uses.entry(held.value).or_default() += usize::from(read);
                *defs.entry(held.value).or_default() += usize::from(!read);
            }
        }
    }
    let mut blocks = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
        let mut at = 0;
        while at < insns.len() {
            match reader_of(&insns, at, &uses, &defs) {
                Some(reader) => {
                    let load = insns.remove(at);
                    insns.insert(reader - 1, load);
                    changed = true;
                }
                None => at += 1,
            }
        }
        blocks.push(block.with_insns(insns));
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// Where the load at `at` is read, if it can wait for it: an index past the
/// next instruction.
fn reader_of(
    insns: &[Arc<Insn>],
    at: usize,
    uses: &HashMap<u32, usize>,
    defs: &HashMap<u32, usize>,
) -> Option<usize> {
    let load = &insns[at];
    if !_loads_memory(load.what.as_ref()) || load.volatile || !load.clobbers.is_empty() || load.group.is_some() {
        return None;
    }
    let value = load.defines.first().copied()?;
    if uses.get(&value) != Some(&1) || defs.get(&value) != Some(&1) {
        return None;
    }
    let reader = (at + 1..insns.len()).find(|&next| insns[next].uses.contains(&value))?;
    if reader == at + 1 || insns[reader].group.is_some() {
        return None;
    }
    let cell = cell_of(load);
    let between = &insns[at + 1..reader];
    let blocked = between
        .iter()
        .any(
            |one| one.defines.iter().any(|defined| load.uses.contains(defined))
                || one.volatile
                || one.call.is_some()
                || may_write(one, cell, false)
                || one
                    .what
                    .as_ref()
                    .is_none_or(|what| matches!(
                        what.op,
                        Operation::Call | Operation::Barrier | Operation::Escape
                    )),
        );
    let exchange = between.iter().any(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::FloatArith));
    (!blocked && exchange).then_some(reader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Addr, Held, Mem, Semantics, Space};

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 10 })
    }

    fn global(symbol: i64) -> Mem {
        Mem::new(Some(Addr { index: symbol, ..Addr::new(Space::External, 0) }), 10)
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Arc<Insn> {
        let value = |args: &[Loc]| -> Vec<u32> {
            args.iter()
                .filter_map(|arg| match arg {
                    Loc::Held(held) => Some(held.value),
                    _ => None,
                })
                .collect()
        };
        let (defines, uses) = (value(&dests), value(&sources));
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, None, Some(what), defines, uses))
    }

    fn fld(
        at: i64,
        value: u32,
        symbol: i64,
    ) -> Arc<Insn> {
        insn(at, Operation::FloatLoad, "fld", vec![held(value)], vec![Loc::Mem(global(symbol))])
    }

    fn arith(
        at: i64,
        name: &str,
        result: u32,
        left: u32,
        right: u32,
    ) -> Arc<Insn> {
        insn(at, Operation::FloatArith, name, vec![held(result)], vec![held(left), held(right)])
    }

    fn fstp(
        at: i64,
        value: u32,
        symbol: i64,
    ) -> Arc<Insn> {
        insn(at, Operation::FloatStore, "fstp", vec![Loc::Mem(global(symbol))], vec![held(value)])
    }

    fn one_block(insns: Vec<Arc<Insn>>) -> LirBody {
        LirBody::new(
            "sink",
            0,
            vec![crate::model::lir::Block::from(crate::model::lir::LirBlock::new(0, insns))],
            Default::default(),
            Default::default(),
        )
    }

    fn order(body: &LirBody) -> Vec<i64> {
        body.blocks.iter().flat_map(|block| block.insns.iter()).map(|one| one.at).collect()
    }

    /// x_ld_axpy -O3 ran `fld x; fld y; fxch; fmul K; faddp` 512 times: the
    /// load of `y` sat below the multiply that wants `x` on top. It waits for
    /// the add.
    #[test]
    fn test_a_load_waits_for_its_reader_past_arithmetic() {
        let body = one_block(vec![
            fld(0, 1, 0),
            fld(1, 2, 1),
            insn(2, Operation::FloatLoad, "fld1", vec![held(3)], vec![]),
            arith(3, "fmul", 4, 1, 3),
            arith(4, "fadd", 5, 4, 2),
            fstp(5, 5, 1),
        ]);
        assert_eq!(order(&sunk(&body)), [0, 2, 3, 1, 4, 5]);
    }

    /// A store to the cell between the load and its reader changes the value:
    /// the load stays.
    #[test]
    fn test_a_load_does_not_move_past_a_write_to_its_cell() {
        let body = one_block(vec![
            fld(0, 1, 0),
            arith(1, "fadd", 3, 1, 1),
            fstp(2, 3, 0),
            arith(3, "fadd", 4, 1, 1),
            fstp(4, 4, 2),
        ]);
        assert_eq!(order(&sunk(&body)), [0, 1, 2, 3, 4]);
    }

    /// Nothing to exchange around: loads side by side stay in order.
    #[test]
    fn test_loads_with_no_arithmetic_between_stay_in_order() {
        let body = one_block(vec![fld(0, 1, 0), fld(1, 2, 1), fld(2, 3, 2), arith(3, "fadd", 4, 1, 3), fstp(4, 4, 0)]);
        assert_eq!(order(&sunk(&body)), [0, 1, 2, 3, 4]);
    }
}
