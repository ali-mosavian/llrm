//! The table back as records: what the analysis says replaces what the
//! function said (`AssignmentTrackingPass::runOnFunction` ends by deleting
//! the `#dbg_assign`s, and `FunctionVarLocs` is what `SelectionDAG` reads).
//!
//! A single location becomes a `Declare` ahead of the first instruction; a
//! wedge becomes a `Value` (`Args` for several values, `Gone` for none)
//! ahead of its instruction, in the table's order. O(n) in locations.
//!
//! DEVIATE: a wedge that says a variable is in its alloca's cell
//! (`DW_OP_deref` of a static alloca) is stated as a `Declare` of that alloca,
//! once, ahead of the first instruction: the selectors read a variable's
//! frame cell from its `Declare` and do not read memory wedges. Exit: the
//! selectors read wedges (`DBG_VALUE` through selection and allocation).

use llrm_mir::DwOp;
use llrm_mir::{
    DebugRecord, DebugWhat, GlobalId, GlobalKind, InstId, MetadataId, MetadataNode, MetadataOperand, Module, Operand,
    module::Special,
};

use super::track::static_alloca;
use super::{FunctionVarLocs, VarLocInfo, VariableId, expression_text, op_words};
use crate::support::hash::{HashMap, HashSet};

/// The `!DIExpression` nodes made so far, one per distinct text.
#[derive(Default)]
pub(super) struct Expressions {
    by_text: HashMap<String, MetadataId>,
}

impl Expressions {
    /// The node of `ops`, none for no operations (a record's empty expression
    /// is no node). `ops` hold no [`DwOp::Other`].
    pub(super) fn node(
        &mut self,
        module: &mut Module,
        ops: &[DwOp],
    ) -> Option<MetadataId> {
        if ops.is_empty() {
            return None;
        }
        Some(self.make(module, ops))
    }

    /// As [`node`](Self::node), a node for no operations too.
    pub(super) fn make(
        &mut self,
        module: &mut Module,
        ops: &[DwOp],
    ) -> MetadataId {
        *self
            .by_text
            .entry(expression_text(ops))
            .or_insert_with(
                || {
                    let i64 = module.context.types.int(64);
                    let mut operands = Vec::new();
                    for op in ops {
                        let (word, numbers) = op_words(op).expect("no DwOp::Other");
                        operands.push(MetadataOperand::Ident(word.to_owned()));
                        operands.extend(numbers.iter().map(|&n| MetadataOperand::Constant(module.context.int(i64, n))));
                    }
                    let id = MetadataId(module.metadata.len() as u32);
                    module.metadata.push(MetadataNode { distinct: false, operands });
                    module.special.insert(id, Special { kind: "DIExpression".to_owned(), fields: Vec::new() });
                    id
                },
            )
    }
}

/// Replaces the records of the function `at` with those of `locs`. Nothing
/// changes, and `false` comes back, when an expression has an operation the
/// table does not keep (`DwOp::Other`): the records would say less than they
/// did.
pub fn rewrite(
    module: &mut Module,
    at: GlobalId,
    locs: &FunctionVarLocs,
) -> bool {
    let all: Vec<&VarLocInfo> =
        locs.single_locs().iter().chain(locs.wedged().flat_map(|i| locs.locs_before(i))).collect();
    if all.iter().any(|one| one.expression.iter().any(|op| op_words(op).is_none())) {
        return false;
    }
    let mut minted = Expressions::default();
    let Some(function) = module.global(at).function() else { return false };
    let first = function.walk().next().map(|(_, inst)| inst);
    let mut wanted: Vec<(InstId, bool, &VarLocInfo)> = Vec::new();
    // The variables stated in a cell, for the `Declare` each gets once.
    let mut homed: HashSet<(VariableId, Operand)> = HashSet::default();
    let in_cell = |one: &VarLocInfo| match (&one.expression[..], &one.values[..]) {
        ([DwOp::Deref], [address @ Operand::Value(value)]) => match function.value(*value).def {
            llrm_mir::ValueDef::Instruction(def) if static_alloca(function, def) => Some(*address),
            _ => None,
        },
        _ => None,
    };
    if let Some(first) = first {
        wanted.extend(locs.single_locs().iter().map(|one| (first, true, one)));
    }
    for inst in locs.wedged() {
        for one in locs.locs_before(inst) {
            match (in_cell(one), first) {
                (Some(address), Some(first)) => {
                    if homed.insert((one.variable, address)) {
                        wanted.push((first, true, one));
                    }
                }
                _ => wanted.push((inst, false, one)),
            }
        }
    }
    let expressions: Vec<Option<MetadataId>> = wanted
        .iter()
        .map(|&(_, single, one)| {
            // The cell's `Declare` is of the address itself.
            let ops = if single && one.expression == [DwOp::Deref] { &[][..] } else { &one.expression[..] };
            minted.node(module, ops)
        })
        .collect();
    let GlobalKind::Function(function) = &mut module.globals[at.0 as usize].kind else { unreachable!("checked above") };
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
