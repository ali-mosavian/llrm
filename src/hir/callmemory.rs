//! Port of `qbopt/hir/callmemory.py`: whole-module call memory effects for
//! source-neutral HIR lowering.

use crate::support::hash::IndexMap;

use crate::analysis::alias::{self, Actual};
use crate::hir::lower::Lowered;
use crate::hir::model;
use crate::model::mir;

/// Instantiate every defined callee's parameter effects at each call.
///
/// HIR call operands stay in source-parameter order.  That makes this the
/// common boundary where all source frontends can compute mod/ref effects,
/// before ABI physicalization chooses stack order or register locations.
///
/// `object_name` defaults to the identity.
pub fn annotated(
    module: &model::Module,
    functions: &[model::Function],
    semantic: &[Lowered],
    object_name: Option<&dyn Fn(&str) -> String>,
) -> Result<Vec<Lowered>, String> {
    let identity = |name: &str| name.to_owned();
    let object_name: &dyn Fn(&str) -> String = object_name.unwrap_or(&identity);
    assert_eq!(functions.len(), semantic.len(), "zip(strict=True)");
    let types: IndexMap<i64, &model::Type> = module.types.iter().map(|one| (one.id, one)).collect();
    let callables: IndexMap<i64, &model::Callable> = module.callables.iter().map(|one| (one.id, one)).collect();
    let mut procedures: IndexMap<String, alias::Procedure> = IndexMap::default();
    let named = crate::hir::lower::named_externals(module);
    let mut lowered_by_name: IndexMap<String, Lowered> = IndexMap::default();
    for (function, lowered) in functions.iter().zip(semantic) {
        let body = alias::annotated(&std::rc::Rc::new(lowered.body.clone()))?;
        let mut calls: IndexMap<i64, String> = IndexMap::default();
        let mut arguments: IndexMap<i64, Vec<Actual>> = IndexMap::default();
        let abi_sites: Vec<i64> = function.calls.iter().map(|site| site.instruction).collect();
        let instructions: IndexMap<i64, &model::Instruction> = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| abi_sites.contains(&instruction.id))
            .map(|instruction| (instruction.id, instruction))
            .collect();
        let call_ops: IndexMap<Option<u32>, &mir::Op> = body
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter(|operation| operation.kind == mir::Kind::Call)
            .map(|operation| (operation.id, operation))
            .collect();
        let value_types: IndexMap<i64, &model::Type> =
            function.values.iter().map(|one| (one.id, types[&one.r#type])).collect();
        let actuals = |instruction: &model::Instruction| -> Vec<Actual> {
            instruction
                .operands
                .iter()
                .map(|operand| match operand {
                    model::Operand::ValueRef(one) if value_types[&one.value].kind == model::TypeKind::Pointer => {
                        Actual::Pointer(lowered.values[&one.value], 0)
                    }
                    _ => Actual::Absent,
                })
                .collect()
        };
        // An inline block that declares memory reaches what an unknown
        // routine would, its pointer inputs' objects among it; one that does
        // not declares none.
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let (Some(asm), Some(operation)) = (&instruction.asm, call_ops.get(&Some(instruction.id as u32))) {
                if !asm.memory {
                    calls.insert(operation.at, crate::hir::lower::ASM.to_owned());
                }
                arguments.insert(operation.at, actuals(instruction));
            }
        }
        for site in &function.calls {
            let operation = call_ops.get(&Some(site.instruction as u32));
            let instruction = instructions.get(&site.instruction);
            let (Some(operation), Some(instruction)) = (operation, instruction) else {
                return Err(format!("{}: call {} did not survive HIR lowering", function.name, site.instruction));
            };
            let target = match site.callee {
                Some(callee) => callables[&callee].name.as_str(),
                None => operation.name.as_str(),
            };
            calls.insert(operation.at, object_name(target));
            arguments.insert(operation.at, actuals(instruction));
        }
        let name = object_name(&function.name);
        let procedure = alias::Procedure { body: body.clone(), calls, arguments, named: named.clone() };
        procedures.insert(name.clone(), procedure);
        lowered_by_name.insert(name, Lowered { body, ..lowered.clone() });
    }
    let known = IndexMap::from_iter([(crate::hir::lower::ASM.to_owned(), alias::Summary::default())]);
    let summaries = alias::summaries(&procedures, Some(&known))?;
    functions
        .iter()
        .map(|function| {
            let name = object_name(&function.name);
            Ok(Lowered { body: alias::calls_annotated(&procedures[&name], &summaries)?, ..lowered_by_name[&name].clone() })
        })
        .collect()
}
