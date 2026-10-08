//! SSA helpers, adapted from llrm-core's `analysis/ssa.rs` (a port of
//! `qbopt/analysis/ssa.py`).
//!
//! `use_index` returns [`InstId`]s where the old one returned snapshot-local
//! occurrences.  A substitution maps a [`ValueId`] to an [`Operand`], since a
//! rich MIR value may be replaced by a constant.
//!
//! `constructed` rebuilt SSA for a few of the old MIR's variables, each
//! defined in several places; [`SsaUpdater`] is that for one value, as
//! LLVM's SSAUpdater, and places the phis where its definitions meet. Its
//! `ConstructionError`, the old renamer's, has no counterpart.
//!
//! Skipped, with no meaning on SSA IR:
//! - `renumbered`: variable versions.
//! - `cloned_pointer_metadata` and `cloned_integer_ranges`: the old body's
//!   side tables; a pointer is its type here, and an instruction's metadata
//!   travels with `clone_instruction`.
//!
//! Tests skipped: `constructed_matches_blocks_by_identity_not_position` and
//! `constructed_repairs_existing_phi_inputs_by_predecessor` (test
//! `constructed`); the consumed mode of
//! `use_index_preserves_modes_filtering_and_operation_order` (carried merge
//! inputs do not exist, every operand is consumed); and the exits, cells,
//! known-memory and merge parts of the `substituted` tests (an address is an
//! operand here).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, Use, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::types::TypeId;
use llrm_mir::{Constant, ConstantKind, Context};

use crate::occurrence::{operations, phis};

/// A substitution followed a cycle.
///
/// Direct port of `qbopt.analysis.ssa:provider`'s
/// `ValueError("cyclic value substitution")`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubstitutionError {
    CyclicValueSubstitution,
}

impl fmt::Display for SubstitutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CyclicValueSubstitution => formatter.write_str("cyclic value substitution"),
        }
    }
}

impl std::error::Error for SubstitutionError {}

/// The local values an instruction reads, in operand order.
fn reads(instruction: &Instruction) -> impl Iterator<Item = ValueId> + '_ {
    instruction.operands.iter().filter_map(|operand| match operand {
        Operand::Value(value) => Some(*value),
        _ => None,
    })
}

/// Each value's operation users, built in one function traversal.
///
/// Users come in layout order; each operation occurs at most once in a
/// value's list.
///
/// Direct port of `qbopt.analysis.ssa:use_index`.
pub fn use_index(function: &Function, values: Option<&BTreeSet<ValueId>>) -> BTreeMap<ValueId, Vec<InstId>> {
    let mut users = BTreeMap::<ValueId, Vec<InstId>>::new();
    for (occurrence, _, operation) in operations(function) {
        let mut read: BTreeSet<ValueId> = reads(operation).collect();
        if let Some(wanted) = values {
            read.retain(|value| wanted.contains(value));
        }
        for value in read {
            users.entry(value).or_default().push(occurrence);
        }
    }
    users
}

/// Drop phis nothing needs, including cycles only other dead phis read.
/// Whether any went.
///
/// Direct port of `qbopt.analysis.ssa:pruned_phis`.
pub fn pruned_phis(function: &mut Function, roots: &BTreeSet<ValueId>) -> bool {
    let mut needed: BTreeSet<ValueId> = roots.clone();
    for (_, _, op) in operations(function) {
        needed.extend(reads(op));
    }
    let found: BTreeMap<ValueId, InstId> =
        phis(function).filter_map(|(inst, _, phi)| Some((phi.result?, inst))).collect();
    let mut pending: Vec<ValueId> = needed.iter().filter(|value| found.contains_key(value)).copied().collect();
    while let Some(next) = pending.pop() {
        let incoming: BTreeSet<ValueId> =
            reads(function.instruction(found[&next])).filter(|value| !needed.contains(value)).collect();
        needed.extend(incoming.iter().copied());
        pending.extend(incoming.into_iter().filter(|value| found.contains_key(value)));
    }
    let removed: Vec<InstId> = found.iter().filter(|(value, _)| !needed.contains(value)).map(|(_, &inst)| inst).collect();
    // Only removed phis read removed phis; drop those reads before erasing.
    for &inst in &removed {
        function.set_operands(inst, Vec::new());
    }
    for &inst in &removed {
        function.erase(inst).expect("only dead phis read a dead phi");
    }
    !removed.is_empty()
}

/// Follow a substitution until its provider is unchanged.
///
/// Direct port of `qbopt.analysis.ssa:provider`.
pub fn provider(value: Operand, swap: &BTreeMap<ValueId, Operand>) -> Result<Operand, SubstitutionError> {
    let mut value = value;
    let mut seen = BTreeSet::new();
    while let Operand::Value(id) = value
        && let Some(&replacement) = swap.get(&id)
    {
        if replacement == value {
            break;
        }
        if !seen.insert(id) {
            return Err(SubstitutionError::CyclicValueSubstitution);
        }
        value = replacement;
    }
    Ok(value)
}

/// An instruction's operands with each value read through `swap`.
///
/// The definition is deliberately not rewritten.  Direct port of
/// `qbopt.analysis.ssa:substituted`.
pub fn substituted(instruction: &Instruction, swap: &BTreeMap<ValueId, Operand>) -> Result<Vec<Operand>, SubstitutionError> {
    instruction.operands.iter().map(|&operand| provider(operand, swap)).collect()
}

/// Every value a function mentions, in layout order: each instruction's
/// result, then the values it reads.
///
/// Direct port of `qbopt.analysis.ssa:values`.
pub fn values(function: &Function) -> impl Iterator<Item = ValueId> + '_ {
    function.walk().flat_map(|(_, one)| {
        let instruction = function.instruction(one);
        instruction.result.into_iter().chain(reads(instruction))
    })
}

/// One value defined in several blocks, as LLVM's SSAUpdater: each
/// definition is available at the end of its block, and the value anywhere
/// else is what reaches it, a phi where definitions meet.
///
/// A phi whose inputs turn out to be one value is that value instead. A
/// block nothing defines and no path from a definition reaches reads
/// poison.
#[derive(Clone, Debug)]
pub struct SsaUpdater {
    ty: TypeId,
    name: Option<String>,
    available: BTreeMap<BlockId, Operand>,
    entered: BTreeMap<BlockId, Operand>,
    visiting: BTreeSet<BlockId>,
    revisiting: BTreeSet<BlockId>,
    inserted: Vec<InstId>,
}

impl SsaUpdater {
    /// A value of type `ty`; its phis are named `name`.
    pub fn new(ty: TypeId, name: Option<&str>) -> Self {
        Self {
            ty,
            name: name.map(str::to_owned),
            available: BTreeMap::new(),
            entered: BTreeMap::new(),
            visiting: BTreeSet::new(),
            revisiting: BTreeSet::new(),
            inserted: Vec::new(),
        }
    }

    /// `value` is the definition at the end of `block`.
    pub fn add_available_value(&mut self, block: BlockId, value: Operand) {
        self.available.insert(block, value);
        self.entered.clear();
    }

    pub fn has_value_for_block(&self, block: BlockId) -> bool {
        self.available.contains_key(&block)
    }

    /// The value at the end of `block`.
    pub fn value_at_end_of_block(&mut self, context: &mut Context, function: &mut Function, block: BlockId) -> Operand {
        match self.available.get(&block) {
            Some(&value) => value,
            None => self.value_in_middle_of_block(context, function, block),
        }
    }

    /// The value on entry to `block`, before any definition in it.
    pub fn value_in_middle_of_block(&mut self, context: &mut Context, function: &mut Function, block: BlockId) -> Operand {
        if let Some(&value) = self.entered.get(&block) {
            return value;
        }
        let predecessors = function.predecessors(block);
        if predecessors.len() == 1 {
            if !self.visiting.insert(block) {
                // Back round a cycle: through a join, the join's phi is
                // entered and ends the walk; a cycle of single predecessors
                // alone, met twice, is unreachable.
                if !self.revisiting.insert(block) {
                    return self.poison(context);
                }
                let value = self.value_at_end_of_block(context, function, predecessors[0]);
                self.revisiting.remove(&block);
                return value;
            }
            let value = self.value_at_end_of_block(context, function, predecessors[0]);
            self.visiting.remove(&block);
            self.entered.insert(block, value);
            return value;
        }
        if predecessors.is_empty() {
            let value = self.poison(context);
            self.entered.insert(block, value);
            return value;
        }
        let phi = function.create_instruction(Opcode::Phi, self.ty, Vec::new(), Flags::default(), self.name.as_deref());
        let first = function.block(block).instructions().first().copied();
        function.insert(phi, first.map_or(Position::End(block), Position::Before)).expect("a placed block");
        let result = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        self.entered.insert(block, result);
        self.inserted.push(phi);
        let mut operands = Vec::new();
        for predecessor in predecessors {
            let value = self.value_at_end_of_block(context, function, predecessor);
            // One input for each edge: a switch may reach the block by several.
            let edges = function.terminator(predecessor).map_or(1, |end| function.instruction(end).operands.iter().filter(|&&one| one == Operand::Block(block)).count().max(1));
            for _ in 0..edges {
                operands.push(value);
                operands.push(Operand::Block(predecessor));
            }
        }
        function.set_operands(phi, operands);
        let mut inputs = function.instruction(phi).operands.iter().step_by(2).copied().filter(|&one| one != result);
        let Some(single) = inputs.next() else {
            return self.replaced(function, phi, result, self.poison(context));
        };
        if inputs.all(|one| one == single) {
            return self.replaced(function, phi, result, single);
        }
        result
    }

    /// Makes `one` read the value where it reads: a phi's input at the end
    /// of the block it comes from.
    pub fn rewrite_use(&mut self, context: &mut Context, function: &mut Function, one: Use) {
        let user = function.instruction(one.user);
        let value = if user.opcode == Opcode::Phi {
            let Operand::Block(from) = user.operands[one.index as usize + 1] else { unreachable!("a phi's block operand") };
            self.value_at_end_of_block(context, function, from)
        } else {
            let block = function.parent(one.user).expect("a placed user");
            self.value_in_middle_of_block(context, function, block)
        };
        function.set_operand(one.user, one.index as usize, value);
    }

    /// The phis placed so far that stand.
    pub fn inserted_phis(&self) -> &[InstId] {
        &self.inserted
    }

    fn poison(&self, context: &mut Context) -> Operand {
        Operand::Constant(context.constant(Constant { ty: self.ty, kind: ConstantKind::Poison }))
    }

    /// `single` in place of the trivial `phi`.
    fn replaced(&mut self, function: &mut Function, phi: InstId, result: Operand, single: Operand) -> Operand {
        let Operand::Value(value) = result else { unreachable!("a phi's value") };
        function.replace_all_uses_with(value, single);
        function.set_operands(phi, Vec::new());
        function.erase(phi).expect("its uses were replaced");
        self.inserted.retain(|&one| one != phi);
        for entry in self.entered.values_mut().chain(self.available.values_mut()) {
            if *entry == result {
                *entry = single;
            }
        }
        single
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use llrm_mir::module::{Operand, ValueId};

    use llrm_mir::module::Use;

    use super::{SsaUpdater, SubstitutionError, operations, phis, provider, pruned_phis, substituted, use_index, values};
    use crate::testing::{block, function, parsed, value};

    #[test]
    fn test_unused_phi_cycles_are_pruned_but_real_dependencies_survive() {
        for (reader, line) in [
            ("none", ""),
            ("argument", "  call void @use(ptr %first)\n"),
            ("memory", "  %loaded = load i16, ptr %first\n"),
            ("root", ""),
        ] {
            let mut module = parsed(&format!(
                "declare void @use(ptr)

define void @f(ptr %seed, i1 %c) {{
b0:
  br label %b1

b1:
  %first = phi ptr [ %seed, %b0 ], [ %second, %b1 ]
  %second = phi ptr [ %seed, %b0 ], [ %first, %b1 ]
{line}  br i1 %c, label %b1, label %b2

b2:
  ret void
}}
"
            ));
            let roots = if reader == "root" {
                BTreeSet::from([value(function(&module, "f"), "first")])
            } else {
                BTreeSet::new()
            };
            let f = module.function_mut("f").unwrap().1;
            let changed = pruned_phis(f, &roots);
            assert_eq!(changed, reader == "none", "{reader}");
            assert_eq!(phis(f).count(), if reader == "none" { 0 } else { 2 }, "{reader}");
            assert!(f.check_uses().is_empty(), "{reader}: {:?}", f.check_uses());
        }
    }

    /// Direct Rust port of
    /// `tests/test_sccp.py:test_ssa_use_index_preserves_modes_filtering_and_operation_order`.
    #[test]
    fn use_index_preserves_modes_filtering_and_operation_order() {
        let module = parsed(
            "declare i16 @g(i16, i16, i16)

define void @f(i16 %source, i16 %carried, i16 %ignored) {
b0:
  %first = call i16 @g(i16 %source, i16 %source, i16 %carried)
  %second = add i16 %source, %ignored
  ret void
}
",
        );
        let function = function(&module, "f");
        let occurrences = operations(function).map(|(occurrence, _, _)| occurrence).collect::<Vec<_>>();
        let (source, carried) = (value(function, "source"), value(function, "carried"));
        let wanted = BTreeSet::from([source, carried]);

        assert_eq!(
            use_index(function, Some(&wanted)),
            BTreeMap::from([(source, vec![occurrences[0], occurrences[1]]), (carried, vec![occurrences[0]])])
        );
    }

    /// Direct Rust port of `tests/test_pointer_memory.py`'s substitution
    /// regression: a whole-pointer load retains the replacement address.
    #[test]
    fn substituted_rewrites_whole_pointer_and_known_memory_cells() {
        let module = parsed(
            "define i16 @f(ptr %pointer, ptr %replacement) {
b0:
  %value = load i16, ptr %pointer
  ret i16 %value
}
",
        );
        let function = function(&module, "f");
        let (load, _, instruction) = operations(function).next().unwrap();
        let replacement = Operand::Value(value(function, "replacement"));

        let changed = substituted(instruction, &BTreeMap::from([(value(function, "pointer"), replacement)])).unwrap();
        assert_eq!(changed, vec![replacement]);
        assert_eq!(function.instruction(load).result, Some(value(function, "value")));
    }

    #[test]
    fn substituted_rewrites_exactly_python_ssa_uses() {
        let module = parsed(
            "define i16 @f(i16 %old, i16 %middle, i16 %replacement) {
b0:
  %defined = add i16 %old, 7
  br label %b1

b1:
  %joined = phi i16 [ %middle, %b0 ]
  ret i16 %defined
}
",
        );
        let function = function(&module, "f");
        let [old, middle, replacement, defined] = ["old", "middle", "replacement", "defined"].map(|name| value(function, name));
        let swaps = BTreeMap::from([
            (old, Operand::Value(middle)),
            (middle, Operand::Value(replacement)),
            (defined, Operand::Value(old)),
        ]);
        let (add, _, instruction) = operations(function).next().unwrap();
        let (_, _, phi) = phis(function).next().unwrap();

        let changed = substituted(instruction, &swaps).unwrap();
        assert_eq!(changed[0], Operand::Value(replacement));
        assert_eq!(changed[1], instruction.operands[1]);
        assert_eq!(function.instruction(add).result, Some(defined));
        let arms = substituted(phi, &swaps).unwrap();
        assert_eq!(arms, vec![Operand::Value(replacement), phi.operands[1]]);
        assert_eq!(substituted(instruction, &BTreeMap::new()).unwrap(), instruction.operands);
    }

    #[test]
    fn provider_follows_transitively_and_reports_cycles() {
        let [first, second, third] = [1, 2, 3].map(ValueId);
        let value = |one| Operand::Value(one);
        assert_eq!(
            provider(value(first), &BTreeMap::from([(first, value(second)), (second, value(third))])).unwrap(),
            value(third)
        );
        assert_eq!(
            provider(value(first), &BTreeMap::from([(first, value(second)), (second, value(first))])),
            Err(SubstitutionError::CyclicValueSubstitution)
        );
        assert_eq!(provider(value(first), &BTreeMap::from([(first, value(first))])).unwrap(), value(first));
    }

    #[test]
    fn values_preserves_body_order_and_duplicates() {
        let module = parsed(
            "define i16 @f(i16 %used, i16 %exited) {
b0:
  %defined = add i16 %used, %exited
  %second = add i16 %defined, %used
  br label %b1

b1:
  %phi_result = phi i16 [ %used, %b0 ]
  %third = add i16 %exited, %used
  ret i16 %defined
}
",
        );
        let function = function(&module, "f");
        let [used, exited, defined, second, phi_result, third] =
            ["used", "exited", "defined", "second", "phi_result", "third"].map(|name| value(function, name));

        assert_eq!(
            values(function).collect::<Vec<_>>(),
            vec![defined, used, exited, second, defined, used, phi_result, used, third, exited, used, defined]
        );
    }

    #[test]
    fn a_phi_read_only_by_a_live_phi_survives_pruning() {
        let mut module = parsed(
            "define i16 @f(i1 %c, i16 %x) {
b0:
  br label %b1

b1:
  %inner = phi i16 [ %x, %b0 ], [ %inner, %b1 ]
  br i1 %c, label %b1, label %b2

b2:
  %outer = phi i16 [ %inner, %b1 ]
  %dead = phi i16 [ %inner, %b1 ]
  ret i16 %outer
}
",
        );
        let f = module.function_mut("f").unwrap().1;
        assert!(pruned_phis(f, &BTreeSet::new()));
        let names = phis(f).map(|(_, _, phi)| f.value(phi.result.unwrap()).name.clone().unwrap()).collect::<Vec<_>>();
        assert_eq!(names, ["inner", "outer"]);
        assert!(f.check_uses().is_empty());
    }

    #[test]
    fn a_body_without_phis_is_left_unpruned() {
        let mut module = parsed(
            "define i16 @f(i16 %x) {
b0:
  %y = add i16 %x, 1
  ret i16 %y
}
",
        );
        let f = module.function_mut("f").unwrap().1;
        let before = f.clone();
        assert!(!pruned_phis(f, &BTreeSet::new()));
        assert_eq!(*f, before);
    }

    #[test]
    fn use_index_lists_a_repeated_reader_once_and_counts_terminators_but_not_phis() {
        let module = parsed(
            "define i16 @f(i16 %x) {
b0:
  %y = mul i16 %x, %x
  br label %b1

b1:
  %p = phi i16 [ %x, %b0 ]
  ret i16 %y
}
",
        );
        let f = function(&module, "f");
        let ops = operations(f).map(|(inst, _, _)| inst).collect::<Vec<_>>();
        let (x, y) = (value(f, "x"), value(f, "y"));
        assert_eq!(use_index(f, None), BTreeMap::from([(x, vec![ops[0]]), (y, vec![ops[2]])]));
    }

    #[test]
    fn a_value_substituted_by_a_constant_reads_the_constant() {
        let mut module = parsed(
            "define i16 @f(i16 %x) {
b0:
  %y = add i16 %x, %x
  ret i16 %y
}
",
        );
        let i16 = module.context.types.int(16);
        let seven = module.context.int(i16, 7);
        let f = function(&module, "f");
        let (_, _, add) = operations(f).next().unwrap();
        let swap = BTreeMap::from([(value(f, "x"), Operand::Constant(seven))]);
        assert_eq!(substituted(add, &swap).unwrap(), vec![Operand::Constant(seven); 2]);
    }

    /// @f of `text` with the value `name` reads rewritten as the updater
    /// given `defined` (block, value) says, and the phis it placed.
    fn updated(text: &str, reader: &str, defined: &[(&str, &str)]) -> (String, usize) {
        let mut module = parsed(text);
        let f = function(&module, "f");
        let definitions = defined.iter().map(|(at, name)| (block(f, at), Operand::Value(value(f, name)))).collect::<Vec<_>>();
        let reader = value(f, reader);
        let user = f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(reader)).expect("defined");
        let ty = f.value(reader).ty;
        let (context, f) = module.function_mut("f").unwrap();
        let mut updater = SsaUpdater::new(ty, Some("v"));
        for (at, one) in definitions {
            updater.add_available_value(at, one);
        }
        updater.rewrite_use(context, f, Use { user, index: 0 });
        assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
        let placed = updater.inserted_phis().len();
        (llrm_mir::print::module(&module), placed)
    }

    #[test]
    fn a_value_defined_before_a_loop_and_in_its_latch_meets_in_a_header_phi() {
        let (text, placed) = updated(
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  %u = add i16 %x, 0
  br i1 %c, label %b2, label %b3

b2:
  %y = add i16 %x, 1
  br label %b1

b3:
  ret i16 %u
}
",
            "u",
            &[("b0", "x"), ("b2", "y")],
        );
        assert_eq!(placed, 1);
        assert!(text.contains("b1:\n  %v = phi i16 [ %x, %b0 ], [ %y, %b2 ]\n  %u = add i16 %v, 0\n"), "{text}");
    }

    /// Rotating nbody's outer loop read its counter after the inner one as
    /// a phi of poison: a walk out of a single-predecessor block came back
    /// to it round the loop and called the cycle unreachable.
    #[test]
    fn a_value_defined_before_a_loop_reaches_its_body_round_the_back_edge() {
        let (text, placed) = updated(
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  br label %b2

b2:
  %u = add i16 %x, 0
  br i1 %c, label %b1, label %b3

b3:
  ret i16 %u
}
",
            "u",
            &[("b0", "x")],
        );
        assert_eq!(placed, 0, "{text}");
        assert!(text.contains("%u = add i16 %x, 0"), "{text}");
    }

    const DIAMOND: &str = "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %a = add i16 %x, 1
  br label %b3

b2:
  %b = add i16 %x, 2
  br label %b3

b3:
  %r = add i16 %x, 0
  ret i16 %r
}
";

    #[test]
    fn a_join_takes_each_arms_definition_or_what_reaches_it() {
        let (text, placed) = updated(DIAMOND, "r", &[("b0", "x"), ("b1", "a")]);
        assert_eq!(placed, 1);
        assert!(text.contains("b3:\n  %v = phi i16 [ %a, %b1 ], [ %x, %b2 ]\n  %r = add i16 %v, 0\n"), "{text}");
    }

    #[test]
    fn one_value_on_every_edge_needs_no_phi() {
        let (text, placed) = updated(DIAMOND, "r", &[("b1", "x"), ("b2", "x")]);
        assert_eq!(placed, 0);
        assert!(text.contains("b3:\n  %r = add i16 %x, 0\n"), "{text}");
    }

    #[test]
    fn an_edge_no_definition_reaches_brings_poison() {
        let (text, placed) = updated(DIAMOND, "r", &[("b1", "a")]);
        assert_eq!(placed, 1);
        assert!(text.contains("%v = phi i16 [ %a, %b1 ], [ poison, %b2 ]"), "{text}");
    }

    #[test]
    fn a_phi_input_reads_the_value_at_the_end_of_its_edge() {
        let (text, placed) = updated(
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %a = add i16 %x, 1
  br label %b2

b2:
  %e = phi i16 [ %x, %b1 ], [ %x, %b0 ]
  ret i16 %e
}
",
            "e",
            &[("b0", "x"), ("b1", "a")],
        );
        assert_eq!(placed, 0);
        assert!(text.contains("%e = phi i16 [ %a, %b1 ], [ %x, %b0 ]"), "{text}");
    }
}
