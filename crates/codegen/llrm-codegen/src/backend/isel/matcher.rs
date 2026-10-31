//! The runtime of the generated selector: it matches a MIR instruction against
//! the target's patterns and hosts what the patterns call by name.
//!
//! # How it works
//! `llrm-iselgen` turns a target's `patterns.isel` into a `Compiled`: a
//! decision automaton (`State`) and the `holds`, `cost` and `emit` functions of
//! each pattern. This file runs it, for `Selector::instruction`:
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   a["<b>instruction -#gt;</b><br/><small>Match #123; ops, at, ... #125;<br/>(constant second if<br/>commutative)</small>"]:::mid
//!   b["<b>features: [opcode,</b><br/><small>result type, (kind, type)<br/>x up to 3 operands]</small>"]:::mid
//!   c["<b>candidates, in file order</b>"]:::mid
//!   d["<b>emit(pattern)</b><br/><small>LIR instructions, or<br/>refuse(opcode) if none holds</small>"]:::mid
//!   a --> b
//!   b -->|"walk: Test #123; feature,<br/>edges, default #125; ...<br/>-#gt; Leaf(patterns)"| c
//!   c -->|"choose: the first whose<br/>holds is true#59; in a group,<br/>the cheapest holding member<br/>(the earlier on a tie)"| d
//! ```
//!
//! *Matching one instruction to a pattern*
//!
//! `covered_by_pattern` is the cover phase that runs before any instruction is
//! emitted: the first covering pattern that holds marks the nested instruction
//! it covers (`cover`, `covered_by`), and that instruction is then selected by
//! the cover alone.
//!
//! What a pattern names is a method here, by prefix:
//!
//! - `op_` operand constructors (`let x = held(a)`): make the value, immediate,
//!   memory cell or x87 operand an `emit` takes.
//! - `is_` predicates (`when only_reader(v)`): facts about the MIR the pattern
//!   needs, such as a single reader, an unwritten cell, or a promoted word.
//! - `cost_` costs of a `group` member.
//! - `hook_` selection by hand, for what is not pattern-shaped: calls, wide and
//!   far loads, stores and casts, `getelementptr`, `setcc`, `br`, `ret`,
//!   landing pads, scaled multiplies, float bit patterns.
//!
//! # Example
//! No example: the runtime of the generated tables, with no output of its own.
//! The example in `isel.rs` shows what it selects.
//!
//! # Used by
//! `isel.rs` (`Selector::instruction` and `covered_by_pattern`) and the
//! emitters generated into each target's selector.
//!
//! # See also
//! `llrm-iselgen` (`crates/backend/llrm-iselgen`); LLVM's `DAGISelMatcher`.

use std::sync::Arc;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, InstId, Module, Operand, ValueDef};
use llrm_mir::{BinaryOp, CastOp, ConstantKind, Opcode, TypeId};

use super::{
    Convention, FLOAT, Pointer, Selector, Test, TypeClass, Unselected, float_conditions, insn, insn_of, refuse,
    semantics,
};
use crate::backend::arithmetic;
use crate::model::ir::{Field, FieldKind, Held, Imm, Loc, Operation};
use crate::model::lir::Insn;
use crate::support::hash::IndexMap;

pub enum State {
    Test { feature: usize, edges: &'static [(u16, usize)], default: Option<usize> },
    Leaf(&'static [usize]),
}

/// The most operands a pattern tests.
pub const OPERANDS: usize = 3;

pub type Holds = for<'a, 'b, 'c, 'd> fn(&mut Selector<'a, 'b, 'c>, usize, &Match<'d>) -> bool;
pub type Costs = for<'a, 'b, 'c, 'd> fn(&mut Selector<'a, 'b, 'c>, usize, &Match<'d>) -> Result<i64, Unselected>;
pub type Emits = for<'a, 'b, 'c, 'd, 'e> fn(
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
    pub opcodes: &'static [&'static str],
    pub types: &'static [&'static str],
    pub kinds: &'static [&'static str],
    pub commutative: &'static [&'static str],
    pub root: Option<usize>,
    pub states: &'static [State],
    pub groups: &'static [Option<usize>],
    pub covers: &'static [bool],
    pub holds: Holds,
    pub covers_here: Holds,
    pub cost: Costs,
    pub emit: Emits,
    /// The target's peephole rules, generated from the same directory.
    pub rules: &'static crate::backend::peep::Rules,
}

impl Compiled {
    /// The peephole rules generated beside this selector.
    pub fn rules(&self) -> &'static crate::backend::peep::Rules {
        self.rules
    }
}

/// The fixture selector of the target `name`, as its definition directory is
/// named: what the tests of this crate use in place of a select crate's.
#[cfg(feature = "fixtures")]
pub fn selector(name: &str) -> Option<&'static Compiled> {
    crate::backend::selectors::ALL.iter().copied().find(|one| one.name == name)
}

/// The selector built for 16-bit x86, which the tests of this crate use.
#[cfg(test)]
pub(crate) fn m16() -> &'static Compiled {
    &crate::backend::selectors::x86_m16::SELECTOR
}

/// The operand kinds `patterns.isel` names, in the order of
/// `Selector::kind_slot`.
pub(super) const KINDS: [&str; 6] = ["value", "block", "int", "poison", "fconst", "const"];

/// Where `name` stands in `domain`, or `u16::MAX` where the target's patterns
/// never name it: found when a selector is made, not for each instruction.
pub(super) fn position(
    domain: &[&str],
    name: &str,
) -> u16 {
    domain.iter().position(|one| *one == name).map_or(u16::MAX, |at| at as u16)
}

fn known(at: u16) -> u16 {
    assert_ne!(at, u16::MAX, "a name the generator knows");
    at
}

#[cfg(test)]
thread_local! {
    pub(crate) static OPCODE_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The instruction a pattern matched: its operands, a commutative
/// opcode's constant taken second.
pub struct Match<'a> {
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

/// The type of field `index` of a struct's `types`.
fn instruction_field(
    types: &[llrm_mir::TypeId],
    index: u32,
) -> Result<llrm_mir::TypeId, Unselected> {
    types.get(index as usize).copied().ok_or_else(|| Unselected(format!("field {index} of an aggregate")))
}

impl Selector<'_, '_, '_> {
    /// The opcode's name, for a pattern that refuses an instruction by it.
    pub fn mnemonic(
        &self,
        m: &Match,
    ) -> &'static str {
        self.function.instruction(m.inst).opcode.mnemonic()
    }

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
        let first = out.len();
        (self.compiled.emit)(self, chosen, &m, out)?;
        if self.loads_constant_memory(inst) {
            for one in &mut out[first..] {
                let plain = one
                    .what
                    .as_ref()
                    .is_some_and(
                        |what| matches!(what.op, Operation::Move | Operation::FloatLoad)
                            && matches!(what.sources.as_slice(), [Loc::Mem(_)]),
                    );
                if plain && !one.volatile {
                    *one = Arc::new(Insn { invariant: true, ..(**one).clone() });
                }
            }
        }
        Ok(())
    }

    /// `AA->pointsToConstantMemory` for a load: see [`reads_invariant_memory`].
    fn loads_constant_memory(
        &self,
        inst: InstId,
    ) -> bool {
        reads_invariant_memory(self.module, self.function, &self.layout, inst)
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
        let mut out = [0; 2 + 2 * OPERANDS];
        out[0] =
            *self.opcodes.borrow_mut().entry(instruction.opcode.mnemonic().as_ptr() as usize).or_insert_with(|| {
                #[cfg(test)]
                OPCODE_SCANS.with(|one| one.set(one.get() + 1));
                known(position(self.compiled.opcodes, instruction.opcode.mnemonic()))
            });
        out[1] = known(self.type_at[TypeClass::of(self.types(), &self.layout, Some(instruction.ty)) as usize]);
        for (index, &operand) in m.ops.iter().take(OPERANDS).enumerate() {
            out[2 + 2 * index] = known(self.kind_at[self.kind_slot(operand)]);
            let ty = self.function.operand_type(&self.module.context, operand);
            out[3 + 2 * index] = known(self.type_at[TypeClass::of(self.types(), &self.layout, ty) as usize]);
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
        KINDS[self.kind_slot(operand)]
    }

    /// `operand`'s place in `KINDS`.
    fn kind_slot(
        &self,
        operand: Operand,
    ) -> usize {
        match operand {
            Operand::Value(_) => 0,
            Operand::Block(_) => 1,
            Operand::Constant(id) => match self.module.context.get(id).kind {
                ConstantKind::Int(_) | ConstantKind::Null | ConstantKind::Zero => 2,
                ConstantKind::Poison | ConstantKind::Undef => 3,
                ConstantKind::Float(_) | ConstantKind::Float80(_) => 4,
                _ => 5,
            },
        }
    }

    pub(super) fn type_of(
        &self,
        operand: Operand,
    ) -> TypeId {
        self.function.operand_type(&self.module.context, operand).expect("a typed operand")
    }

    /// An integer constant's value at its own width.
    pub fn literal(
        &self,
        operand: Operand,
    ) -> Option<i64> {
        let width = self.width(self.type_of(operand)).ok()?;
        self.constant(operand, width)
    }

    /// Whether `operand` is of one of `kinds` and `types`.
    pub fn operand_is(
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
    pub fn defines(
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
    pub fn inner(
        &self,
        operand: Operand,
        index: usize,
    ) -> Operand {
        self.function.instruction(self.definition(operand).expect("a nested instruction")).operands[index]
    }

    pub fn cover(
        &mut self,
        operand: Operand,
        root: InstId,
    ) {
        self.covered.insert(self.definition(operand).expect("a nested instruction"), root);
    }

    pub fn covered_by(
        &self,
        operand: Operand,
        root: InstId,
    ) -> bool {
        self.definition(operand).is_some_and(|inst| self.covered.get(&inst) == Some(&root))
    }

    pub fn emitted(
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

    pub fn op_held(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        Ok(Loc::Held(self.held(operand, self.type_of(operand), m.at, out)?))
    }

    pub fn op_source(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        self.source(operand, self.type_of(operand), m.at, out)
    }

    /// Held, as its byte.
    pub fn op_byte(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        let held = self.held(operand, self.type_of(operand), m.at, out)?;
        Ok(Loc::Held(Held { width: 1, ..held }))
    }

    /// A shift's count: cl counts, so a register count is its byte.
    pub fn op_count(
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
    pub fn op_narrowed(
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

    /// Held, read at the native width: an operation the target runs there
    /// (`promote`) reads what the narrow value left above it and does not care.
    pub fn op_promoted(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        let held = self.held(operand, self.type_of(operand), m.at, out)?;
        Ok(Loc::Held(Held { width: self.native_width(), ..held }))
    }

    /// As `source`, a register at the native width or an immediate.
    pub fn op_promoted_source(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        Ok(match self.source(operand, self.type_of(operand), m.at, out)? {
            Loc::Held(held) => Loc::Held(Held { width: self.native_width(), ..held }),
            Loc::Imm(imm) => Loc::Imm(Imm { width: self.native_width(), ..imm }),
            other => other,
        })
    }

    /// The result, written at the native width.
    pub fn op_promoted_result(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
    ) -> Result<Loc, Unselected> {
        let instruction = self.function.instruction(m.inst);
        Ok(Loc::Held(Held { value: self.value(instruction.result.expect("a result")), width: self.native_width() }))
    }

    pub fn op_result(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
    ) -> Result<Loc, Unselected> {
        let instruction = self.function.instruction(m.inst);
        let width = self.width(instruction.ty)?;
        Ok(Loc::Held(Held { value: self.value(instruction.result.expect("a result")), width }))
    }

    /// A fresh register of the result's width.
    pub fn op_fresh(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
    ) -> Result<Loc, Unselected> {
        let width = self.width(self.function.instruction(m.inst).ty)?;
        Ok(Loc::Held(self.fresh_held(width)))
    }

    pub fn op_float(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        operand: Operand,
    ) -> Result<Loc, Unselected> {
        Ok(Loc::Held(self.float(operand, m.at, out)?))
    }

    pub fn op_fresult(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
    ) -> Result<Loc, Unselected> {
        let result = self.function.instruction(m.inst).result.expect("a result");
        let bytes = self.size(self.function.instruction(m.inst).ty)?;
        Ok(Loc::Held(Held { value: self.value(result), width: self.arch.float_width(bytes) }))
    }

    /// The cell a float load or store reaches: its bytes in memory.
    pub fn op_cell(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
        pointer: Operand,
    ) -> Result<Loc, Unselected> {
        let regs = self.regs();
        let pointer = self.pointer(pointer)?;
        Ok(Loc::Mem(Self::memory(regs, pointer, self.size(self.accessed(m))?)))
    }

    /// The cell the load defining `value` reads.
    pub fn op_loaded(
        &mut self,
        _: &Match,
        _: &mut Vec<Arc<Insn>>,
        value: Operand,
    ) -> Result<Loc, Unselected> {
        let regs = self.regs();
        let loaded = self.function.instruction(self.definition(value).expect("a load"));
        let pointer = self.pointer(loaded.operands[0])?;
        Ok(Loc::Mem(Self::memory(regs, pointer, self.size(loaded.ty)?)))
    }

    /// The cell an integer load or store reaches: its register's bytes.
    pub fn op_access(
        &mut self,
        m: &Match,
        _: &mut Vec<Arc<Insn>>,
        pointer: Operand,
    ) -> Result<Loc, Unselected> {
        let regs = self.regs();
        let width = self.width(self.accessed(m))?;
        Ok(Loc::Mem(Self::memory(regs, self.pointer(pointer)?, width)))
    }

    pub fn op_imm(
        &mut self,
        _: &Match,
        _: &mut Vec<Arc<Insn>>,
        value: i64,
        width: i64,
    ) -> Result<Loc, Unselected> {
        Ok(Loc::Imm(Imm { value, width: width as u32, address: None }))
    }

    /// A shifted register's field: the target's shift `code` and the `amount`.
    pub fn op_shift(
        &mut self,
        _: &Match,
        _: &mut Vec<Arc<Insn>>,
        code: i64,
        amount: i64,
    ) -> Result<Loc, Unselected> {
        Ok(Loc::Field(Field { kind: FieldKind::Shift, code: code as u8, amount: amount as u8 }))
    }

    /// A condition field: the target's condition `code`.
    pub fn op_cond(
        &mut self,
        _: &Match,
        _: &mut Vec<Arc<Insn>>,
        code: i64,
    ) -> Result<Loc, Unselected> {
        Ok(Loc::Field(Field { kind: FieldKind::Condition, code: code as u8, amount: 0 }))
    }

    /// An extended register's field: the target's extension `code` and the
    /// `amount`.
    pub fn op_extend(
        &mut self,
        _: &Match,
        _: &mut Vec<Arc<Insn>>,
        code: i64,
        amount: i64,
    ) -> Result<Loc, Unselected> {
        Ok(Loc::Field(Field { kind: FieldKind::Extend, code: code as u8, amount: amount as u8 }))
    }

    // Predicates.

    /// Whether the target would rather run this operation, at its width, on
    /// the whole register (`promote`).
    pub fn is_promoted(
        &self,
        m: &Match,
    ) -> bool {
        let instruction = self.function.instruction(m.inst);
        let Some(bits) = self.types().int_bits(instruction.ty) else { return false };
        // What decides most operations first: asked of every add, and / or /
        // xor of a function, so no allocation and none of what follows
        // for a dword.
        if bits >= self.native_width() * 8 || self.promote.is_empty() {
            return false;
        }
        let mnemonic = instruction.opcode.mnemonic();
        let listed = |one: &String| {
            one.strip_suffix(&format!("i{bits}"))
                .and_then(|rest| rest.strip_suffix('.'))
                .is_some_and(|rest| rest == mnemonic)
        };
        if !self.promote.iter().any(listed) {
            return false;
        }
        // Not where a load nothing else reads can be the operation's memory
        // operand (the other is a register, not a constant), or its cell is
        // updated in place (a store is the only reader of the result): the word
        // operation does that, the dword operation cannot (LLVM's
        // `IsDesirableToPromoteOp`: `MayFoldLoad`, `IsFoldableRMW`).
        let loaded = |operand: Operand| {
            let Operand::Value(value) = operand else { return false };
            let sole = self.function.users(value).only().is_some_and(|only| only.user == m.inst);
            sole && match self.function.value(value).def {
                ValueDef::Instruction(inst) => matches!(self.function.instruction(inst).opcode, Opcode::Load { .. }),
                ValueDef::Argument(_) => self.in_cells.contains(&value),
            }
        };
        let width = bits / 8;
        let registers = instruction.operands.iter().filter(|operand| self.constant(**operand, width).is_none()).count();
        let stored = instruction
            .result
            .is_some_and(
                |result| self.function
                    .users(result)
                    .only()
                    .is_some_and(|only| matches!(
                        self.function.instruction(only.user).opcode,
                        Opcode::Store { .. }
                    )),
            );
        let foldable = instruction.operands.iter().any(|operand| loaded(*operand)) && (registers >= 2 || stored);
        // Nor where a compare reads the result: the word's flags serve it, and
        // the dword's are of bits it cannot ignore (`inc cx; jne` against
        // `inc ecx; test cx, cx; jne`).
        let compared = instruction
            .result
            .is_some_and(
                |result| {
                    let users = self.function.users(result);
                    users.iter().any(|user| matches!(self.function.instruction(user.user).opcode, Opcode::ICmp(_)))
                },
            );
        !foldable && !compared
    }

    /// The bytes of the widest integer the target computes natively.
    fn native_width(&self) -> u32 {
        self.layout.largest_legal_integer() / 8
    }

    pub fn is_selected_elsewhere(
        &self,
        m: &Match,
    ) -> bool {
        self.covered.contains_key(&m.inst) || self.consumed.contains(&m.inst)
    }

    /// Whether `value` is read once, by the root, in its block.
    pub fn is_only_reader(
        &self,
        m: &Match,
        value: Operand,
    ) -> bool {
        matches!(
            value,
            Operand::Value(value) if self.only_reader(value, m.inst)
        )
    }

    /// Whether `value` is free after the root (`dies_at`).
    pub fn is_dies(
        &self,
        m: &Match,
        value: Operand,
    ) -> bool {
        matches!(
            value,
            Operand::Value(value) if self.dies_at(value, m.inst)
        )
    }

    /// Whether a `getelementptr` takes the root's result, directly or through
    /// the constants added to it (`peeled`: `x + y - 1` is an address's
    /// displacement): the address reads the sum's parts, not the sum, so a load
    /// covered by the sum is never selected.
    pub fn is_indexing(
        &self,
        m: &Match,
    ) -> bool {
        let function = self.function;
        let mut at = function.instruction(m.inst).result;
        while let Some(result) = at {
            if function
                .users(result)
                .iter()
                .any(|one| matches!(
                    function.instruction(one.user).opcode,
                    Opcode::GetElementPtr { .. }
                ))
            {
                return true;
            }
            // One reader that adds or subtracts a constant: the chain goes on
            // there.
            at = function.users(result).only().and_then(|one| {
                let reading = function.instruction(one.user);
                let constant = matches!(&reading.operands[..], [_, by] if self.literal(*by).is_some());
                (matches!(reading.opcode, Opcode::Binary(BinaryOp::Add | BinaryOp::Sub)) && one.index == 0 && constant)
                    .then_some(reading.result)
                    .flatten()
            });
        }
        false
    }

    pub fn is_nonvolatile(
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

    /// Whether the root's value is stored back to where one of its operands
    /// was loaded from: a read-modify-write, which `rmw` makes one instruction
    /// of the load, the operation and the store, and not a memory operand of
    /// this operation (that leaves the other operand's load and the store
    /// apart).
    pub fn is_stored_back(
        &self,
        m: &Match,
        _load: Operand,
    ) -> bool {
        let function = self.function;
        let Some(result) = function.instruction(m.inst).result else { return false };
        let Some(only) = function.users(result).only() else { return false };
        let store = function.instruction(only.user);
        if !matches!(store.opcode, Opcode::Store { .. }) || only.index != 0 {
            return false;
        }
        function
            .instruction(m.inst)
            .operands
            .iter()
            .any(
                |&operand| self.definition(operand).is_some_and(|inst| {
                    let read = function.instruction(inst);
                    matches!(read.opcode, Opcode::Load { .. }) && self.same_address(read.operands[0], store.operands[1])
                }),
            )
    }

    /// Whether `load` reads a local: an alloca, or a GEP from one.
    pub fn is_local(
        &self,
        _: &Match,
        load: Operand,
    ) -> bool {
        let Some(load) = self.definition(load) else { return false };
        let mut at = self.function.instruction(load).operands[0];
        while let Some(inst) = self.definition(at) {
            match self.function.instruction(inst).opcode {
                Opcode::Alloca { .. } => return true,
                Opcode::GetElementPtr { .. } => at = self.function.instruction(inst).operands[0],
                _ => return false,
            }
        }
        false
    }

    /// Whether two pointer operands compute one address: the same value, or
    /// two GEPs of the same type from the same address with the same indices.
    fn same_address(
        &self,
        one: Operand,
        other: Operand,
    ) -> bool {
        if one == other {
            return true;
        }
        let (Some(first), Some(second)) = (self.definition(one), self.definition(other)) else { return false };
        let (first, second) = (self.function.instruction(first), self.function.instruction(second));
        matches!(
            (&first.opcode, &second.opcode),
            (Opcode::GetElementPtr { source: a }, Opcode::GetElementPtr { source: b }) if a == b
        ) && first.operands.len() == second.operands.len()
            && self.same_address(first.operands[0], second.operands[0])
            && first.operands[1..] == second.operands[1..]
    }

    /// Whether nothing may write memory between the load defining `value`
    /// and where the root is made: beside the branch, for a fused compare.
    pub fn is_unwritten(
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
    pub fn is_compared_second(
        &self,
        m: &Match,
        value: Operand,
    ) -> bool {
        let Opcode::FCmp(predicate) = *self.opcode(m) else { return false };
        float_conditions(predicate).is_some_and(|(swapped, _)| m.ops[usize::from(!swapped)] == value)
            && matches!(self.size(self.type_of(value)), Ok(4 | 8))
    }

    pub fn is_lrint(
        &self,
        _: &Match,
        call: Operand,
    ) -> bool {
        self.definition(call).is_some_and(|inst| self.lrint(inst))
    }

    /// Whether an i64 is held as two halves: the datalayout names no legal
    /// integer as wide.
    pub fn is_splits_i64(
        &self,
        _: &Match,
    ) -> bool {
        self.layout.largest_legal_integer() < 64
    }

    pub fn is_narrowed(
        &self,
        m: &Match,
    ) -> bool {
        self.words.contains_key(&m.inst)
    }

    pub fn is_volatile(
        &self,
        m: &Match,
    ) -> bool {
        m.volatile
    }

    pub fn is_fused(
        &self,
        m: &Match,
    ) -> bool {
        self.fused.contains(&m.inst)
    }

    /// Whether a multiply by `factor` has a chain of shifts and adds whose
    /// shifts all fit the width.
    pub fn is_scalable(
        &self,
        m: &Match,
        factor: Operand,
    ) -> bool {
        self.chain(m, factor).is_some_and(|(chain, _)| {
            let width = self.width(self.function.instruction(m.inst).ty).expect("a held width");
            chain.iter().all(|&(name, count)| !["shl", "fadd", "fsub"].contains(&name) || count < i64::from(width) * 8)
        })
    }

    /// Whether the integer operand `k` is a shift amount of the root
    /// instruction's type: a constant in `0..bits`, on a 32- or 64-bit type.
    pub fn is_shift_amount(
        &self,
        m: &Match,
        k: Operand,
    ) -> bool {
        let Some(bits) = self.types().int_bits(self.function.instruction(m.inst).ty) else { return false };
        if bits != 32 && bits != 64 {
            return false;
        }
        let Operand::Constant(id) = k else { return false };
        match self.module.context.get(id).kind {
            ConstantKind::Int(value) => value < u128::from(bits),
            _ => false,
        }
    }

    /// Whether the root instruction carries the fast-math `contract` flag: a
    /// multiply it reads may fuse into one rounding.
    pub fn is_contract(
        &self,
        m: &Match,
    ) -> bool {
        self.function.instruction(m.inst).flags.contains(llrm_mir::opcode::Flags::CONTRACT)
    }

    pub fn is_same_width(
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
        // Run on the whole register (`promote`) it is a dword chain, `lea`s
        // too.
        if width == 4 || self.is_promoted(m) {
            arithmetic::cheapest_chain(n, self.cpu)
        } else {
            arithmetic::cheapest_narrow_chain(n, self.cpu)
        }
        .ok()
        .flatten()
    }

    // Costs.

    /// Bytes of `chain`'s shifts, adds and subtracts. The copy that seeds it is
    /// not counted: the allocator drops it where the source dies, as `add
    /// si, si` shows.
    fn chain_bytes(
        &self,
        chain: &[(&str, i64)],
        width: i64,
    ) -> i64 {
        arithmetic::chain_bytes(chain, width, self.arch)
    }

    /// Bytes first, clocks to break a tie: both are under 100.
    fn by_size(
        bytes: i64,
        clocks: i64,
    ) -> i64 {
        bytes * 100 + clocks
    }

    /// The chain a multiply of `width` bytes by the constant `n` is where it
    /// prices below `imul r,r,n` in the channel the profile tunes for (bytes,
    /// then clocks, tuned for size; else clocks): what `multiply_scaled`
    /// selects, for a multiply made outside the patterns.
    pub fn cheaper_chain(
        &self,
        n: i64,
        width: i64,
    ) -> Option<Vec<(&'static str, i64)>> {
        let (chain, clocks) = arithmetic::cheapest_chain(n, self.cpu).ok().flatten()?;
        let imul = arithmetic::immediate_multiply(self.cpu, n).ok()?;
        let (by_chain, by_imul) = if self.cpu.size {
            (
                Self::by_size(self.chain_bytes(&chain, width), clocks),
                Self::by_size(self.arch.multiply_immediate_bytes(n, width), imul),
            )
        } else {
            (clocks, imul)
        };
        (by_chain < by_imul).then_some(chain)
    }

    pub fn cost_immediate_multiply(
        &self,
        m: &Match,
        factor: Operand,
    ) -> Result<i64, Unselected> {
        let width = self.width(self.function.instruction(m.inst).ty)?;
        let n = self.constant(factor, width).expect("an integer factor");
        if self.cpu.size {
            return Ok(Self::by_size(
                self.arch.multiply_immediate_bytes(n, i64::from(width)),
                arithmetic::immediate_multiply(self.cpu, n).map_err(Unselected)?,
            ));
        }
        arithmetic::immediate_multiply(self.cpu, n).map_err(Unselected)
    }

    pub fn cost_scaled(
        &self,
        m: &Match,
        factor: Operand,
    ) -> Result<i64, Unselected> {
        let (chain, clocks) = self.chain(m, factor).expect("a scalable factor");
        // Tuned for size, the two compete in bytes, as they compete in clocks
        // otherwise.
        let width = self.width(self.function.instruction(m.inst).ty)?;
        Ok(if self.cpu.size { Self::by_size(self.chain_bytes(&chain, i64::from(width)), clocks) } else { clocks })
    }

    // Hooks: what is selected by hand.

    /// Shifts and adds of the source, as the old route's _scaled selects.
    pub fn hook_scaled(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        source: Operand,
        factor: Operand,
    ) -> Result<(), Unselected> {
        let promoted = self.is_promoted(m);
        let a = if promoted { self.op_promoted(m, out, source)? } else { self.op_held(m, out, source)? };
        let (chain, _) = self.chain(m, factor).expect("a scalable factor");
        let Loc::Held(result) = (if promoted { self.op_promoted_result(m, out)? } else { self.op_result(m, out)? })
        else {
            unreachable!("a register")
        };
        self.emit_chain(m.at, out, a, &chain, result);
        Ok(())
    }

    /// `chain`'s steps on `a`, the last into `result`: a multiply by a constant
    /// as shifts, adds and `lea`s.
    pub fn emit_chain(
        &mut self,
        at: i64,
        out: &mut Vec<Arc<Insn>>,
        a: Loc,
        chain: &[(&'static str, i64)],
        result: Held,
    ) {
        let mut current = a.clone();
        for (index, &(name, count)) in chain.iter().enumerate() {
            let into = if index == chain.len() - 1 { result } else { self.fresh_held(result.width) };
            // A step the next one reads is read; one nothing reads, the product
            // an address took into its own, goes with it.
            self.folded.insert(into.value);
            if name == "flea" {
                // `into = current + current*count`.
                let Loc::Held(base) = current.clone() else { unreachable!("a chain works on registers") };
                let cell = crate::model::ir::Mem {
                    base: Some(base),
                    index: Some(base),
                    scale: count,
                    ..crate::model::ir::Mem::new(None, result.width)
                };
                out.push(insn(at, semantics(Operation::Address, "lea", vec![Loc::Held(into)], vec![Loc::Mem(cell)])));
            } else if name == "lea" {
                // `into = a + current*count`: the shift and add of one digit,
                // made by the address unit.
                let (Loc::Held(base), Loc::Held(scaled)) = (a.clone(), current.clone()) else {
                    unreachable!("a chain works on registers")
                };
                let cell = crate::model::ir::Mem {
                    base: Some(base),
                    index: Some(scaled),
                    scale: count,
                    ..crate::model::ir::Mem::new(None, result.width)
                };
                out.push(insn(at, semantics(Operation::Address, "lea", vec![Loc::Held(into)], vec![Loc::Mem(cell)])));
            } else if name == "neg" {
                out.push(insn(at, semantics(Operation::Unary, "neg", vec![Loc::Held(into)], vec![current])));
            } else if name == "rsub" {
                // `into = source - current`.
                out.push(insn(
                    at,
                    semantics(Operation::Binary, "sub", vec![Loc::Held(into)], vec![a.clone(), current]),
                ));
            } else if name == "fadd" || name == "fsub" {
                // `current*(2^count +- 1)`: the shifted copy, then the sum or
                // difference with `current`.
                let Loc::Held(width_of) = current.clone() else { unreachable!("a chain works on registers") };
                let shifted = self.fresh_held(width_of.width);
                out.push(insn(
                    at,
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
                out.push(insn(at, semantics(Operation::Binary, add, vec![Loc::Held(into)], vec![left, right])));
            } else {
                let other =
                    if name == "shl" { Loc::Imm(Imm { value: count, width: 1, address: None }) } else { a.clone() };
                out.push(insn(at, semantics(Operation::Binary, name, vec![Loc::Held(into)], vec![current, other])));
            }
            current = Loc::Held(into);
        }
    }

    /// A byte product, by the operand size's own multiply of both factors
    /// extended: only the low byte is kept, which no extension changes.
    pub fn hook_byte_multiply(
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

    pub fn hook_divide(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let Opcode::Binary(op) = *self.opcode(m) else { unreachable!("a division") };
        self.divide(op, m.inst, out)
    }

    pub fn hook_wide_cast(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.wide_cast(self.cast(m), m.inst, m.at, out)
    }

    /// A frozen i64 is its halves (LLVM selects `FREEZE` as a copy).
    pub fn hook_wide_freeze(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let instruction = self.function.instruction(m.inst);
        let (operand, result) = (instruction.operands[0], instruction.result.expect("a freeze has a result"));
        let pair = self.wide(operand, m.at, out)?;
        self.wides.insert(result, pair);
        Ok(())
    }

    pub fn hook_wide_binary(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let Opcode::Binary(op) = *self.opcode(m) else { unreachable!("a binary operation") };
        #[cfg(test)]
        HOOKED.with(|hooked| hooked.borrow_mut().push(op));
        self.wide_binary(op, m.inst, m.at, out)
    }

    pub fn hook_float_cast(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.float_cast(self.cast(m), m.inst, m.at, out)
    }

    pub fn hook_far_cast(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.far_cast(self.cast(m), m.inst, m.at, out)
    }

    /// A cast no pattern selects, refused once its types are.
    pub fn hook_unselected_cast(
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
    pub fn hook_words(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let regs = self.regs();
        let pointer = self.pointer(pointer)?;
        let mut made: IndexMap<i64, Held> = IndexMap::default();
        let mut halves = self.words[&m.inst].clone();
        halves.sort_by_key(|&(offset, _)| offset);
        for (offset, result) in halves {
            let held = Held { value: self.value(result), width: 2 };
            let source = match made.get(&offset) {
                Some(&earlier) => Loc::Held(earlier),
                None => Loc::Mem(Self::memory(regs, pointer.moved(offset), 2)),
            };
            made.entry(offset).or_insert(held);
            out.push(insn(m.at, semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![source])));
        }
        Ok(())
    }

    pub fn hook_getelementptr(
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
    pub fn hook_float_bits(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        constant: Operand,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let regs = self.regs();
        let (pointer, size) = (self.pointer(pointer)?, self.size(self.type_of(constant))?);
        let Operand::Constant(id) = constant else { unreachable!("a constant") };
        // Poison is any value: +0.0.
        let (bits, extended) = match self.module.context.get(id).kind {
            ConstantKind::Float(bits) => (bits, None),
            ConstantKind::Float80(bits) => (0, Some(bits)),
            ConstantKind::Poison | ConstantKind::Undef => (0, None),
            _ => return refuse("a float constant of no bits"),
        };
        // An extended float is its 10 bytes: a dword, a dword and a word, as a
        // global's initializer lays them down.
        if size == 10 {
            let image = extended.map_or_else(|| llrm_mir::types::x87_extended(bits), llrm_mir::types::x87_image);
            for (by, width) in [(0_usize, 4_u32), (4, 4), (8, 2)] {
                let value =
                    image[by..by + width as usize].iter().rev().fold(0_i64, |acc, byte| acc << 8 | i64::from(*byte));
                let what = semantics(
                    Operation::Move,
                    "mov",
                    vec![Loc::Mem(Self::memory(regs, pointer.moved(by as i64), width))],
                    vec![Loc::Imm(Imm { value, width, address: None })],
                );
                out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
            }
            return Ok(());
        }
        // Tuned for size, +-0.0 and +-1.0 are `fldz`/`fld1` (and `fchs`) and a
        // store of the register: 5 to 7 bytes where two dword
        // immediates are 14. gcc allows these constants in `*movdf_internal`
        // when optimising for size (i386.md L3758-3772, the ones
        // `standard_80387_constant_p` knows, i386.cc L5113-5153), and LLVM's
        // X86 keeps them legal immediates, so it does not turn their
        // stores into integer ones (X86ISelLowering.cpp L777-780,
        // DAGCombiner.cpp L24445-24448). Not otherwise: two `mov`s take fewer
        // clocks. A lone float 0.0 or -1.0 is not: `x87_store_pays`.
        if self.cpu.size && matches!(size, 4 | 8) {
            let value = if size == 4 { f64::from(f32::from_bits(bits as u32)) } else { f64::from_bits(bits) };
            if let Some((name, negated)) = crate::backend::constpool::x87_constant(value)
                .filter(|_| crate::backend::constpool::x87_store_pays(value, size as u64))
            {
                let mut held = self.fresh_held(FLOAT);
                out.push(insn(m.at, semantics(Operation::FloatLoad, name, vec![Loc::Held(held)], vec![])));
                if negated {
                    let flipped = self.fresh_held(FLOAT);
                    out.push(insn(
                        m.at,
                        semantics(Operation::FloatUnary, "fchs", vec![Loc::Held(flipped)], vec![Loc::Held(held)]),
                    ));
                    held = flipped;
                }
                let store = semantics(
                    Operation::FloatStore,
                    "fstp",
                    vec![Loc::Mem(Self::memory(regs, pointer, size as u32))],
                    vec![Loc::Held(held)],
                );
                out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, store) }));
                return Ok(());
            }
        }
        let bits = if size == 4 { u128::from(bits as u32) } else { u128::from(bits) };
        let low = semantics(
            Operation::Move,
            "mov",
            vec![Loc::Mem(Self::memory(regs, pointer, 4))],
            vec![Loc::Imm(Imm { value: bits as u32 as i64, width: 4, address: None })],
        );
        let what = if size == 8 {
            out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, low) }));
            let high = Loc::Imm(Imm { value: (bits >> 32) as u32 as i64, width: 4, address: None });
            semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(regs, pointer.moved(4), 4))], vec![high])
        } else {
            low
        };
        out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
        Ok(())
    }

    /// An i64 read as its two dwords, the low at the address: x86 is
    /// little-endian.
    pub fn hook_wide_load(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let regs = self.regs();
        let pointer = self.pointer(pointer)?;
        let halves = self.wide_halves()?;
        let held = [self.fresh_held(halves[0].bytes), self.fresh_held(halves[1].bytes)];
        for (half, held) in halves.iter().zip(held) {
            let what = semantics(
                Operation::Move,
                "mov",
                vec![Loc::Held(held)],
                vec![Loc::Mem(Self::memory(regs, pointer.moved(half.offset), half.bytes))],
            );
            out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
        }
        self.wides.insert(self.function.instruction(m.inst).result.expect("a load's value"), (held[0], held[1]));
        Ok(())
    }

    /// An i64 written as its two dwords.
    pub fn hook_wide_store(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        value: Operand,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let regs = self.regs();
        let (low, high) = self.wide(value, m.at, out)?;
        let pointer = self.pointer(pointer)?;
        for (half, held) in self.wide_halves()?.iter().zip([low, high]) {
            let what = semantics(
                Operation::Move,
                "mov",
                vec![Loc::Mem(Self::memory(regs, pointer.moved(half.offset), half.bytes))],
                vec![Loc::Held(held)],
            );
            out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
        }
        Ok(())
    }

    pub fn hook_far_load(
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

    pub fn hook_far_store(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
        value: Operand,
        pointer: Operand,
    ) -> Result<(), Unselected> {
        let regs = self.regs();
        let words = match self.far_words(value)? {
            Some(words) => words,
            None => self.far(value, m.at, out).map(|(offset, selector)| [Loc::Held(offset), Loc::Held(selector)])?,
        };
        let pointer = self.pointer(pointer)?;
        for (word, by) in words.into_iter().zip([0, 2]) {
            let what =
                semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(regs, pointer.moved(by), 2))], vec![word]);
            out.push(Arc::new(Insn { volatile: m.volatile, ..insn_of(m.at, what) }));
        }
        Ok(())
    }

    /// A field of an answer in registers: the register it came in.
    pub fn hook_extractvalue(
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

    /// `select c, a, b` as a conditional move: the flags of `c`, made here,
    /// then `cmov` of `a` over `b` into the result (X86's `CMOV`; the
    /// compare of a comparison nothing else reads is its flags, and any other
    /// condition is tested).
    pub fn hook_select(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let instruction = self.function.instruction(m.inst);
        let [condition, yes, no] = instruction.operands[..] else { return refuse("a select of three operands") };
        let ty = instruction.ty;
        let width = self.width(ty)?;
        let code = match condition {
            Operand::Value(value) if self.fused_compare(value).is_some() => {
                let compare = self.fused_compare(value).expect("a fused comparison");
                match self.compare(compare, m.at, out)? {
                    Test::One(code) => code,
                    _ => return refuse("a select on a float comparison"),
                }
            }
            _ => {
                let bit_ty = self.function.operand_type(&self.module.context, condition).expect("a typed condition");
                let held = self.held(condition, bit_ty, m.at, out)?;
                let test = semantics(Operation::Compare, "test", vec![], vec![Loc::Held(held), Loc::Held(held)]);
                out.push(insn(m.at, test));
                "jne"
            }
        };
        let (no, yes) = (self.held(no, ty, m.at, out)?, self.held(yes, ty, m.at, out)?);
        let result = Held { value: self.value(instruction.result.expect("a select's value")), width };
        let name = format!("cmov{}", &code[1..]);
        out.push(insn(
            m.at,
            semantics(Operation::Select, &name, vec![Loc::Held(result)], vec![Loc::Held(no), Loc::Held(yes)]),
        ));
        Ok(())
    }

    /// Whether an `alloca` takes its bytes from the stack when it runs (an
    /// element count that is a value).
    pub fn is_dynamic(
        &self,
        m: &Match,
    ) -> bool {
        self.alloca_bytes(m.inst).is_none()
    }

    /// `alloca T, %n`: the stack pointer lowered by `n * sizeof T` rounded up
    /// to the stack's alignment (and to the alloca's, where that is more),
    /// the new stack pointer being the address (X86's
    /// `LowerDYNAMIC_STACKALLOC`).
    pub fn hook_dynamic_alloca(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let instruction = self.function.instruction(m.inst);
        let Opcode::Alloca { allocated, align, .. } = instruction.opcode else { unreachable!("an alloca") };
        let element = self.layout.alloc_size(self.types(), allocated) as i64;
        let count = instruction.operands[0];
        let ty = self.function.operand_type(&self.module.context, count).expect("a typed count");
        let width = self.address_bytes();
        let held = self.held(count, ty, m.at, out)?;
        let count = self.extended(held, width, false, m.at, out);
        let at = m.at;
        let stack = Loc::Reg(llrm_lir::Reg { register: self.arch.stack_pointer(), width });
        let step = |this: &mut Self, name: &str, op: Operation, from: Held, with: i64, out: &mut Vec<Arc<Insn>>| {
            let into = this.fresh_held(width);
            let what = Imm { value: with, width, address: None };
            out.push(insn(at, semantics(op, name, vec![Loc::Held(into)], vec![Loc::Held(from), Loc::Imm(what)])));
            into
        };
        let bytes = if element.count_ones() == 1 {
            step(self, "shl", Operation::Binary, count, i64::from(element.trailing_zeros()), out)
        } else {
            step(self, "imul", Operation::Multiply, count, element, out)
        };
        let keep = self.arch.stack_alignment().max(align.map_or(1, |align| align as i64));
        let padded = step(self, "add", Operation::Binary, bytes, keep - 1, out);
        let rounded = step(self, "and", Operation::Binary, padded, -keep, out);
        out.push(insn(
            at,
            semantics(Operation::Binary, "sub", vec![stack.clone()], vec![stack.clone(), Loc::Held(rounded)]),
        ));
        let result = Held { value: self.value(instruction.result.expect("an address")), width };
        out.push(insn(at, semantics(Operation::Move, "mov", vec![Loc::Held(result)], vec![stack])));
        Ok(())
    }

    /// An aggregate with one field replaced: the fields of the aggregate it is
    /// made of, each in a register, and the new one. An undefined aggregate
    /// has fields nothing set, loaded as zero (a field that is never read is
    /// dead code).
    pub fn hook_insertvalue(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let instruction = self.function.instruction(m.inst);
        let Opcode::InsertValue(indices) = &instruction.opcode else { unreachable!("an insertvalue") };
        let &[index] = &indices[..] else { return refuse("an insertvalue into a nested aggregate") };
        let llrm_mir::types::Type::Struct { fields: types, .. } = self.types().get(instruction.ty).clone() else {
            return refuse("an insertvalue into other than a struct");
        };
        let mut fields = match instruction.operands[0] {
            Operand::Value(aggregate) => self.fields.get(&aggregate).cloned(),
            _ => None,
        }
        .unwrap_or_default();
        if fields.len() != types.len() {
            fields = Vec::new();
            for &ty in &types {
                let field = self.fresh_held(self.width(ty)?);
                let zero = Loc::Imm(Imm { value: 0, width: field.width, address: None });
                out.push(insn(m.at, semantics(Operation::Move, "mov", vec![Loc::Held(field)], vec![zero])));
                fields.push(field);
            }
        }
        let ty = instruction_field(&types, index)?;
        fields[index as usize] = self.held(instruction.operands[1], ty, m.at, out)?;
        let result = instruction.result.expect("an insertvalue's value");
        self.fields.insert(result, fields);
        Ok(())
    }

    /// SETcc, as LLVM selects a comparison it keeps as a value.
    pub fn hook_setcc(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let test = self.compare(m.inst, m.at, out)?;
        self.set_condition(m, test, out)
    }

    /// SETcc of the opposite condition for a `not` of a comparison, as
    /// DAGCombiner's `visitXOR` rewrites `(xor (setcc cc), 1)` to
    /// `(setcc !cc)`.
    pub fn hook_setcc_inverted(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let Operand::Value(operand) = self.function.instruction(m.inst).operands[0] else { unreachable!("a value") };
        let ValueDef::Instruction(compare) = self.function.value(operand).def else { unreachable!("a comparison") };
        self.inverted = true;
        let test = self.compare(compare, m.at, out)?;
        self.set_condition(m, test, out)
    }

    fn set_condition(
        &mut self,
        m: &Match,
        test: Test,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
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

    pub fn hook_br(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.branch(m.inst, m.block_at, m.at, out)
    }

    pub fn hook_ret(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.ret(m.inst, m.convention, m.at, out)
    }

    pub fn hook_call(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        let Opcode::Call(info) = self.opcode(m) else { unreachable!("a call") };
        super::in_the_frame(&info.argument_attrs)?;
        self.call(m.inst, info.calling_convention, m.at, out)
    }

    pub fn hook_invoke(
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

    pub fn hook_landing_pad(
        &mut self,
        m: &Match,
        out: &mut Vec<Arc<Insn>>,
    ) -> Result<(), Unselected> {
        self.landing_pad(m.inst, m.at, out)
    }
}

/// `isInvariant` of `SelectionDAGBuilder::visitLoad`: `!invariant.load`, or
/// `AA->pointsToConstantMemory`: a pointer into a global that is constant,
/// defined here and not externally initialized, or a `noalias readonly`
/// argument.
pub(crate) fn reads_invariant_memory(
    module: &Module,
    function: &Function,
    layout: &DataLayout,
    inst: InstId,
) -> bool {
    llrm_mir::memory::invariant_load(&module.context, layout, function, inst)
        || loads_constant_global(module, function, layout, inst)
}

fn loads_constant_global(
    module: &Module,
    function: &Function,
    layout: &DataLayout,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    let (Opcode::Load { volatile: false, .. }, Some(&pointer)) = (&instruction.opcode, instruction.operands.first())
    else {
        return false;
    };
    let query = llrm_mir::valuetracking::Query::new(module, function, layout);
    let Operand::Constant(object) = llrm_mir::valuetracking::underlying::get_underlying_object(&query, pointer, 0)
    else {
        return false;
    };
    let ConstantKind::Global(global) = module.context.get(object).kind else { return false };
    let global = module.global(global);
    matches!(
        &global.kind,
        llrm_mir::module::GlobalKind::Variable(variable)
            if variable.constant
                && variable.initializer.is_some()
                && !variable.externally_initialized
                && !matches!(
                    global.linkage,
                    llrm_mir::module::Linkage::Weak
                        | llrm_mir::module::Linkage::LinkOnce
                        | llrm_mir::module::Linkage::Common
                        | llrm_mir::module::Linkage::ExternWeak
                )
    )
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

// The operations `hook_wide_binary` was asked for, for the test that none of
// the chained ones are.
#[cfg(test)]
thread_local! {
    pub(crate) static HOOKED: std::cell::RefCell<Vec<llrm_mir::BinaryOp>> = const { std::cell::RefCell::new(Vec::new()) };
}
