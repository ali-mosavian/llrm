//! The generated selector's runtime: an instruction's features, the walk
//! of the automaton `generator` built from `patterns.isel`, the choice
//! among the patterns that match, and what the patterns call by name --
//! operand constructors (`op_`), predicates (`is_`), costs (`cost_`) and
//! the hooks (`hook_`) for what is not pattern-shaped.

use std::sync::Arc;

use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::{CastOp, ConstantKind, Opcode, Type, TypeId};

use super::{float_conditions, insn, insn_of, refuse, semantics, Convention, Selector, Test, Unselected, FLOAT};
use crate::backend::arithmetic;
use crate::model::ir::{Held, Imm, Loc, Operation};
use crate::model::lir::Insn;
use crate::support::hash::IndexMap;

pub(super) enum State {
    Test { feature: usize, edges: &'static [(u16, usize)], default: Option<usize> },
    Leaf(&'static [usize]),
}

include!(concat!(env!("OUT_DIR"), "/isel.rs"));

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
fn choose<E>(candidates: &[usize], groups: &[Option<usize>], mut holds: impl FnMut(usize) -> bool, mut cost: impl FnMut(usize) -> Result<i64, E>) -> Result<Option<usize>, E> {
    let Some(position) = candidates.iter().position(|&one| holds(one)) else { return Ok(None) };
    let first = candidates[position];
    let Some(group) = groups[first] else { return Ok(Some(first)) };
    let rivals: Vec<usize> = candidates[position + 1..].iter().copied().filter(|&one| groups[one] == Some(group) && holds(one)).collect();
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
    pub(super) fn selected_by_pattern(&mut self, inst: InstId, block_at: &IndexMap<BlockId, i64>, out: &mut Vec<Arc<Insn>>, convention: &Convention) -> Result<(), Unselected> {
        let m = self.matched(inst, block_at, convention);
        let candidates = self.candidates(&m);
        let Some(chosen) = self.chosen(candidates, &m)? else { return refuse(self.function.instruction(inst).opcode.mnemonic()) };
        self.pattern_emit(chosen, &m, out)
    }

    fn matched<'a>(&self, inst: InstId, block_at: &'a IndexMap<BlockId, i64>, convention: &'a Convention) -> Match<'a> {
        let instruction = self.function.instruction(inst);
        let mut ops = instruction.operands.clone();
        if COMMUTATIVE.contains(&instruction.opcode.mnemonic()) && matches!(ops.first(), Some(Operand::Constant(_))) {
            ops.swap(0, 1);
        }
        let volatile = matches!(instruction.opcode, Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. });
        Match { inst, at: self.ats[&inst], ops, volatile, block_at, convention }
    }

    /// The patterns the automaton leaves standing for `m`, in file order.
    fn candidates(&self, m: &Match) -> &'static [usize] {
        let features = self.features(m);
        let mut state = ROOT;
        loop {
            match state.map(|one| &STATES[one]) {
                None => return &[],
                Some(State::Leaf(patterns)) => return patterns,
                Some(State::Test { feature, edges, default }) => {
                    state = edges.iter().find(|(value, _)| *value == features[*feature]).map_or(*default, |&(_, next)| Some(next));
                }
            }
        }
    }

    /// The cover phase at `inst`: the first covering pattern that holds
    /// marks what it covers, which is then selected by it alone.
    pub(super) fn covered_by_pattern(&mut self, inst: InstId, block_at: &IndexMap<BlockId, i64>, convention: &Convention) {
        let m = self.matched(inst, block_at, convention);
        for &one in self.candidates(&m) {
            if COVERS[one] && self.pattern_covers(one, &m) {
                return;
            }
        }
    }

    fn chosen(&mut self, candidates: &[usize], m: &Match) -> Result<Option<usize>, Unselected> {
        let this = std::cell::RefCell::new(self);
        choose(candidates, &GROUPS, |one| this.borrow_mut().pattern_holds(one, m), |one| this.borrow_mut().pattern_cost(one, m))
    }

    fn features(&self, m: &Match) -> [u16; 2 + 2 * OPERANDS] {
        let instruction = self.function.instruction(m.inst);
        let position = |domain: &[&str], name: &str| domain.iter().position(|one| *one == name).expect("a name the generator knows") as u16;
        let mut out = [0; 2 + 2 * OPERANDS];
        out[0] = position(&OPCODES, instruction.opcode.mnemonic());
        out[1] = position(&TYPES, self.class(Some(instruction.ty)));
        for (index, &operand) in m.ops.iter().take(OPERANDS).enumerate() {
            out[2 + 2 * index] = position(&KINDS, self.kind(operand));
            out[3 + 2 * index] = position(&TYPES, self.class(self.function.operand_type(&self.module.context, operand)));
        }
        out
    }

    fn class(&self, ty: Option<TypeId>) -> &'static str {
        let Some(ty) = ty else { return "none" };
        match self.types().get(ty) {
            Type::Void => "void",
            Type::Int(1) => "i1",
            Type::Int(8) => "i8",
            Type::Int(16) => "i16",
            Type::Int(32) => "i32",
            Type::Int(64) => "i64",
            Type::Pointer(1) => "far",
            Type::Pointer(0 | 2) => "ptr",
            Type::Float(_) => "float",
            _ => "other",
        }
    }

    fn kind(&self, operand: Operand) -> &'static str {
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

    fn type_of(&self, operand: Operand) -> TypeId {
        self.function.operand_type(&self.module.context, operand).expect("a typed operand")
    }

    /// An integer constant's value at its own width.
    fn literal(&self, operand: Operand) -> Option<i64> {
        let width = self.width(self.type_of(operand)).ok()?;
        self.constant(operand, width)
    }

    /// Whether `operand` is of one of `kinds` and `types`.
    fn operand_is(&self, operand: Operand, kinds: Option<&[&str]>, types: Option<&[&str]>) -> bool {
        kinds.is_none_or(|kinds| kinds.contains(&self.kind(operand))) && types.is_none_or(|types| types.contains(&self.class(self.function.operand_type(&self.module.context, operand))))
    }

    /// The instruction defining `operand`.
    fn definition(&self, operand: Operand) -> Option<InstId> {
        let Operand::Value(value) = operand else { return None };
        match self.function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        }
    }

    /// Whether an instruction of one of `opcodes`, of one of `types` and
    /// with at least `operands` operands, defines `operand`.
    fn defines(&self, operand: Operand, opcodes: &[&str], types: Option<&[&str]>, operands: usize) -> bool {
        self.definition(operand).is_some_and(|inst| {
            let instruction = self.function.instruction(inst);
            opcodes.contains(&instruction.opcode.mnemonic()) && types.is_none_or(|types| types.contains(&self.class(Some(instruction.ty)))) && instruction.operands.len() >= operands
        })
    }

    /// Operand `index` of the instruction defining `operand`.
    fn inner(&self, operand: Operand, index: usize) -> Operand {
        self.function.instruction(self.definition(operand).expect("a nested instruction")).operands[index]
    }

    fn cover(&mut self, operand: Operand, root: InstId) {
        self.covered.insert(self.definition(operand).expect("a nested instruction"), root);
    }

    fn covered_by(&self, operand: Operand, root: InstId) -> bool {
        self.definition(operand).is_some_and(|inst| self.covered.get(&inst) == Some(&root))
    }

    fn emitted(&self, m: &Match, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, volatile: bool) -> Arc<Insn> {
        let what = semantics(op, name, dests, sources);
        if volatile { Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }) } else { insn(m.at, what) }
    }

    fn opcode(&self, m: &Match) -> &Opcode {
        &self.function.instruction(m.inst).opcode
    }

    fn cast(&self, m: &Match) -> CastOp {
        let Opcode::Cast(op) = *self.opcode(m) else { unreachable!("a cast pattern") };
        op
    }

    /// The type a load reads or a store writes.
    fn accessed(&self, m: &Match) -> TypeId {
        let instruction = self.function.instruction(m.inst);
        if matches!(instruction.opcode, Opcode::Store { .. }) { self.type_of(instruction.operands[0]) } else { instruction.ty }
    }

    // Operand constructors: a LIR operand, and what making it took.

    fn op_held(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, operand: Operand) -> Result<Loc, Unselected> {
        Ok(Loc::Held(self.held(operand, self.type_of(operand), m.at, out)?))
    }

    fn op_source(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, operand: Operand) -> Result<Loc, Unselected> {
        self.source(operand, self.type_of(operand), m.at, out)
    }

    /// Held, as its byte.
    fn op_byte(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, operand: Operand) -> Result<Loc, Unselected> {
        let held = self.held(operand, self.type_of(operand), m.at, out)?;
        Ok(Loc::Held(Held { width: 1, ..held }))
    }

    /// A shift's count: cl counts, so a register count is its byte.
    fn op_count(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, operand: Operand) -> Result<Loc, Unselected> {
        Ok(match self.source(operand, self.type_of(operand), m.at, out)? {
            Loc::Held(count) => Loc::Held(Held { width: 1, ..count }),
            other => other,
        })
    }

    /// Held, at the result's width; a joined dword's low word is the word
    /// it was joined from.
    fn op_narrowed(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, operand: Operand) -> Result<Loc, Unselected> {
        let to = self.width(self.function.instruction(m.inst).ty)?;
        let held = self.held(operand, self.type_of(operand), m.at, out)?;
        let held = match self.joins.get(&held.value) {
            Some(&(low, _)) if to <= 2 => low,
            _ => held,
        };
        Ok(Loc::Held(Held { width: to, ..held }))
    }

    fn op_result(&mut self, m: &Match, _: &mut Vec<Arc<Insn>>) -> Result<Loc, Unselected> {
        let instruction = self.function.instruction(m.inst);
        let width = self.width(instruction.ty)?;
        Ok(Loc::Held(Held { value: self.value(instruction.result.expect("a result")), width }))
    }

    /// A fresh register of the result's width.
    fn op_fresh(&mut self, m: &Match, _: &mut Vec<Arc<Insn>>) -> Result<Loc, Unselected> {
        let width = self.width(self.function.instruction(m.inst).ty)?;
        Ok(Loc::Held(self.fresh_held(width)))
    }

    fn op_float(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, operand: Operand) -> Result<Loc, Unselected> {
        Ok(Loc::Held(self.float(operand, m.at, out)?))
    }

    fn op_fresult(&mut self, m: &Match, _: &mut Vec<Arc<Insn>>) -> Result<Loc, Unselected> {
        let result = self.function.instruction(m.inst).result.expect("a result");
        Ok(Loc::Held(Held { value: self.value(result), width: FLOAT }))
    }

    /// The cell a float load or store reaches: its bytes in memory.
    fn op_cell(&mut self, m: &Match, _: &mut Vec<Arc<Insn>>, pointer: Operand) -> Result<Loc, Unselected> {
        let pointer = self.pointer(pointer)?;
        Ok(Loc::Mem(Self::memory(pointer, self.size(self.accessed(m))?)))
    }

    /// The cell the load defining `value` reads.
    fn op_loaded(&mut self, _: &Match, _: &mut Vec<Arc<Insn>>, value: Operand) -> Result<Loc, Unselected> {
        let loaded = self.function.instruction(self.definition(value).expect("a load"));
        let pointer = self.pointer(loaded.operands[0])?;
        Ok(Loc::Mem(Self::memory(pointer, self.size(loaded.ty)?)))
    }

    /// The cell an integer load or store reaches: its register's bytes.
    fn op_access(&mut self, m: &Match, _: &mut Vec<Arc<Insn>>, pointer: Operand) -> Result<Loc, Unselected> {
        let width = self.width(self.accessed(m))?;
        Ok(Loc::Mem(Self::memory(self.pointer(pointer)?, width)))
    }

    fn op_imm(&mut self, _: &Match, _: &mut Vec<Arc<Insn>>, value: i64, width: i64) -> Result<Loc, Unselected> {
        Ok(Loc::Imm(Imm { value, width: width as u32, address: None }))
    }

    // Predicates.

    fn is_selected_elsewhere(&self, m: &Match) -> bool {
        self.covered.contains_key(&m.inst) || self.consumed.contains(&m.inst)
    }

    /// Whether `value` is read once, by the root, in its block.
    fn is_only_reader(&self, m: &Match, value: Operand) -> bool {
        matches!(value, Operand::Value(value) if self.only_reader(value, m.inst))
    }

    fn is_nonvolatile(&self, _: &Match, load: Operand) -> bool {
        self.definition(load).is_some_and(|inst| matches!(self.function.instruction(inst).opcode, Opcode::Load { volatile: false, .. }))
    }

    /// Whether nothing may write memory between the load defining `value`
    /// and where the root is made: beside the branch, for a fused compare.
    fn is_unwritten(&self, m: &Match, value: Operand) -> bool {
        let Some(load) = self.definition(value) else { return false };
        let Some(block) = self.function.parent(m.inst) else { return false };
        let at = if self.fused.contains(&m.inst) { self.function.terminator(block).expect("a terminator") } else { m.inst };
        self.unwritten(load, at)
    }

    /// Whether `value` is what the float comparison compares second, as
    /// its row in FLOAT_CONDITIONS orders the operands.
    fn is_compared_second(&self, m: &Match, value: Operand) -> bool {
        let Opcode::FCmp(predicate) = *self.opcode(m) else { return false };
        float_conditions(predicate).is_some_and(|(swapped, _)| m.ops[usize::from(!swapped)] == value)
    }

    fn is_lrint(&self, _: &Match, call: Operand) -> bool {
        self.definition(call).is_some_and(|inst| self.lrint(inst))
    }

    fn is_narrowed(&self, m: &Match) -> bool {
        self.words.contains_key(&m.inst)
    }

    fn is_volatile(&self, m: &Match) -> bool {
        m.volatile
    }

    fn is_fused(&self, m: &Match) -> bool {
        self.fused.contains(&m.inst)
    }

    /// Whether a multiply by `factor` has a chain of shifts and adds whose
    /// shifts all fit the width.
    fn is_scalable(&self, m: &Match, factor: Operand) -> bool {
        self.chain(m, factor).is_some_and(|(chain, _)| {
            let width = self.width(self.function.instruction(m.inst).ty).expect("a held width");
            chain.iter().all(|&(name, count)| name != "shl" || count < i64::from(width) * 8)
        })
    }

    fn is_same_width(&self, m: &Match, operand: Operand) -> bool {
        let to = self.width(self.function.instruction(m.inst).ty);
        to.is_ok() && to == self.width(self.type_of(operand))
    }

    fn chain(&self, m: &Match, factor: Operand) -> Option<(Vec<(&'static str, i64)>, i64)> {
        let width = self.width(self.function.instruction(m.inst).ty).ok()?;
        let n = self.constant(factor, width).filter(|&n| matches!(width, 2 | 4) && 1 < n)?;
        arithmetic::cheapest_chain(n, self.cpu).ok().flatten()
    }

    // Costs.

    fn cost_immediate_multiply(&self, m: &Match, factor: Operand) -> Result<i64, Unselected> {
        let width = self.width(self.function.instruction(m.inst).ty)?;
        let n = self.constant(factor, width).expect("an integer factor");
        arithmetic::immediate_multiply(self.cpu, n).map_err(Unselected)
    }

    fn cost_scaled(&self, m: &Match, factor: Operand) -> Result<i64, Unselected> {
        Ok(self.chain(m, factor).expect("a scalable factor").1)
    }

    // Hooks: what is selected by hand.

    /// Shifts and adds of the source, as the old route's _scaled selects.
    fn hook_scaled(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, source: Operand, factor: Operand) -> Result<(), Unselected> {
        let a = self.op_held(m, out, source)?;
        let (chain, _) = self.chain(m, factor).expect("a scalable factor");
        let Loc::Held(result) = self.op_result(m, out)? else { unreachable!("a register") };
        let mut current = a.clone();
        for (index, &(name, count)) in chain.iter().enumerate() {
            let into = if index == chain.len() - 1 { result } else { self.fresh_held(result.width) };
            let other = if name == "shl" { Loc::Imm(Imm { value: count, width: 1, address: None }) } else { a.clone() };
            out.push(insn(m.at, semantics(Operation::Binary, name, vec![Loc::Held(into)], vec![current, other])));
            current = Loc::Held(into);
        }
        Ok(())
    }

    fn hook_divide(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let Opcode::Binary(op) = *self.opcode(m) else { unreachable!("a division") };
        self.divide(op, m.inst, out)
    }

    fn hook_wide_cast(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        self.wide_cast(self.cast(m), m.inst, m.at, out)
    }

    fn hook_wide_binary(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let Opcode::Binary(op) = *self.opcode(m) else { unreachable!("a binary operation") };
        self.wide_binary(op, m.inst, m.at, out)
    }

    fn hook_float_cast(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        self.float_cast(self.cast(m), m.inst, m.at, out)
    }

    fn hook_far_cast(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        self.far_cast(self.cast(m), m.inst, m.at, out)
    }

    /// A cast no pattern selects, refused once its types are.
    fn hook_unselected_cast(&mut self, m: &Match, _: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let instruction = self.function.instruction(m.inst);
        self.width(instruction.ty)?;
        self.width(self.type_of(instruction.operands[0]))?;
        refuse(instruction.opcode.mnemonic())
    }

    /// A dword load read only as words: each word loaded once.
    fn hook_words(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, pointer: Operand) -> Result<(), Unselected> {
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

    fn hook_getelementptr(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let instruction = self.function.instruction(m.inst);
        let Opcode::GetElementPtr { source } = instruction.opcode else { unreachable!("a getelementptr") };
        if self.folded(instruction.result.expect("an address"))?.is_none() {
            self.indexed(m.inst, source, m.at, out)?;
        }
        Ok(())
    }

    /// A float constant is its bits, stored as integers are.
    fn hook_float_bits(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, constant: Operand, pointer: Operand) -> Result<(), Unselected> {
        let (pointer, size) = (self.pointer(pointer)?, self.size(self.type_of(constant))?);
        let Operand::Constant(id) = constant else { unreachable!("a constant") };
        let ConstantKind::Float(bits) = self.module.context.get(id).kind else { return refuse("a float constant of no bits") };
        let bits = if size == 4 { u128::from(bits as u32) } else { u128::from(bits) };
        let low = semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(pointer, 4))], vec![Loc::Imm(Imm { value: bits as u32 as i64, width: 4, address: None })]);
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

    fn hook_far_load(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, pointer: Operand) -> Result<(), Unselected> {
        let pointer = self.pointer(pointer)?;
        let (offset, selector) = self.far_loaded(pointer, m.volatile, m.at, out);
        self.fars.insert(self.function.instruction(m.inst).result.expect("a load's value"), (Some(offset), selector));
        Ok(())
    }

    fn hook_far_store(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>, value: Operand, pointer: Operand) -> Result<(), Unselected> {
        let words = match self.far_words(value)? {
            Some(words) => words,
            None => self.far(value, m.at, out).map(|(offset, selector)| [Loc::Held(offset), Loc::Held(selector)])?,
        };
        let pointer = self.pointer(pointer)?;
        for (word, by) in words.into_iter().zip([0, 2]) {
            let what = semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(pointer.moved(by), 2))], vec![word]);
            out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
        }
        Ok(())
    }

    /// A field of an answer in registers: the register it came in.
    fn hook_extractvalue(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let instruction = self.function.instruction(m.inst);
        let Opcode::ExtractValue(indices) = &instruction.opcode else { unreachable!("an extractvalue") };
        let field = match (instruction.operands[0], &indices[..]) {
            (Operand::Value(aggregate), &[index]) => self.fields.get(&aggregate).and_then(|fields| fields.get(index as usize)).copied(),
            _ => None,
        };
        let Some(field) = field else { return refuse("an extractvalue of no answer in registers") };
        let result = Held { value: self.value(instruction.result.expect("a field")), width: field.width };
        out.push(insn(m.at, semantics(Operation::Move, "mov", vec![Loc::Held(result)], vec![Loc::Held(field)])));
        Ok(())
    }

    /// SETcc, as LLVM selects a comparison it keeps as a value.
    fn hook_setcc(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let test = self.compare(m.inst, m.at, out)?;
        let result = Held { value: self.value(self.function.instruction(m.inst).result.expect("a result")), width: 1 };
        let at = m.at;
        let set = |code: &str, into: Held| insn(at, semantics(Operation::Unary, &format!("set{}", &code[1..]), vec![Loc::Held(into)], vec![]));
        match test {
            Test::One(code) => out.push(set(code, result)),
            Test::Both(a, b) | Test::Either(a, b) => {
                let (first, second) = (self.fresh_held(1), self.fresh_held(1));
                out.extend([set(a, first), set(b, second)]);
                let join = if matches!(test, Test::Both(..)) { "and" } else { "or" };
                out.push(insn(at, semantics(Operation::Binary, join, vec![Loc::Held(result)], vec![Loc::Held(first), Loc::Held(second)])));
            }
        }
        Ok(())
    }

    fn hook_br(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        self.branch(m.inst, m.block_at, m.at, out)
    }

    fn hook_ret(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        self.ret(m.inst, m.convention, m.at, out)
    }

    fn hook_call(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let Opcode::Call(info) = self.opcode(m) else { unreachable!("a call") };
        super::in_the_frame(&info.argument_attrs)?;
        self.call(m.inst, info.calling_convention, m.at, out)
    }

    fn hook_invoke(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let Opcode::Invoke(info) = self.opcode(m) else { unreachable!("an invoke") };
        super::in_the_frame(&info.argument_attrs)?;
        self.call(m.inst, info.calling_convention, m.at, out)?;
        let block = self.function.parent(m.inst).expect("a placed invoke");
        out.push(insn(m.at, super::jump(m.block_at[&self.successors(block)[0]])));
        Ok(())
    }

    fn hook_landing_pad(&mut self, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
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
