//! `AssignmentTrackingPass`: the `#dbg_declare`s of stack variables become
//! `#dbg_assign`s, one per store-like instruction that writes the variable's
//! alloca (`trackAssignments`), and the declares go.
//!
//! O(instructions + records): one scan collects the variables of each alloca,
//! one the store-likes, and the records are rebuilt once.

use std::collections::HashMap;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::valuetracking::Query;
use llrm_mir::{
    Attribute, ConstantKind, DebugRecord, DebugWhat, DwOp, GlobalId, InstId, MetadataId, MetadataNode, Module, Opcode,
    Operand, module::Special,
};

use super::rewrite::Expressions;
use super::{assignment_info, variable_size_in_bits};

/// A variable of an alloca and where its declare stood (`VarRecord`).
type VarRecord = (MetadataId, Option<MetadataId>);

/// `AssignmentTrackingPass::run(Module)`: every function but those marked
/// `optnone`. True when something changed.
pub fn tracked(module: &mut Module) -> Result<bool, String> {
    let Some(text) = module.datalayout.clone() else { return Ok(false) };
    let layout = DataLayout::parse(&text)?;
    let ids: Vec<GlobalId> = module
        .functions()
        .filter(|(_, _, function)| !function.attrs.contains(&Attribute::Flag("optnone".to_owned())))
        .map(|(id, ..)| id)
        .collect();
    let mut expressions = Expressions::default();
    let mut changed = false;
    for id in ids {
        changed |= function_tracked(module, id, &layout, &mut expressions);
    }
    Ok(changed)
}

fn function_tracked(
    module: &mut Module,
    at: GlobalId,
    layout: &DataLayout,
    expressions: &mut Expressions,
) -> bool {
    let function = module.global(at).function().expect("a function");
    // The variables of each static alloca, and the declares they replace.
    let mut vars: HashMap<InstId, Vec<VarRecord>> = HashMap::new();
    let mut declares: Vec<usize> = Vec::new();
    for (index, record) in function.debug_records().iter().enumerate() {
        let DebugWhat::Declare(Operand::Value(address)) = record.what else { continue };
        // A declare with operations (an offset, a fragment) stays.
        if record.expression.is_some_and(|node| module.expression(node).is_none_or(|ops| !ops.is_empty())) {
            continue;
        }
        let llrm_mir::ValueDef::Instruction(alloca) = function.value(address).def else { continue };
        if !static_alloca(function, alloca) {
            continue;
        }
        let one = (record.variable, record.location);
        let list = vars.entry(alloca).or_default();
        if !list.contains(&one) {
            list.push(one);
        }
        declares.push(index);
    }
    if vars.is_empty() {
        return false;
    }
    let q = Query::new(module, function, layout);
    // `trackAssignments`: what each store-like instruction writes, in order.
    struct Write {
        inst: InstId,
        alloca: InstId,
        offset: u64,
        size: u64,
        value: Option<Operand>,
        dest: Operand,
        next: InstId,
    }
    let mut writes: Vec<Write> = Vec::new();
    for &block in function.layout() {
        let insts: Vec<InstId> = function.iter_block(block).collect();
        for (at, &inst) in insts.iter().enumerate() {
            let instruction = function.instruction(inst);
            let found = match &instruction.opcode {
                Opcode::Alloca { allocated, .. } => vars.contains_key(&inst).then(|| {
                    let result = Operand::Value(instruction.result.expect("an alloca has a result"));
                    (inst, 0, layout.size_bits(&module.context.types, *allocated), None, result)
                }),
                Opcode::Store { .. } => assignment_info(&q, inst).map(|info| {
                    let dest = instruction.operands[1];
                    (info.alloca, info.offset_in_bits, info.size_in_bits, Some(instruction.operands[0]), dest)
                }),
                Opcode::Call(_) => assignment_info(&q, inst).map(|info| {
                    // A `memset` of zero says zero; the others and any other
                    // fill, nothing.
                    let value = match instruction.operands.get(1) {
                        Some(&Operand::Constant(c)) if is_memset(&q, inst) && is_zero(&module.context.get(c).kind) => {
                            Some(Operand::Constant(c))
                        }
                        _ => None,
                    };
                    (info.alloca, info.offset_in_bits, info.size_in_bits, value, instruction.operands[0])
                }),
                _ => None,
            };
            let Some((alloca, offset, size, value, dest)) = found else { continue };
            if !vars.contains_key(&alloca) {
                continue;
            }
            // A terminator is never a store-like: something follows.
            let next = insts[at + 1];
            writes.push(Write { inst, alloca, offset, size, value, dest, next });
        }
    }
    // The alloca's size, for a variable of unknown size.
    let allocated_bits = |alloca: InstId| match function.instruction(alloca).opcode {
        Opcode::Alloca { allocated, .. } => layout.size_bits(&module.context.types, allocated),
        _ => unreachable!("an alloca"),
    };
    // One plan per record to make, then the module is edited.
    struct Make {
        write: usize,
        variable: MetadataId,
        location: Option<MetadataId>,
        fragment: Option<(u64, u64)>,
    }
    let mut makes: Vec<Make> = Vec::new();
    for (index, write) in writes.iter().enumerate() {
        for &(variable, location) in &vars[&write.alloca] {
            let start = write.offset;
            let mut end = write.offset + write.size;
            let mut whole = start == 0 && write.size == allocated_bits(write.alloca);
            if let Some(size) = variable_size_in_bits(module, variable) {
                end = end.min(size);
                if start >= end {
                    continue;
                }
                whole = start == 0 && end >= size;
            }
            makes.push(Make { write: index, variable, location, fragment: (!whole).then_some((start, end - start)) });
        }
    }
    let ids: Vec<(InstId, Option<MetadataId>)> = writes
        .iter()
        .map(|write| {
            let existing = function.instruction(write.inst).metadata.iter().find(|(kind, _)| kind == "DIAssignID");
            (write.inst, existing.map(|&(_, id)| id))
        })
        .collect();
    let writes_info: Vec<(Option<Operand>, Operand, InstId)> =
        writes.iter().map(|w| (w.value, w.dest, w.next)).collect();
    let _ = q;

    // Edit: an id for each store-like that lacks one, the expressions, the
    // records.
    let empty = expressions.make(module, &[]);
    let mut assign_ids: Vec<MetadataId> = Vec::with_capacity(ids.len());
    for (_, existing) in &ids {
        assign_ids.push(existing.unwrap_or_else(|| {
            let id = MetadataId(module.metadata.len() as u32);
            module.metadata.push(MetadataNode { distinct: true, operands: Vec::new() });
            module.special.insert(id, Special { kind: "DIAssignID".to_owned(), fields: Vec::new() });
            id
        }));
    }
    let fragments: Vec<Option<MetadataId>> = makes
        .iter()
        .map(|make| make.fragment.map(|(offset, size)| expressions.make(module, &[DwOp::Fragment { offset, size }])))
        .collect();
    let llrm_mir::GlobalKind::Function(function) = &mut module.globals[at.0 as usize].kind else { unreachable!() };
    for ((inst, existing), id) in ids.iter().zip(&assign_ids) {
        if existing.is_none() {
            function.set_metadata(*inst, "DIAssignID", Some(*id));
        }
    }
    // Each new record stands first before the instruction after its store,
    // the later ones ahead of the earlier (`insertDbgRecordAfter`).
    let mut ahead: HashMap<InstId, Vec<DebugRecord>> = HashMap::new();
    for (make, expression) in makes.iter().zip(fragments) {
        let (value, dest, next) = writes_info[make.write];
        ahead
            .entry(next)
            .or_default()
            .push(
                DebugRecord {
                    before: next,
                    variable: make.variable,
                    what: DebugWhat::Assign {
                        value,
                        address: Some(dest),
                        id: assign_ids[make.write],
                        address_expression: empty,
                    },
                    expression,
                    location: make.location,
                },
            );
    }
    let mut records: Vec<DebugRecord> = Vec::new();
    for mut list in ahead.into_values() {
        list.reverse();
        records.extend(list);
    }
    let gone: std::collections::HashSet<usize> = declares.into_iter().collect();
    records.extend(
        function.debug_records().iter().enumerate().filter(|(index, _)| !gone.contains(index)).map(|(_, one)| *one),
    );
    function.replace_debug_records(records);
    true
}

fn is_memset(
    q: &Query,
    inst: InstId,
) -> bool {
    use llrm_mir::valuetracking::pattern::IntrinsicId;
    llrm_mir::memory::callee(q.context, q.function, inst)
        .and_then(|callee| q.globals.get(callee.0 as usize)?.name.intrinsic_id())
        == Some(IntrinsicId::Memset)
}

fn is_zero(kind: &ConstantKind) -> bool {
    matches!(kind, ConstantKind::Zero | ConstantKind::Int(0))
}

/// `AllocaInst::isStaticAlloca`: a constant size, in the entry block.
fn static_alloca(
    function: &llrm_mir::Function,
    inst: InstId,
) -> bool {
    let sized = function.instruction(inst).operands.first().is_none_or(|count| matches!(count, Operand::Constant(_)));
    sized && function.layout().first().is_some_and(|&entry| function.parent(inst) == Some(entry))
}
