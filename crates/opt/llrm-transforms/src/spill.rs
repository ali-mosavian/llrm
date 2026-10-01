//! What values past the target's registers cost: the one spill model every
//! pricing of register pressure shares.
//!
//! A value kept in memory costs its traffic there: a store after each
//! definition, a load at each read, and a memory update each time a counter
//! is stepped in place; a value its one instruction rebuilds for less than a
//! load costs that at each read instead. A counter and its step share one
//! cell, as a copy-joined web does, and a phi's own edges into that cell
//! move nothing. Where more values live than the target's registers hold,
//! or than a call leaves across it, the cheapest are spilled, and stay
//! spilled.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;
use llrm_analysis::liveness::{self, Liveness};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::target::OperationCosts;
use llrm_mir::types::Type;

/// How many integer values the target holds in registers, and how many of
/// them a call leaves; none leaves pressure unpriced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Room {
    pub registers: i64,
    pub across_call: i64,
    /// What an access through a far pointer takes besides its address: its selector.
    pub far_access: i64,
}

impl Room {
    pub fn of(outer: &Outer) -> Room {
        Room { registers: outer.target().registers(), across_call: outer.target().call_registers(), far_access: outer.target().far_access_registers() }
    }

    pub fn priced(&self) -> bool {
        self.registers > 0
    }
}

/// A value's traffic in memory, each count weighted by its block's frequency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Traffic {
    pub stores: i64,
    pub updates: i64,
    pub loads: i64,
    /// What rebuilding it costs at a read, where one instruction makes it.
    pub rebuild: Option<i64>,
}

impl Traffic {
    pub fn price(&self, costs: &OperationCosts) -> i64 {
        let cell = self.stores * costs.store + self.updates * costs.memory_update + self.loads * costs.load;
        self.rebuild.map_or(cell, |each| cell.min(self.loads * each))
    }
}

/// Where too many values may live: the registers there, and who lives.
#[derive(Clone, Debug)]
pub struct Point<K> {
    pub registers: i64,
    pub residents: Vec<K>,
}

/// What spilling costs to fit `points`, in order: at each, the cheapest
/// residents past its registers are spilled, and stay spilled.
pub fn spilled<K: Ord + Copy>(points: impl IntoIterator<Item = Point<K>>, price: impl Fn(K) -> i64) -> i64 {
    let mut spilled = BTreeSet::new();
    let mut cost = 0;
    for point in points {
        let resident = point.residents.into_iter().filter(|one| !spilled.contains(one)).collect::<BTreeSet<_>>();
        let excess = resident.len() as i64 - point.registers.max(0);
        if excess <= 0 {
            continue;
        }
        let mut cheapest = resident.into_iter().map(|one| (price(one), one)).collect::<Vec<_>>();
        cheapest.sort();
        for (each, one) in cheapest.into_iter().take(excess as usize) {
            cost += each;
            spilled.insert(one);
        }
    }
    cost
}

/// Whether `inst` leaves only the registers a call does.
pub fn calls(function: &Function, inst: InstId) -> bool {
    matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_))
}

/// How many registers call `inst` keeps: its callee's own count where the
/// call is direct.
pub fn kept_across(outer: &Outer, context: &Context, function: &Function, inst: InstId) -> i64 {
    outer.kept_across(llrm_mir::memory::callee(context, function, inst))
}

/// The registers `inst` needs for its address beyond the values live: a
/// spilled operand is read back into one. They are the runtime pointer it
/// is an offset from, each variable index, and for a far pointer the
/// selector. A symbol or frame object is a displacement.
pub fn transient(context: &Context, layout: &DataLayout, function: &Function, inst: InstId, room: Room) -> i64 {
    let op = function.instruction(inst);
    let address = match op.opcode {
        Opcode::Load { .. } => op.operands.first(),
        Opcode::Store { .. } => op.operands.get(1),
        _ => None,
    };
    let Some(Operand::Value(pointer)) = address else { return 0 };
    let defined = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => Some(function.instruction(def)),
            _ => None,
        },
        _ => None,
    };
    // The access's own `getelementptr` names its variable indices; what it
    // is an offset from is one register, however it was made.
    let mut components = 0;
    let mut base = Operand::Value(*pointer);
    if let Some(op) = defined(base)
        && let (Opcode::GetElementPtr { .. }, [from, indices @ ..]) = (&op.opcode, &op.operands[..])
    {
        components += indices.iter().filter(|index| matches!(index, Operand::Value(_))).count() as i64;
        base = *from;
    }
    // A constant displacement from a symbol or frame object needs none.
    let symbolic = loop {
        match base {
            Operand::Value(_) => match defined(base).map(|op| (&op.opcode, &op.operands[..])) {
                Some((Opcode::GetElementPtr { .. }, [from, indices @ ..])) if indices.iter().all(|index| matches!(index, Operand::Constant(_))) => base = *from,
                Some((Opcode::Cast(CastOp::AddrSpaceCast), [from])) => base = *from,
                Some((Opcode::Alloca { .. }, _)) => break true,
                _ => break false,
            },
            _ => break true,
        }
    };
    components += i64::from(!symbolic);
    components + if words(context, layout, function, *pointer) > 1 { room.far_access } else { 0 }
}

/// Whether `value` takes an integer register: floating values do not, nor
/// does a truth value only its own block's branch reads, which is flags.
pub fn integer(context: &Context, function: &Function, value: ValueId) -> bool {
    match context.types.get(function.value(value).ty) {
        Type::Int(1) => !_flags(function, value),
        Type::Int(_) | Type::Pointer(_) => true,
        _ => false,
    }
}

fn _flags(function: &Function, value: ValueId) -> bool {
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let block = function.parent(def);
    let users = function.users(value);
    !users.is_empty() && users.iter().all(|one| function.instruction(one.user).opcode == Opcode::Br && function.parent(one.user) == block)
}

/// How many stores spill `value`: a pointer wider than its offset, a
/// segment and an offset, takes one each.
pub fn words(context: &Context, layout: &DataLayout, function: &Function, value: ValueId) -> i64 {
    match context.types.get(function.value(value).ty) {
        Type::Pointer(space) => {
            let spec = layout.pointer(*space);
            i64::from((spec.bits / spec.index_bits.max(1)).max(1))
        }
        _ => 1,
    }
}

/// The cell each value is spilled to, by the value that names it: a
/// counter's step, `p + c` or `p - c` taken back into phi `p`, shares `p`'s.
pub fn cells(function: &Function) -> BTreeMap<ValueId, ValueId> {
    let mut cells = BTreeMap::new();
    for &block in function.layout() {
        for &inst in function.block(block).instructions() {
            let phi = function.instruction(inst);
            let (Opcode::Phi, Some(counter)) = (&phi.opcode, phi.result) else { continue };
            for pair in phi.operands.chunks(2) {
                if let Operand::Value(step) = pair[0]
                    && _steps(function, step) == Some(counter)
                {
                    cells.insert(step, counter);
                }
            }
        }
    }
    cells
}

/// The counter `value` steps by a constant, if it does.
fn _steps(function: &Function, value: ValueId) -> Option<ValueId> {
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    let instruction = function.instruction(inst);
    let constant = |operand: Operand| matches!(operand, Operand::Constant(_));
    match (&instruction.opcode, &instruction.operands[..]) {
        (Opcode::Binary(BinaryOp::Add), [Operand::Value(counter), other]) | (Opcode::Binary(BinaryOp::Add), [other, Operand::Value(counter)]) if constant(*other) => Some(*counter),
        (Opcode::Binary(BinaryOp::Sub), [Operand::Value(counter), other]) if constant(*other) => Some(*counter),
        _ => None,
    }
}

fn _cell(cells: &BTreeMap<ValueId, ValueId>, value: ValueId) -> ValueId {
    cells.get(&value).copied().unwrap_or(value)
}

/// Each cell's traffic in `function`, weighted by `frequency`, from the
/// instructions `kept` says stay; a cell of `words` words is stored a word
/// at a time and loaded whole.
pub fn traffic(
    function: &Function,
    frequency: &BTreeMap<i64, i64>,
    cells: &BTreeMap<ValueId, ValueId>,
    costs: &OperationCosts,
    kept: &dyn Fn(InstId) -> bool,
    words: &dyn Fn(ValueId) -> i64,
) -> BTreeMap<ValueId, Traffic> {
    let mut found = BTreeMap::<ValueId, Traffic>::new();
    let often = |block: BlockId| frequency.get(&cfg::id(block)).copied().unwrap_or(1);
    let mut makers = BTreeMap::<ValueId, usize>::new();
    for &block in function.layout() {
        let each = often(block);
        for &inst in function.block(block).instructions() {
            let instruction = function.instruction(inst);
            let result = instruction.result;
            if let Some(value) = result {
                *makers.entry(value).or_default() += 1;
            }
            if !kept(inst) {
                continue;
            }
            let own = result.map(|value| _cell(cells, value));
            if instruction.opcode == Opcode::Phi {
                // Each edge from another cell copies into this one.
                for pair in instruction.operands.chunks(2) {
                    let from = match pair[1] {
                        Operand::Block(from) => often(from),
                        _ => each,
                    };
                    match pair[0] {
                        Operand::Value(value) if Some(_cell(cells, value)) == own => {}
                        Operand::Value(value) => {
                            found.entry(_cell(cells, value)).or_default().loads += from;
                            found.entry(own.expect("a phi's value")).or_default().stores += from;
                        }
                        _ => found.entry(own.expect("a phi's value")).or_default().stores += from,
                    }
                }
                continue;
            }
            if let Some(value) = result
                && cells.contains_key(&value)
            {
                found.entry(_cell(cells, value)).or_default().updates += each;
                continue;
            }
            for operand in &instruction.operands {
                if let Operand::Value(value) = *operand {
                    found.entry(_cell(cells, value)).or_default().loads += each;
                }
            }
            if let Some(cell) = own {
                found.entry(cell).or_default().stores += each;
            }
        }
    }
    for (value, one) in &mut found {
        one.stores *= words(*value);
        if makers.get(value) == Some(&1) {
            one.rebuild = _rebuild(function, *value, costs);
        }
    }
    found
}

/// What making `value` again costs, where one instruction does from
/// nothing kept: a frame or symbol address, or a far view of one, which
/// also loads its segment.
fn _rebuild(function: &Function, value: ValueId, costs: &OperationCosts) -> Option<i64> {
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    let instruction = function.instruction(inst);
    let constant = instruction.operands.iter().all(|operand| matches!(operand, Operand::Constant(_)));
    match instruction.opcode {
        Opcode::Alloca { .. } => Some(costs.address),
        Opcode::GetElementPtr { .. } if constant => Some(costs.address),
        Opcode::Cast(CastOp::AddrSpaceCast) => match instruction.operands[0] {
            Operand::Value(from) => _rebuild(function, from, costs).map(|near| near + costs.r#move),
            _ => Some(costs.address + costs.r#move),
        },
        _ => None,
    }
}

/// A point of an instruction: the cells `counted` says live before it, in
/// the target's registers, and for a call, those live across it, in the
/// registers `across` says it keeps.
pub struct Site {
    pub inst: InstId,
    pub before: Point<ValueId>,
    pub across: Option<Point<ValueId>>,
}

/// Each instruction's site in `block` but its phis, in order.
pub fn sites(
    function: &Function,
    found: &Liveness,
    block: BlockId,
    room: Room,
    across: &dyn Fn(InstId) -> i64,
    transient: &dyn Fn(InstId) -> i64,
    cells: &BTreeMap<ValueId, ValueId>,
    counted: &dyn Fn(ValueId) -> bool,
) -> Vec<Site> {
    let residents = |live: BTreeSet<ValueId>| live.into_iter().filter(|&one| counted(one)).map(|one| _cell(cells, one)).collect::<BTreeSet<_>>().into_iter().collect();
    liveness::live_points(function, found, block)
        .into_iter()
        .map(|(inst, before, past)| Site {
            inst,
            before: Point { registers: room.registers - transient(inst), residents: residents(before) },
            across: calls(function, inst).then(|| Point { registers: across(inst), residents: residents(past) }),
        })
        .collect()
}

impl Site {
    /// Its points, in order.
    pub fn points(self) -> impl Iterator<Item = Point<ValueId>> {
        std::iter::once(self.before).chain(self.across)
    }
}

#[cfg(test)]
#[path = "spill_tests.rs"]
mod tests;
