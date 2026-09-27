//! SSA helpers, adapted from llrm-core's `analysis/ssa.rs` (a port of
//! `qbopt/analysis/ssa.py`).
//!
//! `use_index` returns [`InstId`]s where the old one returned snapshot-local
//! occurrences.  A substitution maps a [`ValueId`] to an [`Operand`], since a
//! rich MIR value may be replaced by a constant.
//!
//! Skipped, with no meaning on SSA IR:
//! - `constructed` and `ConstructionError`: SSA reconstruction over the old
//!   MIR's variables.
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

use llrm_mir::module::{Function, InstId, Instruction, Operand, ValueId};

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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use llrm_mir::module::{Operand, ValueId};

    use super::{SubstitutionError, operations, phis, provider, pruned_phis, substituted, use_index, values};
    use crate::testing::{function, parsed, value};

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
}
