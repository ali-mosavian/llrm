//! Which addresses a loop's counter indexes, as LLVM's LoopStrengthReduce
//! asks `isLegalAddressingMode`: `base + counter * scale + k` needs no
//! recurrence, no multiply and no register where the target has an address
//! form with that scale, only the form's price at each access.
//!
//! One decision a counter, over every address it derives. Each is carried
//! as a recurrence of its own -- an add a trip and an address register,
//! spilled past the room there is -- or indexed by the counter through a
//! form whose scales hold its own. A form wider than the counter widens
//! it, and each operation on the counter then pays the target's prefix.
//! Priced on the target's costs and address forms alone.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{Affine, AffineOperand, Derived};
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::target::{AddressForm, OperationCosts};

/// What a counter indexes, and how wide it must be to.
#[derive(Debug, Eq, PartialEq)]
pub struct Indexing {
    /// The formulas left to the counter: each address, the constant
    /// offsets from it, and the products only they read.
    pub indexed: BTreeSet<InstId>,
    /// The width in bits the counter is widened to, where the form asks it.
    pub widened: Option<u32>,
}

/// An address the counter may index.
struct Candidate {
    op: InstId,
    scale: i64,
    /// Its accesses, each weighed by its block's trips over the loop's.
    accesses: i64,
    /// Its accesses counted once: a spilled recurrence invariant in a loop
    /// inside is reloaded before it.
    reads: i64,
    /// The base's register, where the base is not the frame's or a symbol.
    base: Option<ValueId>,
    /// Whether only the native form spells it: a wider one widens the base
    /// where it is loaded or passed, which a symbol or a computed base
    /// cannot be.
    native: bool,
}

/// The addresses of `derived` that `counter` indexes more cheaply than
/// recurrences would carry them, with `room` registers for recurrences;
/// None where it indexes none. `widenable` says whether induction proved
/// the counter can be widened; `credited` are the formulas that can take
/// over its control, so that it dies, with its step and register, where
/// it indexes nothing and nothing else reads it. `frequencies` are the
/// blocks' profile-free frequencies: an access in a loop inside weighs its
/// trips.
#[allow(clippy::too_many_arguments)]
pub fn indexing(
    function: &Function,
    loop_: &Loop,
    counter: &Affine,
    derived: &[Derived],
    forms: &[AddressForm],
    costs: &OperationCosts,
    room: i64,
    widenable: bool,
    credited: &BTreeSet<InstId>,
    frequencies: Option<&BTreeMap<i64, i64>>,
) -> Option<Indexing> {
    let narrow = i64::from(counter.start.width());
    let ours = derived.iter().filter(|one| one.of.value == counter.value).collect::<Vec<_>>();
    let results = ours.iter().filter_map(|one| function.instruction(one.op).result).collect::<BTreeSet<_>>();
    let weight = |inst: InstId| {
        let at = function.parent(inst).map(cfg::id);
        let of = |block: Option<i64>| block.and_then(|block| frequencies.and_then(|known| known.get(&block).copied())).unwrap_or(1).max(1);
        (of(at) / of(Some(loop_.header))).max(1)
    };
    let candidates = ours.iter().filter_map(|one| _candidate(function, one, &results, &weight)).collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    let native = forms.first()?;
    // Address registers other accesses of the loop already hold.
    let held = _bases(function, loop_, &candidates);
    let operations = _operations(function, loop_, counter, &results);
    // Every formula of the counter is carried or indexed, so only a reader
    // that is none keeps it once a formula takes over its control.
    let formulas = ours.iter().map(|one| one.op).collect::<BTreeSet<_>>();
    let dies = formulas.iter().any(|op| credited.contains(op)) && !_read_beyond(function, loop_, counter, &formulas);
    let mut best: Option<(i64, usize, i64, Vec<bool>)> = None;
    let widths = forms.iter().map(|form| form.index_width * 8).filter(|&bits| bits == narrow || (bits > narrow && widenable)).collect::<BTreeSet<_>>();
    for bits in widths {
        let Some(form) = forms.iter().find(|form| form.index_width * 8 == bits) else { continue };
        let indexable = |one: &Candidate| form.scales.contains(&one.scale) && (std::ptr::eq(form, native) || !one.native);
        // Cheapest to carry first: the most expensive to index.
        let mut order = (0..candidates.len()).collect::<Vec<_>>();
        order.sort_by_key(|&at| (indexable(&candidates[at]), -(candidates[at].accesses * form.use_cost), -candidates[at].accesses));
        let forced = order.iter().filter(|&&at| !indexable(&candidates[at])).count();
        for carried in forced..=candidates.len() {
            let recurrence = order[..carried].to_vec();
            let indexed = order[carried..].to_vec();
            // Each recurrence and each base an index is added to takes an
            // address register; a native index is one more, the counter's.
            let bases = indexed.iter().filter_map(|&at| candidates[at].base).filter(|base| !held.contains(base)).collect::<BTreeSet<_>>();
            // A frame or symbol base is a displacement, so its recurrences
            // of one scale are one: the scaled counter.
            let mut carried = BTreeMap::<(i64, Option<InstId>), i64>::new();
            for &at in &recurrence {
                let one = &candidates[at];
                *carried.entry((one.scale, one.base.map(|_| one.op))).or_default() += one.reads;
            }
            let registers = (carried.len() + bases.len()) as i64;
            let index = i64::from(!indexed.is_empty() && form.partners.is_some());
            let over = native.address_registers().map_or(0, |count| registers + index - (count - held.len() as i64));
            let keeps = !indexed.is_empty() || !dies;
            let excess = (registers - room - i64::from(!keeps)).max(over - i64::from(!keeps)).max(0) as usize;
            let mut spilled = carried.values().copied().collect::<Vec<_>>();
            spilled.sort();
            let spill = spilled.iter().take(excess).map(|reads| costs.memory_update + reads * costs.load).sum::<i64>();
            let widening = if bits > narrow { operations * costs.prefix } else { 0 };
            let cost = (carried.len() as i64 + i64::from(keeps)) * costs.add + indexed.iter().map(|&at| candidates[at].accesses * form.use_cost).sum::<i64>() + spill + widening;
            let mut chosen = vec![false; candidates.len()];
            for &at in &indexed {
                chosen[at] = true;
            }
            // Ties keep the recurrences, then the narrower counter.
            let key = (cost, indexed.len(), bits);
            if best.as_ref().is_none_or(|(cost, count, width, _)| key < (*cost, *count, *width)) {
                best = Some((key.0, key.1, key.2, chosen));
            }
        }
    }
    let (_, count, bits, chosen) = best?;
    if count == 0 {
        return None;
    }
    let addresses = candidates.iter().zip(&chosen).filter(|(_, chosen)| **chosen).map(|(one, _)| one.op).collect::<BTreeSet<_>>();
    Some(Indexing { indexed: _closed(function, &ours, &addresses), widened: (bits > narrow).then(|| u32::try_from(bits).expect("a width in bits")) })
}

/// `one` as an address the counter may index: a `getelementptr` off an
/// invariant base no other formula computes, a constant multiple of the
/// counter plus a constant, read only by accesses.
fn _candidate(function: &Function, one: &Derived, results: &BTreeSet<ValueId>, weight: &dyn Fn(InstId) -> i64) -> Option<Candidate> {
    let instruction = function.instruction(one.op);
    let Opcode::GetElementPtr { .. } = instruction.opcode else { return None };
    let pointer = one.pointer?;
    let AffineOperand::Const(by) = &one.by else { return None };
    let scale = i64::try_from(&by.n).ok().filter(|&scale| scale > 0)?;
    let constant = one.offsets.iter().all(|(term, _)| matches!(term, AffineOperand::Const(_)));
    if matches!(instruction.operands[0], Operand::Value(base) if results.contains(&base)) {
        return None;
    }
    let (accesses, reads) = _accesses(function, instruction.result?, weight)?;
    let (base, native) = match pointer {
        Operand::Value(pointer) => match _root(function, pointer).map(|root| _kind(function, root)) {
            Some(Base::Frame) => (None, false),
            Some(Base::Register(root, loaded)) => (Some(root), !loaded),
            None => return None,
        },
        _ => (None, true),
    };
    (constant && accesses > 0).then_some(Candidate { op: one.op, scale, accesses, reads, base, native })
}

/// Where an address's base lives.
enum Base {
    /// An offset from the frame pointer.
    Frame,
    /// A pointer in a register, and whether it is loaded or passed in.
    Register(ValueId, bool),
}

fn _kind(function: &Function, root: ValueId) -> Base {
    match function.value(root).def {
        ValueDef::Argument(_) => Base::Register(root, true),
        ValueDef::Instruction(inst) => match function.instruction(inst).opcode {
            Opcode::Alloca { .. } => Base::Frame,
            Opcode::Load { .. } => Base::Register(root, true),
            _ => Base::Register(root, false),
        },
    }
}

/// The loads and stores through `address`, directly or at a constant
/// offset, weighed by `weight` and counted once; None where anything else
/// reads it.
fn _accesses(function: &Function, address: ValueId, weight: &dyn Fn(InstId) -> i64) -> Option<(i64, i64)> {
    let (mut weighed, mut count) = (0, 0);
    for one in function.users(address) {
        let user = function.instruction(one.user);
        let (more, reads) = match user.opcode {
            Opcode::Load { .. } if one.index == 0 => (weight(one.user), 1),
            Opcode::Store { .. } if one.index == 1 => (weight(one.user), 1),
            Opcode::GetElementPtr { .. } if one.index == 0 && user.operands[1..].iter().all(|operand| matches!(operand, Operand::Constant(_))) => _accesses(function, user.result?, weight)?,
            _ => return None,
        };
        weighed += more;
        count += reads;
    }
    Some((weighed, count))
}

/// The value `pointer` offsets by constants.
fn _root(function: &Function, mut pointer: ValueId) -> Option<ValueId> {
    loop {
        let ValueDef::Instruction(inst) = function.value(pointer).def else { return Some(pointer) };
        let instruction = function.instruction(inst);
        match (&instruction.opcode, &instruction.operands[..]) {
            (Opcode::GetElementPtr { .. }, [Operand::Value(base), rest @ ..]) if rest.iter().all(|one| matches!(one, Operand::Constant(_))) => pointer = *base,
            _ => return Some(pointer),
        }
    }
}

/// The loaded or passed-in bases the accesses of `loop_` address through
/// a register.
fn _bases(function: &Function, loop_: &Loop, candidates: &[Candidate]) -> BTreeSet<ValueId> {
    let ours = candidates.iter().filter_map(|one| function.instruction(one.op).result).collect::<BTreeSet<_>>();
    let mut bases = BTreeSet::new();
    for &at in &loop_.body {
        for &inst in function.block(cfg::block(at)).instructions() {
            let instruction = function.instruction(inst);
            let pointer = match instruction.opcode {
                Opcode::Load { .. } => instruction.operands[0],
                Opcode::Store { .. } => instruction.operands[1],
                _ => continue,
            };
            let Operand::Value(pointer) = pointer else { continue };
            if let Some(Base::Register(root, true)) = _root(function, pointer).map(|root| _kind(function, root))
                && !ours.contains(&root)
            {
                bases.insert(root);
            }
        }
    }
    bases
}

/// Whether anything in `loop_` reads the counter or its step but its phi,
/// its step, its tests and `formulas`.
fn _read_beyond(function: &Function, loop_: &Loop, counter: &Affine, formulas: &BTreeSet<InstId>) -> bool {
    let phi = match function.value(counter.value).def {
        ValueDef::Instruction(inst) => inst,
        ValueDef::Argument(_) => return true,
    };
    let steps = function.instruction(phi).operands.iter().filter_map(|operand| match operand {
        Operand::Value(value) => Some(*value),
        _ => None,
    });
    let values = std::iter::once(counter.value).chain(steps.filter(|&value| {
        matches!(function.value(value).def, ValueDef::Instruction(inst) if function.instruction(inst).operands.contains(&Operand::Value(counter.value)))
    }));
    values.collect::<Vec<_>>().into_iter().any(|value| {
        function.users(value).iter().any(|one| {
            let user = function.instruction(one.user);
            let inside = function.parent(one.user).is_some_and(|block| loop_.body.contains(&cfg::id(block)));
            let own = one.user == phi || formulas.contains(&one.user) || matches!(user.opcode, Opcode::ICmp(_)) || user.result.is_some_and(|result| function.users(result).iter().any(|reader| reader.user == phi));
            inside && !own
        })
    })
}

/// How many operations of `loop_` compute on the counter itself: its step
/// and tests, which a wider counter makes wider.
fn _operations(function: &Function, loop_: &Loop, counter: &Affine, formulas: &BTreeSet<ValueId>) -> i64 {
    let mut values = vec![counter.value];
    let mut seen = BTreeSet::new();
    let mut count = 0;
    while let Some(value) = values.pop() {
        if !seen.insert(value) {
            continue;
        }
        for one in function.users(value) {
            let user = function.instruction(one.user);
            let inside = function.parent(one.user).is_some_and(|block| loop_.body.contains(&cfg::id(block)));
            if !inside || user.opcode == Opcode::Phi || user.result.is_some_and(|result| formulas.contains(&result)) {
                continue;
            }
            if matches!(user.opcode, Opcode::Binary(_) | Opcode::ICmp(_)) {
                count += 1;
                // The step's own readers: the counter's next test.
                if user.result.is_some_and(|result| function.users(result).iter().any(|reader| function.instruction(reader.user).opcode == Opcode::Phi)) {
                    values.extend(user.result);
                }
            }
        }
    }
    count
}

/// `addresses`, the formulas computed from them, and every formula of
/// `ours` only they read.
fn _closed(function: &Function, ours: &[&Derived], addresses: &BTreeSet<InstId>) -> BTreeSet<InstId> {
    let mut closed = addresses.clone();
    loop {
        let before = closed.len();
        let values = closed.iter().filter_map(|&inst| function.instruction(inst).result).collect::<BTreeSet<_>>();
        for one in ours {
            if closed.contains(&one.op) {
                continue;
            }
            let instruction = function.instruction(one.op);
            let reads = instruction.operands.iter().any(|operand| matches!(operand, Operand::Value(value) if values.contains(value)));
            let feeds = instruction.result.is_some_and(|result| {
                let users = function.users(result);
                !users.is_empty() && users.iter().all(|user| closed.contains(&user.user))
            });
            if reads || feeds {
                closed.insert(one.op);
            }
        }
        if closed.len() == before {
            return closed;
        }
    }
}

#[cfg(test)]
#[path = "indexing_tests.rs"]
mod tests;
