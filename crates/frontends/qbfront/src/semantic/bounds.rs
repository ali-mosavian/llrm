//! LBOUND and UBOUND.
//!
//! A bound is read from the descriptor, at the record its dimension names
//! where the declaration states the rank. Where this procedure's own DIM or
//! REDIM reached the read, the bound is what that statement handed the
//! runtime: each allocation also writes its bounds and rank to locals the
//! frontend holds per array, which promote and GVN fold.
//!
//! The held bounds are dropped wherever another agent may have changed the
//! array: ERASE, a DIM or REDIM before it returns, a call of a procedure that
//! can reach the array, and any entry the runtime makes, as RESUME does.
//!
//! B$LBND and B$UBND run only to raise "Subscript out of range": always for a
//! dimension past the declared rank, and under `-fsanitize=bounds` wherever
//! the array may be unallocated or the dimension outside its rank.

use std::collections::{BTreeMap, BTreeSet};

use super::tags::{Passing, Slot, Tag};
use super::{Compiler, Instruction, Number, Operand, Place, SemanticError, Variable, BYTE, INTEGER, LONG};

/// An array, by its descriptor: a place, or an array parameter's pointer.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Held {
    Place(u32),
    Parameter(u32),
}

impl Held {
    pub(super) fn of(variable: &Variable) -> Option<Self> {
        variable.descriptor_place.map(Self::Place).or(variable.descriptor.map(Self::Parameter))
    }
}

/// The locals one array's bounds are held in: its rank, 0 where nothing is
/// held, then each dimension's lower and upper bound.
#[derive(Default)]
pub(super) struct Holding {
    rank: u32,
    bounds: Vec<(u32, u32)>,
}

/// What this procedure holds, and the statements that change it.
#[derive(Default)]
pub(super) struct Holdings {
    held: BTreeMap<Held, Holding>,
    /// Each DIM, REDIM or ERASE call by instruction: the array, and the
    /// bounds a DIM or REDIM handed it, source dimension first.
    changes: BTreeMap<u32, (Held, Option<Vec<(Operand, Operand)>>)>,
}

impl Compiler {
    /// The last emitted call (re)allocates `held` with `bounds`, or with none
    /// releases it.
    pub(super) fn holds(&mut self, held: Held, bounds: Option<Vec<(Operand, Operand)>>) {
        let call = self.blocks[self.current_block].instructions.last().expect("the call was just emitted").id;
        self.holdings.changes.insert(call, (held, bounds));
    }

    /// A local of this procedure's own frame, whatever its variables' storage.
    fn held_local(&mut self) -> u32 {
        let id = self.next_place;
        self.next_place += 1;
        let offset = self.place_offset("local", 2);
        self.data_offset += 2;
        self.places.push(Place {
            volatile: false,
            id,
            name: format!("$held{id}"),
            type_id: INTEGER,
            offset,
            extent: 2,
            storage: "local",
            symbol: 0,
        });
        id
    }

    /// The locals holding dimension `dimension`'s bounds of `held`, and its rank's.
    fn holding(&mut self, held: Held, dimension: usize) -> (u32, (u32, u32)) {
        if !self.holdings.held.contains_key(&held) {
            let rank = self.held_local();
            self.holdings.held.insert(held, Holding { rank, bounds: Vec::new() });
        }
        while self.holdings.held[&held].bounds.len() < dimension {
            let pair = (self.held_local(), self.held_local());
            self.holdings.held.get_mut(&held).expect("inserted").bounds.push(pair);
        }
        let holding = &self.holdings.held[&held];
        (holding.rank, holding.bounds[dimension - 1])
    }

    /// Write the held bounds where they change, and drop them where they may
    /// have changed unseen: at the entry and `entries`, after a call of a
    /// procedure that can reach the array. An error handler may run on
    /// another procedure's frame, so what it reaches holds nothing.
    pub(super) fn place_holdings(&mut self, entries: &[u32]) {
        let Holdings { held, changes } = std::mem::take(&mut self.holdings);
        if held.is_empty() {
            return;
        }
        let storage: BTreeMap<u32, &'static str> = self.places.iter().map(|place| (place.id, place.storage)).collect();
        let named: BTreeMap<u32, Held> = self
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|one| match (one.op, one.results.as_slice(), one.operands.as_slice()) {
                ("address", [result], [Operand::Place(place)]) => Some((*result, Held::Place(*place))),
                _ => None,
            })
            .collect();
        // Another procedure can name only an array this one does not own.
        let shared: BTreeSet<Held> = held
            .keys()
            .copied()
            .filter(|one| !matches!(one, Held::Place(place) if storage.get(place) == Some(&"local")))
            .collect();
        let user_calls: BTreeSet<u32> =
            self.calls.iter().filter(|call| call.callee.is_some()).map(|call| call.instruction).collect();
        let mut next = self.next_instruction;
        let mut store = |place: u32, value: Operand, line: usize| {
            next += 1;
            Instruction {
                id: next - 1,
                op: "store",
                results: Vec::new(),
                operands: vec![Operand::Place(place), value],
                callee: None,
                tag: None,
                nowrap: false,
                line,
            }
        };
        let zero = || Operand::Constant(INTEGER, Number::Integer(0));
        let mut handling: BTreeSet<u32> = BTreeSet::new();
        let mut pending: Vec<u32> = self.error_handlers.iter().copied().collect();
        while let Some(one) = pending.pop() {
            if handling.insert(one) {
                let block = self.blocks.iter().find(|block| block.id == one);
                pending.extend(block.and_then(|block| block.terminator.as_ref()).map_or(&[][..], |one| &one.targets));
            }
        }
        let places: BTreeSet<u32> =
            held.values().flat_map(|one| std::iter::once(one.rank).chain(one.bounds.iter().flat_map(|&(low, high)| [low, high]))).collect();
        for block in &mut self.blocks {
            if handling.contains(&block.id) {
                for one in &mut block.instructions {
                    if matches!((one.op, one.operands.as_slice()), ("load", [Operand::Place(place)]) if places.contains(place)) {
                        one.op = "copy";
                        one.operands = vec![zero()];
                    }
                }
                continue;
            }
            let mut out = Vec::with_capacity(block.instructions.len());
            if block.id == 1 || entries.contains(&block.id) {
                out.extend(held.values().map(|one| store(one.rank, zero(), 0)));
            }
            for one in std::mem::take(&mut block.instructions) {
                let line = one.line;
                let change = changes.get(&one.id).and_then(|(array, bounds)| Some((held.get(array)?, bounds.clone())));
                let mut dropped: Vec<Held> = Vec::new();
                if user_calls.contains(&one.id) {
                    dropped.extend(shared.iter().copied());
                    if let Some(Tag::Invoke { arguments }) = &one.tag {
                        for passing in arguments {
                            if let Passing::Array(value) = passing {
                                dropped.extend(named.get(value).copied().or(Some(Held::Parameter(*value))));
                            }
                        }
                    }
                }
                if let Some((holding, _)) = &change {
                    // A DIM or REDIM that raises leaves nothing held.
                    out.push(store(holding.rank, zero(), line));
                }
                out.push(one);
                if let Some((holding, Some(bounds))) = change {
                    for (&(lower, upper), (low, high)) in holding.bounds.iter().zip(&bounds) {
                        out.push(store(lower, low.clone(), line));
                        out.push(store(upper, high.clone(), line));
                    }
                    let rank = Operand::Constant(INTEGER, Number::Integer(bounds.len() as i64));
                    out.push(store(holding.rank, rank, line));
                }
                for array in dropped {
                    if let Some(holding) = held.get(&array) {
                        out.push(store(holding.rank, zero(), line));
                    }
                }
            }
            block.instructions = out;
        }
        self.next_instruction = next;
    }

    /// LBOUND or UBOUND of `variable`'s dimension `dimension`.
    pub(super) fn array_bound(&mut self, variable: &Variable, dimension: Operand, upper: bool) -> Result<Operand, SemanticError> {
        let descriptor = self.descriptor_pointer(variable)?;
        let result = self.compiler_temporary("$bound", INTEGER)?;
        let done = self.new_block();
        let constant = match dimension {
            Operand::Constant(_, Number::Integer(value)) => Some(value),
            _ => None,
        };
        let raise = |compiler: &mut Self| -> Result<(), SemanticError> {
            let called = compiler.value(INTEGER);
            compiler.emit_runtime_call(
                if upper { "B$UBND" } else { "B$LBND" },
                vec![called],
                vec![Operand::Value(descriptor), dimension.clone()],
            );
            compiler.emit("store", Vec::new(), vec![Operand::Place(result), Operand::Value(called)]);
            compiler.terminate("jump", Vec::new(), vec![done])
        };
        let past = |rank: usize| constant.is_some_and(|value| value < 1 || value > rank as i64);
        if constant.is_some_and(|value| value < 1) || variable.rank.is_some_and(past) {
            raise(self)?;
            return self.bound_done(done, result);
        }

        // What this procedure's own DIM or REDIM handed the runtime.
        if let (Some(held), Some(value)) = (Held::of(variable), constant) {
            let (rank, (lower, upper_place)) = self.holding(held, value as usize);
            let rank = self.computed("load", INTEGER, vec![Operand::Place(rank)]);
            let holds = self.computed("ge", super::BOOLEAN, vec![rank, dimension.clone()]);
            let (from_held, from_descriptor) = (self.new_block(), self.new_block());
            self.terminate("branch", vec![holds], vec![from_held, from_descriptor])?;
            self.select_block(from_held);
            let bound = self.computed("load", INTEGER, vec![Operand::Place(if upper { upper_place } else { lower })]);
            self.emit("store", Vec::new(), vec![Operand::Place(result), bound]);
            self.terminate("jump", Vec::new(), vec![done])?;
            self.select_block(from_descriptor);
        }

        let call = self.options.checked_arrays.then(|| self.error_block());
        if let Some(call) = call {
            let data = self.descriptor_field(descriptor, 2, INTEGER);
            let allocated = self.computed("ne", super::BOOLEAN, vec![Operand::Value(data), Operand::Constant(INTEGER, Number::Integer(0))]);
            let read = self.new_block();
            self.terminate("branch", vec![allocated], vec![read, call])?;
            self.select_block(read);
        }
        let rank = match variable.rank {
            Some(rank) => Operand::Constant(INTEGER, Number::Integer(rank as i64)),
            None => {
                let rank = self.descriptor_field(descriptor, 8, BYTE);
                self.convert(Operand::Value(rank), BYTE, INTEGER)?
            }
        };
        if let Some(call) = call {
            // A constant dimension is at least 1, and every allocated array
            // has a first dimension.
            let mut tests = Vec::new();
            if constant.is_none() {
                tests.push(("ge", Operand::Constant(INTEGER, Number::Integer(1))));
            }
            if constant.is_none() || (variable.rank.is_none() && constant != Some(1)) {
                tests.push(("le", rank.clone()));
            }
            for (op, limit) in tests {
                let within = self.computed(op, super::BOOLEAN, vec![dimension.clone(), limit]);
                let next = self.new_block();
                self.terminate("branch", vec![within], vec![next, call])?;
                self.select_block(next);
            }
        }

        // As B$LBND: dimension d reads record cDims - d, which under /R holds
        // source dimension cDims + 1 - d, as in BC.
        let (lower, count) = match (&rank, constant) {
            (Operand::Constant(_, Number::Integer(rank)), Some(value)) => {
                let record = (rank - value) as usize;
                let lower = self.descriptor_field(descriptor, 16 + 4 * record, INTEGER);
                let count = upper.then(|| self.descriptor_field(descriptor, 14 + 4 * record, INTEGER));
                (Operand::Value(lower), count.map(Operand::Value))
            }
            _ => {
                let entry = self.computed("sub", INTEGER, vec![rank, dimension.clone()]);
                let bytes = self.computed("mul", INTEGER, vec![entry, Operand::Constant(INTEGER, Number::Integer(4))]);
                let pointer_type = self.values.iter().find_map(|(id, type_id)| (*id == descriptor).then_some(*type_id)).expect("descriptor value");
                let offset_type = if self.width(pointer_type) == 4 { LONG } else { INTEGER };
                let bytes = self.convert(bytes, INTEGER, offset_type)?;
                let at = self.value(pointer_type);
                self.emit("ptr_offset", vec![at], vec![Operand::Value(descriptor), bytes]);
                let mut field = |offset, slot: Option<Slot>| {
                    let read = self.computed("load", INTEGER, vec![Operand::Indirect { base: at, offset, type_id: INTEGER, volatile: false, inbounds: false }]);
                    if let Some(field) = slot {
                        self.tag_last(Tag::DescriptorField { descriptor, field });
                    }
                    read
                };
                let dimension = constant.map(|value| value as usize);
                let lower = field(16, dimension.map(Slot::LowerOf));
                let count = upper.then(|| field(14, dimension.map(Slot::CountOf)));
                (lower, count)
            }
        };
        let value = match count {
            Some(count) => {
                let end = self.computed("add", INTEGER, vec![lower, count]);
                self.computed("sub", INTEGER, vec![end, Operand::Constant(INTEGER, Number::Integer(1))])
            }
            None => lower,
        };
        self.emit("store", Vec::new(), vec![Operand::Place(result), value]);
        self.terminate("jump", Vec::new(), vec![done])?;
        if let Some(call) = call {
            self.select_block(call);
            raise(self)?;
        }
        self.bound_done(done, result)
    }

    fn bound_done(&mut self, done: u32, result: u32) -> Result<Operand, SemanticError> {
        self.select_block(done);
        Ok(self.computed("load", INTEGER, vec![Operand::Place(result)]))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{built, Options};
    use crate::{parse, Dialect};

    /// A REDIM of another rank compiled, so LBOUND's record, fixed by the
    /// first rank, read another dimension's. BC refuses it.
    #[test]
    fn test_a_redim_of_another_rank_is_refused() {
        let module = parse("DEFINT A-Z\nREDIM a(5)\nREDIM a(2, 2)\nPRINT UBOUND(a)\n", Dialect::VbDos).expect("parses");
        let refused = built(&module, "T", Dialect::VbDos, "vbdos", &Options::default()).err().expect("refused");
        assert!(refused.message.contains("wrong number of dimensions"), "{}", refused.message);
    }
}
