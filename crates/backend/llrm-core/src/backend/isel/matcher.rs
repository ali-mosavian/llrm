//! The generated selector's runtime: an instruction's features, the walk
//! of the automaton `generator` built from `patterns.isel`, the choice
//! among the patterns that match, and what the patterns call by name --
//! operand constructors (`op_`), predicates (`is_`), costs (`cost_`) and
//! the hooks (`hook_`) for what is not pattern-shaped.

use std::sync::Arc;

use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::{CastOp, ConstantKind, Opcode, Type, TypeId};

use super::{
    Convention, FLOAT, Pointer, Selector, Test, TypeClass, Unselected, float_conditions, insn, insn_of, refuse,
    semantics,
};
use crate::backend::arithmetic;
use crate::model::ir::{Held, Imm, Loc, Operation};
use crate::model::lir::Insn;
use crate::support::hash::IndexMap;

pub(super) enum State {
    Test { feature: usize, edges: &'static [(u16, usize)], default: Option<usize> },
    Leaf(&'static [usize]),
}

/// The most operands a pattern tests.
pub(super) const OPERANDS: usize = 3;

type Holds = for<'a, 'b, 'c, 'd> fn(&mut Selector<'a, 'b, 'c>, usize, &Match<'d>) -> bool;
type Costs = for<'a, 'b, 'c, 'd> fn(&mut Selector<'a, 'b, 'c>, usize, &Match<'d>) -> Result<i64, Unselected>;
type Emits = for<'a, 'b, 'c, 'd, 'e> fn(
    &mut Selector<'a, 'b, 'c>,
    usize,
    &Match<'d>,
    &'e mut Vec<Arc<Insn>>,
) -> Result<(), Unselected>;

/// One target's instruction selector, as generated from its definition
/// directory: the automaton's tables and the patterns' methods. A target is
/// bound to its selector by `llrm-driver`; nothing here names one.
pub struct Compiled {
    pub name: &'static str,
    opcodes: &'static [&'static str],
    types: &'static [&'static str],
    kinds: &'static [&'static str],
    commutative: &'static [&'static str],
    root: Option<usize>,
    states: &'static [State],
    groups: &'static [Option<usize>],
    covers: &'static [bool],
    holds: Holds,
    covers_here: Holds,
    cost: Costs,
    emit: Emits,
    /// The target's peephole rules, generated from the same directory.
    rules: &'static crate::backend::peep::Rules,
}

impl Compiled {
    /// The peephole rules generated beside this selector.
    pub fn rules(&self) -> &'static crate::backend::peep::Rules {
        self.rules
    }
}

mod selectors {
    use super::*;

    include!(concat!(env!("OUT_DIR"), "/selectors.rs"));
}

/// The selector of the target `name`, as its definition directory is named.
pub fn selector(name: &str) -> Option<&'static Compiled> {
    selectors::ALL.iter().copied().find(|one| one.name == name)
}

/// The selector built for 16-bit x86, which the tests of this crate use.
#[cfg(test)]
pub(crate) fn m16() -> &'static Compiled {
    &selectors::x86_m16::SELECTOR
}

/// The instruction a pattern matched: its operands, a commutative
/// opcode's constant taken second.
pub(super) struct Match<'a> {
    pub inst: InstId,
    pub at: i64,
    pub ops: Vec<Operand>,
    pub volatile: bool,
    pub block_at: &'a IndexMap<BlockId, i64>,
    pub convention: &'a Convention,
}

/// The first candidate that holds; if it is in a group, the cheapest of its
/// group that holds, the earlier on a tie.
fn choose<E>(
    candidates: &[usize],
    groups: &[Option<usize>],
    mut holds: impl FnMut(usize) -> bool,
    mut cost: impl FnMut(usize) -> Result<i64, E>,
) -> Result<Option<usize>, E> {
    let Some(position) = candidates.iter().position(|&one| holds(one)) else { return Ok(None) };
    let first = candidates[position];
    let Some(group) = groups[first] else { return Ok(Some(first)) };
    let rivals: Vec<usize> =
        candidates[position + 1..].iter().copied().filter(|&one| groups[one] == Some(group) && holds(one)).collect();
    if rivals.is_empty() {
        return Ok(Some(first));
    }
    let mut best = (cost(first)?, first);
    for one in rivals {
        let price = cost(one)?;
        if price < best.0 {
            best = (price, one);
        }
    }
    Ok(Some(best.1))
}

impl Selector<'_, '_, '_> {
    pub(super) fn selected_by_pattern(
        &mut self,
        inst: InstId,
        block_at: &IndexMap<BlockId, i64>,
        out: &mut Vec<Arc<Insn>>,
        convention: &Convention,
    ) -> Result<(), Unselected> {
        let m = self.matched(inst, block_at, convention);
        let candidates = self.candidates(&m);
        let Some(chosen) = self.chosen(candidates, &m)? else {
            return refuse(self.function.instruction(inst).opcode.mnemonic());
        };
        (self.compiled.emit)(self, chosen, &m, out)
    }

    fn matched<'a>(
        &self,
        inst: InstId,
        block_at: &'a IndexMap<BlockId, i64>,
        convention: &'a Convention,
    ) -> Match<'a> {
        let instruction = self.function.instruction(inst);
        let mut ops = instruction.operands.clone();
        if self.compiled.commutative.contains(&instruction.opcode.mnemonic())
            && matches!(ops.first(), Some(Operand::Constant(_)))
        {
            ops.swap(0, 1);
        }
        let volatile =
            matches!(
                instruction.opcode,
                Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. }
            );
        Match { inst, at: self.ats[&inst], ops, volatile, block_at, convention }
    }

    /// The patterns the automaton leaves standing for `m`, in file order.
    fn candidates(
        &self,
        m: &Match,
    ) -> &'static [usize] {
        let features = self.features(m);
        let mut state = self.compiled.root;
        loop {
            match state.map(|one| &self.compiled.states[one]) {
                None => return &[],
                Some(State::Leaf(patterns)) => return patterns,
                Some(State::Test { feature, edges, default }) => {
                    state = edges
                        .iter()
                        .find(|(value, _)| *value == features[*feature])
                        .map_or(*default, |&(_, next)| Some(next));
                }
            }
        }
    }

    /// The cover phase at `inst`: the first covering pattern that holds
    /// marks what it covers, which is then selected by it alone.
    pub(super) fn covered_by_pattern(
        &mut self,
        inst: InstId,
        block_at: &IndexMap<BlockId, i64>,
        convention: &Convention,
    ) {
        let m = self.matched(inst, block_at, convention);
        for &one in self.candidates(&m) {
            if self.compiled.covers[one] && (self.compiled.covers_here)(self, one, &m) {
                return;
            }
        }
    }

    fn chosen(
        &mut self,
        candidates: &[usize],
        m: &Match,
    ) -> Result<Option<usize>, Unselected> {
        let this = std::cell::RefCell::new(self);
        let compiled = this.borrow().compiled;
        choose(
            candidates,
            compiled.groups,
            |one| (compiled.holds)(&mut this.borrow_mut(), one, m),
            |one| (compiled.cost)(&mut this.borrow_mut(), one, m),
        )
    }

    fn features(
        &self,
        m: &Match,
    ) -> [u16; 2 + 2 * OPERANDS] {
        let instruction = self.function.instruction(m.inst);
        let position = |domain: &[&str], name: &str| {
            domain.iter().position(|one| *one == name).expect("a name the generator knows") as u16
        };
        let mut out = [0; 2 + 2 * OPERANDS];
        out[0] = position(self.compiled.opcodes, instruction.opcode.mnemonic());
        out[1] = position(self.compiled.types, self.class(Some(instruction.ty)));
        for (index, &operand) in m.ops.iter().take(OPERANDS).enumerate() {
            out[2 + 2 * index] = position(self.compiled.kinds, self.kind(operand));
            out[3 + 2 * index] =
                position(self.compiled.types, self.class(self.function.operand_type(&self.module.context, operand)));
        }
        out
    }

    fn class(
        &self,
        ty: Option<TypeId>,
    ) -> &'static str {
        TypeClass::of(self.types(), &self.layout, ty).name()
    }

    fn kind(
        &self,
        operand: Operand,
    ) -> &'static str {
        match operand {
            Operand::Value(_) => "value",
            Operand::Block(_) => "block",
            Operand::Constant(id) => match self.module.context.get(id).kind {
                ConstantKind::Int(_) | ConstantKind::Null | ConstantKind::Zero => "int",
                ConstantKind::Poison => "poison",
                ConstantKind::Float(_) => "fconst",
                _ => "const",
            },
        }
    }

    fn type_of(
        &self,
        operand: Operand,
    ) -> TypeId {
        self.function.operand_type(&self.module.context, operand).expect("a typed operand")
    }

    /// An integer constant's value at its own width.
    fn literal(
        &self,
        operand: Operand,
    ) -> Option<i64> {
        let width = self.width(self.type_of(operand)).ok()?;
        self.constant(operand, width)
    }

    /// Whether `operand` is of one of `kinds` and `types`.
    fn operand_is(
        &self,
        operand: Operand,
        kinds: Option<&[&str]>,
        types: Option<&[&str]>,
    ) -> bool {
        kinds.is_none_or(|kinds| kinds.contains(&self.kind(operand)))
            && types.is_none_or(|types| {
                types.contains(&self.class(self.function.operand_type(&self.module.context, operand)))
            })
    }

    /// The instruction defining `operand`.
    fn definition(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = operand else { return None };
        match self.function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        }
    }

    /// Whether an instruction of one of `opcodes`, of one of `types` and
    /// with at least `operands` operands, defines `operand`.
    fn defines(
        &self,
        operand: Operand,
        opcodes: &[&str],
        types: Option<&[&str]>,
        operands: usize,
    ) -> bool {
        self.definition(operand).is_some_and(|inst| {
            let instruction = self.function.instruction(inst);
            opcodes.contains(&instruction.opcode.mnemonic())
                && types.is_none_or(|types| types.contains(&self.class(Some(instruction.ty))))
                && instruction.operands.len() >= operands
        })
    }

    /// Operand `index` of the instruction defining `operand`.
    fn inner(
        &self,
        operand: Operand,
        index: usize,
    ) -> Operand {
        self.function.instruction(self.definition(operand).expect("a nested instruction")).operands[index]
    }

    fn cover(
        &mut self,
        operand: Operand,
        root: InstId,
    ) {
        self.covered.insert(self.definition(operand).expect("a nested instruction"), root);
    }

    fn covered_by(
        &self,
        operand: Operand,
        root: InstId,
    ) -> bool {
        self.definition(operand).is_some_and(|inst| self.covered.get(&inst) == Some(&root))
    }

    fn emitted(
        &self,
        m: &Match,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        volatile: bool,
    ) -> Arc<Insn> {
        let what = semantics(op, name, dests, sources);
        if volatile { Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }) } else { insn(m.at, what) }
    }

    fn opcode(
        &self,
        m: &Match,
    ) -> &Opcode {
        &self.function.instruction(m.inst).opcode
    }

    fn cast(
        &self,
        m: &Match,
    ) -> CastOp {
        let Opcode::Cast(op) = *self.opcode(m) else { unreachable!("a cast pattern") };
        op
    }

    /// The type a load reads or a store writes.
    fn accessed(
        &self,
        m: &Match,
    ) -> TypeId {
        let instruction = self.function.instruction(m.inst);
        if matches!(instruction.opcode, Opcode::Store { .. }) {
            self.type_of(instruction.operands[0])
        } else {
            instruction.ty
        }
    }

    // Operand constructors: a LIR operand, and what making it took.

    fn op_held(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        Ok(Loc::Held(self.held(operand, self.type_of(operand), m.at, out)?))
    }

    fn op_source(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        self.source(operand, self.type_of(operand), m.at, out)
    }

    /// Held, as its byte.
    fn op_byte(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        let held = self.held(operand, self.type_of(operand), m.at, out)?;
        Ok(Loc::Held(Held { width: 1, ..held }))
    }

    /// A shift's count: cl counts, so a register count is its byte.
    fn op_count(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        Ok(match self.source(operand, self.type_of(operand), m.at, out)? {
            Loc::Held(count) => Loc::Held(Held { width: 1, ..count }),
            other => other,
        })
    }

    /// Held, at the result's width; a joined dword's low word is the word
    /// it was joined from.
    fn op_narrowed(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        let to = self.width(self.function.instruction(m.inst).ty)?;
        let held = self.held(operand, self.type_of(operand), m.at, out)?;
        let held = match self.joins.get(&held.value) {
            Some(&(low, _)) if to <= 2 => low,
            _ => held,
        };
        Ok(Loc::Held(Held { width: to, ..held }))
    }

    fn op_result(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
    ) -> Result<Loc, Unselected> {
        let instruction = self.function.instruction(m.inst);
        let width = self.width(instruction.ty)?;
        Ok(Loc::Held(Held { value: self.value(instruction.result.expect("a result")), width }))
    }

    /// A fresh register of the result's width.
    fn op_fresh(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
    ) -> Result<Loc, Unselected> {
        let width = self.width(self.function.instruction(m.inst).ty)?;
        Ok(Loc::Held(self.fresh_held(width)))
    }

    fn op_float(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        Ok(Loc::Held(self.float(operand, m.at, out)?))
    }

    fn op_fresult(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
    ) -> Result<Loc, Unselected> {
        let result = self.function.instruction(m.inst).result.expect("a result");
        Ok(Loc::Held(Held { value: self.value(result), width: FLOAT }))
    }

    /// The cell a float load or store reaches: its bytes in memory.
    fn op_cell(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
        pointer: Operand,
    ) -> Result<Loc, Unselected> {
        let pointer = self.pointer(pointer)?;
        Ok(Loc::Mem(Self::memory(pointer, self.size(self.accessed(m))?)))
    }

    /// The cell the load defining `value` reads.
    fn op_loaded(
        &mut self,
        _: &Match,
        _: &mut Vec<Arc<Insn>>,
        value: Operand,
    ) -> Result<Loc, Unselected> {
        let loaded = self.function.instruction(self.definition(value).expect("a load"));
        let pointer = self.pointer(loaded.operands[0])?;
        Ok(Loc::Mem(Self::memory(pointer, self.size(loaded.ty)?)))
    }

    /// The cell an integer load or store reaches: its register's bytes.
    fn op_access(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
        pointer: Operand,
    ) -> Result<Loc, Unselected> {
        let width = self.width(self.accessed(m))?;
        Ok(Loc::Mem(Self::memory(self.pointer(pointer)?, width)))
    }

    fn op_imm(
        &mut self,
        _: &Match,
        _: &mut Vec<Arc<Insn>>,
        value: i64,
        width: i64,
    ) -> Result<Loc, Unselected> {
        Ok(Loc::Imm(Imm { value, width: width as u32, address: None }))
    }

    // Predicates.

    fn is_selected_elsewhere(
        &self,
        m: &Match,
    ) -> bool {
        self.covered.contains_key(&m.inst) || self.consumed.contains(&m.inst)
    }

    /// Whether `value` is read once, by the root, in its block.
    fn is_only_reader(
        &self,
        m: &Match,
        value: Operand,
    ) -> bool {
        matches!(
            value,
            Operand::Value(value) if self.only_reader(value, m.inst)
        )
    }

    fn is_nonvolatile(
        &self,
        _: &Match,
        load: Operand,
    ) -> bool {
        self.definition(load)
            .is_some_and(|inst| matches!(
                self.function.instruction(inst).opcode,
                Opcode::Load { volatile: false, .. }
            ))
    }

    /// Whether nothing may write memory between the load defining `value`
    /// and where the root is made: beside the branch, for a fused compare.
    fn is_unwritten(
        &self,
        m: &Match,
        value: Operand,
    ) -> bool {
        let Some(load) = self.definition(value) else { return false };
        let Some(block) = self.function.parent(m.inst) else { return false };
        let at =
            if self.fused.contains(&m.inst) { self.function.terminator(block).expect("a terminator") } else { m.inst };
        self.unwritten(load, at)
    }

    /// Whether `value` is what the float comparison compares second, as
    /// its row in FLOAT_CONDITIONS orders the operands, and a cell fcom can
    /// read: a float's 4 bytes or a double's 8, not an extended's 10.
    fn is_compared_second(
        &self,
        m: &Match,
        value: Operand,
    ) -> bool {
        let Opcode::FCmp(predicate) = *self.opcode(m) else { return false };
        float_conditions(predicate).is_some_and(|(swapped, _)| m.ops[usize::from(!swapped)] == value)
            && matches!(self.size(self.type_of(value)), Ok(4 | 8))
    }

    fn is_lrint(
        &self,
        _: &Match,
        call: Operand,
    ) -> bool {
        self.definition(call).is_some_and(|inst| self.lrint(inst))
    }

    fn is_narrowed(
        &self,
        m: &Match,
    ) -> bool {
        self.words.contains_key(&m.inst)
    }

    fn is_volatile(
        &self,
        m: &Match,
    ) -> bool {
        m.volatile
    }

    fn is_fused(
        &self,
        m: &Match,
    ) -> bool {
        self.fused.contains(&m.inst)
    }

    /// Whether a multiply by `factor` has a chain of shifts and adds whose
    /// shifts all fit the width.
    fn is_scalable(
        &self,
        m: &Match,
        factor: Operand,
    ) -> bool {
        self.chain(m, factor).is_some_and(|(chain, _)| {
            let width = self.width(self.function.instruction(m.inst).ty).expect("a held width");
            chain.iter().all(|&(name, count)| !["shl", "fadd", "fsub"].contains(&name) || count < i64::from(width) * 8)
        })
    }

    fn is_same_width(
        &self,
        m: &Match,
        operand: Operand,
    ) -> bool {
        let to = self.width(self.function.instruction(m.inst).ty);
        to.is_ok() && to == self.width(self.type_of(operand))
    }

    fn chain(
        &self,
        m: &Match,
        factor: Operand,
    ) -> Option<(Vec<(&'static str, i64)>, i64)> {
        let width = self.width(self.function.instruction(m.inst).ty).ok()?;
        let n = self.constant(factor, width).filter(|&n| matches!(width, 2 | 4) && (1 < n || n < -1))?;
        if width == 4 {
            arithmetic::cheapest_chain(n, self.cpu)
        } else {
            arithmetic::cheapest_narrow_chain(n, self.cpu)
        }
        .ok()
        .flatten()
    }

    // Costs.

    /// Bytes of `chain`'s shifts, adds and subtracts. The copy that seeds it is not counted:
    /// the allocator drops it where the source dies, as `add si, si` shows.
    fn chain_bytes(
        chain: &[(&str, i64)],
        width: i64,
        operand: i64,
    ) -> i64 {
        use llrm_x86::encoding::{register_bytes, shift_bytes};
        // A `lea r,[a+cur*s]` is the opcode, the ModRM and the SIB.
        chain
            .iter()
            .map(|&(name, count)| match name {
                "shl" => shift_bytes(count, width, operand),
                "lea" | "flea" => 3 + i64::from(width != operand),
                "fadd" | "fsub" => {
                    shift_bytes(count, width, operand) + register_bytes(width, operand) + register_bytes(width, operand)
                }
                _ => register_bytes(width, operand),
            })
            .sum()
    }

    /// Bytes first, clocks to break a tie: both are under 100.
    fn by_size(
        bytes: i64,
        clocks: i64,
    ) -> i64 {
        bytes * 100 + clocks
    }

    fn cost_immediate_multiply(
        &self,
        m: &Match,
        factor: Operand,
    ) -> Result<i64, Unselected> {
        let width = self.width(self.function.instruction(m.inst).ty)?;
        let n = self.constant(factor, width).expect("an integer factor");
        if self.cpu.size {
            return Ok(Self::by_size(
                llrm_x86::encoding::imul_immediate_bytes(n, i64::from(width), self.cpu.operand_bytes),
                arithmetic::immediate_multiply(self.cpu, n).map_err(Unselected)?,
            ));
        }
        arithmetic::immediate_multiply(self.cpu, n).map_err(Unselected)
    }

    fn cost_scaled(
        &self,
        m: &Match,
        factor: Operand,
    ) -> Result<i64, Unselected> {
        let (chain, clocks) = self.chain(m, factor).expect("a scalable factor");
        // Tuned for size, the two compete in bytes, as they compete in clocks otherwise.
        let width = self.width(self.function.instruction(m.inst).ty)?;
        Ok(if self.cpu.size {
            Self::by_size(Self::chain_bytes(&chain, i64::from(width), self.cpu.operand_bytes), clocks)
        } else {
            clocks
        })
    }

    // Hooks: what is selected by hand.

    /// Shifts and adds of the source, as the old route's _scaled selects.
    fn hook_scaled(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        source: Operand,
        factor: Operand,
    ) -> Result<(), Unselected> {
        let a = self.op_held(m, out, source)?;
        let (chain, _) = self.chain(m, factor).expect("a scalable factor");
        let Loc::Held(result) = self.op_result(m, out)? else { unreachable!("a register") };
        let mut current = a.clone();
        for (index, &(name, count)) in chain.iter().enumerate() {
            let into = if index == chain.len() - 1 { result } else { self.fresh_held(result.width) };
            if name == "flea" {
                // `into = current + current*count`.
                let Loc::Held(base) = current.clone() else { unreachable!("a chain works on registers") };
                let cell = crate::model::ir::Mem {
                    base: Some(base),
                    index: Some(base),
                    scale: count,
                    ..crate::model::ir::Mem::new(None, result.width)
                };
                out.push(insn(m.at, semantics(Operation::Address, "lea", vec![Loc::Held(into)], vec![Loc::Mem(cell)])));
            } else if name == "lea" {
                // `into = a + current*count`: the shift and add of one digit, made by the address unit.
                let (Loc::Held(base), Loc::Held(scaled)) = (a.clone(), current.clone()) else {
                    unreachable!("a chain works on registers")
                };
                let cell = crate::model::ir::Mem {
                    base: Some(base),
                    index: Some(scaled),
                    scale: count,
                    ..crate::model::ir::Mem::new(None, result.width)
                };
                out.push(insn(m.at, semantics(Operation::Address, "lea", vec![Loc::Held(into)], vec![Loc::Mem(cell)])));
            } else if name == "neg" {
                out.push(insn(m.at, semantics(Operation::Unary, "neg", vec![Loc::Held(into)], vec![current])));
            } else if name == "rsub" {
                // `into = source - current`.
                out.push(insn(
                    m.at,
                    semantics(Operation::Binary, "sub", vec![Loc::Held(into)], vec![a.clone(), current]),
                ));
            } else if name == "fadd" || name == "fsub" {
                // `current*(2^count +- 1)`: the shifted copy, then the sum or difference with `current`.
                let Loc::Held(width_of) = current.clone() else { unreachable!("a chain works on registers") };
                let shifted = self.fresh_held(width_of.width);
                out.push(insn(
                    m.at,
                    semantics(
                        Operation::Binary,
                        "shl",
                        vec![Loc::Held(shifted)],
                        vec![current.clone(), Loc::Imm(Imm { value: count, width: 1, address: None })],
                    ),
                ));
                let (add, left, right) = if name == "fadd" {
                    ("add", Loc::Held(shifted), current)
                } else {
                    ("sub", Loc::Held(shifted), current)
                };
                out.push(insn(m.at, semantics(Operation::Binary, add, vec![Loc::Held(into)], vec![left, right])));
            } else {
                let other =
                    if name == "shl" { Loc::Imm(Imm { value: count, width: 1, address: None }) } else { a.clone() };
                out.push(insn(m.at, semantics(Operation::Binary, name, vec![Loc::Held(into)], vec![current, other])));
            }
            current = Loc::Held(into);
        }
        Ok(())
    }

    /// A byte product, by the operand size's own multiply of both factors extended: only the low byte is kept, which no
    /// extension changes.
    fn hook_byte_multiply(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        left: Operand,
        right: Operand,
    ) -> Result<(), Unselected> {
        let wide = self.cpu.operand_bytes as u32;
        let extend = |this: &mut Self, operand: Operand, out: &mut Vec<Arc<Insn>>| -> Result<Loc, Unselected> {
            if let Some(value) = this.constant(operand, 1) {
                return Ok(Loc::Imm(Imm { value, width: wide, address: None }));
            }
            let byte = this.op_held(m, out, operand)?;
            let into = this.fresh_held(wide);
            out.push(insn(m.at, semantics(Operation::Extend, "movzx", vec![Loc::Held(into)], vec![byte])));
            Ok(Loc::Held(into))
        };
        let (a, b) = (extend(self, left, out)?, extend(self, right, out)?);
        let product = self.fresh_held(wide);
        out.push(insn(m.at, semantics(Operation::Multiply, "imul", vec![Loc::Held(product)], vec![a, b])));
        let Loc::Held(result) = self.op_result(m, out)? else { unreachable!("a register") };
        out.push(insn(
            m.at,
            semantics(Operation::Move, "mov", vec![Loc::Held(result)], vec![Loc::Held(Held { width: 1, ..product })]),
        ));
        Ok(())
    }

    fn hook_divide(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let Opcode::Binary(op) = *self.opcode(m) else { unreachable!("a division") };
        self.divide(op, m.inst, out)
    }

    fn hook_wide_cast(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.wide_cast(self.cast(m), m.inst, m.at, out)
    }

    fn hook_wide_binary(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let Opcode::Binary(op) = *self.opcode(m) else { unreachable!("a binary operation") };
        self.wide_binary(op, m.inst, m.at, out)
    }

    fn hook_float_cast(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.float_cast(self.cast(m), m.inst, m.at, out)
    }

    fn hook_far_cast(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.far_cast(self.cast(m), m.inst, m.at, out)
    }

    /// A cast no pattern selects, refused once its types are.
    fn hook_unselected_cast(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let instruction = self.function.instruction(m.inst);
        self.width(instruction.ty)?;
        self.width(self.type_of(instruction.operands[0]))?;
        refuse(instruction.opcode.mnemonic())
    }

    /// A dword load read only as words: each word loaded once.
    fn hook_words(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let pointer = self.pointer(pointer)?;
        let mut made: IndexMap<i64, Held> = IndexMap::default();
        let mut halves = self.words[&m.inst].clone();
        halves.sort_by_key(|&(offset, _)| offset);
        for (offset, result) in halves {
            let held = Held { value: self.value(result), width: 2 };
            let source = match made.get(&offset) {
                Some(&earlier) => Loc::Held(earlier),
                None => Loc::Mem(Self::memory(pointer.moved(offset), 2)),
            };
            made.entry(offset).or_insert(held);
            out.push(insn(m.at, semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![source])));
        }
        Ok(())
    }

    fn hook_getelementptr(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let instruction = self.function.instruction(m.inst);
        let Opcode::GetElementPtr { source } = instruction.opcode else { unreachable!("a getelementptr") };
        let result = instruction.result.expect("an address");
        match self.folded(result)? {
            None => self.indexed(m.inst, source, m.at, out)?,
            // A far address read in another block, or by a phi, is made once
            // here: refolded there, it would hold its base past its own step.
            Some(pointer @ Pointer::Far { selector, .. }) if self.read_elsewhere(result) => {
                let moved = self.fresh_held(2);
                out.push(insn(m.at, self.address(pointer, moved)));
                self.fars.insert(result, (Some(moved), selector));
            }
            Some(_) => {}
        }
        Ok(())
    }

    /// A float constant is its bits, stored as integers are.
    fn hook_float_bits(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        constant: Operand,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let (pointer, size) = (self.pointer(pointer)?, self.size(self.type_of(constant))?);
        let Operand::Constant(id) = constant else { unreachable!("a constant") };
        let ConstantKind::Float(bits) = self.module.context.get(id).kind else {
            return refuse("a float constant of no bits");
        };
        // An extended float is its 10 bytes: a dword, a dword and a word, as a global's initializer lays them down.
        if size == 10 {
            let image = llrm_mir::types::x87_extended(bits);
            for (by, width) in [(0_usize, 4_u32), (4, 4), (8, 2)] {
                let value =
                    image[by..by + width as usize].iter().rev().fold(0_i64, |acc, byte| acc << 8 | i64::from(*byte));
                let what = semantics(
                    Operation::Move,
                    "mov",
                    vec![Loc::Mem(Self::memory(pointer.moved(by as i64), width))],
                    vec![Loc::Imm(Imm { value, width, address: None })],
                );
                out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
            }
            return Ok(());
        }
        let bits = if size == 4 { u128::from(bits as u32) } else { u128::from(bits) };
        let low = semantics(
            Operation::Move,
            "mov",
            vec![Loc::Mem(Self::memory(pointer, 4))],
            vec![Loc::Imm(Imm { value: bits as u32 as i64, width: 4, address: None })],
        );
        let what = if size == 8 {
            out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, low) }));
            let high = Loc::Imm(Imm { value: (bits >> 32) as u32 as i64, width: 4, address: None });
            semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(pointer.moved(4), 4))], vec![high])
        } else {
            low
        };
        out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
        Ok(())
    }

    /// An i64 read as its two dwords, the low at the address: x86 is little-endian.
    fn hook_wide_load(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let pointer = self.pointer(pointer)?;
        let (low, high) = (self.fresh_held(4), self.fresh_held(4));
        for (held, by) in [(low, 0), (high, 4)] {
            let what = semantics(
                Operation::Move,
                "mov",
                vec![Loc::Held(held)],
                vec![Loc::Mem(Self::memory(pointer.moved(by), 4))],
            );
            out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
        }
        self.wides.insert(self.function.instruction(m.inst).result.expect("a load's value"), (low, high));
        Ok(())
    }

    /// An i64 written as its two dwords.
    fn hook_wide_store(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        value: Operand,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let (low, high) = self.wide(value, m.at, out)?;
        let pointer = self.pointer(pointer)?;
        for (held, by) in [(low, 0), (high, 4)] {
            let what = semantics(
                Operation::Move,
                "mov",
                vec![Loc::Mem(Self::memory(pointer.moved(by), 4))],
                vec![Loc::Held(held)],
            );
            out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
        }
        Ok(())
    }

    fn hook_far_load(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let pointer = self.pointer(pointer)?;
        let (offset, selector) = self.far_loaded(pointer, m.volatile, m.at, out);
        self.fars.insert(self.function.instruction(m.inst).result.expect("a load's value"), (Some(offset), selector));
        Ok(())
    }

    fn hook_far_store(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        value: Operand,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let words = match self.far_words(value)? {
            Some(words) => words,
            None => self.far(value, m.at, out).map(|(offset, selector)| [Loc::Held(offset), Loc::Held(selector)])?,
        };
        let pointer = self.pointer(pointer)?;
        for (word, by) in words.into_iter().zip([0, 2]) {
            let what =
                semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(pointer.moved(by), 2))], vec![word]);
            out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
        }
        Ok(())
    }

    /// A field of an answer in registers: the register it came in.
    fn hook_extractvalue(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let instruction = self.function.instruction(m.inst);
        let Opcode::ExtractValue(indices) = &instruction.opcode else { unreachable!("an extractvalue") };
        let field = match (instruction.operands[0], &indices[..]) {
            (Operand::Value(aggregate), &[index]) => {
                self.fields.get(&aggregate).and_then(|fields| fields.get(index as usize)).copied()
            }
            _ => None,
        };
        let Some(field) = field else { return refuse("an extractvalue of no answer in registers") };
        let result = Held { value: self.value(instruction.result.expect("a field")), width: field.width };
        out.push(insn(m.at, semantics(Operation::Move, "mov", vec![Loc::Held(result)], vec![Loc::Held(field)])));
        Ok(())
    }

    /// SETcc, as LLVM selects a comparison it keeps as a value.
    fn hook_setcc(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let test = self.compare(m.inst, m.at, out)?;
        let result = Held { value: self.value(self.function.instruction(m.inst).result.expect("a result")), width: 1 };
        let at = m.at;
        let set = |code: &str, into: Held| {
            insn(at, semantics(Operation::Unary, &format!("set{}", &code[1..]), vec![Loc::Held(into)], vec![]))
        };
        match test {
            Test::One(code) => out.push(set(code, result)),
            Test::Both(a, b) | Test::Either(a, b) => {
                let (first, second) = (self.fresh_held(1), self.fresh_held(1));
                out.extend([set(a, first), set(b, second)]);
                let join = if matches!(test, Test::Both(..)) { "and" } else { "or" };
                out.push(insn(
                    at,
                    semantics(
                        Operation::Binary,
                        join,
                        vec![Loc::Held(result)],
                        vec![Loc::Held(first), Loc::Held(second)],
                    ),
                ));
            }
        }
        Ok(())
    }

    fn hook_br(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.branch(m.inst, m.block_at, m.at, out)
    }

    fn hook_ret(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.ret(m.inst, m.convention, m.at, out)
    }

    fn hook_call(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let Opcode::Call(info) = self.opcode(m) else { unreachable!("a call") };
        super::in_the_frame(&info.argument_attrs)?;
        self.call(m.inst, info.calling_convention, m.at, out)
    }

    fn hook_invoke(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let Opcode::Invoke(info) = self.opcode(m) else { unreachable!("an invoke") };
        super::in_the_frame(&info.argument_attrs)?;
        self.call(m.inst, info.calling_convention, m.at, out)?;
        let block = self.function.parent(m.inst).expect("a placed invoke");
        out.push(insn(m.at, super::jump(m.block_at[&self.successors(block)[0]])));
        Ok(())
    }

    fn hook_landing_pad(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.landing_pad(m.inst, m.at, out)
    }
}

#[cfg(test)]
mod tests {
    use super::choose;

    /// Of two patterns of one group that both hold, the cheaper is chosen,
    /// the earlier on a tie; a pattern of no group is chosen as it holds.
    #[test]
    fn the_cheaper_of_two_matching_patterns_is_chosen() {
        let groups = [Some(0), Some(0), None];
        let priced = |costs: [i64; 3]| choose::<()>(&[0, 1, 2], &groups, |_| true, |one| Ok(costs[one])).unwrap();
        assert_eq!(priced([5, 3, 1]), Some(1));
        assert_eq!(priced([3, 5, 1]), Some(0));
        assert_eq!(priced([4, 4, 1]), Some(0));
        assert_eq!(choose::<()>(&[2, 0, 1], &groups, |_| true, |_| Ok(0)).unwrap(), Some(2));
        assert_eq!(choose::<()>(&[0, 1], &groups, |one| one == 1, |_| unreachable!("one holds")).unwrap(), Some(1));
    }
}
