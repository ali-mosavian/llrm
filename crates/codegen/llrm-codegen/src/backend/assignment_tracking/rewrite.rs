//! The table back as records: what the analysis says replaces what the
//! function said (`AssignmentTrackingPass::runOnFunction` ends by deleting
//! the `#dbg_assign`s, and `FunctionVarLocs` is what `SelectionDAG` reads).
//!
//! A single location becomes a `Declare` ahead of the first instruction; a
//! wedge becomes a `Value` (`Args` for several values, `Gone` for none)
//! ahead of its instruction, in the table's order. O(n) in locations.

use std::collections::HashMap;

use llrm_mir::{DebugRecord, DebugWhat, InstId, MetadataId, MetadataNode, MetadataOperand, Module, module::Special};

use super::{FunctionVarLocs, VarLocInfo, expression_text, op_words};

/// Replaces the records of `name` with those of `locs`. Nothing changes, and
/// `false` comes back, when an expression has an operation the table does not
/// keep (`DwOp::Other`): the records would say less than they did.
pub fn rewrite(
    module: &mut Module,
    name: &str,
    locs: &FunctionVarLocs,
) -> bool {
    let all: Vec<&VarLocInfo> =
        locs.single_locs().iter().chain(locs.instructions().flat_map(|i| locs.locs_before(i))).collect();
    if all.iter().any(|one| one.expression.iter().any(|op| op_words(op).is_none())) {
        return false;
    }
    let mut minted: HashMap<String, MetadataId> = HashMap::new();
    let mut expression = |module: &mut Module, one: &VarLocInfo| -> Option<MetadataId> {
        if one.expression.is_empty() {
            return None;
        }
        Some(*minted.entry(expression_text(&one.expression)).or_insert_with(|| {
            let i64 = module.context.types.int(64);
            let mut operands = Vec::new();
            for op in &one.expression {
                let (word, numbers) = op_words(op).expect("checked above");
                operands.push(MetadataOperand::Ident(word.to_owned()));
                operands.extend(numbers.iter().map(|&n| MetadataOperand::Constant(module.context.int(i64, n))));
            }
            let id = MetadataId(module.metadata.len() as u32);
            module.metadata.push(MetadataNode { distinct: false, operands });
            module.special.insert(id, Special { kind: "DIExpression".to_owned(), fields: Vec::new() });
            id
        }))
    };
    let Some((_, function)) = module.function_mut(name) else { return false };
    let first = function.walk().next().map(|(_, inst)| inst);
    let mut wanted: Vec<(InstId, bool, &VarLocInfo)> = Vec::new();
    if let Some(first) = first {
        wanted.extend(locs.single_locs().iter().map(|one| (first, true, one)));
    }
    for inst in locs.instructions() {
        wanted.extend(locs.locs_before(inst).iter().map(|one| (inst, false, one)));
    }
    let expressions: Vec<Option<MetadataId>> = wanted.iter().map(|&(_, _, one)| expression(module, one)).collect();
    let (_, function) = module.function_mut(name).expect("the function the table is of");
    let mut records = Vec::with_capacity(wanted.len());
    for ((before, single, one), expression) in wanted.into_iter().zip(expressions) {
        let what = match (&one.values[..], single) {
            ([], _) => DebugWhat::Gone,
            ([address], true) => DebugWhat::Declare(*address),
            ([value], false) => DebugWhat::Value(*value),
            (several, _) => DebugWhat::Args(function.add_debug_args(several.to_vec())),
        };
        records.push(DebugRecord {
            before,
            variable: locs.variable(one.variable).variable,
            what,
            expression,
            location: one.location,
        });
    }
    function.replace_debug_records(records);
    true
}
