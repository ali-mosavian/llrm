//! Port of `qbopt/frontend/fpstack.py`: the x87 register stack, as values
//! rather than as positions.
//!
//! `ir::St(index)` names `st(i)` relative to wherever the top happens to be,
//! so two mentions of the same index in different nodes are usually
//! different physical registers. This walks a block forward, keeps a stack
//! of value identities, and resolves each `St` to the value actually in that
//! slot.
//!
//! Block-scoped, and the stack at a block's entry is unknown rather than
//! empty: BC leaves values on the x87 stack across a branch, so entering
//! slots are minted as their own values.
//!
//! Nothing here emits. It answers "which value is in st(i) at this op".

use std::fmt;

use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Kind, MirBody, OpCode};
use crate::support::hash::IndexMap;

/// The x87 stack is eight deep and BC never comes close, but a program that
/// overflowed it would wrap rather than fault, so the depth is checked.
pub const DEPTH: i64 = 8;

/// One value living on the x87 stack.
///
/// `at` is where it was computed, or None for one that was already there
/// when the block was entered.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Float {
    pub id: u32,
    pub at: Option<i64>,
}

impl fmt::Display for Float {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "f{}", self.id)?;
        match self.at {
            None => Ok(()),
            Some(at) => write!(formatter, "@{at:#x}"),
        }
    }
}

/// A frozen dataclass hashes as the tuple of its fields; `hash(None)` is
/// CPython 3.13's constant.
impl crate::support::pyset::PyHash for Float {
    fn py_hash(&self) -> i64 {
        use crate::support::pyset::{int_hash, tuple_hash};
        tuple_hash(&[int_hash(i64::from(self.id)), self.at.map_or(4238894112, int_hash)])
    }
}

/// One op's x87 operands, resolved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reading {
    pub at: i64,
    /// st index -> the value there
    pub uses: IndexMap<i64, Float>,
    /// new load or arithmetic result, before any pop
    pub defines: Option<Float>,
    /// what it took off
    pub popped: Vec<Float>,
}

impl Reading {
    fn new(at: i64) -> Self {
        Self { at, uses: IndexMap::default(), defines: None, popped: Vec::new() }
    }
}

/// Every stack slot this operation names, in the order it names them.
///
/// A float operand has no MIR value, so it arrives as `mir::Opaque` with the
/// resource's own name, "st0" and up. The name, never the operand inside it.
fn _slots(operands: &[Arg]) -> Vec<i64> {
    operands
        .iter()
        .filter_map(|one| match one {
            Arg::Opaque(one) => {
                let digits = one.name.strip_prefix("st")?;
                if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                Some(digits.parse().unwrap_or(i64::MAX))
            }
            _ => None,
        })
        .collect()
}

/// Python's nested `at`: the value in `st(index)`, minting entering slots.
fn at(stack: &mut Vec<Float>, known: bool, minted: &mut u32, entering: &mut i64, index: i64) -> Option<Float> {
    if !known || !(0 <= index && index < DEPTH) {
        return None;
    }
    while stack.len() as i64 <= index {
        if *entering >= DEPTH || stack.len() as i64 >= DEPTH {
            return None;
        }
        *minted += 1;
        *entering += 1;
        stack.push(Float { id: *minted, at: None });
    }
    Some(stack[index as usize])
}

/// Which value is in each st(i) the ops of `body` name.
///
/// Forward through each block from an unknown entry stack. An op this cannot
/// model makes the whole stack unknown again rather than shifting it by a
/// guess: a wrong answer here is silent.
pub fn readings(body: &MirBody) -> IndexMap<i64, Reading> {
    let mut out: IndexMap<i64, Reading> = IndexMap::default();
    let mut minted: u32 = 0;

    for block in &body.blocks {
        // Top first. Entering slots are minted lazily, so a block that never
        // reaches past its own pushes never invents one.
        let mut stack: Vec<Float> = Vec::new();
        let mut entering: i64 = 0;
        let mut known = true;

        for op in &block.ops {
            let named = _slots(&op.args);
            let destinations = _slots(&op.results);
            if op.barrier() || op.kind == Kind::Call || (op.stack.is_none() && (!named.is_empty() || !destinations.is_empty()))
            {
                // It touches the stack in a way this does not model.
                known = false;
                stack = Vec::new();
                out.insert(op.at, Reading::new(op.at));
                continue;
            }
            let Some(depth) = op.stack else {
                continue;
            };
            if !matches!(depth, -2..=1) {
                known = false;
                stack = Vec::new();
                out.insert(op.at, Reading::new(op.at));
                continue;
            }

            let mut uses = IndexMap::default();
            for &index in &named {
                let got = at(&mut stack, known, &mut minted, &mut entering, index);
                let Some(got) = got else {
                    known = false;
                    break;
                };
                uses.insert(index, got);
            }
            if !known {
                stack = Vec::new();
                out.insert(op.at, Reading::new(op.at));
                continue;
            }

            let mut made: Option<Float> = None;
            let mut popped: Vec<Float> = Vec::new();
            if depth > 0 {
                if depth != 1 || destinations != [0] || stack.len() as i64 >= DEPTH {
                    known = false;
                    stack = Vec::new();
                    out.insert(op.at, Reading::new(op.at));
                    continue;
                }
                minted += 1;
                made = Some(Float { id: minted, at: Some(op.at) });
                stack.insert(0, made.unwrap());
            } else if !destinations.is_empty() {
                if destinations.len() != 1
                    || !matches!(
                        op.op,
                        Some(OpCode::Operation(Operation::FloatArith | Operation::FloatArithPop | Operation::FloatUnary))
                    )
                    || at(&mut stack, known, &mut minted, &mut entering, destinations[0]).is_none()
                {
                    known = false;
                    stack = Vec::new();
                    out.insert(op.at, Reading::new(op.at));
                    continue;
                }
                minted += 1;
                made = Some(Float { id: minted, at: Some(op.at) });
                stack[destinations[0] as usize] = made.unwrap();
            }
            if depth < 0 {
                let mut removed = Vec::new();
                for _ in 0..-depth {
                    let value = at(&mut stack, known, &mut minted, &mut entering, 0);
                    let Some(value) = value else {
                        known = false;
                        break;
                    };
                    removed.push(value);
                    stack.remove(0);
                }
                if !known {
                    stack = Vec::new();
                    out.insert(op.at, Reading::new(op.at));
                    continue;
                }
                popped = removed;
            }
            out.insert(op.at, Reading { at: op.at, uses, defines: made, popped });
        }
    }
    out
}

/// Pairs of pushes of the same address that are two values, not one.
///
/// Same address, same bytes, and not redundant at all, because each one puts
/// another value on the stack.
pub fn pushed_twice(body: &MirBody) -> Vec<(i64, i64)> {
    let mut found: Vec<(i64, i64)> = Vec::new();
    let reads = readings(body);
    for block in &body.blocks {
        let loads: Vec<_> = block.ops.iter().filter(|op| op.stack.is_some_and(|stack| stack > 0)).collect();
        for (one, other) in loads.iter().zip(loads.iter().skip(1)) {
            if one.loads.is_empty() || other.loads.is_empty() {
                continue;
            }
            if !mir::same_bytes(&one.loads[0], &other.loads[0]) {
                continue;
            }
            let (Some(first), Some(second)) = (reads.get(&one.at), reads.get(&other.at)) else {
                continue;
            };
            if first.defines.is_some() && second.defines.is_some() && first.defines != second.defines {
                found.push((one.at, other.at));
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "fpstack_tests.rs"]
mod tests;
